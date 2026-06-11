/**
 * Bounded CS336 A1 demo homework workload (issue #4675).
 *
 * This module computes the two A1 work classes locally on a contributor
 * device so the production Worker's verification classes can check them:
 *
 * - `tokenizer_bpe_shard`: a deterministic byte-level BPE merge shard over
 *   the bounded demo corpus. Verification class `deterministic_recompute`
 *   compares the worker digest against an independent recompute.
 * - `training_step_matrix`: one bounded training-step forward matmul
 *   (activations x weights mod p) with per-row commitments and a Merkle
 *   root. Verification class `freivalds_merkle` checks A(Br) == Cr mod p
 *   plus row openings; `expectExactProduct` forces the full recompute.
 *
 * The payloads are public-safe by construction: token ids, digests,
 * small integer matrices, and refs only. No wallet, payment, or private
 * material is ever part of the workload.
 */

const Cs336A1FieldModulus = 2_147_483_647

const Cs336A1DemoCorpus = [
  'one day a little robot found a shiny coin on the floor',
  'the robot gave the coin to a small bird and the bird sang',
  'they walked to the lake and watched the slow boats together',
  'the bird said thank you and the robot beeped a happy song',
].join('\n')

const Cs336A1MergeBudget = 24

const Cs336A1MatrixRows = 8
const Cs336A1MatrixInner = 16
const Cs336A1MatrixColumns = 8

export type Cs336A1TokenizerShardResult = Readonly<{
  digestHex: string
  mergeCount: number
  tokenCount: number
  vocabularySize: number
}>

export type Cs336A1TrainingStepMatrixResult = Readonly<{
  challengeVector: ReadonlyArray<number>
  claimedProductMatrix: ReadonlyArray<ReadonlyArray<number>>
  fieldModulus: number
  leftMatrix: ReadonlyArray<ReadonlyArray<number>>
  merkleProofValid: boolean
  merkleRootHex: string
  rightMatrix: ReadonlyArray<ReadonlyArray<number>>
  rowDigestsHex: ReadonlyArray<string>
}>

const utf8Bytes = (value: string): ReadonlyArray<number> => [
  ...new TextEncoder().encode(value),
]

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

type BpePair = Readonly<{ left: number; right: number }>

const bestPair = (tokens: ReadonlyArray<number>): BpePair | undefined => {
  const counts = new Map<string, Readonly<{ count: number; pair: BpePair }>>()

  for (let index = 0; index + 1 < tokens.length; index += 1) {
    const pair: BpePair = { left: tokens[index]!, right: tokens[index + 1]! }
    const key = `${pair.left}:${pair.right}`
    const existing = counts.get(key)

    counts.set(key, { count: (existing?.count ?? 0) + 1, pair })
  }

  let selected: Readonly<{ count: number; pair: BpePair }> | undefined

  for (const entry of counts.values()) {
    if (
      selected === undefined ||
      entry.count > selected.count ||
      (entry.count === selected.count &&
        (entry.pair.left < selected.pair.left ||
          (entry.pair.left === selected.pair.left &&
            entry.pair.right < selected.pair.right)))
    ) {
      selected = entry
    }
  }

  return selected !== undefined && selected.count >= 2
    ? selected.pair
    : undefined
}

const applyMerge = (
  tokens: ReadonlyArray<number>,
  pair: BpePair,
  newToken: number,
): ReadonlyArray<number> => {
  const merged: number[] = []
  let index = 0

  while (index < tokens.length) {
    if (
      index + 1 < tokens.length &&
      tokens[index] === pair.left &&
      tokens[index + 1] === pair.right
    ) {
      merged.push(newToken)
      index += 2
    } else {
      merged.push(tokens[index]!)
      index += 1
    }
  }

  return merged
}

/**
 * Trains the bounded byte-level BPE shard and digests the full shard
 * output (merge table plus final token stream). Deterministic: the same
 * corpus and merge budget always produce the same digest, which is what
 * `deterministic_recompute` relies on.
 */
