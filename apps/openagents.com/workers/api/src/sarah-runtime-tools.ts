import {
  type FullAutoRunControlAuthorityRepositoryShape,
  type FullAutoRunProjectionAuthorityRepositoryShape,
  type SyncSql,
  makeFullAutoRunControlAuthority,
  makeFullAutoRunProjectionRepository,
} from '@openagentsinc/khala-sync-server'
import {
  detectOpenAgentsMcpUnsafeMaterial,
  redactOpenAgentsMcpUnsafeText,
} from '@openagentsinc/mcp-contract'
import { Effect, Schema as S } from 'effect'

import type { CrmMcpCatalog, McpToolCallOutcome } from './crm-mcp-routes'
import { resolveManagedCloudRepositoryCommit } from './khala-cloud-runtime-dispatch'
import { khalaMcpOwnerPrincipal } from './khala-mcp'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type SarahAgentTool,
  SarahAgentToolError,
  type SarahAgentToolResult,
} from './sarah-agent-runtime'
import type {
  SarahHarnessReviewOutcome,
  SarahHarnessStatus,
} from './sarah-harness-service'
import {
  type SarahOperationAuthorityInput,
  type SarahOperationAuthorityOutcome,
  authorizeSarahOperation,
} from './sarah-owner-routes'
import { recordSarahWorkerDispatchMapping } from './sarah-worker-closeout-notify'

const OWNER_REPOSITORY = 'OpenAgentsInc/openagents'
const OWNER_REPOSITORY_BRANCH = 'main'
const OWNER_REPOSITORY_VERIFY = 'pnpm run check'

// Binds the sarah_web_comms tool to grant.sarah.web_communications
// (AUTHORITY.md rev 7 / docs/authority/SARAH_AUTHORITY.md rev 5). The tool
// composes the existing repository-delivery and owner-review paths; it is never
// a second outward-publish authority. Blog and document drafts, the Nostr open
// channel, and the owner-reviewed X tweet queue deliver through repository
// delivery now. Animated-spoken publication refuses with a receipt until the
// owner-supplied animation and speech interfaces and the web-communications
// broker are deployed, healthy, and receipt-capable.
const WEB_COMMS_PROGRAM = 'program.sarah_web_communications'
const SARAH_TWEET_QUEUE_PATH = 'docs/sarah/SARAH_TWEET_QUEUE.md'

const slugify = (value: string): string => {
  const slug = value
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, '-')
    .replaceAll(/^-+|-+$/g, '')
    .slice(0, 60)
  return slug.length === 0 ? 'untitled' : slug
}

const WorkerCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(8),
)
const Objective = S.Trim.check(S.isMinLength(3), S.isMaxLength(8_000))
const PublicRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const StartWorkersInput = S.Struct({
  objective: Objective,
  count: WorkerCount,
  maxParallel: S.optional(WorkerCount),
})
const SpawnStatusInput = S.Struct({ spawnRef: PublicRef })
const FullAutoStatusInput = S.Struct({ runRef: S.optional(PublicRef) })
const FullAutoControlInput = S.Struct({
  action: S.Literals(['pause', 'resume', 'stop']),
  runRef: S.optional(PublicRef),
})

const WebCommsTitle = S.Trim.check(S.isMinLength(3), S.isMaxLength(200))
const WebCommsBody = S.Trim.check(S.isMinLength(3), S.isMaxLength(4_000))
const WebCommsInput = S.Struct({
  action: S.Literals(['draft', 'publish_outward']),
  kind: S.optional(S.Literals(['blog', 'document', 'forum'])),
  channel: S.optional(S.Literals(['timeline', 'animated_spoken', 'nostr'])),
  title: WebCommsTitle,
  body: WebCommsBody,
  deliver: S.optional(S.Boolean),
})

type AuthorizeOperation = (
  sql: SyncSql,
  input: SarahOperationAuthorityInput,
) => Effect.Effect<SarahOperationAuthorityOutcome, unknown>

export type SarahRuntimeToolDependencies<Bindings> = Readonly<{
  env: Bindings
  sql: SyncSql
  ownerUserId: string
  threadRef: string
  turnId: string
  khalaCatalog: CrmMcpCatalog<Bindings>
  authorizeOperation?: AuthorizeOperation | undefined
  fullAutoProjection?: FullAutoRunProjectionAuthorityRepositoryShape | undefined
  fullAutoControl?: FullAutoRunControlAuthorityRepositoryShape | undefined
  resolveRepositoryCommit?: (() => Promise<string | null>) | undefined
  nowIso?: (() => string) | undefined
  harnessStatus?: (() => Effect.Effect<SarahHarnessStatus, unknown>) | undefined
  reviewHarness?:
    (() => Effect.Effect<SarahHarnessReviewOutcome, unknown>) | undefined
  managedSandboxTools?: ReadonlyArray<SarahAgentTool> | undefined
}>

