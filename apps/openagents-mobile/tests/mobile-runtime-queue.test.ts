import { describe, expect, test } from "vite-plus/test"
import { classifyRuntimeControlReplay } from "@openagentsinc/agent-runtime-schema"

import {
  makeMobileRuntimeQueueControl,
  mobileRuntimeQueueAdmissionOutcome,
} from "../src/conversation/mobile-runtime-queue"

const control = () => makeMobileRuntimeQueueControl({
  intentRef: "queue.mobile.fixture.1",
  messageRef: "message.mobile.fixture.1",
  threadRef: "thread.mobile.fixture.1",
  runVersion: 17,
  createdAt: "2026-07-17T22:18:00.000Z",
  expiresAt: "2026-07-17T22:23:00.000Z",
})

describe("T3M-B2.3b provider-neutral mobile queue control", () => {
  test("binds exact thread, message, generation, ordering, origin, and deadline", () => {
    expect(control()).toEqual({
      schema: "openagents.runtime_control_intent.v2",
      kind: "turn.queue",
      intentRef: "queue.mobile.fixture.1",
      idempotencyKey: "idem.queue.mobile.fixture.1",
      threadRef: "thread.mobile.fixture.1",
      messageRef: "message.mobile.fixture.1",
      targetGeneration: { state: "known", value: 17 },
      orderingKey: "order.thread.mobile.fixture.1",
      createdAt: "2026-07-17T22:18:00.000Z",
      expiresAt: "2026-07-17T22:23:00.000Z",
      origin: { surface: "mobile", lane: "khala_sync" },
    })
  })

  test("keeps exact retries distinct from conflicting identity reuse", () => {
    const original = control()
    expect(classifyRuntimeControlReplay(original, { ...original })).toBe("exact_retry")
    expect(classifyRuntimeControlReplay(original, {
      ...original,
      messageRef: "message.mobile.foreign",
    })).toBe("conflicting_reuse")
  })

  test("keeps admission, delivery, and terminal observation separate", () => {
    const accepted = mobileRuntimeQueueAdmissionOutcome({
      control: control(),
      observedAt: "2026-07-17T22:18:01.000Z",
      admission: "accepted",
    })
    expect(accepted).toMatchObject({
      admission: { status: "accepted" },
      delivery: { status: "pending" },
      terminal: { status: "pending" },
    })
    expect(mobileRuntimeQueueAdmissionOutcome({
      control: control(),
      observedAt: "2026-07-17T22:23:00.000Z",
      admission: "expired",
    })).toMatchObject({
      admission: { status: "expired" },
      delivery: { status: "failed", reasonRef: "reason.queue_expired" },
    })
  })
})
