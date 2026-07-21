/**
 * STREAM-05 (#9133): Effect `ExecutionPlan` for in-lane provider fallback.
 *
 * This module is the bounded, ADVISORY integration of the evaluation spike:
 * a thin typed wrapper that constructs an `ExecutionPlan` whose steps provide
 * `LanguageModel` Layers, so ONE lane's model call can retry transient
 * failures in-step and fall through to an alternate provider Layer.
 *
 * Hard boundary (issue #9133): Full Auto keeps all durable authority —
 * leases, the eight-active-run cap, journals, receipts, account custody, and
 * health-ordered rotation. This plan never chooses the next lane, never holds
 * run state, and never moves run-selection or settlement authority. It only
 * backs the retry/fallback mechanics inside one lane's model call.
 *
 * Honesty invariant: a fallback must never mask an `account_exhausted`
 * truth. The verified Effect 4.0.0-beta.94 runtime
 * (`effect/dist/internal/executionPlan.js`) surfaces the LAST failure
 * unchanged when the plan is exhausted, and this module never wraps or
 * rewrites the error. The surfaced value stays the true typed `AiError`, so
 * the caller classifies it with the PUBLIC
 * `modelFailureClassForAiErrorReasonTag` from
 * `@openagentsinc/agent-runtime-schema` (a total map over `reason._tag`,
 * made public under AISDK-05 #9151), which reports `account_exhausted`,
 * `account_rate_limited`, or `auth_required` exactly as the last provider
 * failure said. That mapping is a public workspace dependency this package
 * already carries; the private `@openagentsinc/harness-conformance` package
 * now wraps the same public function for typed `AiError` objects, so no
 * private symbol is ever needed here (see `effect-ai.ts`).
 *
 * Verified v4 runtime semantics this module relies on:
 * - Steps run in order; the terminal error is the last step failure, unwrapped.
 * - A step's `while` predicate gates BOTH in-step retries of that step's own
 *   failures AND (for steps after the first) whether the previous step's
 *   error may enter the step at all. A fallback step with no
 *   `attempts`/`schedule` runs at most once (`Schedule.recurs(1)` internally),
 *   so its `while` acts purely as the fall-through gate.
 * - `ExecutionPlan.CurrentMetadata` is a `Context.Reference` giving the
 *   1-based cumulative attempt and the 0-based step index of the running
 *   attempt.
 */
import { Duration, Effect, ExecutionPlan, Schedule } from "effect";
import type { Layer } from "effect";
import type { AiError, LanguageModel } from "effect/unstable/ai";
import {
  runKhalaEffectAiCoreRuntime,
  type KhalaAiSdkCoreRunResult,
  type KhalaEffectAiRunInput,
} from "./index.js";

/** One fallback step: a `LanguageModel` Layer plus optional retry policy. */
export type KhalaModelFallbackStep = Readonly<{
  /** The `LanguageModel` Layer this step provides to the lane's model call. */
  layer: Layer.Layer<LanguageModel.LanguageModel>;
  /** Total in-step attempts (initial call included). Default: 3 for the
   * first step, 1 for fallback steps. */
  attempts?: number;
  /** Delay schedule between in-step retries. Default: exponential backoff
   * from 200 ms when the step allows more than one attempt. */
  schedule?: Schedule.Schedule<unknown, AiError.AiError>;
  /** Retry/fall-through gate. Defaults: the first step retries only
   * transient reasons (`RateLimitError`); fallback steps accept only
   * provider-side reasons (`khalaModelFallbackFallthroughReasonTags`).
   * On fallback steps this single predicate gates both the incoming error
   * and any in-step retries (verified runtime semantics). */
  while?: (error: AiError.AiError) => boolean;
}>;

/** The exact plan config this wrapper produces. */
export type KhalaModelFallbackPlan = ExecutionPlan.ExecutionPlan<{
  provides: LanguageModel.LanguageModel;
  input: AiError.AiError;
  error: never;
  requirements: never;
}>;

/**
 * Reasons the first step retries in-step by default. Only genuinely
 * transient capacity signals belong here: retrying quota, auth, or
 * request-shaped failures against the same provider cannot succeed and
 * would delay the honest failure class.
 */
export const khalaModelFallbackTransientReasonTags: ReadonlySet<AiError.AiErrorReason["_tag"]> =
  new Set(["RateLimitError"]);

/**
 * Reasons that may fall through to the next provider Layer by default.
 * Provider-side faults (capacity, credentials, transport, provider
 * internals) may be served by a different provider account. Request-shaped
 * faults (invalid request/input, tool and schema configuration, content
 * policy) would fail identically everywhere, so they fail fast with their
 * true reason instead of burning fallback spend.
 */
export const khalaModelFallbackFallthroughReasonTags: ReadonlySet<AiError.AiErrorReason["_tag"]> =
  new Set([
    "AuthenticationError",
    "InternalProviderError",
    "NetworkError",
    "QuotaExhaustedError",
    "RateLimitError",
    "UnknownError",
  ]);

/** Default first-step retry gate: transient reasons only. */
export function khalaModelFallbackDefaultRetryWhile(error: AiError.AiError): boolean {
  return khalaModelFallbackTransientReasonTags.has(error.reason._tag);
}

/** Default fallback-step gate: provider-side reasons only. */
export function khalaModelFallbackDefaultFallthroughWhile(error: AiError.AiError): boolean {
  return khalaModelFallbackFallthroughReasonTags.has(error.reason._tag);
}

