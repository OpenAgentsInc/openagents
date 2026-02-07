import { Context, Effect, Layer, Schema } from "effect"

export class UserSpaceError extends Schema.TaggedError<UserSpaceError>()("UserSpaceError", {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export type UserSpaceAgent = {
  readonly id: string
  readonly json: string
  readonly updatedAtMs: number
}

export type UserSpaceEvent = {
  readonly seq: number
  readonly eventId: string
  readonly kind: string
  readonly json: string
  readonly createdAtMs: number
}

export type UserSpaceApi = {
  readonly listAgents: () => Effect.Effect<ReadonlyArray<UserSpaceAgent>, UserSpaceError>
  readonly createAgent: (input: { readonly json: unknown }) => Effect.Effect<string, UserSpaceError>
  readonly listEvents: (input: { readonly afterSeq: number }) => Effect.Effect<ReadonlyArray<UserSpaceEvent>, UserSpaceError>
}

export class UserSpaceService extends Context.Tag("@openagents/web/UserSpaceService")<
  UserSpaceService,
  UserSpaceApi
>() {}

const jsonHeaders = { "content-type": "application/json; charset=utf-8" }

export const UserSpaceLive = Layer.succeed(
  UserSpaceService,
  UserSpaceService.of({
    listAgents: Effect.fn("UserSpace.listAgents")(function* () {
      const response = yield* Effect.tryPromise({
        try: () => fetch("/api/user-space/agents", { method: "GET", credentials: "include" }),
        catch: (error) => UserSpaceError.make({ operation: "listAgents.fetch", error }),
      })

      if (!response.ok) {
        return yield* UserSpaceError.make({
          operation: "listAgents.http",
          error: new Error(`HTTP ${response.status}`),
        })
      }

      const json: any = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => UserSpaceError.make({ operation: "listAgents.json", error }),
      })

      return Array.isArray(json?.agents) ? (json.agents as ReadonlyArray<UserSpaceAgent>) : []
    }),

    createAgent: Effect.fn("UserSpace.createAgent")(function* (input: { readonly json: unknown }) {
      const response = yield* Effect.tryPromise({
        try: () =>
          fetch("/api/user-space/agents", {
            method: "POST",
            credentials: "include",
            headers: jsonHeaders,
            body: JSON.stringify({ json: input.json }),
          }),
        catch: (error) => UserSpaceError.make({ operation: "createAgent.fetch", error }),
      })

      const payload: any = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => UserSpaceError.make({ operation: "createAgent.json", error }),
      })

      if (!response.ok) {
        return yield* UserSpaceError.make({
          operation: "createAgent.http",
          error: new Error(String(payload?.error ?? `HTTP ${response.status}`)),
        })
      }

      return String(payload?.agentId ?? "")
    }),

    listEvents: Effect.fn("UserSpace.listEvents")(function* (input: { readonly afterSeq: number }) {
      const url = new URL("/api/user-space/events", window.location.origin)
      url.searchParams.set("after", String(input.afterSeq))

      const response = yield* Effect.tryPromise({
        try: () => fetch(url.toString(), { method: "GET", credentials: "include" }),
        catch: (error) => UserSpaceError.make({ operation: "listEvents.fetch", error }),
      })

      if (!response.ok) {
        return yield* UserSpaceError.make({
          operation: "listEvents.http",
          error: new Error(`HTTP ${response.status}`),
        })
      }

      const json: any = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => UserSpaceError.make({ operation: "listEvents.json", error }),
      })

      return Array.isArray(json?.events) ? (json.events as ReadonlyArray<UserSpaceEvent>) : []
    }),
  }),
)
