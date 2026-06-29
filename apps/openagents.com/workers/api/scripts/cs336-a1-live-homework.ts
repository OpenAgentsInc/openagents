/**
 * Contributor-side executor for the bounded CS336 A1 demo homework
 * (issue #4675). Computes the two A1 work classes locally and prints the
 * exact verification-challenge payloads the production Worker checks:
 *
 * - `deterministic_recompute` payload for the BPE merge shard digest
 * - `freivalds_merkle` payload for the training-step matrix (matrices,
 *   challenge vector, row openings, Merkle root)
 *
 * No network, no secrets, no spend: dispatch, challenge creation,
 * finalize, closeout, and settlement stay on the Worker authority routes
 * (see docs/2026-06-11-cs336-a1-live-homework-paid-closeout-evidence.md
 * for the live route chain this feeds).
 *
 * Usage:
 *   bun run scripts/cs336-a1-live-homework.ts [--assignment <assignmentRef>]
 */
import {
  computeCs336A1TokenizerShard,
  computeCs336A1TrainingStepMatrix,
} from '../src/cs336-a1-homework-workload'

const args = process.argv.slice(2)
const flagIndex = args.indexOf('--assignment')
const assignmentRef =
  flagIndex >= 0
    ? args[flagIndex + 1]
    : `assignment.cs336_a1.homework.${Date.now()}`

const run = async () => {
  const shard = await computeCs336A1TokenizerShard()
  const recompute = await computeCs336A1TokenizerShard()
  const step = await computeCs336A1TrainingStepMatrix(shard.digestHex)
  const contributionRef = `contribution.cs336_a1.${assignmentRef}.homework`

  console.log(
    JSON.stringify(
      {
        assignmentRef,
        deterministicRecomputePayload: {
          contributionRefs: [contributionRef],
          expectedDigestRef: `digest.sha256.${shard.digestHex}`,
          recomputedDigestRef: `digest.sha256.${recompute.digestHex}`,
        },
        freivaldsMerklePayload: {
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
        },
        merkleRootRef: `commitment.cs336_a1.merkle_root.sha256_${step.merkleRootHex.slice(0, 16)}`,
        shardSummary: {
          mergeCount: shard.mergeCount,
          tokenCount: shard.tokenCount,
          vocabularySize: shard.vocabularySize,
        },
      },
      null,
      2,
    ),
  )
}

run()
