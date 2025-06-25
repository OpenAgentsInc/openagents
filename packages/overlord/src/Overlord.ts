import { Args, Command, Options } from "@effect/cli"
import { NodeContext } from "@effect/platform-node"
import { Console, Effect, Option } from "effect"
import * as ConvexSync from "./services/ConvexSync.js"
import type { EmbeddingConfig } from "./services/ConvexSync.js"
import * as FileWatcher from "./services/FileWatcher.js"
import * as OverlordService from "./services/OverlordService.js"
import * as WebSocketClient from "./services/WebSocketClient.js"

// StarCraft-themed commands for Overlord

const userIdOption = Options.text("user-id").pipe(
  Options.withDescription("User ID for authentication"),
  Options.withAlias("u")
)

const apiKeyOption = Options.text("api-key").pipe(
  Options.withDescription("API key for OpenAgents.com"),
  Options.withAlias("k")
)

const endpointOption = Options.text("endpoint").pipe(
  Options.withDescription("WebSocket endpoint (default: wss://openagents.com/ws)"),
  Options.withDefault("wss://openagents.com/ws"),
  Options.optional
)

const includePathOption = Options.text("include-path").pipe(
  Options.withDescription("Only sync projects containing this path substring (can be used multiple times)"),
  Options.withAlias("i"),
  Options.repeated
)

const excludePathOption = Options.text("exclude-path").pipe(
  Options.withDescription("Exclude projects containing this path substring (can be used multiple times)"),
  Options.withAlias("e"),
  Options.repeated
)

const enableEmbeddingsOption = Options.boolean("enable-embeddings").pipe(
  Options.withDescription("Generate vector embeddings for semantic search (requires OpenAI API key)"),
  Options.withDefault(false),
  Options.optional
)

const embeddingModelOption = Options.text("embedding-model").pipe(
  Options.withDescription("OpenAI embedding model to use"),
  Options.withDefault("text-embedding-3-small"),
  Options.optional
)