const toolFailure = (reason: string): SarahAgentToolError =>
  new SarahAgentToolError({ reason })

const toolFailureFrom = (
  error: unknown,
  fallback: string,
): SarahAgentToolError =>
  toolFailure(error instanceof Error ? error.message : fallback)

const toolPromise = <A>(
  operation: () => PromiseLike<A>,
  fallback: string,
): Effect.Effect<A, SarahAgentToolError> =>
  Effect.tryPromise({
    try: operation,
    catch: error => toolFailureFrom(error, fallback),
  })

const decodeInput = <A>(decode: (input: unknown) => A, input: unknown): A => {
  try {
    return decode(input)
  } catch {
    throw toolFailure('invalid_tool_arguments')
  }
}

const isRecord = (value: unknown): value is Readonly<Record<string, unknown>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const json = (value: unknown): string => {
  try {
    return JSON.stringify(value)
  } catch {
    return JSON.stringify({
      error: 'tool_result_serialization_failed',
      ok: false,
    })
  }
}

const textForOutcome = (outcome: McpToolCallOutcome): string =>
  outcome.content
    .map(block => block.text)
    .join('\n')
    .slice(0, 8_000)

const structuredFor = (
  outcome: McpToolCallOutcome,
): Readonly<Record<string, unknown>> =>
  isRecord(outcome.structuredContent) ? outcome.structuredContent : {}

const numberField = (
  value: Readonly<Record<string, unknown>>,
  key: string,
): number | null =>
  typeof value[key] === 'number' && Number.isFinite(value[key])
    ? (value[key] as number)
    : null

const stringField = (
  value: Readonly<Record<string, unknown>>,
  key: string,
): string | null =>
  typeof value[key] === 'string' && value[key] !== ''
    ? (value[key] as string)
    : null

const stringArrayField = (
  value: Readonly<Record<string, unknown>>,
  key: string,
): ReadonlyArray<string> =>
  Array.isArray(value[key])
    ? (value[key] as ReadonlyArray<unknown>).filter(
        (entry): entry is string =>
          typeof entry === 'string' && entry.length > 0,
      )
    : []

const childResultRefs = (
  value: Readonly<Record<string, unknown>>,
): ReadonlyArray<string> => {
  const children = Array.isArray(value.children) ? value.children : []
  return children.flatMap(child => {
    if (!isRecord(child)) return []
    return [child.assignmentRef, child.workerRef].filter(
      (entry): entry is string => typeof entry === 'string' && entry.length > 0,
    )
  })
}

/** SARAH-PROACTIVE-1 (#9064): the real assignment refs `khala.spawn` actually
 * admitted (`child.ok === true`). Used to durably bind each assignment to
 * this turn's owner+thread at dispatch time, so a later Codex worker_closeout
 * event can post one proactive status message without inferring identity
 * from the agent-token-scoped Pylon assignment record. */
const successfulChildAssignmentRefs = (
  value: Readonly<Record<string, unknown>>,
): ReadonlyArray<string> => {
  const children = Array.isArray(value.children) ? value.children : []
  return children.flatMap(child => {
    if (!isRecord(child) || child.ok !== true) return []
    return typeof child.assignmentRef === 'string' && child.assignmentRef.length > 0
      ? [child.assignmentRef]
      : []
  })
}

const publicRefSegment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

const mcpRequest = (): Request =>
  new Request('https://openagents.com/api/mcp', { method: 'POST' })

const resultFromMcp = (
  outcome: McpToolCallOutcome,
  receiptRef: string,
  summary: string,
  resultRefs: ReadonlyArray<string>,
): SarahAgentToolResult => ({
  authorityReceiptRef: receiptRef,
  content: textForOutcome(outcome),
  isError: outcome.isError === true,
  resultRefs,
  summary,
})

const refused = (
  authority: SarahOperationAuthorityOutcome,
): SarahAgentToolResult => ({
  authorityReceiptRef: authority.receiptRef,
  authorityAllowed: false,
  content: json({
    error: 'authority_refused',
    ok: false,
    reason: authority.refusalReason ?? 'Sarah is not admitted for this action.',
  }),
  isError: true,
  resultRefs: ['blocker.sarah.authority_refused'],
  summary: 'Sarah is not authorized for that action in this owner thread.',
})

