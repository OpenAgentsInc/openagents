import { describe, expect, test } from "bun:test"
import { discoverHostInventory, projectHostInventoryFixture } from "../src/inventory"
import { assertPublicProjectionSafe } from "../src/state"

describe("Pylon host inventory projection", () => {
  test("projects a macOS Apple Silicon fixture without raw private paths", () => {
    const inventory = projectHostInventoryFixture({
      platform: "darwin",
      arch: "arm64",
      cpuCores: 12,
      cpuModel: "Apple M3 Max",
      totalMemoryBytes: 36 * 1024 * 1024 * 1024,
      freeMemoryBytes: 12 * 1024 * 1024 * 1024,
      homeFreeBytes: 100 * 1024 * 1024 * 1024,
      networkInterfaceCount: 6,
      externalNetworkInterfaceCount: 2,
      opencodeInstalled: true,
      geminiConfigured: false,
      appleFmReady: true,
      psionicConfigured: true,
      psionicReady: true,
      psionicModelRefs: ["model.psionic.qwen35.2b.q8_0", "/Users/christopherdavid/.cache/qwen.gguf"],
      localModelRefs: ["model.local.probe.retained_fixture_cache"],
      now: "2026-06-09T00:00:00.000Z",
    })

    expect(inventory.freshness).toBe("fresh")
    expect(inventory.platform).toBe("darwin")
    expect(inventory.accelerator.kind).toBe("apple_silicon")
    expect(inventory.accelerator.modelRef).toBe("accelerator.apple_silicon")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.apple_fm")?.state).toBe("ready")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.opencode.cli")?.state).toBe("ready")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.psionic.qwen35")).toMatchObject({
      state: "ready",
      modelRef: "model.psionic.qwen35.2b.q8_0",
      blockerRefs: [],
    })
    expect(inventory.eligibleInventoryCount).toBe(1)
    expect(JSON.stringify(inventory)).not.toContain("/Users/")
    expect(JSON.stringify(inventory)).not.toContain("GEMINI_API_KEY")
  })

  test("projects a Linux NVIDIA CUDA fixture without calling it serving-ready", () => {
    const inventory = projectHostInventoryFixture({
      platform: "linux",
      arch: "x64",
      cpuCores: 8,
      cpuModel: "Intel Xeon",
      totalMemoryBytes: 32 * 1024 * 1024 * 1024,
      freeMemoryBytes: 24 * 1024 * 1024 * 1024,
      homeFreeBytes: 160 * 1024 * 1024 * 1024,
      networkInterfaceCount: 4,
      externalNetworkInterfaceCount: 1,
      geminiConfigured: true,
      accelerator: {
        kind: "nvidia_cuda",
        modelRef: "NVIDIA L4",
        vramBytes: 23034 * 1024 * 1024,
      },
      now: "2026-06-23T20:16:22.000Z",
    })

    expect(inventory.accelerator).toMatchObject({
      kind: "nvidia_cuda",
      modelRef: "accelerator.nvidia_l4",
      vramGb: 22.5,
      blockerRefs: [],
    })
    expect(inventory.blockerRefs).not.toContain("blocker.inventory.accelerator_unproven")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.local_model")?.state).toBe("unknown")
  })

  test("projects a Linux baseline with unavailable accelerator but configured Gemini", () => {
    const inventory = projectHostInventoryFixture({
      platform: "linux",
      arch: "x64",
      cpuCores: 8,
      cpuModel: "AMD EPYC",
      totalMemoryBytes: 16 * 1024 * 1024 * 1024,
      freeMemoryBytes: 8 * 1024 * 1024 * 1024,
      homeFreeBytes: 40 * 1024 * 1024 * 1024,
      networkInterfaceCount: 3,
      externalNetworkInterfaceCount: 1,
      opencodeInstalled: false,
      geminiConfigured: true,
      appleFmReady: false,
      now: "2026-06-09T00:00:00.000Z",
    })

    expect(inventory.freshness).toBe("fresh")
    expect(inventory.platform).toBe("linux")
    expect(inventory.accelerator.blockerRefs).toContain("blocker.inventory.accelerator_unproven")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.gemini")?.state).toBe("configured")
    expect(inventory.backendHealth.find((backend) => backend.backendRef === "backend.apple_fm")?.state).toBe("unsupported")
  })

  test("public projection rejects private topology, auth, raw cache paths, and environment dumps", () => {
    expect(() => assertPublicProjectionSafe({ privateTopology: "tailnet node map" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ providerAuth: "token" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ cachePath: "/Users/christopherdavid/.cache/model" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ env: { apiKey: "abc" } })).toThrow("not public-safe")
  })

  test("live discovery emits fresh or unavailable public-safe inventory", async () => {
    const inventory = await discoverHostInventory({
      now: new Date("2026-06-09T00:00:00.000Z"),
      env: {},
    })

    expect(inventory.schema).toBe("openagents.pylon.host_inventory.v0.3")
    expect(["fresh", "unavailable"]).toContain(inventory.freshness)
    assertPublicProjectionSafe(inventory)
  })

  test("live discovery projects Psionic as unconfigured without probing localhost", async () => {
    const inventory = await discoverHostInventory({
      now: new Date("2026-06-09T00:00:00.000Z"),
      env: {},
    })
    const psionic = inventory.backendHealth.find((backend) => backend.backendRef === "backend.psionic.qwen35")

    expect(psionic).toMatchObject({
      state: "missing",
      modelRef: null,
      blockerRefs: ["blocker.psionic_qwen35.connector_unconfigured"],
    })
    expect(JSON.stringify(inventory)).not.toContain("127.0.0.1")
  })

  test("live discovery marks configured Psionic from env without leaking the base URL", async () => {
    const inventory = await discoverHostInventory({
      now: new Date("2026-06-09T00:00:00.000Z"),
      env: {
        PYLON_PSIONIC_BASE_URL: "http://127.0.0.1:8080",
      },
    })
    const psionic = inventory.backendHealth.find((backend) => backend.backendRef === "backend.psionic.qwen35")

    expect(psionic).toMatchObject({
      state: "configured",
      modelRef: null,
      blockerRefs: ["blocker.psionic_qwen35.qwen35_model_missing"],
    })
    expect(JSON.stringify(inventory)).not.toContain("127.0.0.1")
  })
})
