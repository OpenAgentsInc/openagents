import { createHash } from "node:crypto"
import { readFile, rm, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { mkdtemp } from "node:fs/promises"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  checkPsionicInstallMachine,
  installPsionicBinary,
  installPsionicModelArtifact,
  psionicBinaryPath,
  psionicModelPath,
} from "../src/psionic-install"
import { assertPublicProjectionSafe } from "../src/state"

describe("Pylon optional Psionic installer", () => {
  test("refuses unsupported machines before fetching", async () => {
    let fetched = false
    const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: "/tmp/pylon-test" }, "darwin")
    const result = await installPsionicBinary(summary, {
      consent: true,
      manifestUrl: "https://example.invalid/manifest.json",
      platform: "win32",
      arch: "x64",
      totalMemoryBytes: 16 * 1024 * 1024 * 1024,
      fetch: async () => {
        fetched = true
        return Response.json({})
      },
    })

    expect(result.state).toBe("blocked")
    expect(result.blockerRefs).toContain("blocker.psionic_installer.unsupported_platform")
    expect(result.blockerRefs).toContain("blocker.psionic_installer.unsupported_architecture")
    expect(fetched).toBe(false)
    assertPublicProjectionSafe(result)
  })

  test("requires explicit operator consent before fetching", async () => {
    let fetched = false
    const machine = checkPsionicInstallMachine("binary", {
      platform: "darwin",
      arch: "arm64",
      totalMemoryBytes: 16 * 1024 * 1024 * 1024,
      consent: false,
      fetch: async () => {
        fetched = true
        return Response.json({})
      },
    })

    expect(machine.blockerRefs).toContain("blocker.psionic_installer.operator_consent_required")
    expect(fetched).toBe(false)
  })

  test("verifies release manifest and SHA-256 before placing binary", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const binary = new TextEncoder().encode("psionic binary")
      const digest = sha256(binary)
      const result = await installPsionicBinary(summary, {
        channel: "rc",
        consent: true,
        manifestUrl: "https://manifest.test/psionic-release.json",
        platform: "darwin",
        arch: "arm64",
        totalMemoryBytes: 16 * 1024 * 1024 * 1024,
        availableDiskBytes: 1024 * 1024 * 1024,
        fetch: fakeInstallerFetch({
          "https://manifest.test/psionic-release.json": {
            schema: "openagents.psionic.release_manifest.v0.3",
            channel: "rc",
            version: "0.1.0-rc1",
            platform: "darwin-arm64",
            binary: {
              url: "https://artifact.test/psionic-openai-server",
              sha256: digest,
              artifactRef: "artifact.psionic.binary.darwin_arm64.rc1",
              binaryRef: "binary.psionic.openai_server.darwin_arm64.rc1",
            },
          },
          "https://artifact.test/psionic-openai-server": binary,
        }),
      })

      expect(result).toMatchObject({
        state: "installed",
        kind: "binary",
        platformRef: "darwin-arm64",
        artifactRef: "artifact.psionic.binary.darwin_arm64.rc1",
        digestRef: `artifact.digest.sha256.${digest}`,
        blockerRefs: [],
      })
      expect(JSON.stringify(result)).not.toContain(home)
      assertPublicProjectionSafe(result)
      await expect(stat(psionicBinaryPath(summary, digest))).resolves.toMatchObject({ mode: expect.any(Number) })
      expect(await readFile(psionicBinaryPath(summary, digest), "utf8")).toBe("psionic binary")
    })
  })

  test("refuses binary placement on SHA-256 mismatch", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "linux")
      const result = await installPsionicBinary(summary, {
        consent: true,
        manifestUrl: "https://manifest.test/psionic-release.json",
        platform: "linux",
        arch: "x64",
        totalMemoryBytes: 16 * 1024 * 1024 * 1024,
        availableDiskBytes: 1024 * 1024 * 1024,
        fetch: fakeInstallerFetch({
          "https://manifest.test/psionic-release.json": {
            schema: "openagents.psionic.release_manifest.v0.3",
            channel: "rc",
            version: "0.1.0-rc1",
            platform: "linux-x64",
            binary: {
              url: "https://artifact.test/psionic-openai-server",
              sha256: "0".repeat(64),
              artifactRef: "artifact.psionic.binary.linux_x64.rc1",
              binaryRef: "binary.psionic.openai_server.linux_x64.rc1",
            },
          },
          "https://artifact.test/psionic-openai-server": new TextEncoder().encode("not matching"),
        }),
      })

      expect(result.state).toBe("blocked")
      expect(result.blockerRefs).toContain("blocker.psionic_installer.artifact_digest_mismatch")
    })
  })

  test("verifies model artifact manifest before placing 0.8B or 2B artifact", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const model = new TextEncoder().encode("qwen model")
      const digest = sha256(model)
      const result = await installPsionicModelArtifact(summary, {
        modelKey: "qwen35-0_8b-q8_0",
        consent: true,
        manifestUrl: "https://manifest.test/qwen35-0_8b.json",
        platform: "darwin",
        arch: "arm64",
        totalMemoryBytes: 16 * 1024 * 1024 * 1024,
        availableDiskBytes: 10 * 1024 * 1024 * 1024,
        fetch: fakeInstallerFetch({
          "https://manifest.test/qwen35-0_8b.json": {
            schema: "openagents.psionic.model_artifact_manifest.v0.3",
            modelKey: "qwen35-0_8b-q8_0",
            modelRef: "model.psionic.qwen35.0_8b.q8_0",
            url: "https://artifact.test/qwen35-0_8b.gguf",
            sha256: digest,
            artifactRef: "artifact.psionic.qwen35.0_8b.q8_0.gguf",
          },
          "https://artifact.test/qwen35-0_8b.gguf": model,
        }),
      })

      expect(result).toMatchObject({
        state: "installed",
        kind: "model",
        modelKey: "qwen35-0_8b-q8_0",
        backendRef: "model.psionic.qwen35.0_8b.q8_0",
        digestRef: `artifact.digest.sha256.${digest}`,
        blockerRefs: [],
      })
      expect(JSON.stringify(result)).not.toContain(home)
      expect(await readFile(psionicModelPath(summary, "qwen35-0_8b-q8_0", digest), "utf8")).toBe("qwen model")
      assertPublicProjectionSafe(result)
    })
  })
})

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-psionic-install-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

function fakeInstallerFetch(routes: Record<string, unknown>): typeof fetch {
  return async (url) => {
    const value = routes[url.toString()]
    if (value === undefined) return new Response("not found", { status: 404 })
    if (value instanceof Uint8Array) return new Response(value)
    return Response.json(value)
  }
}

function sha256(bytes: Uint8Array) {
  return createHash("sha256").update(bytes).digest("hex")
}
