import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisForumRewardSmokeMode = S.Literals([
  'live_bitcoin',
  'simulation',
])
export type ArtanisForumRewardSmokeMode =
  typeof ArtanisForumRewardSmokeMode.Type

export class ArtanisForumRewardSmokeAuthority extends S.Class<ArtanisForumRewardSmokeAuthority>(
  'ArtanisForumRewardSmokeAuthority',
)({
  noAcceptedWorkPayoutMutation: S.Boolean,
  noForumReceiptMutation: S.Boolean,
  noProviderSettlementMutation: S.Boolean,
  noWalletSpendExecution: S.Boolean,
}) {}

export class ArtanisForumRewardSmokeExchangeRecord extends S.Class<ArtanisForumRewardSmokeExchangeRecord>(
  'ArtanisForumRewardSmokeExchangeRecord',
)({
  amountAsset: S.Literal('sats'),
  amountValue: S.Number,
  earningNotificationRef: S.String,
  fromAgentRef: S.String,
  postRef: S.String,
  previewChallengeRef: S.String,
  receiptProjectionRef: S.String,
  receiptRef: S.String,
  toAgentRef: S.String,
}) {}

export class ArtanisForumRewardSmokeRecord extends S.Class<ArtanisForumRewardSmokeRecord>(
  'ArtanisForumRewardSmokeRecord',
)({
  acceptedContributionBoundaryRefs: S.Array(S.String),
  acceptedWorkPayoutRefs: S.Array(S.String),
  agentRef: S.String,
  authority: ArtanisForumRewardSmokeAuthority,
  caveatRefs: S.Array(S.String),
  exchangeRecords: S.Array(ArtanisForumRewardSmokeExchangeRecord),
  mode: ArtanisForumRewardSmokeMode,
  namedWalletRefs: S.Array(S.String),
  providerSettlementRefs: S.Array(S.String),
  receiptProjectionRefs: S.Array(S.String),
  registeredAgentRefs: S.Array(S.String),
  runReasonRefs: S.Array(S.String),
  smokeRef: S.String,
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  updatedAtIso: S.String,
  usedLiveBitcoin: S.Boolean,
  walletAuthorityRefs: S.Array(S.String),
}) {}

export class ArtanisForumRewardSmokeProjection extends S.Class<ArtanisForumRewardSmokeProjection>(
  'ArtanisForumRewardSmokeProjection',
)({
  acceptedContributionBoundaryRefs: S.Array(S.String),
  acceptedWorkPayoutClaimAllowed: S.Boolean,
  acceptedWorkPayoutRefs: S.Array(S.String),
  agentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisForumRewardSmokeAuthority,
  caveatRefs: S.Array(S.String),
  exchangeCount: S.Number,
  exchangeRecords: S.Array(ArtanisForumRewardSmokeExchangeRecord),
  mode: ArtanisForumRewardSmokeMode,
  modeLabel: S.String,
  namedWalletRefs: S.Array(S.String),
  providerSettlementClaimAllowed: S.Boolean,
  providerSettlementRefs: S.Array(S.String),
  receiptProjectionRefs: S.Array(S.String),
  registeredAgentRefs: S.Array(S.String),
  runReasonRefs: S.Array(S.String),
  smokeRef: S.String,
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  usedLiveBitcoin: S.Boolean,
  walletAuthorityRefs: S.Array(S.String),
}) {}

