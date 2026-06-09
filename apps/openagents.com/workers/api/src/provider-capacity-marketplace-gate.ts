import { containsProviderSecretMaterial } from '@openagents/provider-account-schema'
import { Schema as S } from 'effect'

export const ProviderCapacityMarketplaceProvider = S.Literals([
  'chatgpt_codex',
  'claude',
  'venice',
])
export type ProviderCapacityMarketplaceProvider =
  typeof ProviderCapacityMarketplaceProvider.Type

export const ProviderCapacityPricingMode = S.Literals([
  'agentic_work',
  'base_inference_resale',
])
export type ProviderCapacityPricingMode =
  typeof ProviderCapacityPricingMode.Type

export const ProviderCapacityMarketplaceState = S.Literals([
  'planned_unsupported',
  'blocked_unsupported',
  'blocked',
  'grant_ready',
  'dispatch_ready',
  'assignment_receipted',
  'settled',
])
export type ProviderCapacityMarketplaceState =
  typeof ProviderCapacityMarketplaceState.Type

export const ProviderCapacityConnectorState = S.Literals([
  'unsupported',
  'configured',
  'healthy',
  'assignable',
  'payable',
  'settled',
])
export type ProviderCapacityConnectorState =
  typeof ProviderCapacityConnectorState.Type

export const ProviderCapacityMarketplaceGate = S.Struct({
  accountSchemaRefs: S.Array(S.String),
  assignmentModeRefs: S.Array(S.String),
  assignmentDispatchAllowed: S.Boolean,
  assignmentDispatchRefs: S.Array(S.String),
  assignmentReceiptClaimAllowed: S.Boolean,
  assignmentReceiptRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  connectorHealthRefs: S.Array(S.String),
  connectorState: ProviderCapacityConnectorState,
  marketableCapacityCopyAllowed: S.Boolean,
  meteringReceiptRefs: S.Array(S.String),
  paidSettlementClaimAllowed: S.Boolean,
  pricingMode: ProviderCapacityPricingMode,
  pricingPolicyRefs: S.Array(S.String),
  provider: ProviderCapacityMarketplaceProvider,
  providerGrantRefs: S.Array(S.String),
  providerLabel: S.String,
  publicCopyRefs: S.Array(S.String),
  quotaEvidenceRefs: S.Array(S.String),
  routePolicyRefs: S.Array(S.String),
  secretPolicyRefs: S.Array(S.String),
  sellableCapacityListed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  state: ProviderCapacityMarketplaceState,
  tosBoundaryRefs: S.Array(S.String),
})
export type ProviderCapacityMarketplaceGate =
  typeof ProviderCapacityMarketplaceGate.Type

export type ProviderCapacityMarketplaceGateInput = Readonly<{
  accountSchemaRefs?: ReadonlyArray<string> | undefined
  assignmentModeRefs?: ReadonlyArray<string> | undefined
  assignmentDispatchRefs?: ReadonlyArray<string> | undefined
  assignmentReceiptRefs?: ReadonlyArray<string> | undefined
  connectorHealthRefs?: ReadonlyArray<string> | undefined
  meteringReceiptRefs?: ReadonlyArray<string> | undefined
  pricingMode: ProviderCapacityPricingMode
  pricingPolicyRefs?: ReadonlyArray<string> | undefined
  provider: ProviderCapacityMarketplaceProvider
  providerGrantRefs?: ReadonlyArray<string> | undefined
  quotaEvidenceRefs?: ReadonlyArray<string> | undefined
  routePolicyRefs?: ReadonlyArray<string> | undefined
  secretPolicyRefs?: ReadonlyArray<string> | undefined
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  tosBoundaryRefs?: ReadonlyArray<string> | undefined
}>

