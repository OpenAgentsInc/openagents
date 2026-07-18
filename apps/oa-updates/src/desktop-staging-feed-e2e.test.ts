/**
 * REL-FEED-01 (#8993) — staging-channel end-to-end proof.
 *
 * Runs a LOCAL instance of the oa-updates ReleaseSet v2 feed (the real
 * `createReleaseSetFeed` routing served over a real `node:http` socket, the
 * production code path minus Cloud Storage/TLS) and drives the REAL Desktop
 * update client (`openDesktopUpdateStagingHost` + the REL-FEED-01 feed-config
 * resolver, exactly as Electron main wires them) through the complete cycle:
 *
 *   env override discovery -> pointer/alias fetch -> pinned Ed25519 verify ->
 *   native target selection -> digest-gated staging -> apply -> first-launch
 *   receipt confirmation -> retained-slot rollback -> feed pointer rollback.
 *
 * Trust: throwaway Ed25519 keys generated INSIDE this test. The production
 * private key never appears; the production PUBLIC pin appears only to prove
 * a production-pinned client REFUSES this staging feed (kid_not_pinned).
 * The only stand-in for production transport is TLS termination: artifact
 * URLs are `https://staging-artifacts.test/...` inside the signed set (the
 * contract refuses non-HTTPS artifact URLs) and the injected fetch maps that
 * host onto the local HTTP listener — bytes still cross a real socket and
 * are still admitted only through the signed sha256/byteLength gates.
 */
import { createHash, generateKeyPairSync } from "node:crypto"
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { createServer, type Server } from "node:http"
import { tmpdir } from "node:os"
import path from "node:path"

import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"

import {
  canonicalizeReleaseSet,
  decodeReleaseSet,
  type ReleaseSet,
} from "../../openagents-desktop/src/release-set-contract.ts"
import {
  deriveReleaseKeyPin,
  signReleasePayload,
  type ReleaseSigningKey,
} from "../../openagents-desktop/src/release-publish.ts"
import { PRODUCTION_RELEASE_KEY_PIN } from "../../openagents-desktop/src/update-contract.ts"
import {
  DESKTOP_UPDATE_FEED_BASE_URL_ENV,
  DESKTOP_UPDATE_FEED_STAGING_PIN_ENV,
  resolveDesktopUpdateFeedConfig,
} from "../../openagents-desktop/src/update-feed-config.ts"
import { openDesktopUpdateStagingHost } from "../../openagents-desktop/src/update-staging-host.ts"
import { childRuntimeKinds } from "../../openagents-desktop/src/update-platform-applier.ts"
import {
  createInMemoryReleaseSetFeedStore,
  createReleaseSetFeed,
  type ReleaseSetFeed,
  type ReleaseSetPointer,
} from "./release-set-feed.ts"

// Throwaway staging key — generated fresh on every run, never persisted.
const pair = generateKeyPairSync("ed25519")
const stagingKey: ReleaseSigningKey = {
  d: (pair.privateKey.export({ format: "jwk" }) as { d: string }).d,
  kid: "staging-e2e-throwaway",
}
const stagingPin = deriveReleaseKeyPin(stagingKey)

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex")

/** Local-server byte store standing in for the artifact bucket. */
const artifactBytesByPath = new Map<string, Uint8Array>()

const buildSignedSet = (version: string): {
  releaseSet: ReleaseSet
  payloadBytes: Uint8Array
  signatureBytes: Uint8Array
} => {
  const text = readFileSync(
    path.join(import.meta.dirname, "../../openagents-desktop/tests/fixtures/release-set-v2.json"),
    "utf8",
  ).replaceAll("2.4.0-rc.3", version)
  const raw = JSON.parse(text) as {
    signingPolicy: { keyId: string }
    targets: ReadonlyArray<{
      artifacts: Array<{ name: string; url: string; sha256: string; byteLength: number }>
    }>
  }
  raw.signingPolicy.keyId = stagingKey.kid
  for (const target of raw.targets) {
    for (const artifact of target.artifacts) {
      const bytes = new TextEncoder().encode(`staging artifact ${artifact.name} ${version} `.repeat(4))
      artifact.sha256 = sha256(bytes)
      artifact.byteLength = bytes.byteLength
      artifact.url = `https://staging-artifacts.test/staging-artifacts/${artifact.name}`
      artifactBytesByPath.set(`/staging-artifacts/${artifact.name}`, bytes)
    }
  }
  const decoded = decodeReleaseSet(raw)
  if (!decoded.ok) throw new Error(`staging fixture rejected: ${decoded.reason}`)
  const payloadBytes = canonicalizeReleaseSet(decoded.releaseSet)
  const signed = signReleasePayload(payloadBytes, stagingKey)
  return {
    releaseSet: decoded.releaseSet,
    payloadBytes,
    signatureBytes: new TextEncoder().encode(JSON.stringify(signed.envelope)),
  }
}

