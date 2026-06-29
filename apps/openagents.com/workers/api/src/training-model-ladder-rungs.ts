import { Schema as S } from 'effect'

import { PublicProductPromisesVersion } from './product-promises'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TrainingModelLadderRungsEndpoint =
  '/api/public/training/model-ladder-rungs'
export const TrainingModelLadderRungsSchemaVersion =
  'openagents.training.model_ladder.rungs.v1'
export const TrainingModelLadderR1FullRehearsalBlocker =
  'blocker.product_promises.r1_full_rehearsal_missing'
export const TrainingModelLadderNetworkRungBlocker =
  'blocker.product_promises.model_ladder_network_rungs_not_run'

export const TrainingModelLadderRungsStaleness = liveAtReadStaleness([
  'product_promise_registry_updated',
  'training_model_ladder_rung_closeout_receipt_published',
  'training_model_ladder_economics_gate_report_published',
])

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

export class TrainingModelLadderRung extends S.Class<TrainingModelLadderRung>(
  'TrainingModelLadderRung',
)({
  blockerRefs: S.Array(S.String),
  closeoutReceiptAvailable: S.Boolean,
  evidenceRefs: S.Array(S.String),
  hardwareReality: S.String,
  modelClass: S.String,
  networkRung: S.Boolean,
  proofBoundary: S.String,
  rung: S.Literals(['R0', 'R1', 'R2', 'R3', 'R4']),
  status: S.Literals(['retained_rehearsal', 'not_run']),
  tokenScale: S.String,
}) {}

export class TrainingModelLadderR1CloseoutCriterion extends S.Class<TrainingModelLadderR1CloseoutCriterion>(
  'TrainingModelLadderR1CloseoutCriterion',
)({
  criterionId: S.String,
  label: S.String,
  receiptAvailable: S.Boolean,
  requiredReceipt: S.String,
  sourceRefs: S.Array(S.String),
  status: S.Literal('missing_receipt'),
}) {}

export class TrainingModelLadderEconomicsGateField extends S.Class<TrainingModelLadderEconomicsGateField>(
  'TrainingModelLadderEconomicsGateField',
)({
  definition: S.String,
  fieldKey: S.String,
  requiredProvenanceLabels: S.Array(S.String),
  settledReceiptRequiredForNetworkRung: S.Boolean,
}) {}

export class TrainingModelLadderRungsProjection extends S.Class<TrainingModelLadderRungsProjection>(
  'TrainingModelLadderRungsProjection',
)({
  authorityBoundary: S.String,
  economicsGate: S.Struct({
    fieldCount: S.Int,
    fields: S.Array(TrainingModelLadderEconomicsGateField),
    formatAvailable: S.Boolean,
    formatDocRef: S.String,
    gateOutcomeAvailable: S.Boolean,
    r1PopulatedReportAvailable: S.Boolean,
    settledNetworkEconomicsAvailable: S.Boolean,
  }),
  endpoint: S.Literal(TrainingModelLadderRungsEndpoint),
  gate: S.Struct({
    clearsBlockerRefs: S.Array(S.String),
    greenGateSatisfied: S.Boolean,
    networkRungRemainingBlockerRefs: S.Array(S.String),
    publicProjectionAvailable: S.Boolean,
    r1CloseoutReceiptAvailable: S.Boolean,
    r1FullRehearsalAvailable: S.Boolean,
    r2NetworkRungReceiptAvailable: S.Boolean,
    remainingBlockerRefs: S.Array(S.String),
    rungEconomicsGateFormatAvailable: S.Boolean,
  }),
  generatedAt: S.String,
  promiseRef: S.Literal('promise:training.model_ladder.v1'),
  promiseState: S.Literal('planned'),
  registryVersion: S.Literal(PublicProductPromisesVersion),
  r1CloseoutCriteria: S.Array(TrainingModelLadderR1CloseoutCriterion),
  rungSummary: S.Struct({
    closedRungCount: S.Int,
    economicsGateFieldCount: S.Int,
    highestClosedRung: S.Literal('R0'),
    nextRequiredRung: S.Literal('R1'),
    r1CloseoutCriteriaCount: S.Int,
    rungCount: S.Int,
  }),
  rungs: S.Array(TrainingModelLadderRung),
  schemaVersion: S.Literal(TrainingModelLadderRungsSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('model_ladder_rung_status_projection'),
  statusLabel: S.String,
  unsafeCopy: S.String,
}) {}

export class TrainingModelLadderRungsUnsafe extends Error {
  readonly _tag = 'TrainingModelLadderRungsUnsafe'
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new TrainingModelLadderRungsUnsafe(
      `${label} contains material that is not public-safe.`,
    )
  }
}

