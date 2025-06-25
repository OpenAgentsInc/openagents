/**
 * Claude Code Control Service
 * Manages remote control and interaction with local Claude Code instances
 * @since Phase 3
 */

import { Context, Effect, Layer, Stream, Queue, Ref, Schedule } from "effect"
import * as path from "node:path"
import * as fs from "node:fs/promises"
import type {
  ClaudeCodeCommand,
  ClaudeCodeResponse,
  ClaudeCodeSession,
  ClaudeCodeOptions,
  RemotePrompt,
  ClaudeCodeServiceConfig,
  MachineClaudeInfo,
  ClaudeCodeAuditEvent
} from "../types/ClaudeCodeTypes.js"
import * as WebSocketClient from "./WebSocketClient.js"

// Service interface
export interface ClaudeCodeControlService {
  readonly sendPrompt: (
    machineId: string,
    sessionId: string,
    prompt: string,
    options?: ClaudeCodeOptions
  ) => Effect.Effect<RemotePrompt, Error>

  readonly startSession: (
    machineId: string,
    projectPath: string,
    userId: string
  ) => Effect.Effect<ClaudeCodeSession, Error>

  readonly endSession: (
    machineId: string,
    sessionId: string
  ) => Effect.Effect<void, Error>

  readonly getActiveSessions: (
    machineId: string
  ) => Effect.Effect<ReadonlyArray<ClaudeCodeSession>, Error>

  readonly getMachineInfo: (
    machineId: string
  ) => Effect.Effect<MachineClaudeInfo, Error>

  readonly streamResponses: (
    sessionId: string
  ) => Effect.Effect<Stream.Stream<ClaudeCodeResponse>, Error>

  readonly getSessionHistory: (
    sessionId: string
  ) => Effect.Effect<ReadonlyArray<RemotePrompt>, Error>

  readonly cancelPrompt: (
    promptId: string
  ) => Effect.Effect<void, Error>
}

export const ClaudeCodeControlService = Context.GenericTag<ClaudeCodeControlService>(
  "@openagentsinc/overlord/ClaudeCodeControlService"
)

