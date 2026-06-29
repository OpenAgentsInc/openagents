import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisGepaProductionSmokeSchemaVersion =
  'omega.artanis_gepa_production_smoke.v1'

export const ArtanisGepaProductionSmokeState = S.Literals([
  'blocked',
  'retained',
])
export type ArtanisGepaProductionSmokeState =
  typeof ArtanisGepaProductionSmokeState.Type

export const ArtanisGepaProductionSmokeCloseoutState = S.Literals([
  'accepted_work',
  'rejected',
])
export type ArtanisGepaProductionSmokeCloseoutState =
  typeof ArtanisGepaProductionSmokeCloseoutState.Type

export const ArtanisGepaProductionSmokePaymentMode = S.Literals([
  'unpaid_smoke',
])
export type ArtanisGepaProductionSmokePaymentMode =
  typeof ArtanisGepaProductionSmokePaymentMode.Type

export class ArtanisGepaProductionSmokeAuthority extends S.Class<ArtanisGepaProductionSmokeAuthority>(
  'ArtanisGepaProductionSmokeAuthority',
)({
  noAutomaticPromotion: S.Boolean,
  noForumAutoPost: S.Boolean,
  noModelTraining: S.Boolean,
  noPayoutClaim: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicBenchmarkScoreClaim: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export class ArtanisGepaProductionSmokePylonCloseout extends S.Class<ArtanisGepaProductionSmokePylonCloseout>(
  'ArtanisGepaProductionSmokePylonCloseout',
)({
  acceptedWorkRefs: S.Array(S.String),
  artifactManifestRefs: S.Array(S.String),
  assignmentRef: S.String,
  closeoutRefs: S.Array(S.String),
  eventRefs: S.Array(S.String),
  paymentMode: ArtanisGepaProductionSmokePaymentMode,
  proofBundleRefs: S.Array(S.String),
  pylonRef: S.String,
  rejectionRefs: S.Array(S.String),
  resourceReceiptRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  state: ArtanisGepaProductionSmokeCloseoutState,
  verifierRefs: S.Array(S.String),
}) {}

export class ArtanisGepaProductionSmokeRecord extends S.Class<ArtanisGepaProductionSmokeRecord>(
  'ArtanisGepaProductionSmokeRecord',
)({
  acceptedCloseoutCount: S.Number,
  artifactManifestRefs: S.Array(S.String),
  authority: ArtanisGepaProductionSmokeAuthority,
  benchmarkSuiteRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  campaignRef: S.String,
  candidateHashRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutBundleRefs: S.Array(S.String),
  completedMetricCalls: S.Number,
  forumSummaryRefs: S.Array(S.String),
  harborRunRefs: S.Array(S.String),
  policyFindingRefs: S.Array(S.String),
  probeCloseoutRefs: S.Array(S.String),
  probeCommitRefs: S.Array(S.String),
  psionicImportRefs: S.Array(S.String),
  pylonCloseouts: S.Array(ArtanisGepaProductionSmokePylonCloseout),
  rejectedCloseoutCount: S.Number,
  retainedResultRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  schemaVersion: S.Literal(ArtanisGepaProductionSmokeSchemaVersion),
  shcRunRefs: S.Array(S.String),
  smokeRef: S.String,
  splitManifestRefs: S.Array(S.String),
  stageRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisGepaProductionSmokeProjection extends S.Class<ArtanisGepaProductionSmokeProjection>(
  'ArtanisGepaProductionSmokeProjection',
)({
  acceptedCloseoutCount: S.Number,
  artifactManifestRefs: S.Array(S.String),
  benchmarkSuiteRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  campaignRef: S.String,
  candidateHashRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutBundleRefs: S.Array(S.String),
  completedMetricCalls: S.Number,
  forumSummaryRefs: S.Array(S.String),
  harborRunRefs: S.Array(S.String),
  mutationAuthorityAllowed: S.Boolean,
  payoutClaimAllowed: S.Boolean,
  policyFindingRefs: S.Array(S.String),
  probeCloseoutRefs: S.Array(S.String),
  probeCommitRefs: S.Array(S.String),
  psionicImportRefs: S.Array(S.String),
  pylonAssignmentRefs: S.Array(S.String),
  pylonRefs: S.Array(S.String),
  rejectedCloseoutCount: S.Number,
  retainedResultRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  schemaVersion: S.Literal(ArtanisGepaProductionSmokeSchemaVersion),
  shcRunRefs: S.Array(S.String),
  smokeRef: S.String,
  splitManifestRefs: S.Array(S.String),
  state: ArtanisGepaProductionSmokeState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisGepaProductionSmokeUnsafe extends S.TaggedErrorClass<ArtanisGepaProductionSmokeUnsafe>()(
  'ArtanisGepaProductionSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

type ProductionLaunchGateCheckInput = Readonly<{
  category: 'production_e2e_smoke'
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
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|phone|value)|email[_-]?(address|body)|fixture[_-]?body|full[_-]?(prompt|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|fixture|invoice|log|payment|payload|prompt|provider|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
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
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        `${label} contains private material, raw benchmark data, raw traces, provider credentials, wallet material, payment secrets, local paths, raw logs, or raw timestamps.`,
    })
  }
}

const assertNoRiskyAuthority = (
  authority: ArtanisGepaProductionSmokeAuthority,
): void => {
  if (
    authority.noAutomaticPromotion !== true ||
    authority.noForumAutoPost !== true ||
    authority.noModelTraining !== true ||
    authority.noPayoutClaim !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicBenchmarkScoreClaim !== true ||
    authority.noSettlementMutation !== true ||
    authority.noWalletSpend !== true
  ) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        'Probe GEPA production smoke evidence cannot promote candidates, auto-post as Artanis, train models, claim payouts, mutate providers, claim public benchmark scores, mutate settlement, or spend wallet funds.',
    })
  }
}

const assertCloseout = (
  closeout: ArtanisGepaProductionSmokePylonCloseout,
): void => {
  assertSafeRefs('Probe GEPA Pylon closeout refs', [
    closeout.assignmentRef,
    closeout.paymentMode,
    closeout.pylonRef,
    closeout.state,
    ...closeout.acceptedWorkRefs,
    ...closeout.artifactManifestRefs,
    ...closeout.closeoutRefs,
    ...closeout.eventRefs,
    ...closeout.proofBundleRefs,
    ...closeout.rejectionRefs,
    ...closeout.resourceReceiptRefs,
    ...closeout.routeScorecardRefs,
    ...closeout.verifierRefs,
  ])

  if (closeout.state === 'accepted_work') {
    if (
      closeout.acceptedWorkRefs.length === 0 ||
      closeout.artifactManifestRefs.length === 0 ||
      closeout.proofBundleRefs.length === 0 ||
      closeout.resourceReceiptRefs.length === 0 ||
      closeout.verifierRefs.length === 0
    ) {
      throw new ArtanisGepaProductionSmokeUnsafe({
        reason:
          'Accepted Probe GEPA Pylon closeouts require accepted-work, artifact, proof, resource, and verifier refs.',
      })
    }
  }

  if (closeout.state === 'rejected' && closeout.rejectionRefs.length === 0) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason: 'Rejected Probe GEPA Pylon closeouts require rejection refs.',
    })
  }
}

