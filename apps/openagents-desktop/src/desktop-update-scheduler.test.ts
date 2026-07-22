import { afterEach, describe, expect, test } from "vite-plus/test"
import { generateKeyPairSync } from "node:crypto"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { buildUpdateManifestForArtifact, deriveReleaseKeyPin, signUpdateManifest } from "./release-publish.ts"
import { openDesktopUpdateScheduler, type DesktopUpdateSchedulerHost } from "./desktop-update-scheduler.ts"
import { openDesktopUpdateStagingHost, type DesktopUpdateProjection } from "./update-staging-host.ts"

// ---------------------------------------------------------------------------
// A recording fake host. It never touches the network or the filesystem; it
// only proves the scheduler's WHEN decisions (gating, ordering, lifecycle).
// ---------------------------------------------------------------------------
const projection = (phase: DesktopUpdateProjection["phase"]): DesktopUpdateProjection => ({
  phase,
  channel: "stable",
  installedVersion: "0.1.0",
  candidateVersion: phase === "available" || phase === "staged" ? "0.1.1" : null,
  rollbackVersion: null,
  reason: phase === "rejected" ? "key_id_mismatch" : null,
})

const fakeHost = (
  checkResult: DesktopUpdateProjection["phase"],
  downloadResult: DesktopUpdateProjection["phase"] = "staged",
) => {
  const calls: string[] = []
  const host: DesktopUpdateSchedulerHost = {
    snapshot: () => projection("current"),
    check: async () => {
      calls.push("check")
      return projection(checkResult)
    },
    download: async () => {
      calls.push("download")
      return projection(downloadResult)
    },
  }
  return { host, calls }
}

// ---------------------------------------------------------------------------
// A real signed v1 fixture feed (macOS arm64), reused by the integration tests
// so the scheduler drives the exact verify -> stage path, never a stub.
// ---------------------------------------------------------------------------
const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const signedFixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-update-sched-"))
  roots.push(root)
  const pair = generateKeyPairSync("ed25519")
  const privateJwk = pair.privateKey.export({ format: "jwk" }) as { d: string }
  const key = { d: privateJwk.d, kid: "fixture-scheduler-key" }
  const artifact = new TextEncoder().encode("signed desktop artifact for scheduler")
  const manifest = buildUpdateManifestForArtifact({
    channel: "rc",
    version: "0.1.0-rc.6",
    artifactName: "OpenAgents-0.1.0-rc.6-arm64.dmg",
    artifactBytes: artifact,
    releasedAt: "2026-07-13T20:00:00.000Z",
  })
  const signed = signUpdateManifest(manifest, key)
  const base = "https://updates.test/desktop/openagents/rc"
  const artifactUrl = "https://artifacts.test/OpenAgents-0.1.0-rc.6-arm64.dmg"
  const responses = new Map<string, Uint8Array>([
    [`${base}/manifest.json`, signed.payloadBytes],
    [`${base}/manifest.sig.json`, new TextEncoder().encode(JSON.stringify(signed.envelope))],
    [
      `${base}/release.json`,
      new TextEncoder().encode(
        JSON.stringify({ channel: "rc", version: manifest.version, artifactName: manifest.artifactName, artifactUrl }),
      ),
    ],
    [artifactUrl, artifact],
  ])
  const fetch = (async (value: string | URL | Request) => {
    const bytes = responses.get(String(value))
    return bytes === undefined
      ? new Response("missing", { status: 404 })
      : new Response(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer)
  }) as typeof globalThis.fetch
  const realHost = (pin: ReturnType<typeof deriveReleaseKeyPin>) =>
    openDesktopUpdateStagingHost({
      root,
      installedVersion: "0.1.0-rc.5",
      channel: "rc",
      baseUrl: base,
      pin,
      fetch,
      platform: "darwin",
      hostArchitecture: "arm64",
      applicationArchitecture: "arm64",
      hostVersion: "14.0",
      openPath: async () => "",
    })
  return { root, correctPin: deriveReleaseKeyPin(key), realHost }
}

