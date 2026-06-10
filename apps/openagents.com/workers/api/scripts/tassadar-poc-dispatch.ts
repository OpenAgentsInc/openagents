/**
 * Operator dispatch for the Tassadar executor-trace proof of concept
 * (compute.tassadar_executor_poc.v1, issues #4690/#4691). Creates one
 * `tassadar_executor_trace` assignment for a registered Pylon with the
 * digest-pinned workload fixture embedded in the codingAssignment
 * payload. unpaid_smoke by default; paid modes are operator decisions.
 *
 * Usage:
 *   OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/tassadar-poc-dispatch.ts \
 *     --pylon <pylonRef> [--base-url https://openagents.com] \
 *     [--payment-mode unpaid_smoke] [--assignment-ref <ref>]
 */
import { readFileSync } from 'node:fs'

import {
  TassadarBoundedProfileRef,
  TassadarExactTraceReplayVerificationClass,
  TassadarExecutorTraceJobKind,
  buildTassadarExecutorTracePayload,
} from '../src/tassadar-executor-trace-homework'

const args = process.argv.slice(2)
const flag = (name: string, fallback?: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const token = process.env['OPENAGENTS_ADMIN_API_TOKEN']
const pylonRef = flag('--pylon')
const baseUrl = flag('--base-url', 'https://openagents.com')
const paymentMode = flag('--payment-mode', 'unpaid_smoke')
const assignmentRef =
  flag('--assignment-ref') ?? `assignment.tassadar_poc.${Date.now()}`

if (token === undefined || pylonRef === undefined) {
  console.error(
    'usage: OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/tassadar-poc-dispatch.ts --pylon <pylonRef>',
  )
  process.exit(2)
}

const fixture = JSON.parse(
  readFileSync(
    new URL(
      '../../../../../packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json',
      import.meta.url,
    ),
    'utf8',
  ),
)

const homeworkPayload = buildTassadarExecutorTracePayload({
  assignmentRef,
  workloadFamily: 'kernel_trace',
})

const body = {
  acceptanceCriteriaRefs: [
    'acceptance.tassadar_poc.trace_digest_matches_fixture',
    'acceptance.tassadar_poc.closeout_carries_trace_digest',
  ],
  assignmentRef,
  campaignPolicyRefs: ['policy.tassadar_poc.single_assignment_smoke'],
  closeoutPathRefs: ['route:/api/pylons/{pylonRef}/assignments/{leaseRef}/closeout'],
  codingAssignment: {
    kind: TassadarExecutorTraceJobKind,
    objective: { objectiveRef: `goal.tassadar_poc.execute.${fixture.fixtureId}` },
    requiredCapabilityRefs: [],
    tassadar: {
      boundedProfileRef: TassadarBoundedProfileRef,
      expectedModelDigest: fixture.expectedModelDigest,
      expectedTraceDigest: fixture.expectedTraceDigest,
      fixtureId: fixture.fixtureId,
      homework: homeworkPayload,
      model: fixture.model,
      steps: fixture.steps,
      verificationClass: TassadarExactTraceReplayVerificationClass,
    },
  },
  idempotencyRefs: [`idempotency.tassadar_poc.${assignmentRef}`],
  jobKind: 'tassadar_executor_trace',
  leaseSeconds: 3600,
  noDuplicateAssignmentRefs: ['gate.tassadar_poc.no_duplicate'],
  noForumAutoPublishRefs: ['gate.tassadar_poc.no_forum_auto_publish'],
  operatorPauseRefs: ['gate.tassadar_poc.operator_pause_available'],
  paymentMode,
  pylonRef,
  requiredCapabilityRefs: [],
  resultExpectationRefs: [
    `expectation.tassadar_poc.trace_digest.${fixture.expectedTraceDigest.slice(0, 16)}`,
  ],
  rollbackRefs: ['gate.tassadar_poc.rollback_cancel_assignment'],
  selectionPolicyRefs: ['policy.tassadar_poc.operator_selected_pylon'],
  spendCapRefs:
    paymentMode === 'unpaid_smoke'
      ? ['gate.tassadar_poc.no_spend']
      : ['gate.tassadar_poc.minimal_operator_funded_cap'],
  taskRefs: [`task.tassadar_poc.${fixture.fixtureId}`],
}

const run = async () => {
  const response = await fetch(`${baseUrl}/api/operator/pylons/assignments`, {
    body: JSON.stringify(body),
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      'Idempotency-Key': `tassadar-poc-${assignmentRef}`,
    },
    method: 'POST',
  })
  const payload = await response.json()
  console.log(
    JSON.stringify(
      { assignmentRef, paymentMode, pylonRef, status: response.status, payload },
      null,
      2,
    ),
  )
  process.exit(response.status < 300 ? 0 : 1)
}

run()
