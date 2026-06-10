import { mkdtemp, readFile, rm } from "node:fs/promises"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs, PYLON_DEFAULT_CAPABILITY_REFS } from "../src/bootstrap"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  loadOrCreateIdentity,
  projectPublicStatus,
  resolveStatePaths,
} from "../src/state"

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-state-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("Pylon identity and public projection state", () => {
  test("creates and reloads a persisted NIP-06 identity without projecting private key material", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--pylon-ref", "pylon.test.identity", "--display-name", "State Test"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const paths = resolveStatePaths(summary.paths)
      const first = await loadOrCreateIdentity(paths, {
        nodeLabel: "State Test",
        pylonRef: "pylon.test.identity",
      })
      const second = await loadOrCreateIdentity(paths)
      const rawIdentity = await readFile(paths.identity, "utf8")

      expect(first).toEqual(second)
      expect(first.pylonRef).toBe("pylon.test.identity")
      expect(first.nodeLabel).toBe("state-test")
      expect(first.npub.startsWith("npub1")).toBe(true)
      expect(first.publicKey).toMatch(/^[0-9a-f]{64}$/)
      expect(rawIdentity).not.toContain("privateKeyPem")
      expect(rawIdentity).not.toContain("nsec")
      expect(rawIdentity).not.toContain("mnemonic")
      expect(JSON.stringify(first)).not.toContain("privateKeyPem")
    })
  })

  test("emits redacted status JSON without requiring the TUI", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Status Test", "--capability-ref", "cap.gepa.retained.v1"]),
        { PYLON_HOME: home },
        "linux",
      )
      const state = await ensurePylonLocalState(summary)
      const projected = projectPublicStatus(state)

      expect(projected.kind).toBe("status")
      expect(projected.state.identity.nodeLabel).toBe("status-test")
      expect(projected.state.identity.publicKey).toMatch(/^[0-9a-f]{64}$/)
      expect(projected.state.runtime.lifecycle).toBe("offline")
      expect(projected.state.runtime.capabilityRefs).toEqual([
        "cap.gepa.retained.v1",
        ...PYLON_DEFAULT_CAPABILITY_REFS,
      ])
      expect(JSON.stringify(projected)).not.toContain("privateKeyPem")
      expect(JSON.stringify(projected)).not.toContain("nsec")
      expect(JSON.stringify(projected)).not.toContain("mnemonic")
      expect(existsSync(state.paths.runtimeState)).toBe(true)
    })
  })

  test("preserves persisted capabilities when a later command provides no capability flags", async () => {
    await withTempHome(async (home) => {
      const configured = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Capability Test", "--capability-ref", "cap.gepa.retained.v1"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const first = await ensurePylonLocalState(configured)
      expect(first.runtime.capabilityRefs).toEqual(["cap.gepa.retained.v1", ...PYLON_DEFAULT_CAPABILITY_REFS])

      const commandSummary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const reloaded = await ensurePylonLocalState(commandSummary)

      expect(reloaded.runtime.capabilityRefs).toEqual(["cap.gepa.retained.v1", ...PYLON_DEFAULT_CAPABILITY_REFS])
    })
  })

  test("accepts safe identity, availability, heartbeat, inventory, and receipt projection shapes", () => {
    expect(() =>
      assertPublicProjectionSafe({
        identity: {
          pylonRef: "pylon.public",
          npub: "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu",
          publicKey: "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917",
          nodeLabel: "public",
        },
        admin: { operatorRef: "operator.public", approved: false },
        availability: { lifecycle: "assignment-ready", blockerRefs: [] },
        inventory: { cpuCores: 8, memoryGb: 32, backendRefs: ["backend.apple_fm.ready"] },
        heartbeat: { sequence: 1, statusRef: "heartbeat.pylon.public.1" },
        receipt: { artifactRefs: ["artifact.public.ref"], settlementState: "not_applicable" },
      }),
    ).not.toThrow()
  })

  test("rejects forbidden public fields and secret-shaped strings", () => {
    expect(() => assertPublicProjectionSafe({ walletSeed: "abc" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ nested: { private_key: "abc" } })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ message: "bearer abc.def" })).toThrow("private-data-shaped")
    expect(() => assertPublicProjectionSafe({ prompt: "raw prompt: private repo content" })).toThrow(
      "private-data-shaped",
    )
  })
})
