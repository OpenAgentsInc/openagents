export const EFFECTIVE_CONFIG_LAYER_ORDER = [
  "defaults",
  "project",
  "user",
  "runtime",
] as const

export type EffectiveConfigLayerName =
  (typeof EFFECTIVE_CONFIG_LAYER_ORDER)[number]

export type ProviderConfig = {
  readonly providerId: string
  readonly model: string
  readonly accountRef: string | null
}

export type BudgetConfig = {
  readonly maxTokens: number | null
  readonly maxCostUsd: number | null
}

export type ApprovalConfig = {
  readonly mode: "manual" | "auto" | "denied"
  readonly requireApprovalFor: readonly string[]
}

export type TelemetryConfig = {
  readonly enabled: boolean
  readonly level: "off" | "errors" | "aggregate" | "debug"
}

export type RetentionConfig = {
  readonly class: "short" | "standard" | "long"
  readonly maxAgeDays: number
}

export type RoutingConfig = {
  readonly mode: "local_only" | "provider_peers" | "managed"
  readonly allowedProviderIds: readonly string[]
}

export type EffectiveConfig = {
  readonly provider: ProviderConfig
  readonly budget: BudgetConfig
  readonly approval: ApprovalConfig
  readonly telemetry: TelemetryConfig
  readonly retention: RetentionConfig
  readonly routing: RoutingConfig
}

export type PartialEffectiveConfig = {
  readonly provider?: Partial<ProviderConfig>
  readonly budget?: Partial<BudgetConfig>
  readonly approval?: Partial<ApprovalConfig>
  readonly telemetry?: Partial<TelemetryConfig>
  readonly retention?: Partial<RetentionConfig>
  readonly routing?: Partial<RoutingConfig>
}

export type EffectiveConfigLayer = {
  readonly layer: EffectiveConfigLayerName
  readonly config: PartialEffectiveConfig
}

export type EffectiveConfigProvenanceEntry = {
  readonly layer: EffectiveConfigLayerName
}

export type EffectiveConfigProvenance = {
  readonly provider: Record<keyof ProviderConfig, EffectiveConfigProvenanceEntry>
  readonly budget: Record<keyof BudgetConfig, EffectiveConfigProvenanceEntry>
  readonly approval: Record<keyof ApprovalConfig, EffectiveConfigProvenanceEntry>
  readonly telemetry: Record<keyof TelemetryConfig, EffectiveConfigProvenanceEntry>
  readonly retention: Record<keyof RetentionConfig, EffectiveConfigProvenanceEntry>
  readonly routing: Record<keyof RoutingConfig, EffectiveConfigProvenanceEntry>
}

export type EffectiveConfigSnapshot = {
  readonly config: EffectiveConfig
  readonly provenance: EffectiveConfigProvenance
}

export const DEFAULT_EFFECTIVE_CONFIG: EffectiveConfig = {
  provider: {
    providerId: "chatgpt_codex",
    model: "default",
    accountRef: null,
  },
  budget: {
    maxTokens: null,
    maxCostUsd: null,
  },
  approval: {
    mode: "manual",
    requireApprovalFor: ["write", "network", "shell"],
  },
  telemetry: {
    enabled: true,
    level: "aggregate",
  },
  retention: {
    class: "standard",
    maxAgeDays: 30,
  },
  routing: {
    mode: "local_only",
    allowedProviderIds: ["chatgpt_codex"],
  },
}

const DEFAULT_PROVENANCE: EffectiveConfigProvenance = {
  provider: {
    providerId: { layer: "defaults" },
    model: { layer: "defaults" },
    accountRef: { layer: "defaults" },
  },
  budget: {
    maxTokens: { layer: "defaults" },
    maxCostUsd: { layer: "defaults" },
  },
  approval: {
    mode: { layer: "defaults" },
    requireApprovalFor: { layer: "defaults" },
  },
  telemetry: {
    enabled: { layer: "defaults" },
    level: { layer: "defaults" },
  },
  retention: {
    class: { layer: "defaults" },
    maxAgeDays: { layer: "defaults" },
  },
  routing: {
    mode: { layer: "defaults" },
    allowedProviderIds: { layer: "defaults" },
  },
}

const layerRank = (layer: EffectiveConfigLayerName): number =>
  EFFECTIVE_CONFIG_LAYER_ORDER.indexOf(layer)

const orderedLayers = (
  layers: readonly EffectiveConfigLayer[],
): readonly EffectiveConfigLayer[] =>
  [...layers].sort((left, right) => layerRank(left.layer) - layerRank(right.layer))

const applyDomain = <DomainName extends keyof EffectiveConfig>(
  config: EffectiveConfig,
  provenance: EffectiveConfigProvenance,
  domainName: DomainName,
  values: PartialEffectiveConfig[DomainName],
  layer: EffectiveConfigLayerName,
): void => {
  if (values === undefined) {
    return
  }

  const target = config[domainName] as Record<string, unknown>
  const targetProvenance = provenance[domainName] as Record<
    string,
    EffectiveConfigProvenanceEntry
  >

  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined) {
      target[key] = Array.isArray(value) ? [...value] : value
      targetProvenance[key] = { layer }
    }
  }
}

export function resolveEffectiveConfig(
  layers: readonly EffectiveConfigLayer[],
): EffectiveConfigSnapshot {
  const config = structuredClone(DEFAULT_EFFECTIVE_CONFIG)
  const provenance = structuredClone(DEFAULT_PROVENANCE)

  for (const layer of orderedLayers(layers)) {
    applyDomain(config, provenance, "provider", layer.config.provider, layer.layer)
    applyDomain(config, provenance, "budget", layer.config.budget, layer.layer)
    applyDomain(config, provenance, "approval", layer.config.approval, layer.layer)
    applyDomain(
      config,
      provenance,
      "telemetry",
      layer.config.telemetry,
      layer.layer,
    )
    applyDomain(
      config,
      provenance,
      "retention",
      layer.config.retention,
      layer.layer,
    )
    applyDomain(config, provenance, "routing", layer.config.routing, layer.layer)
  }

  return {
    config,
    provenance,
  }
}
