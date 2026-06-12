import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { OmniProjectionAudience } from './omni-data-classification'

export const OpenAgentsSiteMdkSmokeImplementationState = S.Literals([
  'fake_provider',
  'live_provider',
  'sandbox_signet',
])
export type OpenAgentsSiteMdkSmokeImplementationState =
  typeof OpenAgentsSiteMdkSmokeImplementationState.Type

export const OpenAgentsSiteMdkSmokeCheckName = S.Literals([
  'checkout_intent',
  'clean_return_status',
  'discovery',
  'l402_challenge',
  'l402_redemption',
  'payment_proof',
  'provider_reconciliation',
  'provider_replay',
  'redaction',
  'spend_cap_rejection',
  'stale_rejection',
])
export type OpenAgentsSiteMdkSmokeCheckName =
  typeof OpenAgentsSiteMdkSmokeCheckName.Type

export const OpenAgentsSiteMdkSmokeCheckStatus = S.Literals([
  'failed',
  'passed',
  'skipped',
])
export type OpenAgentsSiteMdkSmokeCheckStatus =
  typeof OpenAgentsSiteMdkSmokeCheckStatus.Type

export class OpenAgentsSiteMdkSmokeAuthority extends S.Class<OpenAgentsSiteMdkSmokeAuthority>(
  'OpenAgentsSiteMdkSmokeAuthority',
)({
  noAcceptedWorkPayoutAuthority: S.Boolean,
  noDeploymentAuthority: S.Boolean,
  noLiveMainnetRequired: S.Boolean,
  noProviderPayoutAuthority: S.Boolean,
  noSettlementAuthority: S.Boolean,
  noWalletSpendAuthority: S.Boolean,
}) {}

export class OpenAgentsSiteMdkSmokeCheckRecord extends S.Class<OpenAgentsSiteMdkSmokeCheckRecord>(
  'OpenAgentsSiteMdkSmokeCheckRecord',
)({
  blockerRefs: S.Array(S.String),
  checkName: OpenAgentsSiteMdkSmokeCheckName,
  evidenceRefs: S.Array(S.String),
  status: OpenAgentsSiteMdkSmokeCheckStatus,
}) {}

