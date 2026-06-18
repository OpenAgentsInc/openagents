import {
  type TassadarAlmNumericModel,
  type TassadarNumericAttentionRow,
  type TassadarNumericFfnRow,
  type TassadarNumericTrace,
  type TassadarNumericWiringRow,
  executeTassadarNumericModel,
} from "./numeric-executor.js"

export const TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND =
  "tassadar_alm_dense_weight_module.v1"

export type TassadarDenseWiringBlock = Readonly<{
  phase: number
  outSlots: ReadonlyArray<number>
  wResidual: ReadonlyArray<ReadonlyArray<number>>
  bias: ReadonlyArray<number>
  inputFields: ReadonlyArray<number | null>
}>

export type TassadarDenseAttentionHead =
  | Readonly<{
      keyedRead: Readonly<{
        channel: number
        query_slot: number
        out_slot: number
      }>
    }>
  | Readonly<{
      cumSum: Readonly<{
        channel: number
        value_slot: number
        out_slot: number
      }>
    }>

export type TassadarDenseAttentionBlock = Readonly<{
  phase: number
  heads: ReadonlyArray<TassadarDenseAttentionHead>
  wQ: ReadonlyArray<ReadonlyArray<number>>
  wK: ReadonlyArray<ReadonlyArray<number>>
  wV: ReadonlyArray<ReadonlyArray<number>>
  wO: ReadonlyArray<ReadonlyArray<number>>
}>

export type TassadarDenseFfnBlock = Readonly<{
  phase: number
  wValue: ReadonlyArray<ReadonlyArray<number>>
  wGate: ReadonlyArray<ReadonlyArray<number>>
  wOut: ReadonlyArray<ReadonlyArray<number>>
}>

export type TassadarDenseWriteRow = Readonly<{
  channel: number
  key_slot: number
  value_slot: number
}>

export type TassadarDenseWeightModule = Readonly<{
  schemaVersion: number
  moduleKind: typeof TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND
  moduleId: string
  sourceModelId: string
  sourceModelDigest: string
  graphDigest: string
  bundleDigest: string
  inputFieldCount: number
  dModel: number
  layerCount: number
  seedWrites: ReadonlyArray<readonly [number, number, number]>
  wiringBlocks: ReadonlyArray<TassadarDenseWiringBlock>
  attentionBlocks: ReadonlyArray<TassadarDenseAttentionBlock>
  ffnBlocks: ReadonlyArray<TassadarDenseFfnBlock>
  writeRows: ReadonlyArray<TassadarDenseWriteRow>
  outputSlots: ReadonlyArray<number>
  claimBoundary: string
}>

export type TassadarDenseProgramFixture = Readonly<{
  schemaVersion: number
  fixtureId: string
  generatedBy: string
  claimBoundary: string
  programId: string
  programDigest: string
  workloadKind: string
  profileId: string
  numericModelDigest: string
  denseModuleDigest: string
  denseModule: TassadarDenseWeightModule
  steps: ReadonlyArray<ReadonlyArray<number>>
  expectedTraceDigest: string
  expectedFinalRow: ReadonlyArray<number> | null
  expectedOutputs: ReadonlyArray<number>
  halted: boolean
  compileReceiptRefs: ReadonlyArray<string>
  runArtifactRefs: ReadonlyArray<string>
}>

export class TassadarDenseModuleError extends Error {
  readonly field: string

  constructor(field: string, detail: string) {
    super(`${field}: ${detail}`)
    this.field = field
  }
}

const validateLength = (
  field: string,
  found: number,
  expected: number,
): void => {
  if (found !== expected) {
    throw new TassadarDenseModuleError(
      field,
      `dimension ${found} does not match expected ${expected}`,
    )
  }
}

const oneHotRowSlot = (
  row: ReadonlyArray<number>,
  field: string,
): number => {
  let slot: number | null = null
  for (let index = 0; index < row.length; index += 1) {
    const value = row[index] ?? 0
    if (value === 1 && slot === null) {
      slot = index
    } else if (value !== 0) {
      throw new TassadarDenseModuleError(field, "row is not one-hot")
    }
  }
  if (slot === null) {
    throw new TassadarDenseModuleError(field, "row is not one-hot")
  }
  return slot
}

const validateZeroRow = (
  row: ReadonlyArray<number>,
  field: string,
): void => {
  if (row.some(value => value !== 0)) {
    throw new TassadarDenseModuleError(field, "row is not zero")
  }
}

const oneHotColumnSlot = (
  matrix: ReadonlyArray<ReadonlyArray<number>>,
  column: number,
  field: string,
): number => {
  let slot: number | null = null
  for (let rowIndex = 0; rowIndex < matrix.length; rowIndex += 1) {
    const row = matrix[rowIndex] ?? []
    const value = row[column] ?? 0
    if (value === 1 && slot === null) {
      slot = rowIndex
    } else if (value !== 0) {
      throw new TassadarDenseModuleError(field, "column is not one-hot")
    }
  }
  if (slot === null) {
    throw new TassadarDenseModuleError(field, "column is not one-hot")
  }
  return slot
}

