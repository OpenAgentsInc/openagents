import { Buffer } from "node:buffer"
import { Effect } from "effect"
import {
  decideAgentDefinitionCompiledToolAuthority,
  type AgentDefinitionCompiledToolRuntimePolicy,
  type AgentDefinitionToolAuthorityDecision,
} from "@openagentsinc/agent-runtime-schema"
import { redactKhalaPublicText } from "./redaction.js"
import { makeKhalaToolRuntimeService, type KhalaToolRuntimeServiceShape } from "./runtime.js"
import type {
  KhalaPermissionRequest,
  KhalaPublicSafety,
  KhalaToolArtifact,
  KhalaToolDefinition,
  KhalaToolEvent,
  KhalaToolEventKind,
  KhalaToolInvocation,
  KhalaToolRegistry,
  KhalaToolResult,
  KhalaToolResultStatus,
  KhalaToolRuntimeError,
  KhalaToolServices,
  RegisteredKhalaTool,
} from "./index.js"

export type KhalaToolTelemetryTagValue = string | number | boolean
export type KhalaToolTelemetryTags = Readonly<Record<string, KhalaToolTelemetryTagValue>>

export type KhalaToolDispatchPhase =
  | "resolve"
  | "validate"
  | "permission"
  | "execute"
  | "bound_output"
  | "completed"
  | "failed"

export type KhalaToolDispatchHookContext = Readonly<{
  definition: KhalaToolDefinition | undefined
  invocation: KhalaToolInvocation
  phase: KhalaToolDispatchPhase
  services: KhalaToolServices
  telemetryTags: KhalaToolTelemetryTags
  tool: RegisteredKhalaTool | undefined
}>

export type KhalaToolDispatchAfterHookContext = KhalaToolDispatchHookContext & Readonly<{
  durationMs: number
  result: KhalaToolResult
}>

export type KhalaToolDispatchEventContext = Readonly<{
  event: KhalaToolEvent
  telemetryTags: KhalaToolTelemetryTags
}>

export interface KhalaToolDispatchHooks {
  readonly afterTool?: (context: KhalaToolDispatchAfterHookContext) => Effect.Effect<void, never>
  readonly beforeTool?: (context: KhalaToolDispatchHookContext) => Effect.Effect<void, never>
  readonly onEvent?: (context: KhalaToolDispatchEventContext) => Effect.Effect<void, never>
}

export interface KhalaToolTurnAccounting {
  readonly maxToolCalls?: number
  readonly recordToolCall: (input: Readonly<{
    invocation: KhalaToolInvocation
    telemetryTags: KhalaToolTelemetryTags
  }>) => Effect.Effect<Readonly<{ count: number }>, never>
  readonly snapshot: () => KhalaToolTurnAccountingSnapshot
  readonly turnId: string
}

export type KhalaToolTurnAccountingSnapshot = Readonly<{
  maxToolCalls?: number
  toolCallCount: number
  turnId: string
}>

export type KhalaToolDispatcherOptions = Readonly<{
  agentDefinitionToolPolicy?: AgentDefinitionCompiledToolRuntimePolicy
  hooks?: KhalaToolDispatchHooks
  maxModelOutputBytes?: number
  maxPublicSummaryBytes?: number
  telemetryTags?: KhalaToolTelemetryTags
  turnAccounting?: KhalaToolTurnAccounting
}>

export type KhalaToolDispatchInput = Readonly<{
  invocation: KhalaToolInvocation
  registry: KhalaToolRegistry
  services: KhalaToolServices
  telemetryTags?: KhalaToolTelemetryTags
}>

export type KhalaToolDispatchResult = Readonly<{
  accounting?: KhalaToolTurnAccountingSnapshot
  events: ReadonlyArray<KhalaToolEvent>
  result: KhalaToolResult
  telemetryTags: KhalaToolTelemetryTags
}>

export interface KhalaToolDispatcher {
  readonly dispatch: (input: KhalaToolDispatchInput) => Effect.Effect<KhalaToolDispatchResult, never>
}

