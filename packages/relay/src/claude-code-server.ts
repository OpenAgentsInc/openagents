/**
 * Claude Code WebSocket server implementation
 * Handles machine registration, command routing, and real-time communication
 */
import { Schema as S } from "@effect/schema"
import { Context, Data, Effect, HashMap, Layer, Option, Queue, Ref, Stream } from "effect"
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

interface ClaudeCodeSession {
  readonly sessionId: string
  readonly machineId: string
  readonly userId: string
  readonly projectPath: string
  readonly projectName: string
  readonly status: "active" | "idle" | "ended"
  readonly claudeVersion: string
  readonly startedAt: Date
  readonly endedAt?: Date
  readonly lastPromptAt?: Date
  readonly lastResponseAt?: Date
  readonly messageCount: number
  readonly totalTokens: number
}

interface MachineClaudeInfo {
  readonly machineId: string
  readonly hostname: string
  readonly claudeVersion: string
  readonly sdkVersion: string
  readonly supportedFeatures: ReadonlyArray<string>
  readonly activeProjects: ReadonlyArray<string>
  readonly activeSessions: ReadonlyArray<ClaudeCodeSession>
  readonly lastHeartbeat: Date
  readonly status: "online" | "offline" | "busy"
}

// Machine connection state
interface MachineConnection {
  readonly id: string
  readonly connectionId: string
  readonly machineInfo: MachineClaudeInfo
  readonly activeSessions: HashMap.HashMap<string, ClaudeCodeSession>
  readonly isActive: boolean
  readonly connectedAt: Date
  readonly lastHeartbeat: Date
}

// Web client connection state
interface ClientConnection {
  readonly id: string
  readonly userId: string
  readonly subscribedMachines: Set<string>
  readonly isActive: boolean
  readonly connectedAt: Date
}

// Error types
export class ClaudeCodeServerError extends Data.TaggedError("ClaudeCodeServerError")<{
  message: string
  machineId?: string
  sessionId?: string
}> {}

export class MachineNotFoundError extends Data.TaggedError("MachineNotFoundError")<{
  machineId: string
}> {}

export class SessionNotFoundError extends Data.TaggedError("SessionNotFoundError")<{
  sessionId: string
}> {}

// Message types for WebSocket communication
export const MachineMessage = S.Union(
  S.Struct({
    type: S.Literal("register"),
    machineId: S.String,
    machineInfo: S.Struct({
      hostname: S.String,
      claudeVersion: S.String,
      sdkVersion: S.String,
      supportedFeatures: S.Array(S.String),
      activeProjects: S.Array(S.String)
    })
  }),
  S.Struct({
    type: S.Literal("heartbeat"),
    machineId: S.String,
    sessions: S.Array(S.Struct({
      sessionId: S.String,
      status: S.String,
      messageCount: S.Number
    }))
  }),
  S.Struct({
    type: S.Literal("response"),
    machineId: S.String,
    response: S.Unknown // ClaudeCodeResponse
  }),
  S.Struct({
    type: S.Literal("error"),
    machineId: S.String,
    error: S.String,
    sessionId: S.optional(S.String)
  })
)

export type MachineMessage = S.Schema.Type<typeof MachineMessage>

export const ClientMessage = S.Union(
  S.Struct({
    type: S.Literal("subscribe"),
    machineIds: S.Array(S.String)
  }),
  S.Struct({
    type: S.Literal("command"),
    command: S.Unknown // ClaudeCodeCommand
  }),
  S.Struct({
    type: S.Literal("query"),
    query: S.Union(
      S.Struct({ type: S.Literal("machines") }),
      S.Struct({ type: S.Literal("sessions"), machineId: S.String })
    )
  })
)

export type ClientMessage = S.Schema.Type<typeof ClientMessage>

// Server response types
export const ServerMessage = S.Union(
  S.Struct({
    type: S.Literal("machines"),
    machines: S.Array(S.Unknown) // MachineClaudeInfo[]
  }),
  S.Struct({
    type: S.Literal("sessions"),
    sessions: S.Array(S.Unknown) // ClaudeCodeSession[]
  }),
  S.Struct({
    type: S.Literal("response"),
    response: S.Unknown // ClaudeCodeResponse
  }),
  S.Struct({
    type: S.Literal("error"),
    error: S.String
  }),
  S.Struct({
    type: S.Literal("machine_status"),
    machineId: S.String,
    status: S.Union(S.Literal("online"), S.Literal("offline"))
  })
)

export type ServerMessage = S.Schema.Type<typeof ServerMessage>

