import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'
import {
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const OpenAgentsProbeRunStatus = S.Literals([
  'cancelled',
  'failed',
  'needs_context',
  'needs_review',
  'queued',
  'retained_failure',
  'running',
  'succeeded',
  'timed_out',
])
export type OpenAgentsProbeRunStatus = typeof OpenAgentsProbeRunStatus.Type

export const OpenAgentsProbeToolKind = S.Literals([
  'browser',
  'edit',
  'file',
  'git',
  'mcp',
  'search',
  'shell',
  'test_runner',
])
export type OpenAgentsProbeToolKind = typeof OpenAgentsProbeToolKind.Type

export const OpenAgentsProbeTurnKind = S.Literals([
  'assistant',
  'system',
  'tool',
  'user',
])
export type OpenAgentsProbeTurnKind = typeof OpenAgentsProbeTurnKind.Type

export class OpenAgentsProbeRunRequest extends S.Class<OpenAgentsProbeRunRequest>(
  'OpenAgentsProbeRunRequest',
)({
  assignmentRef: S.String,
  correlationRefs: S.Array(S.String),
  createdAtIso: S.String,
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  objectiveRef: S.String,
  policyRefs: S.Array(S.String),
  programRunRef: S.String,
  routeRefs: S.Array(S.String),
  runtimeRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  trustLevel: OpenAgentsRunnerWorkloadTrust,
  workroomRef: S.String,
}) {}

export class OpenAgentsProbeTurnEvent extends S.Class<OpenAgentsProbeTurnEvent>(
  'OpenAgentsProbeTurnEvent',
)({
  artifactRefs: S.Array(S.String),
  createdAtIso: S.String,
  eventRef: S.String,
  kind: OpenAgentsProbeTurnKind,
  summaryRef: S.String,
  toolCallRefs: S.Array(S.String),
}) {}

export class OpenAgentsProbeToolCallSummary extends S.Class<OpenAgentsProbeToolCallSummary>(
  'OpenAgentsProbeToolCallSummary',
)({
  artifactRefs: S.Array(S.String),
  costRefs: S.Array(S.String),
  createdAtIso: S.String,
  diagnosticRefs: S.Array(S.String),
  id: S.String,
  receiptRefs: S.Array(S.String),
  resultSummaryRef: S.String,
  toolKind: OpenAgentsProbeToolKind,
}) {}

export class OpenAgentsProbeRunRecord extends S.Class<OpenAgentsProbeRunRecord>(
  'OpenAgentsProbeRunRecord',
)({
  artifactRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  costRefs: S.Array(S.String),
  createdAtIso: S.String,
  diffRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  id: S.String,
  previewRefs: S.Array(S.String),
  request: OpenAgentsProbeRunRequest,
  retainedFailureRefs: S.Array(S.String),
  status: OpenAgentsProbeRunStatus,
  testResultRefs: S.Array(S.String),
  toolCalls: S.Array(OpenAgentsProbeToolCallSummary),
  turnEvents: S.Array(OpenAgentsProbeTurnEvent),
  updatedAtIso: S.String,
}) {}

export class OpenAgentsProbeTurnEventProjection extends S.Class<OpenAgentsProbeTurnEventProjection>(
  'OpenAgentsProbeTurnEventProjection',
)({
  artifactRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  eventRef: S.String,
  kind: OpenAgentsProbeTurnKind,
  summaryRef: S.String,
  toolCallRefs: S.Array(S.String),
}) {}

export class OpenAgentsProbeToolCallProjection extends S.Class<OpenAgentsProbeToolCallProjection>(
  'OpenAgentsProbeToolCallProjection',
)({
  artifactRefs: S.Array(S.String),
  costRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  diagnosticRefs: S.Array(S.String),
  id: S.String,
  receiptRefs: S.Array(S.String),
  resultSummaryRef: S.String,
  toolKind: OpenAgentsProbeToolKind,
}) {}

export class OpenAgentsProbeRunProjection extends S.Class<OpenAgentsProbeRunProjection>(
  'OpenAgentsProbeRunProjection',
)({
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  audience: OmniProjectionAudience,
  closeoutReceiptRefs: S.Array(S.String),
  correlationRefs: S.Array(S.String),
  costRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  diffRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  objectiveRef: S.String,
  policyRefs: S.Array(S.String),
  previewRefs: S.Array(S.String),
  programRunRef: S.String,
  retainedFailureRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  runtimeRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: OpenAgentsProbeRunStatus,
  terminal: S.Boolean,
  testResultRefs: S.Array(S.String),
  toolCalls: S.Array(OpenAgentsProbeToolCallProjection),
  trustLevel: OpenAgentsRunnerWorkloadTrust,
  turnEvents: S.Array(OpenAgentsProbeTurnEventProjection),
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OpenAgentsProbeContractUnsafe extends S.TaggedErrorClass<OpenAgentsProbeContractUnsafe>()(
  'OpenAgentsProbeContractUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeProbeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|log[_-]?line|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|tool[_-]?log|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(cost\.private|diagnostic|failure\.operator|route\.private|source\.private|tool\.operator|workroom\.private)/i
const customerUnsafeRefPattern =
  /(cost\.private|diagnostic|failure\.operator|route\.private|source\.private|tool\.operator|workroom\.private)/i
const teamUnsafeRefPattern =
  /(cost\.private|route\.private|source\.private|tool\.operator|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeProbeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsProbeContractUnsafe({
      reason: `${label} contains raw tool logs, provider payloads, credentials, local paths, raw source archives, private repo refs, wallet/payment material, payout targets, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const requestRefs = (
  request: OpenAgentsProbeRunRequest,
): ReadonlyArray<string> => [
  request.id,
  request.assignmentRef,
  request.objectiveRef,
  request.programRunRef,
  request.runtimeRef,
  request.workroomRef,
  ...request.correlationRefs,
  ...request.idempotencyRefs,
  ...request.policyRefs,
  ...request.routeRefs,
  ...request.sourceAuthorityRefs,
]

const turnRefs = (
  event: OpenAgentsProbeTurnEvent,
): ReadonlyArray<string> => [
  event.eventRef,
  event.summaryRef,
  ...event.artifactRefs,
  ...event.toolCallRefs,
]

const toolRefs = (
  toolCall: OpenAgentsProbeToolCallSummary,
): ReadonlyArray<string> => [
  toolCall.id,
  toolCall.resultSummaryRef,
  ...toolCall.artifactRefs,
  ...toolCall.costRefs,
  ...toolCall.diagnosticRefs,
  ...toolCall.receiptRefs,
]

const runRefs = (
  run: OpenAgentsProbeRunRecord,
): ReadonlyArray<string> => [
  run.id,
  ...requestRefs(run.request),
  ...run.artifactRefs,
  ...run.closeoutReceiptRefs,
  ...run.costRefs,
  ...run.diffRefs,
  ...run.failureRefs,
  ...run.previewRefs,
  ...run.retainedFailureRefs,
  ...run.testResultRefs,
  ...run.toolCalls.flatMap(toolRefs),
  ...run.turnEvents.flatMap(turnRefs),
]

const assertRunSafe = (run: OpenAgentsProbeRunRecord): void => {
  assertSafeRefs('Probe run refs', runRefs(run))
}

export const openAgentsProbeRunIsTerminal = (
  status: OpenAgentsProbeRunStatus,
): boolean =>
  status === 'succeeded' ||
  status === 'failed' ||
  status === 'cancelled' ||
  status === 'timed_out' ||
  status === 'retained_failure'

export const openAgentsProbeRunRequiresRetainedFailure = (
  status: OpenAgentsProbeRunStatus,
): boolean => status === 'failed' || status === 'timed_out'

export const openAgentsProbeRunHasRequiredTerminalEvidence = (
  run: OpenAgentsProbeRunRecord,
): boolean =>
  run.status === 'succeeded'
    ? run.closeoutReceiptRefs.length > 0 &&
      (run.artifactRefs.length > 0 || run.diffRefs.length > 0)
    : openAgentsProbeRunRequiresRetainedFailure(run.status)
      ? run.failureRefs.length > 0 && run.retainedFailureRefs.length > 0
      : run.status === 'cancelled'
        ? run.closeoutReceiptRefs.length > 0
        : true

const projectTurn = (
  event: OpenAgentsProbeTurnEvent,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsProbeTurnEventProjection => ({
  artifactRefs: safeRefsForAudience(
    'Probe turn artifact refs',
    event.artifactRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    event.createdAtIso,
    nowIso,
  ),
  eventRef: safeRefForAudience('Probe turn event ref', event.eventRef, audience),
  kind: event.kind,
  summaryRef: safeRefForAudience(
    'Probe turn summary ref',
    event.summaryRef,
    audience,
  ),
  toolCallRefs: safeRefsForAudience(
    'Probe turn tool call refs',
    event.toolCallRefs,
    audience,
  ),
})

const projectToolCall = (
  toolCall: OpenAgentsProbeToolCallSummary,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsProbeToolCallProjection => ({
  artifactRefs: safeRefsForAudience(
    'Probe tool artifact refs',
    toolCall.artifactRefs,
    audience,
  ),
  costRefs: safeRefsForAudience(
    'Probe tool cost refs',
    toolCall.costRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    toolCall.createdAtIso,
    nowIso,
  ),
  diagnosticRefs:
    audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
          'Probe tool diagnostic refs',
          toolCall.diagnosticRefs,
          audience,
        )
      : [],
  id: safeRefForAudience('Probe tool id', toolCall.id, audience),
  receiptRefs: safeRefsForAudience(
    'Probe tool receipt refs',
    toolCall.receiptRefs,
    audience,
  ),
  resultSummaryRef: safeRefForAudience(
    'Probe tool result summary ref',
    toolCall.resultSummaryRef,
    audience,
  ),
  toolKind: toolCall.toolKind,
})

export const projectOpenAgentsProbeRun = (
  run: OpenAgentsProbeRunRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsProbeRunProjection => {
  assertRunSafe(run)

  const projection: OpenAgentsProbeRunProjection = {
    artifactRefs: safeRefsForAudience(
      'Probe artifact refs',
      run.artifactRefs,
      audience,
    ),
    assignmentRef: safeRefForAudience(
      'Probe assignment ref',
      run.request.assignmentRef,
      audience,
    ),
    audience,
    closeoutReceiptRefs: safeRefsForAudience(
      'Probe closeout receipt refs',
      run.closeoutReceiptRefs,
      audience,
    ),
    correlationRefs: safeRefsForAudience(
      'Probe correlation refs',
      run.request.correlationRefs,
      audience,
    ),
    costRefs: safeRefsForAudience('Probe cost refs', run.costRefs, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      run.createdAtIso,
      nowIso,
    ),
    diffRefs: safeRefsForAudience('Probe diff refs', run.diffRefs, audience),
    failureRefs: safeRefsForAudience(
      'Probe failure refs',
      run.failureRefs,
      audience,
    ),
    id: safeRefForAudience('Probe run id', run.id, audience),
    idempotencyRefs: safeRefsForAudience(
      'Probe idempotency refs',
      run.request.idempotencyRefs,
      audience,
    ),
    objectiveRef: safeRefForAudience(
      'Probe objective ref',
      run.request.objectiveRef,
      audience,
    ),
    policyRefs: safeRefsForAudience(
      'Probe policy refs',
      run.request.policyRefs,
      audience,
    ),
    previewRefs: safeRefsForAudience(
      'Probe preview refs',
      run.previewRefs,
      audience,
    ),
    programRunRef: safeRefForAudience(
      'Probe Program Run ref',
      run.request.programRunRef,
      audience,
    ),
    retainedFailureRefs: safeRefsForAudience(
      'Probe retained failure refs',
      run.retainedFailureRefs,
      audience,
    ),
    routeRefs: safeRefsForAudience(
      'Probe route refs',
      run.request.routeRefs,
      audience,
    ),
    runtimeRef: safeRefForAudience(
      'Probe runtime ref',
      run.request.runtimeRef,
      audience,
    ),
    sourceAuthorityRefs: safeRefsForAudience(
      'Probe source authority refs',
      run.request.sourceAuthorityRefs,
      audience,
    ),
    status: run.status,
    terminal: openAgentsProbeRunIsTerminal(run.status),
    testResultRefs: safeRefsForAudience(
      'Probe test result refs',
      run.testResultRefs,
      audience,
    ),
    toolCalls: run.toolCalls.map(toolCall =>
      projectToolCall(toolCall, audience, nowIso)
    ),
    trustLevel: run.request.trustLevel,
    turnEvents: run.turnEvents.map(event =>
      projectTurn(event, audience, nowIso)
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      run.updatedAtIso,
      nowIso,
    ),
    workroomRef: safeRefForAudience(
      'Probe workroom ref',
      run.request.workroomRef,
      audience,
    ),
  }

  if (openAgentsProbeRunProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsProbeContractUnsafe({
      reason: 'Probe run projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsProbeRunProjectionHasPrivateMaterial = (
  projection: OpenAgentsProbeRunProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafeProbeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

const probeRequestFixture: OpenAgentsProbeRunRequest = {
  assignmentRef: 'assignment.otec.site_revision',
  correlationRefs: ['correlation.probe.otec.1'],
  createdAtIso: '2026-06-07T02:00:00.000Z',
  id: 'probe_request.otec.1',
  idempotencyRefs: ['idempotency.probe.otec.1'],
  objectiveRef: 'objective.site_revision.public_safe',
  policyRefs: ['policy.probe.no_raw_logs'],
  programRunRef: 'program_run.probe.otec.1',
  routeRefs: ['route.probe.shc_vm'],
  runtimeRef: 'runtime.probe.coding_adapter',
  sourceAuthorityRefs: ['source_authority.order.summary'],
  trustLevel: 'medium',
  workroomRef: 'workroom.otec.public',
}

export const OPENAGENTS_PROBE_CONFORMANCE_FIXTURES:
  ReadonlyArray<OpenAgentsProbeRunRecord> = [
    {
      artifactRefs: ['artifact.probe.diff_summary'],
      closeoutReceiptRefs: ['receipt.probe.closeout.success'],
      costRefs: ['cost.probe.summary'],
      createdAtIso: '2026-06-07T02:01:00.000Z',
      diffRefs: ['diff.probe.public_summary'],
      failureRefs: [],
      id: 'probe_run.fixture.success',
      previewRefs: ['preview.probe.site_public'],
      request: probeRequestFixture,
      retainedFailureRefs: [],
      status: 'succeeded',
      testResultRefs: ['test.probe.vitest_passed'],
      toolCalls: [
        {
          artifactRefs: ['artifact.probe.test_summary'],
          costRefs: ['cost.probe.tool_summary'],
          createdAtIso: '2026-06-07T02:02:00.000Z',
          diagnosticRefs: ['diagnostic.probe.operator_summary'],
          id: 'tool_call.probe.test_runner.1',
          receiptRefs: ['receipt.probe.tool_call.1'],
          resultSummaryRef: 'summary.probe.tests_passed',
          toolKind: 'test_runner',
        },
      ],
      turnEvents: [
        {
          artifactRefs: ['artifact.probe.turn_summary'],
          createdAtIso: '2026-06-07T02:02:00.000Z',
          eventRef: 'event.probe.turn.assistant.1',
          kind: 'assistant',
          summaryRef: 'summary.probe.turn_1',
          toolCallRefs: ['tool_call.probe.test_runner.1'],
        },
      ],
      updatedAtIso: '2026-06-07T02:05:00.000Z',
    },
    {
      artifactRefs: [],
      closeoutReceiptRefs: [],
      costRefs: ['cost.probe.summary'],
      createdAtIso: '2026-06-07T02:10:00.000Z',
      diffRefs: [],
      failureRefs: ['failure.probe.timeout_summary'],
      id: 'probe_run.fixture.retained_failure',
      previewRefs: [],
      request: {
        ...probeRequestFixture,
        id: 'probe_request.otec.retained_failure',
        programRunRef: 'program_run.probe.otec.retained_failure',
      },
      retainedFailureRefs: ['retained_failure.probe.timeout_1'],
      status: 'retained_failure',
      testResultRefs: [],
      toolCalls: [],
      turnEvents: [],
      updatedAtIso: '2026-06-07T02:12:00.000Z',
    },
  ]
