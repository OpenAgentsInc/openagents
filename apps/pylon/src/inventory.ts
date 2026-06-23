import { arch, cpus, freemem, networkInterfaces, platform, totalmem } from "node:os"
import { statfs } from "node:fs/promises"
import { assertPublicProjectionSafe } from "./state.js"

export type InventoryPlatform = "darwin" | "linux" | "unsupported"
export type InventoryFreshness = "fresh" | "mock" | "unavailable" | "stale"
export type BackendHealthState = "ready" | "configured" | "missing" | "unsupported" | "unknown"

export type PylonBackendHealth = {
  backendRef: string
  state: BackendHealthState
  modelRef: string | null
  blockerRefs: string[]
}

export type PylonHostInventoryProjection = {
  schema: "openagents.pylon.host_inventory.v0.3"
  observedAt: string
  freshness: InventoryFreshness
  platform: InventoryPlatform
  arch: string
  cpu: {
    cores: number
    modelRef: string
  }
  memory: {
    totalGb: number
    freeGb: number
  }
  disk: {
    homeFreeGb: number | null
  }
  network: {
    interfaceCount: number
    externalInterfaceCount: number
  }
  accelerator: {
    kind: "apple_silicon" | "nvidia_cuda" | "gpu_unknown" | "none"
    modelRef: string | null
    vramGb: number | null
    blockerRefs: string[]
  }
  resourceMode: "background_20" | "interactive" | "unavailable"
  backendHealth: PylonBackendHealth[]
  modelCache: {
    state: "warm" | "cold" | "unknown" | "unavailable"
    modelRefs: string[]
  }
  eligibleInventoryCount: number
  blockerRefs: string[]
}

export type HostInventoryFixture = {
  platform: InventoryPlatform
  arch: string
  cpuCores: number
  cpuModel: string
  totalMemoryBytes: number
  freeMemoryBytes: number
  homeFreeBytes?: number | null
  networkInterfaceCount: number
  externalNetworkInterfaceCount: number
  opencodeInstalled?: boolean
  geminiConfigured?: boolean
  appleFmReady?: boolean
  psionicConfigured?: boolean
  psionicReady?: boolean
  psionicModelRefs?: string[]
  localModelRefs?: string[]
  accelerator?: HostAcceleratorFixture
  now?: string
}

export type HostAcceleratorFixture = {
  kind: "apple_silicon" | "nvidia_cuda" | "gpu_unknown" | "none"
  modelRef?: string | null
  vramBytes?: number | null
  blockerRefs?: string[]
}

const gb = (bytes: number) => Math.round((bytes / 1024 / 1024 / 1024) * 10) / 10

function sanitizeRefSegment(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9._-]+/g, "_").replace(/^_+|_+$/g, "") || "unknown"
}

function platformRef(value: NodeJS.Platform): InventoryPlatform {
  if (value === "darwin" || value === "linux") return value
  return "unsupported"
}

function countNetworkInterfaces() {
  const entries = Object.values(networkInterfaces()).flat().filter(Boolean) as NonNullable<ReturnType<typeof networkInterfaces>[string]>[number][]
  return {
    interfaceCount: entries.length,
    externalInterfaceCount: entries.filter((entry) => !entry.internal).length,
  }
}

