import { describe, expect, test } from "bun:test"

import { openPylonFleetRunIntakePoller } from "../src/node/fleet-run-intake-poller.js"
import type {
  PylonFleetRunRemoteIntakeProjection,
  PylonFleetRunRemoteIntakeService,
} from "../src/orchestration/fleet-run-remote-intake.js"

const active: PylonFleetRunRemoteIntakeProjection = {
  schema: "openagents.pylon.fleet_run_remote_intake.v1",
  pylonRef: "pylon.public.poller",
  runRef: "fleet_run.sarah.0123456789abcdef0123",
  state: "active",
  retryable: false,
  blockerRefs: [],
}

const deferred = <A>() => {
  let resolve!: (value: A) => void
  const promise = new Promise<A>(done => {
    resolve = done
  })
  return { promise, resolve }
}

describe("standing Pylon FleetRun intake poller", () => {
  test("polls immediately, serializes overlapping wakes, and closes cleanly", async () => {
    const gate = deferred<PylonFleetRunRemoteIntakeProjection>()
    let calls = 0
    let closes = 0
    let timerCallback: (() => void) | undefined
    const intake: PylonFleetRunRemoteIntakeService = {
      runOnce: async () => {
        calls += 1
        return gate.promise
      },
      reconcile: async () => active,
      close: async () => {
        closes += 1
      },
    }
    const poller = openPylonFleetRunIntakePoller({
      intake,
      intervalMs: 250,
      setTimer: callback => {
        timerCallback = callback
        return 1 as unknown as ReturnType<typeof setTimeout>
      },
      clearTimer: () => undefined,
    })
    expect(calls).toBe(1)
    expect(poller.status()).toMatchObject({ state: "polling", pollCount: 1 })
    const sameA = poller.runNow()
    const sameB = poller.runNow()
    expect(sameA).toBe(sameB)
    expect(calls).toBe(1)

    const closing = poller.close()
    expect(closes).toBe(0)
    gate.resolve(active)
    await closing
    expect(closes).toBe(1)
    expect(poller.status()).toMatchObject({
      state: "closed",
      pollCount: 1,
      lastProjection: active,
    })
    expect(timerCallback).toBeUndefined()
  })

  test("schedules the next tick only after settlement and never overlaps", async () => {
    const gates = [deferred<PylonFleetRunRemoteIntakeProjection>(), deferred<PylonFleetRunRemoteIntakeProjection>()]
    const callbacks: Array<() => void> = []
    let calls = 0
    const intake: PylonFleetRunRemoteIntakeService = {
      runOnce: async () => gates[calls++]!.promise,
      reconcile: async () => active,
      close: async () => undefined,
    }
    const poller = openPylonFleetRunIntakePoller({
      intake,
      intervalMs: 250,
      setTimer: callback => {
        callbacks.push(callback)
        return callbacks.length as unknown as ReturnType<typeof setTimeout>
      },
      clearTimer: () => undefined,
    })
    expect(callbacks).toEqual([])
    gates[0]!.resolve(active)
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(callbacks).toHaveLength(1)
    callbacks.shift()!()
    expect(calls).toBe(2)
    expect(callbacks).toEqual([])
    callbacks[0]?.()
    expect(calls).toBe(2)
    gates[1]!.resolve(active)
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(callbacks).toHaveLength(1)
    await poller.close()
  })

  test("projects only a fixed blocker after intake failure and keeps polling", async () => {
    let calls = 0
    const intake: PylonFleetRunRemoteIntakeService = {
      runOnce: async () => {
        calls += 1
        throw new Error("postgres://operator:secret@private-host")
      },
      reconcile: async () => active,
      close: async () => undefined,
    }
    const poller = openPylonFleetRunIntakePoller({
      intake,
      intervalMs: 250,
      startImmediately: false,
      setTimer: () => 1 as unknown as ReturnType<typeof setTimeout>,
      clearTimer: () => undefined,
    })
    await poller.runNow()
    const status = poller.status()
    expect(calls).toBe(1)
    expect(status.blockerRefs).toEqual([
      "blocker.pylon.fleet_run_intake.poll_failed",
    ])
    expect(JSON.stringify(status)).not.toContain("private-host")
    expect(JSON.stringify(status)).not.toContain("secret")
    await poller.close()
  })
})
