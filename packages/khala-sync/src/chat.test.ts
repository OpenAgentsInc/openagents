import { describe, expect, test } from "bun:test"

import {
  decodeChatThreadCodexContinuityPin,
  chatThreadRepoBindingRef,
  decodeChatThreadEntity,
  decodeChatThreadRepoBinding,
  encodeChatThreadCodexContinuityPin,
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

describe("ChatThreadCodexContinuityPin (CX-6, #8550)", () => {
  test("decodes and encodes public-safe custody refs only", () => {
    const pin = decodeChatThreadCodexContinuityPin({
      accountRefHash: "acct_hash_1",
      authGrantRef: "grant.codex.thread_1",
      pinnedAt: "2026-07-08T00:00:00.000Z",
      provider: "chatgpt_codex",
      providerAccountRef: "provider-account.codex.owner_1",
    })
    expect(encodeChatThreadCodexContinuityPin(pin)).toEqual({
      accountRefHash: "acct_hash_1",
      authGrantRef: "grant.codex.thread_1",
      pinnedAt: "2026-07-08T00:00:00.000Z",
      provider: "chatgpt_codex",
      providerAccountRef: "provider-account.codex.owner_1",
    })
    expect(JSON.stringify(pin)).not.toMatch(/token|secret|authJson|CODEX_HOME/i)
  })

  test("rejects non-Codex providers and slash-shaped refs", () => {
    expect(() =>
      decodeChatThreadCodexContinuityPin({
        authGrantRef: "grant.codex.thread_1",
        pinnedAt: "2026-07-08T00:00:00.000Z",
        provider: "claude",
        providerAccountRef: "provider-account.codex.owner_1",
      }),
    ).toThrow()
    expect(() =>
      decodeChatThreadCodexContinuityPin({
        authGrantRef: "grant/codex/thread_1",
        pinnedAt: "2026-07-08T00:00:00.000Z",
        provider: "chatgpt_codex",
        providerAccountRef: "provider-account.codex.owner_1",
      }),
    ).toThrow()
  })
})

describe("ChatThreadEntity.codexContinuity (CX-6, #8550)", () => {
  test("decodes a legacy row with no codexContinuity key", () => {
    const thread = decodeChatThreadEntity(baseThreadJson)
    expect(thread.codexContinuity).toBeUndefined()
    expect(encodeChatThreadEntity(thread)).not.toHaveProperty("codexContinuity")
  })

  test("decodes explicit null and a real continuity pin", () => {
    expect(decodeChatThreadEntity({ ...baseThreadJson, codexContinuity: null }).codexContinuity).toBeNull()
    const thread = decodeChatThreadEntity({
      ...baseThreadJson,
      codexContinuity: {
        authGrantRef: "grant.codex.thread_1",
        pinnedAt: "2026-07-08T00:00:00.000Z",
        provider: "chatgpt_codex",
        providerAccountRef: "provider-account.codex.owner_1",
      },
    })
    expect(thread.codexContinuity).toEqual({
      authGrantRef: "grant.codex.thread_1",
      pinnedAt: "2026-07-08T00:00:00.000Z",
      provider: "chatgpt_codex",
      providerAccountRef: "provider-account.codex.owner_1",
    })
  })

  test("rejects a malformed continuity pin on an otherwise-valid thread", () => {
    expect(() =>
      decodeChatThreadEntity({
        ...baseThreadJson,
        codexContinuity: {
          pinnedAt: "2026-07-08T00:00:00.000Z",
          provider: "chatgpt_codex",
          providerAccountRef: "provider-account.codex.owner_1",
        },
      }),
    ).toThrow()
  })
})
