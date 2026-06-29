import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Array as Arr, Schema as S } from 'effect'

export const SignatureMarketplaceRevenueState = S.Literals([
  'blocked',
  'validated',
  'metered',
  'attributed',
  'priced',
  'eligible',
  'payable',
  'settled',
])
export type SignatureMarketplaceRevenueState =
  typeof SignatureMarketplaceRevenueState.Type

export const SignatureMarketplaceRevenueGate = S.Struct({
  activationRefs: S.Array(S.String),
  attributionRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRuntimeActivationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  contributorPayableCents: S.Number,
  disputePolicyRefs: S.Array(S.String),
  exactUsageSubjectRefs: S.Array(S.String),
  forkPolicyRefs: S.Array(S.String),
  grossRevenueCents: S.Number,
  installAllowed: S.Boolean,
  licensePolicyRefs: S.Array(S.String),
  marketplaceListingMutationAllowed: S.Boolean,
  meteredUsageEventCount: S.Number,
  packagePublicationRefs: S.Array(S.String),
  packageRefs: S.Array(S.String),
  packageValidationRefs: S.Array(S.String),
  payoutClaimAllowed: S.Boolean,
  payoutEligibilityClaimAllowed: S.Boolean,
  payoutEligibilityRefs: S.Array(S.String),
  pricingPolicyRefs: S.Array(S.String),
  programSignatureRefs: S.Array(S.String),
  publicCopyRefs: S.Array(S.String),
  refundPolicyRefs: S.Array(S.String),
  revenueProjectionAllowed: S.Boolean,
  revenueProjectionRefs: S.Array(S.String),
  revSharePolicyRefs: S.Array(S.String),
  settledContributorCents: S.Number,
  settlementClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  signatureRevenueCopyAllowed: S.Boolean,
  state: SignatureMarketplaceRevenueState,
  usageEventRefs: S.Array(S.String),
  usageIdempotencyRefs: S.Array(S.String),
})
export type SignatureMarketplaceRevenueGate =
  typeof SignatureMarketplaceRevenueGate.Type

export type SignatureMarketplaceRevenueGateInput = Readonly<{
  activationRefs?: ReadonlyArray<string> | undefined
  attributionRefs?: ReadonlyArray<string> | undefined
  contributorPayableCents?: number | undefined
  disputePolicyRefs?: ReadonlyArray<string> | undefined
  exactUsageSubjectRefs?: ReadonlyArray<string> | undefined
  forkPolicyRefs?: ReadonlyArray<string> | undefined
  grossRevenueCents?: number | undefined
  licensePolicyRefs?: ReadonlyArray<string> | undefined
  packagePublicationRefs?: ReadonlyArray<string> | undefined
  packageRefs?: ReadonlyArray<string> | undefined
  packageValidationRefs?: ReadonlyArray<string> | undefined
  payoutEligibilityRefs?: ReadonlyArray<string> | undefined
  pricingPolicyRefs?: ReadonlyArray<string> | undefined
  programSignatureRefs?: ReadonlyArray<string> | undefined
  refundPolicyRefs?: ReadonlyArray<string> | undefined
  revenueProjectionRefs?: ReadonlyArray<string> | undefined
  revSharePolicyRefs?: ReadonlyArray<string> | undefined
  settledContributorCents?: number | undefined
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  usageEventRefs?: ReadonlyArray<string> | undefined
  usageIdempotencyRefs?: ReadonlyArray<string> | undefined
}>

export class SignatureMarketplaceRevenueGateUnsafe extends S.TaggedErrorClass<SignatureMarketplaceRevenueGateUnsafe>()(
  'SignatureMarketplaceRevenueGateUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeGate = S.decodeUnknownSync(SignatureMarketplaceRevenueGate)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeSignatureMarketplaceRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|fork[_-]?private|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|license[_-]?private|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|package[_-]?source[_-]?private|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|package|repo|source|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|customer|document|email|fixture|invoice|log|package|payment|payload|prompt|provider|receipt|runner|run[_-]?log|schema|source|source[_-]?archive|usage|webhook)|runner[_-]?log|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|usage[_-]?(event[_-]?raw|payload|raw)|wallet)/i
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
  !unsafeSignatureMarketplaceRefPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref => !refIsSafe(ref))

  if (unsafe !== undefined) {
    throw new SignatureMarketplaceRevenueGateUnsafe({
      reason: `${label} must be public-safe refs without raw package, usage, provider, customer, payment, wallet, payout, private repo, secret, or timestamp material.`,
    })
  }

  return normalized
}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  Arr.isReadonlyArrayNonEmpty(refs)