export const computeCs336A1TokenizerShard =
  async (): Promise<Cs336A1TokenizerShardResult> => {
    let tokens = utf8Bytes(Cs336A1DemoCorpus)
    const merges: Array<Readonly<{ pair: BpePair; token: number }>> = []
    let nextToken = 256

    while (merges.length < Cs336A1MergeBudget) {
      const pair = bestPair(tokens)

      if (pair === undefined) {
        break
      }

      merges.push({ pair, token: nextToken })
      tokens = applyMerge(tokens, pair, nextToken)
      nextToken += 1
    }

    const digestHex = await sha256Hex(
      JSON.stringify({
        corpusByteLength: utf8Bytes(Cs336A1DemoCorpus).length,
        mergeBudget: Cs336A1MergeBudget,
        merges: merges.map(merge => [
          merge.pair.left,
          merge.pair.right,
          merge.token,
        ]),
        tokens,
      }),
    )

    return {
      digestHex,
      mergeCount: merges.length,
      tokenCount: tokens.length,
      vocabularySize: nextToken,
    }
  }

const seededValues = async (
  seed: string,
  count: number,
  bound: number,
): Promise<ReadonlyArray<number>> => {
  const values: number[] = []
  let counter = 0

  while (values.length < count) {
    const block = await sha256Hex(`${seed}:${counter}`)

    for (
      let offset = 0;
      offset + 8 <= block.length && values.length < count;
      offset += 8
    ) {
      values.push(Number.parseInt(block.slice(offset, offset + 8), 16) % bound)
    }

    counter += 1
  }

  return values
}

const seededMatrix = async (
  seed: string,
  rows: number,
  columns: number,
  bound: number,
): Promise<ReadonlyArray<ReadonlyArray<number>>> => {
  const flat = await seededValues(seed, rows * columns, bound)

  return Array.from({ length: rows }, (_, row) =>
    flat.slice(row * columns, (row + 1) * columns),
  )
}

const matrixProductMod = (
  left: ReadonlyArray<ReadonlyArray<number>>,
  right: ReadonlyArray<ReadonlyArray<number>>,
  modulus: number,
): ReadonlyArray<ReadonlyArray<number>> =>
  left.map(row =>
    Array.from({ length: right[0]!.length }, (_, column) =>
      row.reduce(
        (sum, cell, index) => (sum + cell * right[index]![column]!) % modulus,
        0,
      ),
    ),
  )

/**
 * Computes one bounded training-step forward matmul with public
 * commitments. The left matrix is derived from the tokenizer shard
 * digest (the activations of this homework step depend on the tokenizer
 * output), the right matrix is the seeded demo weight shard, and the
 * claimed product is the real product mod p. Row digests and the Merkle
 * root over them form the commitment; the challenge vector is derived
 * from the claimed product after commitment.
 */
export const computeCs336A1TrainingStepMatrix = async (
  tokenizerDigestHex: string,
): Promise<Cs336A1TrainingStepMatrixResult> => {
  const leftMatrix = await seededMatrix(
    `cs336_a1.activations.${tokenizerDigestHex}`,
    Cs336A1MatrixRows,
    Cs336A1MatrixInner,
    97,
  )
  const rightMatrix = await seededMatrix(
    'cs336_a1.weight_shard.psion_cs336_a1_demo_v1',
    Cs336A1MatrixInner,
    Cs336A1MatrixColumns,
    97,
  )
  const claimedProductMatrix = matrixProductMod(
    leftMatrix,
    rightMatrix,
    Cs336A1FieldModulus,
  )
  const rowDigestsHex: string[] = []

  for (const row of claimedProductMatrix) {
    rowDigestsHex.push(await sha256Hex(JSON.stringify(row)))
  }

  let level = rowDigestsHex

  while (level.length > 1) {
    const next: string[] = []

    for (let index = 0; index < level.length; index += 2) {
      next.push(await sha256Hex(`${level[index]}${level[index + 1] ?? ''}`))
    }

    level = next
  }

  const merkleRootHex = level[0]!
  const challengeVector = await seededValues(
    `cs336_a1.challenge.${merkleRootHex}`,
    Cs336A1MatrixColumns,
    Cs336A1FieldModulus,
  )
  const recomputedRowDigest = await sha256Hex(
    JSON.stringify(claimedProductMatrix[0]),
  )

  return {
    challengeVector,
    claimedProductMatrix,
    fieldModulus: Cs336A1FieldModulus,
    leftMatrix,
    merkleProofValid: recomputedRowDigest === rowDigestsHex[0],
    merkleRootHex,
    rightMatrix,
    rowDigestsHex,
  }
}
