import { createHash } from "node:crypto"

import {
  assertAgentRuntimePublicEventSafe,
  type AgentRuntimeEvent,
  type AgentRuntimeEventId,
  type AgentRuntimeRunId,
} from "@openagentsinc/agent-runtime-schema"
import { Context, Effect, Layer, Schema as S, Stream } from "effect"

import {
  AgentRuntimeAdapterError,
  type AgentRuntimeAdapter,
  type AgentRuntimeRunRequest,
} from "./agent-runtime-adapter.js"

export const OPENAGENTS_NATIVE_TASK_SCHEMA = "openagents.pylon.openagents_native_task.v0.3"
export const OPENAGENTS_NATIVE_SUMMARY_TOOL_REF = "tool.public.openagents_native.fixture_summary.v1"

export const OpenAgentsNativeTask = S.Struct({
  schema: S.Literal(OPENAGENTS_NATIVE_TASK_SCHEMA),
  fixtureRef: S.optional(S.String),
  allowedToolRefs: S.Array(S.String),
  maxModelEvents: S.optional(S.Number),
})
export type OpenAgentsNativeTask = typeof OpenAgentsNativeTask.Type

const SummaryToolInput = S.Struct({
  artifactRef: S.String,
  summaryRef: S.String,
})
type SummaryToolInput = typeof SummaryToolInput.Type

const SummaryToolOutput = S.Struct({
  artifactRef: S.String,
  summaryRef: S.String,
  resultRef: S.String,
})
type SummaryToolOutput = typeof SummaryToolOutput.Type

export type OpenAgentsNativeModelEvent =
  | { readonly kind: "reasoning_delta"; readonly text: string }
  | { readonly kind: "text_delta"; readonly text: string }
  | {
      readonly kind: "tool_call"
      readonly invocationId: string
      readonly toolName: "fixture_summary"
      readonly toolRef: typeof OPENAGENTS_NATIVE_SUMMARY_TOOL_REF
      readonly input: SummaryToolInput
    }

export class OpenAgentsNativeLanguageModel extends Context.Service<
  OpenAgentsNativeLanguageModel,
  {
    readonly stream: (request: AgentRuntimeRunRequest, task: OpenAgentsNativeTask) => Stream.Stream<OpenAgentsNativeModelEvent>
  }
>()("OpenAgentsNativeLanguageModel") {}

export class OpenAgentsNativeToolkit extends Context.Service<
  OpenAgentsNativeToolkit,
  {
    readonly executeSummary: (input: SummaryToolInput) => Effect.Effect<SummaryToolOutput>
  }
>()("OpenAgentsNativeToolkit") {}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function eventId(runId: AgentRuntimeRunId, sequence: number): AgentRuntimeEventId {
  return `event.public.${stableRef("openagents_native", `${runId}:${sequence}`).slice("openagents_native.".length)}`
}

function event(
  request: AgentRuntimeRunRequest,
  sequence: number,
  tag: AgentRuntimeEvent["tag"],
  input: Partial<AgentRuntimeEvent> = {},
): AgentRuntimeEvent {
  return assertAgentRuntimePublicEventSafe({
    tag,
    eventId: eventId(request.runId, sequence),
    runId: request.runId,
    sequence,
    generatedAt: request.now.toISOString(),
    visibility: "public",
    redactionClass: "public_ref",
    refs: [],
    blockerRefs: [],
    ...input,
  })
}

export function openAgentsNativeTaskFrom(codingAssignment: unknown): OpenAgentsNativeTask | null {
  const native = (codingAssignment as { openagentsNative?: unknown } | null)?.openagentsNative
  if (native === null || typeof native !== "object") {
    return null
  }
  try {
    return S.decodeUnknownSync(OpenAgentsNativeTask)(native)
  } catch {
    return null
  }
}

export const OpenAgentsNativeTestLanguageModelLayer = Layer.succeed(OpenAgentsNativeLanguageModel, {
  stream: (_request, _task) =>
    Stream.fromIterable([
      { kind: "reasoning_delta", text: "select typed fixture summary tool" },
      { kind: "text_delta", text: "Native fixture started." },
      {
        kind: "tool_call",
        invocationId: "tool.public.openagents_native.fixture_summary.1",
        toolName: "fixture_summary",
        toolRef: OPENAGENTS_NATIVE_SUMMARY_TOOL_REF,
        input: {
          artifactRef: "artifact.public.openagents_native.fixture",
          summaryRef: "summary.public.openagents_native.fixture",
        },
      },
      { kind: "text_delta", text: "Native fixture completed." },
    ] satisfies ReadonlyArray<OpenAgentsNativeModelEvent>),
})

export const OpenAgentsNativeBudgetStopLanguageModelLayer = Layer.succeed(OpenAgentsNativeLanguageModel, {
  stream: (_request, _task) =>
    Stream.fromIterable([
      { kind: "reasoning_delta", text: "budget fixture" },
      { kind: "text_delta", text: "first event" },
      { kind: "text_delta", text: "second event" },
    ] satisfies ReadonlyArray<OpenAgentsNativeModelEvent>),
})

export const OpenAgentsNativeTestToolkitLayer = Layer.succeed(OpenAgentsNativeToolkit, {
  executeSummary: (input) =>
    Effect.succeed({
      artifactRef: input.artifactRef,
      summaryRef: input.summaryRef,
      resultRef: stableRef("result.public.openagents_native.fixture_summary", `${input.artifactRef}:${input.summaryRef}`),
    }),
})

