import { describe, expect, test } from "bun:test"

import {
  fetchPylonStatus,
  pylonCountFromPublicPylonList,
  pylonsFromPublicPylonList,
} from "../src/shared/pylon-status.js"

describe("openagents desktop pylon status", () => {
  const publicPylonList = {
    pylons: [
      {
        codingCapacity: [
          { available: 2, busy: 1, queued: 0, ready: 2, service: "codex" },
          { available: 1, busy: 0, queued: 3, ready: 1, service: "claude" },
        ],
        latestHeartbeatDisplay: "Just now",
        latestHeartbeatStatus: "online",
        ownerAgentRef: "agent:owner-one",
        pylonRef: "pylon.one",
        status: "active",
      },
      {
        codingCapacity: [
          { available: 1, busy: 0, queued: 0, ready: 1, service: "codex" },
        ],
        latestHeartbeatDisplay: null,
        latestHeartbeatStatus: null,
        ownerAgentRef: "agent:other-owner",
        pylonRef: "pylon.two",
        status: "active",
      },
      {
        codingCapacity: [],
        latestHeartbeatDisplay: "4 minutes ago",
        latestHeartbeatStatus: "stale",
        ownerAgentRef: "agent:owner-one",
        pylonRef: "pylon.stale",
        status: "active",
      },
    ],
  }

  test("filters public pylons to the authenticated owner agent", () => {
    expect(pylonsFromPublicPylonList(publicPylonList, "agent:owner-one")).toEqual([
      {
        busySlots: 1,
        heartbeatFresh: true,
        latestHeartbeatAt: null,
        latestHeartbeatLabel: "Just now",
        ownerAgentRef: "agent:owner-one",
        pylonRef: "pylon.one",
        queuedSlots: 3,
        readySlots: 3,
        status: "online",
      },
      {
        busySlots: 0,
        heartbeatFresh: false,
        latestHeartbeatAt: null,
        latestHeartbeatLabel: "4 minutes ago",
        ownerAgentRef: "agent:owner-one",
        pylonRef: "pylon.stale",
        queuedSlots: 0,
        readySlots: 0,
        status: "stale",
      },
    ])
  })

  test("counts only online owned pylons", () => {
    expect(
      pylonCountFromPublicPylonList(publicPylonList, "agent:owner-one"),
    ).toBe(1)
  })

  test("falls back to zero for missing public pylon data", () => {
    expect(pylonCountFromPublicPylonList({}, "agent:owner-one")).toBe(0)
    expect(pylonsFromPublicPylonList({}, "agent:owner-one")).toEqual([])
  })

  test("uses product copy for account lookup failures", async () => {
    const fetchImpl = (async () =>
      new Response("{}", { status: 500 })) as unknown as typeof fetch

    const result = await fetchPylonStatus({
      fetch: fetchImpl,
      token: "test-token",
    })

    expect(result).toMatchObject({
      ok: false,
      error: "OpenAgents could not load your account right now.",
    })
    expect(result.ok === false ? result.error : "").not.toContain("/api/")
  })

  test("uses product copy for pylon list failures", async () => {
    const responses = [
      new Response(
        JSON.stringify({ agent: { user: { id: "user-one" } } }),
        { status: 200 },
      ),
      new Response("{}", { status: 500 }),
    ]
    const fetchImpl = (async () =>
      responses.shift() ??
      new Response("{}", { status: 500 })) as unknown as typeof fetch

    const result = await fetchPylonStatus({
      fetch: fetchImpl,
      token: "test-token",
    })

    expect(result).toMatchObject({
      ok: false,
      error: "OpenAgents could not load your pylons right now.",
    })
    expect(result.ok === false ? result.error : "").not.toContain("/api/")
  })
})
