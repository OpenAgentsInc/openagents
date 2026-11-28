// Firm reason: 1 (Effect v4 HttpRouter + SSE SessionHub + HtmlServerHttp SSR entry for docs/effect/half-day-sprint.md)
/// <reference types="@effect/ui/html/HtmlServerViteDocument" />
import { layerHttp } from "@effect/ui/html/HtmlServerHttp"
import { Duration, Effect, Layer, Schedule, Schema, Stream } from "effect"
import { HttpRouter, HttpServer, HttpServerRequest, HttpServerResponse } from "effect/http"
import { document } from "virtual:@effect/ui/document"
import { App } from "../client/App.ts"
import { SessionCommandsNoop } from "../client/commands/SessionCommands.ts"
import { SessionStream } from "../client/queries/SessionStream.ts"
import {
  ApproveRequest,
  StartTurnRequest,
  SteerRequest,
  StopRequest
} from "../shared/api.ts"
import { AppRoute } from "../shared/routes.ts"
import { SessionHub, SessionHubError } from "./SessionHub.ts"

const encoder = new TextEncoder()

const encodeSseFrame = (event: string, id: number, data: unknown): Uint8Array =>
  encoder.encode(`event: ${event}\nid: ${id}\ndata: ${JSON.stringify(data)}\n\n`)

const ssePing = encoder.encode(": ping\n\n")

const jsonError = (status: number, error: string) =>
  HttpServerResponse.json({ error }, { status }).pipe(
    Effect.orElseSucceed(() => HttpServerResponse.text(error, { status }))
  )

const readSessionId = Effect.gen(function*() {
  const params = yield* HttpRouter.params
  const id = (params["id"] ?? "").trim()
  if (id.length === 0 || !/^[a-zA-Z0-9._-]{1,64}$/.test(id)) {
    return yield* Effect.fail(new SessionHubError("Invalid session id"))
  }
  return id
})

const decodeJsonBody = <S extends Schema.Top>(
  schema: S,
  req: HttpServerRequest.HttpServerRequest
): Effect.Effect<S["Type"], SessionHubError> =>
  req.json.pipe(
    Effect.mapError(() => new SessionHubError("Request body must be valid JSON")),
    Effect.flatMap((raw) =>
      Schema.decodeUnknownEffect(schema as unknown as Schema.Codec<S["Type"]>)(raw).pipe(
        Effect.mapError((err) => new SessionHubError(`Invalid payload: ${String(err)}`))
      )
    )
  )

const ApiRoutes = HttpRouter.addAll(
  Effect.gen(function*() {
    const hub = yield* SessionHub

    return [
      HttpRouter.route(
        "GET",
        "/api/health",
        Effect.gen(function*() {
          const data = yield* hub.health()
          return yield* HttpServerResponse.json(data, { status: data.ok ? 200 : 503 }).pipe(
            Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
          )
        })
      ),

      HttpRouter.route(
        "GET",
        "/api/sessions/:id/snapshot",
        Effect.gen(function*() {
          const id = yield* readSessionId
          const snap = yield* hub.getSnapshot(id)
          return yield* HttpServerResponse.json(snap).pipe(
            Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
          )
        }).pipe(
          Effect.catch((e) => jsonError(500, e instanceof SessionHubError ? e.message : String(e)))
        )
      ),

      HttpRouter.route(
        "GET",
        "/api/sessions/:id/stream",
        Effect.gen(function*() {
          const id = yield* readSessionId
          const snapshots = yield* hub.subscribe(id)
          const dataFrames = snapshots.pipe(
            Stream.map((snap) => encodeSseFrame("snapshot", snap.seq, snap))
          )
          const pings = Stream.fromSchedule(Schedule.spaced(Duration.seconds(15))).pipe(
            Stream.map(() => ssePing)
          )
          const body = Stream.merge(dataFrames, pings)
          return HttpServerResponse.stream(body, {
            status: 200,
            headers: {
              "content-type": "text/event-stream; charset=utf-8",
              "cache-control": "no-cache, no-transform",
              "connection": "keep-alive",
              "x-accel-buffering": "no"
            }
          })
        }).pipe(
          Effect.catch((e) => jsonError(500, e instanceof SessionHubError ? e.message : String(e)))
        )
      ),

      HttpRouter.route(
        "POST",
        "/api/sessions/:id/turn",
        (req) =>
          Effect.gen(function*() {
            const id = yield* readSessionId
            const payload = yield* decodeJsonBody(StartTurnRequest, req)
            const snap = yield* hub.startTurn(id, payload)
            return yield* HttpServerResponse.json(snap, { status: 202 }).pipe(
              Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
            )
          }).pipe(
            Effect.catch((e) =>
              jsonError(400, e instanceof SessionHubError ? e.message : String(e))
            )
          )
      ),

      HttpRouter.route(
        "POST",
        "/api/sessions/:id/approve",
        (req) =>
          Effect.gen(function*() {
            const id = yield* readSessionId
            const payload = yield* decodeJsonBody(ApproveRequest, req)
            const snap = yield* hub.approve(id, payload)
            return yield* HttpServerResponse.json(snap).pipe(
              Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
            )
          }).pipe(
            Effect.catch((e) =>
              jsonError(400, e instanceof SessionHubError ? e.message : String(e))
            )
          )
      ),

      HttpRouter.route(
        "POST",
        "/api/sessions/:id/steer",
        (req) =>
          Effect.gen(function*() {
            const id = yield* readSessionId
            const payload = yield* decodeJsonBody(SteerRequest, req)
            const snap = yield* hub.steer(id, payload)
            return yield* HttpServerResponse.json(snap).pipe(
              Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
            )
          }).pipe(
            Effect.catch((e) =>
              jsonError(400, e instanceof SessionHubError ? e.message : String(e))
            )
          )
      ),

      HttpRouter.route(
        "POST",
        "/api/sessions/:id/stop",
        (req) =>
          Effect.gen(function*() {
            const id = yield* readSessionId
            const payload = yield* decodeJsonBody(StopRequest, req).pipe(
              Effect.orElseSucceed(() => ({} as StopRequest))
            )
            const snap = yield* hub.stop(id, payload)
            return yield* HttpServerResponse.json(snap).pipe(
              Effect.orElseSucceed(() => HttpServerResponse.text("encode error", { status: 500 }))
            )
          }).pipe(
            Effect.catch((e) =>
              jsonError(400, e instanceof SessionHubError ? e.message : String(e))
            )
          )
      )
    ]
  })
)

export default Layer.mergeAll(
  ApiRoutes,
  layerHttp(App, {
    routes: AppRoute,
    document
  }).pipe(Layer.provide([SessionStream.layerSsr, SessionCommandsNoop]))
).pipe(Layer.provide([SessionHub.layer, HttpServer.layerServices]))

export const prerender: ReadonlyArray<string> = []
