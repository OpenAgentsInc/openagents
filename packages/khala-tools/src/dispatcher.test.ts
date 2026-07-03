import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  compileAgentDefinitionToolRuntimePolicy,
  decodeAgentDefinition,
  type AgentDefinitionToolset,
} from "@openagentsinc/agent-runtime-schema"
import {
  createKhalaToolTurnAccounting,
  executeKhalaTool,
  khalaToolOk,
  makeDeterministicKhalaToolRuntimeService,
  makeKhalaToolDispatcher,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaToolDefinition,
  type KhalaToolEvent,
} from "./index.js"

// Behavior contract oracle: background_agents.toolset.compiled_policy_enforced.v1

const echoDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect", "coding"],
  description: "Echo input text.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: { text: { type: "string" } },
    required: ["text"],
    type: "object",
  },
  internalId: "khala.test.dispatcher.echo",
  label: "Echo",
  name: "echo",
  permissionMode: "allow",
  prompt: "Echo text.",
  promptGuidelines: ["Use for dispatcher tests only."],
}

const shellDefinition: KhalaToolDefinition = {
  ...echoDefinition,
  authority: "shell",
  description: "Run a command.",
  internalId: "khala.test.dispatcher.shell",
  label: "Shell",
  name: "exec_command",
  prompt: "Run a command.",
}

function compiledPolicy(toolset: AgentDefinitionToolset) {
  return compileAgentDefinitionToolRuntimePolicy(
    decodeAgentDefinition({
      schema: "openagents.agent_definition.v1",
      id: "agent_definition.khala_tools.dispatcher_policy",
      ownerRef: "agent:dispatcher_policy_owner",
      name: "Dispatcher Policy",
      slug: "dispatcher-policy",
      goal: "Exercise the compiled tool runtime policy.",
      harness: { kind: "khala" },
      toolset,
      triggers: [{ kind: "manual", triggerRef: "trigger.public.dispatcher_policy.manual" }],
      lane: "own_pylon",
      budget: { maxRunSeconds: 120, maxRunsPerDay: 3, maxCreditsPerDay: 0 },
      escalation: {
        channel: "operator",
        askPolicy: {
          mode: "operator_required",
          policyRef: "policy.public.agent_definition.operator_required.v1",
        },
      },
      sourceRefs: ["issue.public.github.OpenAgentsInc.openagents.8192"],
      createdAt: "2026-07-03T00:00:00.000Z",
      updatedAt: "2026-07-03T00:00:00.000Z",
    }),
  )
}

