// Fast path (#6193, Rhys req #4 "run quickly"): bounded-pool scenario/variant
// sharding.
//
// A multi-scenario run is embarrassingly parallel — each scenario provisions its
// own isolated backend + artifact dir, so they don't share mutable state. Serial
// execution wastes wall-clock; this runs them through a bounded worker pool so N
// scenarios with M-way concurrency finish in ~ceil(N/M) waves instead of N.
//
// Discipline:
//   - DETERMINISTIC ORDER: results come back in INPUT order regardless of which
//     worker finished first, so the matrix/report is stable across runs.
//   - BOUNDED: at most `concurrency` workers in flight (a cap, not "spawn all").
//   - NO SLEEPS: the pool is driven by promise completion (a worker pulls the
//     next item the instant it finishes), never a polling sleep.
//   - HONEST FAILURES: a worker that throws is captured as that item's error;
//     other shards keep running (partial-failure continuation at the run level),
//     and the error is surfaced in the result — never swallowed into a fake pass.

/** One shard's outcome: either a value or the error its worker threw. */
export type ShardResult<A> =
  | { readonly ok: true; readonly value: A }
  | { readonly ok: false; readonly error: unknown };

export interface ShardOptions {
  /** Max workers in flight. Clamped to [1, items.length]. Default 4. */
  readonly concurrency?: number;
}

/**
 * Run `worker` over `items` through a bounded pool, returning results in INPUT
 * order. A worker that throws is captured (does not reject the whole batch), so
 * one red shard never hides the others. Concurrency is a hard cap.
 *
 * The pool is completion-driven: `concurrency` runners each loop pulling the
 * next unclaimed index until the queue drains. No item is processed twice and
 * none is skipped (a shared cursor hands out indices atomically in the JS single
 * thread). This is the "run quickly" primitive the multi-scenario driver uses.
 */
export async function runShards<I, A>(
  items: ReadonlyArray<I>,
  worker: (item: I, index: number) => Promise<A>,
  options: ShardOptions = {},
): Promise<ReadonlyArray<ShardResult<A>>> {
  const n = items.length;
  const results: Array<ShardResult<A>> = new Array(n);
  if (n === 0) return results;

  const concurrency = Math.min(Math.max(1, options.concurrency ?? 4), n);
  let cursor = 0;

  const runOne = async (): Promise<void> => {
    // Single-threaded JS: reading + incrementing the cursor is atomic, so each
    // index is claimed by exactly one runner. Loop until the queue is drained.
    while (cursor < n) {
      const index = cursor++;
      const item = items[index]!;
      try {
        results[index] = { ok: true, value: await worker(item, index) };
      } catch (error) {
        results[index] = { ok: false, error };
      }
    }
  };

  await Promise.all(Array.from({ length: concurrency }, () => runOne()));
  return results;
}

// ── Fast path over runQaSession: run N independent QA sessions in parallel ────
//
// Each session is fully independent (its own target/brain/backend/artifactDir),
// so the bounded pool runs them concurrently with no shared mutable state. This
// reuses `runQaSession` UNCHANGED — the public run contract stays stable; this
// is just a concurrent driver over it. Honest: a session that throws is captured
// as that shard's error (other shards keep running), and the per-session
// result.json/artifacts are flushed by the runner's own crash/interrupt
// finalizer regardless of how the shard around it behaves.

import { Effect } from "effect";
import type { RunInput, RunOutcome } from "./runner";
import { runQaSession } from "./runner";

export interface ShardedRunInput {
  /** The independent sessions to run (each a complete `runQaSession` input). */
  readonly sessions: ReadonlyArray<RunInput>;
  /** Max sessions in flight. Default 4. */
  readonly concurrency?: number;
}

/**
 * Run every session through the bounded pool and return each outcome in INPUT
 * order. A multi-scenario run finishes in ~ceil(N/concurrency) waves instead of
 * N serial runs — the "run quickly" fast path (req #4). Failures are captured
 * per-shard, never collapsed into a single batch reject.
 */
export function runScenariosSharded(
  input: ShardedRunInput,
): Effect.Effect<ReadonlyArray<ShardResult<RunOutcome>>, never> {
  return Effect.promise(() =>
    runShards(
      input.sessions,
      (session) => Effect.runPromise(runQaSession(session)),
      input.concurrency !== undefined ? { concurrency: input.concurrency } : {},
    ),
  );
}

/** Partition a shard-result list into successes (in order) and failures. */
export function partitionShardResults<A>(
  results: ReadonlyArray<ShardResult<A>>,
): { readonly values: ReadonlyArray<A>; readonly errors: ReadonlyArray<unknown> } {
  const values: A[] = [];
  const errors: unknown[] = [];
  for (const r of results) {
    if (r.ok) values.push(r.value);
    else errors.push(r.error);
  }
  return { values, errors };
}