/** The admitted Sarah tool set. It deliberately composes existing target
 * brokers instead of becoming a second execution authority. Worker dispatch
 * stays owner-linked and commit-pinned. Full Auto controls remain pending
 * until Desktop applies or rejects them. */
export const makeSarahRuntimeTools = <Bindings>(
  deps: SarahRuntimeToolDependencies<Bindings>,
): ReadonlyArray<SarahAgentTool> => {
  const authorize = deps.authorizeOperation ?? authorizeSarahOperation
  const projection =
    deps.fullAutoProjection ??
    makeFullAutoRunProjectionRepository({ sql: deps.sql })
  const control =
    deps.fullAutoControl ?? makeFullAutoRunControlAuthority({ sql: deps.sql })
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  let reservedWorkerCount = 0
  const principal = () => khalaMcpOwnerPrincipal(deps.ownerUserId, nowIso())
  const callMcp = (
    name: string,
    args: unknown,
  ): Effect.Effect<McpToolCallOutcome, SarahAgentToolError> =>
    toolPromise(
      () =>
        deps.khalaCatalog.callTool(
          deps.env,
          mcpRequest(),
          principal(),
          name,
          args,
        ),
      'mcp_tool_failed',
    )
  const authorizeCall = (
    action: string,
    resource: string,
    toolCallId: string,
    targetEvidenceRefs: ReadonlyArray<string> = [],
  ) =>
    authorize(deps.sql, {
      action,
      ownerUserId: deps.ownerUserId,
      resource,
      targetEvidenceRefs,
      threadRef: deps.threadRef,
      triggerRef: `turn.${publicRefSegment(deps.turnId)}.tool.${publicRefSegment(toolCallId)}`,
    }).pipe(
      Effect.mapError(error =>
        toolFailureFrom(error, 'authority_decision_failed'),
      ),
    )

  const capacity: SarahAgentTool = {
    definition: {
      description:
        'Check the owner-linked Pylon Codex worker capacity available right now.',
      name: 'codex_workers_capacity',
      parameters: {
        additionalProperties: false,
        properties: {},
        type: 'object',
      },
    },
    execute: (_args, toolCall) =>
      Effect.gen(function* () {
        const authority = yield* authorizeCall(
          'inspect_owner_coding_capacity',
          'owner_linked_pylon_coding_capacity',
          toolCall.id,
        )
        if (!authority.allowed) return refused(authority)
        const outcome = yield* callMcp('khala.capacity', {})
        const value = structuredFor(outcome)
        const pylons = Array.isArray(value.pylons) ? value.pylons.length : 0
        return resultFromMcp(
          outcome,
          authority.receiptRef,
          pylons === 0
            ? 'No owner-linked Pylon is currently reporting Codex capacity.'
            : `${pylons} owner-linked Pylon${pylons === 1 ? ' is' : 's are'} reporting capacity.`,
          ['capacity.owner_linked_pylon.codex'],
        )
      }).pipe(
        Effect.mapError(error =>
          toolFailureFrom(error, 'capacity_unavailable'),
        ),
      ),
  }

  const startWorkers: SarahAgentTool = {
    definition: {
      description:
        'Start 1-8 real Codex workers on owner-linked Pylon capacity against a pinned OpenAgents main commit.',
      name: 'codex_workers_start',
      parameters: {
        additionalProperties: false,
        properties: {
          count: { maximum: 8, minimum: 1, type: 'integer' },
          maxParallel: { maximum: 8, minimum: 1, type: 'integer' },
          objective: { maxLength: 8000, minLength: 3, type: 'string' },
        },
        required: ['objective', 'count'],
        type: 'object',
      },
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () =>
            decodeInput(
              value =>
                S.decodeUnknownSync(StartWorkersInput)(value, {
                  onExcessProperty: 'error',
                }),
              raw,
            ),
          catch: error => toolFailureFrom(error, 'invalid_tool_arguments'),
        })
        const authority = yield* authorizeCall(
          'dispatch_owner_capacity_coding_workers',
          'owner_linked_pylon_coding_capacity',
          toolCall.id,
          [`repo:${OWNER_REPOSITORY}`, `branch:${OWNER_REPOSITORY_BRANCH}`],
        )
        if (!authority.allowed) return refused(authority)
        if (reservedWorkerCount + input.count > 8) {
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({
              error: 'sarah_turn_worker_limit_exceeded',
              maximum: 8,
              ok: false,
              requested: input.count,
              reserved: reservedWorkerCount,
            }),
            isError: true,
            resultRefs: ['blocker.sarah.codex_workers.turn_limit'],
            summary:
              'This turn is limited to eight Codex workers, so no additional workers were started.',
          }
        }
        // Reserve before the remote call. A dropped broker response must not
        // make a retry capable of exceeding the per-turn execution ceiling.
        reservedWorkerCount += input.count
        const commit = yield* toolPromise(
          deps.resolveRepositoryCommit ??
            (() =>
              resolveManagedCloudRepositoryCommit(
                OWNER_REPOSITORY,
                OWNER_REPOSITORY_BRANCH,
              )),
          'main_commit_unresolved',
        )
        if (commit === null) {
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({ error: 'main_commit_unresolved', ok: false }),
            isError: true,
            resultRefs: ['blocker.sarah.codex_workers.main_commit_unresolved'],
            summary:
              'I could not resolve an immutable OpenAgents main commit, so no workers were started.',
          }
        }
        const outcome = yield* callMcp('khala.spawn', {
          branch: OWNER_REPOSITORY_BRANCH,
          commit,
          count: input.count,
          maxParallel: input.maxParallel ?? input.count,
          objective: input.objective,
          repo: OWNER_REPOSITORY,
          verify: OWNER_REPOSITORY_VERIFY,
          workflow: 'codex_agent_task',
        })
        const value = structuredFor(outcome)
        const assigned = numberField(value, 'assignedCount') ?? 0
        const requested = numberField(value, 'requestedCount') ?? input.count
        const spawnRef = stringField(value, 'spawnRef')
        const blockers = stringArrayField(value, 'blockerRefs')
        const refs = [spawnRef, ...childResultRefs(value), ...blockers].filter(
          (entry): entry is string => entry !== null,
        )
        // SARAH-PROACTIVE-1 (#9064): capture (ownerUserId, threadRef) for each
        // admitted assignment now, while we are inside this authenticated
        // Sarah turn. Best-effort: a mapping-write failure must never fail
        // this tool call — the real dispatch above already happened, and a
        // missing mapping only means the eventual worker_closeout stays a
        // silent no-op (recorded, but no proactive owner notice).
        const dispatchedAssignmentRefs = successfulChildAssignmentRefs(value)
        if (dispatchedAssignmentRefs.length > 0) {
          yield* Effect.promise(() =>
            Promise.allSettled(
              dispatchedAssignmentRefs.map(assignmentRef =>
                recordSarahWorkerDispatchMapping(deps.sql, {
                  assignmentRef,
                  nowIso: nowIso(),
                  ownerUserId: deps.ownerUserId,
                  threadRef: deps.threadRef,
                }),
              ),
            ),
          )
        }
        return resultFromMcp(
          outcome,
          authority.receiptRef,
          assigned === 0
            ? 'No Codex workers started; owner-linked capacity is unavailable or rejected the request.'
            : `Started ${assigned} of ${requested} requested Codex worker${requested === 1 ? '' : 's'}.`,
          refs.length === 0 ? ['blocker.sarah.codex_workers.no_receipt'] : refs,
        )
      }).pipe(
        Effect.mapError(error =>
          toolFailureFrom(error, 'worker_dispatch_failed'),
        ),
      ),
  }

  const workerStatus: SarahAgentTool = {
    definition: {
      description:
        'Read the real child-assignment status for a prior Sarah Codex worker spawn.',
      name: 'codex_workers_status',
      parameters: {
        additionalProperties: false,
        properties: { spawnRef: { type: 'string' } },
        required: ['spawnRef'],
        type: 'object',
      },
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () =>
            decodeInput(
              value =>
                S.decodeUnknownSync(SpawnStatusInput)(value, {
                  onExcessProperty: 'error',
                }),
              raw,
            ),
          catch: error => toolFailureFrom(error, 'invalid_tool_arguments'),
        })
        const authority = yield* authorizeCall(
          'inspect_owner_coding_capacity',
          'owner_linked_pylon_coding_capacity',
          toolCall.id,
          [input.spawnRef],
        )
        if (!authority.allowed) return refused(authority)
        const outcome = yield* callMcp('khala.spawnStatus', input)
        return resultFromMcp(
          outcome,
          authority.receiptRef,
          outcome.isError === true
            ? 'That Codex worker spawn was not found in the owner-linked capacity.'
            : 'Loaded the current Codex worker assignment status.',
          [input.spawnRef],
        )
      }).pipe(
        Effect.mapError(error =>
          toolFailureFrom(error, 'worker_status_failed'),
        ),
      ),
  }

  const fullAutoStatus: SarahAgentTool = {
    definition: {
      description:
        'Read the public-safe projection of the owner’s existing Desktop Full Auto run.',
      name: 'full_auto_status',
      parameters: {
        additionalProperties: false,
        properties: { runRef: { type: 'string' } },
        type: 'object',
      },
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () =>
            decodeInput(
              value =>
                S.decodeUnknownSync(FullAutoStatusInput)(value, {
                  onExcessProperty: 'error',
                }),
              raw,
            ),
          catch: error => toolFailureFrom(error, 'invalid_tool_arguments'),
        })
        const authority = yield* authorizeCall(
          'inspect_existing_full_auto_run',
          'owner_full_auto_runs',
          toolCall.id,
          input.runRef === undefined ? [] : [input.runRef],
        )
        if (!authority.allowed) return refused(authority)
        const observed = yield* projection.observe({
          ownerUserId: deps.ownerUserId,
        })
        const run = observed.projection.run
        if (
          run === null ||
          (input.runRef !== undefined && input.runRef !== run.runRef)
        ) {
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({ ok: false, run: null }),
            isError: true,
            resultRefs: ['blocker.sarah.full_auto.run_not_found'],
            summary:
              'No matching Desktop Full Auto run is currently projected for this owner.',
          }
        }
        return {
          authorityReceiptRef: authority.receiptRef,
          content: json(observed.projection),
          resultRefs: [run.runRef],
          summary: `Full Auto ${run.runRef} is ${run.lifecycleState}.`,
        }
      }).pipe(
        Effect.mapError(error =>
          toolFailureFrom(error, 'full_auto_status_failed'),
        ),
      ),
  }

  const fullAutoControl: SarahAgentTool = {
    definition: {
      description:
        'Queue a pause, resume, or stop intent for the owner’s existing Desktop Full Auto run. The result remains pending until Desktop applies it.',
      name: 'full_auto_control',
      parameters: {
        additionalProperties: false,
        properties: {
          action: { enum: ['pause', 'resume', 'stop'], type: 'string' },
          runRef: { type: 'string' },
        },
        required: ['action'],
        type: 'object',
      },
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () =>
            decodeInput(
              value =>
                S.decodeUnknownSync(FullAutoControlInput)(value, {
                  onExcessProperty: 'error',
                }),
              raw,
            ),
          catch: error => toolFailureFrom(error, 'invalid_tool_arguments'),
        })
        const authority = yield* authorizeCall(
          'control_existing_full_auto_run',
          'owner_full_auto_runs',
          toolCall.id,
          input.runRef === undefined ? [] : [input.runRef],
        )
        if (!authority.allowed) return refused(authority)
        const observed = yield* projection.observe({
          ownerUserId: deps.ownerUserId,
        })
        const run = observed.projection.run
        if (
          run === null ||
          (input.runRef !== undefined && input.runRef !== run.runRef)
        ) {
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({ ok: false, run: null }),
            isError: true,
            resultRefs: ['blocker.sarah.full_auto.run_not_found'],
            summary:
              'No matching existing Desktop Full Auto run is available to control.',
          }
        }
        const suffix = `${publicRefSegment(deps.turnId).slice(-48)}.${publicRefSegment(toolCall.id).slice(-48)}`
        const intent = yield* control.dispatch({
          ownerUserId: deps.ownerUserId,
          request: {
            action: input.action,
            idempotencyKey: `idempotency.sarah.full_auto.${suffix}`,
            intentId: `intent.sarah.full_auto.${suffix}`,
            runRef: run.runRef,
          },
        })
        return {
          authorityReceiptRef: authority.receiptRef,
          content: json(intent),
          isError: intent.status === 'rejected',
          resultRefs: [intent.intentId, run.runRef],
          summary:
            intent.status === 'pending'
              ? `${input.action} is queued for ${run.runRef}; Desktop has not applied it yet.`
              : `${input.action} is ${intent.status} for ${run.runRef}.`,
        }
      }).pipe(
        Effect.mapError(error =>
          toolFailureFrom(error, 'full_auto_control_failed'),
        ),
      ),
  }

  const authorizeWebComms = (
    action: string,
    resource: string,
    toolCallId: string,
    runtimeAdmitted: boolean,
    targetEvidenceRefs: ReadonlyArray<string>,
    gateEvidenceRef: string,
  ) =>
    authorize(deps.sql, {
      action,
      conditionResults: [
        {
          conditionRef: 'condition.owner_scope',
          evidenceRefs: [`thread:${deps.threadRef}`],
          passed: true,
        },
        {
          conditionRef: 'condition.redaction',
          evidenceRefs: ['schema:openagents.sarah.web_comms_tool.v1'],
          passed: true,
        },
        {
          conditionRef: 'condition.no_unsupported_public_claim',
          evidenceRefs: ['gate:web_comms_public_safe_text'],
          passed: true,
        },
        {
          conditionRef: 'condition.web_comms_runtime_admission',
          evidenceRefs: runtimeAdmitted ? [gateEvidenceRef] : [],
          passed: runtimeAdmitted,
        },
        {
          conditionRef: 'condition.existing_runtime_gate',
          evidenceRefs: [gateEvidenceRef],
          passed: true,
        },
        {
          conditionRef: 'condition.rollback',
          evidenceRefs: ['gate:repository_delivery_revert'],
          passed: true,
        },
      ],
      ownerUserId: deps.ownerUserId,
      programRef: WEB_COMMS_PROGRAM,
      resource,
      targetEvidenceRefs,
      threadRef: deps.threadRef,
      triggerRef: `turn.${publicRefSegment(deps.turnId)}.tool.${publicRefSegment(toolCallId)}`,
    }).pipe(
      Effect.mapError(error =>
        toolFailureFrom(error, 'authority_decision_failed'),
      ),
    )

  const invalidWebCommsArguments = (
    reason: string,
    summary: string,
  ): SarahAgentToolResult => ({
    authorityReceiptRef: 'receipt.sarah.web_comms.invalid_arguments',
    content: json({ error: reason, ok: false }),
    isError: true,
    resultRefs: ['blocker.sarah.web_comms.invalid_arguments'],
    summary,
  })

  const deliveryIntent = (input: {
    action: string
    path: string
    mode: 'create' | 'append'
    title: string
    body: string
    content: string
  }): Readonly<Record<string, unknown>> => ({
    action: input.action,
    body: input.body,
    branch: OWNER_REPOSITORY_BRANCH,
    channel: 'repository_delivery',
    content: input.content,
    mode: input.mode,
    note: 'Delivery runs through the normal coding broker, which re-resolves authority and rollback; this tool writes no files.',
    path: input.path,
    repository: OWNER_REPOSITORY,
    title: input.title,
  })

  const webComms: SarahAgentTool = {
    definition: {
      description:
        'Draft public-safe web communications (blog, document, forum) and deliver them through repository delivery, queue an owner-reviewed X timeline post, or draft a Nostr post. Animated-spoken publication refuses with a receipt until the owner-supplied interfaces and broker are admitted.',
      name: 'sarah_web_comms',
      parameters: {
        additionalProperties: false,
        properties: {
          action: { enum: ['draft', 'publish_outward'], type: 'string' },
          body: { maxLength: 4000, minLength: 3, type: 'string' },
          channel: {
            enum: ['timeline', 'animated_spoken', 'nostr'],
            type: 'string',
          },
          deliver: { type: 'boolean' },
          kind: { enum: ['blog', 'document', 'forum'], type: 'string' },
          title: { maxLength: 200, minLength: 3, type: 'string' },
        },
        required: ['action', 'title', 'body'],
        type: 'object',
      },
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () =>
            decodeInput(
              value =>
                S.decodeUnknownSync(WebCommsInput)(value, {
                  onExcessProperty: 'error',
                }),
              raw,
            ),
          catch: error => toolFailureFrom(error, 'invalid_tool_arguments'),
        })
        // Reject secret-shaped material before any authority receipt is minted.
        const unsafeTags = detectOpenAgentsMcpUnsafeMaterial(
          `${input.title}\n${input.body}`,
        )
        if (unsafeTags.length > 0) {
          return {
            authorityReceiptRef:
              'receipt.sarah.web_comms.unsafe_material_rejected',
            content: json({
              error: 'web_comms_unsafe_material',
              ok: false,
              reason: 'secret_shaped_material_rejected',
              tags: unsafeTags,
            }),
            isError: true,
            resultRefs: ['blocker.sarah.web_comms.unsafe_material'],
            summary:
              'The draft was rejected because the title or body contained secret-shaped material.',
          }
        }
        // Defense in depth: the reject above already blocks known unsafe
        // material, so redaction is a no-op on the passing path.
        const title = redactOpenAgentsMcpUnsafeText(input.title)
        const body = redactOpenAgentsMcpUnsafeText(input.body)
        const date = nowIso().slice(0, 10)

        if (input.action === 'draft') {
          if (input.kind === undefined) {
            return invalidWebCommsArguments(
              'web_comms_kind_required',
              'A draft needs a kind of blog, document, or forum.',
            )
          }
          if (input.channel !== undefined) {
            return invalidWebCommsArguments(
              'web_comms_channel_not_allowed_for_draft',
              'A draft does not take an outward channel.',
            )
          }
          const draftAction =
            input.kind === 'blog'
              ? 'draft_blog_post'
              : input.kind === 'document'
                ? 'draft_document'
                : 'draft_forum_post'
          const draftResource =
            input.kind === 'forum'
              ? 'openagents_github_and_forum'
              : 'openagents_blog_and_documents'
          const authority = yield* authorizeWebComms(
            draftAction,
            draftResource,
            toolCall.id,
            true,
            [`kind:${input.kind}`],
            'gate:repository_delivery',
          )
          if (!authority.allowed) return refused(authority)
          const canDeliver =
            input.deliver === true &&
            (input.kind === 'blog' || input.kind === 'document')
          const delivery = canDeliver
            ? deliveryIntent({
                action: 'deliver_blog_or_document_draft',
                body,
                content: `# ${title}\n\n${body}\n`,
                mode: 'create',
                path:
                  input.kind === 'blog'
                    ? `docs/blog/${date}-${slugify(title)}.md`
                    : `docs/${slugify(title)}.md`,
                title,
              })
            : null
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({
              action: 'draft',
              draft: { body, kind: input.kind, title },
              ok: true,
              ...(delivery === null ? {} : { delivery }),
            }),
            resultRefs: [
              authority.receiptRef,
              delivery === null
                ? `sarah.web_comms.${input.kind}_draft_ready`
                : 'sarah.web_comms.blog_or_document_delivery_intent',
            ],
            summary:
              delivery === null
                ? `Prepared a public-safe ${input.kind} draft.`
                : `Prepared a ${input.kind} draft and a repository-delivery handoff.`,
          }
        }

        // action === 'publish_outward'
        if (input.channel === undefined) {
          return invalidWebCommsArguments(
            'web_comms_channel_required',
            'An outward publication needs a channel of timeline, nostr, or animated_spoken.',
          )
        }
        if (input.kind !== undefined) {
          return invalidWebCommsArguments(
            'web_comms_kind_not_allowed_for_publish',
            'An outward publication does not take a draft kind.',
          )
        }

        if (input.channel === 'timeline') {
          // X is not automated. Queue a public-safe draft for the owner to
          // review and post by hand through repository delivery.
          const authority = yield* authorizeWebComms(
            'publish_outward_communication',
            'openagents_public_timeline',
            toolCall.id,
            true,
            ['channel:timeline'],
            'gate:owner_tweet_queue',
          )
          if (!authority.allowed) return refused(authority)
          const delivery = deliveryIntent({
            action: 'publish_outward_communication',
            body,
            content: `### ${date} — ${title} · status: draft\n\n> ${body}\n`,
            mode: 'append',
            path: SARAH_TWEET_QUEUE_PATH,
            title,
          })
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({
              channel: 'timeline',
              delivery,
              draft: { body, title },
              ok: true,
              queued: true,
            }),
            resultRefs: [
              authority.receiptRef,
              'sarah.web_comms.queued_for_owner_review',
            ],
            summary:
              'Queued a public-safe X timeline draft for the owner to review and post by hand.',
          }
        }

        if (input.channel === 'nostr') {
          // Nostr is an open channel. Produce a public-safe draft and a
          // repository-delivery handoff; live relay posting is a later lane.
          const authority = yield* authorizeWebComms(
            'publish_outward_communication',
            'openagents_public_timeline',
            toolCall.id,
            true,
            ['channel:nostr'],
            'gate:nostr_open_channel',
          )
          if (!authority.allowed) return refused(authority)
          const delivery = deliveryIntent({
            action: 'publish_outward_communication',
            body,
            content: `# ${title}\n\nstatus: draft\n\n${body}\n`,
            mode: 'create',
            path: `docs/sarah/nostr/${date}-${slugify(title)}.md`,
            title,
          })
          return {
            authorityReceiptRef: authority.receiptRef,
            content: json({
              channel: 'nostr',
              delivery,
              draft: { body, title },
              ok: true,
            }),
            resultRefs: [
              authority.receiptRef,
              'sarah.web_comms.nostr_draft_ready',
            ],
            summary:
              'Prepared a public-safe Nostr draft and a repository-delivery handoff; no live relay posting yet.',
          }
        }

        // channel === 'animated_spoken': refuse until admission.
        const authority = yield* authorizeWebComms(
          'publish_animated_spoken_communication',
          'openagents_animated_spoken_channel',
          toolCall.id,
          false,
          ['channel:animated_spoken'],
          'gate:web_comms_broker',
        )
        return {
          authorityAllowed: false,
          authorityReceiptRef: authority.receiptRef,
          content: json({
            channel: 'animated_spoken',
            cite: 'The owner-supplied animation and speech interfaces and the web-communications broker are not yet deployed, healthy, or receipt-capable.',
            error: 'web_comms_runtime_unavailable',
            ok: false,
            reason: 'refuse_until_admission',
          }),
          isError: true,
          resultRefs: [
            authority.receiptRef,
            'blocker.sarah.web_comms.runtime_unavailable',
          ],
          summary:
            'Animated-spoken publication refuses until the owner-supplied animation and speech interfaces and the web-communications broker are admitted.',
        }
      }).pipe(
        Effect.mapError(error => toolFailureFrom(error, 'web_comms_failed')),
      ),
  }

  const harnessStatus: SarahAgentTool | undefined =
    deps.harnessStatus === undefined
      ? undefined
      : {
          definition: {
            description:
              'Read the immutable conversational harness bundle bound for Sarah turns and the latest independent review state.',
            name: 'sarah_harness_status',
            parameters: {
              additionalProperties: false,
              properties: {},
              type: 'object',
            },
          },
          execute: (_raw, toolCall) =>
            Effect.gen(function* () {
              const authority = yield* authorizeCall(
                'inspect_own_harness',
                'owner_private_sarah_harness',
                toolCall.id,
              )
              if (!authority.allowed) return refused(authority)
              const status = yield* deps.harnessStatus!().pipe(
                Effect.mapError(error =>
                  toolFailureFrom(error, 'harness_status_failed'),
                ),
              )
              return {
                authorityReceiptRef: authority.receiptRef,
                content: json({
                  bundleDigest: status.bundleDigest,
                  bundleRef: status.bundleRef,
                  latestReviewRef: status.latestReviewRef,
                  latestReviewState: status.latestReviewState,
                  maxReplyWords: status.policy.maxReplyWords,
                  ok: true,
                }),
                resultRefs: [
                  status.bundleRef,
                  ...(status.latestReviewRef === undefined
                    ? []
                    : [status.latestReviewRef]),
                ],
                summary:
                  'Loaded Sarah’s currently released conversational harness.',
              }
            }),
        }

  const harnessReview: SarahAgentTool | undefined =
    deps.reviewHarness === undefined
      ? undefined
      : {
          definition: {
            description:
              'Review Sarah’s completed owner-thread history, compile private terminal experiences, and submit a bounded conversational harness candidate to an independent evaluator and release gate.',
            name: 'sarah_harness_review_history',
            parameters: {
              additionalProperties: false,
              properties: {},
              type: 'object',
            },
          },
          execute: (_raw, toolCall) =>
            Effect.gen(function* () {
              const authority = yield* authorizeCall(
                'review_own_terminal_history_and_propose_harness',
                'owner_private_sarah_harness',
                toolCall.id,
              )
              if (!authority.allowed) return refused(authority)
              const reviewed = yield* deps.reviewHarness!().pipe(
                Effect.mapError(error =>
                  toolFailureFrom(error, 'harness_review_failed'),
                ),
              )
              return {
                authorityReceiptRef: authority.receiptRef,
                content: json({
                  bundleDigest: reviewed.bundleDigest,
                  bundleRef: reviewed.bundleRef,
                  evaluation: reviewed.evaluation,
                  experienceCount: reviewed.experienceCount,
                  heldOutExperienceCount: reviewed.heldOutExperienceCount,
                  ok: true,
                  reviewRef: reviewed.reviewRef,
                  state: reviewed.state,
                  summary: reviewed.summary,
                  trainingExperienceCount: reviewed.trainingExperienceCount,
                }),
                resultRefs: [reviewed.reviewRef, reviewed.bundleRef],
                summary:
                  reviewed.state === 'released'
                    ? 'The independent gate released an improved harness for Sarah’s next turn.'
                    : 'The independent gate rejected the harness candidate; the current harness remains active.',
              }
            }),
        }

  return [
    capacity,
    startWorkers,
    workerStatus,
    fullAutoStatus,
    fullAutoControl,
    webComms,
    ...(harnessStatus === undefined ? [] : [harnessStatus]),
    ...(harnessReview === undefined ? [] : [harnessReview]),
    ...(deps.managedSandboxTools ?? []),
  ]
}
