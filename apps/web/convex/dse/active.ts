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

const recordHistory = (
  ctx: EffectMutationCtx,
  input: {
    readonly signatureId: string;
    readonly action: "set" | "clear" | "rollback";
    readonly fromCompiledId?: string | undefined;
    readonly toCompiledId?: string | undefined;
    readonly reason?: string | undefined;
  },
) =>
  Effect.gen(function* () {
    const actorUserId = yield* getSubject(ctx);
    yield* tryPromise(() =>
      ctx.db.insert("dseActiveArtifactHistory", {
        signatureId: input.signatureId,
        action: input.action,
        fromCompiledId: input.fromCompiledId,
        toCompiledId: input.toCompiledId,
        reason: input.reason,
        ...(actorUserId ? { actorUserId } : {}),
        createdAtMs: nowMs(),
      }),
    );
  });

export const getActiveImpl = (ctx: EffectQueryCtx, args: { readonly signatureId: string }) =>
  Effect.gen(function* () {
    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifacts")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    return {
      ok: true,
      compiled_id: row ? String((row as any).compiled_id ?? "") : null,
      updatedAtMs: row ? Number((row as any).updatedAtMs ?? 0) : null,
    };
  });

export const getActive = effectQuery({
  args: { signatureId: v.string() },
  returns: v.object({
    ok: v.boolean(),
    compiled_id: v.union(v.null(), v.string()),
    updatedAtMs: v.union(v.null(), v.number()),
  }),
  handler: getActiveImpl,
});

export const setActiveImpl = (
  ctx: EffectMutationCtx,
  args: { readonly signatureId: string; readonly compiled_id: string; readonly reason?: string | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const artifact = yield* tryPromise(() =>
      ctx.db
        .query("dseArtifacts")
        .withIndex("by_signatureId_compiled_id", (q) =>
          q.eq("signatureId", args.signatureId).eq("compiled_id", args.compiled_id),
        )
        .unique(),
    );
    if (!artifact) {
      return yield* Effect.fail(new Error("artifact_not_found"));
    }

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifacts")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    const now = nowMs();
    if (existing) {
      yield* tryPromise(() => ctx.db.patch((existing as any)._id, { compiled_id: args.compiled_id, updatedAtMs: now }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("dseActiveArtifacts", { signatureId: args.signatureId, compiled_id: args.compiled_id, updatedAtMs: now }),
      );
    }

    yield* recordHistory(ctx, {
      signatureId: args.signatureId,
      action: "set",
      fromCompiledId: existing ? String((existing as any).compiled_id ?? "") : undefined,
      toCompiledId: args.compiled_id,
      reason: args.reason,
    });

    return { ok: true };
  });

export const setActive = effectMutation({
  args: {
    signatureId: v.string(),
    compiled_id: v.string(),
    reason: v.optional(v.string()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: setActiveImpl,
});

export const clearActiveImpl = (
  ctx: EffectMutationCtx,
  args: { readonly signatureId: string; readonly reason?: string | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifacts")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    if (existing) {
      yield* tryPromise(() => ctx.db.delete((existing as any)._id));
    }

    yield* recordHistory(ctx, {
      signatureId: args.signatureId,
      action: "clear",
      fromCompiledId: existing ? String((existing as any).compiled_id ?? "") : undefined,
      toCompiledId: undefined,
      reason: args.reason,
    });

    return { ok: true };
  });

export const clearActive = effectMutation({
  args: {
    signatureId: v.string(),
    reason: v.optional(v.string()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: clearActiveImpl,
});

export const rollbackActiveImpl = (
  ctx: EffectMutationCtx,
  args: { readonly signatureId: string; readonly reason?: string | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifacts")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    const current = existing ? String((existing as any).compiled_id ?? "") : null;

    const history: any[] = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifactHistory")
        .withIndex("by_signatureId_createdAtMs", (q) => q.eq("signatureId", args.signatureId))
        .order("desc")
        .take(50),
    );

    const prev = history.find((h) => {
      const to = typeof h?.toCompiledId === "string" ? h.toCompiledId : null;
      // Find the most recent "previous" pointer different from current.
      return to !== current;
    });

    const target = prev && typeof prev.toCompiledId === "string" ? String(prev.toCompiledId) : null;

    const now = nowMs();
    if (target) {
      const artifact = yield* tryPromise(() =>
        ctx.db
          .query("dseArtifacts")
          .withIndex("by_signatureId_compiled_id", (q) => q.eq("signatureId", args.signatureId).eq("compiled_id", target))
          .unique(),
      );
      if (!artifact) return yield* Effect.fail(new Error("artifact_not_found"));

      if (existing) {
        yield* tryPromise(() => ctx.db.patch((existing as any)._id, { compiled_id: target, updatedAtMs: now }));
      } else {
        yield* tryPromise(() => ctx.db.insert("dseActiveArtifacts", { signatureId: args.signatureId, compiled_id: target, updatedAtMs: now }));
      }
    } else if (existing) {
      yield* tryPromise(() => ctx.db.delete((existing as any)._id));
    }

    yield* recordHistory(ctx, {
      signatureId: args.signatureId,
      action: "rollback",
      fromCompiledId: current ?? undefined,
      toCompiledId: target ?? undefined,
      reason: args.reason,
    });

    return { ok: true, compiled_id: target };
  });

export const rollbackActive = effectMutation({
  args: {
    signatureId: v.string(),
    reason: v.optional(v.string()),
  },
  returns: v.object({ ok: v.boolean(), compiled_id: v.union(v.null(), v.string()) }),
  handler: rollbackActiveImpl,
});