const assertRecord = (record: ArtanisGepaProductionSmokeRecord): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason: 'Probe GEPA production smoke updatedAtIso must be a valid ISO timestamp.',
    })
  }

  assertNoRiskyAuthority(record.authority)
  record.pylonCloseouts.forEach(assertCloseout)

  assertSafeRefs('Probe GEPA production smoke refs', [
    record.campaignRef,
    record.schemaVersion,
    record.smokeRef,
    record.stageRef,
    ...record.artifactManifestRefs,
    ...record.benchmarkSuiteRefs,
    ...record.blockerRefs,
    ...record.candidateHashRefs,
    ...record.caveatRefs,
    ...record.closeoutBundleRefs,
    ...record.forumSummaryRefs,
    ...record.harborRunRefs,
    ...record.policyFindingRefs,
    ...record.probeCloseoutRefs,
    ...record.probeCommitRefs,
    ...record.psionicImportRefs,
    ...record.retainedResultRefs,
    ...record.routeScorecardRefs,
    ...record.shcRunRefs,
    ...record.splitManifestRefs,
  ])

  if (
    containsProviderSecretMaterial(JSON.stringify(record)) ||
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
    }))
  ) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        'Probe GEPA production smoke records cannot expose provider secrets or raw timestamps outside timestamp fields.',
    })
  }

  if (
    record.completedMetricCalls < 2 ||
    record.pylonCloseouts.length < 2 ||
    record.shcRunRefs.length === 0 ||
    record.harborRunRefs.length === 0 ||
    record.probeCloseoutRefs.length === 0 ||
    record.closeoutBundleRefs.length === 0 ||
    record.retainedResultRefs.length === 0 ||
    record.routeScorecardRefs.length === 0 ||
    record.forumSummaryRefs.length === 0 ||
    record.psionicImportRefs.length === 0
  ) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        'Probe GEPA production smoke requires SHC, Harbor, Probe closeout, closeout bundle, retained result, route scorecard, Forum summary, Psionic import, and at least two Pylon closeout refs.',
    })
  }

  if (
    record.acceptedCloseoutCount !== record.pylonCloseouts.filter(
      closeout => closeout.state === 'accepted_work',
    ).length ||
    record.rejectedCloseoutCount !== record.pylonCloseouts.filter(
      closeout => closeout.state === 'rejected',
    ).length
  ) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        'Probe GEPA production smoke closeout counts must match retained Pylon closeout states.',
    })
  }

  if (
    record.acceptedCloseoutCount < 1 ||
    record.rejectedCloseoutCount < 1
  ) {
    throw new ArtanisGepaProductionSmokeUnsafe({
      reason:
        'Probe GEPA production smoke needs both accepted and rejected Pylon closeout evidence.',
    })
  }
}

