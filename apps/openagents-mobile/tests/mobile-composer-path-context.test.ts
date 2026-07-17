import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import {
  decodeMobileComposerPathSearchPage,
  safeMobileComposerPathRef,
  searchMobileComposerPaths,
  type MobileComposerPathSearchRequest,
} from "../src/coding/mobile-composer-path-context"
import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../src/coding/mobile-execution-targets"
import type { MobileConversationHost, MobileConversationThread } from "../src/conversation/mobile-conversation"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"
import { mobileComposerPathTrigger } from "../src/screens/mobile-composer-discovery"

const now = "2026-07-17T22:00:00.000Z"
const repositoryRef = "repository.mobile.paths"
const worktreeRef = "worktree.mobile.paths"
const thread: MobileConversationThread = {
  threadRef: "thread.mobile.paths",
  title: "Path context",
  status: "active",
  messageCount: 0,
  lastMessageAt: null,
  updatedAt: now,
  version: 1,
  messages: [],
}
const target: MobileExecutionTargetOption = {
  targetId: "codex:mobile-paths",
  label: "Codex work",
  accessibilityLabel: "Codex work, Codex, ready",
  providerLabel: "Codex",
  providerRef: "provider.openai.codex",
  modelRef: "model.gpt-5.6-sol",
  accountRef: "account.mobile.paths",
  runtimeTarget: { lane: "codex_app_server", executionTargetId: "codex:mobile-paths" },
  readiness: "ready",
}
const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "main",
    targetLabel: target.label,
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.paths",
      ownerRef: "owner.mobile.paths",
      sessionRef: "session.mobile.paths",
      threadRef: thread.threadRef,
      revision: 1,
      doc: state.doc,
      selection: state.selection,
      view: state.view,
      context: [{ kind: "repository", repositoryRef, revisionRef: "revision.repository.paths" }, {
        kind: "worktree",
        repositoryRef,
        worktreeRef,
        revisionRef: "revision.worktree.paths",
      }],
      target: {
        laneRef: "lane.codex_app_server",
        providerRef: target.providerRef,
        modelRef: target.modelRef,
        accountRef: target.accountRef,
        executionTargetRef: target.targetId,
        readiness: "ready",
      },
      submission: { status: "editing" },
      updatedAt: now,
    }),
  }
}
const request: MobileComposerPathSearchRequest = {
  repositoryRef,
  worktreeRef,
  query: "src",
  limit: 20,
}
const page = (query: string, pathRef = "src/index.ts") => ({
  repositoryRef,
  worktreeRef,
  query,
  entries: [{ pathRef, kind: "file" as const, revisionRef: `revision.${query}` }],
})
const host: MobileConversationHost = {
  listThreads: async () => [thread],
  newThread: async () => ({ ok: true, thread }),
  openThread: async () => thread,
  sendMessage: async () => ({ ok: true, thread }),
}
const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})
const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })
const coding = (search?: (request: MobileComposerPathSearchRequest) => Promise<unknown>) => {
  const active = composer()
  return {
    activeComposer: () => active,
    directory: {
      authority: "confirmed" as const,
      phase: "live" as const,
      cacheState: "current" as const,
      offlineCache: { accounting: "live_confirmed" as const, ownerScopeRef: "scope.owner", cachedRepositoryCount: 0, cachedSessionCount: 0, lastConfirmedCursor: 1 },
      repositories: [],
      sessions: [],
    },
    executionTargets: [target],
    clearSelection: async () => undefined,
    selectSession: async () => ({ thread, composer: active }),
    updateComposerText: async (session: MobileCodingComposerSession) => session,
    selectComposerTarget: async (session: MobileCodingComposerSession) => session,
    pickComposerAttachments: async () => ({ status: "cancelled" as const }),
    ...(search === undefined ? {} : { searchComposerPaths: search }),
  }
}

describe("T3M-B2.2b mobile repository path context", () => {
  test("decodes only exact-scope bounded safe relative path pages", async () => {
    expect(safeMobileComposerPathRef("src/screens/home.ts")).toBe(true)
    for (const invalid of ["/etc/passwd", "../secret", "src/../secret", "src\\secret", "src//secret", "./src"]) {
      expect(safeMobileComposerPathRef(invalid)).toBe(false)
    }
    expect(decodeMobileComposerPathSearchPage(page("src"), request)).toEqual(page("src"))
    expect(decodeMobileComposerPathSearchPage({ ...page("src"), worktreeRef: "worktree.foreign" }, request)).toBeNull()
    expect(decodeMobileComposerPathSearchPage({ ...page("src"), entries: [
      ...page("src").entries,
      ...page("src").entries,
    ] }, request)).toBeNull()
    expect(await searchMobileComposerPaths({ search: async () => page("src") }, request)).toMatchObject({ state: "ready" })
  })

  test("searches exact composer scope, rejects foreign selection, and inserts only a current result", async () => {
    const calls: MobileComposerPathSearchRequest[] = []
    const program = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: coding(async input => { calls.push(input); return page(input.query) }),
    })
    program.khala.draftChanged("Inspect @src")
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(calls).toEqual([{ repositoryRef, worktreeRef, query: "src", limit: 20 }])
    expect(state.codingPathDiscovery).toMatchObject({ state: "ready", query: "src" })
    expect(JSON.stringify(renderContentView(state))).toContain("src/index.ts · file")

    program.coding.selectPath("../foreign")
    await Effect.runPromise(settle)
    expect((await Effect.runPromise(lastState(program))).khala.draft).toBe("Inspect @src")

    program.coding.selectPath("src/index.ts")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.khala.draft).toBe("Inspect @src/index.ts ")
    expect(state.codingPathDiscovery).toEqual({ state: "idle" })
  })

  test("drops stale search completion and names a missing environment transport", async () => {
    const resolvers = new Map<string, (value: unknown) => void>()
    const program = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: coding(input => new Promise(resolve => { resolvers.set(input.query, resolve) })),
    })
    program.khala.draftChanged("@old")
    await Effect.runPromise(settle)
    program.khala.draftChanged("@new")
    await Effect.runPromise(settle)
    resolvers.get("new")?.(page("new", "src/new.ts"))
    await Effect.runPromise(settle)
    resolvers.get("old")?.(page("old", "src/old.ts"))
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.codingPathDiscovery).toMatchObject({ state: "ready", query: "new" })
    expect(JSON.stringify(state.codingPathDiscovery)).toContain("src/new.ts")
    expect(JSON.stringify(state.codingPathDiscovery)).not.toContain("src/old.ts")

    const unavailable = buildHomeProgram({
      conversation: { mode: "sync", host, threads: [thread], archivedThreads: [], activeThread: thread },
      coding: coding(),
    })
    unavailable.khala.draftChanged("@src")
    await Effect.runPromise(settle)
    const unavailableState = await Effect.runPromise(lastState(unavailable))
    expect(unavailableState.codingPathDiscovery).toMatchObject({
      state: "unavailable",
      message: "Connect the exact worktree environment to search repository files.",
    })
    expect(mobileComposerPathTrigger(unavailableState.khala.draft)?.query).toBe("src")
  })
})
