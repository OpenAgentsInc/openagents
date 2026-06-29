import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisGepaScheduledRunnerProofSchemaVersion =
  'omega.artanis_gepa_scheduled_runner_proof.v1'

export const ArtanisGepaScheduledRunnerProofState = S.Literals([
  'blocked',
  'retained',
])
export type ArtanisGepaScheduledRunnerProofState =
  typeof ArtanisGepaScheduledRunnerProofState.Type

export const ArtanisGepaScheduledRunnerCadence = S.Literals([
  'minute_cron_status_projection',
])
export type ArtanisGepaScheduledRunnerCadence =
  typeof ArtanisGepaScheduledRunnerCadence.Type

export const ArtanisGepaScheduledRunnerBudgetMode = S.Literals([
  'unpaid_smoke_no_spend',
])
export type ArtanisGepaScheduledRunnerBudgetMode =
  typeof ArtanisGepaScheduledRunnerBudgetMode.Type

export class ArtanisGepaScheduledRunnerAuthority extends S.Class<ArtanisGepaScheduledRunnerAuthority>(
  'ArtanisGepaScheduledRunnerAuthority',
)({
  assignmentDispatchAllowed: S.Boolean,
  duplicateAssignmentAllowed: S.Boolean,
  duplicateForumPostAllowed: S.Boolean,
  forumAutoPublishAllowed: S.Boolean,
  modelTrainingAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisGepaScheduledRunnerProofRecord extends S.Class<ArtanisGepaScheduledRunnerProofRecord>(
  'ArtanisGepaScheduledRunnerProofRecord',
)({
  assignmentModeRefs: S.Array(S.String),
  authority: ArtanisGepaScheduledRunnerAuthority,
  blockerRefs: S.Array(S.String),
  budgetMode: ArtanisGepaScheduledRunnerBudgetMode,
  cadence: ArtanisGepaScheduledRunnerCadence,
  closeoutReceiptRefs: S.Array(S.String),
  disableCommandRefs: S.Array(S.String),
  enabled: S.Boolean,
  enablementRefs: S.Array(S.String),
  forumCadenceRefs: S.Array(S.String),
  forumIntentRefs: S.Array(S.String),
  freshnessSignalRefs: S.Array(S.String),
  healthSnapshotRefs: S.Array(S.String),
  idempotencyRefs: S.Array(S.String),
  loopRefs: S.Array(S.String),
  noDuplicateAssignmentRefs: S.Array(S.String),
  noDuplicateForumPostRefs: S.Array(S.String),
  operatorPauseRefs: S.Array(S.String),
  productionSmokeCheckPassed: S.Boolean,
  proofRef: S.String,
  publicReportRefs: S.Array(S.String),
  pylonSelectionPolicyRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  schemaVersion: S.Literal(ArtanisGepaScheduledRunnerProofSchemaVersion),
  sourceRefs: S.Array(S.String),
  stalenessSignalRefs: S.Array(S.String),
  tickRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workerVersionRefs: S.Array(S.String),
}) {}

export class ArtanisGepaScheduledRunnerProofProjection extends S.Class<ArtanisGepaScheduledRunnerProofProjection>(
  'ArtanisGepaScheduledRunnerProofProjection',
)({
  assignmentDispatchAllowed: S.Boolean,
  assignmentModeRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetMode: ArtanisGepaScheduledRunnerBudgetMode,
  cadence: ArtanisGepaScheduledRunnerCadence,
  closeoutReceiptRefs: S.Array(S.String),
  disableCommandRefs: S.Array(S.String),
  enabled: S.Boolean,
  enablementRefs: S.Array(S.String),
  forumAutoPublishAllowed: S.Boolean,
  forumCadenceRefs: S.Array(S.String),
  forumIntentRefs: S.Array(S.String),
  freshnessSignalRefs: S.Array(S.String),
  healthSnapshotRefs: S.Array(S.String),
  idempotencyRefs: S.Array(S.String),
  loopRefs: S.Array(S.String),
  mutationAuthorityAllowed: S.Boolean,
  noDuplicateAssignmentRefs: S.Array(S.String),
  noDuplicateForumPostRefs: S.Array(S.String),
  operatorPauseRefs: S.Array(S.String),
  productionSmokeCheckPassed: S.Boolean,
  proofRef: S.String,
  publicReportRefs: S.Array(S.String),
  pylonSelectionPolicyRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  schemaVersion: S.Literal(ArtanisGepaScheduledRunnerProofSchemaVersion),
  sourceRefs: S.Array(S.String),
  stalenessSignalRefs: S.Array(S.String),
  state: ArtanisGepaScheduledRunnerProofState,
  stateLabel: S.String,
  tickRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workerVersionRefs: S.Array(S.String),
}) {}

export class ArtanisGepaScheduledRunnerProofUnsafe extends S.TaggedErrorClass<ArtanisGepaScheduledRunnerProofUnsafe>()(
  'ArtanisGepaScheduledRunnerProofUnsafe',
  {
    reason: S.String,
  },
) {}

type ProductionLaunchGateCheckInput = Readonly<{
  category: 'scheduled_runner'
  checkRef: string
  description: string
  issueRefs: ReadonlyArray<string>
  requiredForAutonomousClaim: true
  routeRefs: ReadonlyArray<string>
  status: 'blocked' | 'passed'
  testRefs: ReadonlyArray<string>
}>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|phone|value)|email[_-]?(address|body)|fixture[_-]?body|full[_-]?(prompt|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|fixture|invoice|log|payment|payload|prompt|provider|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

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
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisGepaScheduledRunnerProofUnsafe({
      reason:
        `${label} contains private runner material, raw logs, provider credentials, wallet material, payment secrets, raw benchmark payloads, local paths, or raw timestamps.`,
    })
  }
}

