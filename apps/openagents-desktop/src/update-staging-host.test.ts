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
})
