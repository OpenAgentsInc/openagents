/**
 * Psionic framework integration for Nostr relay
 * Mounts relay as WebSocket endpoint at /relay
 */
import { Elysia } from "elysia"
import { Effect, Layer, Runtime } from "effect"
import type { PsionicApp } from "@openagentsinc/psionic"
import { NostrRelay, NostrRelayLive, type ConnectionHandler } from "./relay.js"
import { RelayDatabaseLive } from "./database.js"

// WebSocket connection tracking
interface WebSocketConnection {
  readonly id: string
  readonly ws: any // WebSocket type from Elysia
  readonly handler: ConnectionHandler
  readonly isActive: boolean
  readonly connectedAt: Date
}

// Plugin configuration
export interface RelayPluginConfig {
  readonly path?: string // Default: "/relay"
  readonly maxConnections?: number // Default: 1000
  readonly enableCors?: boolean // Default: true
  readonly rateLimitEnabled?: boolean // Default: false (agent-friendly)
  readonly rateLimitPerMinute?: number // Default: 60
  readonly enableMetrics?: boolean // Default: true
  readonly metricsPath?: string // Default: "/relay/metrics"
}

// Create Psionic plugin for Nostr relay
export const createRelayPlugin = (config: RelayPluginConfig = {}) => {
  const {
    path = "/relay",
    maxConnections = 1000,
    enableCors = true,
    rateLimitEnabled = false,
    rateLimitPerMinute = 60,
    enableMetrics = true,
    metricsPath = "/relay/metrics"
  } = config
  
  return (app: Elysia) => {
    // Create Effect runtime for the relay - compose layers properly
    const MainLayer = NostrRelayLive.pipe(
      Layer.provide(RelayDatabaseLive)
    )
    
    const runtime = Runtime.defaultRuntime
    
    // Track WebSocket connections
    const connections = new Map<string, WebSocketConnection>()
    
    // Generate unique connection ID
    const generateConnectionId = (): string => 
      `conn_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
    
    // Rate limiting state
    const rateLimitMap = new Map<string, { count: number; resetTime: number }>()
    
    const checkRateLimit = (clientId: string): boolean => {
      if (!rateLimitEnabled) return true
      
      const now = Date.now()
      const windowMs = 60 * 1000 // 1 minute
      
      const current = rateLimitMap.get(clientId)
      if (!current || now > current.resetTime) {
        rateLimitMap.set(clientId, { count: 1, resetTime: now + windowMs })
        return true
      }
      
      if (current.count >= rateLimitPerMinute) {
        return false
      }
      
      current.count++
      return true
    }
    
    // WebSocket endpoint
    app.ws(path, {
      message: async (ws, message) => {
        const connectionId = (ws as any).data?.connectionId
        const connection = connections.get(connectionId)
        
        if (!connection || !connection.isActive) {
          console.warn(`Message received for inactive connection: ${connectionId}`)
          return
        }
        
        // Rate limiting check
        if (!checkRateLimit(connectionId)) {
          ws.send(JSON.stringify(["NOTICE", "rate limited: too many requests"]))
          return
        }
        
        // Process message through relay
        const program = connection.handler.processMessage(String(message))
        
        try {
          await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))
        } catch (error) {
          console.error(`Error processing message from ${connectionId}:`, error)
          ws.send(JSON.stringify(["NOTICE", "error: message processing failed"]))
        }
      },
      
      open: async (ws) => {
        // Check connection limit
        if (connections.size >= maxConnections) {
          ws.close(1008, "Connection limit reached")
          return
        }
        
        const connectionId = generateConnectionId()
        ;(ws as any).data = { connectionId }
        
        console.log(`[Relay] New connection: ${connectionId}`)
        
        try {
          // Create relay connection handler
          const program = Effect.gen(function*() {
            const relay = yield* NostrRelay
            const handler = yield* relay.handleConnection(connectionId)
            
            // Store connection
            const connection: WebSocketConnection = {
              id: connectionId,
              ws,
              handler,
              isActive: true,
              connectedAt: new Date()
            }
            connections.set(connectionId, connection)
            
            return handler
          })
          
          await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))
          
          // Send initial relay info
          ws.send(JSON.stringify([
            "NOTICE", 
            `Connected to OpenAgents relay. Connection ID: ${connectionId}`
          ]))
          
        } catch (error) {
          console.error(`Failed to initialize connection ${connectionId}:`, error)
          ws.close(1011, "Internal server error")
        }
      },
      
      close: async (ws) => {
        const connectionId = (ws as any).data?.connectionId
        const connection = connections.get(connectionId)
        
        if (connection) {
          console.log(`[Relay] Connection closed: ${connectionId}`)
          
          // Clean up connection
          try {
            const program = connection.handler.close()
            await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))
          } catch (error) {
            console.error(`Error closing connection ${connectionId}:`, error)
          }
          
          connections.delete(connectionId)
        }
      },
      
    })
    
    // CORS headers for WebSocket upgrade
    if (enableCors) {
      app.options(path, ({ set }) => {
        set.headers = {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type, Authorization, Upgrade, Connection",
          "Access-Control-Allow-Credentials": "true"
        }
        return new Response(null, { status: 204 })
      })
    }
    
    // Metrics endpoint
    if (enableMetrics) {
      app.get(metricsPath, async () => {
        try {
          const program = Effect.gen(function*() {
            const relay = yield* NostrRelay
            const stats = yield* relay.getStats()
            
            return {
              stats,
              connections: {
                active: connections.size,
                total: connections.size
              },
              memory: process.memoryUsage(),
              uptime: process.uptime(),
              timestamp: new Date().toISOString()
            }
          })
          
          const metrics = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))
          
          return Response.json(metrics)
        } catch (error) {
          console.error("Error getting metrics:", error)
          return Response.json({ error: "Failed to get metrics" }, { status: 500 })
        }
      })
    }
    
    // Health check endpoint
    app.get(`${path}/health`, () => ({
      status: "healthy",
      timestamp: new Date().toISOString(),
      connections: connections.size,
      uptime: process.uptime()
    }))
    
    // Relay info endpoint (NIP-11)
    app.get(`${path}/info`, () => ({
      name: "OpenAgents Relay",
      description: "Nostr relay for autonomous agent coordination",
      pubkey: "", // Could be relay operator pubkey
      contact: "https://github.com/OpenAgentsInc/openagents",
      supported_nips: [1, 28, 90], // NIP-01 basic, NIP-28 channels, NIP-90 marketplace
      software: "https://github.com/OpenAgentsInc/openagents/tree/main/packages/relay",
      version: "1.0.0",
      limitation: {
        max_message_length: 16384,
        max_subscriptions: 20,
        max_filters: 100,
        max_limit: 5000,
        max_subid_length: 64,
        min_pow_difficulty: 0,
        auth_required: false,
        payment_required: false
      },
      fees: {
        admission: [],
        subscription: [],
        publication: []
      }
    }))
    
    console.log(`🔌 Nostr relay mounted at ${path}`)
    if (enableMetrics) {
      console.log(`📊 Relay metrics available at ${metricsPath}`)
    }
    
    return app
  }
}

// Helper function to mount relay on Psionic app
export const mountRelay = (app: PsionicApp, config?: RelayPluginConfig) => {
  // Mount the relay plugin on the underlying Elysia instance
  const relayPlugin = createRelayPlugin(config)
  app.elysia.use(relayPlugin)
  
  return app
}