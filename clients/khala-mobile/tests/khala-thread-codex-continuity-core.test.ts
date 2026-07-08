import { describe, expect, test } from "bun:test"

import {
  decodeChatThreadEntity,
  personalScope,
  threadScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import {
  applyThreadCodexContinuityPin,
  CHAT_PIN_CODEX_CONTINUITY_MUTATOR_NAME,
  chatPinCodexContinuityClientMutator,
  chatThreadCodexContinuityOverlayEffects,
  decodePickedCodexContinuityPin,
} from "../src/sync/khala-thread-codex-continuity-core"

const baseThread: ChatThreadEntity = decodeChatThreadEntity({
  createdAt: "2026-07-08T00:00:00.000Z",
  lastMessageAt: null,
  messageCount: 3,
  ownerUserId: "github:12345",
  status: "active",
  threadId: "thread_1",
  title: "My thread",
  updatedAt: "2026-07-08T00:00:00.000Z",
})

const pin = {
  accountRefHash: "acct_hash_1",
  authGrantRef: "grant.codex.thread_1",
  pinnedAt: "2026-07-08T01:00:00.000Z",
  provider: "chatgpt_codex" as const,
  providerAccountRef: "provider-account.codex.owner_1",
}

describe("applyThreadCodexContinuityPin", () => {
  test("sets a Codex continuity pin without changing thread metadata", () => {
    const updated = applyThreadCodexContinuityPin(baseThread, pin)
    expect(updated.codexContinuity).toEqual(pin)
    expect(updated.threadId).toBe(baseThread.threadId)
    expect(updated.title).toBe(baseThread.title)
    expect(updated.messageCount).toBe(baseThread.messageCount)
  })

  test("clears a Codex continuity pin explicitly", () => {
    const updated = applyThreadCodexContinuityPin(baseThread, pin)
    expect(applyThreadCodexContinuityPin(updated, null).codexContinuity).toBeNull()
  })

  test("rejects unsafe refs before they reach the overlay", () => {
    expect(() =>
      applyThreadCodexContinuityPin(baseThread, {
        ...pin,
        authGrantRef: "grant/codex/thread_1",
      }),
    ).toThrow()
  })
})

describe("chatThreadCodexContinuityOverlayEffects", () => {
  test("upserts into owner and thread scopes with no secret-shaped fields", () => {
    const updated = applyThreadCodexContinuityPin(baseThread, pin)
    const effects = chatThreadCodexContinuityOverlayEffects(updated)
    expect(effects).toHaveLength(2)
    expect(effects.map(effect => String(effect.scope))).toEqual([
      String(personalScope(updated.ownerUserId)),
      String(threadScope(updated.threadId)),
    ])
    for (const effect of effects) {
      const decoded = JSON.parse(effect.postImageJson)
      expect(decoded.codexContinuity).toEqual(pin)
      expect(effect.postImageJson).not.toMatch(/token|secret|authJson|CODEX_HOME/i)
    }
  })
})

describe("chatPinCodexContinuityClientMutator", () => {
  test("has the server-recognized mutator name", () => {
    const mutator = chatPinCodexContinuityClientMutator({ ownerUserId: "github:12345" })
    expect(String(mutator.name)).toBe(CHAT_PIN_CODEX_CONTINUITY_MUTATOR_NAME)
  })

  test("reads the current thread and applies the pin over it", () => {
    const mutator = chatPinCodexContinuityClientMutator({ ownerUserId: "github:12345" })
    const view = {
      get: (_scope: unknown, entityType: string, entityId: string) => {
        if (entityType !== "chat_thread" || entityId !== "thread_1") return undefined
        return JSON.stringify({ ...baseThread })
      },
      list: () => [],
    }
    const effects = mutator.apply({ codexContinuity: pin, threadId: "thread_1" }, view)
    const decoded = JSON.parse((effects[0] as { postImageJson: string }).postImageJson)
    expect(decoded.title).toBe("My thread")
    expect(decoded.codexContinuity).toEqual(pin)
  })

  test("falls back to a placeholder thread when necessary", () => {
    const mutator = chatPinCodexContinuityClientMutator({
      now: () => "2026-07-08T02:00:00.000Z",
      ownerUserId: "github:12345",
    })
    const view = { get: () => undefined, list: () => [] }
    const effects = mutator.apply({ codexContinuity: pin, threadId: "brand_new_thread" }, view)
    const decoded = JSON.parse((effects[0] as { postImageJson: string }).postImageJson)
    expect(decoded.threadId).toBe("brand_new_thread")
    expect(decoded.codexContinuity).toEqual(pin)
  })
})

describe("decodePickedCodexContinuityPin", () => {
  test("decodes a picked pin and explicit clear", () => {
    expect(decodePickedCodexContinuityPin(pin)?.providerAccountRef).toBe(
      "provider-account.codex.owner_1",
    )
    expect(decodePickedCodexContinuityPin(null)).toBeNull()
  })
})
