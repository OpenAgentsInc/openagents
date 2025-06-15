/**
 * Simplified Nostr relay for compilation
 * @module
 */

import { HttpRouter, HttpServerResponse } from "@effect/platform"
import { Context, Layer } from "effect"

// Simple HTTP route handler for now
const routes = HttpRouter.empty.pipe(
  HttpRouter.get(
    "/",
    HttpServerResponse.json({
      name: "pylon-relay",
      description: "Nostr relay server",
      supported_nips: [1],
      software: "https://github.com/OpenAgentsInc/openagents",
      version: "0.0.1"
    })
  )
)

/**
 * Nostr relay context
 */
export class NostrRelayContext extends Context.Tag("NostrRelayContext")<
  NostrRelayContext,
  typeof routes
>() {}

/**
 * Nostr relay server layer
 */
export const NostrRelayLive = Layer.succeed(NostrRelayContext, routes)
