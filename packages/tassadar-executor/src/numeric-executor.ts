/**
 * TypeScript executor for the psionic Tassadar ALM numeric model format
 * (`TassadarAlmNumericModel` v1, psionic crates/psionic-compiler/src/
 * tassadar_alm_numeric.rs). The model is portable data: explicit f64
 * coefficient arrays executed with hard-max parabolic attention inside a
 * checked 2^53 exactness window. This implementation must reproduce the
 * Rust trace digest byte-for-byte; the committed fixture test is the bar.
 *
 * Claim boundary: faithful re-execution of digest-pinned compiled
 * workloads only. No softmax, no learning, no serving, no performance
 * claim against conventional CPUs.
 */

export const TASSADAR_ALM_NUMERIC_EXACT_WINDOW = 9_007_199_254_740_992 // 2^53

export type TassadarNumericWiringRow = Readonly<{
  out_slot: number
  bias: number
  terms: ReadonlyArray<readonly [number, number]>
  input_field: number | null
  phase: number
}>

export type TassadarNumericAttentionRow =
  | Readonly<{
      keyed_read: Readonly<{
        channel: number
        query_slot: number
        out_slot: number
        phase: number
      }>
    }>
  | Readonly<{
      cum_sum: Readonly<{
        channel: number
        value_slot: number
        out_slot: number
        phase: number
      }>
    }>

export type TassadarNumericFfnRow = Readonly<{
  value_slot: number
  gate_slot: number
  out_slot: number
  phase: number
}>

export type TassadarNumericWriteRow = Readonly<{
  channel: number
  key_slot: number
  value_slot: number
}>

export type TassadarAlmNumericModel = Readonly<{
  schema_version: number
  model_id: string
  graph_digest: string
  bundle_digest: string
  input_field_count: number
  slot_count: number
  layer_count: number
  seed_writes: ReadonlyArray<readonly [number, number, number]>
  wiring: ReadonlyArray<TassadarNumericWiringRow>
  attention: ReadonlyArray<TassadarNumericAttentionRow>
  ffn: ReadonlyArray<TassadarNumericFfnRow>
  writes: ReadonlyArray<TassadarNumericWriteRow>
  output_slots: ReadonlyArray<number>
}>

export type TassadarNumericExecutionRefusal = Readonly<{
  kind:
    | "exactness_window_exceeded"
    | "input_arity_mismatch"
    | "missing_key"
  step: number
  detail: string
}>

export class TassadarNumericExecutionError extends Error {
  readonly refusal: TassadarNumericExecutionRefusal

  constructor(refusal: TassadarNumericExecutionRefusal) {
    super(`${refusal.kind} at step ${refusal.step}: ${refusal.detail}`)
    this.refusal = refusal
  }
}

export type TassadarNumericTrace = Readonly<{
  executorId: "tassadar.alm_numeric_executor.ts.v1"
  graphDigest: string
  stepCount: number
  stepOutputs: ReadonlyArray<ReadonlyArray<bigint>>
  traceDigest: string
}>

type NumericPoint = { key: number; value: number; writeOrder: number }

const checkWindow = (value: number, step: number): number => {
  if (Math.abs(value) > TASSADAR_ALM_NUMERIC_EXACT_WINDOW) {
    throw new TassadarNumericExecutionError({
      detail: `value ${value} left the 2^53 exactness window`,
      kind: "exactness_window_exceeded",
      step,
    })
  }
  return value
}

/** Little-endian 8-byte two's-complement encoding matching Rust i64::to_le_bytes. */
const i64LeBytes = (value: bigint): Uint8Array => {
  const bytes = new Uint8Array(8)
  let v = BigInt.asUintN(64, value)
  for (let i = 0; i < 8; i += 1) {
    bytes[i] = Number(v & 0xffn)
    v >>= 8n
  }
  return bytes
}

const sha256Hex = async (chunks: ReadonlyArray<Uint8Array>): Promise<string> => {
  let total = 0
  for (const chunk of chunks) total += chunk.length
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }
  const digest = await crypto.subtle.digest("SHA-256", joined)
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
}

const textBytes = (value: string): Uint8Array => new TextEncoder().encode(value)

export const digestTassadarNumericTraceRows = async (
  graphDigest: string,
  rows: ReadonlyArray<ReadonlyArray<bigint>>,
): Promise<string> => {
  const chunks: Uint8Array[] = [
    textBytes("tassadar_alm_trace|"),
    textBytes(graphDigest),
  ]
  for (const row of rows) {
    chunks.push(textBytes("|row|"))
    for (const value of row) {
      chunks.push(i64LeBytes(value))
    }
  }
  return sha256Hex(chunks)
}

/**
 * Executes one numeric model over per-step input rows, reproducing the
 * Rust executor's semantics and trace digest exactly.
 */