export const projectArtanisGepaProductionSmoke = (
  record: ArtanisGepaProductionSmokeRecord,
  nowIso: string,
): ArtanisGepaProductionSmokeProjection => {
  assertRecord(record)

  const pylonAssignmentRefs = uniqueRefs(record.pylonCloseouts.map(
    closeout => closeout.assignmentRef,
  ))
  const pylonRefs = uniqueRefs(record.pylonCloseouts.map(
    closeout => closeout.pylonRef,
  ))
  const state: ArtanisGepaProductionSmokeState =
    record.blockerRefs.length === 0 ? 'retained' : 'blocked'

  return new ArtanisGepaProductionSmokeProjection({
    acceptedCloseoutCount: record.acceptedCloseoutCount,
    artifactManifestRefs: uniqueRefs(record.artifactManifestRefs),
    benchmarkSuiteRefs: uniqueRefs(record.benchmarkSuiteRefs),
    blockerRefs: uniqueRefs(record.blockerRefs),
    campaignRef: record.campaignRef,
    candidateHashRefs: uniqueRefs(record.candidateHashRefs),
    caveatRefs: uniqueRefs(record.caveatRefs),
    closeoutBundleRefs: uniqueRefs(record.closeoutBundleRefs),
    completedMetricCalls: record.completedMetricCalls,
    forumSummaryRefs: uniqueRefs(record.forumSummaryRefs),
    harborRunRefs: uniqueRefs(record.harborRunRefs),
    mutationAuthorityAllowed: false,
    payoutClaimAllowed: false,
    policyFindingRefs: uniqueRefs(record.policyFindingRefs),
    probeCloseoutRefs: uniqueRefs(record.probeCloseoutRefs),
    probeCommitRefs: uniqueRefs(record.probeCommitRefs),
    psionicImportRefs: uniqueRefs(record.psionicImportRefs),
    pylonAssignmentRefs,
    pylonRefs,
    rejectedCloseoutCount: record.rejectedCloseoutCount,
    retainedResultRefs: uniqueRefs(record.retainedResultRefs),
    routeScorecardRefs: uniqueRefs(record.routeScorecardRefs),
    schemaVersion: ArtanisGepaProductionSmokeSchemaVersion,
    shcRunRefs: uniqueRefs(record.shcRunRefs),
    smokeRef: record.smokeRef,
    splitManifestRefs: uniqueRefs(record.splitManifestRefs),
    state,
    stateLabel: state === 'retained'
      ? 'Retained Probe GEPA Pylon production-equivalent smoke'
      : 'Blocked before retained Probe GEPA Pylon production-equivalent smoke',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  })
}

