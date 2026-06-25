// Pylon node runtime: service-owned state behind SubscriptionRefs plus a
// typed PylonEvent PubSub (issue #4736). Services update state through the
// setters here and never touch a renderable; the TUI subscribes to refs and
// events and never runs business logic. Loops are forked into the caller's
// Scope so Ctrl+C interrupts them instead of orphaning them.

import { Effect, PubSub, SubscriptionRef } from "effect"
import {
  appendLogEntry,
  initialOperatorPaneState,
  initialTelemetryPaneState,
  initialWalletPaneState,
  telemetryPaneStateFromInventory,
  walletPaneStateFromStatus,
  walletTransitionMessage,
  type OperatorPaneState,
  type PylonEvent,
  type PylonLogEntry,
  type PylonLogLevel,
  type TelemetryInventoryInput,
  type TelemetryPaneState,
  type WalletPaneState,
  type WalletStatusInput,
} from "./state.js"
import { isPresenceUnauthorizedError } from "../presence-error.js"

export interface PylonNodeRuntime {
  readonly wallet: SubscriptionRef.SubscriptionRef<WalletPaneState>
  readonly telemetry: SubscriptionRef.SubscriptionRef<TelemetryPaneState>
  readonly operator: SubscriptionRef.SubscriptionRef<OperatorPaneState>
  readonly logFeed: SubscriptionRef.SubscriptionRef<ReadonlyArray<PylonLogEntry>>
  readonly events: PubSub.PubSub<PylonEvent>
}

export const makePylonNodeRuntime: Effect.Effect<PylonNodeRuntime> = Effect.gen(function* () {
  const wallet = yield* SubscriptionRef.make(initialWalletPaneState)
  const telemetry = yield* SubscriptionRef.make(initialTelemetryPaneState)
  const operator = yield* SubscriptionRef.make(initialOperatorPaneState)
  const logFeed = yield* SubscriptionRef.make<ReadonlyArray<PylonLogEntry>>([])
  const events = yield* PubSub.unbounded<PylonEvent>()
  return { wallet, telemetry, operator, logFeed, events }
})

export const logMessage = (
  runtime: PylonNodeRuntime,
  level: PylonLogLevel,
  message: string,
  options?: { transient?: boolean },
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const entry: PylonLogEntry = {
      at: new Date().toISOString(),
      level,
      message,
      ...(options?.transient ? { transient: true } : {}),
    }
    yield* SubscriptionRef.update(runtime.logFeed, (entries) => appendLogEntry(entries, entry))
    yield* PubSub.publish(runtime.events, { type: "log", ...entry })
  })

export const setWalletStatus = (
  runtime: PylonNodeRuntime,
  status: WalletStatusInput | null,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const previous = yield* SubscriptionRef.get(runtime.wallet)
    const next = walletPaneStateFromStatus(status)
    yield* SubscriptionRef.set(runtime.wallet, next)
    yield* PubSub.publish(runtime.events, { type: "wallet", at: new Date().toISOString(), wallet: next })
    const transition = walletTransitionMessage(previous, next)
    if (transition) {
      yield* logMessage(runtime, "verbose", transition)
    }
  })

export const setTelemetry = (
  runtime: PylonNodeRuntime,
  inventory: TelemetryInventoryInput | null,
  psionicPhase: string,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const next = telemetryPaneStateFromInventory(inventory, psionicPhase)
    yield* SubscriptionRef.set(runtime.telemetry, next)
    yield* PubSub.publish(runtime.events, { type: "telemetry", at: new Date().toISOString(), telemetry: next })
  })

export const setOperatorText = (
  runtime: PylonNodeRuntime,
  text: string,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    yield* SubscriptionRef.set(runtime.operator, { text })
    yield* PubSub.publish(runtime.events, { type: "operator", at: new Date().toISOString(), text })
  })

// --- Service loops --------------------------------------------------------
// Dependencies are injected so the poll bodies are testable without the real
// daemon, inventory probe, or network.

export interface WalletServiceDeps {
  classify: () => Promise<WalletStatusInput | null>
  intervalMs?: number
}

export const walletPollOnce = (
  runtime: PylonNodeRuntime,
  classify: WalletServiceDeps["classify"],
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const status = yield* Effect.promise(async () => {
      try {
        return await classify()
      } catch (error) {
        return { error: String(error) } as const
      }
    })
    if (status && "error" in status) {
      yield* logMessage(runtime, "verbose", `[Wallet] Primary wallet status unavailable: ${status.error}`)
      yield* setWalletStatus(runtime, null)
      return
    }
    yield* setWalletStatus(runtime, status)
  })

