import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

const nowMs = () => Date.now();

export const putArtifactImpl = (
  ctx: EffectMutationCtx,
  args: { readonly signatureId: string; readonly compiled_id: string; readonly json: unknown },
) =>
  Effect.gen(function* () {
    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseArtifacts")
        .withIndex("by_signatureId_compiled_id", (q) =>
          q.eq("signatureId", args.signatureId).eq("compiled_id", args.compiled_id),
        )
        .unique(),
    );

    if (existing) return { ok: true, existed: true as const };

    yield* tryPromise(() =>
      ctx.db.insert("dseArtifacts", {
        signatureId: args.signatureId,
        compiled_id: args.compiled_id,
        json: args.json,
        createdAtMs: nowMs(),
      }),
    );

    return { ok: true, existed: false as const };
  });

export const putArtifact = effectMutation({
  args: {
    signatureId: v.string(),
    compiled_id: v.string(),
    json: v.any(),
  },
  returns: v.object({ ok: v.boolean(), existed: v.boolean() }),
  handler: putArtifactImpl,
});

export const getArtifactImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly compiled_id: string },
) =>
  Effect.gen(function* () {
    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseArtifacts")
        .withIndex("by_signatureId_compiled_id", (q) =>
          q.eq("signatureId", args.signatureId).eq("compiled_id", args.compiled_id),
        )
        .unique(),
    );

    return { ok: true, artifact: (row as any)?.json ?? null };
  });

export const getArtifact = effectQuery({
  args: {
    signatureId: v.string(),
    compiled_id: v.string(),
  },
  returns: v.object({ ok: v.boolean(), artifact: v.any() }),
  handler: getArtifactImpl,
});

export const listArtifactsImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(0, Math.min(200, Math.floor(args.limit)))
        : 50;

    const rows: any[] =
      limit === 0
        ? []
        : yield* tryPromise(() =>
            ctx.db
              .query("dseArtifacts")
              .withIndex("by_signatureId_createdAtMs", (q) => q.eq("signatureId", args.signatureId))
              .order("desc")
              .take(limit),
          );

    return {
      ok: true,
      artifacts: rows.map((r) => ({
        signatureId: String(r.signatureId ?? ""),
        compiled_id: String(r.compiled_id ?? ""),
        createdAtMs: Number(r.createdAtMs ?? 0),
      })),
    };
  });

export const listArtifacts = effectQuery({
  args: {
    signatureId: v.string(),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    artifacts: v.array(
      v.object({
        signatureId: v.string(),
        compiled_id: v.string(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: listArtifactsImpl,
});

