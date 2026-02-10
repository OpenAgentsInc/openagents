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

/**
 * Apply user handle during bootstrap and advance to ask_agent_name.
 * Only applies when bootstrapState.stage === "ask_user_handle".
 */
export const applyBootstrapUserHandleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly handle: string;
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const handle = String(args.handle ?? "").trim();
    if (!handle) return { ok: true, applied: false, updatedAtMs: 0 };

    const row = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    const current = row ? ((row as any).blueprint ?? null) : makeDefaultBlueprintState(args.threadId);
    if (!current || typeof current !== "object") return { ok: true, applied: false, updatedAtMs: 0 };

    const bs = (current as any).bootstrapState;
    const status = bs?.status;
    const stage = bs?.stage;
    if (status === "complete" || stage !== "ask_user_handle") {
      return { ok: true, applied: false, updatedAtMs: (row as any)?.updatedAtMs ?? 0 };
    }

    const now = new Date().toISOString();
    const updated = {
      ...current,
      docs: {
        ...(current as any).docs,
        user: {
          ...((current as any).docs?.user ?? {}),
          addressAs: handle,
          name: handle,
          updatedAt: now,
          updatedBy: "agent",
        },
      },
      bootstrapState: {
        ...bs,
        status: bs?.status === "pending" ? "in_progress" : (bs?.status ?? "in_progress"),
        startedAt: bs?.startedAt ?? now,
        stage: "ask_agent_name",
      },
    };

    const updatedAtMs = nowMs();
    if (row) {
      yield* tryPromise(() => ctx.db.patch((row as any)._id, { blueprint: updated, updatedAtMs }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("blueprints", { threadId: args.threadId, blueprint: updated, updatedAtMs }),
      );
    }

    yield* tryPromise(() => ctx.db.patch((thread as any)._id, { updatedAtMs }));

    return { ok: true, applied: true, updatedAtMs };
  });

export const applyBootstrapUserHandle = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    handle: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    applied: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: applyBootstrapUserHandleImpl,
});

/**
 * Apply agent name during bootstrap and advance to ask_vibe.
 * Only applies when bootstrapState.stage === "ask_agent_name".
 */
export const applyBootstrapAgentNameImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly name: string;
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const name = String(args.name ?? "").trim();
    if (!name) return { ok: true, applied: false, updatedAtMs: 0 };

    const row = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    const current = row ? ((row as any).blueprint ?? null) : makeDefaultBlueprintState(args.threadId);
    if (!current || typeof current !== "object") return { ok: true, applied: false, updatedAtMs: 0 };

    const bs = (current as any).bootstrapState;
    const status = bs?.status;
    const stage = bs?.stage;
    if (status === "complete" || stage !== "ask_agent_name") {
      return { ok: true, applied: false, updatedAtMs: (row as any)?.updatedAtMs ?? 0 };
    }

    const now = new Date().toISOString();
    const updated = {
      ...current,
      docs: {
        ...(current as any).docs,
        identity: {
          ...((current as any).docs?.identity ?? {}),
          name,
          updatedAt: now,
          updatedBy: "agent",
        },
      },
      bootstrapState: {
        ...bs,
        status: bs?.status === "pending" ? "in_progress" : (bs?.status ?? "in_progress"),
        startedAt: bs?.startedAt ?? now,
        stage: "ask_vibe",
      },
    };

    const updatedAtMs = nowMs();
    if (row) {
      yield* tryPromise(() => ctx.db.patch((row as any)._id, { blueprint: updated, updatedAtMs }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("blueprints", { threadId: args.threadId, blueprint: updated, updatedAtMs }),
      );
    }

    yield* tryPromise(() => ctx.db.patch((thread as any)._id, { updatedAtMs }));

    return { ok: true, applied: true, updatedAtMs };
  });

export const applyBootstrapAgentName = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    name: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    applied: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: applyBootstrapAgentNameImpl,
});

/**
 * Apply agent vibe during bootstrap and advance to ask_boundaries.
 * Only applies when bootstrapState.stage === "ask_vibe".
 */
export const applyBootstrapAgentVibeImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly vibe: string;
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const vibe = String(args.vibe ?? "").trim();
    if (!vibe) return { ok: true, applied: false, updatedAtMs: 0 };

    const row = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    const current = row ? ((row as any).blueprint ?? null) : makeDefaultBlueprintState(args.threadId);
    if (!current || typeof current !== "object") return { ok: true, applied: false, updatedAtMs: 0 };

    const bs = (current as any).bootstrapState;
    const status = bs?.status;
    const stage = bs?.stage;
    if (status === "complete" || stage !== "ask_vibe") {
      return { ok: true, applied: false, updatedAtMs: (row as any)?.updatedAtMs ?? 0 };
    }

    const now = new Date().toISOString();
    const updated = {
      ...current,
      docs: {
        ...(current as any).docs,
        identity: {
          ...((current as any).docs?.identity ?? {}),
          vibe,
          updatedAt: now,
          updatedBy: "agent",
        },
      },
      bootstrapState: {
        ...bs,
        status: bs?.status === "pending" ? "in_progress" : (bs?.status ?? "in_progress"),
        startedAt: bs?.startedAt ?? now,
        stage: "ask_boundaries",
      },
    };

    const updatedAtMs = nowMs();
    if (row) {
      yield* tryPromise(() => ctx.db.patch((row as any)._id, { blueprint: updated, updatedAtMs }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("blueprints", { threadId: args.threadId, blueprint: updated, updatedAtMs }),
      );
    }

    yield* tryPromise(() => ctx.db.patch((thread as any)._id, { updatedAtMs }));

    return { ok: true, applied: true, updatedAtMs };
  });

export const applyBootstrapAgentVibe = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    vibe: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    applied: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: applyBootstrapAgentVibeImpl,
});

/**
 * Mark bootstrap complete (optional boundaries update).
 * Only applies when bootstrapState.stage === "ask_boundaries".
 */
export const applyBootstrapCompleteImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly boundaries?: ReadonlyArray<string> | undefined;
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    const current = row ? ((row as any).blueprint ?? null) : makeDefaultBlueprintState(args.threadId);
    if (!current || typeof current !== "object") return { ok: true, applied: false, updatedAtMs: 0 };

    const bs = (current as any).bootstrapState;
    const status = bs?.status;
    const stage = bs?.stage;
    if (status === "complete" || stage !== "ask_boundaries") {
      return { ok: true, applied: false, updatedAtMs: (row as any)?.updatedAtMs ?? 0 };
    }

    const now = new Date().toISOString();
    const boundariesRaw = Array.isArray(args.boundaries) ? args.boundaries : [];
    const boundaries = boundariesRaw
      .map((b) => String(b ?? "").trim())
      .filter((b) => b.length > 0)
      .slice(0, 32);

    const currentCharacter = (current as any).docs?.character ?? {};
    const nextBoundaries =
      boundaries.length > 0
        ? Array.from(new Set([...(Array.isArray(currentCharacter.boundaries) ? currentCharacter.boundaries : []), ...boundaries])).slice(
            0,
            64,
          )
        : Array.isArray(currentCharacter.boundaries)
          ? currentCharacter.boundaries
          : [];

    const updated = {
      ...current,
      docs: {
        ...(current as any).docs,
        character: {
          ...currentCharacter,
          boundaries: nextBoundaries,
          updatedAt: now,
          updatedBy: "agent",
        },
      },
      bootstrapState: {
        ...bs,
        status: "complete",
        stage: null,
        completedAt: now,
      },
    };

    const updatedAtMs = nowMs();
    if (row) {
      yield* tryPromise(() => ctx.db.patch((row as any)._id, { blueprint: updated, updatedAtMs }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("blueprints", { threadId: args.threadId, blueprint: updated, updatedAtMs }),
      );
    }

    yield* tryPromise(() => ctx.db.patch((thread as any)._id, { updatedAtMs }));

    return { ok: true, applied: true, updatedAtMs };
  });

export const applyBootstrapComplete = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    boundaries: v.optional(v.array(v.string())),
  },
  returns: v.object({
    ok: v.boolean(),
    applied: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: applyBootstrapCompleteImpl,
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
