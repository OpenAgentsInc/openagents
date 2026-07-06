import { describe, expect, test } from "bun:test"

import {
  chatThreadRepoBindingRef,
  decodeChatThreadEntity,
  decodeChatThreadRepoBinding,
  encodeChatThreadEntity,
  encodeChatThreadRepoBinding,
} from "./index.js"

const baseThreadJson = {
  createdAt: "2026-07-06T00:00:00.000Z",
  lastMessageAt: null,
  messageCount: 0,
  ownerUserId: "github:12345",
  status: "active" as const,
  threadId: "thread_1",
  title: "My thread",
  updatedAt: "2026-07-06T00:00:00.000Z",
}

describe("ChatThreadRepoBinding (MM-B2, #8472)", () => {
  test("decodes and encodes a valid binding", () => {
    const binding = decodeChatThreadRepoBinding({
      defaultBranch: "main",
      name: "openagents",
      owner: "OpenAgentsInc",
    })
    expect(binding.owner).toBe("OpenAgentsInc")
    expect(binding.name).toBe("openagents")
    expect(binding.defaultBranch).toBe("main")
    expect(encodeChatThreadRepoBinding(binding)).toEqual({
      defaultBranch: "main",
      name: "openagents",
      owner: "OpenAgentsInc",
    })
  })

  test("rejects an owner/name with disallowed characters", () => {
    expect(() =>
      decodeChatThreadRepoBinding({ defaultBranch: "main", name: "ok", owner: "not a valid owner" }),
    ).toThrow()
    expect(() =>
      decodeChatThreadRepoBinding({ defaultBranch: "main", name: "not/a/valid/name", owner: "ok" }),
    ).toThrow()
  })

  test("chatThreadRepoBindingRef formats owner/name", () => {
    expect(chatThreadRepoBindingRef({ name: "openagents", owner: "OpenAgentsInc" })).toBe(
      "OpenAgentsInc/openagents",
    )
  })
})

describe("ChatThreadEntity.repoBinding (MM-B2, #8472)", () => {
  test("decodes a legacy row with no repoBinding key at all (backward compatibility)", () => {
    const thread = decodeChatThreadEntity(baseThreadJson)
    expect(thread.repoBinding).toBeUndefined()
    // Re-encoding a thread that never had a binding must not invent the key.
    expect(encodeChatThreadEntity(thread)).not.toHaveProperty("repoBinding")
  })

  test("decodes an explicit null repoBinding (repo-less thread, recorded)", () => {
    const thread = decodeChatThreadEntity({ ...baseThreadJson, repoBinding: null })
    expect(thread.repoBinding).toBeNull()
  })

  test("decodes and round-trips a real repo binding", () => {
    const thread = decodeChatThreadEntity({
      ...baseThreadJson,
      repoBinding: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" },
    })
    expect(thread.repoBinding).toEqual({
      defaultBranch: "main",
      name: "openagents",
      owner: "OpenAgentsInc",
    })
    expect(encodeChatThreadEntity(thread)).toEqual({
      ...baseThreadJson,
      repoBinding: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" },
    })
  })

  test("rejects a malformed repoBinding on an otherwise-valid thread", () => {
    expect(() =>
      decodeChatThreadEntity({
        ...baseThreadJson,
        repoBinding: { defaultBranch: "main", name: "openagents" }, // missing owner
      }),
    ).toThrow()
  })
})
