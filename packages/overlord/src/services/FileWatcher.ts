import { FileSystem } from "@effect/platform"
import { NodeFileSystem } from "@effect/platform-node"
import { Context, Effect, Layer, Queue, Stream } from "effect"
import * as fs from "node:fs"
import * as os from "node:os"
import * as path from "node:path"

// File change event types
export interface FileChangeEvent {
  readonly action: "created" | "modified" | "deleted"
  readonly filePath: string
  readonly sessionId: string
  readonly projectPath: string
  readonly timestamp: Date
}

// Service interface
export interface FileWatcher {
  readonly watchPaths: (paths: ReadonlyArray<string>) => Effect.Effect<void>
  readonly stopWatching: () => Effect.Effect<void>
  readonly getChanges: () => Stream.Stream<FileChangeEvent>
  readonly findClaudePaths: () => Effect.Effect<ReadonlyArray<string>>
}

export const FileWatcher = Context.GenericTag<FileWatcher>("@openagentsinc/overlord/FileWatcher")

// Implementation
export const FileWatcherLive = Layer.effect(
  FileWatcher,
  Effect.gen(function*() {
    const fileSystem = yield* FileSystem.FileSystem
    const changeQueue = yield* Queue.unbounded<FileChangeEvent>()
    const watchers = new Map<string, fs.FSWatcher>()

    // Helper to extract session ID from JSONL filename
    const extractSessionId = (filePath: string): string => {
      const basename = path.basename(filePath, ".jsonl")
      return basename
    }

    // Helper to extract project path from file path
    const extractProjectPath = (filePath: string): string => {
      // Claude stores files in format: ~/.claude/projects/{hashed-project-path}/{session-id}.jsonl
      const parts = filePath.split(path.sep)
      const projectsIndex = parts.findIndex((p) => p === "projects")
      if (projectsIndex >= 0 && projectsIndex < parts.length - 2) {
        return parts[projectsIndex + 1] // The hashed project path
      }
      return "unknown"
    }

    // Find Claude installation paths
    const findClaudePaths = (): Effect.Effect<ReadonlyArray<string>> =>
      Effect.gen(function*() {
        const homedir = os.homedir()
        const paths = [
          path.join(homedir, ".claude", "projects"),
          path.join(homedir, ".config", "claude", "projects")
        ]

        const existingPaths: Array<string> = []
        for (const p of paths) {
          const exists = yield* fileSystem.exists(p).pipe(
            Effect.orElse(() => Effect.succeed(false))
          )
          if (exists) {
            existingPaths.push(p)
          }
        }

        return existingPaths
      })

    // Watch a single path
    const watchPath = (watchPath: string) =>
      Effect.gen(function*() {
        if (watchers.has(watchPath)) {
          return // Already watching
        }

        const watcher = fs.watch(watchPath, { recursive: true }, (eventType, filename) => {
          if (!filename || !filename.endsWith(".jsonl")) {
            return // Only interested in JSONL files
          }

          const fullPath = path.join(watchPath, filename)
          const sessionId = extractSessionId(filename)
          const projectPath = extractProjectPath(fullPath)

          // Determine action based on event type and file existence
          let action: FileChangeEvent["action"]
          if (eventType === "rename") {
            // Check if file exists to determine if created or deleted
            if (fs.existsSync(fullPath)) {
              action = "created"
            } else {
              action = "deleted"
            }
          } else {
            action = "modified"
          }

          const event: FileChangeEvent = {
            action,
            filePath: fullPath,
            sessionId,
            projectPath,
            timestamp: new Date()
          }

          // Queue the event
          Effect.runSync(Queue.offer(changeQueue, event))
        })

        watchers.set(watchPath, watcher)
        yield* Effect.logInfo(`Watching path: ${watchPath}`)
      })

    // Stop watching all paths
    const stopWatching = () =>
      Effect.gen(function*() {
        for (const [path, watcher] of watchers) {
          watcher.close()
          yield* Effect.logInfo(`Stopped watching: ${path}`)
        }
        watchers.clear()
      })

    // Watch multiple paths
    const watchPaths = (paths: ReadonlyArray<string>) =>
      Effect.gen(function*() {
        for (const p of paths) {
          yield* watchPath(p)
        }
      })

    // Get change stream
    const getChanges = () => Stream.fromQueue(changeQueue)

    return {
      watchPaths,
      stopWatching,
      getChanges,
      findClaudePaths
    }
  }).pipe(
    Effect.provide(NodeFileSystem.layer)
  )
)
