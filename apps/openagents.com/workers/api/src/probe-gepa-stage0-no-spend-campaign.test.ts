import { describe, expect, test } from 'vitest'

import {
  ProbeGepaStage0NoSpendCampaignUnsafe,
  probeGepaStage0NoSpendCampaignHasPrivateMaterial,
  projectProbeGepaStage0NoSpendCampaign,
} from './probe-gepa-stage0-no-spend-campaign'
import {
  acceptPylonGepaMetricCallAssignment,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  pylonGepaMetricCallCoordinatorImport,
  reportPylonGepaMetricCallProgress,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'

const nowIso = '2026-06-08T12:00:00.000Z'

const assignmentInput = (taskRef: string, workerOrdinal: string) =>
  ({
    assignmentRef: `assignment.public.probe_gepa_stage0.${workerOrdinal}`,
    backendProfileRef: 'backend_profile.probe.apple_fm.local.v1',
    benchmarkSuiteRef: 'benchmark_suite.terminal_bench_2.harbor.retained.v1',
    campaignId: 'campaign.probe_gepa.stage0.no_spend_multi_pylon',
    candidateHash:
      'sha256:a2a44c21a08fcba12108786821dc5045a746e72b0d5a7f45374b08f8ba6a6743',
    closeoutRequirementRefs: ['closeout_requirement.probe_gepa_stage0.v1'],
    expectedArtifactRefs: ['artifact_manifest.expected.probe_gepa_stage0.v1'],
    expectedProofBundleRefs: ['proof_bundle.expected.probe_gepa_stage0.v1'],
    paymentMode: 'unpaid_smoke',
    probeCommit: 'probe_commit.ebe108d',
    runtimeRef: 'runtime.probe.benchmark_cloud.v1',
    scorerRef: 'scorer.terminal_bench.binary.v1',
    splitRef: 'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0.v1',
    taskRef,
    timeoutBudgetRef: 'timeout_budget.probe.stage0.no_spend.v1',
    verifierRef: `verifier.terminal_bench.${workerOrdinal}.v1`,
  }) as const

const acceptedImport = () => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput('task.terminal_bench.configure_git_webserver.v1', 'one'),
    nowIso,
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: 'lease.public.probe_gepa_stage0.one',
    nowIso: '2026-06-08T12:01:00.000Z',
    workerRef: 'pylon.public.stage0.one',
  })
  const progressed = reportPylonGepaMetricCallProgress(accepted, {
    nowIso: '2026-06-08T12:02:00.000Z',
    progressRefs: ['progress.public.probe_gepa_stage0.one.started'],
  })
  const submitted = submitPylonGepaMetricCallResultRefs(progressed, {
    artifactRefs: ['artifact.public.probe_gepa_stage0.one'],
    closeoutResultRefs: ['closeout.public.probe_gepa_stage0.one.accepted'],
    nowIso: '2026-06-08T12:03:00.000Z',
    proofBundleRefs: ['proof.public.probe_gepa_stage0.one'],
    resourceUsageRefs: ['resource.public.probe_gepa_stage0.one'],
    verifierResultRefs: ['verifier.public.probe_gepa_stage0.one.accepted'],
  })
  const closed = closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: 'accepted',
    nowIso: '2026-06-08T12:04:00.000Z',
  })

  return pylonGepaMetricCallCoordinatorImport(closed)
}

const rejectedImport = () => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput('task.terminal_bench.db_wal_recovery.v1', 'two'),
    nowIso,
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: 'lease.public.probe_gepa_stage0.two',
    nowIso: '2026-06-08T12:01:00.000Z',
    workerRef: 'pylon.public.stage0.two',
  })
  const closed = closePylonGepaMetricCallAssignment(accepted, {
    closeoutDecision: 'rejected',
    closeoutResultRefs: ['closeout.public.probe_gepa_stage0.two.rejected'],
    nowIso: '2026-06-08T12:04:00.000Z',
  })

  return pylonGepaMetricCallCoordinatorImport(closed)
}

const stage0Input = () => ({
  artanisSummaryRefs: ['summary.public.artanis.probe_gepa_stage0.no_spend'],
  campaignRef: 'campaign.probe_gepa.stage0.no_spend_multi_pylon',
  coordinatorImports: [acceptedImport(), rejectedImport()],
  probeCloseoutImportRefs: ['probe_import.public.probe_gepa_stage0.closeouts'],
  psionicImportDryRunRefs: ['psionic_import.public.probe_gepa_stage0.dry_run'],
})

