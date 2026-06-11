import { describe, expect, test } from "bun:test"
import { Effect, PubSub, SubscriptionRef } from "effect"
import {
  logMessage,
  makePylonNodeRuntime,
  setTelemetry,
  setWalletStatus,
  telemetryPollOnce,
  walletPollOnce,
} from "../src/node/runtime"

describe("pylon node runtime", () => {
  test("logMessage appends to the feed and publishes a log event", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const subscription = yield* PubSub.subscribe(runtime.events)
          yield* logMessage(runtime, "info", "hello world")
          const feed = yield* SubscriptionRef.get(runtime.logFeed)
          expect(feed).toHaveLength(1)
          expect(feed[0]?.message).toBe("hello world")
          expect(feed[0]?.level).toBe("info")
          const event = yield* PubSub.take(subscription)
          expect(event.type).toBe("log")
          if (event.type === "log") expect(event.message).toBe("hello world")
        }),
      ),
    )
  })

  test("setWalletStatus updates the ref, publishes, and logs the online edge once", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const subscription = yield* PubSub.subscribe(runtime.events)
          const online = { daemonOnline: true, balanceSats: 99, readiness: "receive-ready" }
          yield* setWalletStatus(runtime, online)
          yield* setWalletStatus(runtime, online)
          const state = yield* SubscriptionRef.get(runtime.wallet)
          expect(state.balanceSats).toBe(99)
          const events = yield* PubSub.takeUpTo(subscription, 100)
          const walletEvents = events.filter((event) => event.type === "wallet")
          const transitionLogs = events.filter((event) => event.type === "log")
          expect(walletEvents).toHaveLength(2)
          expect(transitionLogs).toHaveLength(1)
          expect(transitionLogs[0]?.type === "log" && transitionLogs[0].message).toContain("connected")
        }),
      ),
    )
  })

  test("setTelemetry updates the ref and publishes a telemetry event", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const subscription = yield* PubSub.subscribe(runtime.events)
          yield* setTelemetry(
            runtime,
            {
              eligibleInventoryCount: 1,
              accelerator: { vramGb: 8 },
              backendHealth: [{ state: "ready", modelRef: "model.demo" }],
            },
            "configured",
          )
          const state = yield* SubscriptionRef.get(runtime.telemetry)
          expect(state.state).toBe("INVENTORY FRESH")
          expect(state.model).toBe("model.demo")
          const event = yield* PubSub.take(subscription)
          expect(event.type).toBe("telemetry")
        }),
      ),
    )
  })

  test("walletPollOnce treats a throwing classifier as offline without failing", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* setWalletStatus(runtime, { daemonOnline: true, balanceSats: 10, readiness: "receive-ready" })
          yield* walletPollOnce(runtime, async () => {
            throw new Error("daemon gone")
          })
          const state = yield* SubscriptionRef.get(runtime.wallet)
          expect(state.daemonOnline).toBe(false)
          expect(state.balanceSats).toBeNull()
          const feed = yield* SubscriptionRef.get(runtime.logFeed)
          const unavailable = feed.find((entry) => entry.message.includes("MDK status unavailable"))
          expect(unavailable?.level).toBe("verbose")
        }),
      ),
    )
  })

  test("telemetryPollOnce publishes inventory state, operator text, then psionic phase", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* telemetryPollOnce(runtime, {
            discoverInventory: async () => ({
              eligibleInventoryCount: 3,
              accelerator: { vramGb: 16 },
              backendHealth: [{ state: "configured", modelRef: "model.live" }],
            }),
            inspectPsionic: async () => ({ phase: "configured" }),
            makeOperatorText: () => "Operate: automated",
          })
          const telemetry = yield* SubscriptionRef.get(runtime.telemetry)
          expect(telemetry.psionic).toBe("configured")
          expect(telemetry.model).toBe("model.live")
          const operator = yield* SubscriptionRef.get(runtime.operator)
          expect(operator.text).toBe("Operate: automated")
        }),
      ),
    )
  })

  test("telemetryPollOnce degrades to UNAVAILABLE when discovery throws", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* telemetryPollOnce(runtime, {
            discoverInventory: async () => {
              throw new Error("probe exploded")
            },
            inspectPsionic: async () => ({ phase: "configured" }),
            makeOperatorText: () => "unused",
          })
          const telemetry = yield* SubscriptionRef.get(runtime.telemetry)
          expect(telemetry.state).toBe("UNAVAILABLE")
          const feed = yield* SubscriptionRef.get(runtime.logFeed)
          expect(feed.some((entry) => entry.message.includes("Inventory unavailable"))).toBe(true)
        }),
      ),
    )
  })
})