export type KhalaAgentDefinitionToolPolicyDecision =
  | Readonly<{
      status: "allowed"
      toolRefs: ReadonlyArray<string>
      decisions: ReadonlyArray<AgentDefinitionToolAuthorityDecision>
    }>
  | Readonly<{
      status: "denied"
      toolRefs: ReadonlyArray<string>
      decisions: ReadonlyArray<AgentDefinitionToolAuthorityDecision>
      blockerRefs: ReadonlyArray<string>
      reasonRef: string
    }>
  | Readonly<{
      status: "operator_escalation_required"
      toolRefs: ReadonlyArray<string>
      decisions: ReadonlyArray<AgentDefinitionToolAuthorityDecision>
      blockerRefs: ReadonlyArray<string>
      escalation: NonNullable<AgentDefinitionToolAuthorityDecision["escalation"]>
      reasonRef: string
    }>

const DEFAULT_MAX_MODEL_OUTPUT_BYTES = 64 * 1024
const DEFAULT_MAX_PUBLIC_SUMMARY_BYTES = 8 * 1024

export function createKhalaToolTurnAccounting(input: Readonly<{
  maxToolCalls?: number
  turnId: string
}>): KhalaToolTurnAccounting {
  let toolCallCount = 0
  return {
    ...(input.maxToolCalls === undefined ? {} : { maxToolCalls: input.maxToolCalls }),
    recordToolCall: () =>
      Effect.sync(() => {
        toolCallCount += 1
        return { count: toolCallCount }
      }),
    snapshot: () => ({
      ...(input.maxToolCalls === undefined ? {} : { maxToolCalls: input.maxToolCalls }),
      toolCallCount,
      turnId: input.turnId,
    }),
    turnId: input.turnId,
  }
}

export function makeKhalaToolDispatcher(options: KhalaToolDispatcherOptions = {}): KhalaToolDispatcher {
  return {
    dispatch: input => dispatchKhalaTool(input, options),
  }
}

