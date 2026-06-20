import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TrainingAblationDeriskingLedgerEndpoint =
  '/api/public/training/ablation-derisking-ledger'
export const TrainingAblationDeriskingLedgerSchemaVersion =
  'openagents.training.ablation_derisking_ledger.v1'
export const TrainingAblationOneDeltaHarnessRef =
  'harness.training_ablation.one_delta_manifest.v1'
export const TrainingAblationDeriskingLedgerStaleness = liveAtReadStaleness([
  'training_ablation_manifest_published',
  'training_eval_reproduction_receipt_published',
  'training_ablation_verdict_published',
  'product_promise_registry_updated',
])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/#-]*$/),
)
const PublicSafeRefs = S.Array(PublicSafeRef)

export const TrainingAblationDeltaKinds = [
  'optimizer_schedule',
  'corpus_transform',
  'runtime_config',
  'rng_backend',
  'model_architecture',
] as const

export class TrainingAblationManifestDelta extends S.Class<TrainingAblationManifestDelta>(
  'TrainingAblationManifestDelta',
)({
  deltaRef: PublicSafeRef,
  kind: S.Literals(TrainingAblationDeltaKinds),
  sourceRefs: PublicSafeRefs,
  summary: NonEmptyTrimmedString,
  targetRef: PublicSafeRef,
}) {}

export class TrainingAblationOneDeltaManifestInput extends S.Class<TrainingAblationOneDeltaManifestInput>(
  'TrainingAblationOneDeltaManifestInput',
)({
  baselineRef: PublicSafeRef,
  caveatRefs: PublicSafeRefs,
  candidateRef: PublicSafeRef,
  deltas: S.Array(TrainingAblationManifestDelta),
  evaluationPlanRefs: PublicSafeRefs,
  frozenRefSet: PublicSafeRefs,
  manifestRef: PublicSafeRef,
  sourceRefs: PublicSafeRefs,
}) {}

export class TrainingAblationManifestVerification extends S.Class<TrainingAblationManifestVerification>(
  'TrainingAblationManifestVerification',
)({
  accepted: S.Boolean,
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  changedDeltaCount: S.Int,
  clearsBlockerRefs: S.Array(S.String),
  deltaRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  harnessRef: S.Literal(TrainingAblationOneDeltaHarnessRef),
  manifestRef: S.String,
}) {}

export class TrainingAblationOneDeltaHarnessError extends Error {
  readonly _tag = 'TrainingAblationOneDeltaHarnessError'
}

export class TrainingAblationDeriskingLedgerEntry extends S.Class<TrainingAblationDeriskingLedgerEntry>(
  'TrainingAblationDeriskingLedgerEntry',
)({
  entryRef: S.String,
  title: S.String,
  baselineRef: S.String,
  deltaRef: S.String,
  manifestRef: S.String,
  oneDeltaManifestState: S.Literals([
    'candidate_ref_only',
    'manifest_verified',
  ]),
  evalReproductionState: S.Literals(['missing', 'reproduced']),
  paidDispatchState: S.Literals(['not_dispatched', 'settled']),
  verdictState: S.Literals([
    'no_openagents_verdict',
    'accepted',
    'rejected',
  ]),
  decisionState: S.Literals(['candidate', 'hold', 'adopt', 'reject']),
  sourceRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
}) {}

export class TrainingAblationDeriskingLedgerProjection extends S.Class<TrainingAblationDeriskingLedgerProjection>(
  'TrainingAblationDeriskingLedgerProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  endpoint: S.Literal('/api/public/training/ablation-derisking-ledger'),
  promiseRef: S.Literal('promise:training.ablation_system.v1'),
  promiseState: S.Literal('planned'),
  status: S.Literal('candidate_ledger_projection'),
  statusLabel: S.String,
  staleness: PublicProjectionStalenessContract,
  gate: S.Struct({
    publicProjectionAvailable: S.Boolean,
    ablationHarnessAvailable: S.Boolean,
    evalSuiteReproductionAvailable: S.Boolean,
    paidAblationDispatchAvailable: S.Boolean,
    greenGateSatisfied: S.Boolean,
    clearsBlockerRefs: S.Array(S.String),
    remainingBlockerRefs: S.Array(S.String),
  }),
  ledgerSummary: S.Struct({
    entryCount: S.Int,
    candidateEntryCount: S.Int,
    verifiedManifestCount: S.Int,
    reproducedEvalCount: S.Int,
    paidAblationCount: S.Int,
    acceptedVerdictCount: S.Int,
  }),
  entries: S.Array(TrainingAblationDeriskingLedgerEntry),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

const entryRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingAblationOneDeltaHarnessError(
      `${label} contains private material and cannot be projected publicly.`,
    )
  }
}