describe("KhalaToolDispatcher", () => {
  test("emits lifecycle events, telemetry tags, and hook callbacks around a successful tool", async () => {
    const hookOrder: string[] = []
    const observedEvents: KhalaToolEvent[] = []
    const dispatcher = makeKhalaToolDispatcher({
      hooks: {
        afterTool: context => Effect.sync(() => {
          hookOrder.push(`after:${context.result.status}:${context.phase}`)
        }),
        beforeTool: context => Effect.sync(() => {
          hookOrder.push(`before:${context.definition?.name}:${context.phase}`)
        }),
        onEvent: context => Effect.sync(() => {
          observedEvents.push(context.event)
        }),
      },
      telemetryTags: { lane: "test" },
      turnAccounting: createKhalaToolTurnAccounting({ maxToolCalls: 4, turnId: "turn_1" }),
    })

    const dispatched = await Effect.runPromise(
      dispatcher.dispatch({
        invocation: { arguments: { text: "hello" }, id: "call_1", name: "echo", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: input => Effect.succeed(khalaToolOk({ modelText: String(input.text) })),
          },
        ]),
        services: makeKhalaToolServices(),
        telemetryTags: { surface: "unit" },
      }),
    )

    expect(dispatched.result.status).toBe("ok")
    expect(dispatched.result.modelOutput.text).toBe("hello")
    expect(dispatched.accounting).toEqual({ maxToolCalls: 4, toolCallCount: 1, turnId: "turn_1" })
    expect(hookOrder).toEqual(["before:echo:validate", "after:ok:completed"])
    expect(dispatched.events.map(event => event.kind)).toEqual([
      "tool_started",
      "tool_progress",
      "tool_progress",
      "tool_completed",
    ])
    expect(observedEvents.map(event => event.kind)).toEqual(dispatched.events.map(event => event.kind))
    expect(dispatched.telemetryTags).toMatchObject({
      lane: "test",
      surface: "unit",
      toolAuthority: "read",
      toolCallId: "call_1",
      toolCallIndex: 1,
      toolName: "echo",
      turnId: "turn_1",
    })
  })

  test("uses injected runtime for event ids and duration accounting", async () => {
    const runtime = makeDeterministicKhalaToolRuntimeService({ nowMs: 1_000, seed: "dispatcher" })
    const afterDurations: number[] = []
    const dispatcher = makeKhalaToolDispatcher({
      hooks: {
        afterTool: context => Effect.sync(() => {
          afterDurations.push(context.durationMs)
        }),
      },
    })

    const dispatched = await Effect.runPromise(
      dispatcher.dispatch({
        invocation: { arguments: { text: "hello" }, id: "call_clock", name: "echo", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: (_input, context) =>
              Effect.gen(function* () {
                yield* context.services.runtime.sleep(125)
                return khalaToolOk({ modelText: "done" })
              }),
          },
        ]),
        services: makeKhalaToolServices({ runtime }),
      }),
    )

    expect(afterDurations).toEqual([125])
    expect(dispatched.events.map(event => event.eventId)).toEqual([
      "khala.tool.tool_started.rs.fmdonmx0",
      "khala.tool.tool_progress.rs.j6tor2dg",
      "khala.tool.tool_progress.rs.nip47e1g",
      "khala.tool.tool_completed.v9.rixc72xg",
    ])
  })

  test("returns typed model-visible errors for dispatcher failures", async () => {
    const dispatched = await Effect.runPromise(
      makeKhalaToolDispatcher().dispatch({
        invocation: { arguments: {}, id: "call_1", name: "missing", sessionId: "session_1" },
        registry: makeKhalaToolRegistry(),
        services: makeKhalaToolServices(),
      }),
    )

    expect(dispatched.result.status).toBe("failed")
    expect(dispatched.result.modelOutput.text).toContain("unknown_tool")
    expect(dispatched.result.ui).toMatchObject({
      code: "unknown_tool",
      kind: "khala_tool_error",
    })
    expect(dispatched.events.map(event => event.kind)).toContain("tool_failed")
  })

  test("enforces compiled agent-definition allow policy before tool execution", async () => {
    let executed = false
    const dispatched = await Effect.runPromise(
      makeKhalaToolDispatcher({
        agentDefinitionToolPolicy: compiledPolicy({
          allow: ["tool.openagents.khala.echo"],
          ask: [],
          deny: [],
          networkPolicy: "owner_scoped",
          secretPolicy: "owner_scoped_refs_only",
        }),
      }).dispatch({
        invocation: { arguments: { text: "hello" }, id: "call_policy_allow", name: "echo", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: input => Effect.sync(() => {
              executed = true
              return khalaToolOk({ modelText: String(input.text) })
            }),
          },
        ]),
        services: makeKhalaToolServices(),
      }),
    )

    expect(executed).toBe(true)
    expect(dispatched.result.status).toBe("ok")
    expect(dispatched.events.map(event => event.kind)).toContain("tool_completed")
  })

  test("denies tools outside the compiled agent-definition policy without executing them", async () => {
    let executed = false
    const dispatched = await Effect.runPromise(
      makeKhalaToolDispatcher({
        agentDefinitionToolPolicy: compiledPolicy({
          allow: ["tool.openagents.khala.exec_command"],
          ask: [],
          deny: ["tool.openagents.khala.authority.shell"],
          networkPolicy: "owner_scoped",
          secretPolicy: "owner_scoped_refs_only",
        }),
      }).dispatch({
        invocation: { arguments: { command: "pwd" }, id: "call_policy_deny", name: "exec_command", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: shellDefinition,
            execute: () => Effect.sync(() => {
              executed = true
              return khalaToolOk({ modelText: "should not run" })
            }),
          },
        ]),
        services: makeKhalaToolServices(),
      }),
    )

    expect(executed).toBe(false)
    expect(dispatched.result.status).toBe("denied")
    expect(dispatched.result.ui).toMatchObject({
      code: "agent_definition_tool_policy_denied",
      kind: "khala_tool_error",
    })
    expect(dispatched.events.at(-1)?.kind).toBe("tool_failed")
    expect(dispatched.events.at(-1)?.payload).toMatchObject({
      reasonRef: "reason.agent_definition.tool_denied",
      toolRefs: [
        "tool.openagents.khala.exec_command",
        "tool.openagents.khala.authority.shell",
      ],
    })
  })

  test("routes compiled agent-definition ask entries to operator escalation instead of failing", async () => {
    let executed = false
    const dispatched = await Effect.runPromise(
      makeKhalaToolDispatcher({
        agentDefinitionToolPolicy: compiledPolicy({
          allow: [],
          ask: ["tool.openagents.khala.echo"],
          deny: [],
          networkPolicy: "owner_scoped",
          secretPolicy: "owner_scoped_refs_only",
        }),
      }).dispatch({
        invocation: { arguments: { text: "needs review" }, id: "call_policy_ask", name: "echo", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: () => Effect.sync(() => {
              executed = true
              return khalaToolOk({ modelText: "should not run" })
            }),
          },
        ]),
        services: makeKhalaToolServices(),
      }),
    )

    expect(executed).toBe(false)
    expect(dispatched.result.status).toBe("needs_input")
    expect(dispatched.result.ui).toMatchObject({
      kind: "agent_definition_tool_policy_escalation",
      reasonRef: "reason.agent_definition.tool_requires_operator",
      escalation: {
        askPolicyRef: "policy.public.agent_definition.operator_required.v1",
        channel: "operator",
        toolRef: "tool.openagents.khala.echo",
      },
    })
    expect(dispatched.events.map(event => event.kind)).toContain("approval_requested")
    expect(dispatched.events.at(-1)?.payload).toMatchObject({
      reasonRef: "reason.agent_definition.tool_requires_operator",
      status: "needs_input",
    })
  })

  test("lets tools emit per-invocation progress through dispatcher hooks", async () => {
    const observedEvents: KhalaToolEvent[] = []
    const dispatcher = makeKhalaToolDispatcher({
      hooks: {
        onEvent: context => Effect.sync(() => {
          observedEvents.push(context.event)
        }),
      },
    })

    const dispatched = await Effect.runPromise(
      dispatcher.dispatch({
        invocation: { arguments: { text: "hello" }, id: "call_progress", name: "echo", sessionId: "session_1" },
        registry: makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: (_input, context) =>
              Effect.gen(function* () {
                yield* context.emitProgress({
                  kind: "unit_progress",
                  line: "halfway",
                })
                return khalaToolOk({ modelText: "done" })
              }),
          },
        ]),
        services: makeKhalaToolServices(),
      }),
    )

    const customProgress = observedEvents.find(event =>
      event.kind === "tool_progress" &&
      typeof event.payload === "object" &&
      event.payload !== null &&
      "kind" in event.payload &&
      event.payload.kind === "unit_progress"
    )
    expect(dispatched.result.status).toBe("ok")
    expect(customProgress).toBeDefined()
    expect(customProgress?.invocationId).toBe("call_progress")
    expect(customProgress?.sessionId).toBe("session_1")
  })

  test("enforces per-turn tool-call accounting limits", async () => {
    const dispatcher = makeKhalaToolDispatcher({
      turnAccounting: createKhalaToolTurnAccounting({ maxToolCalls: 1, turnId: "turn_1" }),
    })
    const registry = makeKhalaToolRegistry([
      {
        definition: echoDefinition,
        execute: () => Effect.succeed(khalaToolOk({ modelText: "ok" })),
      },
    ])
    const services = makeKhalaToolServices()
    const first = await Effect.runPromise(
      dispatcher.dispatch({
        invocation: { arguments: { text: "first" }, id: "call_1", name: "echo", sessionId: "session_1" },
        registry,
        services,
      }),
    )
    const second = await Effect.runPromise(
      dispatcher.dispatch({
        invocation: { arguments: { text: "second" }, id: "call_2", name: "echo", sessionId: "session_1" },
        registry,
        services,
      }),
    )

    expect(first.result.status).toBe("ok")
    expect(second.result.status).toBe("failed")
    expect(second.result.publicSummary).toContain("tool_call_limit_exceeded")
    expect(second.accounting).toEqual({ maxToolCalls: 1, toolCallCount: 2, turnId: "turn_1" })
  })

  test("spills oversized model output to a private artifact with a bounded preview", async () => {
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([
          {
            definition: echoDefinition,
            execute: () => Effect.succeed(khalaToolOk({ modelText: "abcdefghijklmnopqrstuvwxyz" })),
          },
        ]),
        { arguments: { text: "large" }, id: "call_1", name: "echo", sessionId: "session_1" },
        makeKhalaToolServices(),
        { maxModelOutputBytes: 8 },
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("abcdefgh")
    expect(result.modelOutput.text).toContain("[tool output truncated by dispatcher")
    expect(result.artifacts).toHaveLength(1)
    expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
  })
})