export const artanisProductionLaunchGateCheckInputFromGepaSmoke = (
  record: ArtanisGepaProductionSmokeRecord,
  nowIso: string,
): ProductionLaunchGateCheckInput => {
  const projection = projectArtanisGepaProductionSmoke(record, nowIso)

  return {
    category: 'production_e2e_smoke',
    checkRef: 'check.public.artanis.launch_gate.probe_gepa_pylon_smoke',
    description:
      'Retained Probe GEPA/Pylon production-equivalent smoke evidence exists with SHC Harbor refs, Probe closeouts, accepted and rejected Pylon closeouts, no-spend payment mode, Psionic import refs, and public-safe Forum summary refs.',
    issueRefs: [
      'issue:#511',
      'issue:OpenAgentsInc/probe#188',
      'issue:OpenAgentsInc/openagents#4563',
      'issue:OpenAgentsInc/psionic#1093',
    ],
    requiredForAutonomousClaim: true,
    routeRefs: [
      'route:/api/public/artanis/report',
      'route:/api/pylons/{pylonRef}/assignments',
      'route:/api/operator/pylons/assignments/{assignmentRef}/closeout',
      ...projection.forumSummaryRefs,
      ...projection.pylonAssignmentRefs,
      ...projection.retainedResultRefs,
    ],
    status: projection.state === 'retained' ? 'passed' : 'blocked',
    testRefs: [
      'test:workers/api/src/artanis-gepa-production-smoke.test.ts',
      'test:workers/api/src/probe-gepa-campaign-projection.test.ts',
      'test:workers/api/src/pylon-api-routes.test.ts',
    ],
  }
}

