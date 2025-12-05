/**
 * Step Result Memoization
 *
 * Persists non-deterministic orchestrator step results to allow replay
 * after a crash. When the orchestrator restarts, previously recorded
 * results can be replayed instead of re-executing the step.
 */
import { promises as fs } from "node:fs";
import * as path from "node:path";
import { Effect, Option } from "effect";

export const STEP_RESULTS_FILENAME = "step-results.json";

/**
 * Result of a single orchestrator step.
 */
export interface StepResult {
  readonly stepId: string;
  readonly sessionId: string;
  readonly timestamp: string;
  readonly result: unknown;
  readonly inputHash?: string;
}

/**
 * Step result store persisted on disk.
 */
export interface StepResultStore {
  readonly sessionId: string;
  readonly steps: ReadonlyArray<StepResult>;
}

/**
 * Manager for loading, replaying, and persisting step results.
 */
export interface StepResultsManager {
  /** Session ID associated with the current store (existing or new) */
  readonly sessionId: string;
  /** Whether we are replaying from a previous run */
  readonly replayMode: boolean;
  /** Try to get a cached result for the given step */
  getResult: <A>(stepId: string, inputHash?: string) => Option.Option<A>;
  /** Persist a new step result */
  recordResult: (stepId: string, result: unknown, inputHash?: string) => Effect.Effect<void, Error>;
  /** Clear persisted step results */
  clear: () => Effect.Effect<void, never>;
}

const getStorePath = (openagentsDir: string): string =>
  path.join(openagentsDir, STEP_RESULTS_FILENAME);

const readStore = (
  openagentsDir: string
): Effect.Effect<Option.Option<StepResultStore>, Error> =>
  Effect.tryPromise({
    try: async () => {
      const storePath = getStorePath(openagentsDir);
      try {
        const content = await fs.readFile(storePath, "utf8");
        const parsed = JSON.parse(content) as StepResultStore;
        if (parsed?.sessionId && Array.isArray(parsed.steps)) {
          return Option.some(parsed);
        }
        return Option.none<StepResultStore>();
      } catch (error: any) {
        if (error?.code === "ENOENT") {
          return Option.none<StepResultStore>();
        }
        throw new Error(
          `Failed to read step results: ${
            error instanceof Error ? error.message : String(error)
          }`
        );
      }
    },
    catch: (error) =>
      error instanceof Error ? error : new Error(String(error)),
  });

const writeStore = (
  openagentsDir: string,
  store: StepResultStore
): Effect.Effect<void, Error> =>
  Effect.tryPromise({
    try: async () => {
      const storePath = getStorePath(openagentsDir);
      await fs.mkdir(path.dirname(storePath), { recursive: true });
      await fs.writeFile(storePath, JSON.stringify(store, null, 2), "utf8");
    },
    catch: (error) =>
      new Error(
        `Failed to write step results: ${
          error instanceof Error ? error.message : String(error)
        }`
      ),
  });

/**
 * Create a StepResultsManager for the current orchestrator run.
 * If a previous store exists, replayMode will be true and the existing
 * sessionId will be reused to continue the previous run.
 */
export const createStepResultsManager = (
  openagentsDir: string,
  requestedSessionId: string
): Effect.Effect<StepResultsManager, Error> =>
  Effect.gen(function* () {
    const existing = yield* readStore(openagentsDir);
    const sessionId = Option.isSome(existing)
      ? existing.value.sessionId
      : requestedSessionId;
    const steps = new Map(
      Option.isSome(existing)
        ? existing.value.steps.map((step) => [step.stepId, step] as const)
        : []
    );

    const replayMode = Option.isSome(existing) && steps.size > 0;
    const storePath = getStorePath(openagentsDir);

    const manager: StepResultsManager = {
      sessionId,
      replayMode,
      getResult: <A>(stepId: string, inputHash?: string) => {
        const step = steps.get(stepId);
        if (!step) return Option.none<A>();
        if (step.inputHash && inputHash && step.inputHash !== inputHash) {
          return Option.none<A>();
        }
        return Option.some(step.result as A);
      },
      recordResult: (stepId: string, result: unknown, inputHash?: string) =>
        Effect.gen(function* () {
          const entry: StepResult = {
            stepId,
            sessionId,
            timestamp: new Date().toISOString(),
            result,
            ...(inputHash ? { inputHash } : {}),
          };
          steps.set(stepId, entry);
          const store: StepResultStore = {
            sessionId,
            steps: Array.from(steps.values()),
          };
          yield* writeStore(openagentsDir, store);
        }),
      clear: () =>
        Effect.tryPromise({
          try: () => fs.unlink(storePath),
          catch: () => undefined,
        }).pipe(Effect.ignore),
    };

    return manager;
  });

/**
 * Wrap a non-deterministic operation so that it replays cached results
 * when available and otherwise records the fresh result.
 */
export const durableStep = <A, E, R>(
  manager: StepResultsManager,
  stepId: string,
  operation: () => Effect.Effect<A, E, R>,
  options: { inputHash?: string } = {}
): Effect.Effect<A, E | Error, R> =>
  Effect.gen(function* () {
    const cached = manager.getResult<A>(stepId, options.inputHash);
    if (Option.isSome(cached)) {
      return cached.value;
    }

    const result = yield* operation();
    yield* manager.recordResult(stepId, result, options.inputHash);
    return result;
  });
