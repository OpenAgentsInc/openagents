import { describe, expect, test } from "bun:test"
import {
  compileAgentDefinitionToolRuntimePolicy,
  decodeAgentDefinition,
  decodeAgentRuntimeEvent,
  type AgentDefinitionToolset,
} from "@openagentsinc/agent-runtime-schema"
import {
  khalaToolOk,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaToolDefinition,
} from "@openagentsinc/khala-tools"
import { Effect } from "effect"
import {
  buildKhalaAiSdkCoreStreamTextOptions,
  collectKhalaAiSdkCoreEventsFromStream,
  khalaToolsToAiSdkTools,
  reduceAgentRuntimeEventsAsKhalaTranscript,
  reduceKhalaRuntimeTranscript,
  runKhalaAiSdkCoreRuntime,
} from "./index.js"

const iso = "2026-07-05T00:00:00.000Z"

async function* streamOf(parts: ReadonlyArray<unknown>): AsyncIterable<unknown> {
  for (const part of parts) yield part
}

const provider = {
  headers: { "x-openagents-provider": "fixture" },
  modelRef: "model.fixture.low_risk",
  providerOptions: { compatibility: "strict_false" },
  providerRef: "openai",
  promptCache: { cacheControl: "ephemeral", cacheKey: "prompt-cache.fixture" },
  reasoning: { effort: "low", enabled: true, maxTokens: 128 },
  schemaLowering: "json_schema",
  strictToolSchemas: false,
} as const

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
  internalId: "khala.ai_sdk_core.test.echo",
  label: "Echo",
  name: "echo",
  permissionMode: "allow",
  prompt: "Echo text.",
  promptGuidelines: ["Use for AI SDK Core adapter tests only."],
}

function compiledPolicy(toolset: AgentDefinitionToolset) {
  return compileAgentDefinitionToolRuntimePolicy(
    decodeAgentDefinition({
      schema: "openagents.agent_definition.v1",
      budget: { maxCreditsPerDay: 0, maxRunSeconds: 120, maxRunsPerDay: 3 },
      createdAt: iso,
      escalation: {
        askPolicy: {
          mode: "operator_required",
          policyRef: "policy.public.agent_definition.operator_required.v1",
        },
        channel: "operator",
      },
      goal: "Exercise the AI SDK Core tool bridge.",
      harness: { kind: "khala" },
      id: "agent_definition.khala_ai_sdk_core.test",
      lane: "own_pylon",
      name: "AI SDK Core Test",
      ownerRef: "agent:khala_ai_sdk_core_test_owner",
      slug: "ai-sdk-core-test",
      sourceRefs: ["issue.public.github.OpenAgentsInc.openagents.8373"],
      toolset,
      triggers: [{ kind: "manual", triggerRef: "trigger.public.ai_sdk_core.test" }],
      updatedAt: iso,
    }),
  )
}

