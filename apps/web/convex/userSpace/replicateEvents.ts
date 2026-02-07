import { v } from "convex/values"
import { Effect, Option } from "effect"

import { effectMutation } from "../effect/functions"
import { tryPromise } from "../effect/tryPromise"

export const replicateEvents = effectMutation({
  args: {
    userSpaceId: v.string(),
    events: v.array(
      v.object({
        eventId: v.string(),
        seq: v.number(),
        kind: v.string(),
        json: v.string(),
        createdAtMs: v.number(),
      }),
    ),
  },
  returns: v.object({
    ok: v.boolean(),
    inserted: v.number(),
  }),
  handler: (ctx, args) =>
    Effect.gen(function* () {
      const identity = yield* ctx.auth.getUserIdentity()
      const subject = Option.match(identity, {
        onNone: () => null,
        onSome: (u) => String((u as any).subject ?? ""),
      })

      if (!subject) {
        return yield* Effect.fail(new Error("unauthorized"))
      }
      if (subject !== args.userSpaceId) {
        return yield* Effect.fail(new Error("forbidden"))
      }

      let inserted = 0

      for (const event of args.events) {
        const existing = yield* tryPromise(() =>
          ctx.db
            .query("userSpaceEvents")
            .withIndex("by_eventId", (q) => q.eq("eventId", event.eventId))
            .unique(),
        )

        if (existing) continue

        yield* tryPromise(() =>
          ctx.db.insert("userSpaceEvents", {
            userSpaceId: args.userSpaceId,
            seq: event.seq,
            eventId: event.eventId,
            kind: event.kind,
            json: event.json,
            createdAtMs: event.createdAtMs,
          }),
        )
        inserted++
      }

      return { ok: true, inserted }
    }),
})