export const executeTassadarNumericModel = async (
  model: TassadarAlmNumericModel,
  steps: ReadonlyArray<ReadonlyArray<number>>,
): Promise<TassadarNumericTrace> => {
  const points = new Map<number, NumericPoint[]>()
  const accumulators = new Map<number, number>()
  let writeOrder = 0
  const pushPoint = (channel: number, key: number, value: number) => {
    const list = points.get(channel) ?? []
    list.push({ key, value, writeOrder })
    writeOrder += 1
    points.set(channel, list)
  }
  for (const [channel, key, value] of model.seed_writes) {
    pushPoint(channel, key, value)
  }
  // Phase-ordered plan: attention (0) before wiring (1) before ffn (2)
  // within a phase, mirroring the Rust executor's sort_unstable over
  // (phase, kind, index).
  const plan: Array<readonly [number, number, number]> = []
  model.attention.forEach((row, index) => {
    const phase = "keyed_read" in row ? row.keyed_read.phase : row.cum_sum.phase
    plan.push([phase, 0, index])
  })
  model.wiring.forEach((row, index) => {
    plan.push([row.phase, 1, index])
  })
  model.ffn.forEach((row, index) => {
    plan.push([row.phase, 2, index])
  })
  plan.sort((a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2])
  const stepOutputs: bigint[][] = []
  for (let stepIndex = 0; stepIndex < steps.length; stepIndex += 1) {
    const fields = steps[stepIndex] ?? []
    if (fields.length !== model.input_field_count) {
      throw new TassadarNumericExecutionError({
        detail: `step supplies ${fields.length} fields, expected ${model.input_field_count}`,
        kind: "input_arity_mismatch",
        step: stepIndex,
      })
    }
    const residual = new Array<number>(model.slot_count).fill(0)
    for (const [, kind, index] of plan) {
      if (kind === 1) {
        const row = model.wiring[index]!
        let total = row.bias
        if (row.input_field !== null) {
          total += fields[row.input_field] ?? 0
        }
        for (const [coefficient, slot] of row.terms) {
          total += coefficient * (residual[slot] ?? 0)
        }
        residual[row.out_slot] = checkWindow(total, stepIndex)
      } else if (kind === 0) {
        const row = model.attention[index]!
        if ("keyed_read" in row) {
          const { channel, query_slot, out_slot } = row.keyed_read
          const query = residual[query_slot] ?? 0
          const channelPoints = points.get(channel) ?? []
          // Hard-max over parabolic scores 2qk - k^2; exact ties
          // (duplicate keys) break to the latest write order.
          let best: NumericPoint | null = null
          let bestScore = Number.NEGATIVE_INFINITY
          for (const point of channelPoints) {
            const score = 2 * query * point.key - point.key * point.key
            const better =
              best === null ||
              score > bestScore ||
              (score === bestScore && point.writeOrder > best.writeOrder)
            if (better) {
              best = point
              bestScore = score
            }
          }
          if (best === null || best.key !== query) {
            throw new TassadarNumericExecutionError({
              detail: `missing key ${query} on channel ${channel}`,
              kind: "missing_key",
              step: stepIndex,
            })
          }
          residual[out_slot] = best.value
        } else {
          const { channel, value_slot, out_slot } = row.cum_sum
          const total = checkWindow(
            (accumulators.get(channel) ?? 0) + (residual[value_slot] ?? 0),
            stepIndex,
          )
          accumulators.set(channel, total)
          residual[out_slot] = total
        }
      } else {
        const row = model.ffn[index]!
        const gated = Math.max(residual[row.gate_slot] ?? 0, 0)
        residual[row.out_slot] = checkWindow(
          (residual[row.value_slot] ?? 0) * gated,
          stepIndex,
        )
      }
    }
    for (const write of model.writes) {
      pushPoint(write.channel, residual[write.key_slot] ?? 0, residual[write.value_slot] ?? 0)
    }
    stepOutputs.push(model.output_slots.map((slot) => BigInt(Math.trunc(residual[slot] ?? 0))))
  }
  const traceDigest = await digestTassadarNumericTraceRows(
    model.graph_digest,
    stepOutputs,
  )
  return {
    executorId: "tassadar.alm_numeric_executor.ts.v1",
    graphDigest: model.graph_digest,
    stepCount: steps.length,
    stepOutputs,
    traceDigest,
  }
}

/** Collects (out_flag, out_value, ...) interpreter rows into emitted outputs and the halt flag. */
export const collectInterpreterOutputs = (
  stepOutputs: ReadonlyArray<ReadonlyArray<bigint>>,
): Readonly<{ outputs: ReadonlyArray<bigint>; halted: boolean }> => {
  const outputs = stepOutputs
    .filter((row) => row[0] === 1n)
    .map((row) => row[1] ?? 0n)
  const last = stepOutputs[stepOutputs.length - 1]
  return { halted: last !== undefined && (last[4] ?? 0n) >= 1n, outputs }
}
