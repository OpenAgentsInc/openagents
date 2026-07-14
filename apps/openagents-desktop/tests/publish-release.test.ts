/**
 * Publish-path oracles (CUT-26, #8706).
 *
 * All signing here uses a FIXTURE ed25519 keypair generated in-process. The
 * production release private key is owner custody and is never read, loaded,
 * or printed by any test. The CLI end-to-end tests pass the fixture seed
 * through the SAME documented env seam the production ceremony uses, then
 * assert the seed never appears in any output.
 */
import { afterAll, describe, expect, test } from "bun:test"
import { generateKeyPairSync } from "node:crypto"
import { mkdtempSync, rmSync } from "node:fs"
import { readFile, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  verifyArtifactDigest,
  verifySignedUpdateManifest,
} from "../src/update-contract.ts"
import {
  RELEASE_DESCRIPTOR_FILE,
  artifactExtension,
  assertCredentialFreeHttpsUrl,
  assertProductionKidIntegrity,
  buildUpdateManifestForArtifact,
  computeDesktopReleasePublish,
  decodeReleaseDescriptor,
  decodeUpdateManifest,
  deriveReleaseKeyPin,
  planDesktopReleasePublish,
  type ReleaseSigningKey,
} from "../src/release-publish.ts"

// --- fixture keypair (NEVER the production key) ----------------------------

const fixturePair = generateKeyPairSync("ed25519")
const fixturePrivateJwk = fixturePair.privateKey.export({ format: "jwk" }) as { d?: string }
const fixturePublicJwk = fixturePair.publicKey.export({ format: "jwk" }) as { x?: string }
const FIXTURE_KID = "fixture-publish-key"
const fixtureKey: ReleaseSigningKey = { d: fixturePrivateJwk.d ?? "", kid: FIXTURE_KID }
const fixturePin: PinnedReleaseKey = { alg: "ed25519", kid: FIXTURE_KID, x: fixturePublicJwk.x ?? "" }

const artifactBytes = new TextEncoder().encode("fixture packaged desktop artifact (cut-26 publish path)")
const releasedAt = "2026-07-12T06:00:00Z"
const artifactUrl = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/openagents-desktop/OpenAgents-fixture.dmg"

const publishOnce = (
  existingManifest: unknown | null,
  version: string,
  channel: "stable" | "rc" = "rc",
) =>
  computeDesktopReleasePublish({
    existingManifest,
    channel,
    version,
    artifactName: `OpenAgents-${version}-arm64.dmg`,
    artifactBytes,
    artifactUrl,
    releasedAt,
    key: fixtureKey,
  })

