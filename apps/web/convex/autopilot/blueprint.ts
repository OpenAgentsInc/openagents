import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "./access";
import { makeDefaultBlueprintState } from "./defaults";

const nowMs = () => Date.now();

export const getBlueprintImpl = (
  ctx: EffectQueryCtx,
  args: { readonly threadId: string; readonly anonKey?: string | undefined },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    if (!row) {
      return { ok: true, blueprint: makeDefaultBlueprintState(args.threadId), updatedAtMs: 0 };
    }

    return { ok: true, blueprint: (row as any).blueprint ?? null, updatedAtMs: (row as any).updatedAtMs ?? 0 };
  });

export const getBlueprint = effectQuery({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    blueprint: v.any(),
    updatedAtMs: v.number(),
  }),
  handler: getBlueprintImpl,
});

export const setBlueprintImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly anonKey?: string | undefined; readonly blueprint: unknown },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const updatedAtMs = nowMs();
    const existing = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, { blueprint: args.blueprint ?? null, updatedAtMs }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("blueprints", { threadId: args.threadId, blueprint: args.blueprint ?? null, updatedAtMs }),
      );
    }

    yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs }));

    return { ok: true, updatedAtMs };
  });

export const setBlueprint = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    blueprint: v.any(),
  },
  returns: v.object({
    ok: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: setBlueprintImpl,
});

export const resetBlueprintImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly anonKey?: string | undefined },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const updatedAtMs = nowMs();
    const blueprint = makeDefaultBlueprintState(args.threadId);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, { blueprint, updatedAtMs }));
    } else {
      yield* tryPromise(() => ctx.db.insert("blueprints", { threadId: args.threadId, blueprint, updatedAtMs }));
    }

    yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs }));

    return { ok: true, updatedAtMs };
  });

export const resetBlueprint = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: resetBlueprintImpl,
});
