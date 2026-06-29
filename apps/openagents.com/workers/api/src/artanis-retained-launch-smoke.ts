import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  ArtanisProductionLaunchGateCheck,
} from './artanis-production-launch-gate'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisRetainedLaunchSmokeEnvironment = S.Literals([
  'local_equivalent',
  'production',
  'staging_equivalent',
])
export type ArtanisRetainedLaunchSmokeEnvironment =
  typeof ArtanisRetainedLaunchSmokeEnvironment.Type

export const ArtanisRetainedLaunchSmokeSchedulerMode = S.Literals([
  'disabled',
  'fake_provider_one_tick',
  'no_launch',
  'one_tick_window',
])
export type ArtanisRetainedLaunchSmokeSchedulerMode =
  typeof ArtanisRetainedLaunchSmokeSchedulerMode.Type

export const ArtanisRetainedLaunchSmokeForumMode = S.Literals([
  'delivered_post',
  'no_publish_test',
])
export type ArtanisRetainedLaunchSmokeForumMode =
  typeof ArtanisRetainedLaunchSmokeForumMode.Type

export const ArtanisRetainedLaunchSmokeState = S.Literals([
  'blocked',
  'retained',
])
export type ArtanisRetainedLaunchSmokeState =
  typeof ArtanisRetainedLaunchSmokeState.Type

export const ArtanisRetainedLaunchSmokeAuthorityBoundary = S.Literals([
  'read_only_retained_production_equivalent_smoke',
])
export type ArtanisRetainedLaunchSmokeAuthorityBoundary =
  typeof ArtanisRetainedLaunchSmokeAuthorityBoundary.Type

