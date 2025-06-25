import { Context, Effect, Layer, pipe, Ref, Schedule, Stream } from "effect"
import * as fs from "node:fs/promises"
import * as os from "node:os"
import * as path from "node:path"
import * as ConvexSync from "./ConvexSync.js"
import type { EmbeddingConfig } from "./ConvexSync.js"
import * as FileWatcher from "./FileWatcher.js"
import * as JSONLParser from "./JSONLParser.js"
import * as WebSocketClient from "./WebSocketClient.js"

// Types
export interface DaemonConfig {
  readonly userId: string
  readonly apiKey: string
  readonly endpoint: string
}

export interface ClaudeInstallation {
  readonly path: string
  readonly sessionCount: number
  readonly lastActive: string | null
}

export interface SyncResult {
  readonly synced: number
  readonly failed: number
  readonly errors: ReadonlyArray<string>
}

export interface FilterOptions {
  readonly includePaths?: ReadonlyArray<string>
  readonly excludePaths?: ReadonlyArray<string>
}

export interface DaemonStatus {
  readonly running: boolean
  readonly uptime: string
  readonly watchedPaths: ReadonlyArray<string>
  readonly sessionCount: number
  readonly lastSync: string | null
  readonly websocketConnected: boolean
}

// Service interface
export interface OverlordService {
  readonly startDaemon: (config: DaemonConfig) => Effect.Effect<void, Error>
  readonly stopDaemon: () => Effect.Effect<void>
  readonly detectClaudeInstallations: () => Effect.Effect<ReadonlyArray<ClaudeInstallation>, Error>
  readonly syncSession: (
    sessionId: string,
    auth: { userId: string; apiKey: string },
    embeddingConfig?: EmbeddingConfig
  ) => Effect.Effect<void, Error>
  readonly syncAllSessions: (
    auth: { userId: string; apiKey: string },
    filterOptions?: FilterOptions,
    embeddingConfig?: EmbeddingConfig
  ) => Effect.Effect<SyncResult, Error>
  readonly getStatus: () => Effect.Effect<DaemonStatus>
}

export const OverlordService = Context.GenericTag<OverlordService>("@openagentsinc/overlord/OverlordService")

