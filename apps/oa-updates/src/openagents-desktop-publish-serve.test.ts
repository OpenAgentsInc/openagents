/**
 * Publish → seed → serve → client-verify round-trip (CUT-26, openagents#8706).
 *
 * The cross-shape contract proof: the dist dir is produced by the REAL
 * desktop publisher core (`apps/openagents-desktop/src/release-publish.ts`),
 * seeded by THIS service's real `seedOpenAgentsDesktopRelease`, served by
 * the real server routes, and finally re-verified through the desktop
 * client's own verification seam — the same manifest bytes end to end, all
 * with an in-process FIXTURE keypair. The production private key is never
 * read, loaded, or printed.
 */
import { describe, expect, test } from "vite-plus/test"
import { generateKeyPairSync } from "node:crypto"
import { mkdtempSync } from "node:fs"
import { writeFile, readFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  RELEASE_DESCRIPTOR_FILE,
  computeDesktopReleasePublish,
  type ReleaseSigningKey,
} from "../../openagents-desktop/src/release-publish.ts"
import {
  verifyArtifactDigest,
  verifySignedUpdateManifest,
  type PinnedReleaseKey,
} from "../../openagents-desktop/src/update-contract.ts"
import { seedOpenAgentsDesktopRelease } from "./openagents-desktop-seed.ts"
import { createUpdatesServer } from "./server.ts"

// --- fixture keypair (NEVER the production key) ----------------------------

const fixturePair = generateKeyPairSync("ed25519")
const fixtureD = (fixturePair.privateKey.export({ format: "jwk" }) as { d?: string }).d ?? ""
const fixtureX = (fixturePair.publicKey.export({ format: "jwk" }) as { x?: string }).x ?? ""
const FIXTURE_KID = "fixture-serve-key"
const signingKey: ReleaseSigningKey = { d: fixtureD, kid: FIXTURE_KID }
const clientPin: PinnedReleaseKey = { alg: "ed25519", kid: FIXTURE_KID, x: fixtureX }

const artifactBytes = new TextEncoder().encode("openagents desktop artifact behind the https url")
const releasedAt = "2026-07-12T06:00:00.000Z"
const rcArtifactUrl =
  "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/openagents-desktop/OpenAgents-0.1.0-rc.2-arm64.dmg"
const stableArtifactUrl =
  "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/openagents-desktop/OpenAgents-0.1.0-arm64.zip"

const stageDist = async (): Promise<string> => {
  const distDir = mkdtempSync(join(tmpdir(), "cut26-dist-"))
  const rc = computeDesktopReleasePublish({
    existingManifest: null,
    channel: "rc",
    version: "0.1.0-rc.2",
    artifactName: "OpenAgents-0.1.0-rc.2-arm64.dmg",
    artifactBytes,
    artifactUrl: rcArtifactUrl,
    releasedAt,
    key: signingKey,
  })
  const stable = computeDesktopReleasePublish({
    existingManifest: null,
    channel: "stable",
    version: "0.1.0",
    artifactName: "OpenAgents-0.1.0-arm64.zip",
    artifactBytes,
    artifactUrl: stableArtifactUrl,
    releasedAt,
    key: signingKey,
  })
  await writeFile(join(distDir, rc.manifestFileName), rc.payloadBytes)
  await writeFile(join(distDir, rc.signatureFileName), JSON.stringify(rc.envelope))
  await writeFile(join(distDir, stable.manifestFileName), stable.payloadBytes)
  await writeFile(join(distDir, stable.signatureFileName), JSON.stringify(stable.envelope))
  await writeFile(
    join(distDir, RELEASE_DESCRIPTOR_FILE),
    JSON.stringify({ releases: [rc.descriptorEntry, stable.descriptorEntry] }),
  )
  return distDir
}