let feed: ReleaseSetFeed
let server: Server
let localOrigin = ""
let promotedPointer: ReleaseSetPointer
let previousGeneration = ""

/** TLS-terminator stand-in: map the signed HTTPS artifact host to the local listener. */
const desktopFetch = (async (value: string | URL | Request): Promise<Response> => {
  const url = String(value instanceof Request ? value.url : value)
  const target = url.startsWith("https://staging-artifacts.test/")
    ? `${localOrigin}${new URL(url).pathname}`
    : url
  return await fetch(target)
}) as typeof globalThis.fetch

const roots: string[] = []
const makeRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-staging-e2e-"))
  roots.push(root)
  return root
}

const migrationEvidence = () => ({
  schema: "openagents.desktop.update_migration_evidence.v1" as const,
  strategy: "external_state_roots_unchanged" as const,
  categories: {
    sessions: { disposition: "present" as const, rootRef: `sha256:${"1".repeat(64)}`, kind: "directory" as const },
    vaultRefs: { disposition: "present" as const, rootRef: `sha256:${"2".repeat(64)}`, kind: "file" as const },
    settings: { disposition: "present" as const, rootRef: `sha256:${"3".repeat(64)}`, kind: "directory" as const },
    drafts: { disposition: "present" as const, rootRef: `sha256:${"4".repeat(64)}`, kind: "file" as const },
  },
})

const cleanDrain = {
  ok: true,
  drained: [...childRuntimeKinds],
  timedOut: [],
  elapsedMs: 5,
} as const

/** Minimal faithful retained-slot applier (models the macOS applier's claims). */
const makeApplier = () => {
  const state = { installed: "2.4.0-rc.2", retained: null as string | null }
  return {
    state,
    target: "darwin-arm64" as const,
    format: "dmg" as const,
    rollbackClaim: "retained_slot" as const,
    rollbackAvailable: () => state.retained !== null,
    rollbackVersion: () => state.retained,
    armFirstLaunchRollback: async () => true,
    install: async (_artifactPath: string, candidateVersion: string) => {
      state.retained = state.installed
      state.installed = candidateVersion
      return {
        ok: true,
        action: "installed",
        installedVersion: candidateVersion,
        previousVersion: state.retained,
      } as const
    },
    rollback: async () => {
      if (state.retained === null) return { ok: false, reason: "rollback_unavailable" } as const
      state.installed = state.retained
      state.retained = null
      return {
        ok: true,
        action: "rolled_back",
        installedVersion: state.installed,
        previousVersion: null,
      } as const
    },
  }
}

