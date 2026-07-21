import { describe, expect, test } from "vite-plus/test";
import { modelFailureClassForAiErrorReasonTag } from "@openagentsinc/agent-runtime-schema";
import { Duration, Effect, ExecutionPlan, Fiber, Schedule } from "effect";
import { TestClock } from "effect/testing";
import { AiError } from "effect/unstable/ai";
import {
  makeKhalaModelFallbackPlan,
  khalaEffectAiLanguageModelLayer,
  khalaModelFallbackDefaultFallthroughWhile,
  khalaModelFallbackDefaultRetryWhile,
  reduceKhalaRuntimeTranscript,
  runKhalaEffectAiCoreRuntimeWithFallback,
  type KhalaAiSdkCoreStreamText,
  type KhalaModelFallbackPlan,
} from "./index.js";

const iso = "2026-07-21T00:00:00.000Z";

function scriptedParts(text: string): ReadonlyArray<unknown> {
  return [
    { type: "start" },
    { id: "t1", type: "text-start" },
    { id: "t1", text, type: "text-delta" },
    { id: "t1", type: "text-end" },
    {
      finishReason: "stop",
      totalUsage: { inputTokens: 3, outputTokens: 2, totalTokens: 5 },
      type: "finish",
    },
  ];
}

function aiError(reason: AiError.AiErrorReason): AiError.AiError {
  return AiError.make({
    method: "streamText",
    module: "FallbackFixture",
    reason,
  });
}

/**
 * A transport that fails with the scripted typed `AiError` reasons in order,
 * then serves the scripted text. `calls.count` records every invocation.
 */
function flakyTransport(input: {
  readonly failures: ReadonlyArray<AiError.AiErrorReason>;
  readonly text: string;
  readonly calls: { count: number };
}): KhalaAiSdkCoreStreamText {
  return () => {
    const call = input.calls.count;
    input.calls.count += 1;
    const reason = input.failures[call];
    if (reason !== undefined) throw aiError(reason);
    return {
      stream: (async function* () {
        for (const part of scriptedParts(input.text)) yield part;
      })(),
    };
  };
}

function runInput(id: string) {
  return {
    observedAt: () => iso,
    prompt: "run the fallback lane",
    threadId: `thread.fallback.${id}`,
    turnId: `turn.fallback.${id}`,
  };
}

function runUnderTestClock<A, E>(
  program: Effect.Effect<A, E>,
  adjust: Duration.Duration,
): Promise<A> {
  return Effect.runPromise(
    Effect.gen(function* () {
      const fiber = yield* Effect.forkChild(program);
      yield* TestClock.adjust(adjust);
      return yield* Fiber.join(fiber);
    }).pipe(Effect.provide(TestClock.layer())),
  );
}

describe("makeKhalaModelFallbackPlan", () => {
  test("constructs an ExecutionPlan with per-step defaults", () => {
    const layer = khalaEffectAiLanguageModelLayer({
      streamText: flakyTransport({
        calls: { count: 0 },
        failures: [],
        text: "unused",
      }),
    });
    const plan = makeKhalaModelFallbackPlan([{ layer }, { layer }]);
    expect(ExecutionPlan.isExecutionPlan(plan)).toBe(true);
    expect(plan.steps).toHaveLength(2);
    // First step: bounded in-step retry with backoff for transient reasons.
    expect(plan.steps[0].attempts).toBe(3);
    expect(plan.steps[0].schedule).toBeDefined();
    // ExecutionPlan.make wraps predicates, so assert presence, not identity.
    expect(plan.steps[0].while).toBeDefined();
    // Fallback step: single attempt, gated on provider-side reasons only.
    expect(plan.steps[1]?.attempts).toBeUndefined();
    expect(plan.steps[1]?.schedule).toBeUndefined();
    expect(plan.steps[1]?.while).toBeDefined();
  });

  test("default gates classify reasons honestly", () => {
    const rateLimited = aiError(new AiError.RateLimitError({}));
    const quota = aiError(new AiError.QuotaExhaustedError({}));
    const badRequest = aiError(
      new AiError.InvalidRequestError({ description: "bad request shape" }),
    );
    expect(khalaModelFallbackDefaultRetryWhile(rateLimited)).toBe(true);
    expect(khalaModelFallbackDefaultRetryWhile(quota)).toBe(false);
    expect(khalaModelFallbackDefaultFallthroughWhile(quota)).toBe(true);
    expect(khalaModelFallbackDefaultFallthroughWhile(badRequest)).toBe(false);
  });
});