export const verifyTrainingAblationOneDeltaManifest = (
  input: unknown,
): TrainingAblationManifestVerification => {
  let manifest: TrainingAblationOneDeltaManifestInput

  try {
    manifest = S.decodeUnknownSync(TrainingAblationOneDeltaManifestInput)(input)
  } catch {
    throw new TrainingAblationOneDeltaHarnessError(
      'Training ablation manifest does not match the one-delta manifest schema.',
    )
  }

  assertPublicSafeValue('Training ablation manifest', manifest)

  if (manifest.deltas.length !== 1) {
    throw new TrainingAblationOneDeltaHarnessError(
      'Training ablation manifest must carry exactly one delta.',
    )
  }

  const deltaRefs = manifest.deltas.map(delta => delta.deltaRef)
  const verification = new TrainingAblationManifestVerification({
    accepted: true,
    authorityBoundary:
      'The one-delta manifest verifier proves manifest shape only. It grants no training-dispatch, spend, settlement, evaluation, verdict, checkpoint-promotion, or public-claim authority.',
    blockerRefs: [],
    changedDeltaCount: deltaRefs.length,
    clearsBlockerRefs: [
      'blocker.product_promises.ablation_harness_missing',
    ],
    deltaRefs: entryRefs(deltaRefs),
    evidenceRefs: entryRefs([
      TrainingAblationOneDeltaHarnessRef,
      manifest.manifestRef,
      ...manifest.evaluationPlanRefs,
      ...manifest.frozenRefSet,
      ...manifest.sourceRefs,
    ]),
    harnessRef: TrainingAblationOneDeltaHarnessRef,
    manifestRef: manifest.manifestRef,
  })

  assertPublicSafeValue('Training ablation manifest verification', verification)

  return verification
}

const candidateManifest = (
  input: Readonly<{
    baselineRef: string
    caveatRefs: ReadonlyArray<string>
    candidateRef: string
    delta: TrainingAblationManifestDelta
    evaluationPlanRefs: ReadonlyArray<string>
    frozenRefSet: ReadonlyArray<string>
    manifestRef: string
    sourceRefs: ReadonlyArray<string>
  }>,
): TrainingAblationOneDeltaManifestInput =>
  new TrainingAblationOneDeltaManifestInput({
    baselineRef: input.baselineRef,
    caveatRefs: input.caveatRefs,
    candidateRef: input.candidateRef,
    deltas: [input.delta],
    evaluationPlanRefs: input.evaluationPlanRefs,
    frozenRefSet: input.frozenRefSet,
    manifestRef: input.manifestRef,
    sourceRefs: input.sourceRefs,
  })

const candidateEntry = (
  input: Readonly<{
    baselineRef: string
    caveatRefs: ReadonlyArray<string>
    decisionState: 'candidate' | 'hold'
    delta: TrainingAblationManifestDelta
    entryRef: string
    evaluationPlanRefs: ReadonlyArray<string>
    frozenRefSet: ReadonlyArray<string>
    manifestRef: string
    sourceRefs: ReadonlyArray<string>
    title: string
  }>,
): TrainingAblationDeriskingLedgerEntry => {
  const manifest = candidateManifest({
    baselineRef: input.baselineRef,
    caveatRefs: input.caveatRefs,
    candidateRef: input.entryRef,
    delta: input.delta,
    evaluationPlanRefs: input.evaluationPlanRefs,
    frozenRefSet: input.frozenRefSet,
    manifestRef: input.manifestRef,
    sourceRefs: input.sourceRefs,
  })
  const verification = verifyTrainingAblationOneDeltaManifest(manifest)

  return new TrainingAblationDeriskingLedgerEntry({
    baselineRef: input.baselineRef,
    blockerRefs: [
      'blocker.product_promises.eval_suite_reproduction_missing',
      'blocker.product_promises.paid_ablation_dispatch_missing',
    ],
    caveatRefs: entryRefs([
      ...input.caveatRefs,
      'caveat.training_ablation.manifest_verified_but_not_executed',
    ]),
    decisionState: input.decisionState,
    deltaRef: input.delta.deltaRef,
    entryRef: input.entryRef,
    evalReproductionState: 'missing',
    evidenceRefs: verification.evidenceRefs,
    manifestRef: input.manifestRef,
    oneDeltaManifestState: 'manifest_verified',
    paidDispatchState: 'not_dispatched',
    sourceRefs: entryRefs(input.sourceRefs),
    title: input.title,
    verdictState: 'no_openagents_verdict',
  })
}

