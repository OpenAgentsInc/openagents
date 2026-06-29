import { Schema as S } from 'effect'

import {
  PylonGepaMetricCallAssignmentRecord,
  PylonGepaMetricCallAssignmentUnsafe,
  PylonGepaMetricCallCloseoutDecision,
  PylonGepaMetricCallCoordinatorImport,
  PylonGepaMetricCallPaymentMode,
  acceptPylonGepaMetricCallAssignment,
  assertPylonGepaMetricCallPublicRefs,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  pylonGepaMetricCallCoordinatorImport,
  reportPylonGepaMetricCallProgress,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'

export const ProbeGepaUnpaidPylonLeaseProofSchemaVersion =
  'omega.probe_gepa_unpaid_pylon_lease_proof.v1'

export const ProbeGepaDemoPylonWorkerKind = S.Literals(['demo_pylon_worker'])
export type ProbeGepaDemoPylonWorkerKind =
  typeof ProbeGepaDemoPylonWorkerKind.Type

export class ProbeGepaDemoPylonWorker extends S.Class<ProbeGepaDemoPylonWorker>(
  'ProbeGepaDemoPylonWorker',
)({
  capabilityRef: S.String,
  isolationRef: S.String,
  publicCapabilityRefs: S.Array(S.String),
  workerKind: ProbeGepaDemoPylonWorkerKind,
  workerRef: S.String,
}) {}

export class ProbeGepaUnpaidPylonLeaseProof extends S.Class<ProbeGepaUnpaidPylonLeaseProof>(
  'ProbeGepaUnpaidPylonLeaseProof',
)({
  acceptedCloseoutRefs: S.Array(S.String),
  assignmentRecords: S.Array(PylonGepaMetricCallAssignmentRecord),
  assignmentRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  campaignId: S.String,
  closeoutDecisions: S.Array(PylonGepaMetricCallCloseoutDecision),
  coordinatorImportRefs: S.Array(S.String),
  coordinatorImports: S.Array(PylonGepaMetricCallCoordinatorImport),
  generatedByRef: S.String,
  noAutomaticPromotionClaim: S.Boolean,
  noPaidWorkClaim: S.Boolean,
  noSettlementClaim: S.Boolean,
  paymentModes: S.Array(PylonGepaMetricCallPaymentMode),
  progressRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  proofRef: S.String,
  publicSummaryLabel: S.String,
  rejectedCloseoutRefs: S.Array(S.String),
  resourceUsageRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaUnpaidPylonLeaseProofSchemaVersion),
  verifierResultRefs: S.Array(S.String),
  workerRecords: S.Array(ProbeGepaDemoPylonWorker),
  workerRefs: S.Array(S.String),
}) {}

export class ProbeGepaUnpaidPylonLeaseProofUnsafe extends S.TaggedErrorClass<ProbeGepaUnpaidPylonLeaseProofUnsafe>()(
  'ProbeGepaUnpaidPylonLeaseProofUnsafe',
  {
    reason: S.String,
  },
) {}

type LeaseSpec = Readonly<{
  artifactRef: string
  assignmentRef: string
  candidateHash: string
  closeoutDecision: Exclude<PylonGepaMetricCallCloseoutDecision, 'open'>
  closeoutRef: string
  proofBundleRef: string
  resourceUsageRef: string
  taskRef: string
  verifierRef: string
  verifierResultRef: string
  worker: ProbeGepaDemoPylonWorker
}>

const campaignId = 'probe_gepa.stage0.live_receipts.2026_06_08'
const benchmarkSuiteRef = 'benchmark_suite.terminal_bench_2.harbor.retained.v1'
const splitRef =
  'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1'
const probeCommit = 'probe.commit.shc_live_smoke_20260608'

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const uniquePaymentModes = (
  modes: ReadonlyArray<PylonGepaMetricCallPaymentMode>,
): ReadonlyArray<PylonGepaMetricCallPaymentMode> => [...new Set(modes)].sort()

const uniqueCloseoutDecisions = (
  decisions: ReadonlyArray<PylonGepaMetricCallCloseoutDecision>,
): ReadonlyArray<PylonGepaMetricCallCloseoutDecision> =>
  [...new Set(decisions)].sort()

const assertLeaseProofPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  try {
    assertPylonGepaMetricCallPublicRefs(label, refs)
  } catch (error) {
    if (error instanceof PylonGepaMetricCallAssignmentUnsafe) {
      throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
        reason: error.reason,
      })
    }

    throw error
  }
}