export class ArtanisRetainedLaunchSmokeAuthority extends S.Class<ArtanisRetainedLaunchSmokeAuthority>(
  'ArtanisRetainedLaunchSmokeAuthority',
)({
  authorityBoundary: ArtanisRetainedLaunchSmokeAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noDeployment: S.Boolean,
  noForumMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPylonDispatch: S.Boolean,
  noSchedulerMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noTrainingLaunch: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export class ArtanisRetainedLaunchSmokePersistedRefs extends S.Class<ArtanisRetainedLaunchSmokePersistedRefs>(
  'ArtanisRetainedLaunchSmokePersistedRefs',
)({
  approvalGateRefs: S.Array(S.String),
  forumPublicationIntentRefs: S.Array(S.String),
  healthSnapshotRefs: S.Array(S.String),
  loopRecordRefs: S.Array(S.String),
  loopTickRefs: S.Array(S.String),
  runtimeSnapshotRefs: S.Array(S.String),
  workRoutingProposalRefs: S.Array(S.String),
}) {}

export class ArtanisRetainedLaunchSmokeRecord extends S.Class<ArtanisRetainedLaunchSmokeRecord>(
  'ArtanisRetainedLaunchSmokeRecord',
)({
  agentRef: S.String,
  authority: ArtanisRetainedLaunchSmokeAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  environment: ArtanisRetainedLaunchSmokeEnvironment,
  environmentRef: S.String,
  forumMode: ArtanisRetainedLaunchSmokeForumMode,
  forumPostRefs: S.Array(S.String),
  noPublishProofRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  persistedRefs: ArtanisRetainedLaunchSmokePersistedRefs,
  privateEvidenceRefs: S.Array(S.String),
  publicReportRefs: S.Array(S.String),
  rollbackDisableRefs: S.Array(S.String),
  schedulerMode: ArtanisRetainedLaunchSmokeSchedulerMode,
  smokeRunRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisRetainedLaunchSmokeProjection extends S.Class<ArtanisRetainedLaunchSmokeProjection>(
  'ArtanisRetainedLaunchSmokeProjection',
)({
  agentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisRetainedLaunchSmokeAuthority,
  blockerRefs: S.Array(S.String),
  buyerChargeMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  deploymentAllowed: S.Boolean,
  environment: ArtanisRetainedLaunchSmokeEnvironment,
  environmentRef: S.String,
  forumDeliveryVerified: S.Boolean,
  forumMode: ArtanisRetainedLaunchSmokeForumMode,
  forumMutationAllowed: S.Boolean,
  forumPostRefs: S.Array(S.String),
  noPublishProofRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  persistedRowRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  providerMutationAllowed: S.Boolean,
  publicReportRefs: S.Array(S.String),
  pylonDispatchAllowed: S.Boolean,
  rollbackDisableRefs: S.Array(S.String),
  schedulerMode: ArtanisRetainedLaunchSmokeSchedulerMode,
  schedulerMutationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  smokeRunRef: S.String,
  sourceRefs: S.Array(S.String),
  state: ArtanisRetainedLaunchSmokeState,
  stateLabel: S.String,
  trainingLaunchAllowed: S.Boolean,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisRetainedLaunchSmokeUnsafe extends S.TaggedErrorClass<ArtanisRetainedLaunchSmokeUnsafe>()(
  'ArtanisRetainedLaunchSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_RETAINED_LAUNCH_SMOKE_READ_ONLY_AUTHORITY:
  ArtanisRetainedLaunchSmokeAuthority = {
    authorityBoundary: 'read_only_retained_production_equivalent_smoke',
    noBuyerChargeMutation: true,
    noDeployment: true,
    noForumMutation: true,
    noProviderMutation: true,
    noPylonDispatch: true,
    noSchedulerMutation: true,
    noSettlementMutation: true,
    noTrainingLaunch: true,
    noWalletSpend: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/?#=&{}-]{0,340}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|127\.0\.0\.1|192\.168\.|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|localhost|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|repo|source|trace|url|wallet)|provider[_-]?(account|credential|grant|payload|secret|telemetry|token)|raw[_-]?(artifact|auth|command|customer|d1|email|export|host|invoice|log|market|meter|payment|payload|payout|power|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(evidence\.private|operator\.private|private\.|source\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, raw D1, raw command, or raw timestamp material.`,
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

const persistedRowRefs = (
  persistedRefs: ArtanisRetainedLaunchSmokePersistedRefs,
): ReadonlyArray<string> => [
  ...persistedRefs.approvalGateRefs,
  ...persistedRefs.forumPublicationIntentRefs,
  ...persistedRefs.healthSnapshotRefs,
  ...persistedRefs.loopRecordRefs,
  ...persistedRefs.loopTickRefs,
  ...persistedRefs.runtimeSnapshotRefs,
  ...persistedRefs.workRoutingProposalRefs,
]

const assertReadOnlyAuthority = (
  authority: ArtanisRetainedLaunchSmokeAuthority,
): void => {
  if (
    authority.noBuyerChargeMutation !== true ||
    authority.noDeployment !== true ||
    authority.noForumMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPylonDispatch !== true ||
    authority.noSchedulerMutation !== true ||
    authority.noSettlementMutation !== true ||
    authority.noTrainingLaunch !== true ||
    authority.noWalletSpend !== true
  ) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason:
        'Retained Artanis launch smoke evidence is read-only and cannot mutate buyers, deployment, Forum, providers, Pylon dispatch, scheduler state, settlement, training, or wallets.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisRetainedLaunchSmokeRecord,
): void => {
  assertReadOnlyAuthority(record.authority)
  assertSafeRefs('Retained Artanis launch smoke refs', [
    record.agentRef,
    record.environment,
    record.environmentRef,
    record.forumMode,
    record.schedulerMode,
    record.smokeRunRef,
    ...record.blockerRefs,
    ...record.caveatRefs,
    ...record.deliveryReceiptRefs,
    ...record.forumPostRefs,
    ...record.noPublishProofRefs,
    ...record.operatorApprovalRefs,
    ...persistedRowRefs(record.persistedRefs),
    ...record.privateEvidenceRefs,
    ...record.publicReportRefs,
    ...record.rollbackDisableRefs,
    ...record.sourceRefs,
  ])

  if (
    containsProviderSecretMaterial(JSON.stringify(record)) ||
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
    }))
  ) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason:
        'Retained Artanis launch smoke records cannot expose provider secrets or raw timestamps outside timestamp fields.',
    })
  }

  const requiredPersistedGroups = [
    ['runtime snapshot', record.persistedRefs.runtimeSnapshotRefs],
    ['loop record', record.persistedRefs.loopRecordRefs],
    ['loop tick', record.persistedRefs.loopTickRefs],
    ['health snapshot', record.persistedRefs.healthSnapshotRefs],
    ['work-routing proposal', record.persistedRefs.workRoutingProposalRefs],
    ['Forum publication intent', record.persistedRefs.forumPublicationIntentRefs],
  ] as const

  for (const [label, refs] of requiredPersistedGroups) {
    if (!hasRefs(refs)) {
      throw new ArtanisRetainedLaunchSmokeUnsafe({
        reason: `Retained Artanis launch smoke requires persisted ${label} refs.`,
      })
    }
  }

  if (!hasRefs(record.operatorApprovalRefs)) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason: 'Retained Artanis launch smoke requires operator approval refs.',
    })
  }

  if (!record.publicReportRefs.includes('route:/api/public/artanis/report')) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason: 'Retained Artanis launch smoke requires the public Artanis report ref.',
    })
  }

  if (!hasRefs(record.rollbackDisableRefs)) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason: 'Retained Artanis launch smoke requires rollback or disable refs.',
    })
  }

  if (
    record.forumMode === 'delivered_post' &&
    (!hasRefs(record.forumPostRefs) || !hasRefs(record.deliveryReceiptRefs))
  ) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason:
        'Delivered retained Artanis launch smoke requires Forum post and delivery receipt refs.',
    })
  }

  if (
    record.forumMode === 'no_publish_test' &&
    !hasRefs(record.noPublishProofRefs)
  ) {
    throw new ArtanisRetainedLaunchSmokeUnsafe({
      reason:
        'No-publish retained Artanis launch smoke requires explicit no-publish proof refs.',
    })
  }
}

