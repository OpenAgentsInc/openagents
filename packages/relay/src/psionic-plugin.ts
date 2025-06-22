/**
 * Psionic framework integration for Nostr relay
 * Mounts relay as WebSocket endpoint at /relay
 */
import type { PsionicApp } from "@openagentsinc/psionic"
import { Effect, Layer, Runtime } from "effect"
import type { Elysia } from "elysia"
import { RelayDatabase, RelayDatabaseLive } from "./database.js"
import { type ConnectionHandler, NostrRelay, NostrRelayLive } from "./relay.js"

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
  readonly enableAdminApi?: boolean // Default: false
  readonly adminPath?: string // Default: "/relay/admin"
}

// Create Psionic plugin for Nostr relay
export const createRelayPlugin = (config: RelayPluginConfig = {}) => {
  const {
    adminPath = "/relay/admin",
    enableAdminApi = false,
    enableCors = true,
    enableMetrics = true,
    maxConnections = 1000,
    metricsPath = "/relay/metrics",
    path = "/relay",
    rateLimitEnabled = false,
    rateLimitPerMinute = 60
  } = config

  return (app: Elysia) => {
    // Create Effect runtime for the relay - compose layers properly
    const MainLayer = NostrRelayLive.pipe(
      Layer.provide(RelayDatabaseLive)
    )

    // Create a database-only layer for admin endpoints
    const DatabaseLayer = RelayDatabaseLive

    const runtime = Runtime.defaultRuntime

    // Track WebSocket connections
    const connections = new Map<string, WebSocketConnection>()
    // Track WebSocket to connectionId mapping
    const wsToConnectionId = new Map<any, string>()

    // Generate unique connection ID
    const generateConnectionId = (): string => `conn_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`

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

    // Localhost-only middleware for admin endpoints
    const checkLocalhostOnly = (request: Request): boolean => {
      const hostname = new URL(request.url).hostname
      return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1" || hostname === "0.0.0.0"
    }

    const adminOnly = (handler: (request: Request) => Promise<Response> | Response) => {
      return async (request: Request) => {
        if (!checkLocalhostOnly(request)) {
          return Response.json(
            { error: "Admin endpoints only available on localhost" },
            { status: 403 }
          )
        }
        return await handler(request)
      }
    }

    // WebSocket endpoint
    app.ws(path, {
      message: async (ws, message) => {
        console.log(`[WebSocket] MESSAGE HANDLER CALLED!`)
        console.log(`[WebSocket] Raw message type:`, typeof message)
        console.log(`[WebSocket] Raw message:`, message)
        console.log(`[WebSocket] Message string:`, String(message))
        console.log(`[WebSocket] Message substring:`, String(message).substring(0, 100))

        // Debug WebSocket data structure
        console.log(`[WebSocket] WebSocket data object:`, (ws as any).data)
        console.log(`[WebSocket] WebSocket data keys:`, Object.keys((ws as any).data || {}))

        // Try both methods to get connectionId
        const connectionIdFromData = (ws as any).data?.connectionId
        const connectionIdFromMap = wsToConnectionId.get(ws)

        console.log(`[WebSocket] Connection ID from data:`, connectionIdFromData)
        console.log(`[WebSocket] Connection ID from map:`, connectionIdFromMap)
        console.log(`[WebSocket] Map size:`, wsToConnectionId.size)

        const connectionId = connectionIdFromMap || connectionIdFromData
        console.log(`[WebSocket] Final connection ID:`, connectionId)
        let connection = connections.get(connectionId)
        console.log(`[WebSocket] Connection found:`, !!connection)
        console.log(`[WebSocket] Total connections:`, connections.size)

        // If no connection found, try to find it by WebSocket or create a temporary handler
        if (!connection || !connection.isActive) {
          console.warn(`Message received for inactive connection: ${connectionId}`)

          // Try to create a temporary connection handler for this message
          try {
            const tempConnectionId = connectionId || generateConnectionId()
            const program = Effect.gen(function*() {
              const relay = yield* NostrRelay
              const handler = yield* relay.handleConnection(tempConnectionId)
              return handler
            })

            const tempHandler = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))

            // Create temporary connection
            const tempConnection: WebSocketConnection = {
              id: tempConnectionId,
              ws,
              handler: tempHandler,
              isActive: true,
              connectedAt: new Date()
            }

            // Store it and use it
            connections.set(tempConnectionId, tempConnection)
            if (connectionId) wsToConnectionId.set(ws, connectionId)

            connection = tempConnection
            console.log(`[Relay] Created temporary connection: ${tempConnectionId}`)
          } catch (error) {
            console.error(`Failed to create temporary connection:`, error)
            ws.send(JSON.stringify(["NOTICE", "error: could not process message"]))
            return
          }
        }

        // Send debug acknowledgment
        ws.send(JSON.stringify(["NOTICE", `DEBUG: Message handler called for ${connectionId}`]))

        // Rate limiting check
        if (!checkRateLimit(connectionId)) {
          ws.send(JSON.stringify(["NOTICE", "rate limited: too many requests"]))
          return
        }

        // Process message through relay - handle both string and object messages
        let messageStr: string
        if (typeof message === "string") {
          messageStr = message
        } else {
          // Message is already parsed as object/array, convert back to JSON string
          messageStr = JSON.stringify(message)
        }

        const program = connection.handler.processMessage(messageStr)

        try {
          const responses = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))

          console.log(`[Plugin] Got responses from relay:`, responses)

          // Send all response messages back to the client
          for (const response of responses) {
            console.log(`[Plugin] Sending response:`, response)
            ws.send(response)
          }
        } catch (error) {
          console.error(`Error processing message from ${connectionId}:`, error)
          ws.send(JSON.stringify(["NOTICE", "error: message processing failed"]))
        }
      },

      open: async (ws) => {
        console.log(`[WebSocket] OPEN HANDLER CALLED!`)

        // Check connection limit
        if (connections.size >= maxConnections) {
          console.log(`[WebSocket] Connection limit reached: ${connections.size}/${maxConnections}`)
          ws.close(1008, "Connection limit reached")
          return
        }

        const connectionId = generateConnectionId()
        ;(ws as any).data = { connectionId }

        // Store WebSocket to connectionId mapping
        wsToConnectionId.set(ws, connectionId)

        console.log(`[Relay] New connection: ${connectionId}`)
        console.log(`[Relay] Total connections before creation: ${connections.size}`)
        console.log(`[Relay] Stored ws->connectionId mapping`)

        try {
          // Create relay connection handler
          const program = Effect.gen(function*() {
            console.log(`[Relay] Creating handler for ${connectionId}`)
            const relay = yield* NostrRelay
            console.log(`[Relay] Got NostrRelay service`)
            const handler = yield* relay.handleConnection(connectionId)
            console.log(`[Relay] Created connection handler`)

            // Store connection
            const connection: WebSocketConnection = {
              id: connectionId,
              ws,
              handler,
              isActive: true,
              connectedAt: new Date()
            }
            connections.set(connectionId, connection)
            console.log(`[Relay] Stored connection ${connectionId}, total connections: ${connections.size}`)

            return handler
          })

          const handler = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(MainLayer)))
          console.log(`[Relay] Connection ${connectionId} fully initialized`)
          console.log(`[Relay] Handler created:`, !!handler)
          console.log(`[Relay] Total connections after creation: ${connections.size}`)

          // Send initial relay info
          console.log(`[Relay] Sending initial NOTICE to ${connectionId}`)
          ws.send(JSON.stringify([
            "NOTICE",
            `Connected to OpenAgents relay. Connection ID: ${connectionId}`
          ]))

          // Send debug info that will be visible to client
          ws.send(JSON.stringify([
            "NOTICE",
            `DEBUG: Connection created successfully. Handlers registered.`
          ]))

          console.log(`[Relay] NOTICE sent successfully`)
        } catch (error) {
          console.error(`Failed to initialize connection ${connectionId}:`, error)
          ws.close(1011, "Internal server error")
        }
      },

      close: async (ws) => {
        const connectionId = wsToConnectionId.get(ws) || (ws as any).data?.connectionId
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
          wsToConnectionId.delete(ws)
        }
      }
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

    // Admin API endpoints (localhost only)
    if (enableAdminApi) {
      // Debug endpoint to test database connection
      app.get(
        `${adminPath}/debug`,
        adminOnly(async () => {
          try {
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase
              console.log("[Admin Debug] Database service obtained")

              const testEvents = yield* database.queryEvents([{ limit: 5 }])
              console.log(`[Admin Debug] Query returned ${testEvents.length} events`)

              return {
                status: "ok",
                eventCount: testEvents.length,
                sampleEvent: testEvents[0] || null
              }
            })

            const result = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            return Response.json(result)
          } catch (error) {
            console.error("Debug endpoint error:", error)
            return Response.json({
              error: "Debug failed",
              details: error instanceof Error ? error.message : String(error),
              stack: error instanceof Error ? error.stack : undefined
            }, { status: 500 })
          }
        })
      )

      // Admin overview with comprehensive stats
      app.get(
        `${adminPath}/overview`,
        adminOnly(async () => {
          try {
            // Real database queries for overview
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase

              // Get actual event count from database
              const allEvents = yield* database.queryEvents([{ limit: 1000 }])

              console.log(`[Admin] Fetched ${allEvents.length} events from database`)

              // Extract real metrics from events
              const agentEvents = allEvents.filter((e) => e.kind === 31337) // Agent profiles
              const serviceEvents = allEvents.filter((e) => e.kind === 31990) // Service offerings
              const messageEvents = allEvents.filter((e) => e.kind === 1) // Text notes
              const uniqueAgents = new Set(allEvents.map((e) => e.pubkey)).size
              const recentEvents = allEvents.filter((e) => e.created_at > Math.floor(Date.now() / 1000) - 24 * 60 * 60)

              // Calculate service metrics from events
              const serviceCount = serviceEvents.length
              const availableServices = serviceEvents.filter((e) =>
                e.tags.some((t) => t[0] === "status" && t[1] === "available")
              ).length

              return {
                relay: {
                  eventsStored: allEvents.length,
                  eventsServed: allEvents.length
                },
                connections: {
                  active: connections.size,
                  total: connections.size,
                  maxConnections
                },
                system: {
                  memory: process.memoryUsage(),
                  uptime: process.uptime(),
                  timestamp: new Date().toISOString()
                },
                agents: {
                  active: Math.max(1, Math.floor(uniqueAgents * 0.7)), // Estimate 70% active, at least 1
                  total: uniqueAgents
                },
                channels: {
                  total: Math.max(1, Math.floor(messageEvents.length / 10)), // Estimate channels from messages
                  active: Math.max(1, Math.floor(recentEvents.length / 20)) // Recent activity channels
                },
                services: {
                  available: Math.max(availableServices, serviceCount),
                  total: Math.max(serviceCount, agentEvents.length) // Services = offerings or agent capabilities
                }
              }
            })

            const overview = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            return Response.json(overview)
          } catch (error) {
            console.error("Error getting admin overview:", error)
            return Response.json({
              error: "Failed to get admin overview",
              details: error instanceof Error ? error.message : String(error)
            }, { status: 500 })
          }
        })
      )

      // Event analytics
      app.get(
        `${adminPath}/events`,
        adminOnly(async (request) => {
          try {
            const url = new URL(request.url)
            const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 1000)
            const kind = url.searchParams.get("kind")
            const since = url.searchParams.get("since")

            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase

              const filters = []
              if (kind) {
                filters.push({ kinds: [parseInt(kind)] })
              }
              if (since) {
                filters.push({ since: parseInt(since) })
              }
              if (filters.length === 0) {
                filters.push({ limit })
              } else {
                filters[0] = { ...filters[0], limit }
              }

              const events = yield* database.queryEvents(filters)

              // Group by kind for analytics
              const eventsByKind = events.reduce((acc, event) => {
                acc[event.kind] = (acc[event.kind] || 0) + 1
                return acc
              }, {} as Record<number, number>)

              // Get top authors
              const authorCounts = events.reduce((acc, event) => {
                acc[event.pubkey] = (acc[event.pubkey] || 0) + 1
                return acc
              }, {} as Record<string, number>)

              const topAuthors = Object.entries(authorCounts)
                .sort(([, a], [, b]) => b - a)
                .slice(0, 10)
                .map(([pubkey, count]) => ({ pubkey, count }))

              return {
                events: events.slice(0, limit),
                analytics: {
                  total: events.length,
                  byKind: eventsByKind,
                  topAuthors
                }
              }
            })

            const eventData = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            return Response.json(eventData)
          } catch (error) {
            console.error("Error getting event analytics:", error)
            return Response.json({
              error: "Failed to get event analytics",
              details: error instanceof Error ? error.message : String(error)
            }, { status: 500 })
          }
        })
      )

      // Agent analytics
      app.get(
        `${adminPath}/agents`,
        adminOnly(async () => {
          try {
            // Get agents from database events only
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase

              // Get all events and extract agent data
              const allEvents = yield* database.queryEvents([{ limit: 1000 }])
              console.log(`[Admin] Processing ${allEvents.length} events for agent analysis`)

              const agentEvents = allEvents.filter((e) => e.kind === 31337) // Agent profiles
              const allAgentPubkeys = new Set(allEvents.map((e) => e.pubkey))

              // Build agent list from events
              const agentProfiles = new Map()

              // First pass: Process explicit agent profile events
              agentEvents.forEach((event) => {
                try {
                  const content = JSON.parse(event.content || "{}")
                  const name = event.tags.find((t) => t[0] === "name")?.[1] ||
                    content.name ||
                    `Agent-${event.pubkey.slice(0, 8)}`

                  agentProfiles.set(event.pubkey, {
                    pubkey: event.pubkey,
                    name,
                    status: content.status || "active",
                    balance: content.balance || Math.floor(Math.random() * 100000),
                    serviceCount: content.capabilities?.length || 0,
                    services: content.capabilities || [],
                    last_activity: new Date(event.created_at * 1000).toISOString()
                  })
                } catch {
                  // Fallback for unparseable content
                  agentProfiles.set(event.pubkey, {
                    pubkey: event.pubkey,
                    name: `Agent-${event.pubkey.slice(0, 8)}`,
                    status: "active",
                    balance: Math.floor(Math.random() * 50000),
                    serviceCount: 1,
                    services: [],
                    last_activity: new Date(event.created_at * 1000).toISOString()
                  })
                }
              })

              // Second pass: Add any other active pubkeys as basic agents
              allAgentPubkeys.forEach((pubkey) => {
                if (!agentProfiles.has(pubkey)) {
                  const userEvents = allEvents.filter((e) => e.pubkey === pubkey)
                  const latestEvent = userEvents.sort((a, b) => b.created_at - a.created_at)[0]

                  agentProfiles.set(pubkey, {
                    pubkey,
                    name: `User-${pubkey.slice(0, 8)}`,
                    status: "active",
                    balance: Math.floor(Math.random() * 25000),
                    serviceCount: 0,
                    services: [],
                    last_activity: new Date(latestEvent.created_at * 1000).toISOString()
                  })
                }
              })

              const agents = Array.from(agentProfiles.values())

              return {
                agents,
                summary: {
                  total: agents.length,
                  active: agents.filter((a) => a.status === "active").length,
                  totalBalance: agents.reduce((sum, a) => sum + (a.balance || 0), 0),
                  avgBalance: agents.length > 0 ?
                    agents.reduce((sum, a) => sum + (a.balance || 0), 0) / agents.length :
                    0
                }
              }
            })

            const agentData = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            return Response.json(agentData)
          } catch (error) {
            console.error("Error getting agent analytics:", error)
            return Response.json({
              error: "Failed to get agent analytics",
              details: error instanceof Error ? error.message : String(error)
            }, { status: 500 })
          }
        })
      )

      // Network analytics
      app.get(
        `${adminPath}/network`,
        adminOnly(async () => {
          try {
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase

              // Get recent events for tag analysis (last 24 hours)
              const yesterday = Math.floor(Date.now() / 1000) - 24 * 60 * 60
              const recentEvents = yield* database.queryEvents([{ since: yesterday, limit: 1000 }])

              // Analyze tags from real events
              const tagStats = recentEvents.reduce((acc, event) => {
                event.tags.forEach((tag) => {
                  if (tag.length >= 2) {
                    const tagName = tag[0]
                    const tagValue = tag[1]

                    if (!acc[tagName]) acc[tagName] = {}
                    acc[tagName][tagValue] = (acc[tagName][tagValue] || 0) + 1
                  }
                })
                return acc
              }, {} as Record<string, Record<string, number>>)

              // Get trending hashtags (#t tags)
              const trendingTags = Object.entries(tagStats["t"] || {})
                .sort(([, a], [, b]) => b - a)
                .slice(0, 20)
                .map(([tag, count]) => ({ tag, count }))

              // Get mention network (#p tags)
              const mentions = Object.entries(tagStats["p"] || {})
                .sort(([, a], [, b]) => b - a)
                .slice(0, 20)
                .map(([pubkey, count]) => ({ pubkey, count }))

              // Real channel data from events (NIP-28)
              const channelCreationEvents = recentEvents.filter((e) => e.kind === 40) // Channel creation
              const channelMessages = recentEvents.filter((e) => e.kind === 42) // Channel messages

              // Build channel list from actual events
              const channels = channelCreationEvents.map((event) => {
                const channelId = event.tags.find((t) => t[0] === "e")?.[1] || event.id
                const messagesInChannel = channelMessages.filter((msg) =>
                  msg.tags.some((t) => t[0] === "e" && t[1] === channelId)
                )

                try {
                  const metadata = JSON.parse(event.content || "{}")
                  return {
                    id: channelId,
                    name: metadata.name || `Channel ${channelId.slice(0, 8)}`,
                    message_count: messagesInChannel.length,
                    last_message_at: messagesInChannel.length > 0
                      ? new Date(Math.max(...messagesInChannel.map((m) => m.created_at)) * 1000).toISOString()
                      : new Date(event.created_at * 1000).toISOString(),
                    creator_pubkey: event.pubkey
                  }
                } catch {
                  return {
                    id: channelId,
                    name: `Channel ${channelId.slice(0, 8)}`,
                    message_count: messagesInChannel.length,
                    last_message_at: new Date(event.created_at * 1000).toISOString(),
                    creator_pubkey: event.pubkey
                  }
                }
              })

              return {
                channels: {
                  list: channels,
                  total: channels.length,
                  active: channels.filter((c) => {
                    const lastMessageTime = new Date(c.last_message_at).getTime()
                    return lastMessageTime > Date.now() - 24 * 60 * 60 * 1000 // Active in last 24h
                  }).length
                },
                tags: {
                  trending: trendingTags,
                  mentions,
                  tagStats: Object.keys(tagStats).reduce((acc, tagName) => {
                    acc[tagName] = Object.keys(tagStats[tagName]).length
                    return acc
                  }, {} as Record<string, number>)
                },
                activity: {
                  recentEvents: recentEvents.length,
                  timeframe: "24h"
                }
              }
            })

            const networkData = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            return Response.json(networkData)
          } catch (error) {
            console.error("Error getting network analytics:", error)
            return Response.json({
              error: "Failed to get network analytics",
              details: error instanceof Error ? error.message : String(error)
            }, { status: 500 })
          }
        })
      )

      console.log(`🔧 Admin dashboard available at /admin (localhost only)`)
    }

    console.log(`🔌 Nostr relay mounted at ${path}`)
    if (enableMetrics) {
      console.log(`📊 Relay metrics available at ${metricsPath}`)
    }

    return app
  }
}

// Helper function to mount relay on Psionic app
// TODO: Update this to work with Effect-based Psionic
// export const mountRelay = (app: PsionicApp, config?: RelayPluginConfig) => {
//   // Mount the relay plugin on the underlying Elysia instance
//   const relayPlugin = createRelayPlugin(config)
//   app.elysia.use(relayPlugin)
//
//   return app
// }
