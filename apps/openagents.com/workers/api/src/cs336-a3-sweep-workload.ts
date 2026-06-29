/**
 * Bounded CS336 A3 scaling-sweep cell workload (issue #4679).
 *
 * Each cell is one real bounded training run at a planner-chosen (N, D)
 * IsoFLOP grid point: a seeded factorized bigram language model trained
 * by single-pass SGD over a deterministic synthetic next-token stream,
 * with the final held-out cross-entropy loss as the cell measurement
 * and a SHA-256 digest over the exact cell output as the deterministic
 * commitment the `deterministic_recompute` sampled re-runs check.
 *
 * The grid mirrors the Psionic `psion_cs336_a3_scaling_reference_v1`
 * planner contract: geometric N spacing within declared bounds per
 * compute budget and `D = C / (6 N)` per cell. Dispatch realizes each
 * continuous planned N as the nearest integer factor rank so a real
 * model can train at that point; the realized (N, D) pair is the cell
 * truth the fit consumes.
 *
 * Provenance binds to the A1 trainer (plan step 3): the synthetic
 * stream seed derives from the A1 tokenizer shard digest from
 * `cs336-a1-homework-workload.ts`, so every A3 cell is downstream of
 * the same committed corpus pipeline. The payloads are public-safe by
 * construction: counts, losses, digests, and refs only.
 */

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'

export class Cs336A3SweepWorkloadError extends Error {
  readonly _tag = 'Cs336A3SweepWorkloadError'
}

export const Cs336A3SweepWorkloadRef =
  'workload.cs336_a3.seeded_factored_bigram_lm.v1'
export const Cs336A3SweepDatasetRef =
  'dataset.cs336_a3.seeded_synthetic_bigram_stream.v1'
export const Cs336A3FlopsPerParameterDataUnit = 6

const Cs336A3VocabularySize = 256
const Cs336A3ParametersPerRank = 2 * Cs336A3VocabularySize
const Cs336A3EvaluationDataUnits = 2_048
const Cs336A3LearningRate = 0.3

export const Cs336A3DefaultSweepBudgetsFlops = [
  300_000_000, 600_000_000, 1_200_000_000, 2_400_000_000,
] as const
export const Cs336A3DefaultCellsPerBudget = 6
export const Cs336A3DefaultParametersMin = 1_024
export const Cs336A3DefaultParametersMax = 65_536

export type Cs336A3PlannedCell = Readonly<{
  budgetIndex: number
  cellIndex: number
  computeBudgetFlops: number
  parameterCount: number
  plannedParameterCount: number
  rank: number
  tokenCount: number
}>

export type Cs336A3CellResult = Readonly<{
  computeBudgetFlops: number
  elapsedMs: number
  finalLoss: number
  initialLoss: number
  outputDigestHex: string
  parameterCount: number
  rank: number
  tokenCount: number
  trainedDataUnits: number
  workloadRef: typeof Cs336A3SweepWorkloadRef
}>

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const fnv1a32 = (value: string): number => {
  let hash = 0x81_1c_9d_c5

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01_00_01_93)
  }

  return hash >>> 0
}

/** Deterministic mulberry32 PRNG over a string-derived seed. */
const seededRandom = (seed: string): (() => number) => {
  let state = fnv1a32(seed)

  return () => {
    state = (state + 0x6d_2b_79_f5) >>> 0
    let mixed = state
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1)
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61)

    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296
  }
}

/**
 * Deterministic synthetic next-token stream: a structured bigram
 * mixture (two affine successor maps plus uniform noise) over the
 * bounded vocabulary. The achievable loss floor depends on model
 * capacity, which is what gives each IsoFLOP budget its interior
 * loss minimum across the N grid.
 */
const makeStream = (seed: string): (() => readonly [number, number]) => {
  const random = seededRandom(seed)
  let current = Math.floor(random() * Cs336A3VocabularySize)

  return () => {
    const previous = current
    const draw = random()

    if (draw < 0.45) {
      current = (3 * previous + 11) % Cs336A3VocabularySize
    } else if (draw < 0.75) {
      current = (5 * previous + 29) % Cs336A3VocabularySize
    } else {
      current = Math.floor(random() * Cs336A3VocabularySize)
    }

    return [previous, current]
  }
}

