import { describe, expect, test } from 'vitest'

import {
  acceptPylonGepaMetricCallAssignment,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  pylonGepaMetricCallCoordinatorImport,
  reportPylonGepaMetricCallProgress,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'
import {
  ProbeGepaStage0LivePylonPreflight,
  projectProbeGepaStage0LiveCampaignSmoke,
  projectProbeGepaStage0LivePylonPreflight,
} from './probe-gepa-stage0-live-campaign-smoke'

const assignmentInput = (workerOrdinal: string) =>
  ({
    assignmentRef: `assignment.public.probe_gepa_stage0.live.${workerOrdinal}`,
    backendProfileRef: 'backend_profile.probe.benchmark_cloud.v1',
    benchmarkSuiteRef: 'benchmark_suite.terminal_bench_2.harbor.retained.v1',
    campaignId: 'campaign.public.probe_gepa_stage0.live_multi_pylon',
    candidateHash:
      workerOrdinal === 'accepted'
        ? 'sha256:1000000000000000000000000000000000000000000000000000000000000001'
        : 'sha256:1000000000000000000000000000000000000000000000000000000000000002',
    closeoutRequirementRefs: ['closeout_requirement.probe_gepa_stage0.v1'],
    expectedArtifactRefs: ['artifact_manifest.expected.probe_gepa_stage0.v1'],
    expectedProofBundleRefs: ['proof_bundle.expected.probe_gepa_stage0.v1'],
    paymentMode: 'unpaid_smoke',
    probeCommit: 'probe_commit.public.stage0_live',
    runtimeRef: 'runtime.probe.benchmark_cloud.v1',
    scorerRef: 'scorer.terminal_bench.binary.v1',
    splitRef: 'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0.v1',
    taskRef: `task.terminal_bench.retained.${workerOrdinal}.v1`,
    timeoutBudgetRef: 'timeout_budget.probe.stage0.no_spend.v1',
    verifierRef: `verifier.terminal_bench.${workerOrdinal}.v1`,
  }) as const

const acceptedImport = () => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput('accepted'),
    '2026-06-11T01:00:00.000Z',
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: 'lease.public.probe_gepa_stage0.live.accepted',
    nowIso: '2026-06-11T01:01:00.000Z',
    workerRef: 'pylon.public.stage0.real_one',
  })
  const progressed = reportPylonGepaMetricCallProgress(accepted, {
    nowIso: '2026-06-11T01:02:00.000Z',
    progressRefs: ['progress.public.probe_gepa_stage0.real_one.started'],
  })
  const submitted = submitPylonGepaMetricCallResultRefs(progressed, {
    artifactRefs: ['artifact.public.probe_gepa_stage0.real_one.bundle'],
    closeoutResultRefs: ['closeout.public.probe_gepa_stage0.real_one.accepted'],
    nowIso: '2026-06-11T01:03:00.000Z',
    proofBundleRefs: ['proof.public.probe_gepa_stage0.real_one.bundle'],
    resourceUsageRefs: ['resource.public.probe_gepa_stage0.real_one'],
    verifierResultRefs: [
      'verifier.public.probe_gepa_stage0.real_one.accepted',
    ],
  })
  const closed = closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: 'accepted',
    nowIso: '2026-06-11T01:04:00.000Z',
  })

  return pylonGepaMetricCallCoordinatorImport(closed)
}

const rejectedImport = () => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput('rejected'),
    '2026-06-11T01:00:00.000Z',
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: 'lease.public.probe_gepa_stage0.live.rejected',
    nowIso: '2026-06-11T01:01:00.000Z',
    workerRef: 'pylon.public.stage0.real_two',
  })
  const progressed = reportPylonGepaMetricCallProgress(accepted, {
    nowIso: '2026-06-11T01:02:00.000Z',
    progressRefs: ['progress.public.probe_gepa_stage0.real_two.started'],
  })
  const submitted = submitPylonGepaMetricCallResultRefs(progressed, {
    artifactRefs: ['artifact.public.probe_gepa_stage0.real_two.bundle'],
    closeoutResultRefs: ['closeout.public.probe_gepa_stage0.real_two.rejected'],
    nowIso: '2026-06-11T01:03:00.000Z',
    proofBundleRefs: ['proof.public.probe_gepa_stage0.real_two.bundle'],
    resourceUsageRefs: ['resource.public.probe_gepa_stage0.real_two'],
    verifierResultRefs: [
      'verifier.public.probe_gepa_stage0.real_two.rejected',
    ],
  })
  const closed = closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: 'rejected',
    nowIso: '2026-06-11T01:04:00.000Z',
  })

  return pylonGepaMetricCallCoordinatorImport(closed)
}