export class ArtanisForumRewardSmokeUnsafe extends S.TaggedErrorClass<ArtanisForumRewardSmokeUnsafe>()(
  'ArtanisForumRewardSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_FORUM_REWARD_SMOKE_RECORD_ONLY_AUTHORITY:
  ArtanisForumRewardSmokeAuthority = {
    noAcceptedWorkPayoutMutation: true,
    noForumReceiptMutation: true,
    noProviderSettlementMutation: true,
    noWalletSpendExecution: true,
  }

const modeLabelByMode: Readonly<Record<ArtanisForumRewardSmokeMode, string>> = {
  live_bitcoin: 'Live bitcoin recorded',
  simulation: 'Simulation only',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRewardSmokePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(channel|customer|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|customer|email|invoice|log|payment|payload|payout|prompt|provider|runner|run[_-]?log|source[_-]?archive|state|telemetry|text|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(authority\.private|evidence\.private|receipt\.private|source\.private|wallet\.private)/i

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

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeRewardSmokePattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: `${label} contains wallet material, raw bitcoin payment material, invoices, preimages, customer/private data, private channel state, provider secrets, raw logs, private repo refs, credentials, or raw timestamps.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator' || audience === 'private') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertRecordOnlyAuthority = (
  authority: ArtanisForumRewardSmokeAuthority,
): void => {
  if (
    !authority.noAcceptedWorkPayoutMutation ||
    !authority.noForumReceiptMutation ||
    !authority.noProviderSettlementMutation ||
    !authority.noWalletSpendExecution
  ) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason:
        'Artanis Forum reward smoke records evidence only and cannot mutate Forum receipts, spend wallets, mutate accepted-work payouts, or mutate provider settlement.',
    })
  }
}

const assertExchangeSafe = (
  exchange: ArtanisForumRewardSmokeExchangeRecord,
): void => {
  if (exchange.amountValue <= 0) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Forum reward smoke exchange amount must be positive.',
    })
  }

  assertSafeRefs('Forum reward smoke exchange refs', [
    exchange.earningNotificationRef,
    exchange.fromAgentRef,
    exchange.postRef,
    exchange.previewChallengeRef,
    exchange.receiptProjectionRef,
    exchange.receiptRef,
    exchange.toAgentRef,
  ])
}

const assertRecordSafe = (
  record: ArtanisForumRewardSmokeRecord,
): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Artanis Forum reward smoke updatedAtIso must be valid.',
    })
  }

  assertRecordOnlyAuthority(record.authority)
  assertSafeRefs('Artanis Forum reward smoke identity refs', [
    record.agentRef,
    record.smokeRef,
  ])
  assertSafeRefs(
    'Artanis Forum reward smoke accepted-contribution boundary refs',
    record.acceptedContributionBoundaryRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke accepted-work payout refs',
    record.acceptedWorkPayoutRefs,
  )
  assertSafeRefs('Artanis Forum reward smoke caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Artanis Forum reward smoke named wallet refs',
    record.namedWalletRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke provider settlement refs',
    record.providerSettlementRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke receipt projection refs',
    record.receiptProjectionRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke registered agent refs',
    record.registeredAgentRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke run reason refs',
    record.runReasonRefs,
  )
  assertSafeRefs('Artanis Forum reward smoke source refs', record.sourceRefs)
  assertSafeRefs(
    'Artanis Forum reward smoke spend cap refs',
    record.spendCapRefs,
  )
  assertSafeRefs(
    'Artanis Forum reward smoke wallet authority refs',
    record.walletAuthorityRefs,
  )
  record.exchangeRecords.forEach(assertExchangeSafe)

  if (record.agentRef !== 'agent_artanis') {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Artanis Forum reward smoke must be reported by agent_artanis.',
    })
  }

  if (record.registeredAgentRefs.length < 2) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Forum reward smoke requires at least two registered agent refs.',
    })
  }

  if (record.exchangeRecords.length < 2) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Forum reward smoke requires at least two reward exchanges.',
    })
  }

  if (record.mode === 'simulation' && record.usedLiveBitcoin) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason: 'Simulation Forum reward smoke cannot claim live bitcoin use.',
    })
  }

  if (
    record.mode === 'simulation' &&
    (
      hasAny(record.walletAuthorityRefs) ||
      hasAny(record.namedWalletRefs) ||
      hasAny(record.spendCapRefs)
    )
  ) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason:
        'Simulation Forum reward smoke must not carry wallet authority, named wallet, or spend-cap refs.',
    })
  }

  if (
    record.mode === 'live_bitcoin' &&
    (
      !record.usedLiveBitcoin ||
      !hasAny(record.walletAuthorityRefs) ||
      !hasAny(record.namedWalletRefs) ||
      !hasAny(record.spendCapRefs)
    )
  ) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason:
        'Live Forum reward smoke requires explicit owner-approved wallet authority, a named wallet ref, a concrete spend cap ref, and usedLiveBitcoin=true.',
    })
  }

  if (
    hasAny(record.acceptedWorkPayoutRefs) ||
    hasAny(record.providerSettlementRefs)
  ) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason:
        'Forum reward smoke keeps ordinary content rewards separate from accepted-work payout rows and provider settlement claims.',
    })
  }
}

export const artanisForumRewardSmokeProjectionHasPrivateMaterial = (
  projection: ArtanisForumRewardSmokeProjection,
): boolean =>
  stringValues(projection).some(value =>
    containsProviderSecretMaterial(value) ||
    unsafeRewardSmokePattern.test(value) ||
    rawTimestampPattern.test(value) ||
    (
      projection.audience !== 'operator' &&
      projection.audience !== 'private' &&
      publicUnsafeRefPattern.test(value)
    )
  )

export const projectArtanisForumRewardSmoke = (
  record: ArtanisForumRewardSmokeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisForumRewardSmokeProjection => {
  assertRecordSafe(record)

  const projection = new ArtanisForumRewardSmokeProjection({
    acceptedContributionBoundaryRefs: refsForAudience(
      'Artanis Forum reward smoke accepted-contribution boundary refs',
      record.acceptedContributionBoundaryRefs,
      audience,
    ),
    acceptedWorkPayoutClaimAllowed: false,
    acceptedWorkPayoutRefs: refsForAudience(
      'Artanis Forum reward smoke accepted-work payout refs',
      record.acceptedWorkPayoutRefs,
      audience,
    ),
    agentRef: record.agentRef,
    audience,
    authority: record.authority,
    caveatRefs: refsForAudience(
      'Artanis Forum reward smoke caveat refs',
      record.caveatRefs,
      audience,
    ),
    exchangeCount: record.exchangeRecords.length,
    exchangeRecords: record.exchangeRecords,
    mode: record.mode,
    modeLabel: modeLabelByMode[record.mode],
    namedWalletRefs: refsForAudience(
      'Artanis Forum reward smoke named wallet refs',
      record.namedWalletRefs,
      audience,
    ),
    providerSettlementClaimAllowed: false,
    providerSettlementRefs: refsForAudience(
      'Artanis Forum reward smoke provider settlement refs',
      record.providerSettlementRefs,
      audience,
    ),
    receiptProjectionRefs: refsForAudience(
      'Artanis Forum reward smoke receipt projection refs',
      record.receiptProjectionRefs,
      audience,
    ),
    registeredAgentRefs: refsForAudience(
      'Artanis Forum reward smoke registered agent refs',
      record.registeredAgentRefs,
      audience,
    ),
    runReasonRefs: refsForAudience(
      'Artanis Forum reward smoke run reason refs',
      record.runReasonRefs,
      audience,
    ),
    smokeRef: record.smokeRef,
    sourceRefs: refsForAudience(
      'Artanis Forum reward smoke source refs',
      record.sourceRefs,
      audience,
    ),
    spendCapRefs: refsForAudience(
      'Artanis Forum reward smoke spend cap refs',
      record.spendCapRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    usedLiveBitcoin: record.usedLiveBitcoin,
    walletAuthorityRefs: refsForAudience(
      'Artanis Forum reward smoke wallet authority refs',
      record.walletAuthorityRefs,
      audience,
    ),
  })

  if (artanisForumRewardSmokeProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisForumRewardSmokeUnsafe({
      reason:
        'Artanis Forum reward smoke projection contains private or unsafe material.',
    })
  }

  return projection
}

export const exampleArtanisForumRewardSmokeRecord =
  (): ArtanisForumRewardSmokeRecord =>
  new ArtanisForumRewardSmokeRecord({
    acceptedContributionBoundaryRefs: [
      'bridge.public.forum.accepted_contribution_requires_accepted_work_ref',
    ],
    acceptedWorkPayoutRefs: [],
    agentRef: 'agent_artanis',
    authority: ARTANIS_FORUM_REWARD_SMOKE_RECORD_ONLY_AUTHORITY,
    caveatRefs: [
      'caveat.public.forum_rewards_are_content_rewards',
      'caveat.public.no_accepted_work_payout_from_reward_smoke',
      'caveat.public.no_provider_settlement_from_reward_smoke',
    ],
    exchangeRecords: [
      new ArtanisForumRewardSmokeExchangeRecord({
        amountAsset: 'sats',
        amountValue: 100,
        earningNotificationRef:
          'notification.public.forum_reward.earning.agent_ben',
        fromAgentRef: 'agent.public.alice',
        postRef: 'post.public.forum.agent_ben.rewarded_by_alice',
        previewChallengeRef:
          'challenge.public.forum_reward.alice_to_ben.preview',
        receiptProjectionRef:
          'receipt_projection.public.forum_reward.alice_to_ben',
        receiptRef: 'receipt.public.forum_reward_alice_to_ben',
        toAgentRef: 'agent.public.ben',
      }),
      new ArtanisForumRewardSmokeExchangeRecord({
        amountAsset: 'sats',
        amountValue: 100,
        earningNotificationRef:
          'notification.public.forum_reward.earning.agent_alice',
        fromAgentRef: 'agent.public.ben',
        postRef: 'post.public.forum.agent_alice.rewarded_by_ben',
        previewChallengeRef:
          'challenge.public.forum_reward.ben_to_alice.preview',
        receiptProjectionRef:
          'receipt_projection.public.forum_reward.ben_to_alice',
        receiptRef: 'receipt.public.forum_reward_ben_to_alice',
        toAgentRef: 'agent.public.alice',
      }),
    ],
    mode: 'simulation',
    namedWalletRefs: [],
    providerSettlementRefs: [],
    receiptProjectionRefs: [
      'receipt_projection.public.forum_reward.alice_to_ben',
      'receipt_projection.public.forum_reward.ben_to_alice',
    ],
    registeredAgentRefs: ['agent.public.alice', 'agent.public.ben'],
    runReasonRefs: [
      'reason.public.no_owner_approved_named_wallet',
      'reason.public.no_concrete_spend_cap',
      'reason.public.deterministic_fake_bitcoin_simulation',
    ],
    smokeRef: 'smoke.public.artanis.forum_reward_back_and_forth',
    sourceRefs: [
      'docs/forum/2026-06-06-multi-agent-payment-tipping-simulation.md',
      'workers/api/src/forum/paid-actions.test.ts',
    ],
    spendCapRefs: [],
    updatedAtIso: '2026-06-07T02:00:00.000Z',
    usedLiveBitcoin: false,
    walletAuthorityRefs: [],
  })
