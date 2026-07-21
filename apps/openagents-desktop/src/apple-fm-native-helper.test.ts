import { describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"
import {
  appleFmClientComplete,
  appleFmClientProbe,
  appleFmHelperSupported,
  createPackagedAppleFmLauncher,
  resolveAppleFmHelperPath,
  spawnAppleFmHelper,
  verifyAppleFmHelper,
  type AppleFmChildProcess,
  type AppleFmHelperManifest,
} from "./apple-fm-native-helper.ts"

const stageHelper = (): { resourcesPath: string; helper: string; manifest: AppleFmHelperManifest } => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-apple-fm-helper-"))
  const helper = resolveAppleFmHelperPath(root)
  mkdirSync(path.dirname(helper), { recursive: true })
  writeFileSync(helper, "signed-foundation-bridge")
  chmodSync(helper, 0o755)
  const manifest: AppleFmHelperManifest = {
    protocolVersion: 1,
    helperVersion: "0.1.1",
    architecture: process.arch,
    sha256: createHash("sha256").update("signed-foundation-bridge").digest("hex"),
  }
  return { resourcesPath: root, helper, manifest }
}

const fakeChild = (): { child: AppleFmChildProcess; killed: () => number } => {
  let killed = 0
  return {
    child: { once: () => undefined, kill: () => { killed += 1; return true } },
    killed: () => killed,
  }
}

describe("Apple FM packaged helper oracle", () => {
  test("makes the code signature authoritative and demotes sha256 to an unsigned-only fallback", () => {
    const { resourcesPath, helper, manifest } = stageHelper()
    // Baseline: valid signature + matching digest is accepted.
    expect(verifyAppleFmHelper({ resourcesPath, manifest, verifySignature: (candidate) => candidate === helper })).toBe(helper)
    // #9155 regression pin: a validly-signed binary whose runtime bytes no
    // longer match the pinned PRE-SIGN digest (codesign rewrote the Mach-O)
    // is ACCEPTED. The signature is authoritative; sha256 is NOT compared.
    expect(verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, sha256: "0".repeat(64) }, verifySignature: () => true })).toBe(helper)
    // Unsigned/dev build: fall back to the sha256 pin. A matching digest is
    // accepted; a mismatch is rejected with the typed digest error.
    expect(verifyAppleFmHelper({ resourcesPath, manifest, verifySignature: () => false })).toBe(helper)
    expect(() => verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, sha256: "0".repeat(64) }, verifySignature: () => false })).toThrow("apple_fm_helper_digest_mismatch")
    // Manifest/architecture mismatch is still rejected before any body read.
    expect(() => verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, architecture: "sparc" }, verifySignature: () => true })).toThrow("apple_fm_helper_manifest_mismatch")
  })

  test("spawns an absolute helper with --port and a stripped, shell-free environment", async () => {
    const { readFile } = await import("node:fs/promises")
    const source = await readFile(new URL("./apple-fm-native-helper.ts", import.meta.url), "utf8")
    expect(source).toContain('spawn(absolutePath, ["--port", String(port)]')
    expect(source).toContain('PATH: ""')
    expect(source).toContain('HOME: "/var/empty"')
    expect(source).toContain("detached: false")
    expect(source).not.toContain("shell: true")
    expect(typeof spawnAppleFmHelper).toBe("function")
    expect(appleFmHelperSupported()).toBe(process.platform === "darwin" && process.arch === "arm64")
  })
})

