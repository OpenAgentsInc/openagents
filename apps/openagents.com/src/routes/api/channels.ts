import { Elysia } from "elysia"
import { Effect, Runtime } from "effect"
import * as Nostr from "@openagentsinc/nostr"
import { RelayDatabase } from "@openagentsinc/relay"
import { DATABASE_HOST, DATABASE_NAME, DATABASE_PASSWORD, DATABASE_USERNAME } from "@openagentsinc/relay/database"

// Create a runtime with database layer
const runtime = Runtime.defaultRuntime

// Nostr service layer
const NostrServiceLayer = Nostr.Client.layer({
  relays: ["ws://localhost:3003/relay"],
  autoConnect: true
})

export const channelsApi = new Elysia({ prefix: "/api/channels" })
  // Create a new channel
  .post("/create", async ({ body }: { body: { name: string; about?: string; picture?: string } }) => {
    try {
      const program = Effect.gen(function*() {
        const nostr = yield* Nostr.Client.Client
        const nip28 = yield* Nostr.Nip28.Nip28Service
        
        // Generate keys for this session (in production, use actual user keys)
        const { privateKey, publicKey } = yield* Nostr.Crypto.generateKeyPair()
        
        // Create channel
        const channelEvent = yield* nip28.createChannel({
          name: body.name,
          about: body.about || "",
          picture: body.picture || "",
          relays: ["ws://localhost:3003/relay"]
        })
        
        // Sign and publish event
        const signedEvent = yield* Nostr.Event.signEvent(channelEvent, privateKey)
        yield* nostr.publish(signedEvent)
        
        return { channelId: signedEvent.id }
      })
      
      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(NostrServiceLayer))
      )
      
      return result
    } catch (error) {
      console.error("Failed to create channel:", error)
      return new Response(JSON.stringify({ error: "Failed to create channel" }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      })
    }
  })
  
  // Send a message to a channel
  .post("/message", async ({ body }: { body: { channelId: string; content: string; replyTo?: string } }) => {
    try {
      const program = Effect.gen(function*() {
        const nostr = yield* Nostr.Client.Client
        const nip28 = yield* Nostr.Nip28.Nip28Service
        
        // Generate keys for this session (in production, use actual user keys)
        const { privateKey, publicKey } = yield* Nostr.Crypto.generateKeyPair()
        
        // Create message event
        const messageEvent = yield* nip28.sendChannelMessage(
          body.channelId,
          body.content,
          body.replyTo
        )
        
        // Sign and publish event
        const signedEvent = yield* Nostr.Event.signEvent(messageEvent, privateKey)
        yield* nostr.publish(signedEvent)
        
        return { messageId: signedEvent.id }
      })
      
      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(NostrServiceLayer))
      )
      
      return result
    } catch (error) {
      console.error("Failed to send message:", error)
      return new Response(JSON.stringify({ error: "Failed to send message" }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      })
    }
  })
  
  // List all channels
  .get("/list", async () => {
    try {
      const program = Effect.gen(function*() {
        const database = yield* RelayDatabase
        
        // Get channels from database
        const channels = yield* database.getChannels()
        
        return { channels }
      })
      
      // Create database layer
      const DatabaseLayer = RelayDatabase.layer({
        host: DATABASE_HOST,
        username: DATABASE_USERNAME,
        password: DATABASE_PASSWORD,
        database: DATABASE_NAME
      })
      
      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(DatabaseLayer))
      )
      
      return result
    } catch (error) {
      console.error("Failed to list channels:", error)
      return new Response(JSON.stringify({ error: "Failed to list channels" }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      })
    }
  })
  
  // Get channel details and recent messages
  .get("/:id", async ({ params }: { params: { id: string } }) => {
    try {
      const program = Effect.gen(function*() {
        const database = yield* RelayDatabase
        
        // Get channel from database
        const channels = yield* database.getChannels()
        const channel = channels.find(c => c.id === params.id)
        
        if (!channel) {
          return { error: "Channel not found" }
        }
        
        // Get recent messages
        const messages = yield* database.queryEvents([{
          kinds: [42],
          "#e": [params.id],
          limit: 100
        }])
        
        return { channel, messages }
      })
      
      // Create database layer
      const DatabaseLayer = RelayDatabase.layer({
        host: DATABASE_HOST,
        username: DATABASE_USERNAME,
        password: DATABASE_PASSWORD,
        database: DATABASE_NAME
      })
      
      const result = await Runtime.runPromise(runtime)(
        program.pipe(Effect.provide(DatabaseLayer))
      )
      
      if (result.error) {
        return new Response(JSON.stringify(result), {
          status: 404,
          headers: { 'Content-Type': 'application/json' }
        })
      }
      
      return result
    } catch (error) {
      console.error("Failed to get channel:", error)
      return new Response(JSON.stringify({ error: "Failed to get channel" }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      })
    }
  })