function dispatchKhalaTool(
  input: KhalaToolDispatchInput,
  options: KhalaToolDispatcherOptions,
): Effect.Effect<KhalaToolDispatchResult, never> {
  return Effect.gen(function* () {
    const runtime = input.services.runtime ?? makeKhalaToolRuntimeService()
    const startedAt = yield* runtime.currentTimeMillis
    const localEvents: KhalaToolEvent[] = []
    const baseTelemetryTags = telemetryTagsFor(input, options)
    const accounting = options.turnAccounting === undefined
      ? undefined
      : yield* options.turnAccounting.recordToolCall({
          invocation: input.invocation,
          telemetryTags: baseTelemetryTags,
        })
    const telemetryTags = {
      ...baseTelemetryTags,
      ...(options.turnAccounting === undefined ? {} : { turnId: options.turnAccounting.turnId }),
      ...(accounting === undefined ? {} : { toolCallIndex: accounting.count }),
    }

    const fail = (
      code: string,
      reason: string,
      phase: KhalaToolDispatchPhase,
      definition: KhalaToolDefinition | undefined = undefined,
    ) =>
      finalizeWithDuration(runtime, startedAt, {
        definition,
        events: localEvents,
        input,
        options,
        phase,
        result: dispatcherToolError(code, reason),
        telemetryTags,
        tool: undefined,
      })

    if (accounting !== undefined && options.turnAccounting?.maxToolCalls !== undefined) {
      if (accounting.count > options.turnAccounting.maxToolCalls) {
        yield* emitToolEvent({
          events: localEvents,
          input,
          kind: "tool_failed",
          options,
          payload: {
            code: "tool_call_limit_exceeded",
            limit: options.turnAccounting.maxToolCalls,
            phase: "validate",
          },
          telemetryTags,
        })
        return yield* fail(
          "tool_call_limit_exceeded",
          `Tool call limit exceeded for turn ${options.turnAccounting.turnId}`,
          "validate",
        )
      }
    }

    yield* emitToolEvent({
      events: localEvents,
      input,
      kind: "tool_started",
      options,
      payload: { phase: "resolve" },
      telemetryTags,
    })

    const tool = input.registry.resolve(input.invocation.name)
    if (tool === undefined) {
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: { code: "unknown_tool", phase: "resolve" },
        telemetryTags,
      })
      return yield* fail("unknown_tool", `Unknown tool: ${input.invocation.name}`, "resolve")
    }
    const definition = tool.definition
    const definitionTags = telemetryTagsFor(input, options, definition)
    const taggedTelemetry = { ...definitionTags, ...telemetryTags }

    yield* beforeTool(options, {
      definition,
      invocation: input.invocation,
      phase: "validate",
      services: input.services,
      telemetryTags: taggedTelemetry,
      tool,
    })
    if (tool.execute === undefined) {
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: { code: "missing_handler", phase: "validate", toolName: definition.name },
        telemetryTags: taggedTelemetry,
      })
      return yield* fail("missing_handler", `Tool has no execute handler: ${input.invocation.name}`, "validate", definition)
    }
    if (!isRecord(input.invocation.arguments)) {
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: { code: "invalid_arguments", phase: "validate", toolName: definition.name },
        telemetryTags: taggedTelemetry,
      })
      return yield* fail("invalid_arguments", "Invalid tool input: expected an object", "validate", definition)
    }
    const agentPolicyDecision = decideKhalaToolAgainstAgentDefinitionPolicy(
      definition,
      options.agentDefinitionToolPolicy,
      input.invocation.id,
    )
    if (agentPolicyDecision.status === "denied") {
      const result = dispatcherToolDenied(
        "agent_definition_tool_policy_denied",
        `${definition.name} is outside the compiled agent-definition tool policy`,
      )
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: {
          blockerRefs: agentPolicyDecision.blockerRefs,
          phase: "permission",
          reasonRef: agentPolicyDecision.reasonRef,
          status: result.status,
          toolName: definition.name,
          toolRefs: agentPolicyDecision.toolRefs,
        },
        telemetryTags: taggedTelemetry,
      })
      return yield* finalize({
        definition,
        durationMs: (yield* runtime.currentTimeMillis) - startedAt,
        events: localEvents,
        input,
        options,
        phase: "permission",
        result,
        telemetryTags: taggedTelemetry,
        tool,
      })
    }
    if (agentPolicyDecision.status === "operator_escalation_required") {
      const result = dispatcherToolNeedsInput({
        modelText: "Operator approval is required before this tool can run.",
        publicSummary: "Agent-definition tool policy requires operator escalation.",
        ui: {
          kind: "agent_definition_tool_policy_escalation",
          blockerRefs: agentPolicyDecision.blockerRefs,
          escalation: agentPolicyDecision.escalation,
          reasonRef: agentPolicyDecision.reasonRef,
          toolName: definition.name,
          toolRefs: agentPolicyDecision.toolRefs,
        },
      })
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "approval_requested",
        options,
        payload: {
          blockerRefs: agentPolicyDecision.blockerRefs,
          escalation: agentPolicyDecision.escalation,
          phase: "permission",
          reasonRef: agentPolicyDecision.reasonRef,
          status: result.status,
          toolName: definition.name,
          toolRefs: agentPolicyDecision.toolRefs,
        },
        telemetryTags: taggedTelemetry,
      })
      return yield* finalize({
        definition,
        durationMs: (yield* runtime.currentTimeMillis) - startedAt,
        events: localEvents,
        input,
        options,
        phase: "permission",
        result,
        telemetryTags: taggedTelemetry,
        tool,
      })
    }
    if (definition.permissionMode === "deny") {
      const result = dispatcherToolDenied("permission_policy_denied", `${definition.name} is denied by policy`)
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: { phase: "permission", status: result.status, toolName: definition.name },
        telemetryTags: taggedTelemetry,
      })
      return yield* finalize({
        definition,
        durationMs: (yield* runtime.currentTimeMillis) - startedAt,
        events: localEvents,
        input,
        options,
        phase: "permission",
        result,
        telemetryTags: taggedTelemetry,
        tool,
      })
    }

    yield* emitToolEvent({
      events: localEvents,
      input,
      kind: "tool_progress",
      options,
      payload: { phase: "permission", permissionMode: definition.permissionMode, toolName: definition.name },
      telemetryTags: taggedTelemetry,
    })
    const decision = definition.permissionMode === "approval_required"
      ? yield* requestPermission(input, options, definition, localEvents, taggedTelemetry)
      : "allow"
    if (decision === "deny") {
      const result = dispatcherToolDenied("permission_denied", `${definition.name} denied by permission service`)
      yield* emitToolEvent({
        events: localEvents,
        input,
        kind: "tool_failed",
        options,
        payload: { phase: "permission", status: result.status, toolName: definition.name },
        telemetryTags: taggedTelemetry,
      })
      return yield* finalize({
        definition,
        durationMs: (yield* runtime.currentTimeMillis) - startedAt,
        events: localEvents,
        input,
        options,
        phase: "permission",
        result,
        telemetryTags: taggedTelemetry,
        tool,
      })
    }

    yield* emitToolEvent({
      events: localEvents,
      input,
      kind: "tool_progress",
      options,
      payload: { phase: "execute", toolName: definition.name },
      telemetryTags: taggedTelemetry,
    })

    const rawResult = yield* tool.execute(input.invocation.arguments, {
      definition,
      emitProgress: payload =>
        emitToolEvent({
          events: localEvents,
          input,
          kind: "tool_progress",
          options,
          payload,
          telemetryTags: taggedTelemetry,
        }),
      invocation: input.invocation,
      services: input.services,
    }).pipe(
      Effect.map(sanitizeDispatcherToolResult),
      Effect.catchTag("KhalaToolRuntimeError", (error: KhalaToolRuntimeError) =>
        Effect.succeed(dispatcherToolError(error.code, error.reason)),
      ),
    )
    const boundedResult = yield* enforceOutputBounds(input, options, definition, rawResult)
    const phase: KhalaToolDispatchPhase = isFailedStatus(boundedResult.status) ? "failed" : "completed"
    yield* emitToolEvent({
      events: localEvents,
      input,
      kind: isFailedStatus(boundedResult.status) ? "tool_failed" : "tool_completed",
      options,
      payload: {
        artifactCount: boundedResult.artifacts.length,
        phase,
        privateDataRefCount: boundedResult.privateDataRefs.length,
        status: boundedResult.status,
        toolName: definition.name,
      },
      telemetryTags: taggedTelemetry,
    })
    return yield* finalize({
      definition,
      durationMs: (yield* runtime.currentTimeMillis) - startedAt,
      events: localEvents,
      input,
      options,
      phase,
      result: boundedResult,
      telemetryTags: taggedTelemetry,
      tool,
    })
  })
}

