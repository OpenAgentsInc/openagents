import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisProductionReadinessStage = S.Literals([
  'deployed_parity_ready',
  'persistence_ready',
  'release_ready',
  'scheduler_ready',
  'smoke_ready',
  'source_ready',
])
export type ArtanisProductionReadinessStage =
  typeof ArtanisProductionReadinessStage.Type

export const ArtanisProductionReadinessCheckKind = S.Literals([
  'artanis_page',
  'd1_persistence',
  'forum_status_topic',
  'production_e2e_smoke',
  'public_report_fields',
  'pylon_stats',
  'pylon_v02_release',
  'scheduled_runner_state',
  'source_commit',
])
export type ArtanisProductionReadinessCheckKind =
  typeof ArtanisProductionReadinessCheckKind.Type

export const ArtanisProductionReadinessCheckStatus = S.Literals([
  'blocked',
  'passed',
  'stale',
  'unavailable',
])
export type ArtanisProductionReadinessCheckStatus =
  typeof ArtanisProductionReadinessCheckStatus.Type

export const ArtanisProductionReadinessAuthorityBoundary = S.Literals([
  'read_only_production_readiness_verifier',
])
export type ArtanisProductionReadinessAuthorityBoundary =
  typeof ArtanisProductionReadinessAuthorityBoundary.Type