export function createOpenAgentsNativeAgentRuntimeAdapter(input: {
  layer?: Layer.Layer<OpenAgentsNativeLanguageModel | OpenAgentsNativeToolkit>
} = {}): AgentRuntimeAdapter {
  const cancelledRunIds = new Set<AgentRuntimeRunId>()
  const layer =
    input.layer ??
    Layer.merge(OpenAgentsNativeTestLanguageModelLayer, OpenAgentsNativeTestToolkitLayer)

  return {
    kind: "openagents_native",
    canRun: (request) => Effect.succeed(openAgentsNativeTaskFrom(request.lease.codingAssignment) !== null),
    start: (request) =>
      Stream.unwrap(
        Effect.promise(async () => {
          if (cancelledRunIds.has(request.runId)) {
            return Stream.fromIterable([
              event(request, 1, "run.started"),
              event(request, 2, "run.cancelled", {
                blockerRefs: ["blocker.agent_runtime.openagents_native.cancelled"],
              }),
            ])
          }

          const task = openAgentsNativeTaskFrom(request.lease.codingAssignment)
          if (task === null) {
            return Stream.fromIterable([
              event(request, 1, "run.started"),
              event(request, 2, "run.failed", {
                blockerRefs: ["blocker.agent_runtime.openagents_native.unsupported_assignment"],
              }),
            ])
          }

          const events = await Effect.runPromise(
            Effect.gen(function* () {
              const model = yield* OpenAgentsNativeLanguageModel
              const toolkit = yield* OpenAgentsNativeToolkit
              const output: Array<AgentRuntimeEvent> = [event(request, 1, "run.started")]
              let sequence = 2
              let consumedModelEvents = 0
              const budget = task.maxModelEvents ?? 16

              output.push(event(request, sequence, "model.stream_started", {
                stepRef: stableRef("step.public.openagents_native", request.runId),
              }))
              sequence += 1

              const modelEvents = yield* Stream.runCollect(model.stream(request, task))
              for (const modelEvent of modelEvents) {
                consumedModelEvents += 1
                if (consumedModelEvents > budget) {
                  output.push(event(request, sequence, "run.interrupted", {
                    blockerRefs: ["blocker.agent_runtime.openagents_native.budget_stop"],
                  }))
                  sequence += 1
                  output.push(event(request, sequence, "run.failed", {
                    blockerRefs: ["blocker.agent_runtime.openagents_native.budget_stop"],
                  }))
                  return output
                }

                if (modelEvent.kind === "reasoning_delta") {
                  output.push(event(request, sequence, "model.reasoning_delta", {
                    part: { kind: "reasoning", summary: modelEvent.text },
                  }))
                  sequence += 1
                }

                if (modelEvent.kind === "text_delta") {
                  output.push(event(request, sequence, "model.text_delta", {
                    part: { kind: "text", text: modelEvent.text },
                  }))
                  sequence += 1
                }

                if (modelEvent.kind === "tool_call") {
                  output.push(event(request, sequence, "tool.call_proposed", {
                    toolInvocation: {
                      invocationId: modelEvent.invocationId,
                      toolName: modelEvent.toolName,
                      toolRef: modelEvent.toolRef,
                      inputRef: stableRef("tool_input.public.openagents_native", JSON.stringify(modelEvent.input)),
                      status: "proposed",
                      blockerRefs: [],
                    },
                  }))
                  sequence += 1

                  if (!task.allowedToolRefs.includes(modelEvent.toolRef)) {
                    output.push(event(request, sequence, "tool.denied", {
                      blockerRefs: ["blocker.agent_runtime.openagents_native.tool_denied"],
                      toolInvocation: {
                        invocationId: modelEvent.invocationId,
                        toolName: modelEvent.toolName,
                        toolRef: modelEvent.toolRef,
                        status: "denied",
                        blockerRefs: ["blocker.agent_runtime.openagents_native.tool_denied"],
                      },
                    }))
                    sequence += 1
                    output.push(event(request, sequence, "run.failed", {
                      blockerRefs: ["blocker.agent_runtime.openagents_native.tool_denied"],
                    }))
                    return output
                  }

                  output.push(event(request, sequence, "tool.approved", {
                    toolInvocation: {
                      invocationId: modelEvent.invocationId,
                      toolName: modelEvent.toolName,
                      toolRef: modelEvent.toolRef,
                      status: "approved",
                      blockerRefs: [],
                    },
                  }))
                  sequence += 1
                  output.push(event(request, sequence, "tool.started", {
                    toolInvocation: {
                      invocationId: modelEvent.invocationId,
                      toolName: modelEvent.toolName,
                      toolRef: modelEvent.toolRef,
                      status: "started",
                      blockerRefs: [],
                    },
                  }))
                  sequence += 1

                  const decodedInput = S.decodeUnknownSync(SummaryToolInput)(modelEvent.input)
                  const result = yield* toolkit.executeSummary(decodedInput)
                  S.decodeUnknownSync(SummaryToolOutput)(result)

                  output.push(event(request, sequence, "tool.completed", {
                    artifact: {
                      artifactRef: result.artifactRef,
                      artifactKind: "native_tool_result",
                      visibility: "public",
                      digestRef: result.resultRef,
                      summary: result.summaryRef,
                    },
                    refs: [result.resultRef],
                    toolInvocation: {
                      invocationId: modelEvent.invocationId,
                      toolName: modelEvent.toolName,
                      toolRef: modelEvent.toolRef,
                      outputRef: result.resultRef,
                      status: "completed",
                      blockerRefs: [],
                    },
                  }))
                  sequence += 1
                }
              }

              output.push(event(request, sequence, "model.text_completed"))
              sequence += 1
              output.push(event(request, sequence, "model.reasoning_completed"))
              sequence += 1
              output.push(event(request, sequence, "run.completed"))
              return output
            }).pipe(Effect.provide(layer)),
          )

          return Stream.fromIterable(events)
        }),
      ),
    cancel: (runId) =>
      Effect.sync(() => {
        cancelledRunIds.add(runId)
      }),
  }
}