export function khalaToolRefsForAgentDefinitionPolicy(
  definition: KhalaToolDefinition,
): ReadonlyArray<string> {
  return [
    `tool.openagents.khala.${definition.name}`,
    `tool.openagents.khala.authority.${definition.authority}`,
  ]
}

export function decideKhalaToolAgainstAgentDefinitionPolicy(
  definition: KhalaToolDefinition,
  policy: AgentDefinitionCompiledToolRuntimePolicy | undefined,
  invocationRef?: string,
): KhalaAgentDefinitionToolPolicyDecision {
  const toolRefs = khalaToolRefsForAgentDefinitionPolicy(definition)
  if (policy === undefined) {
    return { status: "allowed", toolRefs, decisions: [] }
  }
  const decisions = toolRefs.map((toolRef) =>
    decideAgentDefinitionCompiledToolAuthority({
      policy,
      toolRef,
      ...(invocationRef === undefined ? {} : { invocationRef: `${invocationRef}:${toolRef}` }),
    }),
  )
  const explicitDeny = decisions.find(
    decision =>
      decision.status === "denied" &&
      decision.reasonRef === "reason.agent_definition.tool_denied",
  )
  if (explicitDeny !== undefined) {
    return {
      status: "denied",
      toolRefs,
      decisions,
      blockerRefs: explicitDeny.blockerRefs,
      reasonRef: explicitDeny.reasonRef,
    }
  }
  const escalation = decisions.find(
    (decision): decision is AgentDefinitionToolAuthorityDecision & {
      escalation: NonNullable<AgentDefinitionToolAuthorityDecision["escalation"]>
    } =>
      decision.status === "operator_escalation_required" &&
      decision.escalation !== undefined,
  )
  if (escalation !== undefined) {
    return {
      status: "operator_escalation_required",
      toolRefs,
      decisions,
      blockerRefs: escalation.blockerRefs,
      escalation: escalation.escalation,
      reasonRef: escalation.reasonRef,
    }
  }
  if (decisions.some(decision => decision.status === "allowed")) {
    return { status: "allowed", toolRefs, decisions }
  }
  const denied = decisions[0]
  return {
    status: "denied",
    toolRefs,
    decisions,
    blockerRefs: denied?.blockerRefs ?? ["blocker.agent_definition.tool_not_in_allowlist"],
    reasonRef: denied?.reasonRef ?? "reason.agent_definition.tool_not_in_allowlist",
  }
}