// Service interface
export class ClaudeCodeWebSocketServer extends Context.Tag("ClaudeCodeWebSocketServer")<
  ClaudeCodeWebSocketServer,
  {
    // Machine connection methods
    readonly registerMachine: (
      connectionId: string,
      machineId: string,
      machineInfo: MachineClaudeInfo
    ) => Effect.Effect<void, ClaudeCodeServerError>

    readonly updateMachineHeartbeat: (
      machineId: string,
      sessions: ReadonlyArray<ClaudeCodeSession>
    ) => Effect.Effect<void, MachineNotFoundError>

    readonly handleMachineResponse: (
      machineId: string,
      response: ClaudeCodeResponse
    ) => Effect.Effect<void, ClaudeCodeServerError>

    // Client connection methods
    readonly registerClient: (
      connectionId: string,
      userId: string
    ) => Effect.Effect<ClientConnectionHandler, ClaudeCodeServerError>

    // Command routing
    readonly routeCommand: (
      command: ClaudeCodeCommand
    ) => Effect.Effect<void, MachineNotFoundError | ClaudeCodeServerError>

    // Query methods
    readonly getActiveMachines: () => Effect.Effect<ReadonlyArray<MachineClaudeInfo>>
    readonly getMachineSessions: (machineId: string) => Effect.Effect<ReadonlyArray<ClaudeCodeSession>>
    readonly getStats: () => Effect.Effect<ClaudeCodeServerStats>

    // Connection cleanup
    readonly removeConnection: (connectionId: string) => Effect.Effect<void>
  }
>() {}

// Client connection handler
export interface ClientConnectionHandler {
  readonly processMessage: (rawMessage: string) => Effect.Effect<Array<string>, ClaudeCodeServerError>
  readonly close: () => Effect.Effect<void>
  readonly subscribeToMachines: (machineIds: ReadonlyArray<string>) => Effect.Effect<void>
  readonly getResponseStream: () => Stream.Stream<ServerMessage>
}

// Machine connection handler
export interface MachineConnectionHandler {
  readonly processMessage: (rawMessage: string) => Effect.Effect<void, ClaudeCodeServerError>
  readonly close: () => Effect.Effect<void>
  readonly getCommandStream: () => Stream.Stream<ClaudeCodeCommand>
}

export interface ClaudeCodeServerStats {
  readonly totalMachines: number
  readonly activeMachines: number
  readonly totalSessions: number
  readonly activeSessions: number
  readonly totalClients: number
  readonly activeClients: number
  readonly commandsRouted: number
  readonly responsesDelivered: number
}

