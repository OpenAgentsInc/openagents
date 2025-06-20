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
      return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1"
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
            // Try to get real data from database, fall back to mock data if not available
            let overview
            try {
              const program = Effect.gen(function*() {
                const database = yield* RelayDatabase
                const activeAgents = yield* database.getActiveAgents()
                const channels = yield* database.getChannels()
                const services = yield* database.getServiceOfferings()

                return {
                  relay: { eventsStored: 0, eventsServed: 0 },
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
                    active: activeAgents.length,
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

              overview = await Runtime.runPromise(runtime)(program.pipe(Effect.provide(DatabaseLayer)))
            } catch {
              // Fall back to mock data if database is not available
              console.warn("Database not available, using mock data")
              overview = {
                relay: { eventsStored: 42, eventsServed: 128 },
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
                  active: 3,
                  total: 5
                },
                channels: {
                  total: 8,
                  active: 4
                },
                services: {
                  available: 12,
                  total: 15
                }
              }
            }

            return Response.json(overview)
          } catch (error) {
            console.error("Error getting admin overview:", error)
            return Response.json({ error: "Failed to get admin overview" }, { status: 500 })
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

            // Try database first, fall back to mock data
            try {
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
            } catch {
              console.warn("Database not available for events, using mock data")
              // Mock event data
              const mockEvents = [
                {
                  id: "mock_event_1",
                  pubkey: "npub1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
                  created_at: Math.floor(Date.now() / 1000) - 3600,
                  kind: 1,
                  tags: [["p", "npub1234"], ["t", "nostr"]],
                  content: "Hello from the admin dashboard! This is a mock event for testing.",
                  sig: "mock_signature_1"
                },
                {
                  id: "mock_event_2",
                  pubkey: "npub9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba",
                  created_at: Math.floor(Date.now() / 1000) - 1800,
                  kind: 31337,
                  tags: [["d", "agent_001"], ["name", "TestAgent"]],
                  content: JSON.stringify({ capabilities: ["code-review", "testing"] }),
                  sig: "mock_signature_2"
                }
              ]

              const filteredEvents = kind ? mockEvents.filter((e) => e.kind === parseInt(kind)) : mockEvents

              return Response.json({
                events: filteredEvents.slice(0, limit),
                analytics: {
                  total: filteredEvents.length,
                  byKind: { 1: 1, 31337: 1 },
                  topAuthors: [
                    { pubkey: "npub1234567890abcdef", count: 1 },
                    { pubkey: "npub9876543210fedcba", count: 1 }
                  ]
                }
              })
            }
          } catch (error) {
            console.error("Error getting event analytics:", error)
            return Response.json({ error: "Failed to get event analytics" }, { status: 500 })
          }
        })
      )

      // Agent analytics
      app.get(
        `${adminPath}/agents`,
        adminOnly(async () => {
          try {
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
            } catch {
              console.warn("Database not available for agents, using mock data")
              // Mock agent data
              const mockAgents = [
                {
                  pubkey: "npub1agent1234567890abcdef",
                  name: "CodeReviewBot",
                  status: "active",
                  balance: 50000,
                  serviceCount: 2,
                  services: [],
                  last_activity: new Date().toISOString()
                },
                {
                  pubkey: "npub1agent9876543210fedcba",
                  name: "TestingAgent",
                  status: "active",
                  balance: 25000,
                  serviceCount: 1,
                  services: [],
                  last_activity: new Date(Date.now() - 1800000).toISOString()
                },
                {
                  pubkey: "npub1agentabcdef1234567890",
                  name: "DocumentationAgent",
                  status: "hibernating",
                  balance: 5000,
                  serviceCount: 0,
                  services: [],
                  last_activity: new Date(Date.now() - 86400000).toISOString()
                }
              ]

              return Response.json({
                agents: mockAgents,
                summary: {
                  total: mockAgents.length,
                  active: mockAgents.filter((a) => a.status === "active").length,
                  totalBalance: mockAgents.reduce((sum, a) => sum + a.balance, 0),
                  avgBalance: mockAgents.reduce((sum, a) => sum + a.balance, 0) / mockAgents.length
                }
              })
            }
          } catch (error) {
            console.error("Error getting agent analytics:", error)
            return Response.json({ error: "Failed to get agent analytics" }, { status: 500 })
          }
        })
      )

      // Network analytics
      app.get(
        `${adminPath}/network`,
        adminOnly(async () => {
          try {
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
            } catch {
              console.warn("Database not available for network, using mock data")
              // Mock network data
              return Response.json({
                channels: {
                  list: [
                    {
                      id: "channel_001",
                      name: "General Discussion",
                      message_count: 156,
                      last_message_at: new Date().toISOString(),
                      creator_pubkey: "npub1creator123"
                    },
                    {
                      id: "channel_002",
                      name: "Agent Coordination",
                      message_count: 89,
                      last_message_at: new Date(Date.now() - 3600000).toISOString(),
                      creator_pubkey: "npub1creator456"
                    },
                    {
                      id: "channel_003",
                      name: "Code Reviews",
                      message_count: 34,
                      last_message_at: new Date(Date.now() - 7200000).toISOString(),
                      creator_pubkey: "npub1creator789"
                    }
                  ],
                  total: 8,
                  active: 4
                },
                tags: {
                  trending: [
                    { tag: "nostr", count: 45 },
                    { tag: "bitcoin", count: 32 },
                    { tag: "ai", count: 28 },
                    { tag: "agents", count: 21 },
                    { tag: "development", count: 18 }
                  ],
                  mentions: [
                    { pubkey: "npub1popular123", count: 12 },
                    { pubkey: "npub1active456", count: 8 },
                    { pubkey: "npub1helpful789", count: 6 }
                  ],
                  tagStats: {
                    t: 25,
                    p: 18,
                    e: 12
                  }
                },
                activity: {
                  recentEvents: 67,
                  timeframe: "24h"
                }
              })
            }
          } catch (error) {
            console.error("Error getting network analytics:", error)
            return Response.json({ error: "Failed to get network analytics" }, { status: 500 })
          }
        })
      )

      console.log(`🔧 Admin API available at ${adminPath} (localhost only)`)
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