/**
 * Plans the IsoFLOP sweep grid mirroring the Psionic
 * `cs336_a3_plan_isoflop_sweep` contract (geometric N spacing,
 * `D = C / (6 N)`), then realizes each continuous planned N as the
 * nearest integer factor rank with its realized token budget.
 */
export const planCs336A3SweepGrid = (
  input?: Readonly<{
    budgetsFlops?: ReadonlyArray<number>
    cellsPerBudget?: number
    parametersMax?: number
    parametersMin?: number
  }>,
): ReadonlyArray<Cs336A3PlannedCell> => {
  const budgets = input?.budgetsFlops ?? Cs336A3DefaultSweepBudgetsFlops
  const cellsPerBudget = input?.cellsPerBudget ?? Cs336A3DefaultCellsPerBudget
  const parametersMin = input?.parametersMin ?? Cs336A3DefaultParametersMin
  const parametersMax = input?.parametersMax ?? Cs336A3DefaultParametersMax

  if (budgets.length === 0 || budgets.some(budget => budget <= 0)) {
    throw new Cs336A3SweepWorkloadError('CS336 A3 sweep budgets must be positive.')
  }

  if (cellsPerBudget < 3) {
    throw new Cs336A3SweepWorkloadError('CS336 A3 sweep needs at least 3 cells per budget.')
  }

  if (parametersMin <= 0 || parametersMax <= parametersMin) {
    throw new Cs336A3SweepWorkloadError('CS336 A3 sweep parameter bounds are invalid.')
  }

  const logMin = Math.log(parametersMin)
  const logMax = Math.log(parametersMax)
  const cells: Cs336A3PlannedCell[] = []

  budgets.forEach((computeBudgetFlops, budgetIndex) => {
    for (let index = 0; index < cellsPerBudget; index += 1) {
      const fraction = index / (cellsPerBudget - 1)
      const plannedParameterCount = Math.exp(
        logMin + fraction * (logMax - logMin),
      )
      const rank = Math.max(
        1,
        Math.round(plannedParameterCount / Cs336A3ParametersPerRank),
      )
      const parameterCount = rank * Cs336A3ParametersPerRank

      cells.push({
        budgetIndex,
        cellIndex: cells.length + 1,
        computeBudgetFlops,
        parameterCount,
        plannedParameterCount,
        rank,
        tokenCount: Math.round(
          computeBudgetFlops /
            (Cs336A3FlopsPerParameterDataUnit * parameterCount),
        ),
      })
    }
  })

  return cells
}

const evaluateLoss = (
  inputFactors: Float64Array,
  outputFactors: Float64Array,
  rank: number,
  streamSeed: string,
): number => {
  const next = makeStream(streamSeed)
  const logits = new Float64Array(Cs336A3VocabularySize)
  let total = 0

  for (let step = 0; step < Cs336A3EvaluationDataUnits; step += 1) {
    const [context, target] = next()
    let maxLogit = -Infinity

    for (let row = 0; row < Cs336A3VocabularySize; row += 1) {
      let dot = 0

      for (let component = 0; component < rank; component += 1) {
        dot +=
          outputFactors[row * rank + component]! *
          inputFactors[context * rank + component]!
      }

      logits[row] = dot

      if (dot > maxLogit) {
        maxLogit = dot
      }
    }

    let normalizer = 0

    for (let row = 0; row < Cs336A3VocabularySize; row += 1) {
      normalizer += Math.exp(logits[row]! - maxLogit)
    }

    total += Math.log(normalizer) - (logits[target]! - maxLogit)
  }

  return total / Cs336A3EvaluationDataUnits
}

/**
 * Trains one sweep cell: single-pass SGD over `D = C / (6 N)` data
 * units of the seeded stream with the factorized bigram model, then
 * measures held-out cross-entropy. Fully deterministic for a given
 * (budget, rank) pair, which is what the sampled-cell
 * `deterministic_recompute` re-runs rely on.
 */
