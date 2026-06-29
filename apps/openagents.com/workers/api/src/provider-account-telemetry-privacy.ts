import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import type {
  ProviderAccountHealth,
  ProviderAccountProvider,
  ProviderAccountStatus,
} from './provider-account-domain'
import { isoTimestampAfterIso } from './runtime-primitives'

export const PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_VERSION =
  'provider-account-telemetry-privacy:v1' as const

const PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_COLLECTION =
  'provider_account_telemetry_privacy_public'

const TELEMETRY_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /raw[_ -]prompt/i,
  /raw[_ -]provider[_ -]response/i,
  /private[_ -]repo/i,
  /shell[_ -]output/i,
  /transcript:/i,
  /\b\/Users\/[^/]+\/work\//,
  /git@github\.com:[^\s]+/,
]

export type ProviderAccountTelemetryMode =
  | 'aggregate'
  | 'local_only'
  | 'off'

export type ProviderAccountTelemetrySharingPolicy =
  | 'approved_users_only'
  | 'local_only'
  | 'opt_out'

export type ProviderAccountTelemetryFreshness = 'fresh' | 'stale'

export type ProviderAccountTelemetryStatus =
  | 'ready'
  | 'blocked'
  | 'local_only'
  | 'disabled'

export type ProviderAccountTelemetryMetricKind =
  | 'account_health'
  | 'cooldown'
  | 'lease_utilization'
  | 'low_credit'
  | 'provider_routing'
  | 'rate_limit'
  | 'reconnect'
  | 'reset_hint'

export type ProviderAccountTelemetryProviderAccountClass =
  | 'aggregate'
  | 'candidate'
  | 'connected'
  | 'leased'
  | 'blocked'

export type ProviderAccountTelemetryMetricStatus =
  | ProviderAccountHealth
  | ProviderAccountStatus
  | 'cooling_down'
  | 'limited'
  | 'low_credit'
  | 'ok'
  | 'reset_pending'
  | 'route_candidate'
  | 'route_skipped'

export type ProviderAccountTelemetryMetricInput = Readonly<{
  caveatRefs?: ReadonlyArray<string> | undefined
  counter?: number | undefined
  durationMs?: number | undefined
  kind: ProviderAccountTelemetryMetricKind
  metricRef: string
  provider: ProviderAccountProvider | 'mixed'
  providerAccountClass: ProviderAccountTelemetryProviderAccountClass
  status: ProviderAccountTelemetryMetricStatus
}>

export type ProviderAccountTelemetryMetricProjection =
  ProviderAccountTelemetryMetricInput & Readonly<{
    caveatRefs: ReadonlyArray<string>
    valueKind: 'counter' | 'duration' | 'status'
  }>

export type ProviderAccountTelemetryPrivacyInput = Readonly<{
  caveatRefs?: ReadonlyArray<string> | undefined
  debugBundleRefs?: ReadonlyArray<string> | undefined
  generatedAt: string
  metrics: ReadonlyArray<ProviderAccountTelemetryMetricInput>
  observedAt: string
  projectionRef: string
  redactionFixtureRefs?: ReadonlyArray<string> | undefined
  sharingPolicy: ProviderAccountTelemetrySharingPolicy
  sourceRefs?: ReadonlyArray<string> | undefined
  staleAfterMs: number
  supportBundleRefs?: ReadonlyArray<string> | undefined
  telemetryMode: ProviderAccountTelemetryMode
}>