const economicsGateFormatDocRef =
  'docs/training/2026-06-19-model-ladder-rung-economics.md'

const r1RemainingBlockerRefs = [TrainingModelLadderR1FullRehearsalBlocker]
const networkRungRemainingBlockerRefs = [TrainingModelLadderNetworkRungBlocker]

const sourceRefs = [
  economicsGateFormatDocRef,
  'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
  'docs/training/2026-06-20-training-full-pipeline-program-status.md',
  'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
  'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
  'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
]

const rungs: ReadonlyArray<TrainingModelLadderRung> = [
  new TrainingModelLadderRung({
    blockerRefs: [],
    closeoutReceiptAvailable: true,
    evidenceRefs: [
      'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
    ],
    hardwareReality: '2 Macs plus 1 RTX 4080.',
    modelClass: 'tri-host bringup',
    networkRung: false,
    proofBoundary:
      'Dispatch, checkpoint, and receipt mechanics only; not network training capability.',
    rung: 'R0',
    status: 'retained_rehearsal',
    tokenScale: 'approximately 4k tokens',
  }),
  new TrainingModelLadderRung({
    blockerRefs: r1RemainingBlockerRefs,
    closeoutReceiptAvailable: false,
    evidenceRefs: [economicsGateFormatDocRef],
    hardwareReality: 'operator-owned devices over days.',
    modelClass: 'approximately 30-50M Psion',
    networkRung: false,
    proofBoundary:
      'Operator-scale full pipeline rehearsal; no closeout receipt exists yet.',
    rung: 'R1',
    status: 'not_run',
    tokenScale: '1-5B tokens',
  }),
  new TrainingModelLadderRung({
    blockerRefs: networkRungRemainingBlockerRefs,
    closeoutReceiptAvailable: false,
    evidenceRefs: [economicsGateFormatDocRef],
    hardwareReality: 'operator plus early contributor Pylons.',
    modelClass: 'approximately 125-200M Psion',
    networkRung: true,
    proofBoundary:
      'First network pretraining rung with paid verified windows; no receipt exists yet.',
    rung: 'R2',
    status: 'not_run',
    tokenScale: '10-50B tokens',
  }),
  new TrainingModelLadderRung({
    blockerRefs: [],
    closeoutReceiptAvailable: false,
    evidenceRefs: [economicsGateFormatDocRef],
    hardwareReality: 'conditioned on R2 measured economics.',
    modelClass: 'approximately 1B Psion',
    networkRung: true,
    proofBoundary:
      'Conditioned on prior closeout receipts; no R3 promise or run is active.',
    rung: 'R3',
    status: 'not_run',
    tokenScale: '100B+ tokens',
  }),
  new TrainingModelLadderRung({
    blockerRefs: [],
    closeoutReceiptAvailable: false,
    evidenceRefs: [economicsGateFormatDocRef],
    hardwareReality: 'only if R3 economics close.',
    modelClass: 'approximately 3B class',
    networkRung: true,
    proofBoundary:
      'Not promised as an active target; priced by prior rung receipts.',
    rung: 'R4',
    status: 'not_run',
    tokenScale: '1T+ tokens',
  }),
]

const r1CloseoutCriteria: ReadonlyArray<TrainingModelLadderR1CloseoutCriterion> =
  [
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'data',
      label: 'Corpus v2 through deterministic refinery.',
      receiptAvailable: false,
      requiredReceipt:
        'source-provenance and transform-digest receipt on each shard.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'ablations',
      label: 'Three to five priority ablations, including WSD confirmation.',
      receiptAvailable: false,
      requiredReceipt:
        'comparable one-delta ablation receipts from the ablation harness.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'marathon',
      label: 'One marathon run with durable checkpoint-seal discipline.',
      receiptAvailable: false,
      requiredReceipt:
        'durable seal plus restart-or-continue decision recorded as a receipt.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'post_training',
      label: 'SFT plus at least one preference-optimization stage.',
      receiptAvailable: false,
      requiredReceipt:
        'rollout-generation and reward-grading dispatched as verified work.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'evals',
      label: 'Decontaminated eval suite against the resulting checkpoint.',
      receiptAvailable: false,
      requiredReceipt:
        'retained eval series usable as the R2 reference trajectory.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
    new TrainingModelLadderR1CloseoutCriterion({
      criterionId: 'economics_gate',
      label: 'Published economics-gate report populated for R1.',
      receiptAvailable: false,
      requiredReceipt:
        'all economics-gate fields populated with provenance labels.',
      sourceRefs: [economicsGateFormatDocRef],
      status: 'missing_receipt',
    }),
  ]

