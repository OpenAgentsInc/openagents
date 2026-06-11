import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect, Deferred } from "effect"
import { logMessage, makePylonNodeRuntime, setWalletStatus } from "../src/node/runtime"
import {
  captureNodeSnapshot,
  ensureControlToken,
  startControlServer,
  type PylonSnapshot,
} from "../src/node/control-server"
import {
  consumeSseBuffer,
  nextBackoffMs,
  runControlClient,
  sendControlCommand,
} from "../src/node/control-client"
import type { PylonEvent } from "../src/node/state"

const stubActions = (calls: Array<Record<string, unknown>>) => ({
  walletSend: async (destinationRef: string, amountSats?: number) => {
    calls.push({ type: "send", destinationRef, amountSats })
    return { dispatched: true }
  },
  walletReceive: async (amountSats: number) => {
    calls.push({ type: "receive", amountSats })
    return { invoice: "lnbc-test" }
  },
  walletAdmitPayoutTarget: async (kind: string, ref: string) => {
    calls.push({ type: "admit", kind, ref })
    return { admitted: true }
  },
})

describe("control protocol", () => {
  test("token file is created once with stable content", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-token-"))
    const first = await ensureControlToken(dir)
    const second = await ensureControlToken(dir)
    expect(first).toBe(second)
    expect(first.length).toBeGreaterThanOrEqual(32)
  })

  test("snapshot captures panes and the log tail", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const runtime = yield* makePylonNodeRuntime
        yield* setWalletStatus(runtime, { daemonOnline: true, balanceSats: 21, readiness: "receive-ready" })
        yield* logMessage(runtime, "info", "hello snapshot")
        const snapshot = yield* captureNodeSnapshot(runtime)
        expect(snapshot.wallet.balanceSats).toBe(21)
        expect(snapshot.logFeed.some((entry) => entry.message === "hello snapshot")).toBe(true)
      }),
    )
  })

  test("attach round trip: snapshot, live events, auth, and money commands", async () => {
    const calls: Array<Record<string, unknown>> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* logMessage(runtime, "info", "pre-attach line")
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions(calls),
            port: 0,
          })

          // Unauthorized requests are rejected.
          const bad = yield* Effect.promise(() =>
            fetch(`${server.url}/events`, { headers: { authorization: "Bearer wrong" } }),
          )
          expect(bad.status).toBe(401)

          // Client: snapshot then live tail.
          const snapshotReceived = yield* Deferred.make<PylonSnapshot>()
          const liveReceived = yield* Deferred.make<PylonEvent>()
          yield* runControlClient(server.url, "test-token-0123456789abcdef", {
            onSnapshot: (snapshot) => {
              Effect.runFork(Deferred.succeed(snapshotReceived, snapshot))
            },
            onEvent: (event) => {
              if (event.type === "log" && event.message === "live line") {
                Effect.runFork(Deferred.succeed(liveReceived, event))
              }
            },
            onStatus: () => {},
          })

          const snapshot = yield* Deferred.await(snapshotReceived)
          expect(snapshot.logFeed.some((entry) => entry.message === "pre-attach line")).toBe(true)

          yield* logMessage(runtime, "info", "live line")
          const live = yield* Deferred.await(liveReceived)
          expect(live.type).toBe("log")

          // Money command round-trips and executes node-side.
          const result = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.send",
              destinationRef: "lno-test-offer",
              amountSats: 42,
            }),
          )
          expect(result).toEqual({ dispatched: true })
          expect(calls).toContainEqual({ type: "send", destinationRef: "lno-test-offer", amountSats: 42 })

          // Bad command surfaces a typed error.
          const bogus = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.bogus",
            } as never).then(
              () => null,
              (error: unknown) => (error instanceof Error ? error.message : String(error)),
            ),
          )
          expect(bogus).toMatch(/unknown command/)
        }),
      ),
    )
  })

  test("assignments commands round-trip and report unavailability", async () => {
    const calls: Array<Record<string, unknown>> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions(calls),
              assignmentsPoll: async () => [{ leaseRef: "lease-1", goal: "g", assignmentRef: "a", paymentMode: "no-spend", expiresAt: "x" }],
              assignmentsAccept: async (leaseRef: string) => {
                calls.push({ type: "accept", leaseRef })
                return { accepted: true }
              },
            },
            port: 0,
          })
          const leases = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "assignments.poll" }),
          )
          expect(Array.isArray(leases)).toBe(true)
          yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "assignments.accept", leaseRef: "lease-1" }),
          )
          expect(calls).toContainEqual({ type: "accept", leaseRef: "lease-1" })

          // A node without assignment actions reports unavailability.
          const bare = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions(calls),
            port: 0,
          })
          const failure = yield* Effect.promise(() =>
            sendControlCommand(bare.url, "test-token-0123456789abcdef", { type: "assignments.poll" }).then(
              () => null,
              (error: unknown) => (error instanceof Error ? error.message : String(error)),
            ),
          )
          expect(failure).toMatch(/unavailable/)
        }),
      ),
    )
  })

  test("backoff doubles to a 30s ceiling", () => {
    expect(nextBackoffMs(0)).toBe(1000)
    expect(nextBackoffMs(1000)).toBe(2000)
    expect(nextBackoffMs(16000)).toBe(30000)
    expect(nextBackoffMs(30000)).toBe(30000)
  })

  test("SSE buffer parser handles partial frames and comments", () => {
    const seen: string[] = []
    let rest = consumeSseBuffer('data: {"a":1}\n\n: ping\n\ndata: {"b"', (payload) => seen.push(payload))
    expect(seen).toEqual(['{"a":1}'])
    expect(rest).toBe('data: {"b"')
    rest = consumeSseBuffer(`${rest}:2}\n\n`, (payload) => seen.push(payload))
    expect(seen).toEqual(['{"a":1}', '{"b":2}'])
    expect(rest).toBe("")
  })
})
