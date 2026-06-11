/**
 * Validator-side executor for the paid weak-device validator lane
 * (issue #4676). Runs entirely on the validator device with no network
 * and no secrets: it independently recomputes the bounded CS336 A1
 * workload, reruns the bound verification class locally through the same
 * registry the production Worker uses, and prints:
 *
 * - the exact verification-challenge payload an operator can queue
 * - the validator's independent verdict (state, verdict refs, failure codes)
 * - the public evidence refs the validator submits on its assignment rail
 * - the single-verdict consensus projection for the bound class
 *
 * Challenge creation, claim, finalize, closeout, and settlement stay on
 * the Worker authority routes; this script is only the local re-execution
 * a validator Pylon performs after claiming a validation assignment.
 *
 * Usage:
 *   bun run scripts/training-validator-live-verify.ts \
 *     --validator-pylon <pylonRef> \
 *     --contribution <contributionRef> \
 *     [--class freivalds_merkle|deterministic_recompute] \
 *     [--training-run <trainingRunRef>] [--window <windowRef>]
 */
import {
  computeCs336A1TokenizerShard,
  computeCs336A1TrainingStepMatrix,
} from '../src/cs336-a1-homework-workload'
import {
  projectTrainingValidatorConsensus,
  type TrainingValidatorVerdict,
} from '../src/training-validator-assignments'
import {
  type TrainingVerificationClass,
  buildTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
} from '../src/training-verification'

const args = process.argv.slice(2)
const flag = (name: string, fallback?: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const validatorPylonRef = flag('--validator-pylon')
const contributionRef = flag('--contribution')
const verificationClass = flag(
  '--class',
  'freivalds_merkle',
) as TrainingVerificationClass
const trainingRunRef = flag('--training-run', 'run.cs336.a1.demo')!
const windowRef = flag('--window', 'training.window.cs336_a1.demo.20260611.w1')!

if (validatorPylonRef === undefined || contributionRef === undefined) {
  console.error(
    'usage: bun run scripts/training-validator-live-verify.ts --validator-pylon <pylonRef> --contribution <contributionRef>',
  )
  process.exit(2)
}

if (
  verificationClass !== 'freivalds_merkle' &&
  verificationClass !== 'deterministic_recompute'
) {
  console.error(
    'This validator executor re-executes freivalds_merkle or deterministic_recompute only.',
  )
  process.exit(2)
}

const run = async () => {
  const shard = await computeCs336A1TokenizerShard()
  const independentRecompute = await computeCs336A1TokenizerShard()
  const step = await computeCs336A1TrainingStepMatrix(shard.digestHex)
  const payload: Record<string, unknown> =
    verificationClass === 'deterministic_recompute'
      ? {
          contributionRefs: [contributionRef],
          expectedDigestRef: `digest.sha256.${shard.digestHex}`,
          recomputedDigestRef: `digest.sha256.${independentRecompute.digestHex}`,
        }
      : {
          challengeVector: step.challengeVector,
          claimedProductMatrix: step.claimedProductMatrix,
          contributionRefs: [contributionRef],
          expectExactProduct: true,
          fieldModulus: step.fieldModulus,
          leftMatrix: step.leftMatrix,
          merkleProofValid: step.merkleProofValid,
          rightMatrix: step.rightMatrix,
          rowOpenings: step.rowDigestsHex.map((digestHex, row) => ({
            rowCommitmentRef: `commitment.cs336_a1.row_${row}.sha256_${digestHex.slice(0, 16)}`,
          })),
        }
  const commitmentRefs =
    verificationClass === 'deterministic_recompute'
      ? [`commitment.cs336_a1.validator_recheck.${verificationClass}`]
      : [
          `commitment.cs336_a1.merkle_root.sha256_${step.merkleRootHex.slice(0, 16)}`,
        ]
  const challengeCreatePayload = {
    commitmentRefs,
    contributionRef,
    homeworkKind: 'validator_recheck',
    payload,
    samplingPolicy: 'per_contribution' as const,
    trainingRunRef,
    verificationClass,
    windowRef,
  }
  // Local re-execution of the bound class: the same registry code the
  // production Worker runs at finalize time, executed on the validator
  // device against the independently recomputed payload.
  const localChallenge = buildTrainingVerificationChallengeRecord({
    makeId: () => `local_validator_${Date.now()}`,
    nowIso: new Date().toISOString(),
    request: challengeCreatePayload,
  }).challenge
  const verdict = await runTrainingVerificationClass({
    challenge: localChallenge,
  })
  const validatorVerdict: TrainingValidatorVerdict = {
    challengeRef: localChallenge.challengeRef,
    failureCodes: verdict.failureCodes,
    state: verdict.state,
    validatorPylonRef,
    verdictRef:
      verdict.verdictRefs[0] ??
      `verdict.training.${verificationClass}.${verdict.state.toLowerCase()}.local`,
    verificationClass,
  }
  const consensus = projectTrainingValidatorConsensus({
    challengeRef: localChallenge.challengeRef,
    verdicts: [validatorVerdict],
    verificationClass,
  })
  const evidenceRefs = [
    ...commitmentRefs,
    `digest.sha256.${independentRecompute.digestHex}`,
    `verdict_evidence.training_validator.${verificationClass}.${verdict.state.toLowerCase()}`,
    `validator_recompute.${validatorPylonRef}.${verificationClass}`,
  ]

  console.log(
    JSON.stringify(
      {
        challengeCreatePayload,
        consensus,
        evidenceRefs,
        independentVerdict: {
          failureCodes: verdict.failureCodes,
          state: verdict.state,
          verdictRefs: verdict.verdictRefs,
        },
        shardSummary: {
          mergeCount: shard.mergeCount,
          tokenCount: shard.tokenCount,
          vocabularySize: shard.vocabularySize,
        },
        validatorPylonRef,
      },
      null,
      2,
    ),
  )
  process.exit(verdict.state === 'Verified' ? 0 : 1)
}

run()