describe("Apple FM packaged launcher", () => {
  const supported = () => true
  const readyProbe = async () => ({ status: "ready" as const, ready: true, model: "apple-foundation-model", profileId: "apple-fm-local", usageTruth: "estimated" as const })
  const notReadyProbe = async () => ({ status: "unreachable" as const, ready: false, unavailableReason: "bridge_unreachable" })

  test("adopts an already-healthy bridge without spawning or stopping it", async () => {
    let spawned = 0
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath: "/does/not/matter",
      verifySignature: () => true,
      supported,
      probe: readyProbe,
      spawnHelper: () => { spawned += 1; return fakeChild().child },
      loadManifest: () => { throw new Error("should_not_load") },
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome.kind).toBe("session")
    if (outcome.kind === "session") {
      expect(outcome.session.mode).toBe("adopted")
      outcome.session.stop() // adopted bridges are never killed
    }
    expect(spawned).toBe(0)
  })

  test("reports helper_missing when no bridge is healthy and the manifest is absent", async () => {
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath: "/does/not/matter",
      verifySignature: () => true,
      supported,
      probe: notReadyProbe,
      spawnHelper: () => fakeChild().child,
      loadManifest: () => { throw new Error("ENOENT: no manifest") },
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome).toMatchObject({ kind: "helper_missing", blockerRef: "blocker.apple_fm.helper_missing" })
  })

  test("a tampered unsigned helper (bad signature and mismatched digest) is failed, not adopted or launched", async () => {
    const { resourcesPath, manifest } = stageHelper()
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath,
      verifySignature: () => false,
      supported,
      probe: notReadyProbe,
      spawnHelper: () => fakeChild().child,
      // Unsigned AND the pinned digest no longer matches the bytes on disk:
      // the sha256 fallback catches the tamper and fails closed.
      loadManifest: () => ({ ...manifest, sha256: "0".repeat(64) }),
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome).toMatchObject({ kind: "failed", failureClass: "apple_fm_helper_digest_mismatch" })
  })

  test("a validly-signed helper whose digest differs from the manifest is launched (the #9155 signed-build case)", async () => {
    const { resourcesPath, manifest } = stageHelper()
    const spawn = fakeChild()
    let probes = 0
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath,
      verifySignature: () => true,
      supported,
      // adopt probe (not ready), then poll probe (ready).
      probe: async () => { probes += 1; return probes <= 1 ? { status: "unreachable" as const, ready: false } : { status: "ready" as const, ready: true, model: "apple-foundation-model", profileId: "apple-fm-local", usageTruth: "estimated" as const } },
      spawnHelper: () => spawn.child,
      // Codesigning rewrote the Mach-O after the pre-sign digest was pinned.
      loadManifest: () => ({ ...manifest, sha256: "0".repeat(64) }),
      sleep: async () => {},
      pollIntervalMs: 1,
      readinessTimeoutMs: 1_000,
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome.kind).toBe("session")
    if (outcome.kind === "session") expect(outcome.session.mode).toBe("launched")
  })

  test("verifies, spawns, and polls to readiness, returning a launched session", async () => {
    const { resourcesPath, manifest } = stageHelper()
    const spawn = fakeChild()
    let probes = 0
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath,
      verifySignature: () => true,
      supported,
      // adopt probe (not ready), then poll probe (ready).
      probe: async () => { probes += 1; return probes <= 1 ? { status: "unreachable" as const, ready: false } : { status: "ready" as const, ready: true, model: "apple-foundation-model", profileId: "apple-fm-local", usageTruth: "estimated" as const } },
      spawnHelper: () => spawn.child,
      loadManifest: () => manifest,
      sleep: async () => {},
      pollIntervalMs: 1,
      readinessTimeoutMs: 1_000,
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome.kind).toBe("session")
    if (outcome.kind === "session") {
      expect(outcome.session.mode).toBe("launched")
      const probe = await outcome.session.probe()
      expect(probe.ready).toBe(true)
      outcome.session.stop()
      expect(spawn.killed()).toBe(1)
    }
  })

  test("times out to failed and kills the child when readiness never arrives", async () => {
    const { resourcesPath, manifest } = stageHelper()
    const spawn = fakeChild()
    const launcher = createPackagedAppleFmLauncher({
      resourcesPath,
      verifySignature: () => true,
      supported,
      probe: notReadyProbe,
      spawnHelper: () => spawn.child,
      loadManifest: () => manifest,
      sleep: async () => {},
      pollIntervalMs: 1,
      readinessTimeoutMs: 5,
    })
    const outcome = await launcher.launch({ onCrash: () => {} })
    expect(outcome).toMatchObject({ kind: "failed", failureClass: "readiness_timeout" })
    expect(spawn.killed()).toBe(1)
  })
})

describe("Apple FM in-process Pylon client adapters", () => {
  const okFetch: typeof fetch = async (input) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : (input as Request).url
    if (url.endsWith("/health")) {
      return new Response(
        JSON.stringify({ ready: true, modelId: "apple-foundation-model", model: "apple-foundation-model", platform: "darwin", version: "0.1.1" }),
        { status: 200, headers: { "content-type": "application/json" } },
      )
    }
    if (url.includes("/chat/completions")) {
      return new Response(
        JSON.stringify({ model: "apple-foundation-model", choices: [{ index: 0, message: { role: "assistant", content: "Hello there" }, finishReason: "stop" }], usage: { truth: "estimated", promptTokens: 3, completionTokens: 2, totalTokens: 5 } }),
        { status: 200, headers: { "content-type": "application/json" } },
      )
    }
    return new Response("{}", { status: 404, headers: { "content-type": "application/json" } })
  }

  test("probe maps a ready health response to a bounded public-safe probe", async () => {
    const probe = await appleFmClientProbe("http://127.0.0.1:11435", okFetch)
    expect(probe).toMatchObject({ status: "ready", ready: true, model: "apple-foundation-model", profileId: "apple-fm-local", usageTruth: "estimated" })
  })

  test("probe maps a not-ready health response to unsupported without leaking transport detail", async () => {
    const notReady: typeof fetch = async (input) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : (input as Request).url
      return url.endsWith("/health")
        ? new Response(JSON.stringify({ ready: false, unavailableReason: "apple_intelligence_not_enabled" }), { status: 200, headers: { "content-type": "application/json" } })
        : new Response("{}", { status: 404 })
    }
    const probe = await appleFmClientProbe("http://127.0.0.1:11435", notReady)
    expect(probe).toMatchObject({ ready: false, status: "unsupported", unavailableReason: "apple_intelligence_disabled" })
  })

  test("a transport failure maps to an unreachable probe, never a throw", async () => {
    const boom: typeof fetch = async () => { throw new Error("connect ECONNREFUSED 127.0.0.1:11435") }
    const probe = await appleFmClientProbe("http://127.0.0.1:11435", boom)
    expect(probe).toMatchObject({ status: "unreachable", ready: false, unavailableReason: "bridge_unreachable" })
  })

  test("complete runs one bounded read-only turn with honest usage truth", async () => {
    const turn = await appleFmClientComplete("http://127.0.0.1:11435", "read the readme", okFetch)
    expect(turn).toMatchObject({ outcome: "completed", text: "Hello there", usageTruth: "estimated", totalTokens: 5 })
  })
})