export class OpenAgentsSiteMdkSmokeRecord extends S.Class<OpenAgentsSiteMdkSmokeRecord>(
  'OpenAgentsSiteMdkSmokeRecord',
)({
  authority: OpenAgentsSiteMdkSmokeAuthority,
  caveatRefs: S.Array(S.String),
  checkRecords: S.Array(OpenAgentsSiteMdkSmokeCheckRecord),
  checkoutIntentRefs: S.Array(S.String),
  implementationState: OpenAgentsSiteMdkSmokeImplementationState,
  l402ChallengeRefs: S.Array(S.String),
  l402RedemptionRefs: S.Array(S.String),
  paymentProofRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  smokeRef: S.String,
  siteId: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OpenAgentsSiteMdkSmokeProjection extends S.Class<OpenAgentsSiteMdkSmokeProjection>(
  'OpenAgentsSiteMdkSmokeProjection',
)({
  acceptedWorkPayoutClaimAllowed: S.Boolean,
  audience: OmniProjectionAudience,
  authority: OpenAgentsSiteMdkSmokeAuthority,
  caveatRefs: S.Array(S.String),
  checkRecords: S.Array(OpenAgentsSiteMdkSmokeCheckRecord),
  checkoutIntentRefs: S.Array(S.String),
  implementationState: OpenAgentsSiteMdkSmokeImplementationState,
  implementationStateLabel: S.String,
  l402ChallengeRefs: S.Array(S.String),
  l402RedemptionRefs: S.Array(S.String),
  notProductionPaymentEvidence: S.Boolean,
  passedCheckCount: S.Number,
  paymentProofRefs: S.Array(S.String),
  providerPayoutClaimAllowed: S.Boolean,
  receiptRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  smokeRef: S.String,
  smokeState: OpenAgentsSiteMdkSmokeCheckStatus,
  siteId: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class OpenAgentsSiteMdkSmokeUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMdkSmokeUnsafe>()(
  'OpenAgentsSiteMdkSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

export const OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY:
  OpenAgentsSiteMdkSmokeAuthority = {
    noAcceptedWorkPayoutAuthority: true,
    noDeploymentAuthority: true,
    noLiveMainnetRequired: true,
    noProviderPayoutAuthority: true,
    noSettlementAuthority: true,
    noWalletSpendAuthority: true,
  }

const requiredCheckNames: ReadonlySet<OpenAgentsSiteMdkSmokeCheckName> =
  new Set([
    'checkout_intent',
    'clean_return_status',
    'discovery',
    'l402_challenge',
    'l402_redemption',
    'payment_proof',
    'provider_reconciliation',
    'provider_replay',
    'redaction',
    'spend_cap_rejection',
    'stale_rejection',
  ])

const implementationStateLabels:
  Readonly<Record<OpenAgentsSiteMdkSmokeImplementationState, string>> = {
    fake_provider: 'Fake provider CI smoke',
    live_provider: 'Live provider smoke',
    sandbox_signet: 'Sandbox or signet smoke',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/u
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/u
const unsafeValuePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer|bolt11|bolt12|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|wallet)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(checkout|customer|email|invoice|log|payment|payload|provider|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend))/i
const publicUnsafeRefPattern =
  /(evidence\.private|operator\.private|private\.|source\.private)/i

const uniqueRefs = (
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

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref) ||
    unsafeValuePattern.test(ref) ||
    containsProviderSecretMaterial(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason: `${label} contains MDK credentials, wallet material, raw invoices, payment hashes, preimages, private customer data, provider grants, private source refs, or raw timestamps.`,
    })
  }
}

const audienceSafeRefs = (
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  const safe = uniqueRefs(refs)

  if (audience === 'operator' || audience === 'private') {
    return safe
  }

  return safe.filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertRecordOnlyAuthority = (
  authority: OpenAgentsSiteMdkSmokeAuthority,
): void => {
  if (
    authority.noAcceptedWorkPayoutAuthority !== true ||
    authority.noDeploymentAuthority !== true ||
    authority.noLiveMainnetRequired !== true ||
    authority.noProviderPayoutAuthority !== true ||
    authority.noSettlementAuthority !== true ||
    authority.noWalletSpendAuthority !== true
  ) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason:
        'Site MDK smoke evidence cannot create accepted-work payout, deployment, live mainnet, provider payout, settlement, or wallet spend authority.',
    })
  }
}

const assertRecordSafe = (record: OpenAgentsSiteMdkSmokeRecord): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason: 'Site MDK smoke updatedAtIso must be valid.',
    })
  }

  assertRecordOnlyAuthority(record.authority)
  assertSafeRefs('Site MDK smoke identity refs', [
    record.smokeRef,
    record.siteId,
  ])
  assertSafeRefs('Site MDK smoke caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Site MDK smoke checkout intent refs',
    record.checkoutIntentRefs,
  )
  assertSafeRefs(
    'Site MDK smoke L402 challenge refs',
    record.l402ChallengeRefs,
  )
  assertSafeRefs(
    'Site MDK smoke L402 redemption refs',
    record.l402RedemptionRefs,
  )
  assertSafeRefs('Site MDK smoke payment proof refs', record.paymentProofRefs)
  assertSafeRefs('Site MDK smoke receipt refs', record.receiptRefs)
  assertSafeRefs('Site MDK smoke source refs', record.sourceRefs)
  record.checkRecords.forEach(check => {
    assertSafeRefs(`Site MDK smoke ${check.checkName} evidence refs`, [
      ...check.blockerRefs,
      ...check.evidenceRefs,
    ])
  })

  const checkNames = new Set(record.checkRecords.map(check => check.checkName))
  const missing = [...requiredCheckNames].find(name => !checkNames.has(name))

  if (missing !== undefined) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason: `Site MDK smoke is missing required check ${missing}.`,
    })
  }

  const recordForPrivateScan = {
    ...record,
    updatedAtIso: '',
  }

  if (
    stringValues(recordForPrivateScan).some(value =>
      containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      rawTimestampPattern.test(value)
    )
  ) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason:
        'Site MDK smoke records cannot expose credentials, wallet material, raw payment material, customer private data, provider secrets, private source refs, or raw timestamps outside timestamp fields.',
    })
  }
}