const assertNoRiskyAuthority = (
  authority: ArtanisGepaScheduledRunnerAuthority,
): void => {
  if (
    authority.assignmentDispatchAllowed !== false ||
    authority.duplicateAssignmentAllowed !== false ||
    authority.duplicateForumPostAllowed !== false ||
    authority.forumAutoPublishAllowed !== false ||
    authority.modelTrainingAllowed !== false ||
    authority.providerMutationAllowed !== false ||
    authority.runtimePromotionAllowed !== false ||
    authority.settlementMutationAllowed !== false ||
    authority.walletSpendAllowed !== false
  ) {
    throw new ArtanisGepaScheduledRunnerProofUnsafe({
      reason:
        'The bounded Artanis GEPA scheduled runner cannot dispatch assignments, duplicate assignments, duplicate Forum posts, auto-publish, train models, mutate providers, promote runtime, mutate settlement, or spend wallet funds.',
    })
  }
}

const assertHasRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  if (refs.length === 0) {
    throw new ArtanisGepaScheduledRunnerProofUnsafe({
      reason: `${label} are required for bounded Artanis scheduled runner proof.`,
    })
  }
}

const assertRecord = (
  record: ArtanisGepaScheduledRunnerProofRecord,
): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisGepaScheduledRunnerProofUnsafe({
      reason: 'Artanis GEPA scheduled runner proof updatedAtIso must be valid.',
    })
  }

  assertNoRiskyAuthority(record.authority)
  assertSafeRefs('Artanis GEPA scheduled runner refs', [
    record.budgetMode,
    record.cadence,
    record.proofRef,
    record.schemaVersion,
    ...record.assignmentModeRefs,
    ...record.blockerRefs,
    ...record.closeoutReceiptRefs,
    ...record.disableCommandRefs,
    ...record.enablementRefs,
    ...record.forumCadenceRefs,
    ...record.forumIntentRefs,
    ...record.freshnessSignalRefs,
    ...record.healthSnapshotRefs,
    ...record.idempotencyRefs,
    ...record.loopRefs,
    ...record.noDuplicateAssignmentRefs,
    ...record.noDuplicateForumPostRefs,
    ...record.operatorPauseRefs,
    ...record.publicReportRefs,
    ...record.pylonSelectionPolicyRefs,
    ...record.rollbackRefs,
    ...record.sourceRefs,
    ...record.stalenessSignalRefs,
    ...record.tickRefs,
    ...record.workerVersionRefs,
  ])

  if (
    containsProviderSecretMaterial(JSON.stringify(record)) ||
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
    }))
  ) {
    throw new ArtanisGepaScheduledRunnerProofUnsafe({
      reason:
        'Artanis GEPA scheduled runner proof cannot expose provider secrets or raw timestamps outside timestamp fields.',
    })
  }

  for (const [label, refs] of [
    ['assignment mode refs', record.assignmentModeRefs],
    ['closeout receipt refs', record.closeoutReceiptRefs],
    ['disable command refs', record.disableCommandRefs],
    ['enablement refs', record.enablementRefs],
    ['Forum cadence refs', record.forumCadenceRefs],
    ['Forum intent refs', record.forumIntentRefs],
    ['freshness signal refs', record.freshnessSignalRefs],
    ['health snapshot refs', record.healthSnapshotRefs],
    ['idempotency refs', record.idempotencyRefs],
    ['loop refs', record.loopRefs],
    ['no duplicate assignment refs', record.noDuplicateAssignmentRefs],
    ['no duplicate Forum post refs', record.noDuplicateForumPostRefs],
    ['operator pause refs', record.operatorPauseRefs],
    ['public report refs', record.publicReportRefs],
    ['Pylon selection policy refs', record.pylonSelectionPolicyRefs],
    ['rollback refs', record.rollbackRefs],
    ['staleness signal refs', record.stalenessSignalRefs],
    ['tick refs', record.tickRefs],
    ['worker version refs', record.workerVersionRefs],
  ] as const) {
    assertHasRefs(label, refs)
  }
}

