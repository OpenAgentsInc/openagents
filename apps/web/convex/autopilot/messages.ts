import { v } from "convex/values";
import { Effect } from "effect";

import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "./access";
import { FIRST_OPEN_WELCOME_MESSAGE } from "./defaults";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

const deleteAllByQuery = (rows: ReadonlyArray<{ readonly _id: any }>, ctx: any) =>
  Effect.forEach(rows, (row) => tryPromise(() => ctx.db.delete(row._id)), { discard: true });

export const getThreadSnapshot = effectQuery({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    maxMessages: v.optional(v.number()),
    maxParts: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    threadId: v.string(),
    messages: v.array(v.any()),
    parts: v.array(v.any()),
  }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      yield* assertThreadAccess(ctx, args);

      const maxMessages =
        typeof args.maxMessages === "number" && Number.isFinite(args.maxMessages)
          ? Math.max(0, Math.min(200, Math.floor(args.maxMessages)))
          : 100;
      const maxParts =
        typeof args.maxParts === "number" && Number.isFinite(args.maxParts)
          ? Math.max(0, Math.min(5_000, Math.floor(args.maxParts)))
          : 2_000;

      const messages =
        maxMessages === 0
          ? []
          : yield* tryPromise(() =>
              ctx.db
                .query("messages")
                .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
                .order("asc")
                .take(maxMessages),
            );

      const parts =
        maxParts === 0
          ? []
          : yield* tryPromise(() =>
              ctx.db
                .query("messageParts")
                .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
                .order("asc")
                .take(maxParts),
            );

      return {
        ok: true,
        threadId: args.threadId,
        messages: messages.map((m: any) => ({
          messageId: m.messageId,
          role: m.role,
          status: m.status,
          text: m.text ?? null,
          runId: m.runId ?? null,
          createdAtMs: m.createdAtMs,
          updatedAtMs: m.updatedAtMs,
        })),
        parts: parts.map((p: any) => ({
          messageId: p.messageId,
          runId: p.runId,
          seq: p.seq,
          part: p.part,
          createdAtMs: p.createdAtMs,
        })),
      };
    }),
});

export const createRun = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    text: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    runId: v.string(),
    userMessageId: v.string(),
    assistantMessageId: v.string(),
  }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      const thread = yield* assertThreadAccess(ctx, args);

      const runId = newId();
      const userMessageId = newId();
      const assistantMessageId = newId();

      const now = nowMs();

      yield* tryPromise(() =>
        ctx.db.insert("messages", {
          threadId: args.threadId,
          messageId: userMessageId,
          role: "user",
          status: "final",
          text: args.text,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );

      yield* tryPromise(() =>
        ctx.db.insert("messages", {
          threadId: args.threadId,
          messageId: assistantMessageId,
          role: "assistant",
          status: "streaming",
          runId,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );

      yield* tryPromise(() =>
        ctx.db.insert("runs", {
          threadId: args.threadId,
          runId,
          assistantMessageId,
          status: "streaming",
          cancelRequested: false,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );

      yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs: now }));

      return { ok: true, runId, userMessageId, assistantMessageId };
    }),
});

export const requestCancel = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      yield* assertThreadAccess(ctx, args);

      const run = yield* tryPromise(() =>
        ctx.db.query("runs").withIndex("by_runId", (q) => q.eq("runId", args.runId)).unique(),
      );
      if (!run || (run as any).threadId !== args.threadId) return { ok: true };

      yield* tryPromise(() => ctx.db.patch(run._id, { cancelRequested: true, updatedAtMs: nowMs() }));
      return { ok: true };
    }),
});

export const isCancelRequested = effectQuery({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
  },
  returns: v.object({ ok: v.boolean(), cancelRequested: v.boolean() }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      yield* assertThreadAccess(ctx, args);

      const run = yield* tryPromise(() =>
        ctx.db.query("runs").withIndex("by_runId", (q) => q.eq("runId", args.runId)).unique(),
      );

      if (!run || (run as any).threadId !== args.threadId) {
        return { ok: true, cancelRequested: false };
      }

      return { ok: true, cancelRequested: Boolean((run as any).cancelRequested) };
    }),
});