function backendHealth(input: {
  platform: InventoryPlatform
  arch: string
  opencodeInstalled: boolean
  geminiConfigured: boolean
  appleFmReady: boolean
  psionicConfigured: boolean
  psionicReady: boolean
  psionicModelRefs: string[]
}) {
  const appleSupported = input.platform === "darwin" && input.arch === "arm64"
  const psionicModelRef = input.psionicModelRefs[0] ?? null
  return [
    {
      backendRef: "backend.opencode.cli",
      state: input.opencodeInstalled ? "ready" : "missing",
      modelRef: input.opencodeInstalled ? "model.opencode.default" : null,
      blockerRefs: input.opencodeInstalled ? [] : ["blocker.backend.opencode_missing"],
    },
    {
      backendRef: "backend.codex.future_adapter",
      state: "unknown",
      modelRef: null,
      blockerRefs: ["blocker.backend.codex_adapter_not_wired"],
    },
    {
      backendRef: "backend.apple_fm",
      state: input.appleFmReady ? "ready" : appleSupported ? "configured" : "unsupported",
      modelRef: appleSupported ? "model.apple_foundation_model" : null,
      blockerRefs: input.appleFmReady
        ? []
        : appleSupported
          ? ["blocker.backend.apple_fm_health_unproven"]
          : ["blocker.backend.apple_fm_unsupported_hardware"],
    },
    {
      backendRef: "backend.gemini",
      state: input.geminiConfigured ? "configured" : "missing",
      modelRef: input.geminiConfigured ? "model.gemini.default" : null,
      blockerRefs: input.geminiConfigured ? [] : ["blocker.backend.gemini_auth_missing"],
    },
    {
      backendRef: "backend.psionic.qwen35",
      state: input.psionicReady ? "ready" : input.psionicConfigured ? "configured" : "missing",
      modelRef: psionicModelRef,
      blockerRefs: input.psionicReady
        ? []
        : input.psionicConfigured
          ? ["blocker.psionic_qwen35.qwen35_model_missing"]
          : ["blocker.psionic_qwen35.connector_unconfigured"],
    },
    {
      backendRef: "backend.local_model",
      state: "unknown",
      modelRef: null,
      blockerRefs: ["blocker.backend.local_model_inventory_unproven"],
    },
  ] satisfies PylonBackendHealth[]
}

export function projectHostInventoryFixture(input: HostInventoryFixture): PylonHostInventoryProjection {
  const isSupported = input.platform === "darwin" || input.platform === "linux"
  const backend = backendHealth({
    platform: input.platform,
    arch: input.arch,
    opencodeInstalled: input.opencodeInstalled ?? false,
    geminiConfigured: input.geminiConfigured ?? false,
    appleFmReady: input.appleFmReady ?? false,
    psionicConfigured: input.psionicConfigured ?? false,
    psionicReady: input.psionicReady ?? false,
    psionicModelRefs: sanitizeModelRefs(input.psionicModelRefs ?? []),
  })
  const blockerRefs = new Set<string>()
  if (!isSupported) blockerRefs.add("blocker.inventory.unsupported_platform")
  if (input.cpuCores <= 0) blockerRefs.add("blocker.inventory.cpu_unavailable")
  if (input.totalMemoryBytes <= 0) blockerRefs.add("blocker.inventory.memory_unavailable")
  if (backend.every((entry) => entry.state !== "ready" && entry.state !== "configured")) {
    blockerRefs.add("blocker.inventory.no_configured_backend")
  }
  const accelerator = input.accelerator ?? defaultAccelerator(input.platform, input.arch)

  const projection: PylonHostInventoryProjection = {
    schema: "openagents.pylon.host_inventory.v0.3",
    observedAt: input.now ?? new Date().toISOString(),
    freshness: isSupported ? "fresh" : "unavailable",
    platform: input.platform,
    arch: sanitizeRefSegment(input.arch),
    cpu: {
      cores: input.cpuCores,
      modelRef: `cpu.${sanitizeRefSegment(input.cpuModel)}`,
    },
    memory: {
      totalGb: gb(input.totalMemoryBytes),
      freeGb: gb(input.freeMemoryBytes),
    },
    disk: {
      homeFreeGb: input.homeFreeBytes === undefined || input.homeFreeBytes === null ? null : gb(input.homeFreeBytes),
    },
    network: {
      interfaceCount: input.networkInterfaceCount,
      externalInterfaceCount: input.externalNetworkInterfaceCount,
    },
    accelerator: {
      kind: accelerator.kind,
      modelRef: sanitizeAcceleratorModelRef(accelerator.modelRef),
      vramGb: accelerator.vramBytes === undefined || accelerator.vramBytes === null ? null : gb(accelerator.vramBytes),
      blockerRefs: accelerator.blockerRefs ?? [],
    },
    resourceMode: isSupported ? "background_20" : "unavailable",
    backendHealth: backend,
    modelCache: {
      state: input.localModelRefs && input.localModelRefs.length > 0 ? "warm" : "unknown",
      modelRefs: input.localModelRefs ?? [],
    },
    eligibleInventoryCount: isSupported && blockerRefs.size === 0 ? 1 : 0,
    blockerRefs: [...blockerRefs],
  }
  assertPublicProjectionSafe(projection)
  return projection
}

