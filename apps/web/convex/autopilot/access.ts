import { Effect, Option } from "effect";

import { tryPromise } from "../effect/tryPromise";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";

export type AutopilotAccessInput = {
  readonly threadId: string;
  /** @deprecated Anon removed; owner-only access. */
  readonly anonKey?: string | undefined;
};

export const getSubject = (ctx: EffectQueryCtx | EffectMutationCtx) =>
  ctx.auth.getUserIdentity().pipe(
    Effect.map((identity) =>
      Option.match(identity, {
        onNone: () => null,
        onSome: (u) => String((u as any).subject ?? ""),
      }),
    ),
    Effect.map((subject) => (subject && subject.length > 0 ? subject : null)),
  );

export const assertThreadAccess = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  input: AutopilotAccessInput,
) =>
  Effect.gen(function* () {
    const thread = yield* tryPromise(() =>
      ctx.db
        .query("threads")
        .withIndex("by_threadId", (q) => q.eq("threadId", input.threadId))
        .unique(),
    );

    if (!thread) {
      return yield* Effect.fail(new Error("thread_not_found"));
    }

    const subject = yield* getSubject(ctx);
    if (subject && thread.ownerId === subject) return thread;

    return yield* Effect.fail(new Error("forbidden"));
  });