const economicsGateFields: ReadonlyArray<TrainingModelLadderEconomicsGateField> =
  [
    new TrainingModelLadderEconomicsGateField({
      definition:
        'Total cost per accepted training outcome, including verification and settlement overhead.',
      fieldKey: 'allInCostPerAcceptedOutcome',
      requiredProvenanceLabels: ['modeled', 'measured', 'settled'],
      settledReceiptRequiredForNetworkRung: true,
    }),
    new TrainingModelLadderEconomicsGateField({
      definition:
        'Contributor payout per device-hour against the relevant opportunity floor.',
      fieldKey: 'contributorPayoutPerDeviceHour',
      requiredProvenanceLabels: ['modeled', 'measured', 'settled'],
      settledReceiptRequiredForNetworkRung: true,
    }),
    new TrainingModelLadderEconomicsGateField({
      definition: 'Verification cost as a fraction of work cost.',
      fieldKey: 'verificationOverheadFraction',
      requiredProvenanceLabels: ['modeled', 'measured', 'settled'],
      settledReceiptRequiredForNetworkRung: true,
    }),
    new TrainingModelLadderEconomicsGateField({
      definition:
        'Honest fallback comparator, using a rented small cluster for R2 and above.',
      fieldKey: 'fallbackComparator',
      requiredProvenanceLabels: ['modeled', 'measured'],
      settledReceiptRequiredForNetworkRung: false,
    }),
    new TrainingModelLadderEconomicsGateField({
      definition: 'pass, fail, or fail-twice outcome for the rung gate.',
      fieldKey: 'gateOutcome',
      requiredProvenanceLabels: [],
      settledReceiptRequiredForNetworkRung: false,
    }),
  ]

export const projectTrainingModelLadderRungs = (
  input: { generatedAt?: string | undefined } = {},
): TrainingModelLadderRungsProjection => {
  const projection = new TrainingModelLadderRungsProjection({
    authorityBoundary:
      'Read-only public model-ladder rung status projection for training.model_ladder.v1. It exposes the retained R0 rehearsal, the published R1 closeout criteria, and the economics-gate format only; it grants no training dispatch, spend, settlement, model promotion, schedule commitment, network-training claim, capability claim, or green product-promise authority.',
    economicsGate: {
      fieldCount: economicsGateFields.length,
      fields: [...economicsGateFields],
      formatAvailable: true,
      formatDocRef: economicsGateFormatDocRef,
      gateOutcomeAvailable: false,
      r1PopulatedReportAvailable: false,
      settledNetworkEconomicsAvailable: false,
    },
    endpoint: TrainingModelLadderRungsEndpoint,
    gate: {
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      networkRungRemainingBlockerRefs,
      publicProjectionAvailable: true,
      r1CloseoutReceiptAvailable: false,
      r1FullRehearsalAvailable: false,
      r2NetworkRungReceiptAvailable: false,
      remainingBlockerRefs: r1RemainingBlockerRefs,
      rungEconomicsGateFormatAvailable: true,
    },
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    promiseRef: 'promise:training.model_ladder.v1',
    promiseState: 'planned',
    registryVersion: PublicProductPromisesVersion,
    r1CloseoutCriteria: [...r1CloseoutCriteria],
    rungSummary: {
      closedRungCount: 1,
      economicsGateFieldCount: economicsGateFields.length,
      highestClosedRung: 'R0',
      nextRequiredRung: 'R1',
      r1CloseoutCriteriaCount: r1CloseoutCriteria.length,
      rungCount: rungs.length,
    },
    rungs: [...rungs],
    schemaVersion: TrainingModelLadderRungsSchemaVersion,
    sourceRefs,
    staleness: TrainingModelLadderRungsStaleness,
    status: 'model_ladder_rung_status_projection',
    statusLabel:
      'R0 is retained and the economics-gate format is published; R1 closeout and R2 network-rung receipts are absent.',
    unsafeCopy:
      'Do not claim any Psion rung above R0 is trained, in progress, scheduled, or economically proven; do not present R0 throughput as network training capability; do not claim the first real model-training network rung exists.',
  })

  assertPublicSafeValue('Training model ladder rungs projection', projection)

  return projection
}