// Implementation
export const ClaudeCodeControlServiceLive = Layer.effect(
  ClaudeCodeControlService,
  Effect.gen(function*() {
    const wsClient = yield* WebSocketClient.WebSocketClient

    // Internal state for session management
    const activeSessions = yield* Ref.make(new Map<string, ClaudeCodeSession>())
    const activePrompts = yield* Ref.make(new Map<string, RemotePrompt>())
    const responseQueues = yield* Ref.make(new Map<string, Queue.Queue<ClaudeCodeResponse>>())

    // Service configuration
    const config: ClaudeCodeServiceConfig = {
      enableRemoteControl: true,
      maxConcurrentSessions: 5,
      sessionTimeoutMs: 30 * 60 * 1000, // 30 minutes
      promptRateLimit: 10, // 10 prompts per minute
      auditLogging: true
    }

    // Generate unique IDs
    const generateId = (): string => {
      return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
    }

    // Validate project path access
    const validateProjectAccess = (projectPath: string): Effect.Effect<void, Error> =>
      Effect.gen(function*() {
        if (config.allowedProjects) {
          const isAllowed = config.allowedProjects.some(allowedPath => 
            projectPath.startsWith(allowedPath)
          )
          if (!isAllowed) {
            yield* Effect.fail(new Error(`Access denied to project path: ${projectPath}`))
          }
        }

        // Verify path exists and is accessible
        yield* Effect.tryPromise({
          try: () => fs.access(projectPath),
          catch: () => new Error(`Project path not accessible: ${projectPath}`)
        })
      })


    // Audit logging
    const auditLog = (event: ClaudeCodeAuditEvent): Effect.Effect<void, Error> =>
      Effect.gen(function*() {
        if (!config.auditLogging) return

        yield* Effect.logInfo(`Claude Code Audit: ${event.eventType} - ${event.machineId}`)
        
        // In real implementation, store in database
        // yield* ConvexSync.storeAuditEvent(event)
      })

    // Send prompt to Claude Code instance
    const sendPrompt = (
      machineId: string,
      sessionId: string,
      prompt: string,
      options?: ClaudeCodeOptions
    ) =>
      Effect.gen(function*() {
        const promptId = generateId()
        const commandId = generateId()

        const command: ClaudeCodeCommand = {
          type: "send_prompt",
          commandId,
          sessionId,
          machineId,
          userId: "current-user", // TODO: Get from context
          timestamp: new Date(),
          data: {
            prompt,
            ...(options && { options })
          }
        }

        const remotePrompt: RemotePrompt = {
          promptId,
          sessionId,
          machineId,
          userId: command.userId,
          promptText: prompt,
          ...(options && { promptOptions: options }),
          status: "sent",
          sentAt: new Date()
        }

        // Store prompt tracking
        yield* Ref.update(activePrompts, (prompts) => 
          new Map(prompts).set(promptId, remotePrompt)
        )

        // Send command via WebSocket
        yield* wsClient.send({
          type: "session_update",
          machineId,
          timestamp: new Date().toISOString(),
          data: {
            claudeCommand: command
          }
        })

        return remotePrompt
      })

    // Start new Claude Code session
    const startSession = (
      machineId: string,
      projectPath: string,
      userId: string
    ) =>
      Effect.gen(function*() {
        yield* validateProjectAccess(projectPath)

        const sessionId = generateId()
        const commandId = generateId()

        const command: ClaudeCodeCommand = {
          type: "start_session",
          commandId,
          machineId,
          userId,
          timestamp: new Date(),
          data: {
            projectPath
          }
        }

        const session: ClaudeCodeSession = {
          sessionId,
          machineId,
          userId,
          projectPath,
          projectName: path.basename(projectPath),
          status: "active",
          claudeVersion: "unknown", // Will be updated by heartbeat
          startedAt: new Date(),
          messageCount: 0,
          totalTokens: 0
        }

        // Store session
        yield* Ref.update(activeSessions, (sessions) =>
          new Map(sessions).set(sessionId, session)
        )

        // Send start command
        yield* wsClient.send({
          type: "session_update",
          machineId,
          timestamp: new Date().toISOString(),
          data: {
            claudeCommand: command
          }
        })

        yield* auditLog({
          eventId: generateId(),
          machineId,
          userId,
          sessionId,
          eventType: "session_started",
          severity: "low",
          details: { projectPath },
          timestamp: new Date()
        })

        return session
      })

    // End Claude Code session
    const endSession = (machineId: string, sessionId: string) =>
      Effect.gen(function*() {
        const sessions = yield* Ref.get(activeSessions)
        const session = sessions.get(sessionId)

        if (!session) {
          yield* Effect.fail(new Error(`Session not found: ${sessionId}`))
          return
        }

        const commandId = generateId()
        const command: ClaudeCodeCommand = {
          type: "end_session",
          commandId,
          sessionId,
          machineId,
          userId: session.userId,
          timestamp: new Date(),
          data: {}
        }

        // Send end command
        yield* wsClient.send({
          type: "session_update",
          machineId,
          timestamp: new Date().toISOString(),
          data: {
            claudeCommand: command
          }
        })

        // Remove from active sessions
        yield* Ref.update(activeSessions, (sessions) => {
          const newSessions = new Map(sessions)
          newSessions.delete(sessionId)
          return newSessions
        })

        yield* auditLog({
          eventId: generateId(),
          machineId,
          userId: session.userId,
          sessionId,
          eventType: "session_ended",
          severity: "low",
          details: {},
          timestamp: new Date()
        })
      })

    // Get active sessions for machine
    const getActiveSessions = (machineId: string) =>
      Effect.gen(function*() {
        const sessions = yield* Ref.get(activeSessions)
        const machineSessions = Array.from(sessions.values()).filter(
          session => session.machineId === machineId
        )
        return machineSessions
      })

    // Get machine Claude Code information
    const getMachineInfo = (machineId: string) =>
      Effect.gen(function*() {
        const sessions = yield* getActiveSessions(machineId)

        // TODO: Get real machine info via WebSocket heartbeat
        const machineInfo: MachineClaudeInfo = {
          machineId,
          hostname: "unknown",
          claudeVersion: "unknown",
          sdkVersion: "unknown",
          supportedFeatures: ["file_edit", "command_exec", "git_ops"],
          activeProjects: sessions.map(s => s.projectPath),
          activeSessions: sessions,
          lastHeartbeat: new Date(),
          status: sessions.length > 0 ? "busy" : "online"
        }

        return machineInfo
      })

    // Stream responses for session
    const streamResponses = (sessionId: string) =>
      Effect.gen(function*() {
        const queues = yield* Ref.get(responseQueues)
        let queue = queues.get(sessionId)

        if (!queue) {
          queue = yield* Queue.unbounded<ClaudeCodeResponse>()
          yield* Ref.update(responseQueues, (queues) =>
            new Map(queues).set(sessionId, queue!)
          )
        }

        return Stream.fromQueue(queue)
      })

    // Get session history
    const getSessionHistory = (sessionId: string) =>
      Effect.gen(function*() {
        const prompts = yield* Ref.get(activePrompts)
        const sessionPrompts = Array.from(prompts.values()).filter(
          prompt => prompt.sessionId === sessionId
        )
        return sessionPrompts.sort((a, b) => a.sentAt.getTime() - b.sentAt.getTime())
      })

    // Cancel active prompt
    const cancelPrompt = (promptId: string) =>
      Effect.gen(function*() {
        yield* Ref.update(activePrompts, (prompts) => {
          const newPrompts = new Map(prompts)
          const prompt = newPrompts.get(promptId)
          if (prompt) {
            newPrompts.set(promptId, { ...prompt, status: "cancelled" })
          }
          return newPrompts
        })
      })

    // Session cleanup task
    const sessionCleanup = Effect.gen(function*() {
      const sessions = yield* Ref.get(activeSessions)
      const now = Date.now()
      const expiredSessions: string[] = []

      for (const [sessionId, session] of sessions) {
        const lastActivity = session.lastResponseAt || session.startedAt
        if (now - lastActivity.getTime() > config.sessionTimeoutMs) {
          expiredSessions.push(sessionId)
        }
      }

      for (const sessionId of expiredSessions) {
        yield* Effect.logInfo(`Cleaning up expired session: ${sessionId}`)
        yield* Ref.update(activeSessions, (sessions) => {
          const newSessions = new Map(sessions)
          newSessions.delete(sessionId)
          return newSessions
        })
      }
    })

    // Start session cleanup task
    yield* Effect.repeat(
      sessionCleanup,
      Schedule.fixed("5 minutes")
    ).pipe(Effect.fork)

    return {
      sendPrompt,
      startSession,
      endSession,
      getActiveSessions,
      getMachineInfo,
      streamResponses,
      getSessionHistory,
      cancelPrompt
    }
  })
)