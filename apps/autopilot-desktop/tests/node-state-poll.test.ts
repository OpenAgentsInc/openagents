import { describe, expect, test } from "bun:test"
import {
  CONTROL_SCHEMA_TAG,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import { sessionListFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"
import { createNodeStatePoller, pollNodeStateOnce } from "../src/bun/node-state-poll"

describe("node state poller", () => {
  test("pollOnce maps fetched node state to a serializable message", async () => {
    const message = await pollNodeStateOnce({
      async fetchNodeState() {
        return {
          ok: true,
          schema: CONTROL_SCHEMA_TAG,
          sessions: sessionListFixture,
        }
      },
    })

    expect(message).toEqual({
      ok: true,
      schema: CONTROL_SCHEMA_TAG,
      sessions: sessionListFixture,
    })
    expect(JSON.parse(JSON.stringify(message))).toEqual(message)
  })

  test("poller start and stop use the injected timer", async () => {
    const seen: unknown[] = []
    const intervals: Array<() => void> = []
    const handles: unknown[] = []
    const poller = createNodeStatePoller({
      intervalMs: 50,
      onState(message) {
        seen.push(message)
      },
      timer: {
        setInterval(callback, intervalMs) {
          intervals.push(callback)
          const handle = { intervalMs }
          handles.push(handle)
          return handle
        },
        clearInterval(handle) {
          handles.splice(handles.indexOf(handle), 1)
        },
      },
      async fetchNodeState() {
        return {
          ok: true,
          schema: CONTROL_SCHEMA_TAG,
          sessions: [] as SessionSummary[],
        }
      },
    })

    poller.start()
    await flushPoll()
    expect(seen).toHaveLength(1)
    expect(intervals).toHaveLength(1)
    expect(handles).toHaveLength(1)

    intervals[0]()
    await flushPoll()
    expect(seen).toHaveLength(2)

    poller.stop()
    expect(handles).toHaveLength(0)
  })
})

async function flushPoll(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
}