export const exampleArtanisGepaProductionSmokeRecord = (
  updatedAtIso = '2026-06-08T05:45:00.000Z',
): ArtanisGepaProductionSmokeRecord =>
  new ArtanisGepaProductionSmokeRecord({
    acceptedCloseoutCount: 1,
    artifactManifestRefs: [
      'artifact_manifest.probe_gepa.live_stage0.configure_git_webserver.001',
      'artifact_manifest.probe_gepa.live_stage0.pypi_server.001',
    ],
    authority: {
      noAutomaticPromotion: true,
      noForumAutoPost: true,
      noModelTraining: true,
      noPayoutClaim: true,
      noProviderMutation: true,
      noPublicBenchmarkScoreClaim: true,
      noSettlementMutation: true,
      noWalletSpend: true,
    },
    benchmarkSuiteRefs: ['benchmark_suite.terminal_bench_2.harbor.v1'],
    blockerRefs: [],
    campaignRef: 'campaign.probe_gepa.stage0.live_shc_harbor_smoke.2026_06_08',
    candidateHashRefs: [
      'sha256:0000000000000000000000000000000000000000000000000000000000001880',
    ],
    caveatRefs: [
      'caveat.public.retained_smoke_not_public_score',
      'caveat.public.pylon_work_unpaid_smoke_no_settlement_claim',
      'caveat.public.gepa_text_optimization_not_model_training',
    ],
    closeoutBundleRefs: [
      'probe_closeout_bundle.probe_gepa.live_stage0.configure_git_webserver.001',
      'probe_closeout_bundle.probe_gepa.live_stage0.pypi_server.001',
    ],
    completedMetricCalls: 2,
    forumSummaryRefs: [
      'forum_summary.probe_gepa.live_stage0.retained_smoke.public_safe',
    ],
    harborRunRefs: [
      'harbor_run.terminal_bench_2.probe_gepa.live_stage0.configure_git_webserver.001',
    ],
    policyFindingRefs: [
      'policy_finding.probe_gepa.no_public_score_claim',
      'policy_finding.probe_gepa.no_model_training',
      'policy_finding.probe_gepa.no_payout_claim',
    ],
    probeCloseoutRefs: [
      'probe_closeout.live_stage0.configure_git_webserver.001',
      'probe_closeout.live_stage0.pypi_server.001',
    ],
    probeCommitRefs: ['probe_commit.main.live_stage0_smoke'],
    psionicImportRefs: ['psionic_import.gepa.live_stage0.omega_pylon_closeouts.001'],
    pylonCloseouts: [
      {
        acceptedWorkRefs: ['accepted_work.probe_gepa.live_stage0.pylon_demo_1'],
        artifactManifestRefs: [
          'artifact_manifest.probe_gepa.live_stage0.configure_git_webserver.001',
        ],
        assignmentRef: 'assignment.public.pylon_gepa.live_stage0.demo_1',
        closeoutRefs: ['closeout.probe_gepa.live_stage0.demo_1.accepted'],
        eventRefs: [
          'pylon_event.assignment_acceptance.live_stage0.demo_1',
          'pylon_event.assignment_progress.live_stage0.demo_1',
          'pylon_event.artifact_proof_metadata.live_stage0.demo_1',
        ],
        paymentMode: 'unpaid_smoke',
        proofBundleRefs: ['proof_bundle.probe_gepa.live_stage0.demo_1'],
        pylonRef: 'pylon.demo.stage0.one',
        rejectionRefs: [],
        resourceReceiptRefs: [
          'resource_usage.probe_gepa.live_stage0.demo_1',
        ],
        routeScorecardRefs: [
          'route_scorecard.probe_gepa.live_stage0.demo_1',
        ],
        state: 'accepted_work',
        verifierRefs: ['verifier.terminal_bench_2.harbor.configure_git_webserver'],
      },
      {
        acceptedWorkRefs: [],
        artifactManifestRefs: [
          'artifact_manifest.probe_gepa.live_stage0.pypi_server.001',
        ],
        assignmentRef: 'assignment.public.pylon_gepa.live_stage0.demo_2',
        closeoutRefs: ['closeout.probe_gepa.live_stage0.demo_2.rejected'],
        eventRefs: [
          'pylon_event.assignment_acceptance.live_stage0.demo_2',
          'pylon_event.assignment_progress.live_stage0.demo_2',
          'pylon_event.artifact_proof_metadata.live_stage0.demo_2',
        ],
        paymentMode: 'unpaid_smoke',
        proofBundleRefs: ['proof_bundle.probe_gepa.live_stage0.demo_2'],
        pylonRef: 'pylon.demo.stage0.two',
        rejectionRefs: ['rejection.probe_gepa.live_stage0.verifier_failed'],
        resourceReceiptRefs: [
          'resource_usage.probe_gepa.live_stage0.demo_2',
        ],
        routeScorecardRefs: [
          'route_scorecard.probe_gepa.live_stage0.demo_2',
        ],
        state: 'rejected',
        verifierRefs: ['verifier.terminal_bench_2.harbor.pypi_server'],
      },
    ],
    rejectedCloseoutCount: 1,
    retainedResultRefs: ['benchmark_result.probe_gepa.live_stage0.retained.001'],
    routeScorecardRefs: [
      'route_scorecard.probe_gepa.live_stage0.demo_1',
      'route_scorecard.probe_gepa.live_stage0.demo_2',
    ],
    schemaVersion: ArtanisGepaProductionSmokeSchemaVersion,
    shcRunRefs: ['shc_run.probe_gepa.live_stage0.harbor_smoke.001'],
    smokeRef: 'smoke.public.artanis.probe_gepa_pylon.live_stage0.001',
    splitManifestRefs: [
      'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1',
    ],
    stageRef: 'stage.probe_gepa.stage0.live_shc_harbor_smoke',
    updatedAtIso,
  })
