import { Schema as S } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  DesktopTurnEventFrame,
  decodeDesktopTurnStartRequest,
  makeDesktopTurnFence,
} from "./desktop-turn-ipc.ts"

const encodeFrame = S.encodeUnknownSync(DesktopTurnEventFrame)

const progress = (requestRef: string, generation: number) =>
  encodeFrame({
    kind: "progress",
    requestRef,
    generation,
    projection: {
      schema: "openagents.agent_turn_projection.v1",
      threadRef: "thread.1",
      requestRef,
      cardState: "running",
      dataDestination: "remote_provider",
      usageTruth: "unknown",
      localOnly: false,
      updatedAt: "2026-07-20T08:00:00.000Z",
      messageChain: [],
      evidenceRefs: [],
    },
  })

const decodeFrame = S.decodeUnknownSync(DesktopTurnEventFrame)

describe("Desktop turn IPC fence", () => {
  test("a late frame from a superseded generation is dropped", () => {
    const fence = makeDesktopTurnFence()
    expect(fence.admit(decodeFrame(progress("request.1", 1)))).toBe(true)
    // A lower generation for the same request is a superseded, out-of-order frame.
    expect(fence.admit(decodeFrame(progress("request.1", 0)))).toBe(false)
    // A newer generation is admitted.
    expect(fence.admit(decodeFrame(progress("request.1", 2)))).toBe(true)
  })

  test("frames for distinct requests are fenced independently", () => {
    const fence = makeDesktopTurnFence()
    expect(fence.admit(decodeFrame(progress("request.a", 3)))).toBe(true)
    expect(fence.admit(decodeFrame(progress("request.b", 0)))).toBe(true)
  })

  test("no frame is admitted after a terminal frame for that request", () => {
    const fence = makeDesktopTurnFence()
    const terminal = decodeFrame(
      encodeFrame({
        kind: "terminal",
        requestRef: "request.1",
        generation: 1,
        projection: {
          schema: "openagents.agent_turn_projection.v1",
          threadRef: "thread.1",
          requestRef: "request.1",
          cardState: "done",
          dataDestination: "remote_provider",
          usageTruth: "exact",
          localOnly: false,
          updatedAt: "2026-07-20T08:00:00.000Z",
          messageChain: [],
          evidenceRefs: [],
        },
        receipt: {
          schema: "openagents.agent_turn_receipt.v1",
          requestRef: "request.1",
          routeDecisionRef: "route.1",
          decision: "accepted",
          usageTruth: "exact",
          evidenceRefs: [],
        },
      }),
    )
    expect(fence.admit(terminal)).toBe(true)
    expect(fence.admit(decodeFrame(progress("request.1", 2)))).toBe(false)
  })

  test("an invalid start request decodes to None", () => {
    expect(decodeDesktopTurnStartRequest({ nonsense: true })._tag).toBe("None")
  })
})
