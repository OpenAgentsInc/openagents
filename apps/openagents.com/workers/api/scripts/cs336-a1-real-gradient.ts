/**
 * Contributor-side executor for the CS336 A1 real-gradient multi-device
 * run (issue #4678) over `src/cs336-a1-real-gradient-workload.ts`.
 *
 * Each contributor device computes the full-batch analytic gradient of
 * its own data shard at the current synchronized parameter state and
 * commits to it with a deterministic SHA-256 digest. The operator-side
 * aggregation averages the shard gradients, applies the SGD step, and
 * measures held-out validation loss. Any shard step can be re-executed
 * on another device and verified by the Worker's
 * `deterministic_recompute` class because all arithmetic is
 * engine-independent IEEE-754 f64.
 *
 * No network, no secrets, no spend: dispatch, challenge creation,
 * finalize, closeout, and settlement stay on the Worker authority
 * routes. Parameter/gradient state files are local transport between
 * the operator and contributor devices; stdout carries public-safe
 * digests, losses, norms, and refs only.
 *
 * Usage:
 *   bun scripts/cs336-a1-real-gradient.ts --mode init --out params0.json
 *   bun scripts/cs336-a1-real-gradient.ts --mode shard --step 0 --shard 1 \
 *     --params-file params0.json --out grad-s0-sh1.json
 *   bun scripts/cs336-a1-real-gradient.ts --mode aggregate --step 0 \
 *     --params-file params0.json --grad-files grad-s0-sh0.json,grad-s0-sh1.json \
 *     --out params1.json
 *   bun scripts/cs336-a1-real-gradient.ts --mode freivalds --state-digest <hex>
 */
import { computeCs336A1TokenizerShard, computeCs336A1TrainingStepMatrix } from '../src/cs336-a1-homework-workload'
import {
  applyAggregatedSgdStep,
  computeCs336A1ShardGradient,
  computeCs336A1ValidationLoss,
  cs336A1RealGradientRunConfig,
  initialParameters,
  parameterStateDigest,
} from '../src/cs336-a1-real-gradient-workload'

const args = process.argv.slice(2)

const flagValue = (flag: string): string | undefined => {
  const index = args.indexOf(flag)

  return index >= 0 ? args[index + 1] : undefined
}

const requireFlag = (flag: string): string => {
  const value = flagValue(flag)

  if (value === undefined) {
    throw new Error(`Missing required flag ${flag}`)
  }

  return value
}

const readValuesFile = async (path: string): Promise<Float64Array> => {
  const parsed = JSON.parse(await Bun.file(path).text()) as {
    values: ReadonlyArray<number>
  }

  return Float64Array.from(parsed.values)
}

const writeValuesFile = async (
  path: string,
  values: Float64Array,
): Promise<void> => {
  await Bun.write(path, JSON.stringify({ values: Array.from(values) }))
}

const emit = (payload: Record<string, unknown>): void => {
  console.log(JSON.stringify(payload, null, 2))
}

const run = async (): Promise<void> => {
  const mode = requireFlag('--mode')
  const config = cs336A1RealGradientRunConfig
  const seedDigestHex = (await computeCs336A1TokenizerShard()).digestHex

  if (mode === 'init') {
    const params = await initialParameters(config, seedDigestHex)

    await writeValuesFile(requireFlag('--out'), params)
    emit({
      mode,
      seedRef: `digest.sha256.${seedDigestHex}`,
      stateDigestRef: `digest.sha256.${await parameterStateDigest(seedDigestHex, 0, params)}`,
      stepIndex: 0,
    })

    return
  }

  if (mode === 'shard') {
    const stepIndex = Number(requireFlag('--step'))
    const shardIndex = Number(requireFlag('--shard'))
    const params = await readValuesFile(requireFlag('--params-file'))
    const result = await computeCs336A1ShardGradient({
      config,
      params,
      seedDigestHex,
      shardIndex,
      stepIndex,
    })
    const outPath = flagValue('--out')

    if (outPath !== undefined) {
      await writeValuesFile(outPath, result.gradient)
    }

    emit({
      dataUnitCount: result.dataUnitCount,
      gradientCommitmentRef: `commitment.cs336_a1.real_grad.step_${stepIndex}_shard_${shardIndex}.sha256_${result.digestHex.slice(0, 16)}`,
      gradientDigestRef: `digest.sha256.${result.digestHex}`,
      gradientL2Norm: result.gradientL2Norm,
      mode,
      shardIndex,
      shardLoss: result.shardLoss,
      stateDigestRef: `digest.sha256.${await parameterStateDigest(seedDigestHex, stepIndex, params)}`,
      stepIndex,
    })

    return
  }

  if (mode === 'aggregate') {
    const stepIndex = Number(requireFlag('--step'))
    const params = await readValuesFile(requireFlag('--params-file'))
    const gradients = await Promise.all(
      requireFlag('--grad-files').split(',').map(readValuesFile),
    )
    const next = applyAggregatedSgdStep(config, params, gradients)

    await writeValuesFile(requireFlag('--out'), next)

    const validation = await computeCs336A1ValidationLoss({
      config,
      params: next,
      seedDigestHex,
      stepIndex: stepIndex + 1,
    })

    emit({
      mode,
      nextStateDigestRef: `digest.sha256.${await parameterStateDigest(seedDigestHex, stepIndex + 1, next)}`,
      nextStepIndex: stepIndex + 1,
      shardGradientCount: gradients.length,
      validationDataUnitCount: validation.dataUnitCount,
      validationDigestRef: `digest.sha256.${validation.digestHex}`,
      validationLoss: validation.validationLoss,
    })

    return
  }

  if (mode === 'validate') {
    const stepIndex = Number(requireFlag('--step'))
    const params = await readValuesFile(requireFlag('--params-file'))
    const validation = await computeCs336A1ValidationLoss({
      config,
      params,
      seedDigestHex,
      stepIndex,
    })

    emit({
      mode,
      stepIndex,
      validationDataUnitCount: validation.dataUnitCount,
      validationDigestRef: `digest.sha256.${validation.digestHex}`,
      validationLoss: validation.validationLoss,
    })

    return
  }

  if (mode === 'freivalds') {
    const stateDigestHex = requireFlag('--state-digest')
    const step = await computeCs336A1TrainingStepMatrix(stateDigestHex)

    emit({
      freivaldsMerklePayload: {
        challengeVector: step.challengeVector,
        claimedProductMatrix: step.claimedProductMatrix,
        expectExactProduct: true,
        fieldModulus: step.fieldModulus,
        leftMatrix: step.leftMatrix,
        merkleProofValid: step.merkleProofValid,
        rightMatrix: step.rightMatrix,
        rowOpenings: step.rowDigestsHex.map((digestHex, row) => ({
          rowCommitmentRef: `commitment.cs336_a1.real_grad.row_${row}.sha256_${digestHex.slice(0, 16)}`,
        })),
      },
      merkleRootRef: `commitment.cs336_a1.real_grad.merkle_root.sha256_${step.merkleRootHex.slice(0, 16)}`,
      mode,
      stateDigestRef: `digest.sha256.${stateDigestHex}`,
    })

    return
  }

  throw new Error(`Unknown mode ${mode}`)
}

run()