export const projectArtanisRetainedLaunchSmoke = (
  record: ArtanisRetainedLaunchSmokeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisRetainedLaunchSmokeProjection => {
  assertRecordSafe(record)

  const forumDeliveryVerified =
    record.forumMode === 'delivered_post'
      ? hasRefs(record.forumPostRefs) && hasRefs(record.deliveryReceiptRefs)
      : hasRefs(record.noPublishProofRefs)
  const state: ArtanisRetainedLaunchSmokeState =
    forumDeliveryVerified && hasRefs(record.rollbackDisableRefs)
      ? 'retained'
      : 'blocked'

  return {
    agentRef: record.agentRef,
    audience,
    authority: record.authority,
    blockerRefs: audienceSafeRefs(record.blockerRefs, audience),
    buyerChargeMutationAllowed: !record.authority.noBuyerChargeMutation,
    caveatRefs: audienceSafeRefs(record.caveatRefs, audience),
    deploymentAllowed: !record.authority.noDeployment,
    environment: record.environment,
    environmentRef: record.environmentRef,
    forumDeliveryVerified,
    forumMode: record.forumMode,
    forumMutationAllowed: !record.authority.noForumMutation,
    forumPostRefs: audienceSafeRefs(record.forumPostRefs, audience),
    noPublishProofRefs: audienceSafeRefs(record.noPublishProofRefs, audience),
    operatorApprovalRefs: audienceSafeRefs(record.operatorApprovalRefs, audience),
    persistedRowRefs: audienceSafeRefs(persistedRowRefs(record.persistedRefs), audience),
    privateEvidenceRefs: audience === 'operator' || audience === 'private'
      ? audienceSafeRefs(record.privateEvidenceRefs, audience)
      : [],
    providerMutationAllowed: !record.authority.noProviderMutation,
    publicReportRefs: audienceSafeRefs(record.publicReportRefs, audience),
    pylonDispatchAllowed: !record.authority.noPylonDispatch,
    rollbackDisableRefs: audienceSafeRefs(record.rollbackDisableRefs, audience),
    schedulerMode: record.schedulerMode,
    schedulerMutationAllowed: !record.authority.noSchedulerMutation,
    settlementMutationAllowed: !record.authority.noSettlementMutation,
    smokeRunRef: record.smokeRunRef,
    sourceRefs: audienceSafeRefs(record.sourceRefs, audience),
    state,
    stateLabel: state === 'retained'
      ? 'Retained production-equivalent smoke evidence'
      : 'Blocked before production-equivalent smoke evidence',
    trainingLaunchAllowed: !record.authority.noTrainingLaunch,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletSpendAllowed: !record.authority.noWalletSpend,
  }
}

export const artanisProductionLaunchGateCheckFromRetainedSmoke = (
  record: ArtanisRetainedLaunchSmokeRecord,
  nowIso: string,
): ArtanisProductionLaunchGateCheck => {
  const projection = projectArtanisRetainedLaunchSmoke(record, 'public', nowIso)

  return {
    category: 'production_e2e_smoke',
    checkRef: 'check.public.artanis.launch_gate.production_e2e_smoke.retained',
    description:
      'Production-equivalent Artanis launch smoke evidence has been retained without granting dispatch, payment, scheduler, training, provider, deployment, or settlement authority.',
    issueRefs: ['issue:#397', 'issue:#414', 'issue:#417'],
    requiredForAutonomousClaim: true,
    routeRefs: projection.publicReportRefs,
    status: projection.state === 'retained' ? 'passed' : 'blocked',
    testRefs: [
      'test:workers/api/src/artanis-retained-launch-smoke.test.ts',
      'test:workers/api/src/artanis-launch-smoke.test.ts',
    ],
  }
}

export const exampleArtanisRetainedLaunchSmokeRecord = (
  nowIso = '2026-06-07T02:00:00.000Z',
): ArtanisRetainedLaunchSmokeRecord => ({
  agentRef: 'agent.public.artanis',
  authority: ARTANIS_RETAINED_LAUNCH_SMOKE_READ_ONLY_AUTHORITY,
  blockerRefs: [
    'blocker.public.artanis.scheduler_still_operator_controlled',
    'blocker.public.artanis.no_live_pylon_dispatch_in_smoke',
  ],
  caveatRefs: [
    'caveat.public.production_equivalent_smoke_not_continuous_autonomy',
    'caveat.public.fake_provider_mode_has_no_live_dispatch',
  ],
  deliveryReceiptRefs: [
    'receipt.public.artanis.forum_delivery.status_post_retained',
  ],
  environment: 'staging_equivalent',
  environmentRef: 'env.public.artanis.staging_equivalent_bindings',
  forumMode: 'delivered_post',
  forumPostRefs: ['post.public.forum.artanis.status.retained_smoke'],
  noPublishProofRefs: [],
  operatorApprovalRefs: ['approval.public.artanis.production_smoke.operator_window'],
  persistedRefs: {
    approvalGateRefs: ['gate.public.artanis.production_smoke.dispatch_blocked'],
    forumPublicationIntentRefs: [
      'forum.public.artanis.status_intent.retained_smoke',
    ],
    healthSnapshotRefs: ['health.public.artanis.snapshot.retained_smoke'],
    loopRecordRefs: ['loop.public.artanis.retained_smoke'],
    loopTickRefs: ['tick.public.artanis.retained_smoke'],
    runtimeSnapshotRefs: ['runtime.public.artanis.snapshot.retained_smoke'],
    workRoutingProposalRefs: [
      'proposal.public.artanis.work_routing.retained_smoke',
    ],
  },
  privateEvidenceRefs: ['evidence.private.operator.artanis.retained_smoke_log'],
  publicReportRefs: [
    'route:/api/public/artanis/report',
    'https://openagents.com/artanis',
  ],
  rollbackDisableRefs: [
    'rollback.public.artanis.scheduler_disabled_after_smoke',
  ],
  schedulerMode: 'fake_provider_one_tick',
  smokeRunRef: 'smoke.public.artanis.production_equivalent.retained_1',
  sourceRefs: [
    'docs/artanis/2026-06-06-end-to-end-launch-smoke.md',
    'docs/artanis/2026-06-06-production-launch-gate-runbook.md',
  ],
  updatedAtIso: nowIso,
})
