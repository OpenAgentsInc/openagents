import { Schema as S } from 'effect'

import {
  PublicClaimProjection,
  PublicClaimProjectionAudience,
  PublicClaimProjectionRecord,
  PublicClaimProjectionUnsafe,
  projectPublicClaimRecord,
  publicClaimProjectionHasPrivateMaterial,
} from './public-claim-projections'
import { PublicClaimKind, PublicClaimState } from './public-claim-state'

const EPISODE_228_UPDATED_AT = '2026-06-06T21:00:00.000Z'
const EPISODE_228_LAUNCH_REF = 'launch:episode_228_free_autopilot'
const EPISODE_228_SUBJECT_REF = 'autopilot:free_beta_launch'
const EPISODE_228_TRANSCRIPT_REF =
  'https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main/docs/transcripts/228.md'

export class OpenAgentsLaunchClaimLedgerEntry extends S.Class<OpenAgentsLaunchClaimLedgerEntry>(
  'OpenAgentsLaunchClaimLedgerEntry',
)({
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  titleRef: S.String,
  updatedAt: S.String,
}) {}

export class OpenAgentsLaunchClaimLedgerInput extends S.Class<OpenAgentsLaunchClaimLedgerInput>(
  'OpenAgentsLaunchClaimLedgerInput',
)({
  entries: S.Array(OpenAgentsLaunchClaimLedgerEntry),
  ledgerId: S.String,
  launchRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAt: S.String,
}) {}

export class OpenAgentsLaunchClaimLedgerProjectionEntry extends S.Class<OpenAgentsLaunchClaimLedgerProjectionEntry>(
  'OpenAgentsLaunchClaimLedgerProjectionEntry',
)({
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimProjection: PublicClaimProjection,
  claimRef: S.String,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  titleRef: S.String,
  updatedAt: S.String,
}) {}

export class OpenAgentsLaunchClaimLedgerProjection extends S.Class<OpenAgentsLaunchClaimLedgerProjection>(
  'OpenAgentsLaunchClaimLedgerProjection',
)({
  audience: PublicClaimProjectionAudience,
  entries: S.Array(OpenAgentsLaunchClaimLedgerProjectionEntry),
  ledgerId: S.String,
  launchRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAt: S.String,
}) {}

const sortedUniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => ref.trim() !== '').sort()

const claimRecordForEntry = (
  entry: OpenAgentsLaunchClaimLedgerEntry,
  ledger: OpenAgentsLaunchClaimLedgerInput,
): PublicClaimProjectionRecord => ({
  caveatRefs: sortedUniqueRefs(entry.caveatRefs),
  claimId: entry.claimId,
  claimKind: entry.claimKind,
  claimRef: entry.claimRef,
  customerRefs: [],
  desiredState: entry.desiredState,
  evidenceRefs: sortedUniqueRefs(entry.evidenceRefs),
  operatorRefs: [],
  sourceRefs: sortedUniqueRefs([...ledger.sourceRefs, ...entry.sourceRefs]),
  subjectRef: entry.subjectRef,
  surface: 'launch',
  teamRefs: [],
  titleRef: entry.titleRef,
  updatedAt: entry.updatedAt,
})

const assertProjectionPublicSafe = (
  projection: OpenAgentsLaunchClaimLedgerProjection,
): void => {
  if (publicClaimProjectionHasPrivateMaterial(projection)) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'Launch claim ledger projection contains private material.',
    })
  }
}

export const projectOpenAgentsLaunchClaimLedger = (
  ledger: OpenAgentsLaunchClaimLedgerInput,
  audience: PublicClaimProjectionAudience = 'public',
): OpenAgentsLaunchClaimLedgerProjection => {
  const projection: OpenAgentsLaunchClaimLedgerProjection = {
    audience,
    entries: ledger.entries.map(entry => {
      const claimProjection = projectPublicClaimRecord(
        claimRecordForEntry(entry, ledger),
        audience,
      )

      return {
        caveatRefs: sortedUniqueRefs(entry.caveatRefs),
        claimId: entry.claimId,
        claimKind: entry.claimKind,
        claimProjection,
        claimRef: entry.claimRef,
        desiredState: entry.desiredState,
        evidenceRefs: sortedUniqueRefs(entry.evidenceRefs),
        sourceRefs: sortedUniqueRefs([
          ...ledger.sourceRefs,
          ...entry.sourceRefs,
        ]),
        subjectRef: entry.subjectRef,
        titleRef: entry.titleRef,
        updatedAt: entry.updatedAt,
      }
    }),
    ledgerId: ledger.ledgerId,
    launchRef: ledger.launchRef,
    sourceRefs: sortedUniqueRefs(ledger.sourceRefs),
    updatedAt: ledger.updatedAt,
  }

  assertProjectionPublicSafe(projection)

  return projection
}

