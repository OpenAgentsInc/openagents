import type { HttpServerRequest } from "@effect/platform"
import { HttpServerResponse } from "@effect/platform"
import * as Nostr from "@openagentsinc/nostr"
import type { RouteContext } from "@openagentsinc/psionic"
import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import { Effect, Layer } from "effect"

/**
 * POST /api/channels/create - Create a new channel
 */
export function createChannel(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text.pipe(Effect.orDie)
    const body = JSON.parse(bodyText) as { name: string; about?: string; picture?: string }
    const program = Effect.gen(function*() {
      const crypto = yield* Nostr.CryptoService.CryptoService
      const nip28 = yield* Nostr.Nip28Service.Nip28Service

      // Generate keypair for the channel creator
      const privateKey = yield* crypto.generatePrivateKey()
      const publicKey = yield* crypto.getPublicKey(privateKey)

      // Create channel using NIP-28 service - build params conditionally
      const createParams: any = {
        name: body.name,
        privateKey,
        relays: ["ws://localhost:3003/relay"]
      }

      if (body.about) {
        createParams.about = body.about
      }
      if (body.picture) {
        createParams.picture = body.picture
      }

      const channelEvent = yield* nip28.createChannel(createParams)

      return {
        channelId: channelEvent.id,
        publicKey,
        event: channelEvent
      }
    })

    // Build service layers in dependency order
    const baseLayer = Layer.merge(
      Nostr.WebSocketService.WebSocketServiceLive,
      Nostr.CryptoService.CryptoServiceLive
    )

    const serviceLayer = Layer.merge(
      Nostr.EventService.EventServiceLive,
      Nostr.RelayService.RelayServiceLive
    )

    const fullLayer = Layer.provideMerge(
      Layer.provideMerge(Nostr.Nip28Service.Nip28ServiceLive, serviceLayer),
      baseLayer
    )

    const result = yield* program.pipe(Effect.provide(fullLayer))

    return yield* HttpServerResponse.json(result).pipe(Effect.orDie)
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to create channel - Full error:", error)
      console.error("Error stack:", error instanceof Error ? error.stack : "No stack")
      return HttpServerResponse.json(
        {
          error: "Failed to create channel",
          details: error instanceof Error ? error.message : String(error)
        },
        { status: 500 }
      ).pipe(Effect.orDie)
    })
  )
}

/**
 * POST /api/channels/message - Send a message to a channel
 */
export function sendChannelMessage(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text.pipe(Effect.orDie)
    const body = JSON.parse(bodyText) as {
      channelId: string
      content: string
      replyTo?: string
      privateKey?: string
    }
    const program = Effect.gen(function*() {
      const crypto = yield* Nostr.CryptoService.CryptoService
      const nip28 = yield* Nostr.Nip28Service.Nip28Service

      // Use provided private key or generate new one
      let privateKey: Nostr.Schema.PrivateKey
      if (body.privateKey) {
        privateKey = body.privateKey as Nostr.Schema.PrivateKey
      } else {
        privateKey = yield* crypto.generatePrivateKey()
      }

      // Send message using NIP-28 service - build params conditionally
      const params: any = {
        channelId: body.channelId as Nostr.Schema.EventId,
        content: body.content,
        privateKey,
        relayHint: "ws://localhost:3003/relay"
      }

      if (body.replyTo) {
        params.replyToEventId = body.replyTo as Nostr.Schema.EventId
      }

      const messageEvent = yield* nip28.sendChannelMessage(params)

      return {
        messageId: messageEvent.id,
        event: messageEvent
      }
    })

    // Build service layers in dependency order
    const baseLayer = Layer.merge(
      Nostr.WebSocketService.WebSocketServiceLive,
      Nostr.CryptoService.CryptoServiceLive
    )

    const serviceLayer = Layer.merge(
      Nostr.EventService.EventServiceLive,
      Nostr.RelayService.RelayServiceLive
    )

    const fullLayer = Layer.provideMerge(
      Layer.provideMerge(Nostr.Nip28Service.Nip28ServiceLive, serviceLayer),
      baseLayer
    )

    const result = yield* program.pipe(Effect.provide(fullLayer))

    return yield* HttpServerResponse.json(result).pipe(Effect.orDie)
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to send message:", error)
      return HttpServerResponse.json(
        { error: "Failed to send message" },
        { status: 500 }
      ).pipe(Effect.orDie)
    })
  )
}

/**
 * GET /api/channels/list - List all channels
 */
export function listChannels(_ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, never> {
  return Effect.gen(function*() {
    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get channels from database
      const channels = yield* database.getChannels()

      return { channels }
    })

    // Create database layer
    const DatabaseLayer = RelayDatabaseLive

    const result = yield* program.pipe(Effect.provide(DatabaseLayer))

    return yield* HttpServerResponse.json(result).pipe(Effect.orDie)
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to list channels:", error)
      return HttpServerResponse.json(
        { error: "Failed to list channels" },
        { status: 500 }
      ).pipe(Effect.orDie)
    })
  )
}

/**
 * GET /api/channels/:id - Get channel details and recent messages
 */
export function getChannel(ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, never> {
  return Effect.gen(function*() {
    const { id } = ctx.params
    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase

      // Get channel from database
      const channels = yield* database.getChannels()
      const channel = channels.find((c) => c.id === id)

      if (!channel) {
        return { error: "Channel not found" }
      }

      // Get recent messages
      const messages = yield* database.queryEvents([{
        kinds: [42],
        "#e": [id as Nostr.Schema.EventId],
        limit: 100
      }])

      return { channel, messages }
    })

    // Create database layer
    const DatabaseLayer = RelayDatabaseLive

    const result = yield* program.pipe(Effect.provide(DatabaseLayer))

    if (result.error) {
      return yield* HttpServerResponse.json(result, { status: 404 }).pipe(Effect.orDie)
    }

    return yield* HttpServerResponse.json(result).pipe(Effect.orDie)
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to get channel:", error)
      return HttpServerResponse.json(
        { error: "Failed to get channel" },
        { status: 500 }
      ).pipe(Effect.orDie)
    })
  )
}
