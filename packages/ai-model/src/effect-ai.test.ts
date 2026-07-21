import { describe, expect, test } from "vite-plus/test"
import { decodeKhalaRuntimeEvent } from "@openagentsinc/agent-runtime-schema"
import { Effect, Stream } from "effect"
import { AiError, LanguageModel, Response } from "effect/unstable/ai"
import {
  collectKhalaAiSdkCoreEventsFromEffectAiStream,
  effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart,
  khalaAiSdkTextStreamPartFromEffectAiStreamPart,
  khalaEffectAiLanguageModelLayer,
  khalaRuntimeEventFromEffectAiStreamPart,
  reduceKhalaRuntimeTranscript,
  runKhalaEffectAiCoreRuntime,
  type KhalaAiSdkCoreStreamText,
} from "./index.js"

const iso = "2026-07-21T00:00:00.000Z"

const usageEncoded = {
  inputTokens: {
    cacheRead: 2,
    cacheWrite: undefined,
    total: 10,
    uncached: undefined,
  },
  outputTokens: { reasoning: 1, text: undefined, total: 5 },
}

const scriptedEffectAiParts: ReadonlyArray<Response.StreamPartEncoded> = [
  { type: "response-metadata" },
  { id: "text_1", type: "text-start" },
  { delta: "Hello ", id: "text_1", type: "text-delta" },
  { delta: "world", id: "text_1", type: "text-delta" },
  { id: "text_1", type: "text-end" },
  { id: "reason_1", type: "reasoning-start" },
  { delta: "thinking", id: "reason_1", type: "reasoning-delta" },
  { id: "reason_1", type: "reasoning-end" },
  { id: "call_1", name: "echo", type: "tool-params-start" },
  { delta: "{\"text\":\"hi\"}", id: "call_1", type: "tool-params-delta" },
  { id: "call_1", type: "tool-params-end" },
  {
    id: "call_1",
    name: "echo",
    params: { text: "hi" },
    providerExecuted: false,
    type: "tool-call",
  },
  {
    id: "call_1",
    isFailure: false,
    name: "echo",
    result: { text: "hi" },
    type: "tool-result",
  },
  { reason: "stop", type: "finish", usage: usageEncoded },
]

function scriptedTransport(
  parts: ReadonlyArray<unknown>,
  seenOptions: Array<Record<string, unknown>> = [],
): KhalaAiSdkCoreStreamText {
  return options => {
    seenOptions.push(options)
    return {
      stream: (async function* () {
        for (const part of parts) yield part
      })(),
    }
  }
}

describe("khalaAiSdkTextStreamPartFromEffectAiStreamPart", () => {
  test("maps every Effect AI stream part type onto the ingestion vocabulary", () => {
    const cases: ReadonlyArray<
      readonly [Response.StreamPartEncoded, string]
    > = [
      [{ type: "response-metadata" }, "start"],
      [{ id: "t", type: "text-start" }, "text-start"],
      [{ delta: "x", id: "t", type: "text-delta" }, "text-delta"],
      [{ id: "t", type: "text-end" }, "text-end"],
      [{ id: "r", type: "reasoning-start" }, "reasoning-start"],
      [{ delta: "x", id: "r", type: "reasoning-delta" }, "reasoning-delta"],
      [{ id: "r", type: "reasoning-end" }, "reasoning-end"],
      [{ id: "c", name: "echo", type: "tool-params-start" }, "tool-input-start"],
      [{ delta: "{", id: "c", type: "tool-params-delta" }, "tool-input-delta"],
      [{ id: "c", type: "tool-params-end" }, "tool-input-end"],
      [
        { id: "c", name: "echo", params: {}, type: "tool-call" },
        "tool-call",
      ],
      [
        { id: "c", isFailure: false, name: "echo", result: "ok", type: "tool-result" },
        "tool-result",
      ],
      [
        { id: "c", isFailure: true, name: "echo", result: "bad", type: "tool-result" },
        "tool-error",
      ],
      [
        { approvalId: "a", toolCallId: "c", type: "tool-approval-request" },
        "tool-approval-request",
      ],
      [{ data: "aGk=", mediaType: "text/plain", type: "file" }, "file"],
      [
        {
          id: "s",
          sourceType: "url",
          title: "Example",
          type: "source",
          url: "https://example.com",
        },
        "source",
      ],
      [{ reason: "stop", type: "finish", usage: usageEncoded }, "finish"],
      [{ error: "boom", type: "error" }, "error"],
    ]
    for (const [part, expected] of cases) {
      expect(khalaAiSdkTextStreamPartFromEffectAiStreamPart(part).type).toBe(
        expected,
      )
    }
  })

  test("maps finish usage into the ingestion usage shape", () => {
    const mapped = khalaAiSdkTextStreamPartFromEffectAiStreamPart({
      reason: "stop",
      type: "finish",
      usage: usageEncoded,
    })
    expect(mapped).toEqual({
      finishReason: "stop",
      totalUsage: {
        inputTokenDetails: { cacheReadTokens: 2, cacheWriteTokens: undefined },
        inputTokens: 10,
        outputTokenDetails: { reasoningTokens: 1 },
        outputTokens: 5,
        totalTokens: 15,
      },
      type: "finish",
    })
  })
})

