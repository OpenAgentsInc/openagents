import { describe, expect, test } from "vite-plus/test"

import { decodeRuntimeInteractionEntity } from "./runtime-interaction.js"
import {
  decodeRuntimeAttentionEntity,
  encodeRuntimeAttentionEntity,
  runtimeAttentionFromInteraction,
} from "./runtime-attention.js"

const privateInteraction = () => decodeRuntimeInteractionEntity({
  interactionRef: "interaction.attention.1",
  threadId: "thread.attention.1",
  turnId: "turn.attention.1",
  ownerUserId: "owner.attention.1",
  kind: "provider_question",
  status: "pending",
  interaction: {
    schema: "openagents.runtime_interaction.v1",
    interactionRef: "interaction.attention.1",
    threadId: "thread.attention.1",
    turnId: "turn.attention.1",
    requestedSequence: 3,
    requestedAt: "2026-07-17T12:00:00.000Z",
    expiresAt: "2026-07-17T12:05:00.000Z",
    source: { lane: "claude_pylon", adapterKind: "claude_code", surface: "server" },
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: ["event.attention.2"],
    payload: {
      kind: "provider_question",
      displayTitle: "Private verification question",
      questions: [{
        questionRef: "question.attention.1",
        displayText: "Private prompt that must stay thread-scoped",
        multiSelect: false,
        options: [{ optionRef: "option.tests", label: "Private choice" }],
      }],
    },
    lifecycle: { status: "pending" },
  },
  createdAt: "2026-07-17T12:00:00.000Z",
  updatedAt: "2026-07-17T12:00:00.000Z",
})

describe("contract khala_sync.runtime_attention.v1", () => {
  test("derives byte-safe personal attention without private interaction detail", () => {
    const attention = runtimeAttentionFromInteraction(privateInteraction())
    expect(attention).toEqual({
      schema: "openagents.runtime_attention.v1",
      attentionRef: "interaction.attention.1",
      ownerUserId: "owner.attention.1",
      interactionRef: "interaction.attention.1",
      threadRef: "thread.attention.1",
      turnRef: "turn.attention.1",
      kind: "provider_question",
      status: "pending",
      requestedAt: "2026-07-17T12:00:00.000Z",
      expiresAt: "2026-07-17T12:05:00.000Z",
      updatedAt: "2026-07-17T12:00:00.000Z",
    })
    const serialized = JSON.stringify(encodeRuntimeAttentionEntity(attention))
    expect(serialized).not.toContain("Private")
    expect(serialized).not.toContain("payload")
    expect(serialized).not.toContain("decision")
    expect(decodeRuntimeAttentionEntity(JSON.parse(serialized))).toEqual(attention)
  })

  test("rejects a projection whose attention identity diverges", () => {
    const attention = runtimeAttentionFromInteraction(privateInteraction())
    expect(() => decodeRuntimeAttentionEntity({
      ...attention,
      attentionRef: "interaction.other",
    })).toThrow("runtime attention identity must match its interaction ref")
  })
})