export class ArtanisProductionReadinessAuthority extends S.Class<ArtanisProductionReadinessAuthority>(
  'ArtanisProductionReadinessAuthority',
)({
  authorityBoundary: ArtanisProductionReadinessAuthorityBoundary,
  noD1Mutation: S.Boolean,
  noDeployment: S.Boolean,
  noForumMutation: S.Boolean,
  noGitHubReleaseMutation: S.Boolean,
  noPylonDispatch: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSchedulerMutation: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export class ArtanisProductionReadinessCheck extends S.Class<ArtanisProductionReadinessCheck>(
  'ArtanisProductionReadinessCheck',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  checkKind: ArtanisProductionReadinessCheckKind,
  checkRef: S.String,
  evidenceRefs: S.Array(S.String),
  expectedSignalRef: S.String,
  observedSignalRef: S.String,
  sourceRefs: S.Array(S.String),
  stage: ArtanisProductionReadinessStage,
  status: ArtanisProductionReadinessCheckStatus,
  updatedAtIso: S.String,
}) {}

export class ArtanisProductionReadinessVerificationRecord extends S.Class<ArtanisProductionReadinessVerificationRecord>(
  'ArtanisProductionReadinessVerificationRecord',
)({
  agentRef: S.String,
  authority: ArtanisProductionReadinessAuthority,
  caveatRefs: S.Array(S.String),
  checks: S.Array(ArtanisProductionReadinessCheck),
  environmentRef: S.String,
  privateEvidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  verifierRef: S.String,
}) {}

export class ArtanisProductionReadinessStageProjection extends S.Class<ArtanisProductionReadinessStageProjection>(
  'ArtanisProductionReadinessStageProjection',
)({
  blockerRefs: S.Array(S.String),
  checkRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  stage: ArtanisProductionReadinessStage,
  status: ArtanisProductionReadinessCheckStatus,
  statusLabel: S.String,
}) {}

export class ArtanisProductionReadinessProjection extends S.Class<ArtanisProductionReadinessProjection>(
  'ArtanisProductionReadinessProjection',
)({
  agentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisProductionReadinessAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  checkCount: S.Number,
  checkRefs: S.Array(S.String),
  d1MutationAllowed: S.Boolean,
  deployedParityReady: S.Boolean,
  deploymentAllowed: S.Boolean,
  environmentRef: S.String,
  failedRequiredCount: S.Number,
  forumMutationAllowed: S.Boolean,
  gitHubReleaseMutationAllowed: S.Boolean,
  persistenceReady: S.Boolean,
  privateEvidenceRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  pylonDispatchAllowed: S.Boolean,
  releaseReady: S.Boolean,
  schedulerMutationAllowed: S.Boolean,
  schedulerReady: S.Boolean,
  smokeReady: S.Boolean,
  sourceReady: S.Boolean,
  sourceRefs: S.Array(S.String),
  stageStatuses: S.Array(ArtanisProductionReadinessStageProjection),
  state: S.Literals(['blocked', 'ready']),
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  verifierRef: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisProductionReadinessUnsafe extends S.TaggedErrorClass<ArtanisProductionReadinessUnsafe>()(
  'ArtanisProductionReadinessUnsafe',
  {
    reason: S.String,
  },
) {}

export interface ArtanisProductionReadinessObservation {
  readonly artanisPageReachable: boolean
  readonly d1TableNames: ReadonlyArray<string> | null
  readonly latestPylonReleaseTag: string | null
  readonly productionSmokeRef: string | null
  readonly publicReportFields: ReadonlyArray<string>
  readonly pylonStatsStatus: 'fresh' | 'stale' | 'unavailable'
  readonly pylonV02ReleaseAssetCount: number
  readonly pylonV02ReleaseTag: string | null
  readonly statusTopicPostCount: number | null
  readonly scheduledRunnerEnabled: boolean | null
  readonly sourceCommitRef: string | null
}

export const ARTANIS_PRODUCTION_READINESS_READ_ONLY_AUTHORITY:
  ArtanisProductionReadinessAuthority = {
    authorityBoundary: 'read_only_production_readiness_verifier',
    noD1Mutation: true,
    noDeployment: true,
    noForumMutation: true,
    noGitHubReleaseMutation: true,
    noPylonDispatch: true,
    noPublicClaimUpgrade: true,
    noSchedulerMutation: true,
    noWalletSpend: true,
  }

const requiredStages: ReadonlyArray<ArtanisProductionReadinessStage> = [
  'deployed_parity_ready',
  'persistence_ready',
  'release_ready',
  'scheduler_ready',
  'smoke_ready',
  'source_ready',
]

const requiredCheckKinds: ReadonlyArray<ArtanisProductionReadinessCheckKind> = [
  'artanis_page',
  'd1_persistence',
  'forum_status_topic',
  'production_e2e_smoke',
  'public_report_fields',
  'pylon_stats',
  'pylon_v02_release',
  'scheduled_runner_state',
  'source_commit',
]

const expectedD1Tables = [
  'artanis_approval_gates',
  'artanis_forum_publication_intents',
  'artanis_health_snapshots',
  'artanis_loop_records',
  'artanis_loop_ticks',
  'artanis_nexus_pylon_adapter_dispatches',
  'artanis_runtime_snapshots',
  'artanis_work_routing_proposals',
] as const

const expectedPublicReportFields = [
  'autonomousLoop',
  'forumRewardSmoke',
  'healthSummary',
  'productionLaunchGate',
  'pylonLaunchCommunication',
] as const

const statusLabelByStatus:
  Readonly<Record<ArtanisProductionReadinessCheckStatus, string>> = {
    blocked: 'Blocked',
    passed: 'Passed',
    stale: 'Stale',
    unavailable: 'Unavailable',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/?#=&{}-]{0,340}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|127\.0\.0\.1|192\.168\.|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|localhost|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|repo|source|trace|url|wallet)|provider[_-]?(account|credential|grant|payload|secret|telemetry|token)|raw[_-]?(artifact|auth|command|customer|d1|email|export|host|invoice|log|market|meter|payment|payload|payout|power|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(evidence\.private|operator\.private|private\.|source\.private|url\.private|workroom\.private)/i

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
    throw new ArtanisProductionReadinessUnsafe({
      reason: `${label} contains private, secret, provider, wallet, payment, customer, private repo, raw command, raw D1, raw telemetry, or raw timestamp material.`,
    })
  }
}

const assertReadOnlyAuthority = (
  authority: ArtanisProductionReadinessAuthority,
): void => {
  if (
    authority.noD1Mutation !== true ||
    authority.noDeployment !== true ||
    authority.noForumMutation !== true ||
    authority.noGitHubReleaseMutation !== true ||
    authority.noPylonDispatch !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noSchedulerMutation !== true ||
    authority.noWalletSpend !== true
  ) {
    throw new ArtanisProductionReadinessUnsafe({
      reason:
        'Artanis production readiness verification is read-only and cannot mutate D1, deployment, Forum, GitHub releases, Pylon dispatch, scheduler state, public claims, or wallets.',
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

const checkRef = (kind: ArtanisProductionReadinessCheckKind): string =>
  `check.public.artanis.production_readiness.${kind}`

const baseCheck = (
  input: Omit<ArtanisProductionReadinessCheck, 'updatedAtIso'>,
  nowIso: string,
): ArtanisProductionReadinessCheck => ({
  ...input,
  updatedAtIso: nowIso,
})

const statusForBoolean = (
  value: boolean,
  unavailable = false,
): ArtanisProductionReadinessCheckStatus =>
  unavailable ? 'unavailable' : value ? 'passed' : 'blocked'

const missingRefs = (
  prefix: string,
  required: ReadonlyArray<string>,
  observed: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const observedSet = new Set(observed)

  return required
    .filter(item => !observedSet.has(item))
    .map(item => `${prefix}.${item}`)
}

const d1CheckFromObservation = (
  observation: ArtanisProductionReadinessObservation,
  nowIso: string,
): ArtanisProductionReadinessCheck => {
  const missing = observation.d1TableNames === null
    ? [...expectedD1Tables]
    : missingRefs(
        'missing.public.artanis.production_readiness.d1_table',
        expectedD1Tables,
        observation.d1TableNames,
      )
  const status: ArtanisProductionReadinessCheckStatus =
    observation.d1TableNames === null
      ? 'unavailable'
      : missing.length === 0
        ? 'passed'
        : 'blocked'

  return baseCheck({
    blockerRefs: missing.length > 0
      ? ['blocker.public.artanis.production_readiness.d1_persistence']
      : [],
    caveatRefs: ['caveat.public.read_only_d1_table_check'],
    checkKind: 'd1_persistence',
    checkRef: checkRef('d1_persistence'),
    evidenceRefs: observation.d1TableNames === null
      ? []
      : ['evidence.public.artanis.production_readiness.d1_table_names'],
    expectedSignalRef:
      'signal.public.artanis.production_readiness.expected_artanis_tables',
    observedSignalRef: missing.length === 0
      ? 'signal.public.artanis.production_readiness.d1_tables_present'
      : 'signal.public.artanis.production_readiness.d1_tables_missing',
    sourceRefs: ['source.public.cloudflare_d1_read_only'],
    stage: 'persistence_ready',
    status,
  }, nowIso)
}

const publicReportCheckFromObservation = (
  observation: ArtanisProductionReadinessObservation,
  nowIso: string,
): ArtanisProductionReadinessCheck => {
  const missing = missingRefs(
    'missing.public.artanis.production_readiness.public_report_field',
    expectedPublicReportFields,
    observation.publicReportFields,
  )

  return baseCheck({
    blockerRefs: missing.length > 0
      ? ['blocker.public.artanis.production_readiness.public_report_fields']
      : [],
    caveatRefs: ['caveat.public.deployed_report_must_match_source'],
    checkKind: 'public_report_fields',
    checkRef: checkRef('public_report_fields'),
    evidenceRefs: missing.length === 0
      ? ['route:/api/public/artanis/report']
      : missing,
    expectedSignalRef:
      'signal.public.artanis.production_readiness.expected_public_report_fields',
    observedSignalRef: missing.length === 0
      ? 'signal.public.artanis.production_readiness.public_report_fields_present'
      : 'signal.public.artanis.production_readiness.public_report_fields_missing',
    sourceRefs: ['route:/api/public/artanis/report'],
    stage: 'deployed_parity_ready',
    status: missing.length === 0 ? 'passed' : 'blocked',
  }, nowIso)
}

export const buildArtanisProductionReadinessVerificationRecordFromObservation = (
  observation: ArtanisProductionReadinessObservation,
  nowIso: string,
): ArtanisProductionReadinessVerificationRecord => {
  const releaseReady =
    observation.pylonV02ReleaseTag === 'pylon-v0.2.0' &&
    observation.pylonV02ReleaseAssetCount > 0
  const smokeReady = observation.productionSmokeRef !== null
  const schedulerReady =
    observation.scheduledRunnerEnabled === true && smokeReady
  const statusTopicReady =
    observation.statusTopicPostCount !== null &&
    observation.statusTopicPostCount > 0
  const sourceReady = observation.sourceCommitRef !== null

  return {
    agentRef: 'agent.public.artanis',
    authority: ARTANIS_PRODUCTION_READINESS_READ_ONLY_AUTHORITY,
    caveatRefs: [
      'caveat.public.artanis_production_readiness_read_only',
      'caveat.public.scheduler_enablement_requires_operator_window',
    ],
    checks: [
      baseCheck({
        blockerRefs: sourceReady
          ? []
          : ['blocker.public.artanis.production_readiness.source_commit_missing'],
        caveatRefs: ['caveat.public.source_commit_is_read_only_evidence'],
        checkKind: 'source_commit',
        checkRef: checkRef('source_commit'),
        evidenceRefs: observation.sourceCommitRef === null
          ? []
          : [observation.sourceCommitRef],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.source_commit_ref',
        observedSignalRef: sourceReady
          ? 'signal.public.artanis.production_readiness.source_commit_present'
          : 'signal.public.artanis.production_readiness.source_commit_missing',
        sourceRefs: ['source.public.autopilot_omega_origin_main'],
        stage: 'source_ready',
        status: statusForBoolean(sourceReady),
      }, nowIso),
      publicReportCheckFromObservation(observation, nowIso),
      baseCheck({
        blockerRefs: observation.artanisPageReachable
          ? []
          : ['blocker.public.artanis.production_readiness.artanis_page_unavailable'],
        caveatRefs: ['caveat.public.page_reachability_is_not_autonomy_proof'],
        checkKind: 'artanis_page',
        checkRef: checkRef('artanis_page'),
        evidenceRefs: observation.artanisPageReachable ? ['route:/artanis'] : [],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.artanis_page_reachable',
        observedSignalRef: observation.artanisPageReachable
          ? 'signal.public.artanis.production_readiness.artanis_page_reachable'
          : 'signal.public.artanis.production_readiness.artanis_page_unavailable',
        sourceRefs: ['route:/artanis'],
        stage: 'deployed_parity_ready',
        status: statusForBoolean(observation.artanisPageReachable),
      }, nowIso),
      d1CheckFromObservation(observation, nowIso),
      baseCheck({
        blockerRefs: statusTopicReady
          ? []
          : ['blocker.public.artanis.production_readiness.status_topic_unproven'],
        caveatRefs: ['caveat.public.status_topic_post_count_not_runner_proof'],
        checkKind: 'forum_status_topic',
        checkRef: checkRef('forum_status_topic'),
        evidenceRefs: statusTopicReady
          ? ['topic.public.forum.artanis.status']
          : [],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.status_topic_readable',
        observedSignalRef: statusTopicReady
          ? 'signal.public.artanis.production_readiness.status_topic_has_posts'
          : 'signal.public.artanis.production_readiness.status_topic_missing_posts',
        sourceRefs: ['route:/api/forum/topics/88888888-4001-4001-8001-888888888888'],
        stage: 'smoke_ready',
        status: observation.statusTopicPostCount === null
          ? 'unavailable'
          : statusForBoolean(statusTopicReady),
      }, nowIso),
      baseCheck({
        blockerRefs: observation.pylonStatsStatus === 'fresh'
          ? []
          : ['blocker.public.artanis.production_readiness.pylon_stats_unfresh'],
        caveatRefs: ['caveat.public.pylon_stats_do_not_prove_v0_2_release'],
        checkKind: 'pylon_stats',
        checkRef: checkRef('pylon_stats'),
        evidenceRefs: observation.pylonStatsStatus === 'unavailable'
          ? []
          : ['route:/api/public/pylon-stats'],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.pylon_stats_fresh',
        observedSignalRef:
          `signal.public.artanis.production_readiness.pylon_stats_${observation.pylonStatsStatus}`,
        sourceRefs: ['route:/api/public/pylon-stats'],
        stage: 'smoke_ready',
        status: observation.pylonStatsStatus === 'fresh'
          ? 'passed'
          : observation.pylonStatsStatus === 'stale'
            ? 'stale'
            : 'unavailable',
      }, nowIso),
      baseCheck({
        blockerRefs: releaseReady
          ? []
          : ['blocker.public.artanis.production_readiness.pylon_v0_2_release_not_shipped'],
        caveatRefs: [
          'caveat.public.source_level_v0_2_support_is_not_release',
        ],
        checkKind: 'pylon_v02_release',
        checkRef: checkRef('pylon_v02_release'),
        evidenceRefs: releaseReady
          ? [
              'release.public.openagents.pylon_v0_2_0',
              'asset.public.openagents.pylon_v0_2_0',
            ]
          : observation.latestPylonReleaseTag === null
            ? []
            : [`release.public.openagents.latest.${observation.latestPylonReleaseTag}`],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.pylon_v0_2_release_assets',
        observedSignalRef: releaseReady
          ? 'signal.public.artanis.production_readiness.pylon_v0_2_release_present'
          : 'signal.public.artanis.production_readiness.pylon_v0_2_release_missing',
        sourceRefs: ['source.public.github_releases.openagents'],
        stage: 'release_ready',
        status: statusForBoolean(releaseReady),
      }, nowIso),
      baseCheck({
        blockerRefs: smokeReady
          ? []
          : ['blocker.public.artanis.production_readiness.production_smoke_missing'],
        caveatRefs: ['caveat.public.production_smoke_must_be_retained'],
        checkKind: 'production_e2e_smoke',
        checkRef: checkRef('production_e2e_smoke'),
        evidenceRefs: observation.productionSmokeRef === null
          ? []
          : [observation.productionSmokeRef],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.production_equivalent_smoke',
        observedSignalRef: smokeReady
          ? 'signal.public.artanis.production_readiness.production_smoke_retained'
          : 'signal.public.artanis.production_readiness.production_smoke_missing',
        sourceRefs: ['source.public.artanis.launch_smoke'],
        stage: 'smoke_ready',
        status: statusForBoolean(smokeReady),
      }, nowIso),
      baseCheck({
        blockerRefs: schedulerReady
          ? []
          : observation.scheduledRunnerEnabled === true
            ? ['blocker.public.artanis.production_readiness.scheduler_enabled_before_smoke']
            : ['blocker.public.artanis.production_readiness.scheduler_not_enabled'],
        caveatRefs: ['caveat.public.scheduler_enablement_requires_gate_pass'],
        checkKind: 'scheduled_runner_state',
        checkRef: checkRef('scheduled_runner_state'),
        evidenceRefs: schedulerReady
          ? ['env.public.artanis_scheduled_runner_enabled_true']
          : ['env.public.artanis_scheduled_runner_enabled_false_or_unknown'],
        expectedSignalRef:
          'signal.public.artanis.production_readiness.scheduler_controlled_enablement',
        observedSignalRef: schedulerReady
          ? 'signal.public.artanis.production_readiness.scheduler_enabled_after_smoke'
          : 'signal.public.artanis.production_readiness.scheduler_not_ready',
        sourceRefs: ['env.public.ARTANIS_SCHEDULED_RUNNER_ENABLED'],
        stage: 'scheduler_ready',
        status: observation.scheduledRunnerEnabled === null
          ? 'unavailable'
          : statusForBoolean(schedulerReady),
      }, nowIso),
    ],
    environmentRef: 'env.production.openagents.worker',
    privateEvidenceRefs: [],
    sourceRefs: [
      'docs/artanis/2026-06-06-artanis-deployment-readiness-audit.md',
      'docs/artanis/2026-06-06-production-launch-gate-runbook.md',
    ],
    updatedAtIso: nowIso,
    verifierRef: 'verifier.public.artanis.production_readiness.v1',
  }
}

const assertRecordSafe = (
  record: ArtanisProductionReadinessVerificationRecord,
): void => {
  assertReadOnlyAuthority(record.authority)

  assertSafeRefs('Artanis production readiness refs', [
    record.agentRef,
    record.environmentRef,
    record.verifierRef,
    ...record.sourceRefs,
    ...record.caveatRefs,
    ...record.privateEvidenceRefs,
    ...record.checks.flatMap(check => [
      check.checkKind,
      check.checkRef,
      check.expectedSignalRef,
      check.observedSignalRef,
      check.stage,
      ...check.evidenceRefs,
      ...check.sourceRefs,
      ...check.blockerRefs,
      ...check.caveatRefs,
    ]),
  ])

  if (
    containsProviderSecretMaterial(JSON.stringify(record)) ||
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
      checks: record.checks.map(check => ({ ...check, updatedAtIso: 'redacted' })),
    }))
  ) {
    throw new ArtanisProductionReadinessUnsafe({
      reason:
        'Artanis production readiness records cannot expose provider secrets or raw timestamps outside timestamp fields.',
    })
  }

  const stages = new Set(record.checks.map(check => check.stage))
  const missingStage = requiredStages.find(stage => !stages.has(stage))

  if (missingStage !== undefined) {
    throw new ArtanisProductionReadinessUnsafe({
      reason: `Artanis production readiness missing required stage ${missingStage}.`,
    })
  }

  const kinds = new Set(record.checks.map(check => check.checkKind))
  const missingKind = requiredCheckKinds.find(kind => !kinds.has(kind))

  if (missingKind !== undefined) {
    throw new ArtanisProductionReadinessUnsafe({
      reason: `Artanis production readiness missing required check ${missingKind}.`,
    })
  }

  const scheduler = record.checks.find(check =>
    check.checkKind === 'scheduled_runner_state'
  )
  const smoke = record.checks.find(check =>
    check.checkKind === 'production_e2e_smoke'
  )

  if (scheduler?.status === 'passed' && smoke?.status !== 'passed') {
    throw new ArtanisProductionReadinessUnsafe({
      reason:
        'Scheduler readiness cannot pass before production-equivalent smoke passes.',
    })
  }
}

const stageStatus = (
  stage: ArtanisProductionReadinessStage,
  checks: ReadonlyArray<ArtanisProductionReadinessCheck>,
  audience: typeof OmniProjectionAudience.Type,
): ArtanisProductionReadinessStageProjection => {
  const stageChecks = checks.filter(check => check.stage === stage)
  const blocked = stageChecks.find(check => check.status === 'blocked')
  const unavailable = stageChecks.find(check => check.status === 'unavailable')
  const stale = stageChecks.find(check => check.status === 'stale')
  const status = blocked?.status ?? unavailable?.status ?? stale?.status ?? 'passed'

  return {
    blockerRefs: audienceSafeRefs(
      stageChecks.flatMap(check => check.blockerRefs),
      audience,
    ),
    checkRefs: uniqueRefs(stageChecks.map(check => check.checkRef)),
    evidenceRefs: audienceSafeRefs(
      stageChecks.flatMap(check => check.evidenceRefs),
      audience,
    ),
    stage,
    status,
    statusLabel: statusLabelByStatus[status],
  }
}

export const projectArtanisProductionReadinessVerification = (
  record: ArtanisProductionReadinessVerificationRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisProductionReadinessProjection => {
  assertRecordSafe(record)

  const stageStatuses = requiredStages.map(stage =>
    stageStatus(stage, record.checks, audience)
  )
  const failedRequiredCount = record.checks.filter(check =>
    check.status !== 'passed'
  ).length
  const state = failedRequiredCount === 0 ? 'ready' : 'blocked'
  const stageReady = (stage: ArtanisProductionReadinessStage): boolean =>
    stageStatuses.find(status => status.stage === stage)?.status === 'passed'

  return {
    agentRef: record.agentRef,
    audience,
    authority: record.authority,
    blockerRefs: audienceSafeRefs(
      record.checks.flatMap(check => check.blockerRefs),
      audience,
    ),
    caveatRefs: audienceSafeRefs(record.caveatRefs, audience),
    checkCount: record.checks.length,
    checkRefs: uniqueRefs(record.checks.map(check => check.checkRef)),
    d1MutationAllowed: !record.authority.noD1Mutation,
    deployedParityReady: stageReady('deployed_parity_ready'),
    deploymentAllowed: !record.authority.noDeployment,
    environmentRef: record.environmentRef,
    failedRequiredCount,
    forumMutationAllowed: !record.authority.noForumMutation,
    gitHubReleaseMutationAllowed: !record.authority.noGitHubReleaseMutation,
    persistenceReady: stageReady('persistence_ready'),
    privateEvidenceRefs: audience === 'operator' || audience === 'private'
      ? audienceSafeRefs(record.privateEvidenceRefs, audience)
      : [],
    publicClaimUpgradeAllowed: !record.authority.noPublicClaimUpgrade,
    pylonDispatchAllowed: !record.authority.noPylonDispatch,
    releaseReady: stageReady('release_ready'),
    schedulerMutationAllowed: !record.authority.noSchedulerMutation,
    schedulerReady: stageReady('scheduler_ready'),
    smokeReady: stageReady('smoke_ready'),
    sourceReady: stageReady('source_ready'),
    sourceRefs: audienceSafeRefs(record.sourceRefs, audience),
    stageStatuses,
    state,
    stateLabel: state === 'ready'
      ? 'Ready for controlled operator launch window'
      : 'Blocked before Artanis production autonomy',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verifierRef: record.verifierRef,
    walletSpendAllowed: !record.authority.noWalletSpend,
  }
}

export const exampleArtanisProductionReadinessObservation =
  (): ArtanisProductionReadinessObservation => ({
    artanisPageReachable: true,
    d1TableNames: [],
    latestPylonReleaseTag: 'pylon-v0.1.23',
    productionSmokeRef: null,
    publicReportFields: [
      'autonomousLoop',
      'healthSummary',
      'productionLaunchGate',
    ],
    pylonStatsStatus: 'fresh',
    pylonV02ReleaseAssetCount: 0,
    pylonV02ReleaseTag: null,
    scheduledRunnerEnabled: false,
    sourceCommitRef: 'commit.public.autopilot_omega.3b24bf68',
    statusTopicPostCount: 1,
  })

export const exampleArtanisProductionReadinessVerificationRecord = (
  nowIso = '2026-06-06T23:59:00.000Z',
): ArtanisProductionReadinessVerificationRecord =>
  buildArtanisProductionReadinessVerificationRecordFromObservation(
    exampleArtanisProductionReadinessObservation(),
    nowIso,
  )
