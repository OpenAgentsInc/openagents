import { describe, expect, test } from "bun:test"
import {
  admitOpenAgentsDesktopRelease,
  decodeOpenAgentsDesktopManifest,
  sha256,
} from "./openagents-desktop-release.ts"

const manifest = {
  schema: "openagents.desktop.update_manifest.v1",
  app: "openagents-desktop",
  channel: "rc",
  version: "0.1.0-rc.1",
  artifactName: "OpenAgents-0.1.0-rc.1-arm64.zip",
  artifactSha256: "a".repeat(64),
  artifactByteLength: 123,
  releasedAt: "2026-07-12T06:00:00.000Z",
} as const

describe("OpenAgents Desktop release boundary", () => {
  test("admits exact signed-manifest bytes and a credential-free HTTPS artifact", () => {
    const bytes = new TextEncoder().encode(JSON.stringify(manifest))
    expect(admitOpenAgentsDesktopRelease({
      manifestBytes: bytes,
      signature: { alg: "ed25519", kid: "release.1", sha256: sha256(bytes), signature: "fixture" },
      artifactUrl: "https://updates.openagents.com/assets/abc",
    })).toMatchObject({ channel: "rc", manifest, artifactUrl: "https://updates.openagents.com/assets/abc" })
  })

  test("rejects digest drift, legacy products, stable prereleases, paths, and credential URLs", () => {
    const bytes = new TextEncoder().encode(JSON.stringify(manifest))
    expect(() => admitOpenAgentsDesktopRelease({
      manifestBytes: bytes,
      signature: { alg: "ed25519", kid: "release.1", sha256: "b".repeat(64), signature: "fixture" },
      artifactUrl: "https://updates.openagents.com/assets/abc",
    })).toThrow("digest mismatch")
    expect(() => admitOpenAgentsDesktopRelease({
      manifestBytes: bytes,
      signature: { alg: "ed25519", kid: "release.1", sha256: sha256(bytes), signature: "fixture" },
      artifactUrl: "https://token@example.com/artifact",
    })).toThrow("credential-free HTTPS")
    expect(decodeOpenAgentsDesktopManifest({ ...manifest, app: "khala-code-desktop" })).toBeNull()
    expect(decodeOpenAgentsDesktopManifest({ ...manifest, channel: "stable" })).toBeNull()
    expect(decodeOpenAgentsDesktopManifest({ ...manifest, artifactName: "../OpenAgents.zip" })).toBeNull()
  })
})