const stage0CampaignInput = () => ({
  artanisSummaryRefs: ['summary.public.artanis.probe_gepa_stage0.live'],
  campaignRef: 'campaign.public.probe_gepa_stage0.live_multi_pylon',
  coordinatorImports: [acceptedImport(), rejectedImport()],
  probeCloseoutImportRefs: ['probe_import.public.probe_gepa_stage0.live'],
  psionicImportDryRunRefs: [
    'psionic_import.public.probe_gepa_stage0.live.dry_run',
  ],
})

describe('Probe GEPA Stage 0 live campaign smoke', () => {
  test('blocks public Pylons that are synthetic or missing GEPA retained capability', () => {
    const projection = projectProbeGepaStage0LivePylonPreflight({
      candidates: [
        {
          capabilityRefs: [
            'capability.pylon.assignment_ready',
            'capability.public.pylon.nip90.text_inference.v0.3',
          ],
          displayName: 'Artanis',
          latestHeartbeatDisplay: 'Just now',
          pylonRef: 'pylon.7a41439039d360162e84',
          status: 'active',
          walletReady: true,
        },
        {
          capabilityRefs: [
            'cap.gepa.retained.v1',
            'capability.public.background_loop',
          ],
          displayName: 'Pylon live worker-loop smoke',
          latestHeartbeatDisplay: 'Just now',
          pylonRef: 'pylon.codex.live_smoke.20260611011036',
          status: 'active',
          walletReady: true,
        },
      ],
    })

    expect(projection).toMatchObject({
      candidatePylonRefs: [],
      state: 'blocked',
    })
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_stage0.live_gepa_capable_pylons_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_stage0.required_capability_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa_stage0.synthetic_pylons_selected',
    )
  })

  test('allows two fresh real GEPA-capable Pylons through preflight', () => {
    const projection = projectProbeGepaStage0LivePylonPreflight({
      candidates: [
        {
          capabilityRefs: ['cap.gepa.retained.v1'],
          displayName: 'Stage 0 Worker One',
          latestHeartbeatDisplay: 'Just now',
          pylonRef: 'pylon.public.stage0.real_one',
          status: 'active',
          walletReady: true,
        },
        {
          capabilityRefs: ['cap.gepa.retained.v1'],
          displayName: 'Stage 0 Worker Two',
          latestHeartbeatDisplay: '2 minutes ago',
          pylonRef: 'pylon.public.stage0.real_two',
          status: 'active',
          walletReady: true,
        },
      ],
    })

    expect(projection).toEqual(
      new ProbeGepaStage0LivePylonPreflight({
        blockerRefs: [],
        candidatePylonRefs: [
          'pylon.public.stage0.real_one',
          'pylon.public.stage0.real_two',
        ],
        requiredCapabilityRef: 'cap.gepa.retained.v1',
        selectedPylonRefs: [],
        state: 'green',
      }),
    )
  })

  test('requires both live preflight and retained Stage 0 campaign refs for green', () => {
    const preflight = projectProbeGepaStage0LivePylonPreflight({
      candidates: [
        {
          capabilityRefs: ['cap.gepa.retained.v1'],
          displayName: 'Stage 0 Worker One',
          latestHeartbeatDisplay: 'Just now',
          pylonRef: 'pylon.public.stage0.real_one',
          status: 'active',
          walletReady: true,
        },
        {
          capabilityRefs: ['cap.gepa.retained.v1'],
          displayName: 'Stage 0 Worker Two',
          latestHeartbeatDisplay: 'Just now',
          pylonRef: 'pylon.public.stage0.real_two',
          status: 'active',
          walletReady: true,
        },
      ],
    })
    const projection = projectProbeGepaStage0LiveCampaignSmoke({
      campaignInput: stage0CampaignInput(),
      preflight,
    })

    expect(projection).toMatchObject({
      blockerRefs: [],
      state: 'green',
    })
    expect(projection.campaign).toMatchObject({
      acceptedCount: 1,
      rejectedCount: 1,
      state: 'green',
    })
    expect(projection.campaign.settlementClaimAllowed).toBe(false)
    expect(projection.campaign.paidCampaignClaimAllowed).toBe(false)
  })

  test('keeps the smoke blocked when the live preflight is absent', () => {
    const projection = projectProbeGepaStage0LiveCampaignSmoke({
      campaignInput: {
        ...stage0CampaignInput(),
        coordinatorImports: [acceptedImport()],
      },
      preflight: null,
    })

    expect(projection.state).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.public.probe_gepa_stage0.rejected_closeout_missing',
    )
  })
})