export const appendParts = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
    messageId: v.string(),
    parts: v.array(
      v.object({
        seq: v.number(),
        part: v.any(),
      }),
    ),
  },
  returns: v.object({ ok: v.boolean(), inserted: v.number() }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      yield* assertThreadAccess(ctx, args);

      let inserted = 0;
      const now = nowMs();

      for (const item of args.parts) {
        const seq = Math.max(0, Math.floor(item.seq));
        const existing = yield* tryPromise(() =>
          ctx.db
            .query("messageParts")
            .withIndex("by_runId_seq", (q) => q.eq("runId", args.runId).eq("seq", seq))
            .unique(),
        );
        if (existing) continue;

        yield* tryPromise(() =>
          ctx.db.insert("messageParts", {
            threadId: args.threadId,
            runId: args.runId,
            messageId: args.messageId,
            seq,
            part: item.part,
            createdAtMs: now,
          }),
        );
        inserted++;
      }

      const run = yield* tryPromise(() =>
        ctx.db.query("runs").withIndex("by_runId", (q) => q.eq("runId", args.runId)).unique(),
      );
      if (run && (run as any).threadId === args.threadId) {
        yield* tryPromise(() => ctx.db.patch(run._id, { updatedAtMs: now }));
      }

      const message = yield* tryPromise(() =>
        ctx.db
          .query("messages")
          .withIndex("by_threadId_messageId", (q) =>
            q.eq("threadId", args.threadId).eq("messageId", args.messageId),
          )
          .unique(),
      );
      if (message) {
        yield* tryPromise(() => ctx.db.patch(message._id, { updatedAtMs: now }));
      }

      return { ok: true, inserted };
    }),
});

export const finalizeRun = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
    messageId: v.string(),
    status: v.union(v.literal("final"), v.literal("error"), v.literal("canceled")),
    text: v.optional(v.string()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      yield* assertThreadAccess(ctx, args);

      const now = nowMs();

      const message = yield* tryPromise(() =>
        ctx.db
          .query("messages")
          .withIndex("by_threadId_messageId", (q) =>
            q.eq("threadId", args.threadId).eq("messageId", args.messageId),
          )
          .unique(),
      );

      if (message) {
        yield* tryPromise(() =>
          ctx.db.patch(message._id, {
            status: args.status,
            text: typeof args.text === "string" ? args.text : (message as any).text,
            updatedAtMs: now,
          }),
        );
      }

      const run = yield* tryPromise(() =>
        ctx.db.query("runs").withIndex("by_runId", (q) => q.eq("runId", args.runId)).unique(),
      );
      if (run && (run as any).threadId === args.threadId) {
        yield* tryPromise(() => ctx.db.patch(run._id, { status: args.status, updatedAtMs: now }));
      }

      return { ok: true };
    }),
});

export const clearMessages = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    keepWelcome: v.optional(v.boolean()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      const thread = yield* assertThreadAccess(ctx, args);

      const messages = yield* tryPromise(() =>
        ctx.db
          .query("messages")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .collect(),
      );
      const parts = yield* tryPromise(() =>
        ctx.db
          .query("messageParts")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .collect(),
      );
      const runs = yield* tryPromise(() =>
        ctx.db
          .query("runs")
          .withIndex("by_threadId_updatedAtMs", (q) => q.eq("threadId", args.threadId))
          .collect(),
      );
      const receipts = yield* tryPromise(() =>
        ctx.db
          .query("receipts")
          .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
          .collect(),
      );

      yield* deleteAllByQuery(parts, ctx);
      yield* deleteAllByQuery(runs, ctx);
      yield* deleteAllByQuery(receipts, ctx);
      yield* deleteAllByQuery(messages, ctx);

      const now = nowMs();

      if (args.keepWelcome !== false) {
        yield* tryPromise(() =>
          ctx.db.insert("messages", {
            threadId: args.threadId,
            messageId: newId(),
            role: "assistant",
            status: "final",
            text: FIRST_OPEN_WELCOME_MESSAGE,
            createdAtMs: now,
            updatedAtMs: now,
          }),
        );
      }

      yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs: now }));

      return { ok: true };
    }),
});