describe("effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart", () => {
  test("round-trips the shared vocabulary and drops step-only parts", () => {
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({
        id: "t",
        text: "Hi",
        type: "text-delta",
      }),
    ).toEqual({ delta: "Hi", id: "t", type: "text-delta" })
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({
        toolCallId: "c",
        toolName: "echo",
        type: "tool-error",
      }),
    ).toMatchObject({ isFailure: true, name: "echo", type: "tool-result" })
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({ type: "start-step" }),
    ).toBeUndefined()
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({
        finishReason: "stop",
        type: "finish-step",
      }),
    ).toBeUndefined()
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({
        rawValue: { x: 1 },
        type: "raw",
      }),
    ).toBeUndefined()
  })

  test("maps abort to a finish part with reason other", () => {
    expect(
      effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart({
        reason: "user",
        type: "abort",
      }),
    ).toMatchObject({ reason: "other", type: "finish" })
  })
})

describe("collectKhalaAiSdkCoreEventsFromEffectAiStream", () => {
  test("projects a scripted Effect AI stream into decoded KhalaRuntimeEvents", async () => {
    const result = await Effect.runPromise(
      collectKhalaAiSdkCoreEventsFromEffectAiStream({
        observedAt: () => iso,
        stream: Stream.fromIterable(scriptedEffectAiParts),
        threadId: "thread.effect_ai.fixture",
        turnId: "turn.effect_ai.fixture",
      }),
    )

    expect(result.events.map(event => event.kind)).toEqual([
      "turn.started",
      "raw.sidecar_ref",
      "text.delta",
      "text.delta",
      "text.completed",
      "raw.sidecar_ref",
      "reasoning.delta",
      "reasoning.completed",
      "tool.call",
      "tool.input.delta",
      "tool.input.completed",
      "tool.call",
      "tool.result",
      "turn.finished",
    ])

    for (const event of result.events) {
      expect(decodeKhalaRuntimeEvent(event)).toEqual(event)
    }

    const transcript = reduceKhalaRuntimeTranscript(result.events)
    expect(Object.values(transcript.textByMessageId)).toEqual(["Hello world"])
    expect(Object.values(transcript.reasoningByMessageId)).toEqual(["thinking"])
    expect(transcript.turnState).toBe("completed")

    const finished = result.events.at(-1)
    expect(finished?.kind).toBe("turn.finished")
    if (finished?.kind === "turn.finished") {
      expect(finished.finishReason).toBe("stop")
      expect(finished.usage).toMatchObject({
        inputTokens: 10,
        outputTokens: 5,
        totalTokens: 15,
      })
    }

    expect(
      result.events.every(event => event.sequence >= 1),
    ).toBe(true)
  })

  test("khalaRuntimeEventFromEffectAiStreamPart projects a single part", () => {
    const event = khalaRuntimeEventFromEffectAiStreamPart({
      eventId: "event.effect_ai.single",
      observedAt: iso,
      part: { delta: "Hi", id: "t1", type: "text-delta" },
      sequence: 1,
      threadId: "thread.effect_ai.single",
      turnId: "turn.effect_ai.single",
    })
    expect(event.kind).toBe("text.delta")
    expect(decodeKhalaRuntimeEvent(event)).toEqual(event)
  })
})

