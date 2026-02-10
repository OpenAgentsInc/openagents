import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();

const requireAuthed = (ctx: EffectQueryCtx | EffectMutationCtx) =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

export const putReportImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly signatureId: string;
    readonly evalHash: string;
    readonly compiled_id: string;
    readonly datasetId: string;
    readonly datasetHash: string;
    readonly rewardId: string;
    readonly rewardVersion: number;
    readonly split?: string | undefined;
    readonly selectedExampleIdsHash?: string | undefined;
    readonly n?: number | undefined;
    readonly json: unknown;
  },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseEvalReports")
        .withIndex("by_signatureId_evalHash", (q) => q.eq("signatureId", args.signatureId).eq("evalHash", args.evalHash))
        .unique(),
    );
    if (existing) return { ok: true, existed: true as const };

    yield* tryPromise(() =>
      ctx.db.insert("dseEvalReports", {
        signatureId: args.signatureId,
        evalHash: args.evalHash,
        compiled_id: args.compiled_id,
        datasetId: args.datasetId,
        datasetHash: args.datasetHash,
        rewardId: args.rewardId,
        rewardVersion: args.rewardVersion,
        split: args.split,
        selectedExampleIdsHash: args.selectedExampleIdsHash,
        n: args.n,
        json: args.json,
        createdAtMs: nowMs(),
      }),
    );

    return { ok: true, existed: false as const };
  });

export const putReport = effectMutation({
  args: {
    signatureId: v.string(),
    evalHash: v.string(),
    compiled_id: v.string(),
    datasetId: v.string(),
    datasetHash: v.string(),
    rewardId: v.string(),
    rewardVersion: v.number(),
    split: v.optional(v.string()),
    selectedExampleIdsHash: v.optional(v.string()),
    n: v.optional(v.number()),
    json: v.any(),
  },
  returns: v.object({ ok: v.boolean(), existed: v.boolean() }),
  handler: putReportImpl,
});

export const getReportImpl = (ctx: EffectQueryCtx, args: { readonly signatureId: string; readonly evalHash: string }) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseEvalReports")
        .withIndex("by_signatureId_evalHash", (q) => q.eq("signatureId", args.signatureId).eq("evalHash", args.evalHash))
        .unique(),
    );
    if (!row) return { ok: true, report: null };

    return {
      ok: true,
      report: {
        signatureId: String((row as any).signatureId ?? ""),
        evalHash: String((row as any).evalHash ?? ""),
        compiled_id: String((row as any).compiled_id ?? ""),
        datasetId: String((row as any).datasetId ?? ""),
        datasetHash: String((row as any).datasetHash ?? ""),
        rewardId: String((row as any).rewardId ?? ""),
        rewardVersion: Number((row as any).rewardVersion ?? 0),
        split: typeof (row as any).split === "string" ? String((row as any).split) : null,
        selectedExampleIdsHash:
          typeof (row as any).selectedExampleIdsHash === "string" ? String((row as any).selectedExampleIdsHash) : null,
        n: typeof (row as any).n === "number" ? Number((row as any).n) : null,
        json: (row as any).json ?? null,
        createdAtMs: Number((row as any).createdAtMs ?? 0),
      },
    };
  });

export const getReport = effectQuery({
  args: { signatureId: v.string(), evalHash: v.string() },
  returns: v.object({
    ok: v.boolean(),
    report: v.union(
      v.null(),
      v.object({
        signatureId: v.string(),
        evalHash: v.string(),
        compiled_id: v.string(),
        datasetId: v.string(),
        datasetHash: v.string(),
        rewardId: v.string(),
        rewardVersion: v.number(),
        split: v.union(v.null(), v.string()),
        selectedExampleIdsHash: v.union(v.null(), v.string()),
        n: v.union(v.null(), v.number()),
        json: v.any(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: getReportImpl,
});

export const listReportsImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly compiled_id?: string | undefined; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(0, Math.min(200, Math.floor(args.limit)))
        : 50;

    if (limit === 0) return { ok: true, reports: [] as any[] };

    const compiled_id = typeof args.compiled_id === "string" && args.compiled_id.length > 0 ? args.compiled_id : null;

    const rows: any[] = yield* tryPromise(() => {
      if (compiled_id) {
        return ctx.db
          .query("dseEvalReports")
          .withIndex("by_signatureId_compiled_id_createdAtMs", (q) =>
            q.eq("signatureId", args.signatureId).eq("compiled_id", compiled_id),
          )
          .order("desc")
          .take(limit);
      }
      return ctx.db
        .query("dseEvalReports")
        .withIndex("by_signatureId_createdAtMs", (q) => q.eq("signatureId", args.signatureId))
        .order("desc")
        .take(limit);
    });

    return {
      ok: true,
      reports: rows.map((r) => ({
        signatureId: String(r.signatureId ?? ""),
        evalHash: String(r.evalHash ?? ""),
        compiled_id: String(r.compiled_id ?? ""),
        datasetHash: String(r.datasetHash ?? ""),
        rewardId: String(r.rewardId ?? ""),
        split: typeof r.split === "string" ? String(r.split) : null,
        n: typeof r.n === "number" ? Number(r.n) : null,
        createdAtMs: Number(r.createdAtMs ?? 0),
      })),
    };
  });

export const listReports = effectQuery({
  args: {
    signatureId: v.string(),
    compiled_id: v.optional(v.string()),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    reports: v.array(
      v.object({
        signatureId: v.string(),
        evalHash: v.string(),
        compiled_id: v.string(),
        datasetHash: v.string(),
        rewardId: v.string(),
        split: v.union(v.null(), v.string()),
        n: v.union(v.null(), v.number()),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: listReportsImpl,
});