const demoWorkers = (): ReadonlyArray<ProbeGepaDemoPylonWorker> =>
  [
    new ProbeGepaDemoPylonWorker({
      capabilityRef: 'capability.public.pylon.demo.alpha.probe_gepa_unpaid.v1',
      isolationRef: 'isolation.public.pylon.demo.alpha.harbor_sandbox.v1',
      publicCapabilityRefs: [
        'capability.public.pylon.benchmark_runner',
        'capability.public.pylon.probe_runtime',
        'capability.public.pylon.unpaid_smoke',
      ],
      workerKind: 'demo_pylon_worker',
      workerRef: 'pylon.public.demo.alpha',
    }),
    new ProbeGepaDemoPylonWorker({
      capabilityRef: 'capability.public.pylon.demo.beta.probe_gepa_unpaid.v1',
      isolationRef: 'isolation.public.pylon.demo.beta.harbor_sandbox.v1',
      publicCapabilityRefs: [
        'capability.public.pylon.benchmark_runner',
        'capability.public.pylon.probe_runtime',
        'capability.public.pylon.unpaid_smoke',
      ],
      workerKind: 'demo_pylon_worker',
      workerRef: 'pylon.public.demo.beta',
    }),
    new ProbeGepaDemoPylonWorker({
      capabilityRef: 'capability.public.pylon.demo.gamma.probe_gepa_unpaid.v1',
      isolationRef: 'isolation.public.pylon.demo.gamma.harbor_sandbox.v1',
      publicCapabilityRefs: [
        'capability.public.pylon.benchmark_runner',
        'capability.public.pylon.probe_runtime',
        'capability.public.pylon.unpaid_smoke',
      ],
      workerKind: 'demo_pylon_worker',
      workerRef: 'pylon.public.demo.gamma',
    }),
  ]

const leaseSpecs = (
  workers: ReadonlyArray<ProbeGepaDemoPylonWorker>,
): ReadonlyArray<LeaseSpec> =>
  [
    {
      artifactRef: 'artifact_manifest.probe.configure_git_webserver.demo_alpha',
      assignmentRef:
        'assignment.public.pylon_gepa_metric_call.stage0.configure_git_webserver.demo_alpha',
      candidateHash:
        'sha256:1000000000000000000000000000000000000000000000000000000000000002',
      closeoutDecision: 'accepted',
      closeoutRef: 'probe_closeout.probe_gepa.stage0.configure_git_webserver.demo_alpha',
      proofBundleRef: 'proof_bundle.probe.configure_git_webserver.demo_alpha',
      resourceUsageRef: 'resource_usage.probe.configure_git_webserver.demo_alpha',
      taskRef: 'benchmark_task.terminal_bench.retained.configure_git_webserver.v1',
      verifierRef: 'verifier.terminal_bench.configure_git_webserver.v1',
      verifierResultRef: 'verifier_result.configure_git_webserver.demo_alpha.pass',
      worker: workers[0]!,
    },
    {
      artifactRef: 'artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608',
      assignmentRef:
        'assignment.public.pylon_gepa_metric_call.stage0.db_wal_recovery.demo_beta',
      candidateHash:
        'sha256:0000000000000000000000000000000000000000000000000000000000004563',
      closeoutDecision: 'rejected',
      closeoutRef: 'probe_closeout.shc_harbor.db_wal_recovery.20260608',
      proofBundleRef: 'proof_bundle.probe.shc_harbor.db_wal_recovery.20260608',
      resourceUsageRef:
        'resource_usage_unavailable.probe.benchmark_run_probe_shc_harbor_db_wal_recovery_20260608',
      taskRef: 'benchmark_task.terminal_bench.retained.db_wal_recovery.v1',
      verifierRef: 'verifier.terminal_bench.db_wal_recovery.v1',
      verifierResultRef:
        'verifier_result.terminal_bench.db_wal_recovery.shc_harbor.20260608.reward_0',
      worker: workers[1]!,
    },
    {
      artifactRef: 'artifact_manifest.probe.filter_js_from_html.demo_gamma',
      assignmentRef:
        'assignment.public.pylon_gepa_metric_call.stage0.filter_js_from_html.demo_gamma',
      candidateHash:
        'sha256:1000000000000000000000000000000000000000000000000000000000000003',
      closeoutDecision: 'accepted',
      closeoutRef: 'probe_closeout.probe_gepa.stage0.filter_js_from_html.demo_gamma',
      proofBundleRef: 'proof_bundle.probe.filter_js_from_html.demo_gamma',
      resourceUsageRef: 'resource_usage.probe.filter_js_from_html.demo_gamma',
      taskRef: 'benchmark_task.terminal_bench.retained.filter_js_from_html.v1',
      verifierRef: 'verifier.terminal_bench.filter_js_from_html.v1',
      verifierResultRef: 'verifier_result.filter_js_from_html.demo_gamma.pass',
      worker: workers[2]!,
    },
  ]