export class ProviderCapacityMarketplaceGateUnsafe extends S.TaggedErrorClass<ProviderCapacityMarketplaceGateUnsafe>()(
  'ProviderCapacityMarketplaceGateUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeGate = S.decodeUnknownSync(ProviderCapacityMarketplaceGate)

const providerLabel: Record<ProviderCapacityMarketplaceProvider, string> = {
  chatgpt_codex: 'ChatGPT/Codex',
  claude: 'Claude',
  venice: 'Venice',
}

const supportedProvider = new Set<ProviderCapacityMarketplaceProvider>([
  'chatgpt_codex',
])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const rawMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|quota|wallet)|provider[_-]?(account|credential|grant|payload|quota|secret|token)|quota[_-]?(payload|raw|token|usage)|raw[_-]?(auth|customer|invoice|meter|payment|payload|pricing|provider|quota|receipt|runner|run[_-]?log|telemetry|usage|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|subscription[_-]?(cookie|credential|token)|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const refIsSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !rawMaterialPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref => !refIsSafe(ref))

  if (unsafe !== undefined) {
    throw new ProviderCapacityMarketplaceGateUnsafe({
      reason: `${label} must be public-safe refs without provider-account secrets, tokens, quota payloads, raw metering, payment, wallet, customer, private, or timestamp material.`,
    })
  }

  return normalized
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const requiredBlockers = (
  input: Readonly<{
    accountSchemaRefs: ReadonlyArray<string>
    assignmentModeRefs: ReadonlyArray<string>
    assignmentDispatchRefs: ReadonlyArray<string>
    assignmentReceiptRefs: ReadonlyArray<string>
    connectorHealthRefs: ReadonlyArray<string>
    meteringReceiptRefs: ReadonlyArray<string>
    pricingPolicyRefs: ReadonlyArray<string>
    providerGrantRefs: ReadonlyArray<string>
    quotaEvidenceRefs: ReadonlyArray<string>
    routePolicyRefs: ReadonlyArray<string>
    secretPolicyRefs: ReadonlyArray<string>
    tosBoundaryRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(!hasRefs(input.accountSchemaRefs)
    ? ['blocker.public.provider_capacity.account_schema_missing']
    : []),
  ...(!hasRefs(input.secretPolicyRefs)
    ? ['blocker.public.provider_capacity.secret_ref_policy_missing']
    : []),
  ...(!hasRefs(input.assignmentModeRefs)
    ? ['blocker.public.provider_capacity.assignment_mode_missing']
    : []),
  ...(!hasRefs(input.providerGrantRefs)
    ? ['blocker.public.provider_capacity.provider_grant_missing']
    : []),
  ...(!hasRefs(input.routePolicyRefs)
    ? ['blocker.public.provider_capacity.route_policy_missing']
    : []),
  ...(!hasRefs(input.meteringReceiptRefs)
    ? ['blocker.public.provider_capacity.metering_receipt_missing']
    : []),
  ...(!hasRefs(input.connectorHealthRefs)
    ? ['blocker.public.provider_capacity.connector_health_missing']
    : []),
  ...(!hasRefs(input.quotaEvidenceRefs)
    ? ['blocker.public.provider_capacity.quota_evidence_missing']
    : []),
  ...(!hasRefs(input.assignmentDispatchRefs)
    ? ['blocker.public.provider_capacity.assignment_dispatch_missing']
    : []),
  ...(!hasRefs(input.pricingPolicyRefs)
    ? ['blocker.public.provider_capacity.pricing_policy_missing']
    : []),
  ...(!hasRefs(input.tosBoundaryRefs)
    ? ['blocker.public.provider_capacity.tos_boundary_missing']
    : []),
  ...(!hasRefs(input.assignmentReceiptRefs)
    ? ['blocker.public.provider_capacity.assignment_receipt_missing']
    : []),
]

const baseCaveatRefs = [
  'caveat.public.provider_capacity.provider_connection_is_not_resale_authorization',
  'caveat.public.provider_capacity.private_capacity_material_not_public_refs',
  'caveat.public.provider_capacity.agentic_work_not_base_inference_resale',
  'caveat.public.provider_capacity.settlement_receipts_required_for_bitcoin_copy',
]

export const projectProviderCapacityMarketplaceGate = (
  input: ProviderCapacityMarketplaceGateInput,
): ProviderCapacityMarketplaceGate => {
  const accountSchemaRefs = safeRefs(
    'Capacity provider account schema refs',
    input.accountSchemaRefs,
  )
  const assignmentModeRefs = safeRefs(
    'Capacity assignment mode refs',
    input.assignmentModeRefs,
  )
  const assignmentDispatchRefs = safeRefs(
    'Capacity assignment dispatch refs',
    input.assignmentDispatchRefs,
  )
  const assignmentReceiptRefs = safeRefs(
    'Capacity assignment receipt refs',
    input.assignmentReceiptRefs,
  )
  const connectorHealthRefs = safeRefs(
    'Capacity connector health refs',
    input.connectorHealthRefs,
  )
  const meteringReceiptRefs = safeRefs(
    'Capacity metering receipt refs',
    input.meteringReceiptRefs,
  )
  const pricingPolicyRefs = safeRefs(
    'Capacity pricing policy refs',
    input.pricingPolicyRefs,
  )
  const providerGrantRefs = safeRefs(
    'Capacity provider grant refs',
    input.providerGrantRefs,
  )
  const quotaEvidenceRefs = safeRefs(
    'Capacity quota evidence refs',
    input.quotaEvidenceRefs,
  )
  const routePolicyRefs = safeRefs(
    'Capacity route policy refs',
    input.routePolicyRefs,
  )
  const secretPolicyRefs = safeRefs(
    'Capacity secret ref policy refs',
    input.secretPolicyRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'Capacity settlement receipt refs',
    input.settlementReceiptRefs,
  )
  const tosBoundaryRefs = safeRefs(
    'Capacity ToS boundary refs',
    input.tosBoundaryRefs,
  )
  const unsupported = !supportedProvider.has(input.provider)
  const attemptedUnsupported = [
    accountSchemaRefs,
    assignmentModeRefs,
    assignmentDispatchRefs,
    assignmentReceiptRefs,
    connectorHealthRefs,
    meteringReceiptRefs,
    pricingPolicyRefs,
    providerGrantRefs,
    quotaEvidenceRefs,
    routePolicyRefs,
    secretPolicyRefs,
    settlementReceiptRefs,
    tosBoundaryRefs,
  ].some(hasRefs)
  const baseInferenceResale = input.pricingMode === 'base_inference_resale'
  const missingBlockers = requiredBlockers({
    accountSchemaRefs,
    assignmentModeRefs,
    assignmentDispatchRefs,
    assignmentReceiptRefs,
    connectorHealthRefs,
    meteringReceiptRefs,
    pricingPolicyRefs,
    providerGrantRefs,
    quotaEvidenceRefs,
    routePolicyRefs,
    secretPolicyRefs,
    tosBoundaryRefs,
  })
  const blockerRefs = [
    ...(unsupported
      ? [`blocker.public.provider_capacity.${input.provider}_unsupported`]
      : []),
    ...(baseInferenceResale
      ? [
          'blocker.public.provider_capacity.base_inference_resale_not_authorized',
        ]
      : []),
    ...(unsupported ? [] : missingBlockers),
    ...(!unsupported && !hasRefs(settlementReceiptRefs)
      ? ['blocker.public.provider_capacity.settlement_receipt_missing']
      : []),
  ].sort()
  const assignmentDispatchAllowed =
    !unsupported &&
    !baseInferenceResale &&
    hasRefs(accountSchemaRefs) &&
    hasRefs(secretPolicyRefs) &&
    hasRefs(assignmentModeRefs) &&
    hasRefs(providerGrantRefs) &&
    hasRefs(routePolicyRefs) &&
    hasRefs(meteringReceiptRefs) &&
    hasRefs(connectorHealthRefs) &&
    hasRefs(quotaEvidenceRefs) &&
    hasRefs(pricingPolicyRefs) &&
    hasRefs(tosBoundaryRefs)
  const assignmentReceiptClaimAllowed =
    assignmentDispatchAllowed &&
    hasRefs(assignmentDispatchRefs) &&
    hasRefs(assignmentReceiptRefs)
  const paidSettlementClaimAllowed =
    assignmentReceiptClaimAllowed && hasRefs(settlementReceiptRefs)
  const state: ProviderCapacityMarketplaceState = unsupported
    ? attemptedUnsupported
      ? 'blocked_unsupported'
      : 'planned_unsupported'
    : baseInferenceResale || missingBlockers.length > 0
      ? 'blocked'
      : paidSettlementClaimAllowed
        ? 'settled'
        : assignmentReceiptClaimAllowed
          ? 'assignment_receipted'
          : assignmentDispatchAllowed
            ? 'dispatch_ready'
            : 'grant_ready'
  const configured =
    !unsupported &&
    hasRefs(accountSchemaRefs) &&
    hasRefs(secretPolicyRefs) &&
    hasRefs(providerGrantRefs)
  const healthy =
    configured && hasRefs(connectorHealthRefs) && hasRefs(quotaEvidenceRefs)
  const connectorState: ProviderCapacityConnectorState = unsupported
    ? 'unsupported'
    : paidSettlementClaimAllowed
      ? 'settled'
      : assignmentReceiptClaimAllowed
        ? 'payable'
        : assignmentDispatchAllowed
          ? 'assignable'
          : healthy
            ? 'healthy'
            : 'configured'
  const sellableCapacityListed =
    connectorState === 'assignable' ||
    connectorState === 'payable' ||
    connectorState === 'settled'

  return decodeGate({
    accountSchemaRefs,
    assignmentModeRefs,
    assignmentDispatchAllowed,
    assignmentDispatchRefs,
    assignmentReceiptClaimAllowed,
    assignmentReceiptRefs,
    blockerRefs,
    caveatRefs: baseCaveatRefs,
    connectorHealthRefs,
    connectorState,
    marketableCapacityCopyAllowed: paidSettlementClaimAllowed,
    meteringReceiptRefs,
    paidSettlementClaimAllowed,
    pricingMode: input.pricingMode,
    pricingPolicyRefs,
    provider: input.provider,
    providerGrantRefs,
    providerLabel: providerLabel[input.provider],
    publicCopyRefs: paidSettlementClaimAllowed
      ? ['copy.public.provider_capacity.bitcoin_settlement_receipts_visible']
      : unsupported
        ? ['copy.public.provider_capacity.provider_planned_or_blocked']
        : ['copy.public.provider_capacity.marketplace_monetization_blocked'],
    quotaEvidenceRefs,
    routePolicyRefs,
    secretPolicyRefs,
    sellableCapacityListed,
    settlementReceiptRefs,
    state,
    tosBoundaryRefs,
  })
}

export const providerCapacityMarketplaceGateHasPrivateMaterial = (
  gate: ProviderCapacityMarketplaceGate,
): boolean => {
  const publicValues = [
    gate.pricingMode,
    gate.provider,
    gate.providerLabel,
    gate.state,
    gate.connectorState,
    ...gate.accountSchemaRefs,
    ...gate.assignmentModeRefs,
    ...gate.assignmentDispatchRefs,
    ...gate.assignmentReceiptRefs,
    ...gate.blockerRefs,
    ...gate.caveatRefs,
    ...gate.connectorHealthRefs,
    ...gate.meteringReceiptRefs,
    ...gate.pricingPolicyRefs,
    ...gate.providerGrantRefs,
    ...gate.publicCopyRefs,
    ...gate.quotaEvidenceRefs,
    ...gate.routePolicyRefs,
    ...gate.secretPolicyRefs,
    ...gate.settlementReceiptRefs,
    ...gate.tosBoundaryRefs,
  ]

  return publicValues.some(
    value =>
      containsProviderSecretMaterial(value) ||
      rawMaterialPattern.test(value) ||
      rawTimestampPattern.test(value),
  )
}