const defaultRetrySchedule: Schedule.Schedule<unknown, AiError.AiError> = Schedule.exponential(
  Duration.millis(200),
  2,
);

const defaultFirstStepAttempts = 3;

type PlanStepInput = {
  readonly provide: Layer.Layer<LanguageModel.LanguageModel>;
  readonly attempts?: number | undefined;
  readonly schedule?: Schedule.Schedule<unknown, AiError.AiError> | undefined;
  readonly while: (error: AiError.AiError) => boolean;
};

/**
 * Construct an in-lane provider fallback plan from ordered
 * `LanguageModel` Layer steps.
 *
 * Defaults: the first step retries only transient reasons
 * (`RateLimitError`) with exponential backoff for up to
 * `defaultFirstStepAttempts` total attempts; each fallback step runs at
 * most once and accepts only provider-side reasons, so request-shaped
 * failures fail fast. `QuotaExhaustedError` and `AuthenticationError` are
 * never retried in-step by default and are never swallowed: if every step
 * fails, the true last `AiError` surfaces unchanged.
 */
export function makeKhalaModelFallbackPlan(
  steps: readonly [KhalaModelFallbackStep, ...Array<KhalaModelFallbackStep>],
): KhalaModelFallbackPlan {
  const planSteps = steps.map((step, index): PlanStepInput => {
    const attempts = step.attempts ?? (index === 0 ? defaultFirstStepAttempts : undefined);
    const schedule =
      step.schedule ?? (attempts !== undefined && attempts > 1 ? defaultRetrySchedule : undefined);
    return {
      provide: step.layer,
      while:
        step.while ??
        (index === 0
          ? khalaModelFallbackDefaultRetryWhile
          : khalaModelFallbackDefaultFallthroughWhile),
      ...(attempts === undefined ? {} : { attempts }),
      ...(schedule === undefined ? {} : { schedule }),
    };
  });
  return ExecutionPlan.make(...(planSteps as [PlanStepInput, ...Array<PlanStepInput>]));
}

/** One recorded model-call failure observed while the plan was running. */
export type KhalaModelFallbackAttemptFailure = Readonly<{
  /** 1-based cumulative attempt number across the whole plan. */
  attempt: number;
  /** 0-based index of the plan step that failed. */
  stepIndex: number;
  /** The true typed `AiError` reason tag of that failure. */
  reasonTag: AiError.AiErrorReason["_tag"];
}>;

/** Successful outcome plus the honest record of how it was served. */
export type KhalaModelFallbackOutcome<A> = Readonly<{
  value: A;
  /** 0-based index of the step whose Layer served the answer. */
  servedByStepIndex: number;
  /** 1-based cumulative attempt number that succeeded. */
  servedOnAttempt: number;
  /** Every failed attempt before the success, in order. A non-empty list
   * with a served answer means fallback or retry occurred — surface it,
   * never hide it. */
  priorFailures: ReadonlyArray<KhalaModelFallbackAttemptFailure>;
}>;

/**
 * Run one lane's model-call program under a fallback plan.
 *
 * On success the outcome records which step served the answer, the
 * cumulative attempt count, and every prior failure, so a served answer
 * never hides that a provider was exhausted on the way. On total failure
 * the effect fails with the program's own typed `AiError` — the true last
 * failure, unwrapped — ready for the caller to classify with the public
 * `modelFailureClassForAiErrorReasonTag` from
 * `@openagentsinc/agent-runtime-schema`.
 */
export function runWithModelFallback<A, E extends AiError.AiError, R>(
  program: Effect.Effect<A, E, R>,
  plan: KhalaModelFallbackPlan,
): Effect.Effect<KhalaModelFallbackOutcome<A>, E, Exclude<R, LanguageModel.LanguageModel>> {
  return Effect.suspend(() => {
    const priorFailures: Array<KhalaModelFallbackAttemptFailure> = [];
    const annotated = program.pipe(
      Effect.tapError((error) =>
        Effect.map(Effect.service(ExecutionPlan.CurrentMetadata), (meta) => {
          priorFailures.push({
            attempt: meta.attempt,
            reasonTag: error.reason._tag,
            stepIndex: meta.stepIndex,
          });
        }),
      ),
      Effect.flatMap((value) =>
        Effect.map(
          Effect.service(ExecutionPlan.CurrentMetadata),
          (meta): KhalaModelFallbackOutcome<A> => ({
            priorFailures,
            servedByStepIndex: meta.stepIndex,
            servedOnAttempt: meta.attempt,
            value,
          }),
        ),
      ),
    );
    return Effect.withExecutionPlan(annotated, plan);
  });
}

/**
 * The in-lane composition: stream one model turn through the fallback plan
 * and collect it into `KhalaRuntimeEvent`s. This is
 * `runKhalaEffectAiCoreRuntime` under `runWithModelFallback` — each attempt
 * re-runs the whole turn against the step's Layer, so a served answer is
 * always one provider's complete turn, never a splice of partial streams.
 */
export function runKhalaEffectAiCoreRuntimeWithFallback(
  input: KhalaEffectAiRunInput,
  plan: KhalaModelFallbackPlan,
): Effect.Effect<KhalaModelFallbackOutcome<KhalaAiSdkCoreRunResult>, AiError.AiError> {
  return runWithModelFallback(runKhalaEffectAiCoreRuntime(input), plan);
}
