import { assertNoProviderSecretMaterial } from '@openagents/provider-account-schema'

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
