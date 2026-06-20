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
export const TrainingAblationDeriskingLedgerStaleness = liveAtReadStaleness([
  'training_ablation_manifest_published',
  'training_eval_reproduction_receipt_published',
  'training_ablation_verdict_published',
  'product_promise_registry_updated',
])

export class TrainingAblationDeriskingLedgerEntry extends S.Class<TrainingAblationDeriskingLedgerEntry>(
  'TrainingAblationDeriskingLedgerEntry',
)({
  entryRef: S.String,
  title: S.String,
  baselineRef: S.String,
  deltaRef: S.String,
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

const candidateEntries =
  (): ReadonlyArray<TrainingAblationDeriskingLedgerEntry> => [
    new TrainingAblationDeriskingLedgerEntry({
      baselineRef: 'baseline.psion.r1_reference_optimizer',
      blockerRefs: [
        'blocker.product_promises.ablation_harness_missing',
        'blocker.product_promises.eval_suite_reproduction_missing',
      ],
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
        'caveat.training_ablation.one_delta_manifest_not_published',
      ],
      decisionState: 'candidate',
      deltaRef: 'delta.training.wsd_schedule',
      entryRef: 'ablation.derisking.wsd_schedule_candidate',
      evalReproductionState: 'missing',
      evidenceRefs: [],
      oneDeltaManifestState: 'candidate_ref_only',
      paidDispatchState: 'not_dispatched',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-19-model-ladder-rung-economics.md',
      ]),
      title: 'WSD schedule candidate',
      verdictState: 'no_openagents_verdict',
    }),
    new TrainingAblationDeriskingLedgerEntry({
      baselineRef: 'baseline.psion.r1_reference_corpus_pipeline',
      blockerRefs: [
        'blocker.product_promises.ablation_harness_missing',
        'blocker.product_promises.eval_suite_reproduction_missing',
      ],
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
        'caveat.training_ablation.refinery_eval_delta_unverified',
      ],
      decisionState: 'candidate',
      deltaRef: 'delta.training.intra_document_dedup',
      entryRef: 'ablation.derisking.intra_document_dedup_candidate',
      evalReproductionState: 'missing',
      evidenceRefs: [],
      oneDeltaManifestState: 'candidate_ref_only',
      paidDispatchState: 'not_dispatched',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
      ]),
      title: 'Intra-document deduplication candidate',
      verdictState: 'no_openagents_verdict',
    }),
    new TrainingAblationDeriskingLedgerEntry({
      baselineRef: 'baseline.psion.r1_reference_training_runtime',
      blockerRefs: [
        'blocker.product_promises.ablation_harness_missing',
        'blocker.product_promises.eval_suite_reproduction_missing',
      ],
      caveatRefs: [
        'caveat.training_ablation.candidate_not_openagents_result',
        'caveat.training_ablation.edge_reference_not_openagents_measurement',
      ],
      decisionState: 'hold',
      deltaRef: 'delta.training.qvac_rng_backend',
      entryRef: 'ablation.derisking.qvac_rng_backend_candidate',
      evalReproductionState: 'missing',
      evidenceRefs: [],
      oneDeltaManifestState: 'candidate_ref_only',
      paidDispatchState: 'not_dispatched',
      sourceRefs: entryRefs([
        'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      ]),
      title: 'QVAC RNG backend candidate',
      verdictState: 'no_openagents_verdict',
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
      ablationHarnessAvailable: false,
      clearsBlockerRefs: [
        'blocker.product_promises.ablation_ledger_projection_missing',
      ],
      evalSuiteReproductionAvailable: false,
      greenGateSatisfied: false,
      paidAblationDispatchAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        'blocker.product_promises.ablation_harness_missing',
        'blocker.product_promises.eval_suite_reproduction_missing',
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
      'Public ablation derisking ledger projection is live with candidate-only entries; the one-delta harness, eval reproduction receipt, and paid ablation dispatch remain missing.',
    unsafeCopy:
      'Do not claim OpenAgents has run ablations, reproduced eval suites, accepted ablation verdicts, paid ablation assignments, promoted model changes, or proven training decisions through this ledger. Current entries are candidate refs only.',
  })
}
