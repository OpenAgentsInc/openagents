import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import type { ProviderAccountProvider } from './provider-account-domain'
import { isoTimestampAfterIso } from './runtime-primitives'

export const PROVIDER_ACCOUNT_MANAGED_POLICY_VERSION =
  'provider-account-managed-policy:v1' as const

const PROVIDER_ACCOUNT_MANAGED_POLICY_COLLECTION =
  'provider_account_managed_policy_public'

const MANAGED_POLICY_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /raw[_ -]prompt/i,
  /raw[_ -]provider[_ -]response/i,
  /private[_ -]repo/i,
  /shell[_ -]output/i,
  /transcript:/i,
  /\b\/Users\/[^/]+\/work\//,
  /git@github\.com:[^\s]+/,
]

export type ProviderAccountManagedPolicyState =
  | 'active'
  | 'stale'
  | 'unknown'

export type ProviderAccountManagedPolicyDecisionKind =
  | 'provider_account_lease'
  | 'provider_account_run'
  | 'receipt'
  | 'team_budget'
  | 'work_order'

export type ProviderAccountManagedPolicyStatus =
  | 'allowed'
  | 'denied'
  | 'stale'
  | 'unknown'

export type ProviderAccountManagedPolicyBudgetDecision =
  | 'missing'
  | 'over_budget'
  | 'within_budget'

export type ProviderAccountManagedPolicyRetentionDecision =
  | 'allowed'
  | 'blocked'

export type ProviderAccountManagedPolicyTelemetryDecision =
  | 'aggregate'
  | 'local_only'
  | 'off'

export type ProviderAccountManagedPolicyRefs = Readonly<{
  budgetPolicyRef?: string | undefined
  devicePolicyRef?: string | undefined
  organizationPolicyRef?: string | undefined
  providerPolicyRef?: string | undefined
  repositoryPolicyRef?: string | undefined
  retentionPolicyRef?: string | undefined
  teamPolicyRef?: string | undefined
  telemetryPolicyRef?: string | undefined
  userPolicyRef?: string | undefined
}>

export type ProviderAccountManagedPolicyAttachments = Readonly<{
  leaseRefs?: ReadonlyArray<string> | undefined
  receiptRefs?: ReadonlyArray<string> | undefined
  runRefs?: ReadonlyArray<string> | undefined
  workOrderRefs?: ReadonlyArray<string> | undefined
}>

export type ProviderAccountManagedPolicyInput = Readonly<{
  approvedUserGate: 'disabled' | 'enabled'
  approvedUserRefs?: ReadonlyArray<string> | undefined
  attachments?: ProviderAccountManagedPolicyAttachments | undefined
  budgetCaveatRefs?: ReadonlyArray<string> | undefined
  budgetDecision: ProviderAccountManagedPolicyBudgetDecision
  decisionKind: ProviderAccountManagedPolicyDecisionKind
  decisionRef: string
  evaluatedAt: string
  generatedAt: string
  policyRefs: ProviderAccountManagedPolicyRefs
  policyState: ProviderAccountManagedPolicyState
  provider: ProviderAccountProvider
  providerAllowlist?: ReadonlyArray<ProviderAccountProvider> | undefined
  providerDisallowReasonRefs?: ReadonlyArray<string> | undefined
  requestingUserRef: string
  retentionCaveatRefs?: ReadonlyArray<string> | undefined
  retentionDecision: ProviderAccountManagedPolicyRetentionDecision
  snapshotRef?: string | undefined
  staleAfterMs: number
  telemetryCaveatRefs?: ReadonlyArray<string> | undefined
  telemetryDecision: ProviderAccountManagedPolicyTelemetryDecision
}>

