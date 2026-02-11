import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectQueryCtx } from "../effect/ctx";
import { effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "./access";

const clampLimit = (value: number | undefined, fallback: number, max: number): number => {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.max(0, Math.min(max, Math.floor(value)));
};

const sortedUniqueRunIds = (runs: ReadonlyArray<{ readonly runId: string }>): ReadonlyArray<string> =>
  Array.from(new Set(runs.map((run) => run.runId).filter((runId) => runId.length > 0))).sort((a, b) =>
    a.localeCompare(b),
  );

const tryOrFallback = <A, E, R>(effect: Effect.Effect<A, E, R>, fallback: A): Effect.Effect<A, never, R> =>
  effect.pipe(Effect.catchAll(() => Effect.succeed(fallback)));

export const getThreadTraceBundleImpl = (
  ctx: EffectQueryCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly maxMessages?: number | undefined;
    readonly maxParts?: number | undefined;
    readonly maxRuns?: number | undefined;
    readonly maxReceipts?: number | undefined;
    readonly maxFeatureRequests?: number | undefined;
    readonly includeDseState?: boolean | undefined;
    readonly maxDseRowsPerRun?: number | undefined;
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const maxMessages = clampLimit(args.maxMessages, 400, 2_000);
    const maxParts = clampLimit(args.maxParts, 8_000, 40_000);
    const maxRuns = clampLimit(args.maxRuns, 200, 2_000);
    const maxReceipts = clampLimit(args.maxReceipts, 2_000, 20_000);
    const maxFeatureRequests = clampLimit(args.maxFeatureRequests, 500, 5_000);
    const includeDseState = args.includeDseState === true;
    const maxDseRowsPerRun = clampLimit(args.maxDseRowsPerRun, 200, 2_000);

    const messages = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("messages")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .order("asc")
          .take(maxMessages),
      ),
      [],
    );

    const parts = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("messageParts")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .order("asc")
          .take(maxParts),
      ),
      [],
    );

    const runs = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("runs")
          .withIndex("by_threadId_updatedAtMs", (q) => q.eq("threadId", args.threadId))
          .order("asc")
          .take(maxRuns),
      ),
      [],
    );

    const receipts = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("receipts")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .order("asc")
          .take(maxReceipts),
      ),
      [],
    );

    const featureRequests = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("autopilotFeatureRequests")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .order("asc")
          .take(maxFeatureRequests),
      ),
      [],
    );

    const blueprint = yield* tryOrFallback(
      tryPromise(() =>
        ctx.db
          .query("blueprints")
          .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
          .unique(),
      ),
      null,
    );

    const runIds = sortedUniqueRunIds(
      runs
        .map((run) => (typeof run.runId === "string" ? { runId: run.runId.trim() } : null))
        .filter((row): row is { readonly runId: string } => row !== null),
    );

    const dseBlobs =
      includeDseState && runIds.length > 0
        ? yield* tryOrFallback(
            Effect.forEach(
              runIds,
              (runId) =>
                tryPromise(() =>
                  ctx.db
                    .query("dseBlobs")
                    .withIndex("by_threadId_runId_blobId", (q) => q.eq("threadId", args.threadId).eq("runId", runId))
                    .order("asc")
                    .take(maxDseRowsPerRun),
                ),
              { discard: false },
            ).pipe(Effect.map((groups) => groups.flatMap((group) => group))),
            [],
          )
        : [];

    const dseVars =
      includeDseState && runIds.length > 0
        ? yield* tryOrFallback(
            Effect.forEach(
              runIds,
              (runId) =>
                tryPromise(() =>
                  ctx.db
                    .query("dseVarSpace")
                    .withIndex("by_threadId_runId_updatedAtMs", (q) =>
                      q.eq("threadId", args.threadId).eq("runId", runId),
                    )
                    .order("asc")
                    .take(maxDseRowsPerRun),
                ),
              { discard: false },
            ).pipe(Effect.map((groups) => groups.flatMap((group) => group))),
            [],
          )
        : [];

    if (messages.length === 0 && parts.length === 0 && runs.length === 0 && receipts.length === 0) {
      yield* Effect.logWarning(
        `[autopilot.traces] empty trace bundle for thread=${args.threadId} (may indicate missing index/table access in this deployment)`,
      );
    }

    return {
      ok: true as const,
      thread: {
        threadId: thread.threadId,
        ownerId: thread.ownerId ?? null,
        createdAtMs: thread.createdAtMs,
        updatedAtMs: thread.updatedAtMs,
      },
      blueprint: blueprint?.blueprint ?? null,
      messages,
      parts,
      runs,
      receipts,
      featureRequests,
      dseBlobs,
      dseVars,
      summary: {
        messageCount: messages.length,
        partCount: parts.length,
        runCount: runs.length,
        receiptCount: receipts.length,
        featureRequestCount: featureRequests.length,
        dseBlobCount: dseBlobs.length,
        dseVarCount: dseVars.length,
      },
    };
  });

export const getThreadTraceBundle = effectQuery({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    maxMessages: v.optional(v.number()),
    maxParts: v.optional(v.number()),
    maxRuns: v.optional(v.number()),
    maxReceipts: v.optional(v.number()),
    maxFeatureRequests: v.optional(v.number()),
    includeDseState: v.optional(v.boolean()),
    maxDseRowsPerRun: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    thread: v.object({
      threadId: v.string(),
      ownerId: v.union(v.null(), v.string()),
      createdAtMs: v.number(),
      updatedAtMs: v.number(),
    }),
    blueprint: v.any(),
    messages: v.array(v.any()),
    parts: v.array(v.any()),
    runs: v.array(v.any()),
    receipts: v.array(v.any()),
    featureRequests: v.array(v.any()),
    dseBlobs: v.array(v.any()),
    dseVars: v.array(v.any()),
    summary: v.object({
      messageCount: v.number(),
      partCount: v.number(),
      runCount: v.number(),
      receiptCount: v.number(),
      featureRequestCount: v.number(),
      dseBlobCount: v.number(),
      dseVarCount: v.number(),
    }),
  }),
  handler: getThreadTraceBundleImpl,
});