export const projectArtanisGepaScheduledRunnerProof = (
  record: ArtanisGepaScheduledRunnerProofRecord,
  nowIso: string,
): ArtanisGepaScheduledRunnerProofProjection => {
  assertRecord(record)

  const state: ArtanisGepaScheduledRunnerProofState =
    record.enabled &&
      record.productionSmokeCheckPassed &&
      record.blockerRefs.length === 0
      ? 'retained'
      : 'blocked'

  return new ArtanisGepaScheduledRunnerProofProjection({
    assignmentDispatchAllowed: false,
    assignmentModeRefs: uniqueRefs(record.assignmentModeRefs),
    blockerRefs: uniqueRefs(record.blockerRefs),
    budgetMode: record.budgetMode,
    cadence: record.cadence,
    closeoutReceiptRefs: uniqueRefs(record.closeoutReceiptRefs),
    disableCommandRefs: uniqueRefs(record.disableCommandRefs),
    enabled: record.enabled,
    enablementRefs: uniqueRefs(record.enablementRefs),
    forumAutoPublishAllowed: false,
    forumCadenceRefs: uniqueRefs(record.forumCadenceRefs),
    forumIntentRefs: uniqueRefs(record.forumIntentRefs),
    freshnessSignalRefs: uniqueRefs(record.freshnessSignalRefs),
    healthSnapshotRefs: uniqueRefs(record.healthSnapshotRefs),
    idempotencyRefs: uniqueRefs(record.idempotencyRefs),
    loopRefs: uniqueRefs(record.loopRefs),
    mutationAuthorityAllowed: false,
    noDuplicateAssignmentRefs: uniqueRefs(record.noDuplicateAssignmentRefs),
    noDuplicateForumPostRefs: uniqueRefs(record.noDuplicateForumPostRefs),
    operatorPauseRefs: uniqueRefs(record.operatorPauseRefs),
    productionSmokeCheckPassed: record.productionSmokeCheckPassed,
    proofRef: record.proofRef,
    publicReportRefs: uniqueRefs(record.publicReportRefs),
    pylonSelectionPolicyRefs: uniqueRefs(record.pylonSelectionPolicyRefs),
    rollbackRefs: uniqueRefs(record.rollbackRefs),
    schemaVersion: ArtanisGepaScheduledRunnerProofSchemaVersion,
    sourceRefs: uniqueRefs(record.sourceRefs),
    stalenessSignalRefs: uniqueRefs(record.stalenessSignalRefs),
    state,
    stateLabel: state === 'retained'
      ? 'Retained bounded GEPA scheduled runner proof'
      : 'Blocked before bounded GEPA scheduled runner proof',
    tickRefs: uniqueRefs(record.tickRefs),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workerVersionRefs: uniqueRefs(record.workerVersionRefs),
  })
}

