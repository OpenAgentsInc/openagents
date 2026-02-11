import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { makeLndRestTransportLayer } from "../src/adapters/lndRestTransport.js"
import { LndTransportService } from "../src/services/lndTransportService.js"

const sendWithLayer = (layer: ReturnType<typeof makeLndRestTransportLayer>) =>
  Effect.gen(function* () {
    const transport = yield* LndTransportService
    return yield* transport.send({
      method: "GET",
      path: "/v1/getinfo",
    })
  }).pipe(Effect.provide(layer))

describe("lnd rest transport", () => {
  it.effect("returns typed authentication errors for 401/403 responses", () =>
    Effect.gen(function* () {
      const unauthorized = yield* Effect.either(
        sendWithLayer(
          makeLndRestTransportLayer({
            endpoint: "https://lnd.example",
            fetchImplementation: async () =>
              new Response(JSON.stringify({ error: "unauthorized" }), {
                status: 401,
                headers: { "content-type": "application/json" },
              }),
          }),
        ),
      )

      expect(unauthorized._tag).toBe("Left")
      if (unauthorized._tag === "Left") {
        expect(unauthorized.left._tag).toBe("LndAuthenticationError")
        if (unauthorized.left._tag === "LndAuthenticationError") {
          expect(unauthorized.left.status).toBe(401)
        }
      }

      const forbidden = yield* Effect.either(
        sendWithLayer(
          makeLndRestTransportLayer({
            endpoint: "https://lnd.example",
            fetchImplementation: async () =>
              new Response(JSON.stringify({ error: "forbidden" }), {
                status: 403,
                headers: { "content-type": "application/json" },
              }),
          }),
        ),
      )

      expect(forbidden._tag).toBe("Left")
      if (forbidden._tag === "Left") {
        expect(forbidden.left._tag).toBe("LndAuthenticationError")
        if (forbidden.left._tag === "LndAuthenticationError") {
          expect(forbidden.left.status).toBe(403)
        }
      }
    }),
  )

  it.effect("returns typed decode errors for malformed response body", () =>
    Effect.gen(function* () {
      const result = yield* Effect.either(
        sendWithLayer(
          makeLndRestTransportLayer({
            endpoint: "https://lnd.example",
            fetchImplementation: async () =>
              new Response("not-json", {
                status: 200,
                headers: { "content-type": "application/json" },
              }),
          }),
        ),
      )

      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("LndResponseDecodeError")
      }
    }),
  )

  it.effect("sends requests and decodes valid responses", () =>
    Effect.gen(function* () {
      let receivedMacaroon = ""
      let receivedUrl = ""
      let receivedMethod = ""

      const layer = makeLndRestTransportLayer({
        endpoint: "https://lnd.example",
        macaroonHex: "deadbeef",
        fetchImplementation: async (input, init) => {
          receivedUrl = String(input)
          receivedMethod = String(init?.method)
          receivedMacaroon =
            init?.headers instanceof Headers
              ? init.headers.get("Grpc-Metadata-macaroon") ?? ""
              : ""

          return new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "content-type": "application/json", "x-test": "1" },
          })
        },
      })

      const response = yield* sendWithLayer(layer)
      expect(response.status).toBe(200)
      expect(response.body).toMatchObject({ ok: true })
      expect(response.headers).toMatchObject({ "content-type": "application/json", "x-test": "1" })
      expect(receivedMethod).toBe("GET")
      expect(receivedUrl).toContain("/v1/getinfo")
      expect(receivedMacaroon).toBe("deadbeef")
    }),
  )
})