const smokeStateForChecks = (
  checks: ReadonlyArray<OpenAgentsSiteMdkSmokeCheckRecord>,
): OpenAgentsSiteMdkSmokeCheckStatus =>
  checks.some(check => check.status === 'failed')
    ? 'failed'
    : checks.some(check => check.status === 'skipped')
      ? 'skipped'
      : 'passed'

export const projectOpenAgentsSiteMdkSmoke = (
  record: OpenAgentsSiteMdkSmokeRecord,
  audience: typeof OmniProjectionAudience.Type,
): OpenAgentsSiteMdkSmokeProjection => {
  assertRecordSafe(record)

  const projection = new OpenAgentsSiteMdkSmokeProjection({
    acceptedWorkPayoutClaimAllowed: false,
    audience,
    authority: record.authority,
    caveatRefs: audienceSafeRefs(record.caveatRefs, audience),
    checkRecords: record.checkRecords.map(check =>
      new OpenAgentsSiteMdkSmokeCheckRecord({
        blockerRefs: audienceSafeRefs(check.blockerRefs, audience),
        checkName: check.checkName,
        evidenceRefs: audienceSafeRefs(check.evidenceRefs, audience),
        status: check.status,
      }),
    ),
    checkoutIntentRefs: audienceSafeRefs(record.checkoutIntentRefs, audience),
    implementationState: record.implementationState,
    implementationStateLabel:
      implementationStateLabels[record.implementationState],
    l402ChallengeRefs: audienceSafeRefs(record.l402ChallengeRefs, audience),
    l402RedemptionRefs: audienceSafeRefs(record.l402RedemptionRefs, audience),
    notProductionPaymentEvidence: record.implementationState !== 'live_provider',
    passedCheckCount: record.checkRecords.filter(
      check => check.status === 'passed',
    ).length,
    paymentProofRefs: audienceSafeRefs(record.paymentProofRefs, audience),
    providerPayoutClaimAllowed: false,
    receiptRefs: audienceSafeRefs(record.receiptRefs, audience),
    settlementClaimAllowed: false,
    smokeRef: record.smokeRef,
    smokeState: smokeStateForChecks(record.checkRecords),
    siteId: record.siteId,
    sourceRefs: audienceSafeRefs(record.sourceRefs, audience),
    updatedAtDisplay: 'retained smoke evidence',
    walletSpendAllowed: false,
  })

  if (
    stringValues(projection).some(value =>
      containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      rawTimestampPattern.test(value)
    )
  ) {
    throw new OpenAgentsSiteMdkSmokeUnsafe({
      reason:
        'Site MDK smoke projection contains private payment, wallet, customer, provider, source, or raw timestamp material.',
    })
  }

  return projection
}

export const exampleOpenAgentsSiteMdkSmokeRecord =
  (): OpenAgentsSiteMdkSmokeRecord =>
    new OpenAgentsSiteMdkSmokeRecord({
      authority: OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY,
      caveatRefs: [
        'caveat.public.site_mdk_smoke.fake_provider_not_production_evidence',
      ],
      checkRecords: [...requiredCheckNames].map(checkName =>
        new OpenAgentsSiteMdkSmokeCheckRecord({
          blockerRefs: [],
          checkName,
          evidenceRefs: [`evidence.public.site_mdk_smoke.${checkName}`],
          status: 'passed',
        }),
      ),
      checkoutIntentRefs: ['site_checkout_intent_site_otec_smoke_checkout'],
      implementationState: 'fake_provider',
      l402ChallengeRefs: ['site_l402_challenge_site_otec_smoke_l402'],
      l402RedemptionRefs: ['site_l402_redemption_site_otec_smoke_l402'],
      paymentProofRefs: ['payment_proof.public.site_mdk_smoke'],
      receiptRefs: ['receipt.public.site_mdk_smoke'],
      siteId: 'site_otec',
      smokeRef: 'smoke.public.site_mdk.fake_provider.ci',
      sourceRefs: ['test:workers/api/src/site-commerce-routes.test.ts'],
      updatedAtIso: '2026-06-07T12:00:00.000Z',
    })
