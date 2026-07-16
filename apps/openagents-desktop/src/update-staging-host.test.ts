import { afterEach, describe, expect, test } from "vite-plus/test"
import { createHash, generateKeyPairSync } from "node:crypto"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { buildUpdateManifestForArtifact, deriveReleaseKeyPin, signReleasePayload, signUpdateManifest } from "./release-publish.ts"
import { canonicalizeReleaseSet, type ReleaseSet } from "./release-set-contract.ts"
import { openDesktopUpdateStagingHost, updateRecoveryRequiresStartupExit } from "./update-staging-host.ts"

const roots: string[] = []
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
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

const fixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-update-stage-")); roots.push(root)
  const pair = generateKeyPairSync("ed25519")
  const privateJwk = pair.privateKey.export({ format: "jwk" }) as { d: string }
  const key = { d: privateJwk.d, kid: "fixture-update-key" }
  const artifact = new TextEncoder().encode("signed desktop artifact")
  const manifest = buildUpdateManifestForArtifact({
    channel: "rc", version: "0.1.0-rc.6", artifactName: "OpenAgents-0.1.0-rc.6-arm64.dmg",
    artifactBytes: artifact, releasedAt: "2026-07-13T20:00:00.000Z",
  })
  const signed = signUpdateManifest(manifest, key)
  const base = "https://updates.test/desktop/openagents/rc"
  const artifactUrl = "https://artifacts.test/OpenAgents-0.1.0-rc.6-arm64.dmg"
  const responses = new Map<string, Uint8Array>([
    [`${base}/manifest.json`, signed.payloadBytes],
    [`${base}/manifest.sig.json`, new TextEncoder().encode(JSON.stringify(signed.envelope))],
    [`${base}/release.json`, new TextEncoder().encode(JSON.stringify({ channel: "rc", version: manifest.version, artifactName: manifest.artifactName, artifactUrl }))],
    [artifactUrl, artifact],
  ])
  const fetch = async (value: string | URL | Request) => {
    const bytes = responses.get(String(value))
    return bytes === undefined
      ? new Response("missing", { status: 404 })
      : new Response(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer)
  }
  return { root, base, pin: deriveReleaseKeyPin(key), fetch: fetch as typeof globalThis.fetch, artifactUrl }
}

