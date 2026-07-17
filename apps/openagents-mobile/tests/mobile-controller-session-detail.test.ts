import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type { MobileCodingDirectory, MobileCodingTarget } from "../src/coding/mobile-coding-navigation"
import type { MobileConversationThread } from "../src/conversation/mobile-conversation"
import { buildHomeProgram, renderMobileControllerShell } from "../src/screens/home-core"

const now = "2026-07-17T12:00:00.000Z"
const target: MobileCodingTarget = {
  schema: "openagents.mobile.coding_target.v1",
  repositoryRef: "repository.openagents",
  sessionRef: "session.controller",
  threadRef: "thread.controller",
}
const directory: MobileCodingDirectory = {
  authority: "confirmed",
  phase: "live",
  cacheState: "current",
  offlineCache: {
    accounting: "live_confirmed",
    ownerScopeRef: "scope.user.owner.fixture",
    cachedRepositoryCount: 0,
    cachedSessionCount: 0,
    lastConfirmedCursor: 14,
  },
  repositories: [{
    repositoryRef: target.repositoryRef,
    projectRef: "project.openagents",
    displayName: "openagents",
    sessionCount: 1,
  }],
  sessions: [{
    sessionRef: target.sessionRef,
    projectRef: "project.openagents",
    repositoryRef: target.repositoryRef,
    worktreeRef: "worktree.controller",
    threadRef: target.threadRef,
    runRef: "run.controller",
    fleetRef: "fleet.controller",
    currentCheckpointRef: "checkpoint.controller",
    agentTopologyRef: "topology.controller",
    canonicalEventCursor: 14,
    provider: { state: "known", providerRef: "provider.codex" },
    runtime: { state: "unavailable", reason: "not_attached" },
    state: "idle",
    lastActiveAt: now,
  }],
}
const thread: MobileConversationThread = {
  threadRef: target.threadRef,
  title: "Controller session",
  status: "active",
  lastMessageAt: null,
  messageCount: 0,
  version: 1,
  messages: [],
  timeline: null,
  graphs: [],
  updatedAt: now,
}

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.controller_session_detail.v1", () => {
  test("inspection renders canonical detail without activating the transcript", async () => {
    const selected: MobileCodingTarget[] = []
    const program = buildHomeProgram({ coding: {
      directory,
      activeComposer: () => null,
      clearSelection: async () => undefined,
      selectSession: async input => {
        selected.push(input)
        return { thread, composer: null }
      },
      updateComposerText: async () => null,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
    } })

    program.controller.inspectSession(target.sessionRef)
    const state = await Effect.runPromise(lastState(program))
    const view = JSON.stringify(renderMobileControllerShell(state))

    expect(selected).toEqual([])
    expect(state.activeThreadRef).toBeNull()
    expect(view).toContain("Session session.controller\\nThread thread.controller")
    expect(view).toContain("Provider provider.codex")
    expect(view).toContain("Runtime Unavailable · not attached")
    expect(view).toContain("checkpoint.controller")
    expect(view).toContain('"label":"Continue session"')
    expect(view).toContain('"name":"CodingSessionSelected"')

    program.coding.selectSession(target)
    await Effect.runPromise(Effect.yieldNow)
    expect(selected).toEqual([target])
  })

  test("unknown or withheld session refs cannot become authoritative detail", async () => {
    const program = buildHomeProgram({ coding: {
      directory,
      activeComposer: () => null,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async () => null,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
    } })
    program.controller.inspectSession("session.stale")
    const state = await Effect.runPromise(lastState(program))
    expect(state.inspectedControllerSessionRef).toBeNull()
    expect(JSON.stringify(renderMobileControllerShell({
      ...state,
      codingDirectory: {
        authority: "withheld",
        phase: "must_refetch",
        cacheState: "hidden_until_reconnect",
        offlineCache: {
          accounting: "withheld_counted",
          ownerScopeRef: "scope.user.owner.fixture",
          cachedRepositoryCount: 1,
          cachedSessionCount: 1,
          lastConfirmedCursor: 14,
        },
        repositories: [],
        sessions: [],
      },
      inspectedControllerSessionRef: target.sessionRef,
    }))).not.toContain("controller-session-detail-")
  })
})
