import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const OpenAgentsMdkServiceMode = S.Literals([
  'fake_provider',
  'hosted_platform',
  'self_hosted_mdkd_sidecar',
])
export type OpenAgentsMdkServiceMode = typeof OpenAgentsMdkServiceMode.Type

export const OpenAgentsMdkSidecarRuntime = S.Literals([
  'cloudflare_vpc_service',
  'gcp_cloud_run',
  'local_operator_only',
  'public_https_tunnel',
  'shc_node_service',
])
export type OpenAgentsMdkSidecarRuntime =
  typeof OpenAgentsMdkSidecarRuntime.Type

export const OpenAgentsMdkSidecarHealthStatus = S.Literals([
  'degraded',
  'healthy',
  'unknown',
  'unreachable',
])
export type OpenAgentsMdkSidecarHealthStatus =
  typeof OpenAgentsMdkSidecarHealthStatus.Type

export const OpenAgentsMdkSidecarEmergencyPause = S.Literals([
  'active',
  'inactive',
  'missing',
])
export type OpenAgentsMdkSidecarEmergencyPause =
  typeof OpenAgentsMdkSidecarEmergencyPause.Type

export const OpenAgentsMdkSidecarReadinessStatus = S.Literals([
  'blocked_emergency_pause',
  'blocked_missing_auth',
  'blocked_missing_route',
  'blocked_missing_storage',
  'blocked_unhealthy',
  'fake_provider_only',
  'hosted_platform_ready',
  'sidecar_ready',
])
export type OpenAgentsMdkSidecarReadinessStatus =
  typeof OpenAgentsMdkSidecarReadinessStatus.Type

export class OpenAgentsMdkSidecarAuthBoundary extends S.Class<OpenAgentsMdkSidecarAuthBoundary>(
  'OpenAgentsMdkSidecarAuthBoundary',
)({
  checkoutControlAuthRef: S.NullOr(S.String),
  emergencyPauseRef: S.NullOr(S.String),
  payoutControlAuthRef: S.NullOr(S.String),
  readOnlyStatusAuthRef: S.NullOr(S.String),
  webhookVerificationRef: S.NullOr(S.String),
}) {}

export class OpenAgentsMdkSidecarOptionInput extends S.Class<OpenAgentsMdkSidecarOptionInput>(
  'OpenAgentsMdkSidecarOptionInput',
)({
  auth: OpenAgentsMdkSidecarAuthBoundary,
  checkoutRouteConfigured: S.Boolean,
  emergencyPause: OpenAgentsMdkSidecarEmergencyPause,
  healthCheckedRef: S.NullOr(S.String),
  healthStatus: OpenAgentsMdkSidecarHealthStatus,
  mdkdVersionRef: S.NullOr(S.String),
  mode: OpenAgentsMdkServiceMode,
  observabilityRefs: S.Array(S.String),
  routeBindingRef: S.NullOr(S.String),
  runtime: OpenAgentsMdkSidecarRuntime,
  serviceRef: S.String,
  storageRefs: S.Array(S.String),
  walletReadinessRef: S.NullOr(S.String),
}) {}

export class OpenAgentsMdkSidecarOptionProjection extends S.Class<OpenAgentsMdkSidecarOptionProjection>(
  'OpenAgentsMdkSidecarOptionProjection',
)({
  authTierRefs: S.Array(S.String),
  checkoutCreationAllowed: S.Boolean,
  checkoutStatusLookupAllowed: S.Boolean,
  docsRefs: S.Array(S.String),
  failureClassRefs: S.Array(S.String),
  healthCheckedRef: S.NullOr(S.String),
  healthStatus: OpenAgentsMdkSidecarHealthStatus,
  mode: OpenAgentsMdkServiceMode,
  mdkdVersionRef: S.NullOr(S.String),
  nativeRuntimeInWorker: S.Literal(false),
  observabilityRefs: S.Array(S.String),
  operatorActionRefs: S.Array(S.String),
  payoutAuthorityOwner: S.Literal('nexus_treasury_policy'),
  payoutDispatchAllowed: S.Literal(false),
  readinessStatus: OpenAgentsMdkSidecarReadinessStatus,
  routeBindingRef: S.NullOr(S.String),
  runtime: OpenAgentsMdkSidecarRuntime,
  secretBoundaryRefs: S.Array(S.String),
  serviceRef: S.String,
  sourceRefs: S.Array(S.String),
  storageRefs: S.Array(S.String),
  walletReadinessRef: S.NullOr(S.String),
  workerCompatibilityPreserved: S.Literal(true),
}) {}

export class OpenAgentsMdkSidecarOptionUnsafe extends S.TaggedErrorClass<OpenAgentsMdkSidecarOptionUnsafe>()(
  'OpenAgentsMdkSidecarOptionUnsafe',
  {
    reason: S.String,
  },
) {}