export const walletServiceLoop = (
  runtime: PylonNodeRuntime,
  deps: WalletServiceDeps,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    yield* logMessage(runtime, "verbose", "[Wallet] Connecting to primary agent wallet...")
    while (true) {
      yield* walletPollOnce(runtime, deps.classify)
      yield* Effect.sleep(`${deps.intervalMs ?? 10_000} millis`)
    }
  })

export interface TelemetryServiceDeps<Inventory extends TelemetryInventoryInput = TelemetryInventoryInput> {
  discoverInventory: () => Promise<Inventory>
  inspectPsionic: () => Promise<{ phase: string }>
  makeOperatorText: (inventory: Inventory) => string
  intervalMs?: number
}

export const telemetryPollOnce = <Inventory extends TelemetryInventoryInput>(
  runtime: PylonNodeRuntime,
  deps: TelemetryServiceDeps<Inventory>,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const inventory = yield* Effect.promise(async () => {
      try {
        return await deps.discoverInventory()
      } catch (error) {
        return { error: String(error) } as const
      }
    })
    if ("error" in inventory) {
      yield* logMessage(runtime, "verbose", `[Telemetry] Inventory unavailable: ${inventory.error}`)
      yield* setTelemetry(runtime, null, "unknown")
      return
    }
    yield* setTelemetry(runtime, inventory, "checking")
    yield* setOperatorText(runtime, deps.makeOperatorText(inventory))
    const psionic = yield* Effect.promise(async () => {
      try {
        return await deps.inspectPsionic()
      } catch {
        return { phase: "unknown" }
      }
    })
    yield* setTelemetry(runtime, inventory, psionic.phase)
  })

export const telemetryServiceLoop = <Inventory extends TelemetryInventoryInput>(
  runtime: PylonNodeRuntime,
  deps: TelemetryServiceDeps<Inventory>,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    yield* logMessage(runtime, "verbose", "[Telemetry] Platform discovery initialized.")
    while (true) {
      yield* telemetryPollOnce(runtime, deps)
      yield* Effect.sleep(`${deps.intervalMs ?? 10_000} millis`)
    }
  })

export interface HeartbeatServiceDeps {
  baseUrl: string | undefined
  register: () => Promise<unknown>
  heartbeat: () => Promise<unknown>
  // #5305: best-effort, idempotent, fail-soft auto-registration of this node's
  // OWN Spark address as a payout target. Invoked once after a successful
  // presence-register (first-online) and then re-attempted each heartbeat cycle
  // until it succeeds (it self-skips once registered). It MUST NOT throw — a
  // failure here can never block or fail presence/heartbeat.
  ensurePayoutTarget?: () => Promise<void>
  intervalMs?: number
}

export const heartbeatServiceLoop = (
  runtime: PylonNodeRuntime,
  deps: HeartbeatServiceDeps,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    if (!deps.baseUrl) {
      yield* logMessage(
        runtime,
        "verbose",
        "[Heartbeat] No OpenAgents base URL configured. Presence remains unregistered.",
      )
      return
    }
    yield* logMessage(runtime, "verbose", "[Heartbeat] Presence service initialized.")
    const registered = yield* Effect.promise(async () => {
      try {
        await deps.register()
        return true
      } catch (error) {
        return String(error)
      }
    })
    if (registered !== true) {
      yield* logMessage(runtime, "info", `[Heartbeat] Registration blocked: ${registered}`)
    }
    // #5305: auto-register this node's own Spark address as a payout target once
    // it is online. Fail-soft + idempotent: a failure never blocks the loop and
    // a later heartbeat cycle re-attempts until it succeeds.
    if (registered === true && deps.ensurePayoutTarget) {
      // Fail-soft at BOTH layers: the closure swallows its own errors, AND we
      // swallow any accidental rejection here so a payout-target failure can
      // never take the presence/heartbeat loop down.
      yield* Effect.promise(async () => {
        try {
          await deps.ensurePayoutTarget!()
        } catch {
          // ignore — retried next cycle
        }
      })
    }
    while (true) {
      const sent = yield* Effect.promise(async () => {
        try {
          await deps.heartbeat()
          return true
        } catch (error) {
          if (isPresenceUnauthorizedError(error)) {
            try {
              await deps.register()
              await deps.heartbeat()
              return true
            } catch (retryError) {
              return String(retryError)
            }
          }
          return String(error)
        }
      })
      if (sent !== true) {
        yield* logMessage(runtime, "verbose", `[Heartbeat] Heartbeat blocked: ${sent}`)
      }
      // #5305: re-attempt the fail-soft payout-target auto-register every cycle
      // until it lands (the closure self-skips once the presence state records
      // the digest ref). Never throws, so it cannot take the heartbeat down.
      if (deps.ensurePayoutTarget) {
        yield* Effect.promise(async () => {
          try {
            await deps.ensurePayoutTarget!()
          } catch {
            // ignore — retried next cycle
          }
        })
      }
      yield* Effect.sleep(`${deps.intervalMs ?? 30_000} millis`)
    }
  })