export type ProviderAccountTelemetryPrivacyProjection = Readonly<{
  generatedAt: string
  telemetryVersion: typeof PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_VERSION
  projectionRef: string
  telemetryMode: ProviderAccountTelemetryMode
  sharingPolicy: ProviderAccountTelemetrySharingPolicy
  status: ProviderAccountTelemetryStatus
  freshness: ProviderAccountTelemetryFreshness
  observedAt: string
  staleAt: string
  ageMs: number
  metricRefs: ReadonlyArray<string>
  metrics: ReadonlyArray<ProviderAccountTelemetryMetricProjection>
  redactionFixtureRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  debugBundleRefs: ReadonlyArray<string>
  supportBundleRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

const REDACTION_REQUIRED_METRIC_KINDS: ReadonlySet<ProviderAccountTelemetryMetricKind> =
  new Set([
    'account_health',
    'cooldown',
    'low_credit',
    'rate_limit',
    'reconnect',
    'reset_hint',
  ])

class ProviderAccountTelemetryPrivacyUnsafe extends Error {
  constructor(context: string) {
    super(`${context} contains private telemetry material.`)
    this.name = 'ProviderAccountTelemetryPrivacyUnsafe'
  }
}

const assertNoPrivateTelemetryMaterial = (value: unknown, context: string): void => {
  assertNoProviderSecretMaterial(value, context)

  const json = typeof value === 'string' ? value : JSON.stringify(value)

  if (TELEMETRY_PRIVATE_MARKERS.some(marker => marker.test(json))) {
    throw new ProviderAccountTelemetryPrivacyUnsafe(context)
  }
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  (values ?? []).map(value => {
    assertNoPrivateTelemetryMaterial(value, field)

    return value.trim()
  })

const metricValueKind = (
  metric: ProviderAccountTelemetryMetricInput,
): ProviderAccountTelemetryMetricProjection['valueKind'] => {
  if (metric.counter !== undefined) {
    return 'counter'
  }

  if (metric.durationMs !== undefined) {
    return 'duration'
  }

  return 'status'
}

const sanitizeMetric = (
  metric: ProviderAccountTelemetryMetricInput,
): ProviderAccountTelemetryMetricProjection => {
  assertNoPrivateTelemetryMaterial(metric, 'provider-account-telemetry.metric')

  return {
    caveatRefs: safeRefs(
      'provider-account-telemetry.metric.caveatRefs',
      metric.caveatRefs,
    ),
    counter: metric.counter,
    durationMs: metric.durationMs,
    kind: metric.kind,
    metricRef: metric.metricRef.trim(),
    provider: metric.provider,
    providerAccountClass: metric.providerAccountClass,
    status: metric.status,
    valueKind: metricValueKind(metric),
  }
}

const staleAt = (observedAt: string, staleAfterMs: number): string =>
  isoTimestampAfterIso(observedAt, staleAfterMs)

const ageMs = (generatedAt: string, observedAt: string): number =>
  Math.max(0, Date.parse(generatedAt) - Date.parse(observedAt))

export const projectProviderAccountTelemetryPrivacy = (
  input: ProviderAccountTelemetryPrivacyInput,
): ProviderAccountTelemetryPrivacyProjection => {
  assertNoPrivateTelemetryMaterial(
    input.projectionRef,
    'provider-account-telemetry.projectionRef',
  )

  const redactionFixtureRefs = safeRefs(
    'provider-account-telemetry.redactionFixtureRefs',
    input.redactionFixtureRefs,
  )
  const missingFixtureKinds = [
    ...new Set(
      input.metrics
        .filter(metric => REDACTION_REQUIRED_METRIC_KINDS.has(metric.kind))
        .map(metric => metric.kind),
    ),
  ].filter(() => redactionFixtureRefs.length === 0)
  const blockerRefs = [
    ...missingFixtureKinds.map(
      kind =>
        `provider-account-telemetry-blocker:${input.projectionRef}:missing-redaction-fixture:${kind}`,
    ),
  ]
  const sanitizedMetrics = input.metrics.map(sanitizeMetric)
  const visibleMetrics =
    input.telemetryMode === 'aggregate' ? sanitizedMetrics : []
  const observedAgeMs = ageMs(input.generatedAt, input.observedAt)
  const projection: ProviderAccountTelemetryPrivacyProjection = {
    generatedAt: input.generatedAt,
    telemetryVersion: PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_VERSION,
    projectionRef: input.projectionRef.trim(),
    telemetryMode: input.telemetryMode,
    sharingPolicy: input.sharingPolicy,
    status:
      input.telemetryMode === 'off'
        ? 'disabled'
        : input.telemetryMode === 'local_only'
          ? 'local_only'
          : blockerRefs.length === 0
            ? 'ready'
            : 'blocked',
    freshness:
      observedAgeMs <= input.staleAfterMs && Number.isFinite(observedAgeMs)
        ? 'fresh'
        : 'stale',
    observedAt: input.observedAt,
    staleAt: staleAt(input.observedAt, input.staleAfterMs),
    ageMs: observedAgeMs,
    metricRefs: safeRefs(
      'provider-account-telemetry.metricRef',
      input.metrics.map(metric => metric.metricRef),
    ),
    metrics: visibleMetrics,
    redactionFixtureRefs,
    caveatRefs: safeRefs('provider-account-telemetry.caveatRefs', input.caveatRefs),
    sourceRefs: safeRefs('provider-account-telemetry.sourceRefs', input.sourceRefs),
    debugBundleRefs: safeRefs(
      'provider-account-telemetry.debugBundleRefs',
      input.debugBundleRefs,
    ),
    supportBundleRefs: safeRefs(
      'provider-account-telemetry.supportBundleRefs',
      input.supportBundleRefs,
    ),
    blockerRefs,
  }

  assertNoPrivateTelemetryMaterial(
    projection,
    PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_COLLECTION,
  )

  return projection
}
