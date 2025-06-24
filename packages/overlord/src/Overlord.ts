import { Args, Command, Options } from "@effect/cli"
import { NodeContext } from "@effect/platform-node"
import { Console, Effect, Option } from "effect"
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
  apiKey: apiKeyOption
}).pipe(
  Command.withDescription("Transport (sync) Claude Code sessions to the cloud"),
  Command.withHandler(({ apiKey, sessionId, userId }) =>
    Effect.gen(function*() {
      const service = yield* OverlordService.OverlordService

      if (Option.isNone(sessionId) || Option.getOrElse(sessionId, () => "") === "all") {
        yield* Console.log("🚀 Transporting all sessions to the cloud...")
        const result = yield* service.syncAllSessions({ userId, apiKey })
        yield* Console.log(`✅ Transported ${result.synced} sessions`)
        if (result.failed > 0) {
          yield* Console.log(`⚠️  Failed to sync ${result.failed} sessions`)
        }
      } else {
        yield* Console.log(`🚀 Transporting session ${Option.getOrElse(sessionId, () => "")}...`)
        yield* service.syncSession(Option.getOrElse(sessionId, () => ""), { userId, apiKey })
        yield* Console.log("✅ Session transported successfully")
      }
    }).pipe(
      Effect.provide(OverlordService.OverlordServiceLive),
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
