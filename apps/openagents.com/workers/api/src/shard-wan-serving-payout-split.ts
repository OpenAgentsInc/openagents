import { Schema as S } from 'effect'

/**
 * Product-layer per-stage payout split for the decentralized serving fabric's
 * shard-WAN large-model lane (promise inference.decentralized_serving_fabric.v1).
 *
 * This is the deterministic apportionment + validation the gateway revenue loop
 * runs AGAINST a Psionic-emitted `psionic.serve.pipeline_sharded_run_receipt.v1`
 * receipt. It honors the Psionic boundary: Psionic owns sharded execution and
 * emits the receipt; this product-layer code only meters, validates the
 * born-verified payment gate, and splits the contributor cut across the stages
 * that served each layer-block. It dispatches NO money — the first real
 * serving-node Bitcoin payout stays owner-armed under RL-2/RL-3.
 *
 * Advances blocker.product_promises.shard_wan_large_model_serving_psionic_planned
 * by implementing the product-side payout-split hook the promise verification
 * names ("the per-stage payout split is implemented against
 * psionic.serve.pipeline_sharded_run_receipt.v1"). It does NOT make the
 * promise green: there is no real Psionic shard-WAN receipt yet (that path is
 * Psionic-planned / hardware-blocked), no served gateway request, and no
 * settled payout.
 */

export const ShardWanShardedRunReceiptSchemaVersion =
  'psionic.serve.pipeline_sharded_run_receipt.v1'

export const ShardWanServingPayoutDecisionSchemaVersion =
  'openagents.inference.shard_wan_serving_payout_split.v1'

export const ShardWanLargeModelServingBlocker =
  'blocker.product_promises.shard_wan_large_model_serving_psionic_planned'

export const ShardWanServingFirstPayoutOwnerArmedBlocker =
  'blocker.product_promises.inference_serving_first_real_payout_owner_armed'

/**
 * One serving stage from the sharded-run receipt: the node held a contiguous
 * layer-block [layerStart, layerEnd) resident on its GPU and executed it.
 * Node/owner/payout-target identity is intentionally NOT modeled here — this
 * split operates on public-safe layer-range facts only; identity binding and
 * settlement live in the RL spine.
 */
export class ShardWanServingStage extends S.Class<ShardWanServingStage>(
  'ShardWanServingStage',
)({
  gpuResident: S.Boolean,
  layerEnd: S.Int,
  layerStart: S.Int,
  stageIndex: S.Int,
}) {}

/**
 * Product-side typed view of `psionic.serve.pipeline_sharded_run_receipt.v1`.
 * Public-safe subset: it carries the apportionment inputs (which stage served
 * which layer-block) and the born-verified payment gate (exact-greedy parity),
 * not secrets, identity, or raw activations.
 */
export class ShardWanShardedRunReceipt extends S.Class<ShardWanShardedRunReceipt>(
  'ShardWanShardedRunReceipt',
)({
  modelArtifactDigest: S.String,
  parityMode: S.Literals(['verified', 'no_reference', 'mismatch']),
  schemaVersion: S.Literal(ShardWanShardedRunReceiptSchemaVersion),
  stages: S.Array(ShardWanServingStage),
  totalLayerCount: S.Int,
}) {}

export class ShardWanStageSplit extends S.Class<ShardWanStageSplit>(
  'ShardWanStageSplit',
)({
  layerCount: S.Int,
  payoutSats: S.Int,
  stageIndex: S.Int,
  weightBps: S.Int,
}) {}

export const ShardWanServingPayoutGate = [
  'pay_against_verified_parity',
  'flagged_no_reference_default_hold',
  'rejected_parity_mismatch',
  'rejected_invalid_receipt',
] as const

export class ShardWanServingPayoutDecision extends S.Class<ShardWanServingPayoutDecision>(
  'ShardWanServingPayoutDecision',
)({
  blockerRef: S.Literal(ShardWanLargeModelServingBlocker),
  contributorCutSats: S.Int,
  ownerArmedRequired: S.Literal(true),
  payable: S.Boolean,
  payoutGate: S.Literals(ShardWanServingPayoutGate),
  promiseRef: S.Literal('promise:inference.decentralized_serving_fabric.v1'),
  receiptValid: S.Boolean,
  schemaVersion: S.Literal(ShardWanServingPayoutDecisionSchemaVersion),
  split: S.NullOr(S.Array(ShardWanStageSplit)),
  totalSplitSats: S.Int,
  validationErrors: S.Array(S.String),
  weightingRule: S.Literal('per_layer_block'),
}) {}