export const artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof = (
  record: ArtanisGepaScheduledRunnerProofRecord,
  nowIso: string,
  productionSmokeCheckPassed: boolean,
): ProductionLaunchGateCheckInput => {
  const projection = projectArtanisGepaScheduledRunnerProof(
    {
      ...record,
      productionSmokeCheckPassed:
        record.productionSmokeCheckPassed && productionSmokeCheckPassed,
    },
    nowIso,
  )

  return {
    category: 'scheduled_runner',
    checkRef: 'check.public.artanis.launch_gate.gepa_scheduled_runner',
    description:
      'Bounded Artanis scheduled-runner proof is retained for Probe GEPA status projection with no-spend budget mode, idempotent ticks, no duplicate assignments, no duplicate Forum posts, public health and staleness refs, pause and disable refs, and rollback refs.',
    issueRefs: [
      'issue:#512',
      'issue:OpenAgentsInc/probe#188',
      'issue:OpenAgentsInc/openagents#4563',
      'issue:OpenAgentsInc/psionic#1093',
    ],
    requiredForAutonomousClaim: true,
    routeRefs: [
      'worker:scheduled',
      'route:/api/public/artanis/report',
      'route:/api/operator/autopilot/goals/{goalId}/pause',
      ...projection.forumIntentRefs,
      ...projection.healthSnapshotRefs,
      ...projection.loopRefs,
      ...projection.publicReportRefs,
      ...projection.tickRefs,
    ],
    status: projection.state === 'retained' ? 'passed' : 'blocked',
    testRefs: [
      'test:workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts',
      'test:workers/api/src/artanis-scheduled-runner.test.ts',
      'test:workers/api/src/artanis-production-launch-gate.test.ts',
      'test:workers/api/src/config.test.ts',
    ],
  }
}

export const exampleArtanisGepaScheduledRunnerProofRecord = (
  updatedAtIso = '2026-06-08T06:15:00.000Z',
): ArtanisGepaScheduledRunnerProofRecord =>
  new ArtanisGepaScheduledRunnerProofRecord({
    assignmentModeRefs: [
      'assignment_mode.probe_gepa.unpaid_smoke.no_spend',
      'assignment_mode.probe_gepa.operator_reviewed_closeout',
    ],
    authority: {
      assignmentDispatchAllowed: false,
      duplicateAssignmentAllowed: false,
      duplicateForumPostAllowed: false,
      forumAutoPublishAllowed: false,
      modelTrainingAllowed: false,
      providerMutationAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      walletSpendAllowed: false,
    },
    blockerRefs: [],
    budgetMode: 'unpaid_smoke_no_spend',
    cadence: 'minute_cron_status_projection',
    closeoutReceiptRefs: [
      'receipt.public.artanis.tick_closeout.gepa_status_projection',
    ],
    disableCommandRefs: [
      'runbook.public.artanis.production_launch.disable',
    ],
    enabled: true,
    enablementRefs: [
      'env.public.artanis_scheduled_runner_enabled_true',
      'worker_version.public.artanis.090b8358_40ea_4070_9f18_53c0f9bfa21c',
    ],
    forumCadenceRefs: [
      'forum_cadence.public.artanis.gepa_status_only.operator_authority',
    ],
    forumIntentRefs: [
      'forum.public.artanis.status_intent.gepa_scheduled_runner',
    ],
    freshnessSignalRefs: [
      'signal.public.artanis.health.last_tick.fresh',
    ],
    healthSnapshotRefs: [
      'health.public.artanis.snapshot.gepa_scheduled_runner',
    ],
    idempotencyRefs: [
      'idempotency.public.artanis.scheduled_tick.schedule_ref',
      'idempotency.public.artanis.forum_intent.schedule_ref',
    ],
    loopRefs: ['loop.public.artanis.scope_public_artanis_global'],
    noDuplicateAssignmentRefs: [
      'dedupe.public.artanis.no_assignment_dispatch_in_runner',
    ],
    noDuplicateForumPostRefs: [
      'dedupe.public.artanis.forum_intent_idempotency_key',
    ],
    operatorPauseRefs: [
      'runbook.public.artanis.production_launch.pause',
    ],
    productionSmokeCheckPassed: true,
    proofRef: 'proof.public.artanis.gepa_scheduled_runner.bounded_001',
    publicReportRefs: ['route:/api/public/artanis/report'],
    pylonSelectionPolicyRefs: [
      'pylon_selection.public.probe_gepa.capability_matched_unpaid_smoke',
    ],
    rollbackRefs: [
      'rollback.public.artanis.publication_mistake',
      'rollback.public.artanis.dispatch_mistake',
      'rollback.public.artanis.public_claim_mistake',
    ],
    schemaVersion: ArtanisGepaScheduledRunnerProofSchemaVersion,
    sourceRefs: [
      'docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md',
      'docs/artanis/2026-06-06-scheduled-tick-runner.md',
    ],
    stalenessSignalRefs: [
      'signal.public.artanis.health_staleness',
    ],
    tickRefs: ['tick.public.artanis.gepa_scheduled_runner'],
    updatedAtIso,
    workerVersionRefs: [
      'worker_version.public.artanis.090b8358_40ea_4070_9f18_53c0f9bfa21c',
    ],
  })