describe('Probe GEPA Stage 0 no-spend campaign gate', () => {
  test('marks dashboard green for public-safe multi-Pylon accepted and rejected closeouts', () => {
    const projection = projectProbeGepaStage0NoSpendCampaign(stage0Input())

    expect(projection).toMatchObject({
      acceptedCount: 1,
      dashboardState: 'green',
      modelTrainingClaimAllowed: false,
      noSpendCampaignClaimAllowed: true,
      paidCampaignClaimAllowed: false,
      paidModesBlocked: true,
      rejectedCount: 1,
      runtimeCandidateActivationAllowed: false,
      settlementClaimAllowed: false,
      state: 'green',
      terminalBenchScoreClaimAllowed: false,
    })
    expect(projection.pylonRefs).toEqual([
      'pylon.public.stage0.one',
      'pylon.public.stage0.two',
    ])
    expect(projection.publicSafeBundleRefs).toContain(
      'assignment.public.probe_gepa_stage0.one',
    )
    expect(projection.publicSafeBundleRefs).toContain(
      'artifact.public.probe_gepa_stage0.one',
    )
    expect(projection.publicSafeBundleRefs).toContain(
      'proof.public.probe_gepa_stage0.one',
    )
    expect(projection.publicSafeBundleRefs).toContain(
      'probe_import.public.probe_gepa_stage0.closeouts',
    )
    expect(projection.publicSafeBundleRefs).toContain(
      'psionic_import.public.probe_gepa_stage0.dry_run',
    )
    expect(projection.publicSafeBundleRefs).toContain(
      'summary.public.artanis.probe_gepa_stage0.no_spend',
    )
    expect(probeGepaStage0NoSpendCampaignHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('keeps Stage 0 blocked without rejected closeout or import evidence', () => {
    const projection = projectProbeGepaStage0NoSpendCampaign({
      artanisSummaryRefs: ['summary.public.artanis.probe_gepa_stage0.no_spend'],
      campaignRef: 'campaign.probe_gepa.stage0.no_spend_multi_pylon',
      coordinatorImports: [acceptedImport()],
    })

    expect(projection).toMatchObject({
      dashboardState: 'blocked',
      paidModesBlocked: true,
      settlementClaimAllowed: false,
      state: 'blocked',
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.probe_gepa_stage0.multiple_pylons_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public.probe_gepa_stage0.rejected_closeout_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public.probe_gepa_stage0.probe_closeout_import_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public.probe_gepa_stage0.psionic_import_dry_run_missing',
    )
  })

  test('rejects paid or settlement imports in Stage 0 no-spend campaigns', () => {
    const paidImport = {
      ...acceptedImport(),
      paymentMode: 'payable_pending_settlement' as const,
      payableWorkClaimAllowed: true,
      paymentReceiptRefs: ['payment.public.probe_gepa_stage0.one'],
    }

    expect(() =>
      projectProbeGepaStage0NoSpendCampaign({
        ...stage0Input(),
        coordinatorImports: [paidImport, rejectedImport()],
      }),
    ).toThrow(ProbeGepaStage0NoSpendCampaignUnsafe)
  })

  test('rejects unsafe refs, raw benchmark data, model-training copy, wallet material, and timestamps', () => {
    const unsafeInputs = [
      {
        artanisSummaryRefs: ['raw_benchmark_fixture.hidden'],
      },
      {
        artanisSummaryRefs: ['terminal_bench_score.public.claim'],
      },
      {
        artanisSummaryRefs: ['model_weights.raw'],
      },
      {
        psionicImportDryRunRefs: ['wallet.private.probe_gepa_stage0'],
      },
      {
        psionicImportDryRunRefs: ['2026-06-08T12:00:00Z'],
      },
    ]

    unsafeInputs.forEach(input => {
      expect(() =>
        projectProbeGepaStage0NoSpendCampaign({
          ...stage0Input(),
          ...input,
        }),
      ).toThrow(ProbeGepaStage0NoSpendCampaignUnsafe)
    })
  })
})