// Implementation
export const OverlordServiceLive = Layer.effect(
  OverlordService,
  Effect.gen(function*() {
    const fileWatcher = yield* FileWatcher.FileWatcher
    const wsClient = yield* WebSocketClient.WebSocketClient
    const convexSync = yield* ConvexSync.ConvexSyncService

    // Daemon state
    const daemonState = yield* Ref.make({
      running: false,
      startTime: null as Date | null,
      sessionCount: 0,
      lastSync: null as Date | null
    })

    // Start the daemon
    const startDaemon = (config: DaemonConfig) =>
      Effect.gen(function*() {
        // Update state
        yield* Ref.update(daemonState, (state) => ({
          ...state,
          running: true,
          startTime: new Date()
        }))

        // Find and watch Claude paths
        const claudePaths = yield* fileWatcher.findClaudePaths()
        if (claudePaths.length === 0) {
          yield* Effect.fail(new Error("No Claude installations found"))
          return
        }

        yield* fileWatcher.watchPaths(claudePaths)

        // Connect to WebSocket
        yield* wsClient.connect(config.endpoint, {
          userId: config.userId,
          apiKey: config.apiKey
        })

        // Process file changes
        yield* pipe(
          fileWatcher.getChanges(),
          Stream.tap((event) =>
            Effect.gen(function*() {
              yield* Effect.logInfo(`File ${event.action}: ${event.filePath}`)

              // Send file change notification
              const message: WebSocketClient.OverlordMessage = {
                type: "file_change",
                machineId: getMachineId(),
                timestamp: new Date().toISOString(),
                data: {
                  action: event.action,
                  filePath: event.filePath,
                  sessionId: event.sessionId,
                  projectPath: event.projectPath
                }
              }

              yield* wsClient.send(message).pipe(
                Effect.catchAll((error) => Effect.logError(`Failed to send file change: ${error}`))
              )

              // If file was created or modified, parse and sync content
              // Note: Daemon mode doesn't support embeddings yet - only manual sync commands
              if (event.action !== "deleted") {
                yield* syncSessionFile(event.filePath, event.sessionId, config, undefined)
              }
            })
          ),
          Stream.runDrain
        ).pipe(
          Effect.fork // Run in background
        )

        // Process server commands
        yield* pipe(
          wsClient.receive(),
          Stream.tap((message) =>
            Effect.gen(function*() {
              yield* Effect.logInfo(`Received command: ${message.type}`)
              // Handle different command types here
            })
          ),
          Stream.runDrain
        ).pipe(
          Effect.fork // Run in background
        )

        // Send periodic heartbeats
        yield* Effect.repeat(
          wsClient.send({
            type: "heartbeat",
            machineId: getMachineId(),
            timestamp: new Date().toISOString(),
            data: { status: "alive" }
          }).pipe(
            Effect.catchAll(() => Effect.void)
          ),
          Schedule.fixed("30 seconds")
        ).pipe(
          Effect.fork // Run in background
        )
      })

    // Stop the daemon
    const stopDaemon = () =>
      Effect.gen(function*() {
        yield* fileWatcher.stopWatching()
        yield* wsClient.disconnect()
        yield* Ref.update(daemonState, (state) => ({
          ...state,
          running: false
        }))
      })

    // Detect Claude installations
    const detectClaudeInstallations = () =>
      Effect.gen(function*() {
        const claudePaths = yield* fileWatcher.findClaudePaths()
        const installations: Array<ClaudeInstallation> = []

        for (const claudePath of claudePaths) {
          try {
            const files = yield* Effect.tryPromise(() => fs.readdir(claudePath, { recursive: true }))
            const jsonlFiles = files.filter((f) => f.endsWith(".jsonl"))

            let lastActive: string | null = null
            if (jsonlFiles.length > 0) {
              // Get file stats and find most recent
              const filesWithStats = yield* Effect.tryPromise(async () => {
                const statsPromises = jsonlFiles.map(async (file) => {
                  const filePath = path.join(claudePath, file)
                  const fileStat = await fs.stat(filePath)
                  return { file, mtime: fileStat.mtime }
                })
                const results = await Promise.all(statsPromises)
                return results.sort((a, b) => b.mtime.getTime() - a.mtime.getTime())
              })

              if (filesWithStats.length > 0) {
                lastActive = filesWithStats[0].mtime.toISOString()
              }
            }

            installations.push({
              path: claudePath,
              sessionCount: jsonlFiles.length,
              lastActive
            })
          } catch (error) {
            yield* Effect.logError(`Failed to scan ${claudePath}: ${error}`)
          }
        }

        return installations
      })

    // Sync a specific session
    const syncSession = (
      sessionId: string,
      auth: { userId: string; apiKey: string },
      embeddingConfig?: EmbeddingConfig
    ) =>
      Effect.gen(function*() {
        // Find the session file
        const claudePaths = yield* fileWatcher.findClaudePaths()
        let sessionFile: string | null = null

        for (const claudePath of claudePaths) {
          const files = yield* Effect.tryPromise(() => fs.readdir(claudePath, { recursive: true }))
          const match = files.find((f) => f.includes(sessionId) && f.endsWith(".jsonl"))
          if (match) {
            sessionFile = path.join(claudePath, match)
            break
          }
        }

        if (!sessionFile) {
          yield* Effect.fail(new Error(`Session ${sessionId} not found`))
          return
        }

        yield* syncSessionFile(sessionFile, sessionId, auth, embeddingConfig)
      })

    // Sync all sessions
    const syncAllSessions = (
      auth: { userId: string; apiKey: string },
      filterOptions?: FilterOptions,
      embeddingConfig?: EmbeddingConfig
    ) =>
      Effect.gen(function*() {
        const claudePaths = yield* fileWatcher.findClaudePaths()
        let synced = 0
        let failed = 0
        const errors: Array<string> = []

        for (const claudePath of claudePaths) {
          // Apply path filtering
          if (
            filterOptions?.includePaths && !filterOptions.includePaths.some((include) => claudePath.includes(include))
          ) {
            continue
          }
          if (
            filterOptions?.excludePaths && filterOptions.excludePaths.some((exclude) => claudePath.includes(exclude))
          ) {
            continue
          }
          const files = yield* Effect.tryPromise(() => fs.readdir(claudePath, { recursive: true }))
          const jsonlFiles = files.filter((f) => f.endsWith(".jsonl"))

          // Get file stats and sort by modification time (newest first)
          const filesWithStats = yield* Effect.tryPromise(async () => {
            const statsPromises = jsonlFiles.map(async (file) => {
              const filePath = path.join(claudePath, file)
              const fileStat = await fs.stat(filePath)
              return { file, filePath, mtime: fileStat.mtime.getTime() }
            })
            const results = await Promise.all(statsPromises)
            return results.sort((a, b) => b.mtime - a.mtime) // Sort by mtime descending (newest first)
          })

          for (const { file, filePath } of filesWithStats) {
            const sessionId = path.basename(file, ".jsonl")

            yield* syncSessionFile(filePath, sessionId, auth, embeddingConfig).pipe(
              Effect.tap(() =>
                Effect.sync(() => {
                  synced++
                })
              ),
              Effect.catchAll((error) =>
                Effect.sync(() => {
                  failed++
                  errors.push(`${sessionId}: ${error}`)
                })
              )
            )
          }
        }

        yield* Ref.update(daemonState, (state) => ({
          ...state,
          lastSync: new Date(),
          sessionCount: synced
        }))

        return { synced, failed, errors }
      })

    // Get daemon status
    const getStatus = () =>
      Effect.gen(function*() {
        const state = yield* Ref.get(daemonState)
        const connected = yield* wsClient.isConnected()
        const watchedPaths = yield* fileWatcher.findClaudePaths()

        const uptime = state.startTime
          ? formatUptime(Date.now() - state.startTime.getTime())
          : "Not running"

        return {
          running: state.running,
          uptime,
          watchedPaths,
          sessionCount: state.sessionCount,
          lastSync: state.lastSync?.toISOString() || null,
          websocketConnected: connected
        }
      })

    // Helper functions
    const getMachineId = (): string => {
      return `${os.hostname()}-${os.platform()}-${os.arch()}`
    }

    const formatUptime = (ms: number): string => {
      const seconds = Math.floor(ms / 1000)
      const minutes = Math.floor(seconds / 60)
      const hours = Math.floor(minutes / 60)
      const days = Math.floor(hours / 24)

      if (days > 0) return `${days}d ${hours % 24}h`
      if (hours > 0) return `${hours}h ${minutes % 60}m`
      if (minutes > 0) return `${minutes}m ${seconds % 60}s`
      return `${seconds}s`
    }

    const syncSessionFile = (
      filePath: string,
      sessionId: string,
      auth: { userId: string; apiKey: string },
      embeddingConfig?: EmbeddingConfig
    ) =>
      Effect.gen(function*() {
        const content = yield* Effect.tryPromise(() => fs.readFile(filePath, "utf-8"))

        const entries = yield* JSONLParser.parseJSONL(content)

        // Extract project path from file path
        const projectPath = extractProjectPath(filePath)

        // Save to Convex
        yield* convexSync.saveSession(sessionId, auth.userId, projectPath, entries, embeddingConfig)

        // Also send session update to WebSocket for real-time notification
        const message: WebSocketClient.OverlordMessage = {
          type: "session_update",
          machineId: getMachineId(),
          timestamp: new Date().toISOString(),
          data: {
            sessionId,
            entries: entries.length, // Just send count, not full data
            filePath,
            synced: true
          }
        }

        yield* wsClient.send(message).pipe(
          Effect.catchAll((error) => Effect.logError(`Failed to send WebSocket update: ${error}`))
        )
      })

    // Extract project path from file path
    const extractProjectPath = (filePath: string): string => {
      // Claude stores conversations in folders like:
      // ~/Library/Mobile Documents/com~apple~CloudDocs/Claude/<project-hash>/conversations/<session-id>.jsonl
      const parts = filePath.split("/")
      const claudeIndex = parts.findIndex((p) => p === "Claude")
      if (claudeIndex >= 0 && claudeIndex + 1 < parts.length) {
        return parts[claudeIndex + 1]
      }
      // Fallback to parent directory name
      return path.basename(path.dirname(path.dirname(filePath)))
    }

    return {
      startDaemon,
      stopDaemon,
      detectClaudeInstallations,
      syncSession,
      syncAllSessions,
      getStatus
    }
  })
)