function requestPermission(
  input: KhalaToolDispatchInput,
  options: KhalaToolDispatcherOptions,
  definition: KhalaToolDefinition,
  events: KhalaToolEvent[],
  telemetryTags: KhalaToolTelemetryTags,
) {
  return Effect.gen(function* () {
    const request = permissionRequestFor(definition, input.invocation, input.services)
    yield* emitToolEvent({
      events,
      input,
      kind: "approval_requested",
      options,
      payload: request,
      telemetryTags,
    })
    const decision = yield* input.services.permission.decide(request)
    yield* emitToolEvent({
      events,
      input,
      kind: "approval_answered",
      options,
      payload: {
        decision,
        toolCallId: input.invocation.id,
        toolName: definition.name,
      },
      telemetryTags,
    })
    return decision
  })
}

function beforeTool(options: KhalaToolDispatcherOptions, context: KhalaToolDispatchHookContext): Effect.Effect<void, never> {
  return options.hooks?.beforeTool?.(context) ?? Effect.void
}

function finalize(input: Readonly<{
  definition: KhalaToolDefinition | undefined
  durationMs: number
  events: KhalaToolEvent[]
  input: KhalaToolDispatchInput
  options: KhalaToolDispatcherOptions
  phase: KhalaToolDispatchPhase
  result: KhalaToolResult
  telemetryTags: KhalaToolTelemetryTags
  tool: RegisteredKhalaTool | undefined
}>): Effect.Effect<KhalaToolDispatchResult, never> {
  return Effect.gen(function* () {
    yield* (input.options.hooks?.afterTool?.({
      definition: input.definition,
      durationMs: input.durationMs,
      invocation: input.input.invocation,
      phase: input.phase,
      result: input.result,
      services: input.input.services,
      telemetryTags: input.telemetryTags,
      tool: input.tool,
    }) ?? Effect.void)
    return {
      ...(input.options.turnAccounting === undefined ? {} : { accounting: input.options.turnAccounting.snapshot() }),
      events: input.events,
      result: input.result,
      telemetryTags: input.telemetryTags,
    }
  })
}

function emitToolEvent(input: Readonly<{
  events: KhalaToolEvent[]
  input: KhalaToolDispatchInput
  kind: KhalaToolEventKind
  options: KhalaToolDispatcherOptions
  payload: unknown
  telemetryTags: KhalaToolTelemetryTags
}>): Effect.Effect<void, never> {
  return Effect.gen(function* () {
    const event: KhalaToolEvent = {
      eventId: yield* nextToolEventId(input.input.services.runtime, input.kind),
      invocationId: input.input.invocation.id,
      kind: input.kind,
      payload: {
        ...(isRecord(input.payload) ? input.payload : { value: input.payload }),
        telemetryTags: input.telemetryTags,
      },
      sessionId: input.input.invocation.sessionId,
    }
    input.events.push(event)
    yield* (input.options.hooks?.onEvent?.({ event, telemetryTags: input.telemetryTags }) ?? Effect.void)
  })
}

