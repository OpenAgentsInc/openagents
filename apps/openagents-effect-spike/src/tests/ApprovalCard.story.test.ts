// Firm reason: 1 (Effect UI Story + Story.explore concurrency race test for ApprovalCard in docs/effect/half-day-sprint.md)
import { Story } from "@effect/ui/testing"
import { assert, describe, it } from "@effect/vitest"
import { Effect } from "effect"
import { ApproveCommand, CommandAck } from "../client/commands/SessionCommands.ts"
import {
  ApprovalCard,
  ApprovalCardMessage
} from "../client/components/ApprovalCard.ts"
import type { PendingPermit } from "../shared/wire.ts"

const samplePermit: PendingPermit = {
  ticket: 7,
  kind: "permit",
  question: false,
  title: "Run shell command",
  command: "ls -la /tmp",
  options: []
}

describe("ApprovalCard", () => {
  it.effect("allows a pending permit once and ignores duplicate clicks while submitting or resolved", () =>
    Effect.gen(function*() {
      const s = yield* Story.make(ApprovalCard, {
        sessionId: "demo",
        permit: samplePermit
      })
      assert.strictEqual(s.model.status, "idle")

      s.send(ApprovalCardMessage.ClickedAllow()).expectExact(ApproveCommand)
      assert.strictEqual(s.model.status, "submitting")
      assert.strictEqual(s.model.decision, "allow")
      assert.strictEqual(s.pending.length, 1)

      // Duplicate clicks while submitting must be no-ops (mode: "switch" allows overlap check)
      s.send(ApprovalCardMessage.ClickedDeny())
      s.send(ApprovalCardMessage.ClickedAllow())
      assert.strictEqual(s.pending.length, 1)
      assert.strictEqual(s.model.decision, "allow")

      s.resolve(ApproveCommand, CommandAck.Ok())
      assert.strictEqual(s.model.status, "resolved")

      // Post-resolution clicks must also be no-ops
      s.send(ApprovalCardMessage.ClickedDeny())
      assert.strictEqual(s.pending.length, 0)
      assert.strictEqual(s.model.status, "resolved")
    }))

  it.effect("surfaces failure and lets the user retry cleanly", () =>
    Effect.gen(function*() {
      const s = yield* Story.make(ApprovalCard, {
        sessionId: "demo",
        permit: samplePermit
      })
      s.send(ApprovalCardMessage.ClickedDeny()).expectExact(ApproveCommand)
      s.resolve(ApproveCommand, CommandAck.Failed({ message: "network timeout" }))
      assert.strictEqual(s.model.status, "failed")
      assert.strictEqual(s.model.error, "network timeout")

      s.send(ApprovalCardMessage.ClickedAllow()).expectExact(ApproveCommand)
      assert.strictEqual(s.model.status, "submitting")
      s.resolve(ApproveCommand, CommandAck.Ok())
      assert.strictEqual(s.model.status, "resolved")
    }))

  it("Story.explore proves no interleaving can double-submit or escape resolved state", () => {
    const { closed } = Story.explore(
      ApprovalCard,
      { sessionId: "demo", permit: samplePermit },
      {
        messages: [
          ApprovalCardMessage.ClickedAllow(),
          ApprovalCardMessage.ClickedDeny()
        ],
        commands: [
          [
            ApproveCommand,
            {
              success: () => [
                CommandAck.Ok(),
                CommandAck.Failed({ message: "offline" })
              ]
            }
          ]
        ],
        invariant: (story) => {
          // At most one ApproveCommand may ever be in flight
          if (story.pending.length > 1) return false
          // Resolved state must never have an in-flight command
          if (story.model.status === "resolved" && story.pending.length > 0) {
            return false
          }
          // Submitting state must always have a chosen decision and exactly 1 pending command
          if (
            story.model.status === "submitting" &&
            (story.model.decision === null || story.pending.length !== 1)
          ) {
            return false
          }
          return true
        },
        depth: 6
      }
    )
    assert.isTrue(closed)
  })
})