describe("Desktop signed update staging host", () => {
  test("checks exact signed feed bytes, stages the digest-matched artifact, and survives restart", async () => {
    const h = fixture()
    const opened: string[] = []
    const make = () => openDesktopUpdateStagingHost({
      root: h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch,
      openPath: async value => { opened.push(value); return "" },
    })
    expect(await make().check()).toMatchObject({ phase: "available", candidateVersion: "0.1.0-rc.6" })
    expect(await make().download()).toMatchObject({ phase: "staged", candidateVersion: "0.1.0-rc.6" })
    const restarted = make()
    expect(restarted.snapshot()).toMatchObject({ phase: "staged", candidateVersion: "0.1.0-rc.6" })
    expect(await restarted.openInstaller()).toMatchObject({ phase: "staged" })
    expect(opened[0]?.endsWith("OpenAgents-0.1.0-rc.6-arm64.dmg")).toBe(true)
    expect(JSON.stringify(restarted.snapshot())).not.toContain(h.artifactUrl)
    expect(JSON.stringify(restarted.snapshot())).not.toContain(h.root)
  })

  test("fails closed on a pointer mismatch and a corrupt artifact", async () => {
    const mismatch = fixture()
    const originalFetch = mismatch.fetch
    const badPointerFetch = (async (value: string | URL | Request) => String(value).endsWith("release.json")
      ? new Response(JSON.stringify({ channel: "rc", version: "0.1.0-rc.99", artifactName: "wrong.dmg", artifactUrl: mismatch.artifactUrl }))
      : originalFetch(value)) as typeof globalThis.fetch
    const refused = openDesktopUpdateStagingHost({ root: mismatch.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: mismatch.base, pin: mismatch.pin, fetch: badPointerFetch, openPath: async () => "" })
    expect(await refused.check()).toMatchObject({ phase: "rejected", reason: "release_pointer_mismatch" })

    const corrupt = fixture()
    const corruptFetch = (async (value: string | URL | Request) => String(value) === corrupt.artifactUrl
      ? new Response("corrupt") : corrupt.fetch(value)) as typeof globalThis.fetch
    const host = openDesktopUpdateStagingHost({ root: corrupt.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: corrupt.base, pin: corrupt.pin, fetch: corruptFetch, openPath: async () => "" })
    expect((await host.check()).phase).toBe("available")
    expect(await host.download()).toMatchObject({ phase: "rejected", reason: "artifact_rejected" })
  })

  test("verifies ReleaseSet v2 and resolves native host architecture instead of translated app architecture", async () => {
    const h = fixture(); const pair = generateKeyPairSync("ed25519"); const privateJwk = pair.privateKey.export({ format: "jwk" }) as { d: string }
    const key = { d: privateJwk.d, kid: "fixture-host-v2" }
    const raw = JSON.parse(readFileSync(path.join(import.meta.dirname, "../tests/fixtures/release-set-v2.json"), "utf8"))
    const artifact = new TextEncoder().encode("native arm64 full artifact")
    const selected = raw.targets.find((row: { target: string }) => row.target === "darwin-arm64").artifacts.find((row: { format: string }) => row.format === "dmg")
    selected.sha256 = createHash("sha256").update(artifact).digest("hex"); selected.byteLength = artifact.byteLength; raw.signingPolicy.keyId = key.kid
    const releaseSet = raw as ReleaseSet; const signed = signReleasePayload(canonicalizeReleaseSet(releaseSet), key); let installedArchitecture: string | null = null; let watchdogPreviousArchitecture: string | null = null
    const fetch = (async (value: string | URL | Request) => {
      const url = String(value)
      if (url === `${h.base}/release-set.json`) return new Response(signed.payloadBytes.buffer.slice(signed.payloadBytes.byteOffset, signed.payloadBytes.byteOffset + signed.payloadBytes.byteLength) as ArrayBuffer)
      if (url === `${h.base}/release-set.sig.json`) return new Response(JSON.stringify(signed.envelope))
      if (url === selected.url) return new Response(artifact)
      return h.fetch(value)
    }) as typeof globalThis.fetch
    const host = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "2.4.0-rc.2", channel: "rc", baseUrl: h.base, pin: signed.pin, fetch,
      platform: "darwin", hostArchitecture: "arm64", applicationArchitecture: "x64", hostVersion: "14.0", migrationEvidence, openPath: async () => "",
      applier: { target: "darwin-arm64", format: "dmg", rollbackClaim: "retained_slot", rollbackAvailable: () => false, rollbackVersion: () => null,
        armFirstLaunchRollback: async input => { watchdogPreviousArchitecture = input.previousArchitecture; return true },
        install: async (_path: string, version: string, architecture: "arm64" | "x64") => { installedArchitecture = architecture; return { ok: true, action: "installed", installedVersion: version, previousVersion: "2.4.0-rc.2" } as const },
        rollback: async () => ({ ok: false, reason: "unavailable" } as const) } })
    expect(await host.check()).toMatchObject({ phase: "available", candidateVersion: releaseSet.version })
    expect(await host.download()).toMatchObject({ phase: "staged" }); expect(await host.apply()).toMatchObject({ phase: "restarting" })
    expect(installedArchitecture).toBe("arm64")
    expect(watchdogPreviousArchitecture).toBe("x64")
  })

  test("projects bounded reason codes instead of transport URLs or local paths", async () => {
    const check = fixture()
    const secretFeed = `${check.base}/manifest.json?credential=do-not-project`
    const failedCheck = openDesktopUpdateStagingHost({
      root: check.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: check.base, pin: check.pin,
      fetch: (async (_value: string | URL | Request): Promise<Response> => { throw new Error(`request failed for ${secretFeed}`) }) as typeof globalThis.fetch,
      openPath: async () => "",
    })
    const checkProjection = await failedCheck.check()
    expect(checkProjection).toMatchObject({ phase: "rejected", reason: "update_check_failed" })
    expect(JSON.stringify(checkProjection)).not.toContain(secretFeed)

    const download = fixture()
    let allowFeed = true
    const failedDownload = openDesktopUpdateStagingHost({
      root: download.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: download.base, pin: download.pin,
      fetch: (async value => {
        if (allowFeed) return download.fetch(value)
        throw new Error(`write failed at ${download.root}`)
      }) as typeof globalThis.fetch,
      openPath: async () => "",
    })
    expect((await failedDownload.check()).phase).toBe("available")
    allowFeed = false
    const downloadProjection = await failedDownload.download()
    expect(downloadProjection).toMatchObject({ phase: "rejected", reason: "update_download_failed" })
    expect(JSON.stringify(downloadProjection)).not.toContain(download.root)
  })

  test("proves Runtime A install -> Runtime B health and clean shutdown -> Runtime C retained rollback acceptance", async () => {
    const h = fixture()
    let restarts = 0
    let rollback = false
    const installed: string[] = []
    const applier = {
      rollbackAvailable: () => rollback,
      rollbackVersion: () => rollback ? "0.1.0-rc.5" : null,
      install: async (artifactPath: string, candidateVersion: string) => {
        installed.push(`${path.basename(artifactPath)}:${candidateVersion}`)
        rollback = true
        return { ok: true, action: "installed", installedVersion: candidateVersion, previousVersion: "0.1.0-rc.5" } as const
      },
      rollback: async () => {
        rollback = false
        return { ok: true, action: "rolled_back", installedVersion: "0.1.0-rc.5", previousVersion: null } as const
      },
    }
    const host = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, migrationEvidence, restart: () => { restarts += 1 } })
    expect((await host.check()).phase).toBe("available")
    expect((await host.download()).phase).toBe("staged")
    expect(await host.apply()).toMatchObject({ phase: "restarting", candidateVersion: "0.1.0-rc.6", rollbackVersion: "0.1.0-rc.5" })
    expect(installed).toEqual(["OpenAgents-0.1.0-rc.6-arm64.dmg:0.1.0-rc.6"])
    expect(restarts).toBe(1)

    const restarted = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.6", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, migrationEvidence, restart: () => { restarts += 1 } })
    expect(restarted.snapshot()).toMatchObject({ phase: "restarting", rollbackVersion: "0.1.0-rc.5" })
    const [checked, reconciled] = await Promise.all([restarted.check(), restarted.reconcile()])
    expect(checked).toMatchObject({ phase: "restarting", candidateVersion: "0.1.0-rc.6" })
    expect(reconciled).toMatchObject({ phase: "restarting", candidateVersion: "0.1.0-rc.6" })
    expect(updateRecoveryRequiresStartupExit(reconciled)).toBe(false)
    const [healthy, racedCheck] = await Promise.all([
      restarted.recordHealthyLaunch({ rendererReadyAt: "2026-07-16T10:00:00.000Z", providerReadyAt: "2026-07-16T10:00:01.000Z" }),
      restarted.check(),
    ])
    expect(healthy).toMatchObject({ phase: "restarting" })
    expect(racedCheck).toMatchObject({ phase: "restarting", candidateVersion: "0.1.0-rc.6" })
    expect(restarted.recordCleanShutdown({ ok: false, drained: ["agent"], timedOut: ["pty"], elapsedMs: 15_000 })).toBe(false)
    expect(existsSync(path.join(h.root, "launch-receipt.json"))).toBe(false)
    mkdirSync(path.join(h.root, "launch-receipt.json"))
    expect(restarted.recordCleanShutdown({ ok: true, drained: ["agent", "pty", "local_server", "helper", "window", "wsl"], timedOut: [], elapsedMs: 5 })).toBe(false)
    rmSync(path.join(h.root, "launch-receipt.json"), { recursive: true })
    expect(restarted.recordCleanShutdown({ ok: true, drained: ["agent", "pty", "local_server", "helper", "window", "wsl"], timedOut: [], elapsedMs: 5 })).toBe(true)
    const confirmed = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.6", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, migrationEvidence })
    expect(await confirmed.reconcile()).toMatchObject({ phase: "rollback_available", candidateVersion: null })
    expect(await restarted.rollback()).toMatchObject({ phase: "restarting", rollbackVersion: null })
    expect(restarts).toBe(2)
  })

  test("persists a watchdog rollback failure without retrying or erasing the candidate", async () => {
    const h = fixture(); let now = 1_000; let rollback = false; let rollbackCalls = 0; let restarts = 0
    const applier = {
      rollbackAvailable: () => rollback,
      rollbackVersion: () => rollback ? "0.1.0-rc.5" : null,
      install: async (_artifactPath: string, candidateVersion: string) => { rollback = true; return { ok: true, action: "installed", installedVersion: candidateVersion, previousVersion: "0.1.0-rc.5" } as const },
      rollback: async () => { rollbackCalls += 1; rollback = false; return { ok: true, action: "rolled_back", installedVersion: "0.1.0-rc.5", previousVersion: null } as const },
    }
    const make = (installedVersion: string) => openDesktopUpdateStagingHost({ root: h.root, installedVersion, channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, migrationEvidence, now: () => now, restart: () => { restarts += 1 } })
    const first = make("0.1.0-rc.5"); await first.check(); await first.download(); await first.apply()
    writeFileSync(path.join(h.root, "first-launch-watchdog.result"), "rollback_failed\n")
    expect(await make("0.1.0-rc.6").reconcile()).toMatchObject({ phase: "rejected", reason: "watchdog_rollback_failed", candidateVersion: "0.1.0-rc.6" })
    expect(rollbackCalls).toBe(0); expect(restarts).toBe(1)
  })

  test("recovers power loss after rollback replacement before terminal state or slot deletion", async () => {
    const prepare = async () => {
      const h = fixture(); let rollbackCalls = 0; let rollbackAvailable = true; let completion: "rolled_back" | null = null
      const applier = {
        rollbackAvailable: () => rollbackAvailable,
        rollbackVersion: () => rollbackAvailable ? "0.1.0-rc.5" : null,
        rollbackCompletionStatus: () => completion,
        install: async (_artifactPath: string, candidateVersion: string) => ({ ok: true, action: "installed", installedVersion: candidateVersion, previousVersion: "0.1.0-rc.5" } as const),
        rollback: async () => { rollbackCalls += 1; rollbackAvailable = false; completion = "rolled_back"; return { ok: true, action: "rolled_back", installedVersion: "0.1.0-rc.5", previousVersion: null } as const },
      }
      const first = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, migrationEvidence })
      await first.check(); await first.download(); await first.apply()
      return { h, applier, rollbackCalls: () => rollbackCalls, setCompletion: (value: "rolled_back" | null) => { completion = value }, setRollbackAvailable: (value: boolean) => { rollbackAvailable = value } }
    }

    const prepared = await prepare()
    const afterSelector = openDesktopUpdateStagingHost({ root: prepared.h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: prepared.h.base, pin: prepared.h.pin, fetch: prepared.h.fetch, openPath: async () => "", applier: prepared.applier, migrationEvidence })
    expect(await afterSelector.reconcile()).toMatchObject({ phase: "restarting", candidateVersion: null })
    expect(prepared.rollbackCalls()).toBe(1)

    const manual = await prepare()
    manual.setRollbackAvailable(false); manual.setCompletion("rolled_back")
    const manualStateFile = path.join(manual.h.root, "state.json")
    writeFileSync(manualStateFile, JSON.stringify({ ...JSON.parse(readFileSync(manualStateFile, "utf8")), operation: "rolling_back" }))
    const interruptedManual = openDesktopUpdateStagingHost({ root: manual.h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: manual.h.base, pin: manual.h.pin, fetch: manual.h.fetch, openPath: async () => "", applier: manual.applier, migrationEvidence })
    expect(await interruptedManual.reconcile()).toMatchObject({ phase: "current", reason: null, candidateVersion: null })
    expect(manual.rollbackCalls()).toBe(0)

    for (const crash of [{ transaction: true, slot: true }, { transaction: false, slot: true }, { transaction: false, slot: false }]) {
      const terminal = await prepare()
      terminal.setRollbackAvailable(false); terminal.setCompletion("rolled_back")
      const stateFile = path.join(terminal.h.root, "state.json")
      const state = JSON.parse(readFileSync(stateFile, "utf8"))
      writeFileSync(stateFile, JSON.stringify({ ...state, operation: "rollback_cleanup_pending", candidate: null, releaseSetCandidate: null, artifactUrl: null, stagedArtifactName: null, previousVersion: null, appliedAtMs: null, launchTransactionRef: null, migrationEvidence: null, reason: null }))
      if (crash.slot) mkdirSync(path.join(terminal.h.root, "rollback"), { recursive: true })
      if (crash.transaction) writeFileSync(path.join(terminal.h.root, "apply-transaction.json"), "durable rolled_back")
      const resumed = openDesktopUpdateStagingHost({ root: terminal.h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: terminal.h.base, pin: terminal.h.pin, fetch: terminal.h.fetch, openPath: async () => "", applier: terminal.applier, migrationEvidence })
      expect(await resumed.reconcile()).toMatchObject({ phase: "current", reason: null, candidateVersion: null })
      expect(existsSync(path.join(terminal.h.root, "rollback"))).toBe(false)
      expect(existsSync(path.join(terminal.h.root, "apply-transaction.json"))).toBe(false)
      expect(terminal.rollbackCalls()).toBe(0)
    }
  })

  test("refuses native replacement when a child-runtime class misses the bounded drain", async () => {
    const h = fixture(); let installs = 0; let restarts = 0
    const host = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", migrationEvidence,
      drainChildren: async () => ({ ok: false, drained: ["agent"], timedOut: ["pty"], elapsedMs: 15_000 }),
      restart: () => { restarts += 1 },
      applier: { rollbackAvailable: () => false, rollbackVersion: () => null, install: async () => { installs += 1; return { ok: false, reason: "unsupported_platform" } as const }, rollback: async () => ({ ok: false, reason: "rollback_unavailable" } as const) } })
    await host.check(); await host.download()
    expect(await host.apply()).toMatchObject({ phase: "rejected", reason: "child_runtime_drain_timeout" })
    expect(installs).toBe(0)
    expect(restarts).toBe(1)

    const failed = fixture(); let failedRestarts = 0
    const afterDrain = openDesktopUpdateStagingHost({ root: failed.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: failed.base, pin: failed.pin, fetch: failed.fetch, openPath: async () => "", migrationEvidence,
      drainChildren: async () => ({ ok: true, drained: ["agent", "pty", "local_server", "helper", "window", "wsl"], timedOut: [], elapsedMs: 1 }),
      restart: () => { failedRestarts += 1 },
      applier: { rollbackAvailable: () => false, rollbackVersion: () => null, install: async () => ({ ok: false, reason: "install_failed" } as const), rollback: async () => ({ ok: false, reason: "rollback_unavailable" } as const) },
    })
    await afterDrain.check(); await afterDrain.download()
    expect(await afterDrain.apply()).toMatchObject({ phase: "rejected", reason: "install_failed" })
    expect(failedRestarts).toBe(1)
  })
})