describe("release-publish core", () => {
  test("derives the public pin from the private seed", () => {
    expect(deriveReleaseKeyPin(fixtureKey)).toEqual(fixturePin)
  })

  test("publish produces exact signed bytes the client verification seam accepts", () => {
    const publish = publishOnce(null, "0.1.0-rc.1")
    const verified = verifySignedUpdateManifest(publish.payloadBytes, publish.envelope, fixturePin, "rc")
    expect(verified).toEqual({ ok: true, manifest: publish.manifest })
    expect(verifyArtifactDigest(publish.manifest, artifactBytes)).toBeTrue()
    expect(publish.manifestFileName).toBe("manifest-rc-0.1.0-rc.1.json")
    expect(publish.signatureFileName).toBe("manifest-rc-0.1.0-rc.1.sig.json")
    expect(publish.descriptorEntry).toEqual({
      manifestPath: "manifest-rc-0.1.0-rc.1.json",
      signaturePath: "manifest-rc-0.1.0-rc.1.sig.json",
      artifactUrl,
    })
  })

  test("publish is refused for duplicates, downgrades, and rc-on-stable", () => {
    const head = JSON.parse(new TextDecoder().decode(publishOnce(null, "0.1.0-rc.2").payloadBytes)) as unknown

    expect(planDesktopReleasePublish({ existingManifest: head, channel: "rc", version: "0.1.0-rc.2" }))
      .toEqual({ ok: false, reason: "not_strictly_newer" })
    expect(planDesktopReleasePublish({ existingManifest: head, channel: "rc", version: "0.1.0-rc.1" }))
      .toEqual({ ok: false, reason: "not_strictly_newer" })
    expect(planDesktopReleasePublish({ existingManifest: null, channel: "stable", version: "0.1.0-rc.9" }))
      .toEqual({ ok: false, reason: "prerelease_on_stable_channel" })
    expect(planDesktopReleasePublish({ existingManifest: head, channel: "stable", version: "0.2.0" }))
      .toEqual({ ok: false, reason: "existing_manifest_invalid" })
    expect(planDesktopReleasePublish({ existingManifest: head, channel: "rc", version: "not-a-version" }))
      .toEqual({ ok: false, reason: "unparseable_version" })
    expect(planDesktopReleasePublish({ existingManifest: { junk: true }, channel: "rc", version: "0.1.0-rc.3" }))
      .toEqual({ ok: false, reason: "existing_manifest_invalid" })
    expect(planDesktopReleasePublish({ existingManifest: head, channel: "rc", version: "0.1.0-rc.3" }))
      .toEqual({ ok: true, installedHead: "0.1.0-rc.2" })
    expect(planDesktopReleasePublish({ existingManifest: null, channel: "rc", version: "0.1.0-rc.1" }))
      .toEqual({ ok: true, installedHead: null })
  })

  test("a fixture key claiming the PRODUCTION kid is refused at publish time", () => {
    const impostor: ReleaseSigningKey = { d: fixtureKey.d, kid: PRODUCTION_RELEASE_KEY_PIN.kid }
    expect(() => assertProductionKidIntegrity(deriveReleaseKeyPin(impostor))).toThrow(/refusing to publish/)
    expect(() =>
      computeDesktopReleasePublish({
        existingManifest: null,
        channel: "rc",
        version: "0.1.0-rc.1",
        artifactName: "OpenAgents-0.1.0-rc.1-arm64.dmg",
        artifactBytes,
        artifactUrl,
        releasedAt,
        key: impostor,
      }),
    ).toThrow(/refusing to publish/)
    // The production pin itself is of course self-consistent.
    expect(() => assertProductionKidIntegrity(PRODUCTION_RELEASE_KEY_PIN)).not.toThrow()
  })

  test("artifact URL boundary refuses http and credentialed URLs", () => {
    expect(() => assertCredentialFreeHttpsUrl("http://updates.openagents.com/a.dmg")).toThrow()
    expect(() => assertCredentialFreeHttpsUrl("https://user:pw@host/a.dmg")).toThrow()
    expect(assertCredentialFreeHttpsUrl(artifactUrl).protocol).toBe("https:")
    expect(() => publishOnce(null, "0.1.0-rc.1")).not.toThrow()
    expect(() =>
      computeDesktopReleasePublish({
        existingManifest: null,
        channel: "rc",
        version: "0.1.0-rc.1",
        artifactName: "OpenAgents.dmg",
        artifactBytes,
        artifactUrl: "http://insecure/a.dmg",
        releasedAt,
        key: fixtureKey,
      }),
    ).toThrow(/credential-free HTTPS/)
  })

  test("manifest bounds are enforced (artifact name is never a path)", () => {
    expect(() =>
      buildUpdateManifestForArtifact({
        version: "0.1.0",
        channel: "stable",
        artifactName: "../evil.dmg",
        artifactBytes,
        releasedAt,
      }),
    ).toThrow(/not admissible/)
    expect(artifactExtension("OpenAgents.dmg")).toBe(".dmg")
    expect(artifactExtension("OpenAgents.zip")).toBe(".zip")
    expect(artifactExtension("OpenAgents.pkg")).toBeNull()
  })

  test("descriptor decode accepts both the flat and the releases shapes", () => {
    const entry = {
      manifestPath: "manifest-rc-0.1.0-rc.1.json",
      signaturePath: "manifest-rc-0.1.0-rc.1.sig.json",
      artifactUrl,
    }
    expect(decodeReleaseDescriptor(entry)).toEqual({ releases: [entry] })
    expect(decodeReleaseDescriptor({ releases: [entry] })).toEqual({ releases: [entry] })
    expect(decodeReleaseDescriptor({ releases: [{ manifestPath: "../x", signaturePath: "y", artifactUrl }] }))
      .toBeNull()
    expect(decodeReleaseDescriptor("junk")).toBeNull()
  })
})