export const runCs336A3SweepCell = async (
  cell: Readonly<{ computeBudgetFlops: number; rank: number }>,
): Promise<Cs336A3CellResult> => {
  if (cell.computeBudgetFlops <= 0 || !Number.isFinite(cell.computeBudgetFlops)) {
    throw new Cs336A3SweepWorkloadError('CS336 A3 cell compute budget must be positive.')
  }

  if (!Number.isInteger(cell.rank) || cell.rank < 1) {
    throw new Cs336A3SweepWorkloadError('CS336 A3 cell rank must be a positive integer.')
  }

  const startedAt = performance.now()
  const shard = await computeCs336A1TokenizerShard()
  const rank = cell.rank
  const parameterCount = rank * Cs336A3ParametersPerRank
  const tokenCount = Math.round(
    cell.computeBudgetFlops /
      (Cs336A3FlopsPerParameterDataUnit * parameterCount),
  )
  const initRandom = seededRandom(
    `cs336_a3.init.${shard.digestHex}.${rank}.${cell.computeBudgetFlops}`,
  )
  const inputFactors = new Float64Array(Cs336A3VocabularySize * rank)
  const outputFactors = new Float64Array(Cs336A3VocabularySize * rank)

  for (let index = 0; index < inputFactors.length; index += 1) {
    inputFactors[index] = (initRandom() - 0.5) * 0.2
    outputFactors[index] = (initRandom() - 0.5) * 0.2
  }

  const evaluationSeed = `cs336_a3.eval.${shard.digestHex}`
  const initialLoss = evaluateLoss(
    inputFactors,
    outputFactors,
    rank,
    evaluationSeed,
  )
  const next = makeStream(`cs336_a3.train.${shard.digestHex}`)
  const logits = new Float64Array(Cs336A3VocabularySize)
  const probabilities = new Float64Array(Cs336A3VocabularySize)
  const contextGradient = new Float64Array(rank)

  for (let step = 0; step < tokenCount; step += 1) {
    const [context, target] = next()
    let maxLogit = -Infinity

    for (let row = 0; row < Cs336A3VocabularySize; row += 1) {
      let dot = 0

      for (let component = 0; component < rank; component += 1) {
        dot +=
          outputFactors[row * rank + component]! *
          inputFactors[context * rank + component]!
      }

      logits[row] = dot

      if (dot > maxLogit) {
        maxLogit = dot
      }
    }

    let normalizer = 0

    for (let row = 0; row < Cs336A3VocabularySize; row += 1) {
      const value = Math.exp(logits[row]! - maxLogit)

      probabilities[row] = value
      normalizer += value
    }

    contextGradient.fill(0)

    for (let row = 0; row < Cs336A3VocabularySize; row += 1) {
      const error =
        probabilities[row]! / normalizer - (row === target ? 1 : 0)

      for (let component = 0; component < rank; component += 1) {
        const offset = row * rank + component

        contextGradient[component] =
          contextGradient[component]! + error * outputFactors[offset]!
        outputFactors[offset] =
          outputFactors[offset]! -
          Cs336A3LearningRate *
            error *
            inputFactors[context * rank + component]!
      }
    }

    for (let component = 0; component < rank; component += 1) {
      const offset = context * rank + component

      inputFactors[offset] =
        inputFactors[offset]! -
        Cs336A3LearningRate * contextGradient[component]!
    }
  }

  const finalLoss = evaluateLoss(
    inputFactors,
    outputFactors,
    rank,
    evaluationSeed,
  )
  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      computeBudgetFlops: cell.computeBudgetFlops,
      finalLoss: finalLoss.toPrecision(17),
      initialLoss: initialLoss.toPrecision(17),
      parameterCount,
      rank,
      shardDigestHex: shard.digestHex,
      tokenCount,
      workloadRef: Cs336A3SweepWorkloadRef,
    }),
  )

  return {
    computeBudgetFlops: cell.computeBudgetFlops,
    elapsedMs: performance.now() - startedAt,
    finalLoss,
    initialLoss,
    outputDigestHex,
    parameterCount,
    rank,
    tokenCount,
    trainedDataUnits: tokenCount,
    workloadRef: Cs336A3SweepWorkloadRef,
  }
}
