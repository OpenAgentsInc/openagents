import { Context, Effect, Layer, Schema } from "effect";

import type { DseExecutionBudgetsV1 } from "../params.js";

export class BudgetExceededError extends Schema.TaggedError<BudgetExceededError>()(
  "BudgetExceededError",
  {
    message: Schema.String,
    budget: Schema.Literal(
      "maxTimeMs",
      "maxLmCalls",
      "maxToolCalls",
      "maxRlmIterations",
      "maxSubLmCalls",
      "maxOutputChars"
    ),
    limit: Schema.Number,
    observed: Schema.Number
  }
) {}

export type BudgetSnapshotV1 = {
  readonly limits: DseExecutionBudgetsV1;
  readonly usage: {
    readonly elapsedMs: number;
    readonly lmCalls: number;
    readonly toolCalls?: number | undefined;
    readonly rlmIterations?: number | undefined;
    readonly subLmCalls?: number | undefined;
    readonly outputChars: number;
  };
};

export type BudgetHandle = {
  readonly checkTime: () => Effect.Effect<void, BudgetExceededError>;
  readonly onLmCall: () => Effect.Effect<void, BudgetExceededError>;
  readonly onToolCall: () => Effect.Effect<void, BudgetExceededError>;
  readonly onRlmIteration: () => Effect.Effect<void, BudgetExceededError>;
  readonly onSubLmCall: () => Effect.Effect<void, BudgetExceededError>;
  readonly onOutputChars: (n: number) => Effect.Effect<void, BudgetExceededError>;
  readonly snapshot: () => Effect.Effect<BudgetSnapshotV1>;
};

export type ExecutionBudget = {
  readonly start: (options: {
    readonly runId: string;
    readonly startedAtMs?: number | undefined;
    readonly limits: DseExecutionBudgetsV1;
  }) => Effect.Effect<BudgetHandle>;
};

export class ExecutionBudgetService extends Context.Tag(
  "@openagentsinc/dse/ExecutionBudget"
)<ExecutionBudgetService, ExecutionBudget>() {}

function normalizeLimit(n: number | undefined): number | undefined {
  if (typeof n !== "number" || !Number.isFinite(n)) return undefined;
  return Math.max(0, Math.floor(n));
}

