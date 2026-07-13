import { afterEach, describe, expect, test } from "bun:test"
import { generateKeyPairSync } from "node:crypto"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { buildUpdateManifestForArtifact, deriveReleaseKeyPin, signUpdateManifest } from "./release-publish.ts"
import { openDesktopUpdateStagingHost } from "./update-staging-host.ts"

const roots: string[] = []
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
    expect(opened[0]).toEndWith("OpenAgents-0.1.0-rc.6-arm64.dmg")
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

  test("applies the verified artifact, requests restart, and projects the retained rollback after restart", async () => {
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
    const host = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.5", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, restart: () => { restarts += 1 } })
    expect((await host.check()).phase).toBe("available")
    expect((await host.download()).phase).toBe("staged")
    expect(await host.apply()).toMatchObject({ phase: "restarting", candidateVersion: null, rollbackVersion: "0.1.0-rc.5" })
    expect(installed).toEqual(["OpenAgents-0.1.0-rc.6-arm64.dmg:0.1.0-rc.6"])
    expect(restarts).toBe(1)

    const restarted = openDesktopUpdateStagingHost({ root: h.root, installedVersion: "0.1.0-rc.6", channel: "rc", baseUrl: h.base, pin: h.pin, fetch: h.fetch, openPath: async () => "", applier, restart: () => { restarts += 1 } })
    expect(restarted.snapshot()).toMatchObject({ phase: "rollback_available", rollbackVersion: "0.1.0-rc.5" })
    expect(await restarted.rollback()).toMatchObject({ phase: "restarting", rollbackVersion: null })
    expect(restarts).toBe(2)
  })
})