function defaultAccelerator(platformValue: InventoryPlatform, archValue: string): HostAcceleratorFixture {
  if (platformValue === "darwin" && archValue === "arm64") {
    return {
      kind: "apple_silicon",
      modelRef: "accelerator.apple_silicon",
      vramBytes: null,
      blockerRefs: [],
    }
  }
  return {
    kind: platformValue === "linux" ? "gpu_unknown" : "none",
    modelRef: null,
    vramBytes: null,
    blockerRefs: platformValue === "linux" ? ["blocker.inventory.accelerator_unproven"] : [],
  }
}

function discoverNvidiaAccelerator(): HostAcceleratorFixture | undefined {
  if (platform() !== "linux" || !Bun.which("nvidia-smi")) return undefined
  const result = Bun.spawnSync({
    cmd: [
      "nvidia-smi",
      "--query-gpu=name,memory.total",
      "--format=csv,noheader,nounits",
    ],
    stdout: "pipe",
    stderr: "ignore",
  })
  if (result.exitCode !== 0) return undefined
  const firstLine = new TextDecoder().decode(result.stdout).split(/\r?\n/).find((line) => line.trim().length > 0)
  if (!firstLine) return undefined
  const [rawName, rawMemoryMiB] = firstLine.split(",").map((value) => value.trim())
  const memoryMiB = Number(rawMemoryMiB)
  return {
    kind: "nvidia_cuda",
    modelRef: `accelerator.${sanitizeRefSegment(rawName)}`,
    vramBytes: Number.isFinite(memoryMiB) ? memoryMiB * 1024 * 1024 : null,
    blockerRefs: [],
  }
}

export async function discoverHostInventory(input: { now?: Date; env?: Record<string, string | undefined> } = {}) {
  const net = countNetworkInterfaces()
  let homeFreeBytes: number | null = null
  try {
    const fs = await statfs(Bun.env.HOME ?? ".")
    homeFreeBytes = fs.bavail * fs.bsize
  } catch {
    homeFreeBytes = null
  }

  return projectHostInventoryFixture({
    platform: platformRef(platform()),
    arch: arch(),
    cpuCores: cpus().length,
    cpuModel: cpus()[0]?.model ?? "unknown",
    totalMemoryBytes: totalmem(),
    freeMemoryBytes: freemem(),
    homeFreeBytes,
    networkInterfaceCount: net.interfaceCount,
    externalNetworkInterfaceCount: net.externalInterfaceCount,
    opencodeInstalled: Boolean(Bun.which("opencode")),
    geminiConfigured: Boolean((input.env ?? Bun.env).GEMINI_API_KEY || (input.env ?? Bun.env).GOOGLE_GENERATIVE_AI_API_KEY),
    appleFmReady: platform() === "darwin" && arch() === "arm64",
    psionicConfigured: Boolean((input.env ?? Bun.env).PYLON_PSIONIC_BASE_URL || (input.env ?? Bun.env).PROBE_PSIONIC_BASE_URL),
    psionicReady: false,
    psionicModelRefs: [],
    localModelRefs: [],
    accelerator: discoverNvidiaAccelerator(),
    now: (input.now ?? new Date()).toISOString(),
  })
}

function sanitizeAcceleratorModelRef(value: string | null | undefined) {
  if (!value) return null
  return /^accelerator\.[a-z0-9._-]+$/.test(value) ? value : `accelerator.${sanitizeRefSegment(value)}`
}

function sanitizeModelRefs(values: string[]) {
  return values
    .map((value) => value.trim())
    .filter((value) => /^model\.[a-z0-9._-]+$/.test(value))
}