export const denseWeightModuleToNumericModel = (
  module: TassadarDenseWeightModule,
): TassadarAlmNumericModel => {
  if (module.moduleKind !== TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND) {
    throw new TassadarDenseModuleError("moduleKind", "unsupported module kind")
  }
  const dModel = module.dModel
  const wiring: TassadarNumericWiringRow[] = []
  for (const block of module.wiringBlocks) {
    validateLength("wiring.outSlots", block.outSlots.length, block.wResidual.length)
    validateLength("wiring.bias", block.bias.length, block.wResidual.length)
    validateLength("wiring.inputFields", block.inputFields.length, block.wResidual.length)
    block.wResidual.forEach((row, rowIndex) => {
      validateLength("wiring.wResidual.row", row.length, dModel)
      wiring.push({
        bias: block.bias[rowIndex] ?? 0,
        input_field: block.inputFields[rowIndex] ?? null,
        out_slot: block.outSlots[rowIndex] ?? 0,
        phase: block.phase,
        terms: row.flatMap((coefficient, slot) =>
          coefficient === 0 ? [] : ([[coefficient, slot]] as const),
        ),
      })
    })
  }

  const attention: TassadarNumericAttentionRow[] = []
  for (const block of module.attentionBlocks) {
    const headCount = block.heads.length
    validateLength("attention.wQ", block.wQ.length, headCount)
    validateLength("attention.wK", block.wK.length, headCount)
    validateLength("attention.wV", block.wV.length, headCount)
    validateLength("attention.wO", block.wO.length, dModel)
    block.wO.forEach(row => validateLength("attention.wO.row", row.length, headCount))
    block.heads.forEach((head, headIndex) => {
      const q = block.wQ[headIndex] ?? []
      const k = block.wK[headIndex] ?? []
      const v = block.wV[headIndex] ?? []
      validateLength("attention.wQ.row", q.length, dModel)
      validateLength("attention.wK.row", k.length, dModel)
      validateLength("attention.wV.row", v.length, dModel)
      validateZeroRow(k, "attention.wK")
      const outSlot = oneHotColumnSlot(block.wO, headIndex, "attention.wO")
      if ("keyedRead" in head) {
        const querySlot = oneHotRowSlot(q, "attention.wQ")
        validateZeroRow(v, "attention.wV.keyedRead")
        if (querySlot !== head.keyedRead.query_slot || outSlot !== head.keyedRead.out_slot) {
          throw new TassadarDenseModuleError("attention.keyedRead", "descriptor mismatch")
        }
        attention.push({
          keyed_read: {
            channel: head.keyedRead.channel,
            out_slot: outSlot,
            phase: block.phase,
            query_slot: querySlot,
          },
        })
      } else {
        validateZeroRow(q, "attention.wQ.cumSum")
        const valueSlot = oneHotRowSlot(v, "attention.wV")
        if (valueSlot !== head.cumSum.value_slot || outSlot !== head.cumSum.out_slot) {
          throw new TassadarDenseModuleError("attention.cumSum", "descriptor mismatch")
        }
        attention.push({
          cum_sum: {
            channel: head.cumSum.channel,
            out_slot: outSlot,
            phase: block.phase,
            value_slot: valueSlot,
          },
        })
      }
    })
  }

  const ffn: TassadarNumericFfnRow[] = []
  for (const block of module.ffnBlocks) {
    const neuronCount = block.wValue.length
    validateLength("ffn.wGate", block.wGate.length, neuronCount)
    validateLength("ffn.wOut", block.wOut.length, dModel)
    block.wOut.forEach(row => validateLength("ffn.wOut.row", row.length, neuronCount))
    for (let neuron = 0; neuron < neuronCount; neuron += 1) {
      const valueRow = block.wValue[neuron] ?? []
      const gateRow = block.wGate[neuron] ?? []
      validateLength("ffn.wValue.row", valueRow.length, dModel)
      validateLength("ffn.wGate.row", gateRow.length, dModel)
      ffn.push({
        gate_slot: oneHotRowSlot(gateRow, "ffn.wGate"),
        out_slot: oneHotColumnSlot(block.wOut, neuron, "ffn.wOut"),
        phase: block.phase,
        value_slot: oneHotRowSlot(valueRow, "ffn.wValue"),
      })
    }
  }

  return {
    attention,
    bundle_digest: module.bundleDigest,
    ffn,
    graph_digest: module.graphDigest,
    input_field_count: module.inputFieldCount,
    layer_count: module.layerCount,
    model_id: module.sourceModelId,
    output_slots: module.outputSlots,
    schema_version: 1,
    seed_writes: module.seedWrites,
    slot_count: module.dModel,
    wiring,
    writes: module.writeRows,
  }
}

export const executeTassadarDenseWeightModule = (
  module: TassadarDenseWeightModule,
  steps: ReadonlyArray<ReadonlyArray<number>>,
): Promise<TassadarNumericTrace> =>
  executeTassadarNumericModel(denseWeightModuleToNumericModel(module), steps)