function finalizeWithDuration(inputRuntime: KhalaToolRuntimeServiceShape, startedAt: number, input: Readonly<{
  definition: KhalaToolDefinition | undefined
  events: KhalaToolEvent[]
  input: KhalaToolDispatchInput
  options: KhalaToolDispatcherOptions
  phase: KhalaToolDispatchPhase
  result: KhalaToolResult
  telemetryTags: KhalaToolTelemetryTags
  tool: RegisteredKhalaTool | undefined
}>): Effect.Effect<KhalaToolDispatchResult, never> {
  return Effect.gen(function* () {
    const finishedAt = yield* inputRuntime.currentTimeMillis
    return yield* finalize({
      ...input,
      durationMs: finishedAt - startedAt,
    })
  })
}

function enforceOutputBounds(
  input: KhalaToolDispatchInput,
  options: KhalaToolDispatcherOptions,
  definition: KhalaToolDefinition,
  result: KhalaToolResult,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.gen(function* () {
    const maxModelOutputBytes = options.maxModelOutputBytes ?? DEFAULT_MAX_MODEL_OUTPUT_BYTES
    const maxPublicSummaryBytes = options.maxPublicSummaryBytes ?? DEFAULT_MAX_PUBLIC_SUMMARY_BYTES
    const modelText = result.modelOutput.text
    const modelBytes = Buffer.byteLength(modelText, "utf8")
    const publicSummary = boundUtf8(result.publicSummary, maxPublicSummaryBytes)
    if (modelBytes <= maxModelOutputBytes) {
      return publicSummary.truncated
        ? {
            ...result,
            publicSummary: `${publicSummary.text}\n[tool public summary truncated by dispatcher]`,
          }
        : result
    }

    const artifact = yield* input.services.outputStore.writeArtifact({
      bytes: Buffer.from(modelText, "utf8"),
      mediaType: "text/plain; charset=utf-8",
      summary: `dispatcher-bounded model output for ${definition.name}`,
    }).pipe(
      Effect.catchTag("KhalaToolRuntimeError", (error: KhalaToolRuntimeError) =>
        Effect.succeed({
          artifactRef: `artifact.unavailable.${input.invocation.id}`,
          private: true,
          summary: redactKhalaPublicText(`artifact write failed: ${error.reason}`),
        } satisfies KhalaToolArtifact),
      ),
    )
    const bounded = boundUtf8(modelText, maxModelOutputBytes)
    return {
      ...result,
      artifacts: [...result.artifacts, artifact],
      modelOutput: {
        text: `${bounded.text}\n[tool output truncated by dispatcher; full output stored in private artifact ${artifact.artifactRef}]`,
      },
      privateDataRefs: [...new Set([...result.privateDataRefs, artifact.artifactRef])],
      publicSummary: publicSummary.truncated
        ? `${publicSummary.text}\n[tool public summary truncated by dispatcher]`
        : result.publicSummary,
    }
  })
}

function telemetryTagsFor(
  input: KhalaToolDispatchInput,
  options: KhalaToolDispatcherOptions,
  definition?: KhalaToolDefinition,
): KhalaToolTelemetryTags {
  return {
    ...options.telemetryTags,
    ...input.telemetryTags,
    dispatcher: "khala_tool_dispatcher",
    schemaVersion: "khala.tool.dispatch.v1",
    toolCallId: input.invocation.id,
    toolName: input.invocation.name,
    ...(definition === undefined
      ? {}
      : {
          toolAuthority: definition.authority,
          toolExecutionMode: definition.executionMode,
          toolInternalId: definition.internalId,
          toolPermissionMode: definition.permissionMode,
        }),
  }
}