const candidateEntries =
  (): ReadonlyArray<TrainingAblationDeriskingLedgerEntry> => [
    candidateEntry({
      baselineRef: 'baseline.psion.r1_reference_optimizer',
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
      ],
      decisionState: 'candidate',
      delta: new TrainingAblationManifestDelta({
        deltaRef: 'delta.training.wsd_schedule',
        kind: 'optimizer_schedule',
        sourceRefs: [
          'docs/training/2026-06-19-model-ladder-rung-economics.md',
        ],
        summary: 'Change only the candidate WSD optimizer schedule.',
        targetRef: 'target.training.optimizer_schedule',
      }),
      entryRef: 'ablation.derisking.wsd_schedule_candidate',
      evaluationPlanRefs: [
        'eval_plan.psion.r1_reference_optimizer.fixed_suite',
      ],
      frozenRefSet: [
        'frozen.training.r1_reference_corpus',
        'frozen.training.r1_reference_architecture',
      ],
      manifestRef: 'manifest.training_ablation.wsd_schedule.one_delta.v1',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-19-model-ladder-rung-economics.md',
      ]),
      title: 'WSD schedule candidate',
    }),
    candidateEntry({
      baselineRef: 'baseline.psion.r1_reference_corpus_pipeline',
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
        'caveat.training_ablation.refinery_eval_delta_unverified',
      ],
      decisionState: 'candidate',
      delta: new TrainingAblationManifestDelta({
        deltaRef: 'delta.training.intra_document_dedup',
        kind: 'corpus_transform',
        sourceRefs: [
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
        ],
        summary:
          'Change only the intra-document deduplication transform candidate.',
        targetRef: 'target.training.corpus_transform',
      }),
      entryRef: 'ablation.derisking.intra_document_dedup_candidate',
      evaluationPlanRefs: [
        'eval_plan.psion.r1_reference_corpus_pipeline.fixed_suite',
      ],
      frozenRefSet: [
        'frozen.training.r1_reference_optimizer',
        'frozen.training.r1_reference_architecture',
      ],
      manifestRef:
        'manifest.training_ablation.intra_document_dedup.one_delta.v1',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
      ]),
      title: 'Intra-document deduplication candidate',
    }),
    candidateEntry({
      baselineRef: 'baseline.psion.r1_reference_training_runtime',
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
        'caveat.training_ablation.edge_reference_not_openagents_measurement',
      ],
      decisionState: 'hold',
      delta: new TrainingAblationManifestDelta({
        deltaRef: 'delta.training.qvac_rng_backend',
        kind: 'rng_backend',
        sourceRefs: ['docs/training/2026-06-10-qvac-edge-stack-analysis.md'],
        summary: 'Change only the candidate RNG backend.',
        targetRef: 'target.training.rng_backend',
      }),
      entryRef: 'ablation.derisking.qvac_rng_backend_candidate',
      evaluationPlanRefs: [
        'eval_plan.psion.r1_reference_runtime.fixed_suite',
      ],
      frozenRefSet: [
        'frozen.training.r1_reference_optimizer',
        'frozen.training.r1_reference_corpus',
      ],
      manifestRef: 'manifest.training_ablation.qvac_rng_backend.one_delta.v1',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      ]),
      title: 'QVAC RNG backend candidate',
    }),
  ]

export const projectTrainingAblationDeriskingLedger = (
  input: { generatedAt?: string | undefined } = {},
): TrainingAblationDeriskingLedgerProjection => {
  const entries = candidateEntries()

  return new TrainingAblationDeriskingLedgerProjection({
    authorityBoundary:
      'The ablation derisking ledger is a public read-only planning and evidence index. It grants no training-dispatch, assignment, spend, settlement, model-promotion, public-claim, or capability authority.',
    endpoint: TrainingAblationDeriskingLedgerEndpoint,
    entries,
    gate: {
      ablationHarnessAvailable: true,
      clearsBlockerRefs: [
        'blocker.product_promises.ablation_ledger_projection_missing',
        'blocker.product_promises.ablation_harness_missing',
      ],
      evalSuiteReproductionAvailable: false,
      greenGateSatisfied: false,
      paidAblationDispatchAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        'blocker.product_promises.eval_suite_reproduction_missing',
        'blocker.product_promises.paid_ablation_dispatch_missing',
      ],
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    ledgerSummary: {
      acceptedVerdictCount: entries.filter(
        entry => entry.verdictState === 'accepted',
      ).length,
      candidateEntryCount: entries.filter(
        entry => entry.decisionState === 'candidate',
      ).length,
      entryCount: entries.length,
      paidAblationCount: entries.filter(
        entry => entry.paidDispatchState === 'settled',
      ).length,
      reproducedEvalCount: entries.filter(
        entry => entry.evalReproductionState === 'reproduced',
      ).length,
      verifiedManifestCount: entries.filter(
        entry => entry.oneDeltaManifestState === 'manifest_verified',
      ).length,
    },
    promiseRef: 'promise:training.ablation_system.v1',
    promiseState: 'planned',
    schemaVersion: TrainingAblationDeriskingLedgerSchemaVersion,
    sourceRefs: [
      'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
      'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
      'docs/promises/2026-06-19-weekend-promise-assault-roadmap.md',
      'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.ts',
    ],
    staleness: TrainingAblationDeriskingLedgerStaleness,
    status: 'candidate_ledger_projection',
    statusLabel:
      'Public ablation derisking ledger projection is live with one-delta manifest-verified candidates; eval reproduction receipts and paid ablation dispatch remain missing.',
    unsafeCopy:
      'Do not claim OpenAgents has run ablations, reproduced eval suites, accepted ablation verdicts, paid ablation assignments, promoted model changes, or proven training decisions through this ledger. Current entries are one-delta manifest-verified candidates only.',
  })
}
