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
      // Admin overview with comprehensive stats
      app.get(
        `${adminPath}/overview`,
        adminOnly(async () => {
          try {
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase
              const activeAgents = yield* database.getActiveAgents()
              const channels = yield* database.getChannels()
              const services = yield* database.getServiceOfferings()

              // Get actual event count from database
              const allEvents = yield* database.queryEvents([{ limit: 50000 }])

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
                  active: activeAgents.filter((a) => a.status === "active").length,
                  total: activeAgents.length
                },
                channels: {
                  total: channels.length,
                  active: channels.filter((c) =>
                    c.last_message_at &&
                    new Date(c.last_message_at).getTime() > Date.now() - 24 * 60 * 60 * 1000
                  ).length
                },
                services: {
                  available: services.filter((s) => s.availability === "available").length,
                  total: services.length
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
            const program = Effect.gen(function*() {
              const database = yield* RelayDatabase

              const agents = yield* database.getActiveAgents()
              const services = yield* database.getServiceOfferings()

              // Group services by agent
              const servicesByAgent = services.reduce((acc, service) => {
                const key = service.agent_pubkey
                if (!acc[key]) acc[key] = []
                acc[key].push(service)
                return acc
              }, {} as Record<string, Array<any>>)

              const agentAnalytics = agents.map((agent) => ({
                ...agent,
                services: servicesByAgent[agent.pubkey] || [],
                serviceCount: (servicesByAgent[agent.pubkey] || []).length
              }))

              return {
                agents: agentAnalytics,
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

              const channels = yield* database.getChannels()

              // Get recent events for tag analysis (last 24 hours)
              const yesterday = Math.floor(Date.now() / 1000) - 24 * 60 * 60
              const recentEvents = yield* database.queryEvents([{ since: yesterday, limit: 1000 }])

              // Analyze tags
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

              return {
                channels: {
                  list: channels,
                  total: channels.length,
                  active: channels.filter((c) =>
                    c.last_message_at &&
                    new Date(c.last_message_at).getTime() > Date.now() - 24 * 60 * 60 * 1000
                  ).length
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
export const mountRelay = (app: PsionicApp, config?: RelayPluginConfig) => {
  // Mount the relay plugin on the underlying Elysia instance
  const relayPlugin = createRelayPlugin(config)
  app.elysia.use(relayPlugin)

  return app
}