describe("khalaEffectAiLanguageModelLayer", () => {
  test("satisfies LanguageModel.streamText over the scripted Vercel-shaped transport", async () => {
    const seenOptions: Array<Record<string, unknown>> = []
    const transport = scriptedTransport(
      [
        { type: "start" },
        { id: "t1", type: "text-start" },
        { id: "t1", text: "Hi ", type: "text-delta" },
        { id: "t1", text: "there", type: "text-delta" },
        { id: "t1", type: "text-end" },
        {
          finishReason: "stop",
          totalUsage: { inputTokens: 3, outputTokens: 2, totalTokens: 5 },
          type: "finish",
        },
      ],
      seenOptions,
    )

    const parts = await Effect.runPromise(
      Stream.runCollect(
        LanguageModel.streamText({ prompt: "hello substrate" }),
      ).pipe(
        Effect.provide(
          khalaEffectAiLanguageModelLayer({
            model: "model.fixture",
            streamText: transport,
          }),
        ),
      ),
    )

    expect(parts.map(part => part.type)).toEqual([
      "response-metadata",
      "text-start",
      "text-delta",
      "text-delta",
      "text-end",
      "finish",
    ])
    const deltas = parts.filter(part => part.type === "text-delta")
    expect(deltas.map(part => part.delta).join("")).toBe("Hi there")
    const finish = parts.at(-1)
    if (finish?.type === "finish") {
      expect(finish.reason).toBe("stop")
      expect(finish.usage.inputTokens.total).toBe(3)
      expect(finish.usage.outputTokens.total).toBe(2)
    }

    expect(seenOptions).toHaveLength(1)
    expect(seenOptions[0]?.model).toBe("model.fixture")
    expect(seenOptions[0]?.messages).toEqual([
      { content: "hello substrate", role: "user" },
    ])
  })

  test("satisfies LanguageModel.generateText by coalescing streamed parts", async () => {
    const transport = scriptedTransport([
      { id: "t1", type: "text-start" },
      { id: "t1", text: "Hello ", type: "text-delta" },
      { id: "t1", text: "again", type: "text-delta" },
      { id: "t1", type: "text-end" },
      {
        finishReason: "stop",
        totalUsage: { inputTokens: 4, outputTokens: 3, totalTokens: 7 },
        type: "finish",
      },
    ])

    const response = await Effect.runPromise(
      LanguageModel.generateText({ prompt: "hello" }).pipe(
        Effect.provide(khalaEffectAiLanguageModelLayer({ streamText: transport })),
      ),
    )

    expect(response.text).toBe("Hello again")
    expect(response.usage.inputTokens.total).toBe(4)
    expect(response.finishReason).toBe("stop")
  })

  test("transport failures surface as AiError with an UnknownError reason", async () => {
    const failingTransport: KhalaAiSdkCoreStreamText = () => {
      throw new Error("transport exploded")
    }

    const error = await Effect.runPromise(
      Effect.flip(
        Stream.runCollect(LanguageModel.streamText({ prompt: "hello" })).pipe(
          Effect.provide(
            khalaEffectAiLanguageModelLayer({ streamText: failingTransport }),
          ),
        ),
      ),
    )

    expect(AiError.isAiError(error)).toBe(true)
    expect(error.reason._tag).toBe("UnknownError")
    if (error.reason._tag === "UnknownError") {
      expect(error.reason.description).toBe("transport exploded")
    }
  })
})

describe("runKhalaEffectAiCoreRuntime", () => {
  test("streams one turn through the provided LanguageModel Layer into KhalaRuntimeEvents", async () => {
    const transport = scriptedTransport([
      { type: "start" },
      { id: "t1", type: "text-start" },
      { id: "t1", text: "Substrate ", type: "text-delta" },
      { id: "t1", text: "online", type: "text-delta" },
      { id: "t1", type: "text-end" },
      {
        finishReason: "stop",
        totalUsage: { inputTokens: 6, outputTokens: 2, totalTokens: 8 },
        type: "finish",
      },
    ])

    const result = await Effect.runPromise(
      runKhalaEffectAiCoreRuntime({
        observedAt: () => iso,
        prompt: "bring the substrate online",
        threadId: "thread.effect_ai.run",
        turnId: "turn.effect_ai.run",
      }).pipe(
        Effect.provide(khalaEffectAiLanguageModelLayer({ streamText: transport })),
      ),
    )

    expect(result.events.map(event => event.kind)).toEqual([
      "turn.started",
      "raw.sidecar_ref",
      "text.delta",
      "text.delta",
      "text.completed",
      "turn.finished",
    ])
    for (const event of result.events) {
      expect(decodeKhalaRuntimeEvent(event)).toEqual(event)
      expect(event.source.lane).toBe("ai_sdk_core")
    }
    const transcript = reduceKhalaRuntimeTranscript(result.events)
    expect(Object.values(transcript.textByMessageId)).toEqual([
      "Substrate online",
    ])
    expect(transcript.turnState).toBe("completed")
  })
})