// --- CLI end-to-end against a fixture dist dir -----------------------------

const appRoot = path.resolve(import.meta.dirname, "..")
const scriptPath = path.join(appRoot, "scripts/publish-release.ts")
const workDir = mkdtempSync(path.join(tmpdir(), "cut26-publish-"))
afterAll(() => rmSync(workDir, { recursive: true, force: true }))

const runCli = (
  args: readonly string[],
  env: Record<string, string | undefined> = {},
): { exitCode: number; stdout: string; stderr: string } => {
  const result = Bun.spawnSync(["bun", scriptPath, ...args], {
    cwd: appRoot,
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      // The fixture seed enters through the SAME documented seam production
      // uses. Any inherited real seam values are explicitly overridden so
      // this test can never touch owner custody material.
      OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D: fixtureKey.d,
      OPENAGENTS_RELEASE_SIGNING_KID: fixtureKey.kid,
      OPENAGENTS_RELEASE_SECRETS_PATH: "",
      ...env,
    },
  })
  return {
    exitCode: result.exitCode,
    stdout: result.stdout.toString(),
    stderr: result.stderr.toString(),
  }
}

describe("publish-release CLI (fixture dist dir, fixture key)", () => {
  const distDir = path.join(workDir, "dist")
  const artifactPath = path.join(workDir, "OpenAgents-0.1.0-rc.1-arm64.dmg")
  const artifactPath2 = path.join(workDir, "OpenAgents-0.1.0-rc.2-arm64.dmg")
  const stableArtifactPath = path.join(workDir, "OpenAgents-0.1.0-arm64.zip")

  test("stages signed releases per channel, then refuses a non-monotonic re-publish", async () => {
    await writeFile(artifactPath, artifactBytes)
    await writeFile(artifactPath2, new TextEncoder().encode("second fixture artifact bytes"))
    await writeFile(stableArtifactPath, new TextEncoder().encode("stable fixture artifact bytes"))

    const first = runCli([
      "--channel", "rc",
      "--version", "0.1.0-rc.1",
      "--artifact", artifactPath,
      "--dist-dir", distDir,
      "--released-at", releasedAt,
    ])
    expect(first.stderr).toBe("")
    expect(first.exitCode).toBe(0)
    expect(first.stdout).toContain("staged OpenAgents Desktop 0.1.0-rc.1 for rc")
    expect(first.stdout).toContain("nothing deployed")

    // The staged manifest bytes verify through the exact client seam.
    const descriptor1 = decodeReleaseDescriptor(
      JSON.parse(await readFile(path.join(distDir, RELEASE_DESCRIPTOR_FILE), "utf8")),
    )
    expect(descriptor1).not.toBeNull()
    expect(descriptor1!.releases).toHaveLength(1)
    const manifestBytes1 = new Uint8Array(
      await readFile(path.join(distDir, descriptor1!.releases[0]!.manifestPath)),
    )
    const envelope1 = JSON.parse(
      await readFile(path.join(distDir, descriptor1!.releases[0]!.signaturePath), "utf8"),
    ) as unknown
    const verified1 = verifySignedUpdateManifest(manifestBytes1, envelope1, fixturePin, "rc")
    expect(verified1.ok).toBeTrue()
    if (!verified1.ok) throw new Error("unreachable")
    expect(verifyArtifactDigest(verified1.manifest, artifactBytes)).toBeTrue()

    // Monotonic second rc publish succeeds and replaces the rc entry…
    const second = runCli([
      "--channel", "rc",
      "--version", "0.1.0-rc.2",
      "--artifact", artifactPath2,
      "--dist-dir", distDir,
      "--released-at", releasedAt,
    ])
    expect(second.exitCode).toBe(0)

    // …a stable publish coexists with the rc entry…
    const stable = runCli([
      "--channel", "stable",
      "--version", "0.1.0",
      "--artifact", stableArtifactPath,
      "--dist-dir", distDir,
      "--released-at", releasedAt,
    ])
    expect(stable.stderr).toBe("")
    expect(stable.exitCode).toBe(0)

    // …and a duplicate/downgrade re-publish is refused with a typed reason.
    const refused = runCli([
      "--channel", "rc",
      "--version", "0.1.0-rc.1",
      "--artifact", artifactPath,
      "--dist-dir", distDir,
      "--released-at", releasedAt,
    ])
    expect(refused.exitCode).toBe(1)
    expect(refused.stderr).toContain("publish refused: not_strictly_newer")

    const finalDescriptor = decodeReleaseDescriptor(
      JSON.parse(await readFile(path.join(distDir, RELEASE_DESCRIPTOR_FILE), "utf8")),
    )
    expect(finalDescriptor!.releases).toHaveLength(2)
    const channels = await Promise.all(
      finalDescriptor!.releases.map(async (entry) => {
        const manifest = decodeUpdateManifest(
          JSON.parse(await readFile(path.join(distDir, entry.manifestPath), "utf8")),
        )
        expect(manifest).not.toBeNull()
        return `${manifest!.channel}:${manifest!.version}`
      }),
    )
    expect(channels.sort()).toEqual(["rc:0.1.0-rc.2", "stable:0.1.0"])

    // History files for every published version remain in the dist dir.
    expect((await readFile(path.join(distDir, "manifest-rc-0.1.0-rc.1.json"))).byteLength).toBeGreaterThan(0)

    // The private seed never appears in ANY output of ANY invocation.
    for (const output of [first, second, stable, refused]) {
      expect(output.stdout).not.toContain(fixtureKey.d)
      expect(output.stderr).not.toContain(fixtureKey.d)
    }
  })

  test("dry run writes nothing", async () => {
    const dryDir = path.join(workDir, "dry-dist")
    const result = runCli([
      "--channel", "rc",
      "--version", "0.1.0-rc.1",
      "--artifact", artifactPath,
      "--dist-dir", dryDir,
      "--released-at", releasedAt,
      "--dry-run",
    ])
    expect(result.exitCode).toBe(0)
    expect(result.stdout).toContain("dry run — nothing written")
    expect(result.stdout).not.toContain(fixtureKey.d)
    await expect(readFile(path.join(dryDir, RELEASE_DESCRIPTOR_FILE), "utf8")).rejects.toThrow()
  })

  test("refuses to run without the documented key seam", async () => {
    const result = runCli(
      [
        "--channel", "rc",
        "--version", "0.1.0-rc.9",
        "--artifact", artifactPath,
        "--dist-dir", path.join(workDir, "unused-dist"),
      ],
      {
        OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D: "",
        OPENAGENTS_RELEASE_SIGNING_KID: "",
        OPENAGENTS_RELEASE_SECRETS_PATH: "",
      },
    )
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("no release signing key")
  })

  test("loads the key from an env-format secrets file (fixture file)", async () => {
    const secretsPath = path.join(workDir, "fixture-signing.env")
    await writeFile(
      secretsPath,
      `# fixture only\nOPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D=${fixtureKey.d}\nOPENAGENTS_RELEASE_SIGNING_KID=${fixtureKey.kid}\n`,
    )
    const secretsDistDir = path.join(workDir, "secrets-dist")
    const result = runCli(
      [
        "--channel", "stable",
        "--version", "0.1.0",
        "--artifact", stableArtifactPath,
        "--dist-dir", secretsDistDir,
        "--released-at", releasedAt,
      ],
      {
        OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D: "",
        OPENAGENTS_RELEASE_SIGNING_KID: "",
        OPENAGENTS_RELEASE_SECRETS_PATH: secretsPath,
      },
    )
    expect(result.stderr).toBe("")
    expect(result.exitCode).toBe(0)
    expect(result.stdout).not.toContain(fixtureKey.d)
    const descriptor = decodeReleaseDescriptor(
      JSON.parse(await readFile(path.join(secretsDistDir, RELEASE_DESCRIPTOR_FILE), "utf8")),
    )
    expect(descriptor!.releases).toHaveLength(1)
  })
})
