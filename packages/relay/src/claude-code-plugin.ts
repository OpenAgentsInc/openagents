/**
 * Psionic framework integration for Claude Code WebSocket server
 * Mounts Claude Code control endpoints at /claude-code
 */
import { Effect, Runtime, Stream } from "effect"
import type { Elysia } from "elysia"
import { 
  ClaudeCodeWebSocketServer, 
  ClaudeCodeWebSocketServerLive,
  type ClientConnectionHandler,
  type MachineMessage,
  type MachineConnectionHandler
} from "./claude-code-server.js"
import { Schema as S } from "@effect/schema"

// Types will be imported from overlord once it's built
interface ClaudeCodeCommand {
  readonly commandId: string
  readonly type: "start_session" | "send_prompt" | "end_session" | "get_status" | "switch_project"
  readonly machineId: string
  readonly sessionId?: string
  readonly userId: string
  readonly timestamp: Date
  readonly data: Record<string, any>
}

interface ClaudeCodeResponse {
  readonly type: string
  readonly commandId: string
  readonly sessionId: string
  readonly machineId: string
  readonly timestamp: Date
  readonly data: any
}

// WebSocket connection tracking
interface ConnectionInfo {
  readonly id: string
  readonly type: "machine" | "client"
  readonly ws: any // WebSocket from Elysia
  readonly handler?: ClientConnectionHandler | MachineConnectionHandler
  readonly userId?: string
  readonly machineId?: string
  readonly connectedAt: Date
}

// Plugin configuration
export interface ClaudeCodePluginConfig {
  readonly path?: string // Default: "/claude-code"
  readonly authRequired?: boolean // Default: true
  readonly enableMetrics?: boolean // Default: true
  readonly metricsPath?: string // Default: "/claude-code/metrics"
  readonly maxMachines?: number // Default: 100
  readonly maxClients?: number // Default: 1000
  readonly heartbeatInterval?: number // Default: 30000 (30 seconds)
}

