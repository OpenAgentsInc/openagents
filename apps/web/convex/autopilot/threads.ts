import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx } from "../effect/ctx";
import { effectMutation } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "./access";
import { FIRST_OPEN_WELCOME_MESSAGE, makeDefaultBlueprintState } from "./defaults";

const nowMs = () => Date.now();

const newId = () => crypto.randomUUID();

const ensureWelcomeMessage = (ctx: EffectMutationCtx, threadId: string) =>
  Effect.gen(function* () {
    const existing = yield* tryPromise(() =>
      ctx.db
        .query("messages")
        .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", threadId))
        .first(),
    );

    if (existing) return;

    yield* tryPromise(() =>
      ctx.db.insert("messages", {
        threadId,
        messageId: newId(),
        role: "assistant",
        status: "final",
        text: FIRST_OPEN_WELCOME_MESSAGE,
        createdAtMs: nowMs(),
        updatedAtMs: nowMs(),
      }),
    );
  });

const ensureBlueprintRow = (ctx: EffectMutationCtx, threadId: string) =>
  Effect.gen(function* () {
    const existing = yield* tryPromise(() =>
      ctx.db
        .query("blueprints")
        .withIndex("by_threadId", (q) => q.eq("threadId", threadId))
        .unique(),
    );

    if (existing) return;

    yield* tryPromise(() =>
      ctx.db.insert("blueprints", {
        threadId,
        blueprint: makeDefaultBlueprintState(threadId),
        updatedAtMs: nowMs(),
      }),
    );
  });

export const ensureAnonThreadImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly anonKey: string },
) =>
  Effect.gen(function* () {
    const existing = yield* tryPromise(() =>
      ctx.db
        .query("threads")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );

    if (!existing) {
      yield* tryPromise(() =>
        ctx.db.insert("threads", {
          threadId: args.threadId,
          anonKey: args.anonKey,
          createdAtMs: nowMs(),
          updatedAtMs: nowMs(),
        }),
      );
    } else if (existing.ownerId) {
      // Thread already claimed; allow (client can keep using threadId).
    } else if (existing.anonKey && existing.anonKey !== args.anonKey) {
      return yield* Effect.fail(new Error("forbidden"));
    }

    yield* ensureBlueprintRow(ctx, args.threadId);
    yield* ensureWelcomeMessage(ctx, args.threadId);

    return { ok: true, threadId: args.threadId };
  });

export const ensureAnonThread = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    threadId: v.string(),
  }),
  handler: ensureAnonThreadImpl,
});

export const ensureOwnedThreadImpl = (ctx: EffectMutationCtx) =>
  Effect.gen(function* () {
    const subject = yield* getSubject(ctx);
    if (!subject) {
      return yield* Effect.fail(new Error("unauthorized"));
    }

    const user = yield* tryPromise(() =>
      ctx.db.query("users").withIndex("by_userId", (q) => q.eq("userId", subject)).unique(),
    );

    let defaultThreadId: string | undefined =
      user && typeof (user as any).defaultThreadId === "string"
        ? (user as any).defaultThreadId
        : undefined;

    if (!defaultThreadId) {
      defaultThreadId = newId();

      if (user) {
        yield* tryPromise(() => ctx.db.patch(user._id, { defaultThreadId }));
      } else {
        yield* tryPromise(() =>
          ctx.db.insert("users", { userId: subject, createdAtMs: nowMs(), defaultThreadId }),
        );
      }
    }

    const threadId = defaultThreadId;
    if (!threadId) return yield* Effect.fail(new Error("missing_thread"));

    const threadExisting = yield* tryPromise(() =>
      ctx.db
        .query("threads")
        .withIndex("by_threadId", (q) => q.eq("threadId", threadId))
        .unique(),
    );

    if (!threadExisting) {
      yield* tryPromise(() =>
        ctx.db.insert("threads", {
          threadId,
          ownerId: subject,
          createdAtMs: nowMs(),
          updatedAtMs: nowMs(),
        }),
      );
    } else if (threadExisting.ownerId !== subject) {
      return yield* Effect.fail(new Error("forbidden"));
    }

    yield* ensureBlueprintRow(ctx, threadId);
    yield* ensureWelcomeMessage(ctx, threadId);

    return { ok: true, threadId };
  });

export const ensureOwnedThread = effectMutation({
  args: {},
  returns: v.object({
    ok: v.boolean(),
    threadId: v.string(),
  }),
  handler: ensureOwnedThreadImpl,
});

/** Best-effort: claim anon thread for the current user. Never throws — returns ok so client/Convex don't log Server Error. */
export const claimAnonThreadImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly anonKey: string },
) =>
  Effect.gen(function* () {
    const subject = yield* getSubject(ctx);
    if (!subject) return { ok: true, threadId: args.threadId };

    const thread = yield* tryPromise(() =>
      ctx.db
        .query("threads")
        .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
        .unique(),
    );
    if (!thread) return { ok: true, threadId: args.threadId };

    if (thread.ownerId) {
      if (thread.ownerId !== subject) return { ok: true, threadId: args.threadId };
    } else {
      if (!thread.anonKey || thread.anonKey !== args.anonKey) return { ok: true, threadId: args.threadId };

      yield* tryPromise(() =>
        ctx.db.patch(thread._id, {
          ownerId: subject,
          updatedAtMs: nowMs(),
        }),
      );
    }

    const user = yield* tryPromise(() =>
      ctx.db.query("users").withIndex("by_userId", (q) => q.eq("userId", subject)).unique(),
    );
    if (user) {
      yield* tryPromise(() => ctx.db.patch(user._id, { defaultThreadId: args.threadId }));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("users", { userId: subject, createdAtMs: nowMs(), defaultThreadId: args.threadId }),
      );
    }

    return { ok: true, threadId: args.threadId };
  });

export const claimAnonThread = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    threadId: v.string(),
  }),
  handler: claimAnonThreadImpl,
});