const decodeReceipt = S.decodeUnknownOption(ShardWanShardedRunReceipt)

const rejected = (
  contributorCutSats: number,
  validationErrors: ReadonlyArray<string>,
  payoutGate: (typeof ShardWanServingPayoutGate)[number],
): ShardWanServingPayoutDecision =>
  new ShardWanServingPayoutDecision({
    blockerRef: ShardWanLargeModelServingBlocker,
    contributorCutSats: Number.isSafeInteger(contributorCutSats)
      ? contributorCutSats
      : 0,
    ownerArmedRequired: true,
    payable: false,
    payoutGate,
    promiseRef: 'promise:inference.decentralized_serving_fabric.v1',
    receiptValid: false,
    schemaVersion: ShardWanServingPayoutDecisionSchemaVersion,
    split: null,
    totalSplitSats: 0,
    validationErrors: [...validationErrors],
    weightingRule: 'per_layer_block',
  })

/**
 * Validate the shard-WAN structural invariants the design doc requires before a
 * sharded run is payable:
 *  - more than one admitted stage (a real split, not a single node)
 *  - every stage holds its layers GPU-resident (no whole-model fallback faking)
 *  - layer-blocks are contiguous, gap-free, overlap-free, and cover the model
 *  - no single stage covers the whole model (again: not a fake split)
 */
const collectStructuralErrors = (
  receipt: ShardWanShardedRunReceipt,
): ReadonlyArray<string> => {
  const errors: Array<string> = []

  if (receipt.totalLayerCount <= 0) {
    errors.push('totalLayerCount must be a positive integer.')
  }
  if (receipt.stages.length < 2) {
    errors.push(
      'a sharded run must record more than one serving stage (no single-node split).',
    )
  }

  const sorted = [...receipt.stages].sort((a, b) => a.layerStart - b.layerStart)
  let cursor = 0
  for (const stage of sorted) {
    if (!stage.gpuResident) {
      errors.push(
        `stage ${stage.stageIndex} did not hold its layer-block GPU-resident (no whole-model fallback).`,
      )
    }
    if (stage.layerStart < 0 || stage.layerEnd > receipt.totalLayerCount) {
      errors.push(
        `stage ${stage.stageIndex} layer range [${stage.layerStart}, ${stage.layerEnd}) is outside [0, ${receipt.totalLayerCount}).`,
      )
    }
    if (stage.layerStart >= stage.layerEnd) {
      errors.push(
        `stage ${stage.stageIndex} has an empty or inverted layer range [${stage.layerStart}, ${stage.layerEnd}).`,
      )
    }
    if (
      stage.layerStart === 0 &&
      stage.layerEnd === receipt.totalLayerCount &&
      receipt.totalLayerCount > 0
    ) {
      errors.push(
        `stage ${stage.stageIndex} covers the whole model — that is not a layer-block split.`,
      )
    }
    if (stage.layerStart > cursor) {
      errors.push(
        `gap in layer coverage before stage ${stage.stageIndex}: layers [${cursor}, ${stage.layerStart}) unserved.`,
      )
    }
    if (stage.layerStart < cursor) {
      errors.push(
        `overlap in layer coverage at stage ${stage.stageIndex}: layers [${stage.layerStart}, ${cursor}) double-served.`,
      )
    }
    cursor = Math.max(cursor, stage.layerEnd)
  }
  if (errors.length === 0 && cursor !== receipt.totalLayerCount) {
    errors.push(
      `layer coverage stops at ${cursor} but the model has ${receipt.totalLayerCount} layers.`,
    )
  }

  return errors
}

/**
 * Per-layer-block apportionment with the largest-remainder method so the split
 * sums EXACTLY to the contributor cut (no sat created or lost). Ties on the
 * fractional remainder break toward the lower stageIndex for determinism.
 */