function makeHandle(options: {
  readonly runId: string;
  readonly startedAtMs?: number | undefined;
  readonly limits: DseExecutionBudgetsV1;
  readonly enforce: boolean;
}): BudgetHandle {
  const startedAtMs = options.startedAtMs ?? Date.now();

  const maxTimeMs = normalizeLimit(options.limits.maxTimeMs);
  const maxLmCalls = normalizeLimit(options.limits.maxLmCalls);
  const maxToolCalls = normalizeLimit(options.limits.maxToolCalls);
  const maxRlmIterations = normalizeLimit(options.limits.maxRlmIterations);
  const maxSubLmCalls = normalizeLimit(options.limits.maxSubLmCalls);
  const maxOutputChars = normalizeLimit(options.limits.maxOutputChars);

  const limits: DseExecutionBudgetsV1 = {
    ...(maxTimeMs !== undefined ? { maxTimeMs } : {}),
    ...(maxLmCalls !== undefined ? { maxLmCalls } : {}),
    ...(maxToolCalls !== undefined ? { maxToolCalls } : {}),
    ...(maxRlmIterations !== undefined ? { maxRlmIterations } : {}),
    ...(maxSubLmCalls !== undefined ? { maxSubLmCalls } : {}),
    ...(maxOutputChars !== undefined ? { maxOutputChars } : {})
  };

  let lmCalls = 0;
  let toolCalls = 0;
  let rlmIterations = 0;
  let subLmCalls = 0;
  let outputChars = 0;

  const elapsedMsNow = () => Math.max(0, Date.now() - startedAtMs);

  const checkTime = () =>
    Effect.gen(function* () {
      if (options.enforce && maxTimeMs !== undefined) {
        const elapsedMs = elapsedMsNow();
        if (elapsedMs > maxTimeMs) {
          return yield* Effect.fail(
            BudgetExceededError.make({
              message: `Budget exceeded: maxTimeMs limit=${maxTimeMs} observed=${elapsedMs}`,
              budget: "maxTimeMs",
              limit: maxTimeMs,
              observed: elapsedMs
            })
          );
        }
      }
    });

  const onLmCall = () =>
    Effect.gen(function* () {
      lmCalls++;
      yield* checkTime();
      if (options.enforce && maxLmCalls !== undefined && lmCalls > maxLmCalls) {
        return yield* Effect.fail(
          BudgetExceededError.make({
            message: `Budget exceeded: maxLmCalls limit=${maxLmCalls} observed=${lmCalls}`,
            budget: "maxLmCalls",
            limit: maxLmCalls,
            observed: lmCalls
          })
        );
      }
    });

  const onToolCall = () =>
    Effect.gen(function* () {
      toolCalls++;
      yield* checkTime();
      if (options.enforce && maxToolCalls !== undefined && toolCalls > maxToolCalls) {
        return yield* Effect.fail(
          BudgetExceededError.make({
            message: `Budget exceeded: maxToolCalls limit=${maxToolCalls} observed=${toolCalls}`,
            budget: "maxToolCalls",
            limit: maxToolCalls,
            observed: toolCalls
          })
        );
      }
    });

  const onRlmIteration = () =>
    Effect.gen(function* () {
      rlmIterations++;
      yield* checkTime();
      if (
        options.enforce &&
        maxRlmIterations !== undefined &&
        rlmIterations > maxRlmIterations
      ) {
        return yield* Effect.fail(
          BudgetExceededError.make({
            message: `Budget exceeded: maxRlmIterations limit=${maxRlmIterations} observed=${rlmIterations}`,
            budget: "maxRlmIterations",
            limit: maxRlmIterations,
            observed: rlmIterations
          })
        );
      }
    });

  const onSubLmCall = () =>
    Effect.gen(function* () {
      subLmCalls++;
      yield* checkTime();
      if (options.enforce && maxSubLmCalls !== undefined && subLmCalls > maxSubLmCalls) {
        return yield* Effect.fail(
          BudgetExceededError.make({
            message: `Budget exceeded: maxSubLmCalls limit=${maxSubLmCalls} observed=${subLmCalls}`,
            budget: "maxSubLmCalls",
            limit: maxSubLmCalls,
            observed: subLmCalls
          })
        );
      }
    });

  const onOutputChars = (n: number) =>
    Effect.gen(function* () {
      const delta = Math.max(0, Math.floor(n));
      outputChars += delta;
      yield* checkTime();
      if (
        options.enforce &&
        maxOutputChars !== undefined &&
        outputChars > maxOutputChars
      ) {
        return yield* Effect.fail(
          BudgetExceededError.make({
            message: `Budget exceeded: maxOutputChars limit=${maxOutputChars} observed=${outputChars}`,
            budget: "maxOutputChars",
            limit: maxOutputChars,
            observed: outputChars
          })
        );
      }
    });

  const snapshot = () =>
    Effect.sync(() => ({
      limits,
      usage: {
        elapsedMs: elapsedMsNow(),
        lmCalls,
        toolCalls,
        rlmIterations,
        subLmCalls,
        outputChars
      }
    }));

  return {
    checkTime,
    onLmCall,
    onToolCall,
    onRlmIteration,
    onSubLmCall,
    onOutputChars,
    snapshot
  };
}

export function layerInMemory(options?: {
  readonly enforce?: boolean | undefined;
}): Layer.Layer<ExecutionBudgetService> {
  const enforce = options?.enforce ?? true;
  return Layer.succeed(
    ExecutionBudgetService,
    ExecutionBudgetService.of({
      start: ({ runId, startedAtMs, limits }) =>
        Effect.sync(() => makeHandle({ runId, startedAtMs, limits, enforce }))
    })
  );
}

export function layerNoop(): Layer.Layer<ExecutionBudgetService> {
  // "noop" still tracks usage so receipts can include budget snapshots.
  return layerInMemory({ enforce: false });
}
