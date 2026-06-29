import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

export const PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_VERSION =
  'provider-account-effective-config:v1' as const

const PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_COLLECTION =
  'provider_account_effective_config_public'

export const PROVIDER_ACCOUNT_CONFIG_PRECEDENCE = [
  'default',
  'environment',
  'organization',
  'team',
  'repository',
  'user',
  'device',
  'runtime',
] as const

export type ProviderAccountConfigLayerName =
  (typeof PROVIDER_ACCOUNT_CONFIG_PRECEDENCE)[number]

export type ProviderAccountConfigDecisionKind =
  | 'approval'
  | 'budget'
  | 'provider'
  | 'retention'
  | 'routing'
  | 'telemetry'

export type ProviderAccountConfigKey =
  | 'approval.mode'
  | 'budget.maxCents'
  | 'provider.allowlist'
  | 'retention.class'
  | 'routing.mode'
  | 'telemetry.mode'

export type ProviderAccountConfigValue =
  | boolean
  | number
  | string
  | ReadonlyArray<string>

export type ProviderAccountConfigSetting = Readonly<{
  caveatRefs?: ReadonlyArray<string> | undefined
  configRef: string
  key: ProviderAccountConfigKey
  value: ProviderAccountConfigValue
}>

export type ProviderAccountConfigLayer = Readonly<{
  layer: ProviderAccountConfigLayerName
  settings: ReadonlyArray<ProviderAccountConfigSetting>
}>

export type ProviderAccountConfigResolutionInput = Readonly<{
  decisionKind: ProviderAccountConfigDecisionKind
  decisionRef: string
  generatedAt: string
  layers: ReadonlyArray<ProviderAccountConfigLayer>
  requiredKeys: ReadonlyArray<ProviderAccountConfigKey>
  snapshotRef?: string | undefined
}>

export type ProviderAccountEffectiveConfigSetting = Readonly<{
  caveatRefs: ReadonlyArray<string>
  configRef: string
  key: ProviderAccountConfigKey
  sourceLayer: ProviderAccountConfigLayerName
  valueTag: string
}>

export type ProviderAccountEffectiveConfigProjection = Readonly<{
  generatedAt: string
  configVersion: typeof PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_VERSION
  decisionKind: ProviderAccountConfigDecisionKind
  decisionRef: string
  effectiveConfigRef: string
  status: 'resolved' | 'blocked'
  settings: ReadonlyArray<ProviderAccountEffectiveConfigSetting>
  blockerRefs: ReadonlyArray<string>
  denialReasonRef: string | null
}>

// Optional per-account budget fields layered onto the existing config
// precedent (`budget.maxCents`). These are advisory accounting surfaces only.
//
// authorityBoundary: this module emits typed over-budget EVENTS. It has NO
// enforcement authority — nothing here blocks, throttles, cools down, or
// de-leases a provider account. Lease selection authority remains entirely in
// the M8/M9 pool/lease engine, which does not read these events. An over-budget
// event is an operator-facing signal, not a gate. Promoting any of these
// signals into enforcement requires a separate approved authority path.
export const PROVIDER_ACCOUNT_BUDGET_VERSION =
  'provider-account-budget:v1' as const

export type ProviderAccountBudgetField =
  | 'budget.totalTokens'
  | 'budget.windowTokens'

// A resolved per-account budget. Refs only; never carries spend secrets,
// invoices, or provider credentials.
export type ProviderAccountBudget = Readonly<{
  providerAccountRef: string
  // Optional ceilings. `undefined` means "no budget configured for this field",
  // which can never be over budget.
  maxTotalTokens?: number | undefined
  maxWindowTokens?: number | undefined
  windowLabel?: string | undefined
}>

// Observed usage for one provider account, sourced from the token ledger
// aggregate. Ref-only counts; no credentials.
export type ProviderAccountBudgetObservedUsage = Readonly<{
  providerAccountRef: string
  totalTokens: number
  windowTokens: number
}>