describe("Khala AI SDK Core runtime adapter", () => {
  test("calls streamText and maps streamed text, finish, usage, and provider metadata into Khala runtime events", async () => {
    let capturedOptions: Record<string, unknown> | undefined
    const result = await runKhalaAiSdkCoreRuntime({
      model: "fixture-model",
      observedAt: () => iso,
      prompt: "Say hello.",
      provider,
      streamText: options => {
        capturedOptions = options
        return {
          stream: streamOf([
            { type: "start" },
            { type: "text-delta", id: "message.fixture", text: "Hello " },
            { type: "text-delta", id: "message.fixture", text: "world" },
            {
              type: "finish-step",
              finishReason: "stop",
              providerMetadata: { trace: "private-provider-payload" },
              usage: {
                inputTokens: 3,
                outputTokens: 2,
                totalTokens: 5,
              },
            },
            {
              type: "finish",
              finishReason: "stop",
              totalUsage: {
                inputTokens: 3,
                outputTokens: 2,
                totalTokens: 5,
              },
            },
          ]),
        }
      },
      threadId: "thread.ai_sdk_core.fixture",
      turnId: "turn.ai_sdk_core.fixture",
    })

    expect(capturedOptions?.model).toBe("fixture-model")
    expect(capturedOptions?.prompt).toBe("Say hello.")
    expect(capturedOptions?.headers).toEqual({
      "x-openagents-provider": "fixture",
    })
    expect(capturedOptions?.providerOptions).toMatchObject({
      openai: {
        compatibility: "strict_false",
        promptCache: {
          cacheControl: "ephemeral",
          cacheKey: "prompt-cache.fixture",
        },
        reasoning: { effort: "low", enabled: true, maxTokens: 128 },
        schemaLowering: "json_schema",
        strictToolSchemas: false,
      },
    })

    expect(result.events.map(event => event.kind)).toEqual([
      "turn.started",
      "text.delta",
      "text.delta",
      "step.finished",
      "turn.finished",
    ])
    expect(result.events.every(event => event.source.lane === "ai_sdk_core")).toBe(true)
    expect(result.events.every(event => event.visibility === "private")).toBe(true)
    const projection = reduceKhalaRuntimeTranscript(result.events)
    expect(projection.textByMessageId["message.fixture"]).toBe("Hello world")
    const usageEvent = result.events.find(event => event.kind === "turn.finished")
    expect(usageEvent?.kind).toBe("turn.finished")
    if (usageEvent?.kind === "turn.finished") {
      expect(usageEvent.usage?.totalTokens).toBe(5)
    }
    const providerEvent = result.events.find(event => event.kind === "step.finished")
    expect(providerEvent?.kind).toBe("step.finished")
    if (providerEvent?.kind === "step.finished") {
      expect(providerEvent.providerMetadata?.metadataRefs[0]).toMatch(/^metadata\./)
    }
  })

  test("provider transform merges caller options without requiring an AI SDK Core fork", () => {
    const options = buildKhalaAiSdkCoreStreamTextOptions({
      headers: { "x-request": "one" },
      model: "fixture-model",
      prompt: "hello",
      provider,
      providerOptions: {
        openai: { existing: true },
        telemetry: { enabled: false },
      },
      tools: { echo: { description: "provided by test" } },
    })

    expect(options).toMatchObject({
      headers: {
        "x-openagents-provider": "fixture",
        "x-request": "one",
      },
      providerOptions: {
        openai: {
          compatibility: "strict_false",
          existing: true,
          promptCache: provider.promptCache,
          reasoning: provider.reasoning,
          schemaLowering: "json_schema",
          strictToolSchemas: false,
        },
        telemetry: { enabled: false },
      },
      tools: { echo: { description: "provided by test" } },
    })
  })

  test("bridged AI SDK tools re-enter Khala policy before side effects", async () => {
    let executed = false
    const registry = makeKhalaToolRegistry([
      {
        definition: echoDefinition,
        execute: input => {
          executed = true
          return Effect.succeed(khalaToolOk({ modelText: String(input.text) }))
        },
      },
    ])

    const deniedTools = khalaToolsToAiSdkTools({
      dispatcherOptions: {
        agentDefinitionToolPolicy: compiledPolicy({
          allow: [],
          ask: [],
          deny: ["tool.openagents.khala.echo"],
          networkPolicy: "owner_scoped",
          secretPolicy: "owner_scoped_refs_only",
        }),
      },
      registry,
      services: makeKhalaToolServices(),
    }) as Record<string, { execute: (args: unknown, options: unknown) => Promise<Record<string, unknown>> }>

    const denied = await deniedTools.echo!.execute(
      { text: "blocked" },
      { toolCallId: "tool_call.denied" },
    )
    expect(executed).toBe(false)
    expect(denied.status).toBe("denied")

    const allowedTools = khalaToolsToAiSdkTools({
      dispatcherOptions: {
        agentDefinitionToolPolicy: compiledPolicy({
          allow: ["tool.openagents.khala.echo"],
          ask: [],
          deny: [],
          networkPolicy: "owner_scoped",
          secretPolicy: "owner_scoped_refs_only",
        }),
      },
      registry,
      services: makeKhalaToolServices(),
    }) as Record<string, { execute: (args: unknown, options: unknown) => Promise<Record<string, unknown>> }>

    const allowed = await allowedTools.echo!.execute(
      { text: "allowed" },
      { toolCallId: "tool_call.allowed" },
    )
    expect(executed).toBe(true)
    expect(allowed).toMatchObject({
      status: "ok",
      text: "allowed",
    })
  })

  test("raw provider chunks become private refs and never public transcript state", async () => {
    const secretRaw = {
      providerPayload: {
        path: "/Users/alice/.config/private-token",
        token: "sk-secret",
      },
    }
    const seenPrivateParts: unknown[] = []
    const result = await collectKhalaAiSdkCoreEventsFromStream({
      onPrivateRawPart: ({ part }) => {
        seenPrivateParts.push(part)
      },
      observedAt: () => iso,
      stream: streamOf([
        { rawValue: secretRaw, type: "raw" },
      ]),
      threadId: "thread.ai_sdk_core.raw",
      turnId: "turn.ai_sdk_core.raw",
    })

    expect(seenPrivateParts).toEqual([{ rawValue: secretRaw, type: "raw" }])
    expect(result.rawSidecars).toHaveLength(1)
    expect(result.events).toHaveLength(1)
    expect(result.events[0]!.kind).toBe("raw.sidecar_ref")
    expect(result.events[0]!.visibility).toBe("private")
    expect(JSON.stringify(result.events)).not.toContain("/Users/alice")
    expect(JSON.stringify(result.events)).not.toContain("sk-secret")
    expect(JSON.stringify(result.rawSidecars)).not.toContain("sk-secret")
  })

  test("AI SDK Core and existing AgentRuntimeEvent paths share the Khala runtime transcript consumer", async () => {
    const ai = await collectKhalaAiSdkCoreEventsFromStream({
      observedAt: () => iso,
      stream: streamOf([
        { type: "text-delta", id: "message.shared", text: "AI SDK text" },
      ]),
      threadId: "thread.shared",
      turnId: "turn.shared",
    })

    const pylonProjection = reduceAgentRuntimeEventsAsKhalaTranscript({
      events: [
        decodeAgentRuntimeEvent({
          tag: "model.text_delta",
          blockerRefs: [],
          eventId: "event.public.agent_runtime.shared",
          generatedAt: iso,
          part: { kind: "text", text: "Pylon text" },
          redactionClass: "public_ref",
          refs: [],
          runId: "run.shared",
          sequence: 1,
          stepRef: "message.shared",
          visibility: "public",
        }),
      ],
      source: { lane: "codex_app_server", surface: "server" },
      threadId: "thread.shared",
      turnId: "turn.shared",
    })
    const aiProjection = reduceKhalaRuntimeTranscript(ai.events)

    expect(aiProjection.textByMessageId["message.shared"]).toBe("AI SDK text")
    expect(pylonProjection.textByMessageId["message.shared"]).toBe("Pylon text")
  })
})
