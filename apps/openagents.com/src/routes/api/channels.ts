import * as Nostr from "@openagentsinc/nostr"
import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import { Effect, Layer, Runtime } from "effect"

// Create a runtime with database layer
const runtime = Runtime.defaultRuntime

export const channelsApi = (app: any) => {
  const prefix = "/api/channels"

  // Create a new channel
  app.post(`${prefix}/create`, async (context: any) => {
    try {
      const body = await context.request.json() as { name: string; about?: string; picture?: string }
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

      const result = await Effect.runPromise(
        program.pipe(Effect.provide(fullLayer))
      )

      return result
    } catch (error) {
      console.error("Failed to create channel - Full error:", error)
      console.error("Error stack:", error instanceof Error ? error.stack : "No stack")
      return new Response(
        JSON.stringify({
          error: "Failed to create channel",
          details: error instanceof Error ? error.message : String(error)
        }),
        {
          status: 500,
          headers: { "Content-Type": "application/json" }
        }
      )
    }
  })

  // Send a message to a channel
  app.post(
    `${prefix}/message`,
    async (context: any) => {
      try {
        const body = await context.request.json() as { channelId: string; content: string; replyTo?: string; privateKey?: string }
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

        const result = await Effect.runPromise(
          program.pipe(Effect.provide(fullLayer))
        )

        return result
      } catch (error) {
        console.error("Failed to send message:", error)
        return new Response(JSON.stringify({ error: "Failed to send message" }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        })
      }
    }
  )

  // List all channels
  app.get(`${prefix}/list`, async () => {
    try {
      const program = Effect.gen(function*() {
        const database = yield* RelayDatabase

        // Get channels from database
        const channels = yield* database.getChannels()

        return { channels }
      })

      // Create database layer
      const DatabaseLayer = RelayDatabaseLive

      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(DatabaseLayer))
      )

      return result
    } catch (error) {
      console.error("Failed to list channels:", error)
      return new Response(JSON.stringify({ error: "Failed to list channels" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      })
    }
  })

  // Get channel details and recent messages
  app.get(`${prefix}/:id`, async (context: any) => {
    try {
      const { id } = context.params
      const program = Effect.gen(function*() {
        const database = yield* RelayDatabase

        // Get channel from database
        const channels = yield* database.getChannels()
        const channel = channels.find((c) => c.id === params.id)

        if (!channel) {
          return { error: "Channel not found" }
        }

        // Get recent messages
        const messages = yield* database.queryEvents([{
          kinds: [42],
          "#e": [params.id as Nostr.Schema.EventId],
          limit: 100
        }])

        return { channel, messages }
      })

      // Create database layer
      const DatabaseLayer = RelayDatabaseLive

      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(DatabaseLayer))
      )

      if (result.error) {
        return new Response(JSON.stringify(result), {
          status: 404,
          headers: { "Content-Type": "application/json" }
        })
      }

      return result
    } catch (error) {
      console.error("Failed to get channel:", error)
      return new Response(JSON.stringify({ error: "Failed to get channel" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      })
    }
  })
}