export type ProviderAccountOverBudgetEvent = Readonly<{
  budgetVersion: typeof PROVIDER_ACCOUNT_BUDGET_VERSION
  // authorityBoundary: advisory only. No consumer may treat this as a gate.
  authority: 'advisory_event_only'
  providerAccountRef: string
  field: ProviderAccountBudgetField
  limit: number
  observed: number
  overBy: number
  windowLabel: string | null
  eventRef: string
}>

const overBudgetEvent = (
  input: Readonly<{
    providerAccountRef: string
    field: ProviderAccountBudgetField
    limit: number
    observed: number
    windowLabel: string | null
  }>,
): ProviderAccountOverBudgetEvent => ({
  budgetVersion: PROVIDER_ACCOUNT_BUDGET_VERSION,
  authority: 'advisory_event_only',
  providerAccountRef: safeRef(
    'provider-account-budget.providerAccountRef',
    input.providerAccountRef,
  ),
  field: input.field,
  limit: input.limit,
  observed: input.observed,
  overBy: input.observed - input.limit,
  windowLabel: input.windowLabel,
  eventRef: `provider-account-over-budget:${input.providerAccountRef}:${input.field}`,
})

const isPositiveBudget = (value: number | undefined): value is number =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0

// Pure, deterministic budget evaluation. Emits at most one event per budget
// field per account, only when configured and exceeded. Emits nothing when no
// budget is configured (the common case) or when usage is within budget.
//
// authorityBoundary: callers surface these events for visibility. They must not
// use the return value to deny leases, mutate accounts, or block work.
export const evaluateProviderAccountBudgetEvents = (
  input: Readonly<{
    budgets: ReadonlyArray<ProviderAccountBudget>
    usageByAccountRef: ReadonlyArray<ProviderAccountBudgetObservedUsage>
  }>,
): ReadonlyArray<ProviderAccountOverBudgetEvent> => {
  const usageByRef = new Map(
    input.usageByAccountRef.map(usage => [usage.providerAccountRef, usage]),
  )

  return input.budgets
    .flatMap(budget => {
      const usage = usageByRef.get(budget.providerAccountRef)

      if (usage === undefined) {
        return []
      }

      const events: Array<ProviderAccountOverBudgetEvent> = []

      if (
        isPositiveBudget(budget.maxTotalTokens) &&
        usage.totalTokens > budget.maxTotalTokens
      ) {
        events.push(
          overBudgetEvent({
            providerAccountRef: budget.providerAccountRef,
            field: 'budget.totalTokens',
            limit: budget.maxTotalTokens,
            observed: usage.totalTokens,
            windowLabel: null,
          }),
        )
      }

      if (
        isPositiveBudget(budget.maxWindowTokens) &&
        usage.windowTokens > budget.maxWindowTokens
      ) {
        events.push(
          overBudgetEvent({
            providerAccountRef: budget.providerAccountRef,
            field: 'budget.windowTokens',
            limit: budget.maxWindowTokens,
            observed: usage.windowTokens,
            windowLabel: budget.windowLabel ?? null,
          }),
        )
      }

      return events
    })
    .sort((left, right) =>
      left.eventRef.localeCompare(right.eventRef),
    )
}

const precedenceIndex = (layer: ProviderAccountConfigLayerName): number =>
  PROVIDER_ACCOUNT_CONFIG_PRECEDENCE.indexOf(layer)

const safeRef = (field: string, value: string): string => {
  assertNoProviderSecretMaterial(value, field)

  return value.trim()
}