describe("Desktop automatic update scheduler", () => {
  test("does not contact the feed when autoCheck is disabled", async () => {
    const { host, calls } = fakeHost("available")
    const scheduler = openDesktopUpdateScheduler({
      host,
      readPreferences: () => ({ autoCheck: false, autoDownload: true }),
    })
    expect(await scheduler.runOnce()).toBeNull()
    expect(calls).toEqual([])
  })

  test("runs a check when autoCheck is enabled but never downloads without autoDownload", async () => {
    const { host, calls } = fakeHost("available")
    const scheduler = openDesktopUpdateScheduler({
      host,
      readPreferences: () => ({ autoCheck: true, autoDownload: false }),
    })
    expect(await scheduler.runOnce()).toMatchObject({ phase: "available", candidateVersion: "0.1.1" })
    expect(calls).toEqual(["check"])
  })

  test("downloads an available update only when autoDownload is enabled", async () => {
    const { host, calls } = fakeHost("available", "staged")
    const scheduler = openDesktopUpdateScheduler({
      host,
      readPreferences: () => ({ autoCheck: true, autoDownload: true }),
    })
    expect(await scheduler.runOnce()).toMatchObject({ phase: "staged" })
    expect(calls).toEqual(["check", "download"])
  })

  test("stays fail-soft when the host check throws", async () => {
    const throwing: DesktopUpdateSchedulerHost = {
      snapshot: () => projection("current"),
      check: async () => {
        throw new Error("boom_with_secret /Users/private/path")
      },
      download: async () => projection("staged"),
    }
    const logs: string[] = []
    const scheduler = openDesktopUpdateScheduler({
      host: throwing,
      readPreferences: () => ({ autoCheck: true, autoDownload: true }),
      log: (message) => logs.push(message),
    })
    expect(await scheduler.runOnce()).toBeNull()
    // A bounded, public-safe reason is logged instead of the raw path/message.
    expect(logs.length).toBe(1)
    expect(logs[0]).toContain("update_scheduler_error")
    expect(logs[0]).not.toContain("/Users/private/path")
  })

  test("start arms a periodic re-check, each tick respects the pref, and stop cleans it up", async () => {
    const { host, calls } = fakeHost("current")
    let autoCheck = true
    let tick: (() => void) | null = null
    let cleared = 0
    const scheduler = openDesktopUpdateScheduler({
      host,
      readPreferences: () => ({ autoCheck, autoDownload: false }),
      intervalMs: 60_000,
      setTimer: (handler) => {
        tick = handler
        return { id: 1 }
      },
      clearTimer: () => {
        cleared += 1
      },
    })
    scheduler.start()
    // The launch pass fires immediately.
    await Promise.resolve()
    await Promise.resolve()
    expect(calls.filter((c) => c === "check").length).toBe(1)
    expect(tick).not.toBeNull()

    // A periodic tick with the pref still enabled runs another check.
    tick!()
    await Promise.resolve()
    await Promise.resolve()
    expect(calls.filter((c) => c === "check").length).toBe(2)

    // Flip the pref off: the next tick fires but does nothing.
    autoCheck = false
    tick!()
    await Promise.resolve()
    await Promise.resolve()
    expect(calls.filter((c) => c === "check").length).toBe(2)

    // Shutdown clears the timer exactly once.
    scheduler.stop()
    expect(cleared).toBe(1)
    scheduler.stop()
    expect(cleared).toBe(1)
  })

  test("a verified update from the real signed feed stages through the existing path", async () => {
    const fixture = signedFixture()
    const scheduler = openDesktopUpdateScheduler({
      host: fixture.realHost(fixture.correctPin),
      readPreferences: () => ({ autoCheck: true, autoDownload: true }),
    })
    const result = await scheduler.runOnce()
    expect(result).toMatchObject({ phase: "staged", candidateVersion: "0.1.0-rc.6" })
  })

  test("a feed signed by the wrong key is rejected and never staged", async () => {
    const fixture = signedFixture()
    const wrongPin = deriveReleaseKeyPin({
      d: (generateKeyPairSync("ed25519").privateKey.export({ format: "jwk" }) as { d: string }).d,
      kid: "some-other-key",
    })
    const host = fixture.realHost(wrongPin)
    const scheduler = openDesktopUpdateScheduler({
      host,
      readPreferences: () => ({ autoCheck: true, autoDownload: true }),
    })
    const result = await scheduler.runOnce()
    expect(result?.phase).toBe("rejected")
    // Even with autoDownload enabled, nothing staged: the signature gate held.
    expect(host.snapshot().phase).not.toBe("staged")
  })
})
