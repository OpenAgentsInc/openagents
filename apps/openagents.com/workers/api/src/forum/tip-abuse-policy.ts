import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type ForumPaidActionKind,
  ForumTipSettlementState,
  ForumWriteDenialKind,
} from './schemas'

export const ForumTipPreviewRateLimit = {
  limit: 6,
  windowSeconds: 10 * 60,
} as const

export const ForumTipSelfTippingDenialRef =
  'policy.public.forum_tip.self_tipping_blocked'
export const ForumTipRateLimitDenialRef = 'policy.public.forum_tip.rate_limited'

export const ForumTipAbusePolicyProjection = S.Struct({
  collusionPolicy: S.Struct({
    policyRef: S.String,
    rankingEffect: S.String,
    settlementTruthPreserved: S.Boolean,
  }),
  duplicateTipPolicy: S.Struct({
    idempotencyRef: S.String,
    providerReplayRef: S.String,
  }),
  moderationTargetPolicyRefs: S.Array(S.String),
  paymentCannotUnlockRefs: S.Array(S.String),
  rateLimit: S.Struct({
    denialKind: ForumWriteDenialKind,
    denialRef: S.String,
    limit: S.Number,
    windowSeconds: S.Number,
  }),
  refundSettlementStates: S.Array(ForumTipSettlementState),
  reversalSettlementStates: S.Array(ForumTipSettlementState),
  selfTipping: S.Struct({
    denialKind: ForumWriteDenialKind,
    denialRef: S.String,
    state: S.Literal('blocked'),
  }),
  unsafeMaterialPolicyRefs: S.Array(S.String),
})
export type ForumTipAbusePolicyProjection =
  typeof ForumTipAbusePolicyProjection.Type

export type ForumTipPreviewPolicyDenial = Readonly<{
  denialKind: Exclude<typeof ForumWriteDenialKind.Type, 'payment_required'>
  denialRef: string
  requiredPermission: string | null
}>

export class ForumTipAbusePolicyUnsafe extends S.TaggedErrorClass<ForumTipAbusePolicyUnsafe>()(
  'ForumTipAbusePolicyUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeProjection = S.decodeUnknownSync(ForumTipAbusePolicyProjection)
const unsafePolicyMaterialPattern =
  /(\/Users\/|\/home\/|\.mdk-wallet|bearer\s+|checkout_id=|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?preimage|preimage=|private[_-]?key|provider[_-]?token|raw[_-]?(invoice|payment|payout|payload|webhook)|secret|sk-[a-z0-9]|wallet[_-]?(config|mnemonic|path|secret|state)|whsec_|\S+@\S+)/i

const scanUnsafePolicyMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    if (value.startsWith('policy.public.forum_tip.no_')) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      unsafePolicyMaterialPattern.test(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanUnsafePolicyMaterial(item, [...path, String(index)]),
      )
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) => scanUnsafePolicyMaterial(item, [...path, key]))
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertPublicSafePolicy = (projection: unknown): void => {
  const unsafePath = scanUnsafePolicyMaterial(projection)

  if (unsafePath !== undefined) {
    throw new ForumTipAbusePolicyUnsafe({
      reason: `Forum tip abuse/refund policy contains private payment, wallet, provider, payout, or raw target material at ${unsafePath}.`,
    })
  }
}

export const forumTipImmediatePreviewPolicyDenial = (
  input: Readonly<{
    actionKind: ForumPaidActionKind
    actorRef: string
    recipientActorRef: string | null
  }>,
): ForumTipPreviewPolicyDenial | null =>
  input.actionKind === 'post_reward' &&
  input.recipientActorRef !== null &&
  input.actorRef === input.recipientActorRef
    ? {
        denialKind: 'safety_denied',
        denialRef: ForumTipSelfTippingDenialRef,
        requiredPermission: null,
      }
    : null

export const forumTipRateLimitPreviewPolicyDenial = (
  input: Readonly<{
    actionKind: ForumPaidActionKind
    recentChallengeCount: number
  }>,
): ForumTipPreviewPolicyDenial | null =>
  input.actionKind === 'post_reward' &&
  input.recentChallengeCount >= ForumTipPreviewRateLimit.limit
    ? {
        denialKind: 'rate_limited',
        denialRef: ForumTipRateLimitDenialRef,
        requiredPermission: null,
      }
    : null

export const projectForumTipAbusePolicy = (): ForumTipAbusePolicyProjection => {
  const projection = decodeProjection({
    collusionPolicy: {
      policyRef: 'policy.public.forum_tip.collusion_scoring_exclusion',
      rankingEffect:
        'Suspected collusive tips can be excluded from ranking, scoring, and farming rewards without rewriting payment-settlement truth.',
      settlementTruthPreserved: true,
    },
    duplicateTipPolicy: {
      idempotencyRef: 'policy.public.forum_tip.idempotent_challenge_replay',
      providerReplayRef:
        'policy.public.forum_tip.duplicate_provider_event_rejected',
    },
    moderationTargetPolicyRefs: [
      'policy.public.forum_tip.hidden_targets_do_not_issue_challenges',
      'policy.public.forum_tip.held_for_review_targets_do_not_issue_challenges',
      'policy.public.forum_tip.tombstoned_targets_do_not_issue_challenges',
    ],
    paymentCannotUnlockRefs: [
      'scope.public.forum_tip.cannot_unlock_admin',
      'scope.public.forum_tip.cannot_unlock_customer_order',
      'scope.public.forum_tip.cannot_unlock_legal',
      'scope.public.forum_tip.cannot_unlock_moderation',
      'scope.public.forum_tip.cannot_unlock_owner',
      'scope.public.forum_tip.cannot_unlock_private_data',
      'scope.public.forum_tip.cannot_unlock_repository',
      'scope.public.forum_tip.cannot_unlock_safety',
      'scope.public.forum_tip.cannot_unlock_site_deploy',
    ],
    rateLimit: {
      denialKind: 'rate_limited',
      denialRef: ForumTipRateLimitDenialRef,
      limit: ForumTipPreviewRateLimit.limit,
      windowSeconds: ForumTipPreviewRateLimit.windowSeconds,
    },
    refundSettlementStates: ['refunded'],
    reversalSettlementStates: ['reversed'],
    selfTipping: {
      denialKind: 'safety_denied',
      denialRef: ForumTipSelfTippingDenialRef,
      state: 'blocked',
    },
    unsafeMaterialPolicyRefs: [
      'policy.public.forum_tip.no_invoice_in_public_projection',
      'policy.public.forum_tip.no_l402_token_in_public_projection',
      'policy.public.forum_tip.no_mnemonic_in_public_projection',
      'policy.public.forum_tip.no_payment_hash_in_public_projection',
      'policy.public.forum_tip.no_preimage_in_public_projection',
      'policy.public.forum_tip.no_provider_payload_in_public_projection',
      'policy.public.forum_tip.no_raw_payout_target_in_public_projection',
    ],
  })

  assertPublicSafePolicy(projection)

  return projection
}

export const forumTipAbusePolicyHasPrivateMaterial = (
  value: unknown,
): boolean => scanUnsafePolicyMaterial(value) !== undefined
