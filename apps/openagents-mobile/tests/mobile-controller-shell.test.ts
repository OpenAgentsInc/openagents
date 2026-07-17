import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type { MobileCodingDirectory } from "../src/coding/mobile-coding-navigation"
import {
  buildHomeProgram,
  initialHomeState,
  renderContentView,
  renderMobileControllerShell,
} from "../src/screens/home-core"

const directory: MobileCodingDirectory = {
  authority: "confirmed",
  phase: "live",
  cacheState: "current",
  offlineCache: {
    accounting: "live_confirmed",
    ownerScopeRef: "scope.user.owner.fixture",
    cachedRepositoryCount: 0,
    cachedSessionCount: 0,
    lastConfirmedCursor: 8,
  },
  repositories: [{
    repositoryRef: "repository.openagents",
    projectRef: "project.openagents",
    displayName: "openagents",
    sessionCount: 2,
  }],
  sessions: [
    {
      sessionRef: "session.ready",
      projectRef: "project.openagents",
      repositoryRef: "repository.openagents",
      worktreeRef: "worktree.ready",
      threadRef: "thread.ready",
      runRef: "run.ready",
      fleetRef: null,
      currentCheckpointRef: "checkpoint.ready",
      agentTopologyRef: "topology.ready",
      canonicalEventCursor: 8,
      provider: { state: "known", providerRef: "provider.codex" },
      runtime: { state: "known", runtimeRef: "runtime.owner-local" },
      state: "active",
      lastActiveAt: "2026-07-17T12:00:00.000Z",
    },
    {
      sessionRef: "session.recovery",
      projectRef: "project.openagents",
      repositoryRef: "repository.openagents",
      worktreeRef: "worktree.recovery",
      threadRef: "thread.recovery",
      runRef: null,
      fleetRef: null,
      currentCheckpointRef: null,
      agentTopologyRef: null,
      canonicalEventCursor: 4,
      provider: { state: "known", providerRef: "provider.claude" },
      runtime: { state: "unavailable", reason: "not_attached" },
      state: "recovery_required",
      lastActiveAt: "2026-07-17T11:00:00.000Z",
    },
  ],
}

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.controller_shell.v1", () => {
  test("paints metadata-first Recent without opening transcript detail", () => {
    const view = JSON.stringify(renderContentView({
      ...initialHomeState,
      surfaceMode: "openagents",
      syncPhase: "live",
      codingDirectory: directory,
    }))
    expect(view).toContain('"_tag":"SegmentedControl"')
    expect(view).toContain('"label":"Recent"')
    expect(view).toContain('"label":"Repositories"')
    expect(view).toContain('"label":"Attention"')
    expect(view).toContain("1 repository · 2 sessions · 1 need attention")
    expect(view).toContain("openagents · Active\\nTarget ready")
    expect(view).not.toContain('"_tag":"Transcript"')
  })

  test("switches typed destinations and exposes recovery-only attention", async () => {
    const program = buildHomeProgram({ coding: {
      directory,
      activeComposer: () => null,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async () => null,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
    } })
    program.controller.selectDestination("attention")
    const state = await Effect.runPromise(lastState(program))
    const view = JSON.stringify(renderMobileControllerShell({ ...state, surfaceMode: "openagents" }))
    expect(state.controllerDestination).toBe("attention")
    expect(view).toContain("session.recovery")
    expect(view).toContain("Recovery required")
    expect(view).not.toContain("session.ready")
  })

  test("withheld authority exposes loss accounting but never controller rows", () => {
    const view = JSON.stringify(renderMobileControllerShell({
      ...initialHomeState,
      surfaceMode: "openagents",
      syncPhase: "must_refetch",
      codingDirectory: {
        authority: "withheld",
        phase: "must_refetch",
        cacheState: "hidden_until_reconnect",
        offlineCache: {
          accounting: "withheld_counted",
          ownerScopeRef: "scope.user.owner.fixture",
          cachedRepositoryCount: 2,
          cachedSessionCount: 7,
          lastConfirmedCursor: 10,
        },
        repositories: [],
        sessions: [],
      },
    }))
    expect(view).toContain("2 repositories · 7 sessions hidden until reconnect")
    expect(view).not.toContain("controller-destinations")
    expect(view).not.toContain("controller-session-")
  })
})