const assignmentInputFor = (spec: LeaseSpec) =>
  ({
    assignmentRef: spec.assignmentRef,
    backendProfileRef: 'backend_profile.probe.benchmark_cloud.v1',
    benchmarkSuiteRef,
    campaignId,
    candidateHash: spec.candidateHash,
    closeoutRequirementRefs: ['probe.benchmark_closeout.v1'],
    expectedArtifactRefs: ['openagents.benchmark_artifact_manifest.v1'],
    expectedProofBundleRefs: ['openagents.benchmark_proof_bundle.v1'],
    paymentMode: 'unpaid_smoke',
    probeCommit,
    runtimeRef: 'runtime.probe.benchmark_cloud.v1',
    scorerRef: 'scorer.terminal_bench.binary.v1',
    splitRef,
    taskRef: spec.taskRef,
    timeoutBudgetRef: 'timeout_budget.probe.retained_smoke.v1',
    verifierRef: spec.verifierRef,
  }) as const

const recordForSpec = (spec: LeaseSpec): PylonGepaMetricCallAssignmentRecord => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInputFor(spec),
    '2026-06-08T14:00:00.000Z',
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: `lease.public.pylon_gepa_metric_call.${spec.worker.workerRef}.${spec.assignmentRef}`,
    nowIso: '2026-06-08T14:01:00.000Z',
    workerRef: spec.worker.workerRef,
  })
  const progressed = reportPylonGepaMetricCallProgress(accepted, {
    nowIso: '2026-06-08T14:02:00.000Z',
    progressRefs: [
      `progress.public.pylon_gepa_metric_call.${spec.worker.workerRef}.accepted`,
      `progress.public.pylon_gepa_metric_call.${spec.worker.workerRef}.artifact_streamed`,
    ],
  })
  const submitted = submitPylonGepaMetricCallResultRefs(progressed, {
    artifactRefs: [spec.artifactRef],
    closeoutResultRefs: [spec.closeoutRef],
    nowIso: '2026-06-08T14:03:00.000Z',
    proofBundleRefs: [spec.proofBundleRef],
    resourceUsageRefs: [spec.resourceUsageRef],
    verifierResultRefs: [spec.verifierResultRef],
  })

  return closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: spec.closeoutDecision,
    closeoutResultRefs: [spec.closeoutRef],
    noSpendEvidenceRefs: [
      `evidence.public.pylon_gepa_metric_call.${spec.worker.workerRef}.unpaid_smoke`,
    ],
    nowIso: '2026-06-08T14:04:00.000Z',
  })
}

const importRefFor = (
  coordinatorImport: PylonGepaMetricCallCoordinatorImport,
): string =>
  `psionic_import.probe_gepa.${coordinatorImport.assignmentRef.replaceAll(
    /[^A-Za-z0-9_.-]+/g,
    '_',
  )}`