// Create Psionic plugin for Claude Code WebSocket server
export const createClaudeCodePlugin = (config: ClaudeCodePluginConfig = {}) => {
  const {
    path = "/claude-code",
    authRequired = true,
    enableMetrics = true,
    metricsPath = "/claude-code/metrics",
    maxMachines = 100,
    maxClients = 1000,
    heartbeatInterval = 30000
  } = config

  return (app: Elysia) => {
    // Create Effect runtime with Claude Code server layer
    const MainLayer = ClaudeCodeWebSocketServerLive
    const runtime = Effect.runSync(
      Runtime.runtime<ClaudeCodeWebSocketServer>().pipe(
        Effect.provide(MainLayer)
      )
    )

    // Track connections
    const connections = new Map<string, ConnectionInfo>()
    const wsToConnectionId = new Map<any, string>()

    // Generate unique connection ID
    const generateConnectionId = (): string => 
      `conn_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`

    // Helper to authenticate connection
    const authenticateConnection = (request: Request): { userId?: string; machineId?: string } => {
      const auth = request.headers.get("Authorization")
      if (!auth) return {}
      
      // Parse auth header
      // Format: "Bearer user:${userId}" or "Bearer machine:${machineId}:${apiKey}"
      const [type, ...parts] = auth.replace("Bearer ", "").split(":")
      
      if (type === "user") {
        return { userId: parts[0] }
      } else if (type === "machine") {
        return { machineId: parts[0] }
      }
      
      return {}
    }

    // Machine WebSocket endpoint
    app.ws(`${path}/machine`, {

      message: async (ws: any, message: any) => {
        const connectionId = wsToConnectionId.get(ws)
        if (!connectionId) {
          console.error("Message received from unknown connection")
          ws.send(JSON.stringify({ type: "error", error: "Connection not initialized" }))
          return
        }

        const connection = connections.get(connectionId)
        if (!connection || connection.type !== "machine") {
          console.error("Invalid machine connection")
          return
        }

        try {
          // Parse machine message
          const parsed = (typeof message === "string" ? JSON.parse(message) : message) as MachineMessage

          const program = Effect.gen(function*() {
            const server = yield* ClaudeCodeWebSocketServer

            switch (parsed.type) {
              case "register":
                // Machine registration includes full info
                yield* server.registerMachine(
                  connectionId,
                  parsed.machineId,
                  {
                    machineId: parsed.machineId,
                    hostname: parsed.machineInfo.hostname,
                    claudeVersion: parsed.machineInfo.claudeVersion,
                    sdkVersion: parsed.machineInfo.sdkVersion,
                    supportedFeatures: parsed.machineInfo.supportedFeatures,
                    activeProjects: parsed.machineInfo.activeProjects,
                    activeSessions: [],
                    lastHeartbeat: new Date(),
                    status: "online"
                  }
                )
                
                // Update connection info
                connections.set(connectionId, {
                  ...connection,
                  machineId: parsed.machineId
                })
                
                ws.send(JSON.stringify({ type: "registered", machineId: parsed.machineId }))
                break

              case "heartbeat":
                yield* server.updateMachineHeartbeat(parsed.machineId, parsed.sessions)
                break

              case "response":
                yield* server.handleMachineResponse(
                  parsed.machineId,
                  parsed.response as ClaudeCodeResponse
                )
                break

              case "error":
                // Log machine error
                yield* Effect.logError(`Machine ${parsed.machineId} error: ${parsed.error}`)
                break
            }
          })

          await runtime.runPromise(program)
        } catch (error) {
          console.error("Error processing machine message:", error)
          ws.send(JSON.stringify({ 
            type: "error", 
            error: error instanceof Error ? error.message : "Unknown error" 
          }))
        }
      },

      open: async (ws: any) => {
        const connectionId = generateConnectionId()
        wsToConnectionId.set(ws, connectionId)

        // Check machine limit
        const machineCount = Array.from(connections.values())
          .filter(c => c.type === "machine").length
        if (machineCount >= maxMachines) {
          console.log(`Machine limit reached: ${machineCount}/${maxMachines}`)
          ws.close(1008, "Machine limit reached")
          return
        }

        const connection: ConnectionInfo = {
          id: connectionId,
          type: "machine",
          ws,
          connectedAt: new Date()
        }
        connections.set(connectionId, connection)

        console.log(`[Claude Code] Machine connection opened: ${connectionId}`)
        
        // Request registration
        ws.send(JSON.stringify({ type: "register_request" }))
      },

      close: async (ws: any) => {
        const connectionId = wsToConnectionId.get(ws)
        if (!connectionId) return

        try {
          const program = Effect.gen(function*() {
            const server = yield* ClaudeCodeWebSocketServer
            yield* server.removeConnection(connectionId)
          })

          await runtime.runPromise(program)
        } catch (error) {
          console.error("Error closing machine connection:", error)
        }

        connections.delete(connectionId)
        wsToConnectionId.delete(ws)
        console.log(`[Claude Code] Machine connection closed: ${connectionId}`)
      }
    })

    // Client WebSocket endpoint
    app.ws(`${path}/client`, {

      message: async (ws: any, message: any) => {
        const connectionId = wsToConnectionId.get(ws)
        if (!connectionId) {
          console.error("Message received from unknown client")
          ws.send(JSON.stringify({ type: "error", error: "Connection not initialized" }))
          return
        }

        const connection = connections.get(connectionId)
        if (!connection || connection.type !== "client" || !connection.handler) {
          console.error("Invalid client connection")
          return
        }

        try {
          const messageStr = typeof message === "string" ? message : JSON.stringify(message)
          const handler = connection.handler as ClientConnectionHandler
          
          const responses = await Runtime.runPromise(runtime)(
            handler.processMessage(messageStr).pipe(Effect.provide(MainLayer))
          )

          // Send responses back to client
          for (const response of responses) {
            ws.send(response)
          }
        } catch (error) {
          console.error("Error processing client message:", error)
          ws.send(JSON.stringify({ 
            type: "error", 
            error: error instanceof Error ? error.message : "Unknown error" 
          }))
        }
      },

      open: async (ws: any, request: any) => {
        const connectionId = generateConnectionId()
        wsToConnectionId.set(ws, connectionId)

        // Check client limit
        const clientCount = Array.from(connections.values())
          .filter(c => c.type === "client").length
        if (clientCount >= maxClients) {
          console.log(`Client limit reached: ${clientCount}/${maxClients}`)
          ws.close(1008, "Client limit reached")
          return
        }

        // Get user ID from auth
        const { userId = "anonymous" } = authenticateConnection(request as any)

        try {
          const program = Effect.gen(function*() {
            const server = yield* ClaudeCodeWebSocketServer
            const handler = yield* server.registerClient(connectionId, userId)

            const connection: ConnectionInfo = {
              id: connectionId,
              type: "client",
              ws,
              handler,
              userId,
              connectedAt: new Date()
            }
            connections.set(connectionId, connection)

            console.log(`[Claude Code] Client connection opened: ${connectionId} (user: ${userId})`)

            // Start streaming responses to client
            const responseStream = handler.getResponseStream()
            yield* Stream.runForEach(responseStream, (message) =>
              Effect.sync(() => {
                ws.send(JSON.stringify(message))
              })
            ).pipe(Effect.fork)

            // Send initial machine list
            const machines = yield* server.getActiveMachines()
            ws.send(JSON.stringify({ type: "machines", machines }))
          })

          await runtime.runPromise(program)
        } catch (error) {
          console.error("Error initializing client connection:", error)
          ws.close(1011, "Internal server error")
        }
      },

      close: async (ws: any) => {
        const connectionId = wsToConnectionId.get(ws)
        if (!connectionId) return

        const connection = connections.get(connectionId)
        if (connection?.handler && connection.type === "client") {
          try {
            const handler = connection.handler as ClientConnectionHandler
            await Runtime.runPromise(runtime)(
              handler.close().pipe(Effect.provide(MainLayer))
            )
          } catch (error) {
            console.error("Error closing client connection:", error)
          }
        }

        connections.delete(connectionId)
        wsToConnectionId.delete(ws)
        console.log(`[Claude Code] Client connection closed: ${connectionId}`)
      }
    })

    // REST API for sending commands (alternative to WebSocket)
    app.post(`${path}/command`, async ({ request }) => {
      try {
        const { userId } = authenticateConnection(request)
        if (authRequired && !userId) {
          return new Response(JSON.stringify({ error: "Unauthorized" }), {
            status: 401,
            headers: { "Content-Type": "application/json" }
          })
        }

        const body = await request.json() as ClaudeCodeCommand

        const program = Effect.gen(function*() {
          const server = yield* ClaudeCodeWebSocketServer
          yield* server.routeCommand(body)
          return { success: true, commandId: body.commandId }
        })

        const result = await Runtime.runPromise(runtime)(
          program.pipe(
            Effect.provide(MainLayer),
            Effect.catchTag("MachineNotFoundError", (error) =>
              Effect.succeed({ 
                success: false, 
                error: `Machine not found: ${error.machineId}` 
              })
            )
          )
        )

        return Response.json(result)
      } catch (error) {
        console.error("Error processing command:", error)
        return Response.json({ 
          success: false, 
          error: error instanceof Error ? error.message : "Unknown error" 
        }, { status: 500 })
      }
    })

    // Query endpoints
    app.get(`${path}/machines`, async ({ request }) => {
      try {
        const program = Effect.gen(function*() {
          const server = yield* ClaudeCodeWebSocketServer
          return yield* server.getActiveMachines()
        })

        const machines = await Runtime.runPromise(runtime)(
          program.pipe(Effect.provide(MainLayer))
        )

        return Response.json({ machines })
      } catch (error) {
        console.error("Error getting machines:", error)
        return Response.json({ error: "Failed to get machines" }, { status: 500 })
      }
    })

    app.get(`${path}/machines/:machineId/sessions`, async ({ params }) => {
      try {
        const program = Effect.gen(function*() {
          const server = yield* ClaudeCodeWebSocketServer
          return yield* server.getMachineSessions(params.machineId)
        })

        const sessions = await Runtime.runPromise(runtime)(
          program.pipe(Effect.provide(MainLayer))
        )

        return Response.json({ sessions })
      } catch (error) {
        console.error("Error getting sessions:", error)
        return Response.json({ error: "Failed to get sessions" }, { status: 500 })
      }
    })

    // Metrics endpoint
    if (enableMetrics) {
      app.get(metricsPath, async () => {
        try {
          const program = Effect.gen(function*() {
            const server = yield* ClaudeCodeWebSocketServer
            const stats = yield* server.getStats()

            return {
              ...stats,
              connections: {
                machines: Array.from(connections.values())
                  .filter(c => c.type === "machine").length,
                clients: Array.from(connections.values())
                  .filter(c => c.type === "client").length,
                total: connections.size
              },
              memory: process.memoryUsage(),
              uptime: process.uptime(),
              timestamp: new Date().toISOString()
            }
          })

          const metrics = await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(MainLayer))
          )

          return Response.json(metrics)
        } catch (error) {
          console.error("Error getting metrics:", error)
          return Response.json({ error: "Failed to get metrics" }, { status: 500 })
        }
      })
    }

    // Health check
    app.get(`${path}/health`, () => ({
      status: "healthy",
      service: "claude-code-websocket",
      timestamp: new Date().toISOString(),
      connections: {
        machines: Array.from(connections.values())
          .filter(c => c.type === "machine").length,
        clients: Array.from(connections.values())
          .filter(c => c.type === "client").length
      }
    }))

    console.log(`🤖 Claude Code WebSocket server mounted at ${path}`)
    console.log(`   Machine endpoint: ws://localhost:PORT${path}/machine`)
    console.log(`   Client endpoint: ws://localhost:PORT${path}/client`)
    if (enableMetrics) {
      console.log(`📊 Metrics available at ${metricsPath}`)
    }

    return app
  }
}