const docsRefs = [
  'docs.mdk.self_hosted_mdkd_sidecar_option',
  'docs.mdk.moneydevkit_local_source_audit',
  'docs.mdk.omega_mdk_setup_audit',
]

const sourceRefs = [
  'source.moneydevkit.mdkd.9ffea5f',
  'source.moneydevkit.mdk_checkout.ff64215',
  'source.cloudflare.workers_service_bindings_and_vpc_docs',
]

const secretBoundaryRefs = [
  'boundary.mdkd.platform_auth.file_descriptor_or_worker_binding',
  'boundary.mdkd.recovery_input.file_descriptor_only_preferred',
  'boundary.mdkd.full_control_auth.private_sidecar_only',
  'boundary.mdkd.read_only_auth.private_sidecar_only',
  'boundary.mdkd.webhook_hmac.exact_source_verification',
  'boundary.omega.worker.redacted_refs_only',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeValuePattern =
  /(\/Users\/|\/home\/|\/var\/lib\/mdkd|access[_-]?token|authorization:\s*basic|authorization:\s*bearer|basic\s+[A-Za-z0-9+/=]{12,}|bearer\s+|bolt11|checkout[_-]?(id|secret)|cookie|customer[_-]?(email|name|value)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|http[_-]?password|invoice|lnbc|lntb|lnbcrt|lno1|lnurl1|mdk[_-]?(access[_-]?token|http[_-]?password|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(balance|hash|preimage|secret)|payout[_-]?(destination|target)|preimage|provider[_-]?(grant|token)|raw[_-]?(balance|invoice|payment|payload|wallet)|recovery[_-]?phrase|seed[_-]?phrase|sk-[a-z0-9]|treasury[_-]?secret|webhook[_-]?secret|\S+@\S+)/i

const uniqueSorted = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

export const openAgentsMdkSidecarOptionHasPrivateMaterial = (
  value: unknown,
): boolean =>
  stringValues(value).some(text =>
    containsProviderSecretMaterial(text) ||
    unsafeValuePattern.test(text)
  )

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafeRef = uniqueSorted(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    openAgentsMdkSidecarOptionHasPrivateMaterial(ref)
  )

  if (unsafeRef !== undefined) {
    throw new OpenAgentsMdkSidecarOptionUnsafe({
      reason: `${label} contains raw MDK, wallet, payment, credential, path, or private sidecar material.`,
    })
  }
}

const nullableRefs = (refs: ReadonlyArray<string | null>): ReadonlyArray<string> =>
  refs.filter((ref): ref is string => ref !== null)

const assertSafeInput = (input: OpenAgentsMdkSidecarOptionInput): void => {
  assertSafeRefs('MDK sidecar service refs', [
    input.serviceRef,
    ...nullableRefs([
      input.healthCheckedRef,
      input.mdkdVersionRef,
      input.routeBindingRef,
      input.walletReadinessRef,
    ]),
  ])
  assertSafeRefs('MDK sidecar auth refs', nullableRefs([
    input.auth.checkoutControlAuthRef,
    input.auth.emergencyPauseRef,
    input.auth.payoutControlAuthRef,
    input.auth.readOnlyStatusAuthRef,
    input.auth.webhookVerificationRef,
  ]))
  assertSafeRefs('MDK sidecar storage refs', input.storageRefs)
  assertSafeRefs('MDK sidecar observability refs', input.observabilityRefs)
}

const authTierRefs = (
  auth: OpenAgentsMdkSidecarAuthBoundary,
): ReadonlyArray<string> => uniqueSorted(nullableRefs([
  auth.readOnlyStatusAuthRef === null
    ? null
    : 'auth_tier.mdkd.read_only_status',
  auth.checkoutControlAuthRef === null
    ? null
    : 'auth_tier.mdkd.checkout_control',
  auth.payoutControlAuthRef === null
    ? null
    : 'auth_tier.mdkd.payout_control',
  auth.webhookVerificationRef === null
    ? null
    : 'auth_tier.mdkd.webhook_verification',
  auth.emergencyPauseRef === null
    ? null
    : 'auth_tier.mdkd.emergency_pause',
]))

const sidecarAuthComplete = (
  auth: OpenAgentsMdkSidecarAuthBoundary,
): boolean =>
  auth.readOnlyStatusAuthRef !== null &&
  auth.checkoutControlAuthRef !== null &&
  auth.payoutControlAuthRef !== null &&
  auth.webhookVerificationRef !== null &&
  auth.emergencyPauseRef !== null

const hostedPlatformAuthComplete = (
  auth: OpenAgentsMdkSidecarAuthBoundary,
): boolean =>
  auth.checkoutControlAuthRef !== null &&
  auth.webhookVerificationRef !== null &&
  auth.emergencyPauseRef !== null

const readinessStatus = (
  input: OpenAgentsMdkSidecarOptionInput,
): OpenAgentsMdkSidecarReadinessStatus => {
  if (input.emergencyPause === 'active') {
    return 'blocked_emergency_pause'
  }

  if (input.healthStatus === 'unreachable') {
    return 'blocked_unhealthy'
  }

  if (input.mode === 'fake_provider') {
    return 'fake_provider_only'
  }

  if (!input.checkoutRouteConfigured || input.routeBindingRef === null) {
    return 'blocked_missing_route'
  }

  if (input.mode === 'hosted_platform') {
    return hostedPlatformAuthComplete(input.auth)
      ? 'hosted_platform_ready'
      : 'blocked_missing_auth'
  }

  if (!sidecarAuthComplete(input.auth)) {
    return 'blocked_missing_auth'
  }

  if (
    input.mdkdVersionRef === null ||
    input.storageRefs.length === 0 ||
    input.walletReadinessRef === null
  ) {
    return 'blocked_missing_storage'
  }

  return input.healthStatus === 'healthy' || input.healthStatus === 'degraded'
    ? 'sidecar_ready'
    : 'blocked_unhealthy'
}

const failureClassRefsForStatus = (
  status: OpenAgentsMdkSidecarReadinessStatus,
): ReadonlyArray<string> => {
  const byStatus: Readonly<Record<OpenAgentsMdkSidecarReadinessStatus, ReadonlyArray<string>>> = {
    blocked_emergency_pause: ['failure.mdkd.emergency_pause_active'],
    blocked_missing_auth: ['failure.mdkd.auth_tier_missing'],
    blocked_missing_route: ['failure.mdkd.route_binding_missing'],
    blocked_missing_storage: ['failure.mdkd.storage_or_wallet_state_missing'],
    blocked_unhealthy: ['failure.mdkd.health_unavailable'],
    fake_provider_only: ['failure.mdkd.not_live_provider'],
    hosted_platform_ready: [],
    sidecar_ready: [],
  }

  return byStatus[status]
}

const operatorActionsForStatus = (
  status: OpenAgentsMdkSidecarReadinessStatus,
): ReadonlyArray<string> => {
  const byStatus: Readonly<Record<OpenAgentsMdkSidecarReadinessStatus, ReadonlyArray<string>>> = {
    blocked_emergency_pause: [
      'operator_action.review_pause_before_any_mdk_sidecar_call',
    ],
    blocked_missing_auth: [
      'operator_action.configure_read_only_checkout_payout_webhook_pause_auth',
    ],
    blocked_missing_route: [
      'operator_action.configure_mdk_checkout_route_binding',
    ],
    blocked_missing_storage: [
      'operator_action.configure_mdkd_wallet_storage_vss_sqlite_backup',
    ],
    blocked_unhealthy: ['operator_action.inspect_mdkd_health_and_logs'],
    fake_provider_only: ['operator_action.keep_using_fake_provider_tests'],
    hosted_platform_ready: ['operator_action.run_hosted_platform_smoke'],
    sidecar_ready: ['operator_action.run_sidecar_checkout_status_smoke'],
  }

  return byStatus[status]
}

export const planOpenAgentsMdkSidecarOption = (
  input: OpenAgentsMdkSidecarOptionInput,
): OpenAgentsMdkSidecarOptionProjection => {
  assertSafeInput(input)

  const status = readinessStatus(input)
  const projection: OpenAgentsMdkSidecarOptionProjection = {
    authTierRefs: authTierRefs(input.auth),
    checkoutCreationAllowed:
      status === 'hosted_platform_ready' || status === 'sidecar_ready',
    checkoutStatusLookupAllowed:
      status === 'hosted_platform_ready' ||
      status === 'sidecar_ready' ||
      input.auth.readOnlyStatusAuthRef !== null,
    docsRefs,
    failureClassRefs: failureClassRefsForStatus(status),
    healthCheckedRef: input.healthCheckedRef,
    healthStatus: input.healthStatus,
    mdkdVersionRef: input.mdkdVersionRef,
    mode: input.mode,
    nativeRuntimeInWorker: false,
    observabilityRefs: uniqueSorted(input.observabilityRefs),
    operatorActionRefs: operatorActionsForStatus(status),
    payoutAuthorityOwner: 'nexus_treasury_policy',
    payoutDispatchAllowed: false,
    readinessStatus: status,
    routeBindingRef: input.routeBindingRef,
    runtime: input.runtime,
    secretBoundaryRefs,
    serviceRef: input.serviceRef,
    sourceRefs,
    storageRefs: uniqueSorted(input.storageRefs),
    walletReadinessRef: input.walletReadinessRef,
    workerCompatibilityPreserved: true,
  }

  if (openAgentsMdkSidecarOptionHasPrivateMaterial(projection)) {
    throw new OpenAgentsMdkSidecarOptionUnsafe({
      reason:
        'MDK sidecar option projection contains raw MDK, wallet, payment, credential, path, or private sidecar material.',
    })
  }

  return projection
}