export const assertProbeGepaUnpaidPylonLeaseProofSafe = (
  proof: ProbeGepaUnpaidPylonLeaseProof,
): ProbeGepaUnpaidPylonLeaseProof => {
  const decoded = S.decodeUnknownSync(ProbeGepaUnpaidPylonLeaseProof)(proof)
  const acceptedRecords = decoded.assignmentRecords.filter(
    record => record.state === 'accepted_work',
  )
  const rejectedRecords = decoded.assignmentRecords.filter(
    record => record.state === 'rejected_work',
  )
  const forbiddenPaymentModes = decoded.paymentModes.filter(
    mode =>
      mode === 'operator_credit' ||
      mode === 'payable_pending_settlement' ||
      mode === 'settled_bitcoin',
  )

  assertLeaseProofPublicRefs('Probe GEPA unpaid lease proof refs', [
    decoded.proofRef,
    decoded.campaignId,
    decoded.generatedByRef,
    ...decoded.acceptedCloseoutRefs,
    ...decoded.assignmentRefs,
    ...decoded.artifactRefs,
    ...decoded.coordinatorImportRefs,
    ...decoded.progressRefs,
    ...decoded.proofBundleRefs,
    ...decoded.rejectedCloseoutRefs,
    ...decoded.resourceUsageRefs,
    ...decoded.verifierResultRefs,
    ...decoded.workerRefs,
    ...decoded.workerRecords.flatMap(worker => [
      worker.capabilityRef,
      worker.isolationRef,
      worker.workerRef,
      ...worker.publicCapabilityRefs,
    ]),
  ])

  if (
    decoded.workerRecords.length < 3 ||
    decoded.assignmentRecords.length < 3 ||
    decoded.coordinatorImports.length !== decoded.assignmentRecords.length
  ) {
    throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
      reason:
        'Unpaid Probe GEPA lease proof requires at least three demo Pylon workers, three assignments, and one Psionic import per assignment.',
    })
  }

  if (
    acceptedRecords.length === 0 ||
    rejectedRecords.length === 0 ||
    decoded.acceptedCloseoutRefs.length === 0 ||
    decoded.rejectedCloseoutRefs.length === 0
  ) {
    throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
      reason:
        'Unpaid Probe GEPA lease proof requires accepted and rejected closeout states.',
    })
  }

  if (
    forbiddenPaymentModes.length > 0 ||
    !decoded.noSettlementClaim ||
    !decoded.noPaidWorkClaim ||
    !decoded.noAutomaticPromotionClaim ||
    decoded.publicSummaryLabel !== 'unpaid smoke demo Pylon lease proof'
  ) {
    throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
      reason:
        'Unpaid Probe GEPA lease proof cannot carry payout, settlement, promotion, or public score claims.',
    })
  }

  if (
    decoded.assignmentRecords.some(
      record =>
        record.artifactRefs.length === 0 ||
        record.proofBundleRefs.length === 0 ||
        record.resourceUsageRefs.length === 0 ||
        record.verifierResultRefs.length === 0 ||
        record.closeoutResultRefs.length === 0 ||
        record.workerRef === null,
    )
  ) {
    throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
      reason:
        'Every unpaid Probe GEPA assignment must preserve worker, artifact, proof, resource, verifier, and closeout refs.',
    })
  }

  if (
    decoded.coordinatorImports.some(
      coordinatorImport =>
        coordinatorImport.payableWorkClaimAllowed ||
        coordinatorImport.settledBitcoinPayoutClaimAllowed ||
        coordinatorImport.paymentReceiptRefs.length > 0 ||
        coordinatorImport.settlementReceiptRefs.length > 0,
    )
  ) {
    throw new ProbeGepaUnpaidPylonLeaseProofUnsafe({
      reason:
        'Unpaid Probe GEPA coordinator imports cannot claim payable or settled work.',
    })
  }

  return decoded
}

export const buildProbeGepaUnpaidPylonLeaseProof =
  (): ProbeGepaUnpaidPylonLeaseProof => {
    const workers = demoWorkers()
    const specs = leaseSpecs(workers)
    const assignmentRecords = specs.map(recordForSpec)
    const coordinatorImports = assignmentRecords.map(
      pylonGepaMetricCallCoordinatorImport,
    )
    const proof = new ProbeGepaUnpaidPylonLeaseProof({
      acceptedCloseoutRefs: uniqueRefs(
        assignmentRecords
          .filter(record => record.closeoutDecision === 'accepted')
          .flatMap(record => record.closeoutResultRefs),
      ),
      assignmentRecords,
      assignmentRefs: uniqueRefs(
        assignmentRecords.map(record => record.assignmentRef),
      ),
      artifactRefs: uniqueRefs(
        assignmentRecords.flatMap(record => record.artifactRefs),
      ),
      campaignId,
      closeoutDecisions: uniqueCloseoutDecisions(
        assignmentRecords.map(record => record.closeoutDecision),
      ),
      coordinatorImportRefs: uniqueRefs(coordinatorImports.map(importRefFor)),
      coordinatorImports,
      generatedByRef: 'omega.operator.probe_gepa.unpaid_pylon_lease_smoke.v1',
      noAutomaticPromotionClaim: true,
      noPaidWorkClaim: true,
      noSettlementClaim: true,
      paymentModes: uniquePaymentModes(
        assignmentRecords.map(record => record.paymentMode),
      ),
      progressRefs: uniqueRefs(
        assignmentRecords.flatMap(record => record.progressRefs),
      ),
      proofBundleRefs: uniqueRefs(
        assignmentRecords.flatMap(record => record.proofBundleRefs),
      ),
      proofRef: 'proof.omega.probe_gepa.unpaid_pylon_leases.20260608',
      publicSummaryLabel: 'unpaid smoke demo Pylon lease proof',
      rejectedCloseoutRefs: uniqueRefs(
        assignmentRecords
          .filter(record => record.closeoutDecision === 'rejected')
          .flatMap(record => record.closeoutResultRefs),
      ),
      resourceUsageRefs: uniqueRefs(
        assignmentRecords.flatMap(record => record.resourceUsageRefs),
      ),
      schemaVersion: ProbeGepaUnpaidPylonLeaseProofSchemaVersion,
      verifierResultRefs: uniqueRefs(
        assignmentRecords.flatMap(record => record.verifierResultRefs),
      ),
      workerRecords: workers,
      workerRefs: uniqueRefs(workers.map(worker => worker.workerRef)),
    })

    return assertProbeGepaUnpaidPylonLeaseProofSafe(proof)
  }
