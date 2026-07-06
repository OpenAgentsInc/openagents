import { describe, expect, test } from "bun:test"

import {
  decodeChatThreadEntity,
  personalScope,
  threadScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import {
  applyThreadRepoBinding,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  chatBindThreadRepoClientMutator,
  chatThreadRepoBindingOverlayEffects,
} from "../src/sync/khala-thread-repo-binding-core"

const baseThread: ChatThreadEntity = decodeChatThreadEntity({
  createdAt: "2026-07-06T00:00:00.000Z",
  lastMessageAt: null,
  messageCount: 3,
  ownerUserId: "github:12345",
  status: "active",
  threadId: "thread_1",
  title: "My thread",
  updatedAt: "2026-07-06T00:00:00.000Z",
})

describe("applyThreadRepoBinding", () => {
  test("sets a repo binding on a thread with none", () => {
    const updated = applyThreadRepoBinding(baseThread, {
      defaultBranch: "main",
      name: "openagents",
      owner: "OpenAgentsInc",
    })
    expect(updated.repoBinding).toEqual({ defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
    // Every other field is preserved untouched.
    expect(updated.threadId).toBe(baseThread.threadId)
    expect(updated.title).toBe(baseThread.title)
    expect(updated.messageCount).toBe(baseThread.messageCount)
  })

  test("clears a repo binding when repo is null", () => {
    const bound = applyThreadRepoBinding(baseThread, { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
    const cleared = applyThreadRepoBinding(bound, null)
    expect(cleared.repoBinding).toBeNull()
  })

  test("rebinding to a different repo replaces (does not merge with) the prior binding", () => {
    const bound = applyThreadRepoBinding(baseThread, { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
    const rebound = applyThreadRepoBinding(bound, { defaultBranch: "develop", name: "other-repo", owner: "someone-else" })
    expect(rebound.repoBinding).toEqual({ defaultBranch: "develop", name: "other-repo", owner: "someone-else" })
  })
})

describe("chatThreadRepoBindingOverlayEffects", () => {
  test("upserts into both the owner's personal scope and the thread-local scope", () => {
    const bound = applyThreadRepoBinding(baseThread, { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
    const effects = chatThreadRepoBindingOverlayEffects(bound)
    expect(effects).toHaveLength(2)
    expect(effects.map(effect => String(effect.scope))).toEqual([
      String(personalScope(bound.ownerUserId)),
      String(threadScope(bound.threadId)),
    ])
    for (const effect of effects) {
      expect(effect.kind).toBe("upsert")
      expect(effect.entityId).toBe(bound.threadId)
      expect(JSON.parse(effect.postImageJson).repoBinding).toEqual({
        defaultBranch: "main",
        name: "openagents",
        owner: "OpenAgentsInc",
      })
    }
  })
})

describe("chatBindThreadRepoClientMutator", () => {
  test("has the documented mutator name", () => {
    const mutator = chatBindThreadRepoClientMutator({ ownerUserId: "github:12345" })
    expect(String(mutator.name)).toBe(CHAT_BIND_THREAD_REPO_MUTATOR_NAME)
  })

  test("reads the current thread from the overlay view and applies the binding on top of it", () => {
    const mutator = chatBindThreadRepoClientMutator({ ownerUserId: "github:12345" })
    const view = {
      get: (_scope: unknown, entityType: string, entityId: string) => {
        if (entityType !== "chat_thread" || entityId !== "thread_1") return undefined
        return JSON.stringify({ ...baseThread })
      },
      list: () => [],
    }
    const effects = mutator.apply({ repo: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" }, threadId: "thread_1" }, view)
    expect(effects).toHaveLength(2)
    const decoded = JSON.parse((effects[0] as { postImageJson: string }).postImageJson)
    expect(decoded.title).toBe("My thread") // preserved from the "current" read, not reset
    expect(decoded.repoBinding).toEqual({ defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
  })

  test("falls back to a placeholder thread when the overlay has never seen this thread yet", () => {
    const mutator = chatBindThreadRepoClientMutator({
      now: () => "2026-07-06T01:00:00.000Z",
      ownerUserId: "github:12345",
    })
    const view = { get: () => undefined, list: () => [] }
    const effects = mutator.apply(
      { repo: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" }, threadId: "brand_new_thread" },
      view,
    )
    const decoded = JSON.parse((effects[0] as { postImageJson: string }).postImageJson)
    expect(decoded.threadId).toBe("brand_new_thread")
    expect(decoded.repoBinding).toEqual({ defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" })
  })
})