const makeHost = (input: {
  root: string
  installedVersion: string
  applier: ReturnType<typeof makeApplier>
  restart?: () => void
}) => {
  const resolution = resolveDesktopUpdateFeedConfig({
    [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: localOrigin,
    [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: JSON.stringify(stagingPin),
  }, "rc")
  if (!resolution.ok) throw new Error(`staging resolution refused: ${resolution.reason}`)
  return openDesktopUpdateStagingHost({
    root: input.root,
    installedVersion: input.installedVersion,
    channel: "rc",
    baseUrl: resolution.baseUrl,
    pin: resolution.pin,
    fetch: desktopFetch,
    platform: "darwin",
    hostArchitecture: "arm64",
    applicationArchitecture: "arm64",
    hostVersion: "14.0",
    openPath: async () => "",
    applier: input.applier,
    migrationEvidence,
    drainChildren: async () => cleanDrain,
    ...(input.restart === undefined ? {} : { restart: input.restart }),
  })
}

beforeAll(async () => {
  feed = createReleaseSetFeed({
    store: createInMemoryReleaseSetFeedStore(),
    pins: new Map([[stagingPin.kid, stagingPin]]),
    // Real DIST-04-shaped admission: download every signed artifact URL over
    // the local socket and hash the OBSERVED bytes (no digest echoing).
    verifyArtifact: async (artifact) => {
      const response = await fetch(`${localOrigin}${new URL(artifact.url).pathname}`)
      if (!response.ok) throw new Error(`artifact fetch failed: ${artifact.url}`)
      const bytes = new Uint8Array(await response.arrayBuffer())
      return { byteLength: bytes.byteLength, sha256: sha256(bytes) }
    },
  })

  server = createServer((request, response) => {
    void (async () => {
      const url = `${localOrigin}${request.url ?? "/"}`
      const feedResponse = await feed.fetch(new Request(url, { method: request.method }))
      if (feedResponse !== null) {
        response.writeHead(feedResponse.status, Object.fromEntries(feedResponse.headers.entries()))
        response.end(Buffer.from(await feedResponse.arrayBuffer()))
        return
      }
      const artifact = artifactBytesByPath.get(new URL(url).pathname)
      if (artifact !== undefined) {
        response.writeHead(200, {
          "content-length": String(artifact.byteLength),
          "content-type": "application/octet-stream",
        })
        response.end(Buffer.from(artifact))
        return
      }
      response.writeHead(404, { "content-type": "text/plain" })
      response.end("not found")
    })().catch(() => {
      response.writeHead(500)
      response.end()
    })
  })
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve))
  const address = server.address()
  if (address === null || typeof address === "string") throw new Error("no listener address")
  localOrigin = `http://127.0.0.1:${address.port}`

  // Publish the staging channel exactly as the runbook describes: admit the
  // previous candidate, promote, admit the current candidate, promote — the
  // pointer retains the previous generation as the one rollback slot.
  const previous = buildSignedSet("2.4.0-rc.2")
  const current = buildSignedSet("2.4.0-rc.3")
  const admittedPrevious = await feed.admitCandidate({
    channel: "rc",
    payloadBytes: previous.payloadBytes,
    signatureBytes: previous.signatureBytes,
  })
  const firstPointer = await feed.promote("rc", admittedPrevious.generation, null)
  const admittedCurrent = await feed.admitCandidate({
    channel: "rc",
    payloadBytes: current.payloadBytes,
    signatureBytes: current.signatureBytes,
  })
  promotedPointer = await feed.promote("rc", admittedCurrent.generation, firstPointer.revision)
  previousGeneration = admittedPrevious.generation
})

