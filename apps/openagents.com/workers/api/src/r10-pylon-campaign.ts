import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  PublicClaimCopyRule,
  PublicClaimKind,
  PublicClaimState,
  PublicClaimStateProjection,
} from './public-claim-state'
import {
  PublicClaimProjectionAudience,
  PublicClaimProjectionUnsafe,
  projectPublicClaimRecord,
  publicClaimProjectionHasPrivateMaterial,
} from './public-claim-projections'

export const R10PylonCampaignArea = S.Literals([
  'accepted_work_accounting',
  'artanis_public_surface',
  'bitcoin_settlement_claims',
  'live_spend_authority',
  'provider_registration',
  'pylon_release',
  'work_routing',
])
export type R10PylonCampaignArea = typeof R10PylonCampaignArea.Type

export class R10PylonCampaignClaimEntry extends S.Class<R10PylonCampaignClaimEntry>(
  'R10PylonCampaignClaimEntry',
)({
  area: R10PylonCampaignArea,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  participantActionRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  titleRef: S.String,
  updatedAtIso: S.String,
}) {}

export class R10PylonCampaignInput extends S.Class<R10PylonCampaignInput>(
  'R10PylonCampaignInput',
)({
  agentRef: S.String,
  campaignRef: S.String,
  entries: S.Array(R10PylonCampaignClaimEntry),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class R10PylonCampaignProjectionEntry extends S.Class<R10PylonCampaignProjectionEntry>(
  'R10PylonCampaignProjectionEntry',
)({
  area: R10PylonCampaignArea,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  copyRule: PublicClaimCopyRule,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  participantActionRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PublicClaimStateProjection,
  subjectRef: S.String,
  titleRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class R10PylonCampaignProjection extends S.Class<R10PylonCampaignProjection>(
  'R10PylonCampaignProjection',
)({
  agentRef: S.String,
  audience: PublicClaimProjectionAudience,
  campaignRef: S.String,
  entries: S.Array(R10PylonCampaignProjectionEntry),
  sourceRefs: S.Array(S.String),
  stateCounts: S.Array(S.Struct({
    count: S.Number,
    state: PublicClaimState,
  })),
  updatedAtDisplay: S.String,
}) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PublicClaimProjectionUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw timestamp material.`,
    })
  }
}

const assertEntrySafe = (entry: R10PylonCampaignClaimEntry): void => {
  assertSafeRefs('campaign claim refs', [
    entry.claimId,
    entry.claimRef,
    entry.subjectRef,
    entry.titleRef,
  ])
  assertSafeRefs('campaign caveat refs', entry.caveatRefs)
  assertSafeRefs('campaign evidence refs', entry.evidenceRefs)
  assertSafeRefs('campaign source refs', entry.sourceRefs)
  assertSafeRefs('campaign next-action refs', entry.nextActionRefs)
  assertSafeRefs(
    'campaign participant-action refs',
    entry.participantActionRefs,
  )
  assertSafeRefs('campaign blocker refs', entry.blockedByRefs)
}

const campaignProjectionText = (
  projection: R10PylonCampaignProjection,
): string =>
  [
    projection.agentRef,
    projection.campaignRef,
    ...projection.sourceRefs,
    ...projection.entries.flatMap(entry => [
      entry.claimId,
      entry.claimRef,
      entry.subjectRef,
      entry.titleRef,
      ...entry.caveatRefs,
      ...entry.evidenceRefs,
      ...entry.sourceRefs,
      ...entry.nextActionRefs,
      ...entry.participantActionRefs,
      ...entry.blockedByRefs,
    ]),
  ].join(' ')

export const r10PylonCampaignProjectionHasPrivateMaterial = (
  projection: R10PylonCampaignProjection,
): boolean => {
  const text = campaignProjectionText(projection)

  return publicClaimProjectionHasPrivateMaterial(text) ||
    unsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text)
}

const stateCounts = (
  entries: ReadonlyArray<R10PylonCampaignProjectionEntry>,
): ReadonlyArray<{ count: number; state: PublicClaimState }> =>
  [...entries.reduce((counts, entry) => {
    counts.set(entry.state.state, (counts.get(entry.state.state) ?? 0) + 1)

    return counts
  }, new Map<PublicClaimState, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state, count]) => ({ count, state }))

const projectEntry = (
  entry: R10PylonCampaignClaimEntry,
  ledger: R10PylonCampaignInput,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): R10PylonCampaignProjectionEntry => {
  assertEntrySafe(entry)

  const claimProjection = projectPublicClaimRecord({
    caveatRefs: uniqueRefs(entry.caveatRefs),
    claimId: entry.claimId,
    claimKind: entry.claimKind,
    claimRef: entry.claimRef,
    customerRefs: [],
    desiredState: entry.desiredState,
    evidenceRefs: uniqueRefs(entry.evidenceRefs),
    operatorRefs: [],
    sourceRefs: uniqueRefs([...ledger.sourceRefs, ...entry.sourceRefs]),
    subjectRef: entry.subjectRef,
    surface: 'pylon',
    teamRefs: [],
    titleRef: entry.titleRef,
    updatedAt: entry.updatedAtIso,
  }, audience)

  return {
    area: entry.area,
    blockedByRefs: uniqueRefs(entry.blockedByRefs),
    caveatRefs: claimProjection.caveatRefs,
    claimId: claimProjection.claimId,
    claimKind: claimProjection.claimKind,
    claimRef: claimProjection.claimRef,
    copyRule: claimProjection.copyRule,
    desiredState: entry.desiredState,
    evidenceRefs: claimProjection.evidenceRefs,
    nextActionRefs: uniqueRefs(entry.nextActionRefs),
    participantActionRefs: uniqueRefs(entry.participantActionRefs),
    sourceRefs: claimProjection.sourceRefs,
    state: claimProjection.state,
    subjectRef: claimProjection.subjectRef,
    titleRef: claimProjection.titleRef,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      entry.updatedAtIso,
      nowIso,
    ),
  }
}

export const projectR10PylonCampaign = (
  input: R10PylonCampaignInput,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): R10PylonCampaignProjection => {
  assertSafeRefs('campaign identity refs', [
    input.agentRef,
    input.campaignRef,
  ])
  assertSafeRefs('campaign source refs', input.sourceRefs)

  const entries = input.entries.map(entry =>
    projectEntry(entry, input, audience, nowIso),
  )
  const projection: R10PylonCampaignProjection = {
    agentRef: input.agentRef,
    audience,
    campaignRef: input.campaignRef,
    entries,
    sourceRefs: uniqueRefs(input.sourceRefs),
    stateCounts: stateCounts(entries),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.updatedAtIso,
      nowIso,
    ),
  }

  if (r10PylonCampaignProjectionHasPrivateMaterial(projection)) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'R10 Pylon campaign projection contains private material.',
    })
  }

  return projection
}

export const r10PylonCampaignInput = (): R10PylonCampaignInput => ({
  agentRef: 'agent_artanis',
  campaignRef: 'campaign.r10_pylon',
  entries: [
    {
      area: 'artanis_public_surface',
      blockedByRefs: [],
      caveatRefs: ['caveat.public_surface.sanitized_activity_only'],
      claimId: 'claim_r10_artanis_public_surface',
      claimKind: 'agent_challenge',
      claimRef: 'claim.r10_pylon.artanis_public_surface',
      desiredState: 'measured',
      evidenceRefs: [
        'https://openagents.com/agents/artanis',
        'route:/api/public/pylon-stats',
        'docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md',
      ],
      nextActionRefs: ['next_action.keep_public_goal_current'],
      participantActionRefs: ['participant_action.inspect_public_campaign'],
      sourceRefs: ['source.r10_pylon.autopilot_task'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.artanis_public_surface',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'provider_registration',
      blockedByRefs: [],
      caveatRefs: ['caveat.pylon.setup_requires_owner_approval'],
      claimId: 'claim_r10_pylon_setup_packet',
      claimKind: 'fulfillment_receipt',
      claimRef: 'claim.r10_pylon.setup_packet',
      desiredState: 'verified',
      evidenceRefs: [
        'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
        'docs/live/AGENTS.md',
      ],
      nextActionRefs: ['next_action.validate_pylon_release_assets'],
      participantActionRefs: ['participant_action.review_pylon_setup'],
      sourceRefs: ['source.pylon.local_compute_packet'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.pylon_setup_packet',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'pylon_release',
      blockedByRefs: ['blocker.pylon.release_artifact_not_retained'],
      caveatRefs: ['caveat.pylon.release_not_publicly_verified'],
      claimId: 'claim_r10_pylon_next_release',
      claimKind: 'deployment',
      claimRef: 'claim.r10_pylon.next_release',
      desiredState: 'planned',
      evidenceRefs: [],
      nextActionRefs: ['next_action.retain_release_artifact_and_smoke'],
      participantActionRefs: ['participant_action.wait_for_release_receipt'],
      sourceRefs: ['source.pylon.release_plan'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.pylon_next_release',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'work_routing',
      blockedByRefs: ['blocker.pylon.work_routing_not_live_claim'],
      caveatRefs: ['caveat.pylon.routing_not_yet_verified'],
      claimId: 'claim_r10_pylon_work_routing',
      claimKind: 'agent_challenge',
      claimRef: 'claim.r10_pylon.work_routing',
      desiredState: 'planned',
      evidenceRefs: [],
      nextActionRefs: ['next_action.define_bounded_pylon_work_slice'],
      participantActionRefs: ['participant_action.inspect_routing_plan'],
      sourceRefs: ['source.r10_pylon.autopilot_task'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.pylon_work_routing',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'accepted_work_accounting',
      blockedByRefs: ['blocker.accepted_work_receipts_not_yet_public'],
      caveatRefs: ['caveat.bitcoin_accounting.modeled_not_settled'],
      claimId: 'claim_r10_pylon_bitcoin_accounting_model',
      claimKind: 'provider_settlement',
      claimRef: 'claim.r10_pylon.bitcoin_accounting_model',
      desiredState: 'modeled',
      evidenceRefs: [
        'docs/forum/2026-06-06-multi-agent-payment-tipping-simulation.md',
        'docs/omni/2026-06-05-accepted-outcome-economics-v1.md',
      ],
      nextActionRefs: ['next_action.add_accepted_work_receipt_projection'],
      participantActionRefs: ['participant_action.review_bitcoin_accounting_boundary'],
      sourceRefs: ['source.omega.accepted_outcome_economics'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.pylon_bitcoin_accounting_model',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'live_spend_authority',
      blockedByRefs: ['blocker.no_approved_live_spend_cap'],
      caveatRefs: ['caveat.live_spend_requires_explicit_owner_approval'],
      claimId: 'claim_r10_live_spend_forum_tipping_smoke',
      claimKind: 'provider_settlement',
      claimRef: 'claim.r10_pylon.live_spend_forum_tipping_smoke',
      desiredState: 'blocked',
      evidenceRefs: [],
      nextActionRefs: ['next_action.obtain_named_spend_authority_and_cap'],
      participantActionRefs: ['participant_action.use_fake_bitcoin_simulation_until_approved'],
      sourceRefs: ['source.forum.fake_bitcoin_tipping_simulation'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.live_spend_tipping_smoke',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
    {
      area: 'bitcoin_settlement_claims',
      blockedByRefs: ['blocker.no_public_settlement_receipt_chain'],
      caveatRefs: ['caveat.no_settled_provider_payout_claim'],
      claimId: 'claim_r10_provider_payouts_settled',
      claimKind: 'provider_settlement',
      claimRef: 'claim.r10_pylon.provider_payouts_settled',
      desiredState: 'prohibited',
      evidenceRefs: [],
      nextActionRefs: ['next_action.wait_for_treasury_settlement_receipts'],
      participantActionRefs: ['participant_action.do_not_claim_payouts_settled'],
      sourceRefs: ['source.public_claim_copy_boundary'],
      subjectRef: 'campaign:r10_pylon',
      titleRef: 'title.r10.provider_payouts_settled',
      updatedAtIso: '2026-06-06T21:05:00.000Z',
    },
  ],
  sourceRefs: [
    'docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md',
    'docs/2026-06-04-openai-codex-goal-implementation-audit.md',
  ],
  updatedAtIso: '2026-06-06T21:05:00.000Z',
})
