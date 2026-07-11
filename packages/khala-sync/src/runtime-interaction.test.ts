import { describe, expect, test } from "bun:test"

import {
  decodeRuntimeInteractionEntity,
  encodeRuntimeInteractionEntity,
  type RuntimeInteractionEntity,
} from "./runtime-interaction.js"

const entity = (): RuntimeInteractionEntity =>
  decodeRuntimeInteractionEntity({
    interactionRef: "interaction.question.1",
    threadId: "thread.runtime.1",
    turnId: "turn.runtime.1",
    ownerUserId: "owner.runtime.1",
    kind: "provider_question",
    status: "pending",
    interaction: {
      schema: "openagents.runtime_interaction.v1",
      interactionRef: "interaction.question.1",
      threadId: "thread.runtime.1",
      turnId: "turn.runtime.1",
      requestedSequence: 3,
      requestedAt: "2026-07-11T22:00:00.000Z",
      expiresAt: "2026-07-11T22:05:00.000Z",
      source: {
        lane: "claude_pylon",
        adapterKind: "claude_code",
        surface: "server",
      },
      visibility: "private",
      redactionClass: "private_ref",
      causalityRefs: ["event.runtime.2"],
      payload: {
        kind: "provider_question",
        displayTitle: "Choose verification",
        questions: [{
          questionRef: "question.runtime.1",
          displayText: "Which check should run?",
          multiSelect: false,
          options: [{ optionRef: "option.tests", label: "Tests" }],
        }],
      },
      lifecycle: { status: "pending" },
    },
    createdAt: "2026-07-11T22:00:00.000Z",
    updatedAt: "2026-07-11T22:00:00.000Z",
  })

describe("Khala Sync runtime_interaction entity", () => {
  test("round-trips one exact private thread interaction post-image", () => {
    const value = entity()
    expect(decodeRuntimeInteractionEntity(
      JSON.parse(JSON.stringify(encodeRuntimeInteractionEntity(value))) as unknown,
    )).toEqual(value)
  })

  test("rejects every denormalized identity, kind, and status mismatch", () => {
    const value = entity()
    const mismatches = [
      { ...value, interactionRef: "interaction.other" },
      { ...value, threadId: "thread.other" },
      { ...value, turnId: "turn.other" },
      { ...value, kind: "tool_approval" },
      { ...value, status: "resolved" },
    ]
    for (const mismatch of mismatches) {
      expect(() => decodeRuntimeInteractionEntity(mismatch)).toThrow(
        "runtime interaction entity identity and lifecycle must match its post-image",
      )
    }
  })

  test("requires an owner and a private nested interaction", () => {
    const value = entity()
    expect(() => decodeRuntimeInteractionEntity({
      ...value,
      ownerUserId: "",
    })).toThrow()
    expect(() => decodeRuntimeInteractionEntity({
      ...value,
      interaction: { ...value.interaction, visibility: "public" },
    })).toThrow()
  })
})
