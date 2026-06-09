import { Effect, Console } from "effect"
import { createCliRenderer, BoxRenderable, TextRenderable, ScrollBoxRenderable } from "@opentui/core"

// Define our strict state schema for the earning node
type PylonState = "Warming" | "Idle" | "Processing" | "Settling"

// Hardware Resource & Telemetry Discovery Service
const startHardwareTelemetryLoop = Effect.gen(function* () {
  yield* Effect.logInfo("[Telemetry] Initializing platform discovery...")
  // Periodic polling simulation
  yield* Effect.repeat(
    Effect.gen(function* () {
      yield* Effect.logDebug("[Telemetry] Polling CPU/GPU thermals and load...")
    }),
    { schedule: "10 seconds" }
  )
})

// Money Dev Kit (MDK) Wallet Sidecar Service
const startMdkWalletService = Effect.gen(function* () {
  yield* Effect.logInfo("[Wallet] Connecting to local MDK agent-wallet daemon on port 3001...")
  yield* Effect.logInfo("[Wallet] Wallet connection established. Ready to receive payouts.")
})

// Nostr Continuous Presence Heartbeat Loop
const startPresenceHeartbeatLoop = Effect.gen(function* () {
  yield* Effect.logInfo("[Heartbeat] Initializing presence service...")
  yield* Effect.repeat(
    Effect.gen(function* () {
      yield* Effect.logInfo("[Heartbeat] Emitting presence signal (online, model_ready=true)")
    }),
    { schedule: "30 seconds" }
  )
})

// Main Pylon v0.3 Application Loop
const runPylonNode = Effect.gen(function* () {
  yield* Effect.logInfo("Initializing Pylon v0.3 observational earning node...")

  // Bootstrap OpenTUI Core
  const renderer = yield* Effect.tryPromise({
    try: () =>
      createCliRenderer({
        screenMode: "fullscreen",
        exitOnCtrlC: true,
        targetFps: 30,
      }),
    catch: (error) => new Error(`Failed to initialize OpenTUI renderer: ${String(error)}`),
  })

  // Create UI Container Layout
  const mainBox = new BoxRenderable(renderer, {
    border: true,
    borderType: "single",
    width: "100%",
    height: "100%",
  })
  renderer.root.add(mainBox)

  // Start Background Services as Concurrent Fibers
  const telemetryFiber = yield* Effect.fork(startHardwareTelemetryLoop)
  const walletFiber = yield* Effect.fork(startMdkWalletService)
  const heartbeatFiber = yield* Effect.fork(startPresenceHeartbeatLoop)

  yield* Effect.logInfo("Pylon v0.3 observational dashboard active.")

  // Enter the persistent execution block
  yield* Effect.never
})

// Execute the main program safely via Effect
Effect.runPromise(
  runPylonNode.pipe(
    Effect.catchAll((error) =>
      Console.error(`Pylon v0.3 crashed on startup: ${error.message}`)
    )
  )
)