const valueTag = (
  key: ProviderAccountConfigKey,
  value: ProviderAccountConfigValue,
): string | null => {
  assertNoProviderSecretMaterial(value, `provider-account-config.${key}`)

  if (key === 'provider.allowlist') {
    return Array.isArray(value) && value.length > 0
      ? `provider.allowlist:${value.length}`
      : null
  }

  if (key === 'budget.maxCents') {
    return typeof value === 'number' && Number.isFinite(value) && value >= 0
      ? 'budget.maxCents:configured'
      : null
  }

  if (key === 'approval.mode') {
    return value === 'manual' || value === 'auto' || value === 'denied'
      ? `approval.mode:${value}`
      : null
  }

  if (key === 'retention.class') {
    return value === 'short' || value === 'standard' || value === 'long'
      ? `retention.class:${value}`
      : null
  }

  if (key === 'routing.mode') {
    return value === 'disabled' ||
      value === 'codex_only' ||
      value === 'provider_peers'
      ? `routing.mode:${value}`
      : null
  }

  return value === 'off' || value === 'aggregate' || value === 'local_only'
    ? `telemetry.mode:${value}`
    : null
}

const chooseEffectiveSettings = (
  layers: ReadonlyArray<ProviderAccountConfigLayer>,
): ReadonlyArray<
  Readonly<{
    layer: ProviderAccountConfigLayerName
    setting: ProviderAccountConfigSetting
  }>
> => {
  const sorted = [...layers].sort(
    (left, right) => precedenceIndex(left.layer) - precedenceIndex(right.layer),
  )

  return [
    ...sorted
      .flatMap(layer =>
        layer.settings.map(setting => ({
          layer: layer.layer,
          setting,
        })),
      )
      .reduce(
        (byKey, entry) => byKey.set(entry.setting.key, entry),
        new Map<
          ProviderAccountConfigKey,
          Readonly<{
            layer: ProviderAccountConfigLayerName
            setting: ProviderAccountConfigSetting
          }>
        >(),
      )
      .values(),
  ].sort((left, right) => left.setting.key.localeCompare(right.setting.key))
}

export const resolveProviderAccountEffectiveConfig = (
  input: ProviderAccountConfigResolutionInput,
): ProviderAccountEffectiveConfigProjection => {
  const decisionRef = safeRef('provider-account-config.decisionRef', input.decisionRef)
  const effectiveConfigRef = safeRef(
    'provider-account-config.effectiveConfigRef',
    input.snapshotRef ?? `provider-account-effective-config:${decisionRef}`,
  )
  const effectiveSettings = chooseEffectiveSettings(input.layers)
  const settings = effectiveSettings.map(({ layer, setting }) => ({
    caveatRefs: (setting.caveatRefs ?? []).map(caveatRef =>
      safeRef('provider-account-config.caveatRef', caveatRef),
    ),
    configRef: safeRef('provider-account-config.configRef', setting.configRef),
    key: setting.key,
    sourceLayer: layer,
    valueTag: valueTag(setting.key, setting.value),
  }))
  const invalidSettings = settings
    .filter(setting => setting.valueTag === null)
    .map(
      setting =>
        `provider-account-config-blocker:${decisionRef}:invalid:${setting.key}`,
    )
  const presentKeys = new Set(settings.map(setting => setting.key))
  const missingSettings = input.requiredKeys
    .filter(key => !presentKeys.has(key))
    .map(key => `provider-account-config-blocker:${decisionRef}:missing:${key}`)
  const blockerRefs = [...invalidSettings, ...missingSettings]
  const projection: ProviderAccountEffectiveConfigProjection = {
    generatedAt: input.generatedAt,
    configVersion: PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_VERSION,
    decisionKind: input.decisionKind,
    decisionRef,
    effectiveConfigRef,
    status: blockerRefs.length === 0 ? 'resolved' : 'blocked',
    settings: settings.map(setting => ({
      ...setting,
      valueTag: setting.valueTag ?? 'invalid',
    })),
    blockerRefs,
    denialReasonRef:
      blockerRefs.length === 0
        ? null
        : `provider-account-config-denial:${decisionRef}`,
  }

  assertNoProviderSecretMaterial(
    projection,
    PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_COLLECTION,
  )

  return projection
}