export type ProviderAccountManagedPolicyProjection = Readonly<{
  generatedAt: string
  managedPolicyVersion: typeof PROVIDER_ACCOUNT_MANAGED_POLICY_VERSION
  decisionKind: ProviderAccountManagedPolicyDecisionKind
  decisionRef: string
  effectivePolicyRef: string
  evaluatedAt: string
  staleAt: string
  ageMs: number
  policyState: ProviderAccountManagedPolicyState
  status: ProviderAccountManagedPolicyStatus
  provider: ProviderAccountProvider
  providerAllowlist: ReadonlyArray<ProviderAccountProvider>
  providerDisallowReasonRefs: ReadonlyArray<string>
  approvedUserGate: 'disabled' | 'enabled'
  requestingUserRef: string
  approvedUserRefs: ReadonlyArray<string>
  budgetDecision: ProviderAccountManagedPolicyBudgetDecision
  retentionDecision: ProviderAccountManagedPolicyRetentionDecision
  telemetryDecision: ProviderAccountManagedPolicyTelemetryDecision
  governedByRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  denialReasonRefs: ReadonlyArray<string>
  attachmentRefs: Readonly<{
    leaseRefs: ReadonlyArray<string>
    receiptRefs: ReadonlyArray<string>
    runRefs: ReadonlyArray<string>
    workOrderRefs: ReadonlyArray<string>
  }>
}>

class ProviderAccountManagedPolicyUnsafe extends Error {
  constructor(context: string) {
    super(`${context} contains private managed policy material.`)
    this.name = 'ProviderAccountManagedPolicyUnsafe'
  }
}

const assertNoPrivateManagedPolicyMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const json = typeof value === 'string' ? value : JSON.stringify(value)

  if (MANAGED_POLICY_PRIVATE_MARKERS.some(marker => marker.test(json))) {
    throw new ProviderAccountManagedPolicyUnsafe(context)
  }
}

