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

export const ArtanisStandaloneClaimArea = S.Literals([
  'accepted_work_payout',
  'autonomous_loop',
  'bitcoin_rewards',
  'forum_communication',
  'model_lab_stewardship',
  'nexus_pylon_administration',
  'operator_steering',
  'pylon_campaign',
  'settlement',
  'spend_authority',
  'work_routing',
])
export type ArtanisStandaloneClaimArea =
  typeof ArtanisStandaloneClaimArea.Type

export class ArtanisStandaloneClaimEntry extends S.Class<ArtanisStandaloneClaimEntry>(
  'ArtanisStandaloneClaimEntry',
)({
  area: ArtanisStandaloneClaimArea,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  forumCopyRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  titleRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisStandaloneClaimLedgerInput extends S.Class<ArtanisStandaloneClaimLedgerInput>(
  'ArtanisStandaloneClaimLedgerInput',
)({
  agentRef: S.String,
  entries: S.Array(ArtanisStandaloneClaimEntry),
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisStandaloneClaimLedgerProjectionEntry extends S.Class<ArtanisStandaloneClaimLedgerProjectionEntry>(
  'ArtanisStandaloneClaimLedgerProjectionEntry',
)({
  area: ArtanisStandaloneClaimArea,
  blockedByRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  copyRule: PublicClaimCopyRule,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  forumCopyRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PublicClaimStateProjection,
  subjectRef: S.String,
  titleRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisStandaloneClaimLedgerProjection extends S.Class<ArtanisStandaloneClaimLedgerProjection>(
  'ArtanisStandaloneClaimLedgerProjection',
)({
  agentRef: S.String,
  audience: PublicClaimProjectionAudience,
  entries: S.Array(ArtanisStandaloneClaimLedgerProjectionEntry),
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  stateCounts: S.Array(S.Struct({
    count: S.Number,
    state: PublicClaimState,
  })),
  updatedAtDisplay: S.String,
}) {}

const requiredAreaList: ReadonlyArray<ArtanisStandaloneClaimArea> = [
  'accepted_work_payout',
  'autonomous_loop',
  'bitcoin_rewards',
  'forum_communication',
  'model_lab_stewardship',
  'nexus_pylon_administration',
  'operator_steering',
  'pylon_campaign',
  'settlement',
  'spend_authority',
  'work_routing',
]
const requiredAreas: ReadonlySet<ArtanisStandaloneClaimArea> =
  new Set(requiredAreaList)

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

const assertEntrySafe = (entry: ArtanisStandaloneClaimEntry): void => {
  assertSafeRefs('Artanis claim refs', [
    entry.claimId,
    entry.claimRef,
    entry.subjectRef,
    entry.titleRef,
  ])
  assertSafeRefs('Artanis blocker refs', entry.blockedByRefs)
  assertSafeRefs('Artanis caveat refs', entry.caveatRefs)
  assertSafeRefs('Artanis evidence refs', entry.evidenceRefs)
  assertSafeRefs('Artanis Forum copy refs', entry.forumCopyRefs)
  assertSafeRefs('Artanis next-action refs', entry.nextActionRefs)
  assertSafeRefs('Artanis source refs', entry.sourceRefs)
}

const assertAllRequiredAreasPresent = (
  entries: ReadonlyArray<ArtanisStandaloneClaimEntry>,
): void => {
  const present = new Set(entries.map(entry => entry.area))
  const missing = [...requiredAreas].filter(area => !present.has(area))

  if (missing.length > 0) {
    throw new PublicClaimProjectionUnsafe({
      reason: `Artanis standalone claim ledger is missing required areas: ${missing.join(', ')}.`,
    })
  }
}

const ledgerProjectionText = (
  projection: ArtanisStandaloneClaimLedgerProjection,
): string =>
  [
    projection.agentRef,
    projection.ledgerRef,
    ...projection.sourceRefs,
    ...projection.entries.flatMap(entry => [
      entry.claimId,
      entry.claimRef,
      entry.subjectRef,
      entry.titleRef,
      ...entry.blockedByRefs,
      ...entry.caveatRefs,
      ...entry.evidenceRefs,
      ...entry.forumCopyRefs,
      ...entry.nextActionRefs,
      ...entry.sourceRefs,
    ]),
  ].join(' ')

export const artanisStandaloneClaimLedgerProjectionHasPrivateMaterial = (
  projection: ArtanisStandaloneClaimLedgerProjection,
): boolean => {
  const text = ledgerProjectionText(projection)

  return publicClaimProjectionHasPrivateMaterial(text) ||
    unsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text)
}

const stateCounts = (
  entries: ReadonlyArray<ArtanisStandaloneClaimLedgerProjectionEntry>,
): ReadonlyArray<{ count: number; state: PublicClaimState }> =>
  [...entries.reduce((counts, entry) => {
    counts.set(entry.state.state, (counts.get(entry.state.state) ?? 0) + 1)

    return counts
  }, new Map<PublicClaimState, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state, count]) => ({ count, state }))

const projectEntry = (
  entry: ArtanisStandaloneClaimEntry,
  ledger: ArtanisStandaloneClaimLedgerInput,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): ArtanisStandaloneClaimLedgerProjectionEntry => {
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
    surface: entry.area === 'forum_communication' ? 'forum' : 'public_agent',
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
    forumCopyRefs: uniqueRefs(entry.forumCopyRefs),
    nextActionRefs: uniqueRefs(entry.nextActionRefs),
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

export const projectArtanisStandaloneClaimLedger = (
  input: ArtanisStandaloneClaimLedgerInput,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): ArtanisStandaloneClaimLedgerProjection => {
  assertSafeRefs('Artanis ledger identity refs', [
    input.agentRef,
    input.ledgerRef,
  ])
  assertSafeRefs('Artanis ledger source refs', input.sourceRefs)

  if (input.agentRef !== 'agent_artanis') {
    throw new PublicClaimProjectionUnsafe({
      reason: 'Artanis standalone claim ledger must be bound to agent_artanis.',
    })
  }

  assertAllRequiredAreasPresent(input.entries)

  const entries = input.entries.map(entry =>
    projectEntry(entry, input, audience, nowIso),
  )
  const projection: ArtanisStandaloneClaimLedgerProjection = {
    agentRef: input.agentRef,
    audience,
    entries,
    ledgerRef: input.ledgerRef,
    sourceRefs: uniqueRefs(input.sourceRefs),
    stateCounts: stateCounts(entries),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.updatedAtIso,
      nowIso,
    ),
  }

  if (artanisStandaloneClaimLedgerProjectionHasPrivateMaterial(projection)) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'Artanis standalone claim ledger projection contains private material.',
    })
  }

  return projection
}

export const exampleArtanisStandaloneClaimLedger = ():
  ArtanisStandaloneClaimLedgerInput => ({
  agentRef: 'agent_artanis',
  entries: [
    {
      area: 'autonomous_loop',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.loop_public_projection_only'],
      claimId: 'claim_artanis_autonomous_loop_observed',
      claimKind: 'agent_challenge',
      claimRef: 'claim.artanis.autonomous_loop_observed',
      desiredState: 'measured',
      evidenceRefs: [
        'route:/api/public/artanis/report',
        'docs/artanis/2026-06-06-autonomous-loop-contract.md',
      ],
      forumCopyRefs: ['forum.copy.artanis.loop_measured'],
      nextActionRefs: ['next_action.persist_artanis_loop_records'],
      sourceRefs: ['source.artanis.loop_contract'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.autonomous_loop',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'operator_steering',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.operator_approval_required_for_risk'],
      claimId: 'claim_artanis_operator_steering_verified',
      claimKind: 'fulfillment_receipt',
      claimRef: 'claim.artanis.operator_steering_verified',
      desiredState: 'verified',
      evidenceRefs: [
        'docs/artanis/2026-06-06-operator-steering-contract.md',
        'docs/artanis/2026-06-06-operator-approval-gates.md',
      ],
      forumCopyRefs: ['forum.copy.artanis.operator_steering_verified'],
      nextActionRefs: ['next_action.add_artanis_operator_console'],
      sourceRefs: ['source.artanis.operator_contract'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.operator_steering',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'forum_communication',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.forum_posts_are_public_safe'],
      claimId: 'claim_artanis_forum_communication_verified',
      claimKind: 'agent_challenge',
      claimRef: 'claim.artanis.forum_communication_verified',
      desiredState: 'verified',
      evidenceRefs: [
        'docs/artanis/2026-06-06-forum-taxonomy.md',
        'docs/artanis/2026-06-06-forum-publication-queue.md',
        'route:/api/forum/forums/artanis',
      ],
      forumCopyRefs: ['forum.copy.artanis.forum_communication_verified'],
      nextActionRefs: ['next_action.deliver_artanis_publication_queue'],
      sourceRefs: ['source.artanis.forum_contracts'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.forum_communication',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'pylon_campaign',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.pylon_campaign_uses_r10_ledger'],
      claimId: 'claim_artanis_pylon_campaign_measured',
      claimKind: 'agent_challenge',
      claimRef: 'claim.artanis.pylon_campaign_measured',
      desiredState: 'measured',
      evidenceRefs: [
        'docs/pylon/2026-06-06-r10-artanis-pylon-campaign-ledger.md',
        'route:/api/public/pylon-stats',
      ],
      forumCopyRefs: ['forum.copy.artanis.pylon_campaign_measured'],
      nextActionRefs: ['next_action.prepare_pylon_v0_2_launch_readiness'],
      sourceRefs: ['source.artanis.r10_pylon_ledger'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.pylon_campaign',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'nexus_pylon_administration',
      blockedByRefs: ['blocker.artanis.nexus_pylon_admin_adapters_not_live'],
      caveatRefs: ['caveat.artanis.nexus_pylon_admin_planned_only'],
      claimId: 'claim_artanis_nexus_pylon_admin_planned',
      claimKind: 'agent_challenge',
      claimRef: 'claim.artanis.nexus_pylon_admin_planned',
      desiredState: 'planned',
      evidenceRefs: [],
      forumCopyRefs: ['forum.copy.artanis.nexus_pylon_admin_planned'],
      nextActionRefs: ['next_action.connect_artanis_nexus_pylon_adapters'],
      sourceRefs: ['source.artanis.discord_pylon_v0_2_context'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.nexus_pylon_administration',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'model_lab_stewardship',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.model_lab_stewardship_public_report_only'],
      claimId: 'claim_artanis_model_lab_stewardship_verified',
      claimKind: 'research',
      claimRef: 'claim.artanis.model_lab_stewardship_verified',
      desiredState: 'verified',
      evidenceRefs: [
        'docs/artanis/2026-06-06-model-lab-context-bridge.md',
        'docs/omni/2026-06-06-model-lab-public-report-projection.md',
      ],
      forumCopyRefs: ['forum.copy.artanis.model_lab_verified'],
      nextActionRefs: ['next_action.seed_continual_learning_jobs'],
      sourceRefs: ['source.model_lab.public_report_projection'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.model_lab_stewardship',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'work_routing',
      blockedByRefs: [],
      caveatRefs: ['caveat.artanis.work_routing_modeled_not_dispatched'],
      claimId: 'claim_artanis_work_routing_modeled',
      claimKind: 'agent_challenge',
      claimRef: 'claim.artanis.work_routing_modeled',
      desiredState: 'modeled',
      evidenceRefs: [
        'docs/artanis/2026-06-06-work-routing-contract.md',
      ],
      forumCopyRefs: ['forum.copy.artanis.work_routing_modeled'],
      nextActionRefs: ['next_action.implement_pylon_marketplace_job_intake'],
      sourceRefs: ['source.artanis.work_routing_contract'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.work_routing',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'spend_authority',
      blockedByRefs: ['blocker.artanis.no_live_spend_authority'],
      caveatRefs: ['caveat.artanis.live_spend_requires_named_owner_cap'],
      claimId: 'claim_artanis_spend_authority_blocked',
      claimKind: 'provider_settlement',
      claimRef: 'claim.artanis.spend_authority_blocked',
      desiredState: 'blocked',
      evidenceRefs: [],
      forumCopyRefs: ['forum.copy.artanis.spend_authority_blocked'],
      nextActionRefs: ['next_action.require_operator_spend_gate'],
      sourceRefs: ['source.artanis.approval_gates'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.spend_authority',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'bitcoin_rewards',
      blockedByRefs: ['blocker.artanis.forum_reward_smoke_not_complete'],
      caveatRefs: ['caveat.artanis.bitcoin_rewards_not_automatic_income_claim'],
      claimId: 'claim_artanis_bitcoin_rewards_blocked',
      claimKind: 'provider_settlement',
      claimRef: 'claim.artanis.bitcoin_rewards_blocked',
      desiredState: 'blocked',
      evidenceRefs: [],
      forumCopyRefs: ['forum.copy.artanis.bitcoin_rewards_blocked'],
      nextActionRefs: ['next_action.add_forum_bitcoin_reward_smoke'],
      sourceRefs: ['source.forum.bitcoin_reward_plan'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.bitcoin_rewards',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'accepted_work_payout',
      blockedByRefs: ['blocker.artanis.no_public_accepted_work_receipt_chain'],
      caveatRefs: ['caveat.artanis.accepted_work_payout_claim_prohibited'],
      claimId: 'claim_artanis_accepted_work_payout_prohibited',
      claimKind: 'provider_settlement',
      claimRef: 'claim.artanis.accepted_work_payout_prohibited',
      desiredState: 'prohibited',
      evidenceRefs: [],
      forumCopyRefs: ['forum.copy.artanis.accepted_work_payout_prohibited'],
      nextActionRefs: ['next_action.wait_for_public_accepted_work_receipts'],
      sourceRefs: ['source.public_claim_copy_boundary'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.accepted_work_payout',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
    {
      area: 'settlement',
      blockedByRefs: ['blocker.artanis.no_public_settlement_receipt_chain'],
      caveatRefs: ['caveat.artanis.settlement_claim_prohibited_until_receipts'],
      claimId: 'claim_artanis_settlement_prohibited',
      claimKind: 'provider_settlement',
      claimRef: 'claim.artanis.settlement_prohibited',
      desiredState: 'prohibited',
      evidenceRefs: [],
      forumCopyRefs: ['forum.copy.artanis.settlement_prohibited'],
      nextActionRefs: ['next_action.require_settlement_receipt_chain'],
      sourceRefs: ['source.public_claim_copy_boundary'],
      subjectRef: 'agent:artanis',
      titleRef: 'title.artanis.settlement',
      updatedAtIso: '2026-06-06T23:30:00.000Z',
    },
  ],
  ledgerRef: 'ledger.public.artanis.standalone_autonomy',
  sourceRefs: [
    'docs/artanis/2026-06-06-artanis-implementation-audit.md',
    'docs/pylon/2026-06-06-r10-artanis-pylon-campaign-ledger.md',
  ],
  updatedAtIso: '2026-06-06T23:30:00.000Z',
})