function dispatcherToolError(code: string, reason: string): KhalaToolResult {
  const safe = redactKhalaPublicText(`${code}: ${reason}`)
  return {
    artifacts: [],
    modelOutput: { text: safe },
    privateDataRefs: [],
    publicSafety: safe === `${code}: ${reason}` ? "public_safe" : "redacted",
    publicSummary: safe,
    redactionRefs: safe === `${code}: ${reason}` ? [] : ["redaction.khala_tool.error"],
    status: "failed",
    ui: {
      code,
      kind: "khala_tool_error",
      reason: safe,
    },
  }
}

function dispatcherToolDenied(code: string, reason: string): KhalaToolResult {
  const error = dispatcherToolError(code, reason)
  return { ...error, status: "denied" }
}

function dispatcherToolNeedsInput(input: {
  readonly modelText: string
  readonly publicSummary: string
  readonly ui: unknown
}): KhalaToolResult {
  const modelText = redactKhalaPublicText(input.modelText)
  const publicSummary = redactKhalaPublicText(input.publicSummary)
  const redacted = modelText !== input.modelText || publicSummary !== input.publicSummary
  return {
    artifacts: [],
    modelOutput: { text: modelText },
    privateDataRefs: [],
    publicSafety: redacted ? "redacted" : "public_safe",
    publicSummary,
    redactionRefs: redacted ? ["redaction.khala_tool.needs_input"] : [],
    status: "needs_input",
    ui: input.ui,
  }
}

function sanitizeDispatcherToolResult(result: KhalaToolResult): KhalaToolResult {
  const publicSummary = redactKhalaPublicText(result.publicSummary)
  const modelText = redactKhalaPublicText(result.modelOutput.text)
  const redacted = publicSummary !== result.publicSummary || modelText !== result.modelOutput.text
  return {
    ...result,
    modelOutput: { text: modelText },
    publicSafety: redacted ? "redacted" : result.publicSafety,
    publicSummary,
    redactionRefs: redacted
      ? [...new Set([...result.redactionRefs, "redaction.khala_tool.public_text"])]
      : result.redactionRefs,
  }
}

function permissionRequestFor(
  definition: KhalaToolDefinition,
  invocation: KhalaToolInvocation,
  services: KhalaToolServices,
): KhalaPermissionRequest {
  const resources = typeof invocation.arguments.path === "string"
    ? [invocation.arguments.path]
    : typeof invocation.arguments.url === "string"
      ? [invocation.arguments.url]
      : typeof invocation.arguments.query === "string"
        ? [invocation.arguments.query]
        : typeof invocation.arguments.selector === "string"
          ? [invocation.arguments.selector]
          : typeof invocation.arguments.label === "string"
            ? [invocation.arguments.label]
            : typeof invocation.arguments.value === "string"
              ? [invocation.arguments.value]
              : []
  return {
    action: definition.authority,
    authorityMode: definition.executionMode,
    publicSafety: "private" satisfies KhalaPublicSafety,
    resources,
    saveScope: "once",
    sessionId: invocation.sessionId,
    toolCallId: invocation.id,
    toolName: definition.name,
    workingDirectory: services.workspace.workingDirectory,
  }
}

function boundUtf8(text: string, maxBytes: number): Readonly<{ text: string; truncated: boolean }> {
  const bytes = Buffer.from(text, "utf8")
  if (bytes.byteLength <= maxBytes) return { text, truncated: false }
  return {
    text: bytes.subarray(0, Math.max(0, maxBytes)).toString("utf8").replace(/\uFFFD$/u, ""),
    truncated: true,
  }
}

function isFailedStatus(status: KhalaToolResultStatus): boolean {
  return status === "failed" || status === "denied"
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function nextToolEventId(
  runtime: KhalaToolRuntimeServiceShape | undefined,
  kind: KhalaToolEventKind,
): Effect.Effect<string, never> {
  return (runtime ?? makeKhalaToolRuntimeService()).eventId(`khala.tool.${kind}`)
}
