import { Context, Effect, Layer, Schedule, Schema } from "effect"

import { api } from "../../convex/_generated/api"
import { ConvexService } from "./convex"
import type { RequestContextService } from "./requestContext"

export class ConvexReplicationError extends Schema.TaggedError<ConvexReplicationError>()(
  "ConvexReplicationError",
  {
    operation: Schema.String,
    error: Schema.Defect,
  },
) {}

export type UserSpaceEventForReplication = {
  readonly eventId: string
  readonly seq: number
  readonly kind: string
  readonly json: string
  readonly createdAtMs: number
}

export type ConvexReplicationApi = {
  readonly replicateUserSpaceEvents: (input: {
    readonly userSpaceId: string
    readonly events: ReadonlyArray<UserSpaceEventForReplication>
  }) => Effect.Effect<void, ConvexReplicationError, RequestContextService>
}

export class ConvexReplicationService extends Context.Tag(
  "@openagents/web/ConvexReplicationService",
)<ConvexReplicationService, ConvexReplicationApi>() {}

const MAX_EVENTS_PER_CALL = 200

export const ConvexReplicationLive = Layer.effect(
  ConvexReplicationService,
  Effect.gen(function* () {
    const convex = yield* ConvexService

    const replicateUserSpaceEvents = Effect.fn("ConvexReplication.replicateUserSpaceEvents")(
      function* (input: {
        readonly userSpaceId: string
        readonly events: ReadonlyArray<UserSpaceEventForReplication>
      }) {
        const events = input.events.slice(0, MAX_EVENTS_PER_CALL)

        yield* convex
          .mutation(api.userSpace.replicateEvents.replicateEvents, {
            userSpaceId: input.userSpaceId,
            events,
          } as any)
          .pipe(
            Effect.mapError((error) =>
              ConvexReplicationError.make({
                operation: "replicateUserSpaceEvents",
                error,
              }),
            ),
          )

        return void 0
      },
    )

    return ConvexReplicationService.of({
      replicateUserSpaceEvents: (input) =>
        replicateUserSpaceEvents(input).pipe(
          Effect.retry(
            Schedule.exponential("100 millis").pipe(
              Schedule.jittered,
              Schedule.intersect(Schedule.recurs(3)),
            ),
          ),
        ),
    })
  }),
)
