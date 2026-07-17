import { describe, expect, test } from "vite-plus/test"
import { composerActionPresentation, makeComposerSubmitIntent, type ComposerAdmissionState } from "./composer-admission.ts"

describe("composer admission matrix", () => {
  const states: ReadonlyArray<ComposerAdmissionState> = ["idle", "active_steerable", "active_nonsteerable", "interrupting", "repairing", "queued", "offline", "blocked", "incompatible"]
  test.each(states)("%s is explicit and never silently reroutes", state => {
    const admission = { state, activeTurnId: state === "active_steerable" ? "turn-provider-7" : null, reason: state === "active_steerable" ? null : `Reason: ${state}`, queuedCount: 2 } as const
    const steer = composerActionPresentation(admission, "steer")
    const queue = composerActionPresentation(admission, "queue")
    expect(steer.mode).toBe("steer")
    expect(queue.mode).toBe("queue")
    expect(steer.enabled).toBe(state === "active_steerable")
    expect(queue.enabled).toBe(!["offline", "blocked", "incompatible"].includes(state))
  })

  test("steer binds exact displayed turn and queue omits it", () => {
    const admission = { state: "active_steerable", activeTurnId: "turn-provider-7", reason: null, queuedCount: 0 } as const
    const common = { admission, threadRef: "thread-1", message: " continue ", intentRef: "intent-1", clientUserMessageId: "user-1", createdAt: "2026-07-16T20:00:00.000Z" }
    expect(makeComposerSubmitIntent({ ...common, mode: "steer" })).toMatchObject({
      kind: "steer_current",
      expectedTurnId: "turn-provider-7",
      message: "continue",
      control: {
        kind: "turn.steer",
        turnRef: "turn-provider-7",
        messageRef: "user-1",
        expiresAt: "2026-07-16T20:05:00.000Z",
      },
    })
    expect(makeComposerSubmitIntent({ ...common, mode: "queue" })).toMatchObject({
      kind: "queue_next",
      threadRef: "thread-1",
      message: "continue",
      intentRef: "intent-1",
      clientUserMessageId: "user-1",
      control: {
        kind: "turn.queue",
        messageRef: "user-1",
        targetGeneration: { state: "unknown", reason: "not_observed" },
      },
    })
  })
})