describe("publish → seed → serve → client-verify (all fixture)", () => {
  test("both channels serve the exact signed bytes the client accepts", async () => {
    const distDir = await stageDist()
    const server = createUpdatesServer() // legacy lockout ARMED (default) — must not affect this product
    await seedOpenAgentsDesktopRelease({ server, distDir })

    for (const [channel, expectedVersion, expectedArtifactUrl] of [
      ["rc", "0.1.0-rc.2", rcArtifactUrl],
      ["stable", "0.1.0", stableArtifactUrl],
    ] as const) {
      const manifestResponse = await server.fetch(
        new Request(`https://updates.openagents.com/desktop/openagents/${channel}/manifest.json`),
      )
      expect(manifestResponse.status).toBe(200)
      expect(manifestResponse.headers.get("cache-control")).toBe("no-store")
      const manifestBytes = new Uint8Array(await manifestResponse.arrayBuffer())

      const signatureResponse = await server.fetch(
        new Request(`https://updates.openagents.com/desktop/openagents/${channel}/manifest.sig.json`),
      )
      expect(signatureResponse.status).toBe(200)
      const envelope = await signatureResponse.json() as unknown

      // Client-side verification of the exact served bytes.
      const verified = verifySignedUpdateManifest(manifestBytes, envelope, clientPin, channel)
      expect(verified.ok).toBe(true)
      if (!verified.ok) throw new Error("unreachable")
      expect(verified.manifest.version).toBe(expectedVersion)
      expect(verifyArtifactDigest(verified.manifest, artifactBytes)).toBe(true)

      // release.json carries the transport pointer for the artifact.
      const releaseResponse = await server.fetch(
        new Request(`https://updates.openagents.com/desktop/openagents/${channel}/release.json`),
      )
      expect(releaseResponse.status).toBe(200)
      expect(await releaseResponse.json()).toEqual({
        channel,
        version: expectedVersion,
        artifactName: verified.manifest.artifactName,
        artifactUrl: expectedArtifactUrl,
      })
    }

    // Same server, same moment: the legacy desktop surfaces stay locked out.
    const legacyResponse = await server.fetch(
      new Request(
        "https://updates.openagents.com/desktop/khala-code-desktop/stable-macos-arm64-update.json",
      ),
    )
    expect(legacyResponse.status).toBe(410)
  })

  test("a tampered manifest file fails the seed boundary (fail closed)", async () => {
    const distDir = await stageDist()
    // Flip one byte of the rc manifest: digest no longer matches the envelope.
    const manifestPath = join(distDir, "manifest-rc-0.1.0-rc.2.json")
    const original = await readFile(manifestPath, "utf8")
    await writeFile(manifestPath, original.replace("0.1.0-rc.2", "9.9.9-rc.1"))

    const server = createUpdatesServer()
    await expect(seedOpenAgentsDesktopRelease({ server, distDir })).rejects.toThrow(
      /digest mismatch|rejected/,
    )
  })

  test("a duplicate channel in the descriptor is refused", async () => {
    const distDir = await stageDist()
    const rcEntry = {
      manifestPath: "manifest-rc-0.1.0-rc.2.json",
      signaturePath: "manifest-rc-0.1.0-rc.2.sig.json",
      artifactUrl: rcArtifactUrl,
    }
    await writeFile(
      join(distDir, RELEASE_DESCRIPTOR_FILE),
      JSON.stringify({ releases: [rcEntry, rcEntry] }),
    )
    const server = createUpdatesServer()
    await expect(seedOpenAgentsDesktopRelease({ server, distDir })).rejects.toThrow(
      /duplicate channel/,
    )
  })

  test("the flat legacy descriptor shape still seeds", async () => {
    const distDir = await stageDist()
    await writeFile(
      join(distDir, RELEASE_DESCRIPTOR_FILE),
      JSON.stringify({
        manifestPath: "manifest-rc-0.1.0-rc.2.json",
        signaturePath: "manifest-rc-0.1.0-rc.2.sig.json",
        artifactUrl: rcArtifactUrl,
      }),
    )
    const server = createUpdatesServer()
    await seedOpenAgentsDesktopRelease({ server, distDir })
    const response = await server.fetch(
      new Request("https://updates.openagents.com/desktop/openagents/rc/manifest.json"),
    )
    expect(response.status).toBe(200)
  })
})
