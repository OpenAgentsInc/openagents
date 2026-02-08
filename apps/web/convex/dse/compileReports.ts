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
    readonly jobHash: string;
    readonly datasetId: string;
    readonly datasetHash: string;
    readonly compiled_id: string;
    readonly json: unknown;
  },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseCompileReports")
        .withIndex("by_signatureId_jobHash_datasetHash", (q) =>
          q.eq("signatureId", args.signatureId).eq("jobHash", args.jobHash).eq("datasetHash", args.datasetHash),
        )
        .unique(),
    );

    if (existing) return { ok: true, existed: true as const };

    yield* tryPromise(() =>
      ctx.db.insert("dseCompileReports", {
        signatureId: args.signatureId,
        jobHash: args.jobHash,
        datasetId: args.datasetId,
        datasetHash: args.datasetHash,
        compiled_id: args.compiled_id,
        json: args.json,
        createdAtMs: nowMs(),
      }),
    );

    return { ok: true, existed: false as const };
  });

export const putReport = effectMutation({
  args: {
    signatureId: v.string(),
    jobHash: v.string(),
    datasetId: v.string(),
    datasetHash: v.string(),
    compiled_id: v.string(),
    json: v.any(),
  },
  returns: v.object({ ok: v.boolean(), existed: v.boolean() }),
  handler: putReportImpl,
});

export const getReportImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly jobHash: string; readonly datasetHash: string },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseCompileReports")
        .withIndex("by_signatureId_jobHash_datasetHash", (q) =>
          q.eq("signatureId", args.signatureId).eq("jobHash", args.jobHash).eq("datasetHash", args.datasetHash),
        )
        .unique(),
    );

    if (!row) return { ok: true, report: null };

    return {
      ok: true,
      report: {
        signatureId: String((row as any).signatureId ?? ""),
        jobHash: String((row as any).jobHash ?? ""),
        datasetId: String((row as any).datasetId ?? ""),
        datasetHash: String((row as any).datasetHash ?? ""),
        compiled_id: String((row as any).compiled_id ?? ""),
        json: (row as any).json ?? null,
        createdAtMs: Number((row as any).createdAtMs ?? 0),
      },
    };
  });

export const getReport = effectQuery({
  args: {
    signatureId: v.string(),
    jobHash: v.string(),
    datasetHash: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    report: v.union(
      v.null(),
      v.object({
        signatureId: v.string(),
        jobHash: v.string(),
        datasetId: v.string(),
        datasetHash: v.string(),
        compiled_id: v.string(),
        json: v.any(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: getReportImpl,
});

export const listReportsImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(0, Math.min(200, Math.floor(args.limit)))
        : 50;

    const rows: any[] =
      limit === 0
        ? []
        : yield* tryPromise(() =>
            ctx.db
              .query("dseCompileReports")
              .withIndex("by_signatureId_createdAtMs", (q) => q.eq("signatureId", args.signatureId))
              .order("desc")
              .take(limit),
          );

    return {
      ok: true,
      reports: rows.map((r) => ({
        signatureId: String(r.signatureId ?? ""),
        jobHash: String(r.jobHash ?? ""),
        datasetHash: String(r.datasetHash ?? ""),
        compiled_id: String(r.compiled_id ?? ""),
        createdAtMs: Number(r.createdAtMs ?? 0),
      })),
    };
  });

export const listReports = effectQuery({
  args: {
    signatureId: v.string(),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    reports: v.array(
      v.object({
        signatureId: v.string(),
        jobHash: v.string(),
        datasetHash: v.string(),
        compiled_id: v.string(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: listReportsImpl,
});