const apportionByLayerBlock = (
  receipt: ShardWanShardedRunReceipt,
  contributorCutSats: number,
): ReadonlyArray<ShardWanStageSplit> => {
  const sorted = [...receipt.stages].sort((a, b) => a.stageIndex - b.stageIndex)
  const layerCounts = sorted.map((stage) => stage.layerEnd - stage.layerStart)
  const totalLayers = layerCounts.reduce((sum, count) => sum + count, 0)

  const base = sorted.map((stage, index) => {
    const layerCount = layerCounts[index] ?? 0
    const raw = contributorCutSats * layerCount
    const floorSats = Math.floor(raw / totalLayers)
    const remainder = raw - floorSats * totalLayers
    return {
      floorSats,
      layerCount,
      remainder,
      stageIndex: stage.stageIndex,
      weightBps: Math.round((layerCount / totalLayers) * 10000),
    }
  })

  const distributed = base.reduce((sum, entry) => sum + entry.floorSats, 0)
  let leftover = contributorCutSats - distributed

  const remainderOrder = [...base].sort((a, b) =>
    b.remainder !== a.remainder
      ? b.remainder - a.remainder
      : a.stageIndex - b.stageIndex,
  )
  const bonus = new Set<number>()
  for (const entry of remainderOrder) {
    if (leftover <= 0) break
    bonus.add(entry.stageIndex)
    leftover -= 1
  }

  return base.map(
    (entry) =>
      new ShardWanStageSplit({
        layerCount: entry.layerCount,
        payoutSats: entry.floorSats + (bonus.has(entry.stageIndex) ? 1 : 0),
        stageIndex: entry.stageIndex,
        weightBps: entry.weightBps,
      }),
  )
}

/**
 * Evaluate a sharded-run receipt and return the deterministic per-stage payout
 * split decision. Pure: no I/O, no settlement, no money movement. `payable` is
 * a NECESSARY gate, never a sufficient one — the first real dispatched payout is
 * owner-armed (ownerArmedRequired is always true).
 */
export const evaluateShardWanServingPayout = (input: {
  contributorCutSats: number
  receipt: unknown
}): ShardWanServingPayoutDecision => {
  const { contributorCutSats } = input

  if (
    !Number.isSafeInteger(contributorCutSats) ||
    contributorCutSats < 0
  ) {
    return rejected(
      contributorCutSats,
      ['contributorCutSats must be a non-negative integer.'],
      'rejected_invalid_receipt',
    )
  }

  const decoded = decodeReceipt(input.receipt)
  if (decoded._tag === 'None') {
    return rejected(
      contributorCutSats,
      ['receipt did not decode as psionic.serve.pipeline_sharded_run_receipt.v1.'],
      'rejected_invalid_receipt',
    )
  }
  const receipt = decoded.value

  const structuralErrors = collectStructuralErrors(receipt)
  if (structuralErrors.length > 0) {
    return rejected(
      contributorCutSats,
      structuralErrors,
      'rejected_invalid_receipt',
    )
  }

  if (receipt.parityMode === 'mismatch') {
    return new ShardWanServingPayoutDecision({
      blockerRef: ShardWanLargeModelServingBlocker,
      contributorCutSats,
      ownerArmedRequired: true,
      payable: false,
      payoutGate: 'rejected_parity_mismatch',
      promiseRef: 'promise:inference.decentralized_serving_fabric.v1',
      receiptValid: true,
      schemaVersion: ShardWanServingPayoutDecisionSchemaVersion,
      split: null,
      totalSplitSats: 0,
      validationErrors: [
        'exact-greedy parity check failed (tokens diverged from the reference greedy decode).',
      ],
      weightingRule: 'per_layer_block',
    })
  }

  if (receipt.parityMode === 'no_reference') {
    // Born-verified is the payment gate: with no feasible same-engine
    // reference, default to HOLD rather than paying against self-report.
    return new ShardWanServingPayoutDecision({
      blockerRef: ShardWanLargeModelServingBlocker,
      contributorCutSats,
      ownerArmedRequired: true,
      payable: false,
      payoutGate: 'flagged_no_reference_default_hold',
      promiseRef: 'promise:inference.decentralized_serving_fabric.v1',
      receiptValid: true,
      schemaVersion: ShardWanServingPayoutDecisionSchemaVersion,
      split: null,
      totalSplitSats: 0,
      validationErrors: [],
      weightingRule: 'per_layer_block',
    })
  }

  const split = apportionByLayerBlock(receipt, contributorCutSats)
  const totalSplitSats = split.reduce((sum, entry) => sum + entry.payoutSats, 0)

  return new ShardWanServingPayoutDecision({
    blockerRef: ShardWanLargeModelServingBlocker,
    contributorCutSats,
    ownerArmedRequired: true,
    payable: true,
    payoutGate: 'pay_against_verified_parity',
    promiseRef: 'promise:inference.decentralized_serving_fabric.v1',
    receiptValid: true,
    schemaVersion: ShardWanServingPayoutDecisionSchemaVersion,
    split: [...split],
    totalSplitSats,
    validationErrors: [],
    weightingRule: 'per_layer_block',
  })
}