const safeRef = (field: string, value: string): string => {
  assertNoPrivateManagedPolicyMaterial(value, field)

  return value.trim()
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const governedByRefs = (
  refs: ProviderAccountManagedPolicyRefs,
): ReadonlyArray<string> =>
  safeRefs('provider-account-managed-policy.governedByRef', [
    refs.organizationPolicyRef,
    refs.teamPolicyRef,
    refs.repositoryPolicyRef,
    refs.userPolicyRef,
    refs.devicePolicyRef,
    refs.providerPolicyRef,
    refs.budgetPolicyRef,
    refs.retentionPolicyRef,
    refs.telemetryPolicyRef,
  ].filter((ref): ref is string => ref !== undefined))

const staleAt = (evaluatedAt: string, staleAfterMs: number): string =>
  isoTimestampAfterIso(evaluatedAt, staleAfterMs)

const ageMs = (generatedAt: string, evaluatedAt: string): number =>
  Math.max(0, Date.parse(generatedAt) - Date.parse(evaluatedAt))

const activeDenialRefs = (
  input: ProviderAccountManagedPolicyInput,
  decisionRef: string,
): ReadonlyArray<string> => [
  ...((input.providerAllowlist ?? []).includes(input.provider)
    ? []
    : [
        `provider-account-managed-policy-denial:${decisionRef}:provider_disallowed:${input.provider}`,
      ]),
  ...(input.approvedUserGate === 'enabled' &&
  !(input.approvedUserRefs ?? []).includes(input.requestingUserRef)
    ? [
        `provider-account-managed-policy-denial:${decisionRef}:user_not_approved`,
      ]
    : []),
  ...(input.budgetDecision === 'within_budget'
    ? []
    : [
        `provider-account-managed-policy-denial:${decisionRef}:budget_${input.budgetDecision}`,
      ]),
  ...(input.retentionDecision === 'allowed'
    ? []
    : [
        `provider-account-managed-policy-denial:${decisionRef}:retention_blocked`,
      ]),
]

const stateDenialRefs = (
  input: ProviderAccountManagedPolicyInput,
  decisionRef: string,
  observedAgeMs: number,
): ReadonlyArray<string> => {
  if (input.policyState === 'unknown') {
    return [`provider-account-managed-policy-denial:${decisionRef}:policy_unknown`]
  }

  if (input.policyState === 'stale' || observedAgeMs > input.staleAfterMs) {
    return [`provider-account-managed-policy-denial:${decisionRef}:policy_stale`]
  }

  return []
}

const status = (
  input: ProviderAccountManagedPolicyInput,
  observedAgeMs: number,
  denialReasonRefs: ReadonlyArray<string>,
): ProviderAccountManagedPolicyStatus => {
  if (input.policyState === 'unknown') {
    return 'unknown'
  }

  if (input.policyState === 'stale' || observedAgeMs > input.staleAfterMs) {
    return 'stale'
  }

  return denialReasonRefs.length === 0 ? 'allowed' : 'denied'
}

export const resolveProviderAccountManagedPolicy = (
  input: ProviderAccountManagedPolicyInput,
): ProviderAccountManagedPolicyProjection => {
  const decisionRef = safeRef(
    'provider-account-managed-policy.decisionRef',
    input.decisionRef,
  )
  const effectivePolicyRef = safeRef(
    'provider-account-managed-policy.effectivePolicyRef',
    input.snapshotRef ??
      `provider-account-effective-policy:${decisionRef}:${input.evaluatedAt}`,
  )
  const observedAgeMs = ageMs(input.generatedAt, input.evaluatedAt)
  const stateDenials = stateDenialRefs(input, decisionRef, observedAgeMs)
  const denialReasonRefs =
    stateDenials.length > 0
      ? stateDenials
      : activeDenialRefs(input, decisionRef)
  const projection: ProviderAccountManagedPolicyProjection = {
    generatedAt: input.generatedAt,
    managedPolicyVersion: PROVIDER_ACCOUNT_MANAGED_POLICY_VERSION,
    decisionKind: input.decisionKind,
    decisionRef,
    effectivePolicyRef,
    evaluatedAt: input.evaluatedAt,
    staleAt: staleAt(input.evaluatedAt, input.staleAfterMs),
    ageMs: observedAgeMs,
    policyState: input.policyState,
    status: status(input, observedAgeMs, denialReasonRefs),
    provider: input.provider,
    providerAllowlist: [...(input.providerAllowlist ?? [])],
    providerDisallowReasonRefs: safeRefs(
      'provider-account-managed-policy.providerDisallowReasonRefs',
      input.providerDisallowReasonRefs,
    ),
    approvedUserGate: input.approvedUserGate,
    requestingUserRef: safeRef(
      'provider-account-managed-policy.requestingUserRef',
      input.requestingUserRef,
    ),
    approvedUserRefs: safeRefs(
      'provider-account-managed-policy.approvedUserRefs',
      input.approvedUserRefs,
    ),
    budgetDecision: input.budgetDecision,
    retentionDecision: input.retentionDecision,
    telemetryDecision: input.telemetryDecision,
    governedByRefs: governedByRefs(input.policyRefs),
    caveatRefs: [
      ...safeRefs(
        'provider-account-managed-policy.budgetCaveatRefs',
        input.budgetCaveatRefs,
      ),
      ...safeRefs(
        'provider-account-managed-policy.retentionCaveatRefs',
        input.retentionCaveatRefs,
      ),
      ...safeRefs(
        'provider-account-managed-policy.telemetryCaveatRefs',
        input.telemetryCaveatRefs,
      ),
    ],
    denialReasonRefs: [
      ...denialReasonRefs,
      ...safeRefs(
        'provider-account-managed-policy.providerDisallowReasonRefs',
        denialReasonRefs.some(ref => ref.includes(':provider_disallowed:'))
          ? input.providerDisallowReasonRefs
          : [],
      ),
    ],
    attachmentRefs: {
      leaseRefs: safeRefs(
        'provider-account-managed-policy.attachment.leaseRefs',
        input.attachments?.leaseRefs,
      ),
      receiptRefs: safeRefs(
        'provider-account-managed-policy.attachment.receiptRefs',
        input.attachments?.receiptRefs,
      ),
      runRefs: safeRefs(
        'provider-account-managed-policy.attachment.runRefs',
        input.attachments?.runRefs,
      ),
      workOrderRefs: safeRefs(
        'provider-account-managed-policy.attachment.workOrderRefs',
        input.attachments?.workOrderRefs,
      ),
    },
  }

  assertNoPrivateManagedPolicyMaterial(
    projection,
    PROVIDER_ACCOUNT_MANAGED_POLICY_COLLECTION,
  )

  return projection
}