export const episode228LaunchClaimLedgerInput =
  (): OpenAgentsLaunchClaimLedgerInput => ({
    entries: [
      {
        caveatRefs: ['caveat.launch.scope.beta'],
        claimId: 'claim_episode_228_autopilot_beta_launch',
        claimKind: 'agent_challenge',
        claimRef: 'claim.episode_228.autopilot_beta_launch',
        desiredState: 'verified',
        evidenceRefs: [
          EPISODE_228_TRANSCRIPT_REF,
          'https://openagents.com/blog/free-autopilot',
        ],
        sourceRefs: ['source.openagents.blog.free_autopilot'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.autopilot_beta_launch',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.free_beta_limited_scope'],
        claimId: 'claim_episode_228_limited_free_beta',
        claimKind: 'public_beta_billing',
        claimRef: 'claim.episode_228.limited_free_beta',
        desiredState: 'verified',
        evidenceRefs: [
          EPISODE_228_TRANSCRIPT_REF,
          'https://openagents.com/blog/get-paid-to-code',
        ],
        sourceRefs: [
          'source.openagents.blog.free_autopilot',
          'source.openagents.blog.get_paid_to_code',
        ],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.limited_free_beta',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.public_trace_surface_partial'],
        claimId: 'claim_episode_228_public_traces_visible',
        claimKind: 'fulfillment_receipt',
        claimRef: 'claim.episode_228.public_traces_visible',
        desiredState: 'measured',
        evidenceRefs: [
          EPISODE_228_TRANSCRIPT_REF,
          'route:/api/public/proof/otec',
          'route:/api/public/adjutant/activity',
        ],
        sourceRefs: ['source.openagents.blog.free_autopilot'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.public_traces_visible',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.github_flow_public_repo_scope'],
        claimId: 'claim_episode_228_github_public_repo_flow',
        claimKind: 'agent_challenge',
        claimRef: 'claim.episode_228.github_public_repo_flow',
        desiredState: 'measured',
        evidenceRefs: [
          EPISODE_228_TRANSCRIPT_REF,
          'route:/api/auth/github/start',
          'route:/api/software-orders',
        ],
        sourceRefs: ['source.openagents.blog.free_autopilot'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.github_public_repo_flow',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.private_repo_support_not_live_claim'],
        claimId: 'claim_episode_228_private_repo_support',
        claimKind: 'agent_challenge',
        claimRef: 'claim.episode_228.private_repo_support',
        desiredState: 'planned',
        evidenceRefs: [EPISODE_228_TRANSCRIPT_REF],
        sourceRefs: ['source.openagents.blog.free_autopilot'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.private_repo_support',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.revenue_share_modeled_not_settled'],
        claimId: 'claim_episode_228_revenue_share_model',
        claimKind: 'provider_settlement',
        claimRef: 'claim.episode_228.revenue_share_model',
        desiredState: 'modeled',
        evidenceRefs: [EPISODE_228_TRANSCRIPT_REF],
        sourceRefs: ['source.openagents.blog.get_paid_to_code'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.revenue_share_model',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.no_public_accepted_work_settlement_yet'],
        claimId: 'claim_episode_228_accepted_work_payouts_settled',
        claimKind: 'provider_settlement',
        claimRef: 'claim.episode_228.accepted_work_payouts_settled',
        desiredState: 'prohibited',
        evidenceRefs: [],
        sourceRefs: ['source.openagents.blog.get_paid_to_code'],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.accepted_work_payouts_settled',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
      {
        caveatRefs: ['caveat.launch.superlative_not_public_proof_claim'],
        claimId: 'claim_episode_228_best_coding_agent_superlative',
        claimKind: 'agent_challenge',
        claimRef: 'claim.episode_228.best_coding_agent_superlative',
        desiredState: 'prohibited',
        evidenceRefs: [],
        sourceRefs: [EPISODE_228_TRANSCRIPT_REF],
        subjectRef: EPISODE_228_SUBJECT_REF,
        titleRef: 'title.episode_228.best_coding_agent_superlative',
        updatedAt: EPISODE_228_UPDATED_AT,
      },
    ],
    ledgerId: 'launch_claim_ledger_episode_228',
    launchRef: EPISODE_228_LAUNCH_REF,
    sourceRefs: [EPISODE_228_TRANSCRIPT_REF],
    updatedAt: EPISODE_228_UPDATED_AT,
  })

export const episode228LaunchClaimLedger = (
  audience: PublicClaimProjectionAudience = 'public',
): OpenAgentsLaunchClaimLedgerProjection =>
  projectOpenAgentsLaunchClaimLedger(
    episode228LaunchClaimLedgerInput(),
    audience,
  )