afterAll(async () => {
  await new Promise<void>((resolve) => server.close(() => resolve()))
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

describe("staging-channel Desktop update cycle against a live oa-updates feed instance", () => {
  test("feed promotion retained the previous generation as the rollback slot", () => {
    expect(promotedPointer.channel).toBe("rc")
    expect(promotedPointer.previousGeneration).toBe(previousGeneration)
  })

  test("discovers, verifies, stages, applies, and confirms via first-launch receipt", async () => {
    const root = makeRoot()
    const applier = makeApplier()
    let restarts = 0
    const runtimeA = makeHost({ root, installedVersion: "2.4.0-rc.2", applier, restart: () => { restarts += 1 } })

    expect(await runtimeA.check()).toMatchObject({ phase: "available", candidateVersion: "2.4.0-rc.3", channel: "rc" })
    expect(await runtimeA.download()).toMatchObject({ phase: "staged", candidateVersion: "2.4.0-rc.3" })
    const stagedName = "OpenAgents-2.4.0-rc.3-rc-darwin-arm64.dmg"
    const stagedBytes = readFileSync(path.join(root, stagedName))
    expect(sha256(new Uint8Array(stagedBytes))).toBe(
      sha256(artifactBytesByPath.get(`/staging-artifacts/${stagedName}`)!),
    )
    expect(await runtimeA.apply()).toMatchObject({ phase: "restarting", candidateVersion: "2.4.0-rc.3", rollbackVersion: "2.4.0-rc.2" })
    expect(applier.state.installed).toBe("2.4.0-rc.3")
    expect(restarts).toBe(1)

    // Runtime B: the freshly launched candidate build must demonstrate health
    // and a clean shutdown before the receipt exists.
    const runtimeB = makeHost({ root, installedVersion: "2.4.0-rc.3", applier })
    expect(runtimeB.snapshot()).toMatchObject({ phase: "restarting", rollbackVersion: "2.4.0-rc.2" })
    expect(await runtimeB.reconcile()).toMatchObject({ phase: "restarting" })
    expect(await runtimeB.recordHealthyLaunch({
      rendererReadyAt: "2026-07-17T10:00:00.000Z",
      providerReadyAt: "2026-07-17T10:00:01.000Z",
    })).toMatchObject({ phase: "restarting" })
    expect(runtimeB.recordCleanShutdown(cleanDrain)).toBe(true)
    expect(existsSync(path.join(root, "launch-receipt.json"))).toBe(true)

    // Runtime C: reconcile consumes the exact-version receipt; the retained
    // slot stays available for manual rollback (phase rollback_available).
    const runtimeC = makeHost({ root, installedVersion: "2.4.0-rc.3", applier })
    expect(await runtimeC.reconcile()).toMatchObject({
      phase: "rollback_available",
      candidateVersion: null,
      rollbackVersion: "2.4.0-rc.2",
      reason: null,
    })
  })

  test("a missed launch receipt rolls back to the retained slot and a late receipt cannot resurrect", async () => {
    const root = makeRoot()
    const applier = makeApplier()
    let restarts = 0
    const runtimeA = makeHost({ root, installedVersion: "2.4.0-rc.2", applier, restart: () => { restarts += 1 } })
    await runtimeA.check()
    await runtimeA.download()
    expect(await runtimeA.apply()).toMatchObject({ phase: "restarting" })
    expect(applier.state.installed).toBe("2.4.0-rc.3")

    // The candidate never wrote its receipt; the watchdog restored the old
    // build, which relaunches and reconciles as previousVersion.
    const relaunchedOld = makeHost({ root, installedVersion: "2.4.0-rc.2", applier, restart: () => { restarts += 1 } })
    expect(await relaunchedOld.reconcile()).toMatchObject({ phase: "restarting" })
    expect(applier.state.installed).toBe("2.4.0-rc.2")
    expect(applier.state.retained).toBeNull()

    // After the rollback restart the machine is terminal-current, and a LATE
    // receipt for the rolled-back candidate changes nothing.
    const afterRollback = makeHost({ root, installedVersion: "2.4.0-rc.2", applier })
    expect(await afterRollback.reconcile()).toMatchObject({ phase: "current", candidateVersion: null })
    writeFileSync(path.join(root, "launch-receipt.json"), JSON.stringify({
      schema: "openagents.desktop.launch_health.v1",
      app: "openagents-desktop",
      version: "2.4.0-rc.3",
      transactionRef: "0".repeat(32),
      rendererReadyAt: "2026-07-17T10:00:00.000Z",
      providerReadyAt: "2026-07-17T10:00:01.000Z",
      cleanShutdownAt: "2026-07-17T10:00:02.000Z",
    }))
    expect(await afterRollback.reconcile()).toMatchObject({ phase: "current", candidateVersion: null })
  })

  test("a production-pinned client refuses the staging-signed feed (kid_not_pinned)", async () => {
    const root = makeRoot()
    const host = openDesktopUpdateStagingHost({
      root,
      installedVersion: "2.4.0-rc.2",
      channel: "rc",
      baseUrl: `${localOrigin}/desktop/openagents/rc`,
      pin: PRODUCTION_RELEASE_KEY_PIN,
      fetch: desktopFetch,
      platform: "darwin",
      hostArchitecture: "arm64",
      applicationArchitecture: "arm64",
      hostVersion: "14.0",
      openPath: async () => "",
    })
    expect(await host.check()).toMatchObject({ phase: "rejected", reason: "kid_not_pinned" })
  })

  test("feed pointer rollback restores exactly the retained generation; clients refuse the downgrade", async () => {
    const rolledBack = await feed.rollback("rc", promotedPointer.revision)
    expect(rolledBack.generation).toBe(previousGeneration)
    expect(rolledBack.previousGeneration).toBe(promotedPointer.generation)

    // A client already on 2.4.0-rc.2 must refuse the feed's rolled-back set
    // as a forward update — the ONLY sanctioned downgrade path is the local
    // retained slot, never the feed.
    const currentClient = makeHost({ root: makeRoot(), installedVersion: "2.4.0-rc.2", applier: makeApplier() })
    expect(await currentClient.check()).toMatchObject({ phase: "rejected", reason: "not_monotonic" })

    // An older install still sees the rolled-back-to release as a normal
    // verified forward update.
    const olderClient = makeHost({ root: makeRoot(), installedVersion: "2.4.0-rc.1", applier: makeApplier() })
    expect(await olderClient.check()).toMatchObject({ phase: "available", candidateVersion: "2.4.0-rc.2" })
  })
})
