import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TassadarPerceptaArchitectureReceiptsEndpoint =
  '/api/public/models/tassadar-percepta-executor/architecture-receipts'
export const TassadarPerceptaArchitectureReceiptsSchemaVersion =
  'openagents.models.tassadar_percepta_executor.architecture_receipts.v1'
export const TassadarPerceptaArchitectureReceiptRef =
  'receipt.models.tassadar_percepta_executor.architecture.bundle.v1'
export const TassadarPerceptaArchitectureReceiptsStaleness =
  liveAtReadStaleness([
    'tassadar_percepta_architecture_receipt_published',
    'product_promise_registry_updated',
  ])

export const TassadarPerceptaArchitectureReceiptBlocker =
  'blocker.product_promises.percepta_executor_architecture_receipts_missing'
export const TassadarPerceptaCpuTransformTrainingReceiptBlocker =
  'blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing'
export const TassadarPerceptaCpuTransformRealSettlementBlocker =
  'blocker.product_promises.tassadar_cpu_transform_real_settlement_missing'
export const TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker =
  'blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing'

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

export class TassadarPerceptaArchitectureReceiptsUnsafe extends Error {
  readonly _tag = 'TassadarPerceptaArchitectureReceiptsUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TassadarPerceptaArchitectureReceiptsUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

export class TassadarPerceptaArchitectureComponent extends S.Class<TassadarPerceptaArchitectureComponent>(
  'TassadarPerceptaArchitectureComponent',
)({
  caveatRefs: S.Array(S.String),
  componentKind: S.Literals([
    'compiled_executor_bundle',
    'learned_interface_bundle',
    'verification_boundary',
    'artifact_lineage',
  ]),
  componentRef: S.String,
  digestRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  evidenceState: S.Literals([
    'receipted_architecture_component',
    'green_substrate',
    'research_evaluation_receipt',
  ]),
  sourceRefs: S.Array(S.String),
  summary: S.String,
}) {}

export class TassadarPerceptaArchitectureReceipt extends S.Class<TassadarPerceptaArchitectureReceipt>(
  'TassadarPerceptaArchitectureReceipt',
)({
  architectureFamily: S.Literal('percepta_executor_hybrid'),
  artifactLineage: S.Struct({
    corpusManifestRefs: S.Array(S.String),
    evalReportDigestRefs: S.Array(S.String),
    learnedInterfaceDigestRefs: S.Array(S.String),
    modelDescriptorRefs: S.Array(S.String),
    verifierRefs: S.Array(S.String),
  }),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clearsBlockerRefs: S.Array(S.String),
  compiledExecutorCoverage: S.Struct({
    deploymentCount: S.Int,
    exactnessPosture: S.Literal('exact_trace_and_output'),
    profileRef: S.String,
    sampleModelRef: S.String,
    traceAbiRef: S.String,
  }),
  components: S.Array(TassadarPerceptaArchitectureComponent),
  evidenceRefs: S.Array(S.String),
  learnedInterfaceMetrics: S.Struct({
    baselineRef: S.Literal(
      'baseline_d_frozen_executor_learned_interface',
    ),
    exactRolloutPassAt1Bps: S.Int,
    outputDigestMatchBps: S.Int,
    replayVerifierAcceptanceBps: S.Int,
    validPrefixMedianTokens: S.Int,
  }),
  modelProfileRef: S.String,
  publicSafe: S.Literal(true),
  receiptRef: S.Literal(TassadarPerceptaArchitectureReceiptRef),
  receiptState: S.Literal('available'),
  sourceRefs: S.Array(S.String),
  unsafeCopy: S.String,
}) {}

export class TassadarPerceptaArchitectureReceiptsProjection extends S.Class<TassadarPerceptaArchitectureReceiptsProjection>(
  'TassadarPerceptaArchitectureReceiptsProjection',
)({
  authorityBoundary: S.String,
  endpoint: S.Literal(TassadarPerceptaArchitectureReceiptsEndpoint),
  gate: S.Struct({
    architectureReceiptsAvailable: S.Boolean,
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    pylonCpuTransformTrainingReceiptsAvailable: S.Boolean,
    publicProjectionAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:models.tassadar_percepta_executor.v1'),
  promiseState: S.Literal('planned'),
  receiptSummary: S.Struct({
    architectureReceiptCount: S.Int,
    componentCount: S.Int,
    compiledExecutorDeploymentCount: S.Int,
    greenGateSatisfied: S.Boolean,
    learnedInterfaceReceiptCount: S.Int,
  }),
  receipts: S.Array(TassadarPerceptaArchitectureReceipt),
  schemaVersion: S.Literal(
    TassadarPerceptaArchitectureReceiptsSchemaVersion,
  ),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('architecture_receipts_available'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

const githubPsionicRef = (path: string): string =>
  `https://github.com/OpenAgentsInc/psionic/blob/main/${path}`

const githubPsionicTreeRef = (path: string): string =>
  `https://github.com/OpenAgentsInc/psionic/tree/main/${path}`

const buildArchitectureReceipt = (): TassadarPerceptaArchitectureReceipt => {
  const compiledExecutorComponent =
    new TassadarPerceptaArchitectureComponent({
      caveatRefs: [
        'caveat.models.tassadar.compiled_executor_fixture_not_trained_model',
        'caveat.models.tassadar.hardware_validation_not_product_training',
      ],
      componentKind: 'compiled_executor_bundle',
      componentRef: 'component.tassadar.compiled_kernel_suite_v0',
      digestRefs: [
        'model_descriptor.sha256.9057b23d919da7e6ca427baea9e0850c96368fb22709a188f9e7ff961f76bb70',
        'trace_digest.5f9ad09571337c94b01dfd1fb09954784d0a025bcad384a72182982169771af9',
        'weights.sha256.369d740e4acb6a75bc4dca9513a75efcaf8cd8f89aecd6108d62a7b88b5467c9',
      ],
      evidenceRefs: [
        'receipt.models.tassadar.compiled_executor.kernel_suite_v0',
        'model.tassadar-executor-article-i32-compute-v0-compiled-tassadar.compiled_kernel_suite.forward_branch.count_2.v1',
        'tassadar://trace/compile-forward_branch_kernel_branches_2.compiled_executor/5f9ad09571337c94b01dfd1fb09954784d0a025bcad384a72182982169771af9',
      ],
      evidenceState: 'receipted_architecture_component',
      sourceRefs: [
        githubPsionicTreeRef(
          'fixtures/tassadar/runs/compiled_kernel_suite_v0/deployments',
        ),
        githubPsionicRef(
          'fixtures/tassadar/runs/compiled_kernel_suite_v0/deployments/forward_branch_kernel_branches_2/model_descriptor.json',
        ),
        githubPsionicRef(
          'fixtures/tassadar/runs/compiled_kernel_suite_v0/deployments/forward_branch_kernel_branches_2/compile_evidence_bundle.json',
        ),
        githubPsionicRef(
          'fixtures/tassadar/runs/compiled_kernel_suite_v0/deployments/forward_branch_kernel_branches_2/runtime_execution_proof_bundle.json',
        ),
      ],
      summary:
        'Psionic compiled-kernel suite contains 12 compiled executor deployments with model descriptors, trace ABI refs, compile evidence bundles, and runtime execution proof bundles.',
    })
  const learnedInterfaceComponent =
    new TassadarPerceptaArchitectureComponent({
      caveatRefs: [
        'caveat.models.tassadar.w3_research_evaluation_not_product_model',
      ],
      componentKind: 'learned_interface_bundle',
      componentRef:
        'component.tassadar.w3_baseline_d_frozen_executor_learned_interface',
      digestRefs: [
        'dataset.sha256.d045a53d0cecbe6ffb1b4f0c1522ab76b02014491842f1770d34c12a885c8c3a',
        'interface.sha256.1743636ec83c9592a85cbb7f293c480b810fbcf764ca94ec76b39669a13ee6bb',
        'checkpoint.sha256.9eb153e360f576770a6de0e50abd07fbb6ece1237c80b08d2bf6c4ffbb6d0217',
        'eval_report.sha256.a9b2bf9d95228d69f33f9dc4826d14536e3a70ddd15bb6d6b243888c3baebfd5',
      ],
      evidenceRefs: [
        'receipt.models.tassadar.w3_student_sweep.baseline_d_frozen_interface.v1',
        'corpus.tassadar_trace.v0_2.w3_100m',
        'baseline_d_frozen_executor_learned_interface',
      ],
      evidenceState: 'research_evaluation_receipt',
      sourceRefs: [
        'docs/tassadar/2026-06-14-w3-student-program-report.md',
        githubPsionicRef('fixtures/tassadar/w3_student_sweep_20260612/manifest.json'),
        githubPsionicRef('fixtures/tassadar/w3_student_sweep_20260612/d/receipt.json'),
        githubPsionicRef(
          'fixtures/tassadar/w3_student_sweep_20260612/d/eval-report.json',
        ),
      ],
      summary:
        'W3 baseline D binds a frozen analytic executor to a learned interface and records checkpoint/interface/config/eval digests plus exact-rollout and replay-verifier metrics.',
    })
  const verifierComponent = new TassadarPerceptaArchitectureComponent({
    caveatRefs: [
      'caveat.models.tassadar.executor_poc_not_general_model_capability',
    ],
    componentKind: 'verification_boundary',
    componentRef: 'component.tassadar.exact_trace_replay_boundary',
    digestRefs: [],
    evidenceRefs: [
      'promise:compute.tassadar_executor_poc.v1',
      'verifier_class.exact_trace_replay',
      'packages/tassadar-executor/src/replay.ts',
    ],
    evidenceState: 'green_substrate',
    sourceRefs: [
      'docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md',
      'packages/tassadar-executor/src/replay.ts',
      'promise:compute.tassadar_executor_poc.v1',
    ],
    summary:
      'The architecture receipt is bounded by the existing exact-trace-replay verifier substrate; verifier success does not promote a product model.',
  })
  const artifactLineageComponent =
    new TassadarPerceptaArchitectureComponent({
      caveatRefs: [
        'caveat.models.tassadar.lineage_refs_only_no_raw_artifact_export',
      ],
      componentKind: 'artifact_lineage',
      componentRef: 'component.tassadar.percepta_executor_lineage_refs',
      digestRefs: [
        'train_prep.sha256.8095588b05ff1bc3b8a723431c35015882a25566f74d895b514071f5e1734350',
        'eval_prep.sha256.512830dcbdd4f8e4842adbf1960522c70e8609475581aa4936f6424b4981102b',
      ],
      evidenceRefs: [
        'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
        'docs/tassadar/2026-06-14-w3-student-program-report.md',
        'receipt.models.tassadar.compiled_executor.kernel_suite_v0',
      ],
      evidenceState: 'receipted_architecture_component',
      sourceRefs: [
        'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
        'docs/tassadar/2026-06-14-w3-student-program-report.md',
        githubPsionicTreeRef('fixtures/tassadar/w3_student_sweep_20260612'),
        githubPsionicTreeRef('fixtures/tassadar/runs/compiled_kernel_suite_v0'),
      ],
      summary:
        'The public architecture bundle links model profile, compiled/frozen executor components, learned-interface components, eval digests, and verifier refs without exposing raw traces or runner logs.',
    })

  const components = [
    compiledExecutorComponent,
    learnedInterfaceComponent,
    verifierComponent,
    artifactLineageComponent,
  ]

  const receipt = new TassadarPerceptaArchitectureReceipt({
    architectureFamily: 'percepta_executor_hybrid',
    artifactLineage: {
      corpusManifestRefs: ['corpus.tassadar_trace.v0_2.w3_100m'],
      evalReportDigestRefs: [
        'eval_report.sha256.a9b2bf9d95228d69f33f9dc4826d14536e3a70ddd15bb6d6b243888c3baebfd5',
      ],
      learnedInterfaceDigestRefs: [
        'interface.sha256.1743636ec83c9592a85cbb7f293c480b810fbcf764ca94ec76b39669a13ee6bb',
        'checkpoint.sha256.9eb153e360f576770a6de0e50abd07fbb6ece1237c80b08d2bf6c4ffbb6d0217',
      ],
      modelDescriptorRefs: [
        'model.tassadar-executor-article-i32-compute-v0-compiled-tassadar.compiled_kernel_suite.forward_branch.count_2.v1',
      ],
      verifierRefs: [
        'verifier_class.exact_trace_replay',
        'promise:compute.tassadar_executor_poc.v1',
      ],
    },
    authorityBoundary:
      'This architecture receipt proves only that the Tassadar Percepta Executor direction has public-safe model-profile, compiled/frozen executor, learned-interface, artifact-lineage, and verifier refs. It grants no trained-model, inference, dispatch, spend, settlement, model-promotion, CPU-transform training, or public green-claim authority.',
    blockerRefs: [],
    caveatRefs: [
      'caveat.models.tassadar.architecture_receipt_not_trained_model',
      'caveat.models.tassadar.architecture_receipt_not_pylon_cpu_transform_training',
      'caveat.models.tassadar.no_model_promotion_or_inference_endpoint',
    ],
    clearsBlockerRefs: [TassadarPerceptaArchitectureReceiptBlocker],
    compiledExecutorCoverage: {
      deploymentCount: 12,
      exactnessPosture: 'exact_trace_and_output',
      profileRef: 'tassadar.wasm.article_i32_compute.v1',
      sampleModelRef:
        'tassadar-executor-article-i32-compute-v0-compiled-tassadar.compiled_kernel_suite.forward_branch.count_2.v1',
      traceAbiRef: 'tassadar.trace.v1',
    },
    components,
    evidenceRefs: entryRefs([
      TassadarPerceptaArchitectureReceiptRef,
      ...components.flatMap(component => component.evidenceRefs),
      ...components.flatMap(component => component.digestRefs),
    ]),
    learnedInterfaceMetrics: {
      baselineRef: 'baseline_d_frozen_executor_learned_interface',
      exactRolloutPassAt1Bps: 10000,
      outputDigestMatchBps: 10000,
      replayVerifierAcceptanceBps: 10000,
      validPrefixMedianTokens: 10240,
    },
    modelProfileRef: 'model_profile.tassadar_percepta_executor.v1',
    publicSafe: true,
    receiptRef: TassadarPerceptaArchitectureReceiptRef,
    receiptState: 'available',
    sourceRefs: entryRefs(components.flatMap(component => component.sourceRefs)),
    unsafeCopy:
      'Do not claim a trained Tassadar model exists, that this architecture receipt is the CPU-transform training receipt, that public contributors trained this model, that an inference endpoint exists, or that this architecture receipt makes the planned promise green.',
  })

  assertPublicSafeValue('Tassadar Percepta architecture receipt', receipt)

  return receipt
}

export const projectTassadarPerceptaArchitectureReceipts = (
  input: { generatedAt?: string | undefined } = {},
): TassadarPerceptaArchitectureReceiptsProjection => {
  const receipts = [buildArchitectureReceipt()]
  const componentCount = receipts.reduce(
    (count, receipt) => count + receipt.components.length,
    0,
  )

  const projection = new TassadarPerceptaArchitectureReceiptsProjection({
    authorityBoundary:
      'Read-only public architecture-receipt projection for models.tassadar_percepta_executor.v1. It narrows one product-promise blocker by publishing refs and digests only; it grants no dispatch, spend, settlement, model-promotion, inference, CPU-transform training, or green-claim authority.',
    endpoint: TassadarPerceptaArchitectureReceiptsEndpoint,
    gate: {
      architectureReceiptsAvailable: true,
      clearsBlockerRefs: [TassadarPerceptaArchitectureReceiptBlocker],
      greenGateSatisfied: false,
      pylonCpuTransformTrainingReceiptsAvailable: true,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        TassadarPerceptaCpuTransformRealSettlementBlocker,
        TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker,
      ],
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:models.tassadar_percepta_executor.v1',
    promiseState: 'planned',
    receiptSummary: {
      architectureReceiptCount: receipts.length,
      componentCount,
      compiledExecutorDeploymentCount:
        receipts[0]?.compiledExecutorCoverage.deploymentCount ?? 0,
      greenGateSatisfied: false,
      learnedInterfaceReceiptCount: receipts.filter(receipt =>
        receipt.components.some(
          component => component.componentKind === 'learned_interface_bundle',
        ),
      ).length,
    },
    receipts,
    schemaVersion: TassadarPerceptaArchitectureReceiptsSchemaVersion,
    sourceRefs: entryRefs([
      'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
      'docs/tassadar/2026-06-14-w3-student-program-report.md',
      'docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md',
      'apps/openagents.com/workers/api/src/tassadar-percepta-architecture-receipts.ts',
      ...receipts.flatMap(receipt => receipt.sourceRefs),
    ]),
    staleness: TassadarPerceptaArchitectureReceiptsStaleness,
    status: 'architecture_receipts_available',
    statusLabel:
      'Tassadar Percepta executor architecture receipts and one bounded Pylon CPU-transform training fixture receipt are available; real settlement and owner green sign-off remain missing.',
    unsafeCopy:
      'Do not claim a trained Tassadar Percepta model, public contributor CPU-transform training, an inference endpoint, settlement, model promotion, or a green promise. This projection clears only the architecture-receipt blocker.',
  })

  assertPublicSafeValue(
    'Tassadar Percepta architecture receipts projection',
    projection,
  )

  return projection
}