describe("runKhalaEffectAiCoreRuntimeWithFallback", () => {
  test("retries transient RateLimitError in-step on the schedule, no fallback", async () => {
    const primaryCalls = { count: 0 };
    const secondaryCalls = { count: 0 };
    const plan = makeKhalaModelFallbackPlan([
      {
        attempts: 3,
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: primaryCalls,
            failures: [new AiError.RateLimitError({}), new AiError.RateLimitError({})],
            text: "primary served",
          }),
        }),
        schedule: Schedule.spaced(Duration.seconds(1)),
      },
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: secondaryCalls,
            failures: [],
            text: "secondary served",
          }),
        }),
      },
    ]);

    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          runKhalaEffectAiCoreRuntimeWithFallback(runInput("retry"), plan),
        );
        // First attempt runs without any clock advance.
        yield* TestClock.adjust(Duration.zero);
        expect(primaryCalls.count).toBe(1);
        // The spaced(1s) schedule holds the retry until the full second.
        yield* TestClock.adjust(Duration.millis(999));
        expect(primaryCalls.count).toBe(1);
        yield* TestClock.adjust(Duration.millis(1));
        expect(primaryCalls.count).toBe(2);
        yield* TestClock.adjust(Duration.seconds(1));
        expect(primaryCalls.count).toBe(3);
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(outcome.servedByStepIndex).toBe(0);
    expect(outcome.servedOnAttempt).toBe(3);
    expect(outcome.priorFailures).toEqual([
      { attempt: 1, reasonTag: "RateLimitError", stepIndex: 0 },
      { attempt: 2, reasonTag: "RateLimitError", stepIndex: 0 },
    ]);
    expect(secondaryCalls.count).toBe(0);
    const transcript = reduceKhalaRuntimeTranscript(outcome.value.events);
    expect(Object.values(transcript.textByMessageId)).toEqual(["primary served"]);
    expect(transcript.turnState).toBe("completed");
  });

  test("hard provider error falls through to the secondary Layer, which serves", async () => {
    const primaryCalls = { count: 0 };
    const secondaryCalls = { count: 0 };
    const plan = makeKhalaModelFallbackPlan([
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: primaryCalls,
            failures: [
              new AiError.InternalProviderError({
                description: "provider melted",
              }),
            ],
            text: "primary served",
          }),
        }),
      },
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: secondaryCalls,
            failures: [],
            text: "secondary served",
          }),
        }),
      },
    ]);

    const outcome = await runUnderTestClock(
      runKhalaEffectAiCoreRuntimeWithFallback(runInput("fallthrough"), plan),
      Duration.seconds(10),
    );

    // InternalProviderError is not transient, so the default first-step gate
    // does not retry it in-step even though the step allows 3 attempts.
    expect(primaryCalls.count).toBe(1);
    expect(secondaryCalls.count).toBe(1);
    expect(outcome.servedByStepIndex).toBe(1);
    expect(outcome.servedOnAttempt).toBe(2);
    expect(outcome.priorFailures).toEqual([
      { attempt: 1, reasonTag: "InternalProviderError", stepIndex: 0 },
    ]);
    const transcript = reduceKhalaRuntimeTranscript(outcome.value.events);
    expect(Object.values(transcript.textByMessageId)).toEqual(["secondary served"]);
  });

  test("quota exhaustion on every step surfaces the true QuotaExhaustedError, not laundered", async () => {
    const primaryCalls = { count: 0 };
    const secondaryCalls = { count: 0 };
    const plan = makeKhalaModelFallbackPlan([
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: primaryCalls,
            failures: [new AiError.QuotaExhaustedError({})],
            text: "unused",
          }),
        }),
      },
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: secondaryCalls,
            failures: [new AiError.QuotaExhaustedError({})],
            text: "unused",
          }),
        }),
      },
    ]);

    const error = await runUnderTestClock(
      Effect.flip(runKhalaEffectAiCoreRuntimeWithFallback(runInput("quota"), plan)),
      Duration.seconds(10),
    );

    // Quota is never retried in-step (not transient); it may fall through
    // once to the secondary account, which also fails.
    expect(primaryCalls.count).toBe(1);
    expect(secondaryCalls.count).toBe(1);
    // The surfaced error is the true typed AiError from the last step. The
    // PUBLIC `modelFailureClassForAiErrorReasonTag` from
    // `@openagentsinc/agent-runtime-schema` (AISDK-05 #9151) maps this exact
    // tag to `account_exhausted` (total map over `reason._tag`), so the
    // caller sees the honest failure class — the fallback masked nothing.
    expect(AiError.isAiError(error)).toBe(true);
    expect(error.reason._tag).toBe("QuotaExhaustedError");
    expect(modelFailureClassForAiErrorReasonTag(error.reason._tag)).toBe(
      "account_exhausted",
    );
  });

  test("request-shaped errors fail fast without touching the fallback Layer", async () => {
    const primaryCalls = { count: 0 };
    const secondaryCalls = { count: 0 };
    const plan = makeKhalaModelFallbackPlan([
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: primaryCalls,
            failures: [
              new AiError.InvalidRequestError({
                description: "bad request shape",
              }),
            ],
            text: "unused",
          }),
        }),
      },
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: secondaryCalls,
            failures: [],
            text: "would mask the caller bug",
          }),
        }),
      },
    ]);

    const error = await runUnderTestClock(
      Effect.flip(runKhalaEffectAiCoreRuntimeWithFallback(runInput("request"), plan)),
      Duration.seconds(10),
    );

    expect(primaryCalls.count).toBe(1);
    expect(secondaryCalls.count).toBe(0);
    expect(AiError.isAiError(error)).toBe(true);
    expect(error.reason._tag).toBe("InvalidRequestError");
  });

  test("a custom fallback-step while gate can refuse entry entirely", async () => {
    const primaryCalls = { count: 0 };
    const secondaryCalls = { count: 0 };
    const plan: KhalaModelFallbackPlan = makeKhalaModelFallbackPlan([
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: primaryCalls,
            failures: [
              new AiError.InternalProviderError({
                description: "provider melted",
              }),
            ],
            text: "unused",
          }),
        }),
      },
      {
        layer: khalaEffectAiLanguageModelLayer({
          streamText: flakyTransport({
            calls: secondaryCalls,
            failures: [],
            text: "never served",
          }),
        }),
        while: () => false,
      },
    ]);

    const error = await runUnderTestClock(
      Effect.flip(runKhalaEffectAiCoreRuntimeWithFallback(runInput("gate"), plan)),
      Duration.seconds(10),
    );

    expect(primaryCalls.count).toBe(1);
    expect(secondaryCalls.count).toBe(0);
    expect(AiError.isAiError(error)).toBe(true);
    expect(error.reason._tag).toBe("InternalProviderError");
  });
});
