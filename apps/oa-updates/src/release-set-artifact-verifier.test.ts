import { createHash } from "node:crypto"

import { describe, expect, test } from "vite-plus/test"

import type { ReleaseSetArtifact } from "../../openagents-desktop/src/release-set-contract.ts"
import { createPublicReleaseSetArtifactVerifier } from "./release-set-artifact-verifier.ts"

const bytes = new TextEncoder().encode("signed artifact bytes")
const artifact: ReleaseSetArtifact = {
  target: "darwin-arm64",
  format: "dmg",
  version: "2.4.0-rc.3",
  sourceRevision: "a".repeat(40),
  name: "OpenAgents-2.4.0-rc.3-rc-darwin-arm64.dmg",
  url: "https://downloads.openagents.test/OpenAgents-2.4.0-rc.3-rc-darwin-arm64.dmg",
  objectIdentity: "desktop/rc/fixture",
  sha256: createHash("sha256").update(bytes).digest("hex"),
  byteLength: bytes.byteLength,
  componentLedgerSha256: "b".repeat(64),
  componentLedgerRef: "sha256:fixture",
  buildReceiptRef: "sha256:fixture",
  signingPolicyId: "fixture-policy",
}

describe("public ReleaseSet artifact verifier", () => {
  test("streams and hashes the exact credential-free response", async () => {
    const verifier = createPublicReleaseSetArtifactVerifier({
      fetch: async (_url, init) => {
        expect(init?.redirect).toBe("error")
        expect(new Headers(init?.headers).has("authorization")).toBe(false)
        return new Response(bytes, {
          headers: { "content-length": String(bytes.byteLength) },
        })
      },
    })
    await expect(verifier(artifact)).resolves.toEqual({
      byteLength: artifact.byteLength,
      sha256: artifact.sha256,
    })
  })

  test("rejects redirect/error, declared-size mismatch, and oversized streams", async () => {
    const redirect = createPublicReleaseSetArtifactVerifier({
      fetch: async () => new Response(null, { status: 302 }),
    })
    await expect(redirect(artifact)).rejects.toThrow("artifact_fetch_failed")

    const wrongHeader = createPublicReleaseSetArtifactVerifier({
      fetch: async () => new Response(bytes, { headers: { "content-length": "999" } }),
    })
    await expect(wrongHeader(artifact)).rejects.toThrow("artifact_length_header_mismatch")

    const oversized = createPublicReleaseSetArtifactVerifier({
      fetch: async () => new Response(new Uint8Array(artifact.byteLength + 1)),
    })
    await expect(oversized(artifact)).rejects.toThrow("artifact_body_oversized")
  })
})