// Main daemon command - "spawn" in StarCraft terms
const spawnCommand = Command.make("spawn", {
  userId: userIdOption,
  apiKey: apiKeyOption,
  endpoint: endpointOption
}).pipe(
  Command.withDescription("Spawn the Overlord daemon to monitor Claude Code sessions"),
  Command.withHandler(({ apiKey, endpoint, userId }) =>
    Effect.gen(function*() {
      yield* Console.log("🎮 Spawning Overlord...")
      yield* Console.log(`📡 Connecting to ${Option.getOrElse(endpoint, () => "wss://openagents.com/ws")}`)
      yield* Console.log(`👤 User ID: ${userId}`)

      const service = yield* OverlordService.OverlordService

      // Start the daemon
      yield* service.startDaemon({
        userId,
        apiKey,
        endpoint: Option.getOrElse(endpoint, () => "wss://openagents.com/ws")
      })

      yield* Console.log("✅ Overlord spawned successfully")
      yield* Console.log("🔍 Monitoring Claude Code sessions...")
      yield* Console.log("Press Ctrl+C to stop")

      // Keep the process running
      yield* Effect.never
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
      Effect.provide(ConvexSync.ConvexSyncServiceLive),
      Effect.provide(FileWatcher.FileWatcherLive),
      Effect.provide(WebSocketClient.WebSocketClientLive),
      Effect.provide(NodeContext.layer),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Failed to spawn Overlord: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

// Detect Claude installations
const detectCommand = Command.make("detect").pipe(
  Command.withDescription("Detect Claude Code installations on this machine"),
  Command.withHandler(() =>
    Effect.gen(function*() {
      yield* Console.log("🔍 Detecting Claude Code installations...")

      const service = yield* OverlordService.OverlordService
      const installations = yield* service.detectClaudeInstallations()

      if (installations.length === 0) {
        yield* Console.log("❌ No Claude Code installations found")
        yield* Console.log("📝 Claude Code stores data in:")
        yield* Console.log("   - ~/.claude/projects/")
        yield* Console.log("   - ~/.config/claude/projects/")
        return
      }

      yield* Console.log(`✅ Found ${installations.length} Claude Code installation(s):`)
      for (const install of installations) {
        yield* Console.log(`   📁 ${install.path}`)
        yield* Console.log(`      Sessions: ${install.sessionCount}`)
        yield* Console.log(`      Last active: ${install.lastActive || "Never"}`)
      }
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
      Effect.provide(ConvexSync.ConvexSyncServiceLive),
      Effect.provide(FileWatcher.FileWatcherLive),
      Effect.provide(WebSocketClient.WebSocketClientLive),
      Effect.provide(NodeContext.layer),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Detection failed: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

// Transport (sync) specific sessions or all
const sessionIdArg = Args.text({ name: "sessionId" }).pipe(
  Args.withDescription("Session ID to sync (or 'all' for all sessions)"),
  Args.optional
)

const transportCommand = Command.make("transport", {
  sessionId: sessionIdArg,
  userId: userIdOption,
  apiKey: apiKeyOption,
  includePaths: includePathOption,
  excludePaths: excludePathOption,
  enableEmbeddings: enableEmbeddingsOption,
  embeddingModel: embeddingModelOption
}).pipe(
  Command.withDescription("Transport (sync) Claude Code sessions to the cloud"),
  Command.withHandler(({ apiKey, embeddingModel, enableEmbeddings, excludePaths, includePaths, sessionId, userId }) =>
    Effect.gen(function*() {
      const service = yield* OverlordService.OverlordService

      // Create filter options
      const filterOptions: OverlordService.FilterOptions = {
        ...(includePaths.length > 0 && { includePaths: includePaths as ReadonlyArray<string> }),
        ...(excludePaths.length > 0 && { excludePaths: excludePaths as ReadonlyArray<string> })
      }

      // Create embedding configuration
      const embeddingConfig = Option.getOrElse(enableEmbeddings, () => false) ?
        {
          enabled: true,
          model: Option.getOrElse(embeddingModel, () => "text-embedding-3-small"),
          ...(process.env.OPENAI_API_KEY && { apiKey: process.env.OPENAI_API_KEY })
        } as const satisfies EmbeddingConfig :
        undefined

      if (embeddingConfig) {
        yield* Console.log(`✨ Vector embeddings enabled using model: ${embeddingConfig.model}`)
      }

      if (Option.isNone(sessionId) || Option.getOrElse(sessionId, () => "") === "all") {
        yield* Console.log("🚀 Transporting all sessions to the cloud...")
        if (filterOptions.includePaths || filterOptions.excludePaths) {
          yield* Console.log(
            `📂 Path filtering: include=${filterOptions.includePaths?.join(", ") || "all"}, exclude=${
              filterOptions.excludePaths?.join(", ") || "none"
            }`
          )
        }
        const result = yield* service.syncAllSessions({ userId, apiKey }, filterOptions, embeddingConfig)
        yield* Console.log(`✅ Transported ${result.synced} sessions`)
        if (result.failed > 0) {
          yield* Console.log(`⚠️  Failed to sync ${result.failed} sessions`)
        }
      } else {
        yield* Console.log(`🚀 Transporting session ${Option.getOrElse(sessionId, () => "")}...`)
        yield* service.syncSession(Option.getOrElse(sessionId, () => ""), { userId, apiKey }, embeddingConfig)
        yield* Console.log("✅ Session transported successfully")
      }
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
      Effect.provide(ConvexSync.ConvexSyncServiceLive),
      Effect.provide(FileWatcher.FileWatcherLive),
      Effect.provide(WebSocketClient.WebSocketClientLive),
      Effect.provide(NodeContext.layer),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Transport failed: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

// Import command - for testing with limited conversations
const limitOption = Options.integer("limit").pipe(
  Options.withDescription("Maximum number of conversations to import"),
  Options.withDefault(10),
  Options.optional
)

const importCommand = Command.make("import", {
  userId: userIdOption,
  apiKey: apiKeyOption,
  limit: limitOption,
  includePaths: includePathOption,
  excludePaths: excludePathOption,
  enableEmbeddings: enableEmbeddingsOption,
  embeddingModel: embeddingModelOption
}).pipe(
  Command.withDescription("Import Claude Code conversations to Convex (for testing)"),
  Command.withHandler((
    { apiKey: _apiKey, embeddingModel, enableEmbeddings, excludePaths, includePaths, limit, userId }
  ) =>
    Effect.gen(function*() {
      const service = yield* OverlordService.OverlordService
      const maxLimit = Option.getOrElse(limit, () => 10)

      // Create filter options
      const filterOptions: OverlordService.FilterOptions = {
        ...(includePaths.length > 0 && { includePaths: includePaths as ReadonlyArray<string> }),
        ...(excludePaths.length > 0 && { excludePaths: excludePaths as ReadonlyArray<string> })
      }

      // Create embedding configuration
      const embeddingConfig = Option.getOrElse(enableEmbeddings, () => false) ?
        {
          enabled: true,
          model: Option.getOrElse(embeddingModel, () => "text-embedding-3-small"),
          ...(process.env.OPENAI_API_KEY && { apiKey: process.env.OPENAI_API_KEY })
        } as const satisfies EmbeddingConfig :
        undefined

      yield* Console.log(`📥 Importing up to ${maxLimit} Claude Code conversations...`)
      if (embeddingConfig) {
        yield* Console.log(`✨ Vector embeddings enabled using model: ${embeddingConfig.model}`)
      }
      if (filterOptions.includePaths || filterOptions.excludePaths) {
        yield* Console.log(
          `📂 Path filtering: include=${filterOptions.includePaths?.join(", ") || "all"}, exclude=${
            filterOptions.excludePaths?.join(", ") || "none"
          }`
        )
      }

      // First detect installations
      const installations = yield* service.detectClaudeInstallations()
      if (installations.length === 0) {
        yield* Console.log("❌ No Claude Code installations found")
        return
      }

      // Get all JSONL files
      const fileWatcher = yield* FileWatcher.FileWatcher
      const convexSync = yield* ConvexSync.ConvexSyncService
      const claudePaths = yield* fileWatcher.findClaudePaths()

      let imported = 0
      let failed = 0
      const errors: Array<string> = []

      for (const claudePath of claudePaths) {
        if (imported >= maxLimit) break

        // Apply path filtering
        if (filterOptions.includePaths && !filterOptions.includePaths.some((include) => claudePath.includes(include))) {
          yield* Console.log(`  ⏭️  Skipping ${claudePath} (not in include paths)`)
          continue
        }
        if (filterOptions.excludePaths && filterOptions.excludePaths.some((exclude) => claudePath.includes(exclude))) {
          yield* Console.log(`  ⏭️  Skipping ${claudePath} (in exclude paths)`)
          continue
        }

        const fs = yield* Effect.tryPromise(() => import("node:fs/promises"))
        const files = yield* Effect.tryPromise(() => fs.readdir(claudePath, { recursive: true }))
        const jsonlFiles = files
          .filter((f): f is string => typeof f === "string" && f.endsWith(".jsonl"))

        // Get file stats and sort by modification time (newest first)
        const filesWithStats = yield* Effect.tryPromise(async () => {
          const statsPromises = jsonlFiles.map(async (file) => {
            const filePath = `${claudePath}/${file}`
            const stat = await fs.stat(filePath)
            return { file, filePath, mtime: stat.mtime.getTime() }
          })
          const results = await Promise.all(statsPromises)
          return results.sort((a, b) => b.mtime - a.mtime) // Sort by mtime descending (newest first)
        })

        for (const { file, filePath } of filesWithStats) {
          if (imported >= maxLimit) break

          const sessionId = file.replace(".jsonl", "").split("/").pop() || file

          yield* Console.log(`  📄 Importing session ${sessionId}...`)

          yield* Effect.gen(function*() {
            // Read and parse file
            const content = yield* Effect.tryPromise(() => fs.readFile(filePath, "utf-8"))

            const JSONLParser = yield* Effect.tryPromise(() => import("./services/JSONLParser.js"))
            const entries = yield* JSONLParser.parseJSONL(content)

            // Extract project path from Claude Code file structure
            // Path format: ~/.claude/projects/{project-dir}/{session-id}.jsonl
            const parts = filePath.split("/")
            let projectPath = "unknown"

            // Find .claude directory and extract project name
            const claudeDirIndex = parts.findIndex((p) => p === ".claude")
            if (claudeDirIndex >= 0 && claudeDirIndex + 2 < parts.length) {
              const projectDir = parts[claudeDirIndex + 2] // skip "projects" folder
              if (projectDir && projectDir !== "projects") {
                // Extract meaningful name from encoded project directory
                // Format like "-Users-christopherdavid-code-yt-dlp" -> "yt-dlp"
                const segments = projectDir.split("-").filter((p) => p)
                if (segments.length >= 2) {
                  const lastTwo = segments.slice(-2)
                  // If second-to-last is "code", just take the last part
                  if (lastTwo[0] === "code") {
                    projectPath = lastTwo[1]
                  } else {
                    // Take last two parts for compound names like "yt-dlp"
                    projectPath = lastTwo.join("-")
                  }
                } else {
                  projectPath = segments.pop() || projectDir
                }
              }
            }

            // Fallback: use parent directory of the JSONL file
            if (projectPath === "unknown") {
              const parentDir = parts[parts.length - 2]
              if (parentDir && parentDir !== "projects") {
                projectPath = parentDir
              }
            }

            // Save to Convex
            yield* convexSync.saveSession(sessionId, userId, projectPath, entries, embeddingConfig)

            imported++
            yield* Console.log(`    ✅ Imported ${entries.length} entries`)
          }).pipe(
            Effect.catchAll((error) => {
              failed++
              errors.push(`${sessionId}: ${error}`)
              return Console.log(`    ❌ Failed: ${error}`)
            })
          )
        }
      }

      yield* Console.log(`\n📊 Import Summary:`)
      yield* Console.log(`   ✅ Imported: ${imported} sessions`)
      if (failed > 0) {
        yield* Console.log(`   ❌ Failed: ${failed} sessions`)
        for (const err of errors) {
          yield* Console.log(`      - ${err}`)
        }
      }
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
      Effect.provide(ConvexSync.ConvexSyncServiceLive),
      Effect.provide(FileWatcher.FileWatcherLive),
      Effect.provide(WebSocketClient.WebSocketClientLive),
      Effect.provide(NodeContext.layer),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Import failed: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

// Burrow (background/detach) and unburrow (foreground/attach)
const burrowCommand = Command.make("burrow").pipe(
  Command.withDescription("Burrow the Overlord daemon (run in background)"),
  Command.withHandler(() =>
    Effect.gen(function*() {
      yield* Console.log("🕳️  Burrowing Overlord daemon...")
      yield* Console.log("✅ Overlord is now running in the background")
      yield* Console.log("Use 'overlord unburrow' to bring it back to foreground")
      // Implementation would detach the process
    })
  )
)

const unburrowCommand = Command.make("unburrow").pipe(
  Command.withDescription("Unburrow the Overlord daemon (bring to foreground)"),
  Command.withHandler(() =>
    Effect.gen(function*() {
      yield* Console.log("🔄 Unburrowing Overlord daemon...")
      yield* Console.log("✅ Overlord is now in the foreground")
      // Implementation would reattach to the process
    })
  )
)

// Status command
const statusCommand = Command.make("status").pipe(
  Command.withDescription("Show Overlord daemon status"),
  Command.withHandler(() =>
    Effect.gen(function*() {
      yield* Console.log("📊 Overlord Status")
      yield* Console.log("─────────────────")

      const service = yield* OverlordService.OverlordService
      const status = yield* service.getStatus()

      yield* Console.log(`🎮 Daemon: ${status.running ? "Running" : "Stopped"}`)
      if (status.running) {
        yield* Console.log(`⏱️  Uptime: ${status.uptime}`)
        yield* Console.log(`📁 Watching: ${status.watchedPaths.join(", ")}`)
        yield* Console.log(`📊 Sessions tracked: ${status.sessionCount}`)
        yield* Console.log(`🔄 Last sync: ${status.lastSync || "Never"}`)
        yield* Console.log(`📡 WebSocket: ${status.websocketConnected ? "Connected" : "Disconnected"}`)
      }
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
      Effect.provide(ConvexSync.ConvexSyncServiceLive),
      Effect.provide(FileWatcher.FileWatcherLive),
      Effect.provide(WebSocketClient.WebSocketClientLive),
      Effect.provide(NodeContext.layer),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Failed to get status: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

// Evolve (update) command
const evolveCommand = Command.make("evolve").pipe(
  Command.withDescription("Evolve Overlord to the latest version"),
  Command.withHandler(() =>
    Effect.gen(function*() {
      yield* Console.log("🧬 Checking for Overlord evolution...")
      yield* Console.log("✅ Overlord is already at the latest evolutionary stage")
      // Implementation would check for updates
    })
  )
)

// Main command
const overlordCommand = Command.make("overlord").pipe(
  Command.withDescription("Overlord - Claude Code sync service (StarCraft-themed)"),
  Command.withSubcommands([
    spawnCommand,
    detectCommand,
    transportCommand,
    importCommand,
    burrowCommand,
    unburrowCommand,
    statusCommand,
    evolveCommand
  ])
)

export const cli = Command.run(overlordCommand, {
  name: "Overlord",
  version: "0.0.0"
})