export interface PylonNodeServiceDeps<Inventory extends TelemetryInventoryInput = TelemetryInventoryInput> {
  wallet: WalletServiceDeps
  telemetry: TelemetryServiceDeps<Inventory>
  heartbeat: HeartbeatServiceDeps
}

// Forks a loop into the caller's Scope. A loop that dies on a failure or
// defect logs an error instead of vanishing silently; closing the Scope
// (Ctrl+C, smoke exit) interrupts it.
export const superviseLoop = (
  runtime: PylonNodeRuntime,
  name: string,
  loop: Effect.Effect<void, unknown>,
) =>
  Effect.forkScoped(
    loop.pipe(
      Effect.catchCause((cause) =>
        logMessage(runtime, "error", `[${name}] Service stopped with error: ${String(cause)}`),
      ),
    ),
  )

export const forkNodeServices = <Inventory extends TelemetryInventoryInput>(
  runtime: PylonNodeRuntime,
  deps: PylonNodeServiceDeps<Inventory>,
) =>
  Effect.gen(function* () {
    yield* superviseLoop(runtime, "Telemetry", telemetryServiceLoop(runtime, deps.telemetry))
    yield* superviseLoop(runtime, "Wallet", walletServiceLoop(runtime, deps.wallet))
    yield* superviseLoop(runtime, "Heartbeat", heartbeatServiceLoop(runtime, deps.heartbeat))
  })

// Prepends persisted log entries (issue #4739) to the feed ref without
// publishing events - restored scrollback is replayed by the view bridge,
// not re-persisted or re-announced.
export const seedLogFeed = (
  runtime: PylonNodeRuntime,
  entries: ReadonlyArray<PylonLogEntry>,
): Effect.Effect<void> =>
  SubscriptionRef.update(runtime.logFeed, (current) => [...entries, ...current])

// Forks a scoped consumer that appends every live log event to the durable
// feed log. Subscribed before services fork so no entry is missed.
export const forkLogPersistence = (
  runtime: PylonNodeRuntime,
  writer: { append: (entry: PylonLogEntry) => Promise<void> },
) =>
  Effect.gen(function* () {
    const subscription = yield* PubSub.subscribe(runtime.events)
    yield* Effect.forkScoped(
      Effect.gen(function* () {
        while (true) {
          const event = yield* PubSub.take(subscription)
          if (event.type === "log") {
            yield* Effect.promise(() =>
              writer.append({ at: event.at, level: event.level, message: event.message }),
            )
          }
        }
      }),
    )
  })

// --- Attach-mode mirroring (issue #4740) ------------------------------------
// An attached TUI runs its own local PylonNodeRuntime as a mirror of the
// remote node: snapshots set the pane refs, and remote events are re-applied
// so the same view bridge renders them.

export const applyRemotePanes = (
  runtime: PylonNodeRuntime,
  snapshot: { wallet: WalletPaneState; telemetry: TelemetryPaneState; operatorText: string },
): Effect.Effect<void> =>
  Effect.gen(function* () {
    yield* SubscriptionRef.set(runtime.wallet, snapshot.wallet)
    yield* SubscriptionRef.set(runtime.telemetry, snapshot.telemetry)
    yield* SubscriptionRef.set(runtime.operator, { text: snapshot.operatorText })
  })

export const applyRemoteEvent = (
  runtime: PylonNodeRuntime,
  event: PylonEvent,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    switch (event.type) {
      case "log": {
        const entry: PylonLogEntry = { at: event.at, level: event.level, message: event.message }
        yield* SubscriptionRef.update(runtime.logFeed, (entries) => appendLogEntry(entries, entry))
        yield* PubSub.publish(runtime.events, event)
        return
      }
      case "wallet":
        yield* SubscriptionRef.set(runtime.wallet, event.wallet)
        return
      case "telemetry":
        yield* SubscriptionRef.set(runtime.telemetry, event.telemetry)
        return
      case "operator":
        yield* SubscriptionRef.set(runtime.operator, { text: event.text })
        return
    }
  })

export const publishLogEntries = (
  runtime: PylonNodeRuntime,
  entries: ReadonlyArray<PylonLogEntry>,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    for (const entry of entries) {
      yield* PubSub.publish(runtime.events, { type: "log", ...entry })
    }
  })
