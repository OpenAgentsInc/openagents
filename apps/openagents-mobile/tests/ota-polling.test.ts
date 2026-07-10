import { describe, expect, test } from "bun:test"

import {
  type OtaUpdatesClient,
  startOtaPolling,
  TEMPORARY_OTA_POLL_INTERVAL_MS,
} from "../src/updates/ota-polling"

/**
 * OpenAgents mobile (#8597) OTA poll-loop contract. The loop drives the
 * injected `expo-updates` slice, so these tests exercise the REAL production
 * loop logic (scheduling, availability handling, soft errors, stop) with a
 * fake client and a tiny interval — no native host needed.
 */

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

interface FakeClientState {
  checks: number
  fetches: number
  reloads: number
}

const makeClient = (
  behavior: (state: FakeClientState) => {
    available?: boolean
    checkError?: boolean
  },
): { client: OtaUpdatesClient; state: FakeClientState } => {
  const state: FakeClientState = { checks: 0, fetches: 0, reloads: 0 }
  const client: OtaUpdatesClient = {
    isEnabled: true,
    checkForUpdateAsync: async () => {
      state.checks += 1
      const b = behavior(state)
      if (b.checkError === true) throw new Error("offline")
      return { isAvailable: b.available === true }
    },
    fetchUpdateAsync: async () => {
      state.fetches += 1
    },
    reloadAsync: async () => {
      state.reloads += 1
    },
  }
  return { client, state }
}

describe("contract openagents_mobile.ota.temporary_3s_poll.v1", () => {
  test("TEMPORARY cadence is exactly 3 seconds and marked temporary in source", async () => {
    expect(TEMPORARY_OTA_POLL_INTERVAL_MS).toBe(3000)
    // The aggressive cadence must stay explicitly marked as temporary so it
    // gets dialed down deliberately, not forgotten.
    const source = await Bun.file(
      new URL("../src/updates/ota-polling.ts", import.meta.url).pathname,
    ).text()
    expect(source).toContain("TEMPORARY CADENCE")
    expect(source).toContain("TEMPORARY_OTA_POLL_INTERVAL_MS")
  })

  test("polls repeatedly while no update is available", async () => {
    const { client, state } = makeClient(() => ({ available: false }))
    const handle = startOtaPolling(client, { intervalMs: 5 })
    await wait(40)
    handle.stop()
    expect(state.checks).toBeGreaterThanOrEqual(3)
    expect(state.fetches).toBe(0)
    expect(state.reloads).toBe(0)
  })

  test("on isAvailable: fetches then reloads exactly once and ends the loop", async () => {
    const { client, state } = makeClient((s) => ({ available: s.checks >= 2 }))
    const handle = startOtaPolling(client, { intervalMs: 5 })
    await wait(60)
    handle.stop()
    expect(state.fetches).toBe(1)
    expect(state.reloads).toBe(1)
    // Loop ended at reload — no further checks piled up afterwards.
    const checksAtReload = state.checks
    await wait(30)
    expect(state.checks).toBe(checksAtReload)
  })

  test("check errors are soft: the loop keeps polling after failures", async () => {
    const { client, state } = makeClient((s) => ({
      checkError: s.checks <= 2,
      available: false,
    }))
    const handle = startOtaPolling(client, { intervalMs: 5 })
    await wait(50)
    handle.stop()
    // Failed on checks 1-2, but polling continued well past them.
    expect(state.checks).toBeGreaterThanOrEqual(4)
    expect(state.reloads).toBe(0)
  })

  test("fires onUpdateReady before reloading", async () => {
    const order: Array<string> = []
    const { client } = makeClient(() => ({ available: true }))
    const wrapped: OtaUpdatesClient = {
      ...client,
      reloadAsync: async () => {
        order.push("reload")
      },
    }
    const handle = startOtaPolling(wrapped, {
      intervalMs: 5,
      onUpdateReady: () => order.push("ready"),
    })
    await wait(30)
    handle.stop()
    expect(order).toEqual(["ready", "reload"])
  })

  test("fetches, closes host-owned state, then reloads", async () => {
    const order: Array<string> = []
    const { client } = makeClient(() => ({ available: true }))
    const wrapped: OtaUpdatesClient = {
      ...client,
      fetchUpdateAsync: async () => {
        order.push("fetch")
      },
      reloadAsync: async () => {
        order.push("reload")
      },
    }
    const handle = startOtaPolling(wrapped, {
      intervalMs: 5,
      beforeReload: () => {
        order.push("close-sync")
      },
    })
    await wait(30)
    handle.stop()
    expect(order).toEqual(["fetch", "close-sync", "reload"])
  })

  test("no-op when updates are disabled (Expo Go / dev)", async () => {
    const { client, state } = makeClient(() => ({ available: true }))
    const disabled: OtaUpdatesClient = { ...client, isEnabled: false }
    const handle = startOtaPolling(disabled, { intervalMs: 5 })
    await wait(30)
    handle.stop()
    expect(state.checks).toBe(0)
  })

  test("stop() halts the loop", async () => {
    const { client, state } = makeClient(() => ({ available: false }))
    const handle = startOtaPolling(client, { intervalMs: 5 })
    await wait(25)
    handle.stop()
    const checksAtStop = state.checks
    await wait(30)
    expect(state.checks).toBe(checksAtStop)
  })
})
