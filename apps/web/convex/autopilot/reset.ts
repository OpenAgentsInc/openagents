import { v } from "convex/values";
import { Effect } from "effect";

import { effectMutation } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "./access";
import { FIRST_OPEN_WELCOME_MESSAGE, makeDefaultBlueprintState } from "./defaults";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

const deleteAll = (rows: ReadonlyArray<{ readonly _id: any }>, ctx: any) =>
  Effect.forEach(rows, (row) => tryPromise(() => ctx.db.delete(row._id)), { discard: true });

export const resetThread = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
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

      yield* deleteAll(parts, ctx);
      yield* deleteAll(runs, ctx);
      yield* deleteAll(receipts, ctx);
      yield* deleteAll(messages, ctx);

      const now = nowMs();

      // Reset blueprint.
      const blueprintRow = yield* tryPromise(() =>
        ctx.db
          .query("blueprints")
          .withIndex("by_threadId", (q) => q.eq("threadId", args.threadId))
          .unique(),
      );
      const blueprint = makeDefaultBlueprintState(args.threadId);
      if (blueprintRow) {
        yield* tryPromise(() => ctx.db.patch(blueprintRow._id, { blueprint, updatedAtMs: now }));
      } else {
        yield* tryPromise(() => ctx.db.insert("blueprints", { threadId: args.threadId, blueprint, updatedAtMs: now }));
      }

      // Re-seed welcome message.
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

      yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs: now }));

      return { ok: true };
    }),
});

