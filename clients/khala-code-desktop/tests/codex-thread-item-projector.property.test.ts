import { describe, expect, test } from "bun:test"
import fc from "fast-check"

import {
  createCodexThreadItemEventProjector,
} from "../src/bun/codex-thread-item-projector"
import type { CodexAppServerNotification } from "../src/bun/codex-app-server-client"

type DeltaFamily = "agent" | "command" | "file" | "plan" | "reasoning"

type DeltaOp = Readonly<{
  family: DeltaFamily
  itemId: string
  value: string
}>

const safeId = fc
  .stringMatching(/[a-z][a-z0-9_-]{0,10}/)
  .filter(value => value !== "reasoning")

const shortText = fc.string({ minLength: 1, maxLength: 80 })

const deltaOpArbitrary: fc.Arbitrary<DeltaOp> = fc.record({
  family: fc.constantFrom("agent", "command", "file", "plan", "reasoning"),
  itemId: safeId,
  value: shortText,
})

const note = (
  method: string,
  params: Record<string, unknown>,
): CodexAppServerNotification => ({
  method,
  params: {
    threadId: "thread-property",
    turnId: "turn-property",
    ...params,
  },
  receivedAt: "2026-07-01T17:00:00.000Z",
})

const notificationFor = (op: DeltaOp): CodexAppServerNotification => {
  switch (op.family) {
    case "agent":
      return note("item/agentMessage/delta", {
        itemId: op.itemId,
        delta: op.value,
      })
    case "command":
      return note("item/commandExecution/outputDelta", {
        itemId: op.itemId,
        delta: op.value,
      })
    case "file":
      return note("item/fileChange/patchUpdated", {
        itemId: op.itemId,
        changes: [{
          path: `src/${op.itemId}.ts`,
          kind: "update",
          diff: `--- a/src/${op.itemId}.ts\n+++ b/src/${op.itemId}.ts\n@@ -1 +1 @@\n-old\n+${op.value}\n`,
        }],
      })
    case "plan":
      return note("item/plan/delta", {
        itemId: op.itemId,
        delta: op.value,
      })
    case "reasoning":
      return note("item/reasoning/delta", {
        itemId: op.itemId,
        delta: op.value,
      })
  }
}

describe("Codex ThreadItem projector properties", () => {
  test("keeps stable cards for arbitrary interleavings of item delta families", () => {
    fc.assert(
      fc.property(fc.array(deltaOpArbitrary, { maxLength: 80 }), (ops) => {
        const projector = createCodexThreadItemEventProjector({
          desktopTurnId: "desktop-turn-property",
        })

        for (const op of ops) {
          expect(() => projector.accept(notificationFor(op))).not.toThrow()
        }

        const messages = projector.messages()
        const ids = messages.map(message => message.id)
        expect(new Set(ids).size).toBe(ids.length)

        const latestFamilyById = new Map<string, DeltaFamily>()
        const bodyById = new Map<string, string>()
        for (const op of ops) {
          if (op.family === "reasoning") continue
          latestFamilyById.set(op.itemId, op.family)
          if (op.family === "file") {
            bodyById.set(op.itemId, op.value)
          } else {
            bodyById.set(op.itemId, `${bodyById.get(op.itemId) ?? ""}${op.value}`)
          }
        }

        expect(messages).toHaveLength(latestFamilyById.size)
        for (const message of messages) {
          const family = latestFamilyById.get(message.id)
          expect(family).toBeDefined()
          expect(message.codexItem?.itemId ?? message.id).toBe(message.id)
          expect(message.body).toContain(bodyById.get(message.id) ?? "")
          if (family === "agent") {
            expect(message.role).toBe("assistant")
            expect(message.codexItem).toBeUndefined()
          } else {
            expect(message.role).toBe(family === "plan" ? "assistant" : "tool")
            expect(message.codexItem).toMatchObject({
              itemId: message.id,
              status: "running",
              threadId: "thread-property",
              turnId: "turn-property",
            })
          }
        }
      }),
      { numRuns: 150 },
    )
  })
})
