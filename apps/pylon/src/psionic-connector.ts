import { existsSync } from "node:fs"
import { createHash } from "node:crypto"
import { delimiter, isAbsolute, join } from "node:path"
import { Effect } from "effect"
import {
  makePsionicQwenClient,
  type PsionicQwenReadiness,
} from "../packages/runtime/src/index.js"
import { assertPublicProjectionSafe } from "./state.js"

export type PsionicConnectorPhase = "absent" | "configured" | "negotiated" | "refused"

export type PsionicConnectorState = {
  schema: "openagents.pylon.psionic_connector.v0.3"
  phase: PsionicConnectorPhase
  attachMode: "attach_existing"
  optionalInstall: true
  downloadsOnStartup: false
  service: {
    configured: boolean
    sourceRef: string
    endpointRefs: string[]
  }
  binary: {
    configured: boolean
    sourceRef: string
    binaryRef: string | null
  }
  capabilityRefs: string[]
  modelRefs: string[]
  observedModelRefs: string[]
  blockerRefs: string[]
  refusalRefs: string[]
  receiptRefs: string[]
  updatedAt: string
}

export type PsionicConnectorOptions = {
  env?: Readonly<Record<string, string | undefined>>
  fetch?: typeof fetch
  now?: Date
}

type BinaryDiscovery = {
  configured: boolean
  sourceRef: string
  binaryRef: string | null
  blockerRefs: string[]
}

const psionicCapabilityRefs = [
  "capability.psionic.connector.attach_existing",
  "capability.psionic.qwen35.inference.optional",
]

export async function inspectPsionicConnector(
  options: PsionicConnectorOptions = {},
): Promise<PsionicConnectorState> {
  const env = options.env ?? Bun.env
  const observedAt = (options.now ?? new Date()).toISOString()
  const binary = discoverPsionicBinary(env)
  const serviceConfigured = isServiceConfigured(env)

  if (!serviceConfigured && binary.blockerRefs.length > 0) {
    return connectorState({
      binary,
      blockerRefs: [...binary.blockerRefs, "blocker.psionic_qwen35.service_unconfigured"],
      phase: "refused",
      service: {
        configured: false,
        endpointRefs: [],
        sourceRef: "source.psionic.service.missing",
      },
      updatedAt: observedAt,
    })
  }

  if (!serviceConfigured && binary.configured) {
    return connectorState({
      binary,
      blockerRefs: ["blocker.psionic_qwen35.service_unconfigured"],
      phase: "configured",
      service: {
        configured: false,
        endpointRefs: [],
        sourceRef: "source.psionic.service.missing",
      },
      updatedAt: observedAt,
    })
  }

  if (!serviceConfigured && !binary.configured) {
    return connectorState({
      binary,
      blockerRefs: ["blocker.psionic_qwen35.connector_unconfigured"],
      phase: "absent",
      service: {
        configured: false,
        endpointRefs: [],
        sourceRef: "source.psionic.service.default_unconfigured",
      },
      updatedAt: observedAt,
    })
  }

  const readiness = await Effect.runPromise(
    makePsionicQwenClient({
      env,
      fetch: options.fetch,
      now: options.now,
    }).pipe(
      Effect.flatMap((client) => client.doctor()),
    ),
  )
  const blockerRefs = [...new Set([...binary.blockerRefs, ...readiness.blockerRefs])]
  const phase: PsionicConnectorPhase =
    blockerRefs.length > 0
      ? "refused"
      : readiness.ready
        ? "negotiated"
        : "configured"

  return connectorState({
    binary,
    blockerRefs,
    capabilityRefs: readiness.ready ? psionicCapabilityRefs : [],
    modelRefs: [...readiness.modelRefs],
    observedModelRefs: [...readiness.observedModelRefs],
    phase,
    receiptRefs: [stableRef("receipt.psionic.qwen35.availability", JSON.stringify(readiness.receipt))],
    refusalRefs: blockerRefs,
    service: {
      configured: true,
      endpointRefs: [...readiness.supportedEndpointRefs],
      sourceRef: serviceSourceRef(readiness),
    },
    updatedAt: observedAt,
  })
}

function connectorState(input: {
  binary: BinaryDiscovery
  blockerRefs: string[]
  capabilityRefs?: string[]
  modelRefs?: string[]
  observedModelRefs?: string[]
  phase: PsionicConnectorPhase
  receiptRefs?: string[]
  refusalRefs?: string[]
  service: PsionicConnectorState["service"]
  updatedAt: string
}): PsionicConnectorState {
  const state: PsionicConnectorState = {
    schema: "openagents.pylon.psionic_connector.v0.3",
    phase: input.phase,
    attachMode: "attach_existing",
    optionalInstall: true,
    downloadsOnStartup: false,
    service: input.service,
    binary: {
      configured: input.binary.configured,
      sourceRef: input.binary.sourceRef,
      binaryRef: input.binary.binaryRef,
    },
    capabilityRefs: input.capabilityRefs ?? [],
    modelRefs: input.modelRefs ?? [],
    observedModelRefs: input.observedModelRefs ?? [],
    blockerRefs: input.blockerRefs,
    refusalRefs: input.refusalRefs ?? input.blockerRefs,
    receiptRefs: input.receiptRefs ?? [],
    updatedAt: input.updatedAt,
  }
  assertPublicProjectionSafe(state)
  return state
}

function discoverPsionicBinary(env: Readonly<Record<string, string | undefined>>): BinaryDiscovery {
  const explicit = firstNonEmpty(env.PYLON_PSIONIC_BIN, env.PSIONIC_BIN)
  if (explicit !== undefined) {
    const exists = isAbsolute(explicit) && existsSync(explicit)
    return {
      configured: exists,
      sourceRef: "source.psionic.binary.env",
      binaryRef: exists ? "binary.psionic.user_provided" : null,
      blockerRefs: exists ? [] : ["blocker.psionic_qwen35.binary_missing"],
    }
  }

  const pathHit = findOnPath(env.PATH, ["psionic-openai-server", "psionic-sidecar"])
  if (pathHit !== null) {
    return {
      configured: true,
      sourceRef: "source.psionic.binary.path",
      binaryRef: `binary.psionic.${pathHit}`,
      blockerRefs: [],
    }
  }

  return {
    configured: false,
    sourceRef: "source.psionic.binary.not_found",
    binaryRef: null,
    blockerRefs: [],
  }
}

function findOnPath(pathValue: string | undefined, candidates: string[]): string | null {
  if (!pathValue) return null
  for (const directory of pathValue.split(delimiter)) {
    if (!directory) continue
    for (const candidate of candidates) {
      if (existsSync(join(directory, candidate))) return candidate.replace(/[^a-z0-9_]+/gi, "_").toLowerCase()
    }
  }
  return null
}

function isServiceConfigured(env: Readonly<Record<string, string | undefined>>) {
  return firstNonEmpty(env.PYLON_PSIONIC_BASE_URL, env.PROBE_PSIONIC_BASE_URL) !== undefined
}

function serviceSourceRef(readiness: PsionicQwenReadiness) {
  return `source.psionic.service.${readiness.profile.baseUrlSource.toLowerCase()}`
}

function firstNonEmpty(...values: Array<string | undefined>) {
  return values.find((value) => value !== undefined && value.trim().length > 0)?.trim()
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}
