import { describe, expect, test } from "bun:test"
import { Effect, PubSub, SubscriptionRef } from "effect"
import { Fiber } from "effect"
import {
  heartbeatServiceLoop,
  logMessage,
  makePylonNodeRuntime,
  setTelemetry,
  setWalletStatus,
  telemetryPollOnce,
  walletPollOnce,
} from "../src/node/runtime"
import { PresenceRequestError } from "../src/presence-error"

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
          const unavailable = feed.find((entry) => entry.message.includes("Primary wallet status unavailable"))
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

describe("heartbeat auto-register payout target (#5305)", () => {
  test("re-registers and retries once when a long-running heartbeat gets a 401 (#6236)", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          let registerCalls = 0
          let heartbeatCalls = 0
          let resolveRecovered: (() => void) | undefined
          const recovered = new Promise<void>((resolve) => {
            resolveRecovered = resolve
          })
          const fiber = yield* Effect.forkScoped(
            heartbeatServiceLoop(runtime, {
              baseUrl: "https://openagents.test",
              register: async () => {
                registerCalls += 1
                return { ok: true }
              },
              heartbeat: async () => {
                heartbeatCalls += 1
                if (heartbeatCalls === 1) {
                  throw new PresenceRequestError(401, '{"error":"unauthorized"}')
                }
                resolveRecovered?.()
                return { ok: true }
              },
              intervalMs: 1_000,
            }),
          )
          yield* Effect.promise(() =>
            Promise.race([
              recovered,
              new Promise<void>((_resolve, reject) =>
                setTimeout(() => reject(new Error("heartbeat auth recovery timed out")), 500),
              ),
            ]),
          )
          yield* Fiber.interrupt(fiber)

          expect(registerCalls).toBe(2)
          expect(heartbeatCalls).toBe(2)
          const feed = yield* SubscriptionRef.get(runtime.logFeed)
          expect(feed.some((entry) => entry.message.includes("Heartbeat blocked"))).toBe(false)
        }),
      ),
    )
  })

  test("invokes ensurePayoutTarget after a successful register, then again each cycle (idempotent closure)", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          let registerCalls = 0
          let heartbeatCalls = 0
          let ensureCalls = 0
          // The closure is self-idempotent: it "registers" only once and then
          // self-skips on later cycles (mirroring the presence-state check).
          let alreadyRegistered = false
          let actualRegistrations = 0
          const fiber = yield* Effect.forkScoped(
            heartbeatServiceLoop(runtime, {
              baseUrl: "https://openagents.test",
              register: async () => {
                registerCalls += 1
                return { ok: true }
              },
              heartbeat: async () => {
                heartbeatCalls += 1
                return { ok: true }
              },
              ensurePayoutTarget: async () => {
                ensureCalls += 1
                if (!alreadyRegistered) {
                  actualRegistrations += 1
                  alreadyRegistered = true
                }
              },
              intervalMs: 5,
            }),
          )
          // Let the loop run a few cycles, then interrupt.
          yield* Effect.sleep("40 millis")
          yield* Fiber.interrupt(fiber)
          expect(registerCalls).toBe(1)
          // ensurePayoutTarget runs post-register AND every heartbeat cycle.
          expect(ensureCalls).toBeGreaterThanOrEqual(2)
          // ...but the idempotent closure only registers ONCE (second boot /
          // later cycle does not re-register).
          expect(actualRegistrations).toBe(1)
          expect(heartbeatCalls).toBeGreaterThanOrEqual(1)
        }),
      ),
    )
  })

  test("a throwing ensurePayoutTarget is fail-soft — heartbeat keeps beating", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          let heartbeatCalls = 0
          const fiber = yield* Effect.forkScoped(
            heartbeatServiceLoop(runtime, {
              baseUrl: "https://openagents.test",
              register: async () => ({ ok: true }),
              heartbeat: async () => {
                heartbeatCalls += 1
                return { ok: true }
              },
              // Contract: this closure NEVER throws. But even if a bug made it
              // throw, Effect.promise turns it into a defect; the heartbeat must
              // still beat. We simulate the worst case here.
              ensurePayoutTarget: async () => {
                throw new Error("register endpoint down")
              },
              intervalMs: 5,
            }),
          )
          // Let the loop run several cycles; a throwing hook each cycle must
          // not crash the loop.
          yield* Effect.sleep("40 millis")
          const beatsBefore = heartbeatCalls
          yield* Fiber.interrupt(fiber)
          // The loop survived the throwing hook across multiple cycles: more
          // than one heartbeat fired despite ensurePayoutTarget throwing.
          expect(beatsBefore).toBeGreaterThanOrEqual(2)
        }),
      ),
    )
  })

  test("no ensurePayoutTarget hook leaves the loop unchanged", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          let heartbeatCalls = 0
          const fiber = yield* Effect.forkScoped(
            heartbeatServiceLoop(runtime, {
              baseUrl: "https://openagents.test",
              register: async () => ({ ok: true }),
              heartbeat: async () => {
                heartbeatCalls += 1
                return { ok: true }
              },
              intervalMs: 5,
            }),
          )
          yield* Effect.sleep("20 millis")
          yield* Fiber.interrupt(fiber)
          expect(heartbeatCalls).toBeGreaterThanOrEqual(1)
        }),
      ),
    )
  })
})
