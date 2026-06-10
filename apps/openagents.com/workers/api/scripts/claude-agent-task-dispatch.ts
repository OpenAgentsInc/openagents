/**
 * Operator dispatch for the bounded local-Claude coding task
 * (pylon.local_claude_agent_bridge.v1, issue #4720). Creates one
 * `claude_agent_task` assignment for a registered Pylon whose
 * codingAssignment carries the typed claude_agent_sdk work class. The
 * fixture (files, instructions, verification command) ships inside the
 * Pylon package; the wire payload stays ref-only and public-safe.
 * unpaid_smoke only — paid modes are a later operator decision.
 *
 * Admission hardening (the Tassadar v0.3 readiness lesson): the
 * capability ref travels inside the codingAssignment payload so
 * Pylon-side admission enforces it too, not just operator dispatch.
 *
 * Usage:
 *   OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/claude-agent-task-dispatch.ts \
 *     --pylon <pylonRef> [--base-url https://openagents.com] \
 *     [--assignment-ref <ref>]
 */

const CLAUDE_AGENT_CAPABILITY_REF = 'capability.pylon.local_claude_agent'
const CLAUDE_AGENT_TASK_SCHEMA = 'openagents.pylon.claude_agent_task.v0.3'
const CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF =
  'fixture.public.pylon.claude_agent.sum_repair.v1'

const args = process.argv.slice(2)
const flag = (name: string, fallback?: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const token = process.env['OPENAGENTS_ADMIN_API_TOKEN']
const pylonRef = flag('--pylon')
const baseUrl = flag('--base-url', 'https://openagents.com')
const assignmentRef =
  flag('--assignment-ref') ?? `assignment.claude_agent_task.${Date.now()}`

if (token === undefined || pylonRef === undefined) {
  console.error(
    'usage: OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/claude-agent-task-dispatch.ts --pylon <pylonRef>',
  )
  process.exit(2)
}

const body = {
  campaignPaused: false,
  campaignRef: 'campaign.claude_agent_bridge.v1',
  forumAutoPublishAllowed: false,
  acceptanceCriteriaRefs: [
    'acceptance.claude_agent_task.fixture_repair_test_passes',
    'acceptance.claude_agent_task.closeout_carries_verification_command_ref',
  ],
  assignmentRef,
  campaignPolicyRefs: ['policy.claude_agent_task.single_assignment_smoke'],
  closeoutPathRefs: ['route:/api/pylons/pylonRef/assignments/leaseRef/closeout'],
  codingAssignment: {
    kind: 'claude_agent_task',
    objective: {
      objectiveRef: `goal.public.claude_agent_task.${CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF}`,
    },
    requiredCapabilityRefs: [CLAUDE_AGENT_CAPABILITY_REF],
    claudeAgent: {
      schema: CLAUDE_AGENT_TASK_SCHEMA,
      agentKind: 'claude_agent_sdk',
      fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
      maxTurns: 12,
      timeoutSeconds: 300,
    },
  },
  idempotencyRefs: [`idempotency.claude_agent_task.${assignmentRef}`],
  jobKind: 'claude_agent_task',
  leaseSeconds: 3600,
  noDuplicateAssignmentRefs: ['gate.claude_agent_task.no_duplicate'],
  noForumAutoPublishRefs: ['gate.claude_agent_task.no_forum_auto_publish'],
  operatorPauseRefs: ['gate.claude_agent_task.operator_pause_available'],
  paymentMode: 'unpaid_smoke',
  pylonRef,
  requiredCapabilityRefs: [CLAUDE_AGENT_CAPABILITY_REF],
  resultExpectationRefs: [
    'expectation.claude_agent_task.fixture_repair_passed',
    'expectation.claude_agent_task.no_workspace_escape',
  ],
  rollbackRefs: ['gate.claude_agent_task.rollback_cancel_assignment'],
  selectionPolicyRefs: ['policy.claude_agent_task.operator_selected_pylon'],
  spendCapRefs: ['gate.claude_agent_task.no_spend'],
  taskRefs: [`task.claude_agent_task.${CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF}`],
}

const run = async () => {
  const response = await fetch(`${baseUrl}/api/operator/pylons/assignments`, {
    body: JSON.stringify(body),
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      'Idempotency-Key': `claude-agent-task-${assignmentRef}`,
    },
    method: 'POST',
  })
  const payload = await response.json()
  console.log(
    JSON.stringify(
      { assignmentRef, pylonRef, status: response.status, payload },
      null,
      2,
    ),
  )
  process.exit(response.status < 300 ? 0 : 1)
}

run()