// Implementation
export const ClaudeCodeWebSocketServerLive = Layer.effect(
  ClaudeCodeWebSocketServer,
  Effect.gen(function*() {
    // State management
    const machines = yield* Ref.make(HashMap.empty<string, MachineConnection>())
    const clients = yield* Ref.make(HashMap.empty<string, ClientConnection>())
    const connectionToMachine = yield* Ref.make(HashMap.empty<string, string>())
    const connectionToClient = yield* Ref.make(HashMap.empty<string, string>())

    // Response queues for clients
    const clientQueues = yield* Ref.make(HashMap.empty<string, Queue.Queue<ServerMessage>>())

    // Command queues for machines
    const machineQueues = yield* Ref.make(HashMap.empty<string, Queue.Queue<ClaudeCodeCommand>>())

    // Statistics
    const stats = yield* Ref.make({
      commandsRouted: 0,
      responsesDelivered: 0
    })

    // Helper to broadcast to subscribed clients
    const broadcastToClients = (
      message: ServerMessage,
      targetMachineId?: string
    ) =>
      Effect.gen(function*() {
        const allClients = yield* Ref.get(clients)
        const queues = yield* Ref.get(clientQueues)

        let notifiedCount = 0

        for (const [clientId, client] of allClients) {
          if (!client.isActive) continue

          // Check if client is subscribed to this machine
          if (targetMachineId && !client.subscribedMachines.has(targetMachineId)) continue

          const queue = HashMap.get(queues, clientId)
          if (Option.isSome(queue)) {
            yield* Queue.offer(queue.value, message)
            notifiedCount++
          }
        }

        yield* Ref.update(stats, (s) => ({
          ...s,
          responsesDelivered: s.responsesDelivered + notifiedCount
        }))

        return notifiedCount
      })

    // Machine registration
    const registerMachine = (
      connectionId: string,
      machineId: string,
      machineInfo: MachineClaudeInfo
    ) =>
      Effect.gen(function*() {
        const connection: MachineConnection = {
          id: machineId,
          connectionId,
          machineInfo,
          activeSessions: HashMap.empty(),
          isActive: true,
          connectedAt: new Date(),
          lastHeartbeat: new Date()
        }

        yield* Ref.update(machines, HashMap.set(machineId, connection))
        yield* Ref.update(connectionToMachine, HashMap.set(connectionId, machineId))

        // Create command queue for this machine
        const queue = yield* Queue.unbounded<ClaudeCodeCommand>()
        yield* Ref.update(machineQueues, HashMap.set(machineId, queue))

        // Notify all clients about new machine
        yield* broadcastToClients({
          type: "machine_status",
          machineId,
          status: "online"
        })

        yield* Effect.log(`Machine registered: ${machineId} (${machineInfo.hostname})`)
      })

    // Heartbeat update
    const updateMachineHeartbeat = (
      machineId: string,
      sessions: ReadonlyArray<ClaudeCodeSession>
    ) =>
      Effect.gen(function*() {
        const allMachines = yield* Ref.get(machines)
        const machine = HashMap.get(allMachines, machineId)

        return yield* machine.pipe(
          Option.match({
            onNone: () => Effect.fail(new MachineNotFoundError({ machineId })),
            onSome: (machineValue) => {
              const updatedMachine = {
                ...machineValue,
                lastHeartbeat: new Date(),
                activeSessions: sessions.reduce(
                  (acc, session) => HashMap.set(acc, session.sessionId, session),
                  HashMap.empty<string, ClaudeCodeSession>()
                )
              }

              return Ref.update(machines, HashMap.set(machineId, updatedMachine))
            }
          })
        )
      })

    // Handle response from machine
    const handleMachineResponse = (
      machineId: string,
      response: ClaudeCodeResponse
    ) =>
      Effect.gen(function*() {
        // Broadcast response to all subscribed clients
        yield* broadcastToClients({
          type: "response",
          response
        }, machineId)
      })

    // Client registration
    const registerClient = (
      connectionId: string,
      userId: string
    ) =>
      Effect.gen(function*() {
        const client: ClientConnection = {
          id: connectionId,
          userId,
          subscribedMachines: new Set(),
          isActive: true,
          connectedAt: new Date()
        }

        yield* Ref.update(clients, HashMap.set(connectionId, client))
        yield* Ref.update(connectionToClient, HashMap.set(connectionId, connectionId))

        // Create response queue for this client
        const queue = yield* Queue.unbounded<ServerMessage>()
        yield* Ref.update(clientQueues, HashMap.set(connectionId, queue))

        // Create client handler
        const handler: ClientConnectionHandler = {
          processMessage: (rawMessage: string) =>
            Effect.gen(function*() {
              const parsed = yield* S.decodeUnknown(ClientMessage)(JSON.parse(rawMessage))
              const responses: Array<string> = []

              switch (parsed.type) {
                case "subscribe":
                  yield* handler.subscribeToMachines(parsed.machineIds)
                  responses.push(JSON.stringify({ type: "subscribed", machineIds: parsed.machineIds }))
                  break

                case "command":
                  yield* routeCommand(parsed.command as ClaudeCodeCommand)
                  responses.push(JSON.stringify({ type: "command_sent" }))
                  break

                case "query":
                  if (parsed.query.type === "machines") {
                    const machines = yield* getActiveMachines()
                    responses.push(JSON.stringify({ type: "machines", machines }))
                  } else {
                    const sessions = yield* getMachineSessions(parsed.query.machineId)
                    responses.push(JSON.stringify({ type: "sessions", sessions }))
                  }
                  break
              }

              return responses
            }).pipe(
              Effect.catchTag(
                "ParseError",
                (error) => Effect.fail(new ClaudeCodeServerError({ message: `Parse error: ${error.message}` }))
              ),
              Effect.catchTag("MachineNotFoundError", (error) =>
                Effect.fail(
                  new ClaudeCodeServerError({
                    message: `Machine not found: ${error.machineId}`,
                    machineId: error.machineId
                  })
                ))
            ),

          close: () =>
            Effect.gen(function*() {
              yield* Ref.update(clients, HashMap.remove(connectionId))
              yield* Ref.update(connectionToClient, HashMap.remove(connectionId))
              yield* Ref.update(clientQueues, HashMap.remove(connectionId))
            }),

          subscribeToMachines: (machineIds: ReadonlyArray<string>) =>
            Effect.gen(function*() {
              yield* Ref.update(clients, (allClients) => {
                const client = HashMap.get(allClients, connectionId)
                if (Option.isSome(client)) {
                  const clientValue = Option.getOrThrow(client)
                  return HashMap.set(allClients, connectionId, {
                    ...clientValue,
                    subscribedMachines: new Set([...clientValue.subscribedMachines, ...machineIds])
                  })
                }
                return allClients
              })
            }),

          getResponseStream: () => {
            const queue = Ref.get(clientQueues).pipe(
              Effect.map(HashMap.get(connectionId)),
              Effect.flatMap((option) =>
                Option.isSome(option)
                  ? Effect.succeed(option.value)
                  : Effect.fail(new ClaudeCodeServerError({ message: "Client queue not found" }))
              )
            )

            return Stream.fromQueue(Effect.runSync(queue))
          }
        }

        yield* Effect.log(`Client registered: ${userId} (${connectionId})`)
        return handler
      })

    // Command routing
    const routeCommand = (
      command: ClaudeCodeCommand
    ) =>
      Effect.gen(function*() {
        const allMachines = yield* Ref.get(machines)
        const machine = HashMap.get(allMachines, command.machineId)

        if (Option.isNone(machine)) {
          yield* Effect.fail(new MachineNotFoundError({ machineId: command.machineId }))
        } else {
          const queues = yield* Ref.get(machineQueues)
          const queue = HashMap.get(queues, command.machineId)

          if (Option.isSome(queue)) {
            yield* Queue.offer(queue.value, command)
            yield* Ref.update(stats, (s) => ({
              ...s,
              commandsRouted: s.commandsRouted + 1
            }))
            yield* Effect.log(`Command routed to machine ${command.machineId}: ${command.type}`)
          } else {
            yield* Effect.fail(
              new ClaudeCodeServerError({
                message: "Machine command queue not found",
                machineId: command.machineId
              })
            )
          }
        }
      })

    // Query methods
    const getActiveMachines = () =>
      Effect.gen(function*() {
        const allMachines = yield* Ref.get(machines)
        return Array.from(HashMap.values(allMachines))
          .filter((m: MachineConnection) => m.isActive)
          .map((m: MachineConnection) => ({
            ...m.machineInfo,
            machineId: m.id,
            activeSessions: Array.from(HashMap.values(m.activeSessions)),
            lastHeartbeat: m.lastHeartbeat,
            status: "online" as const
          }))
      })

    const getMachineSessions = (machineId: string) =>
      Effect.gen(function*() {
        const allMachines = yield* Ref.get(machines)
        const machine = HashMap.get(allMachines, machineId)

        return machine.pipe(
          Option.match({
            onNone: () => [],
            onSome: (machineValue) => Array.from(HashMap.values(machineValue.activeSessions))
          })
        )
      })

    const getStats = () =>
      Effect.gen(function*() {
        const allMachines = yield* Ref.get(machines)
        const allClients = yield* Ref.get(clients)
        const currentStats = yield* Ref.get(stats)

        const activeMachines = Array.from(HashMap.values(allMachines)).filter((m: MachineConnection) => m.isActive)
        const totalSessions = activeMachines.reduce(
          (sum: number, m: MachineConnection) => sum + HashMap.size(m.activeSessions),
          0
        )
        const activeSessions = activeMachines.reduce(
          (sum: number, m: MachineConnection) =>
            sum + Array.from(HashMap.values(m.activeSessions))
              .filter((s: ClaudeCodeSession) => s.status === "active").length,
          0
        )

        return {
          totalMachines: HashMap.size(allMachines),
          activeMachines: activeMachines.length,
          totalSessions,
          activeSessions,
          totalClients: HashMap.size(allClients),
          activeClients: Array.from(HashMap.values(allClients)).filter((c: ClientConnection) => c.isActive).length,
          ...currentStats
        }
      })

    // Connection cleanup
    const removeConnection = (connectionId: string) =>
      Effect.gen(function*() {
        // Check if it's a machine connection
        const machineMap = yield* Ref.get(connectionToMachine)
        const machineId = HashMap.get(machineMap, connectionId)

        if (Option.isSome(machineId)) {
          const machineIdValue = machineId.value
          yield* Ref.update(machines, (allMachines) => {
            const machine = HashMap.get(allMachines, machineIdValue)
            if (Option.isSome(machine)) {
              const machineValue = machine.value
              return HashMap.set(allMachines, machineIdValue, {
                ...machineValue,
                isActive: false
              })
            }
            return allMachines
          })
          yield* Ref.update(connectionToMachine, HashMap.remove(connectionId))
          yield* Ref.update(machineQueues, HashMap.remove(machineIdValue))

          // Notify clients about machine going offline
          yield* broadcastToClients({
            type: "machine_status",
            machineId: machineIdValue,
            status: "offline"
          })

          yield* Effect.log(`Machine disconnected: ${machineIdValue}`)
          return
        }

        // Check if it's a client connection
        const clientMap = yield* Ref.get(connectionToClient)
        const clientId = HashMap.get(clientMap, connectionId)

        if (Option.isSome(clientId)) {
          const clientIdValue = clientId.value
          yield* Ref.update(clients, HashMap.remove(clientIdValue))
          yield* Ref.update(connectionToClient, HashMap.remove(connectionId))
          yield* Ref.update(clientQueues, HashMap.remove(clientIdValue))

          yield* Effect.log(`Client disconnected: ${clientIdValue}`)
        }
      })

    return {
      registerMachine,
      updateMachineHeartbeat,
      handleMachineResponse,
      registerClient,
      routeCommand,
      getActiveMachines,
      getMachineSessions,
      getStats,
      removeConnection
    }
  })
)
