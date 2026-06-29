/**
 * Operator dispatch for paid validator assignments (issue #4676).
 *
 * Fetches a queued training verification challenge from the production
 * Worker, routes it through `buildTrainingValidatorAssignmentRequest` —
 * which enforces the no-self-validation guard and the validator
 * capability/selection policy refs — and only then POSTs the resulting
 * `validation` assignment to the controlled Pylon dispatch route. When
 * the bridge blocks (validator equals worker, or the validator owns the
 * contribution), nothing is sent and the blocker refs are printed.
 *
 * Usage:
 *   OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/training-validator-live-dispatch.ts \
 *     --challenge <challengeRef> \
 *     --validator-pylon <pylonRef> \
 *     --worker-pylon <pylonRef> \
 *     [--base-url https://openagents.com] [--assignment-ref <ref>] [--dry-run]
 */
import {
  buildTrainingValidatorAssignmentRequest,
  type TrainingValidatorChallengeSummary,
} from '../src/training-validator-assignments'

const args = process.argv.slice(2)
const flag = (name: string, fallback?: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const token = process.env['OPENAGENTS_ADMIN_API_TOKEN']
const challengeRef = flag('--challenge')
const validatorPylonRef = flag('--validator-pylon')
const workerPylonRef = flag('--worker-pylon')
const baseUrl = flag('--base-url', 'https://openagents.com')!
const assignmentRef = flag('--assignment-ref')
const dryRun = args.includes('--dry-run')

if (
  token === undefined ||
  challengeRef === undefined ||
  validatorPylonRef === undefined ||
  workerPylonRef === undefined
) {
  console.error(
    'usage: OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/training-validator-live-dispatch.ts --challenge <challengeRef> --validator-pylon <pylonRef> --worker-pylon <pylonRef>',
  )
  process.exit(2)
}

const run = async () => {
  const challengeResponse = await fetch(
    `${baseUrl}/api/training/verification/challenges/${encodeURIComponent(challengeRef)}`,
  )

  if (!challengeResponse.ok) {
    console.error(
      JSON.stringify({
        error: 'challenge_fetch_failed',
        status: challengeResponse.status,
      }),
    )
    process.exit(1)
  }

  const challengePayload = (await challengeResponse.json()) as {
    challenge: TrainingValidatorChallengeSummary
  }
  const bridged = buildTrainingValidatorAssignmentRequest({
    ...(assignmentRef === undefined ? {} : { assignmentRef }),
    challenge: challengePayload.challenge,
    nowIso: new Date().toISOString(),
    validatorPylonRef,
    workerPylonRef,
  })

  if (bridged.kind === 'blocked') {
    console.log(
      JSON.stringify(
        {
          blockerRefs: bridged.blockerRefs,
          challengeRef,
          dispatched: false,
          kind: 'blocked',
          validatorPylonRef,
          workerPylonRef,
        },
        null,
        2,
      ),
    )
    process.exit(1)
  }

  if (dryRun) {
    console.log(
      JSON.stringify(
        {
          assignmentRequest: bridged.assignmentRequest,
          dispatched: false,
          kind: 'dry_run',
          paymentBlockedRefs: bridged.paymentBlockedRefs,
        },
        null,
        2,
      ),
    )
    process.exit(0)
  }

  const response = await fetch(`${baseUrl}/api/operator/pylons/assignments`, {
    body: JSON.stringify(bridged.assignmentRequest),
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      'Idempotency-Key': `training-validator-${bridged.assignmentRequest.assignmentRef}`,
    },
    method: 'POST',
  })
  const payload = await response.json()

  console.log(
    JSON.stringify(
      {
        assignmentRef: bridged.assignmentRequest.assignmentRef,
        paymentBlockedRefs: bridged.paymentBlockedRefs,
        payload,
        status: response.status,
        validatorPylonRef,
        workerPylonRef,
      },
      null,
      2,
    ),
  )
  process.exit(response.status < 300 ? 0 : 1)
}

run()