const safeNonNegativeInteger = (
  label: string,
  value: number | undefined,
): number => {
  const normalized = value ?? 0

  if (!Number.isInteger(normalized) || normalized < 0) {
    throw new SignatureMarketplaceRevenueGateUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }

  return normalized
}

const missingEvidenceBlockers = (
  input: Readonly<{
    attributionRefs: ReadonlyArray<string>
    activationRefs: ReadonlyArray<string>
    contributorPayableCents: number
    disputePolicyRefs: ReadonlyArray<string>
    exactUsageSubjectRefs: ReadonlyArray<string>
    forkPolicyRefs: ReadonlyArray<string>
    grossRevenueCents: number
    licensePolicyRefs: ReadonlyArray<string>
    packagePublicationRefs: ReadonlyArray<string>
    packageRefs: ReadonlyArray<string>
    packageValidationRefs: ReadonlyArray<string>
    payoutEligibilityRefs: ReadonlyArray<string>
    pricingPolicyRefs: ReadonlyArray<string>
    programSignatureRefs: ReadonlyArray<string>
    refundPolicyRefs: ReadonlyArray<string>
    revenueProjectionRefs: ReadonlyArray<string>
    revSharePolicyRefs: ReadonlyArray<string>
    settlementReceiptRefs: ReadonlyArray<string>
    usageEventRefs: ReadonlyArray<string>
    usageIdempotencyRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(!hasRefs(input.packageValidationRefs)
    ? ['blocker.public.signature_market.package_validation_missing']
    : []),
  ...(!hasRefs(input.packagePublicationRefs)
    ? ['blocker.public.signature_market.package_publication_missing']
    : []),
  ...(!hasRefs(input.activationRefs)
    ? ['blocker.public.signature_market.package_activation_missing']
    : []),
  ...(!hasRefs(input.packageRefs)
    ? ['blocker.public.signature_market.package_ref_missing']
    : []),
  ...(!hasRefs(input.programSignatureRefs)
    ? ['blocker.public.signature_market.program_signature_missing']
    : []),
  ...(!hasRefs(input.usageEventRefs)
    ? ['blocker.public.signature_market.usage_event_missing']
    : []),
  ...(!hasRefs(input.usageIdempotencyRefs)
    ? ['blocker.public.signature_market.usage_idempotency_missing']
    : []),
  ...(!hasRefs(input.exactUsageSubjectRefs)
    ? ['blocker.public.signature_market.exact_usage_subject_missing']
    : []),
  ...(!hasRefs(input.attributionRefs)
    ? ['blocker.public.signature_market.attribution_missing']
    : []),
  ...(!hasRefs(input.pricingPolicyRefs)
    ? ['blocker.public.signature_market.pricing_policy_missing']
    : []),
  ...(!hasRefs(input.revenueProjectionRefs)
    ? ['blocker.public.signature_market.revenue_projection_missing']
    : []),
  ...(input.grossRevenueCents === 0
    ? ['blocker.public.signature_market.gross_revenue_missing']
    : []),
  ...(!hasRefs(input.revSharePolicyRefs)
    ? ['blocker.public.signature_market.rev_share_policy_missing']
    : []),
  ...(!hasRefs(input.forkPolicyRefs)
    ? ['blocker.public.signature_market.fork_policy_missing']
    : []),
  ...(!hasRefs(input.licensePolicyRefs)
    ? ['blocker.public.signature_market.license_policy_missing']
    : []),
  ...(!hasRefs(input.disputePolicyRefs)
    ? ['blocker.public.signature_market.dispute_policy_missing']
    : []),
  ...(!hasRefs(input.refundPolicyRefs)
    ? ['blocker.public.signature_market.refund_policy_missing']
    : []),
  ...(!hasRefs(input.payoutEligibilityRefs)
    ? ['blocker.public.signature_market.payout_eligibility_missing']
    : []),
  ...(input.contributorPayableCents === 0
    ? ['blocker.public.signature_market.contributor_share_missing']
    : []),
  ...(!hasRefs(input.settlementReceiptRefs)
    ? ['blocker.public.signature_market.settlement_receipt_missing']
    : []),
]

const baseCaveatRefs = [
  'caveat.public.signature_market.validation_does_not_install',
  'caveat.public.signature_market.candidate_acceptance_is_not_runtime_activation',
  'caveat.public.signature_market.usage_meters_bind_to_exact_refs',
  'caveat.public.signature_market.revenue_projection_is_not_payout',
  'caveat.public.signature_market.purchase_or_usage_is_not_settlement',
]

export const projectSignatureMarketplaceRevenueGate = (
  input: SignatureMarketplaceRevenueGateInput,
): SignatureMarketplaceRevenueGate => {
  const activationRefs = safeRefs(
    'Signature marketplace activation refs',
    input.activationRefs,
  )
  const attributionRefs = safeRefs(
    'Signature marketplace attribution refs',
    input.attributionRefs,
  )
  const disputePolicyRefs = safeRefs(
    'Signature marketplace dispute policy refs',
    input.disputePolicyRefs,
  )
  const exactUsageSubjectRefs = safeRefs(
    'Signature marketplace exact usage subject refs',
    input.exactUsageSubjectRefs,
  )
  const forkPolicyRefs = safeRefs(
    'Signature marketplace fork policy refs',
    input.forkPolicyRefs,
  )
  const licensePolicyRefs = safeRefs(
    'Signature marketplace license policy refs',
    input.licensePolicyRefs,
  )
  const packageRefs = safeRefs(
    'Signature marketplace package refs',
    input.packageRefs,
  )
  const packagePublicationRefs = safeRefs(
    'Signature marketplace package publication refs',
    input.packagePublicationRefs,
  )
  const packageValidationRefs = safeRefs(
    'Signature marketplace package validation refs',
    input.packageValidationRefs,
  )
  const payoutEligibilityRefs = safeRefs(
    'Signature marketplace payout eligibility refs',
    input.payoutEligibilityRefs,
  )
  const pricingPolicyRefs = safeRefs(
    'Signature marketplace pricing policy refs',
    input.pricingPolicyRefs,
  )
  const programSignatureRefs = safeRefs(
    'Signature marketplace program signature refs',
    input.programSignatureRefs,
  )
  const refundPolicyRefs = safeRefs(
    'Signature marketplace refund policy refs',
    input.refundPolicyRefs,
  )
  const revenueProjectionRefs = safeRefs(
    'Signature marketplace revenue projection refs',
    input.revenueProjectionRefs,
  )
  const revSharePolicyRefs = safeRefs(
    'Signature marketplace revenue share policy refs',
    input.revSharePolicyRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'Signature marketplace settlement receipt refs',
    input.settlementReceiptRefs,
  )
  const usageEventRefs = safeRefs(
    'Signature marketplace usage event refs',
    input.usageEventRefs,
  )
  const usageIdempotencyRefs = safeRefs(
    'Signature marketplace usage idempotency refs',
    input.usageIdempotencyRefs,
  )
  const grossRevenueCents = safeNonNegativeInteger(
    'grossRevenueCents',
    input.grossRevenueCents,
  )
  const contributorPayableCents = safeNonNegativeInteger(
    'contributorPayableCents',
    input.contributorPayableCents,
  )
  const settledContributorCents = safeNonNegativeInteger(
    'settledContributorCents',
    input.settledContributorCents,
  )

  if (contributorPayableCents > grossRevenueCents) {
    throw new SignatureMarketplaceRevenueGateUnsafe({
      reason: 'Contributor payable cents cannot exceed gross revenue cents.',
    })
  }

  if (settledContributorCents > contributorPayableCents) {
    throw new SignatureMarketplaceRevenueGateUnsafe({
      reason:
        'Settled contributor cents cannot exceed contributor payable cents.',
    })
  }

  const validated =
    hasRefs(packageValidationRefs) &&
    hasRefs(packageRefs) &&
    hasRefs(programSignatureRefs)
  const activated =
    validated &&
    hasRefs(packagePublicationRefs) &&
    hasRefs(activationRefs)
  const metered =
    activated &&
    hasRefs(usageEventRefs) &&
    hasRefs(usageIdempotencyRefs) &&
    hasRefs(exactUsageSubjectRefs)
  const attributed = metered && hasRefs(attributionRefs)
  const priced =
    attributed &&
    hasRefs(pricingPolicyRefs) &&
    hasRefs(revenueProjectionRefs) &&
    grossRevenueCents > 0
  const eligible =
    priced &&
    hasRefs(revSharePolicyRefs) &&
    hasRefs(forkPolicyRefs) &&
    hasRefs(licensePolicyRefs) &&
    hasRefs(disputePolicyRefs) &&
    hasRefs(refundPolicyRefs)
  const payable =
    eligible && hasRefs(payoutEligibilityRefs) && contributorPayableCents > 0
  const settled =
    payable &&
    hasRefs(settlementReceiptRefs) &&
    settledContributorCents === contributorPayableCents
  const state: SignatureMarketplaceRevenueState = settled
    ? 'settled'
    : payable
      ? 'payable'
      : eligible
        ? 'eligible'
        : priced
          ? 'priced'
          : attributed
            ? 'attributed'
            : metered
              ? 'metered'
              : validated
                ? 'validated'
                : 'blocked'

  return decodeGate({
    attributionRefs,
    blockerRefs: [
      ...missingEvidenceBlockers({
        attributionRefs,
        activationRefs,
        contributorPayableCents,
        disputePolicyRefs,
        exactUsageSubjectRefs,
        forkPolicyRefs,
        grossRevenueCents,
        licensePolicyRefs,
        packagePublicationRefs,
        packageRefs,
        packageValidationRefs,
        payoutEligibilityRefs,
        pricingPolicyRefs,
        programSignatureRefs,
        refundPolicyRefs,
        revenueProjectionRefs,
        revSharePolicyRefs,
        settlementReceiptRefs,
        usageEventRefs,
        usageIdempotencyRefs,
      }),
    ].sort(),
    activationRefs,
    candidateRuntimeActivationAllowed: activated,
    caveatRefs: baseCaveatRefs,
    contributorPayableCents,
    disputePolicyRefs,
    exactUsageSubjectRefs,
    forkPolicyRefs,
    grossRevenueCents,
    installAllowed: activated,
    licensePolicyRefs,
    marketplaceListingMutationAllowed: activated,
    meteredUsageEventCount: usageEventRefs.length,
    packagePublicationRefs,
    packageRefs,
    packageValidationRefs,
    payoutClaimAllowed: settled,
    payoutEligibilityClaimAllowed: payable,
    payoutEligibilityRefs,
    pricingPolicyRefs,
    programSignatureRefs,
    publicCopyRefs: settled
      ? ['copy.public.signature_market.settlement_receipts_visible']
      : priced
        ? ['copy.public.signature_market.revenue_projection_pending_settlement']
        : ['copy.public.signature_market.revenue_claim_blocked'],
    refundPolicyRefs,
    revenueProjectionAllowed: priced,
    revenueProjectionRefs,
    revSharePolicyRefs,
    settledContributorCents,
    settlementClaimAllowed: settled,
    settlementReceiptRefs,
    signatureRevenueCopyAllowed: settled,
    state,
    usageEventRefs,
    usageIdempotencyRefs,
  })
}

export const signatureMarketplaceRevenueGateHasPrivateMaterial = (
  gate: SignatureMarketplaceRevenueGate,
): boolean => {
  const publicValues = [
    gate.state,
    ...gate.activationRefs,
    ...gate.attributionRefs,
    ...gate.blockerRefs,
    ...gate.caveatRefs,
    ...gate.disputePolicyRefs,
    ...gate.exactUsageSubjectRefs,
    ...gate.forkPolicyRefs,
    ...gate.licensePolicyRefs,
    ...gate.packagePublicationRefs,
    ...gate.packageRefs,
    ...gate.packageValidationRefs,
    ...gate.payoutEligibilityRefs,
    ...gate.pricingPolicyRefs,
    ...gate.programSignatureRefs,
    ...gate.publicCopyRefs,
    ...gate.refundPolicyRefs,
    ...gate.revenueProjectionRefs,
    ...gate.revSharePolicyRefs,
    ...gate.settlementReceiptRefs,
    ...gate.usageEventRefs,
    ...gate.usageIdempotencyRefs,
  ]

  return publicValues.some(
    value =>
      containsProviderSecretMaterial(value) ||
      unsafeSignatureMarketplaceRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )
}
