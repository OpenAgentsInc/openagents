import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import type { ConfirmedPortableSessionSnapshot } from "@openagentsinc/khala-sync-client"

import type { MobileCodingDirectory } from "../src/coding/mobile-coding-navigation"
import type { MobilePortableControlAction } from "../src/coding/mobile-portable-session-controls"
import { buildHomeProgram, renderMobileControllerShell } from "../src/screens/home-core"

const sessionRef = "session.portable.ui"
const directory: MobileCodingDirectory = {
  authority: "confirmed",
  phase: "live",
  cacheState: "current",
  offlineCache: {
    accounting: "live_confirmed",
    ownerScopeRef: "scope.user.owner.mobile",
    cachedRepositoryCount: 0,
    cachedSessionCount: 0,
    lastConfirmedCursor: 22,
  },
  repositories: [{
    repositoryRef: "repository.openagents",
    projectRef: "project.openagents",
    displayName: "openagents",
    sessionCount: 1,
  }],
  sessions: [{
    sessionRef,
    projectRef: "project.openagents",
    repositoryRef: "repository.openagents",
    worktreeRef: "worktree.portable.ui",
    threadRef: "thread.portable.ui",
    runRef: "run.portable.ui",
    fleetRef: null,
    currentCheckpointRef: "checkpoint.portable.ui",
    agentTopologyRef: "topology.portable.ui",
    canonicalEventCursor: 22,
    provider: { state: "known", providerRef: "provider.codex" },
    runtime: { state: "known", runtimeRef: "runtime.owner-local" },
    state: "active",
    lastActiveAt: "2026-07-17T12:00:00.000Z",
  }],
}

const snapshot = (): ConfirmedPortableSessionSnapshot => ({
  status: { phase: "live", cursor: 22, pendingCommandCount: 0 },
  sessions: [{
    schema: "openagents.portable_session.v1",
    sessionRef,
    ownerRef: "owner.mobile",
    identityBasis: "owner_minted",
    workContextRef: "work-context.mobile",
    eventLogRef: "event-log.mobile",
    currentProjectionRef: "projection.mobile",
    commandScopeRef: "command-scope.mobile",
    graph: {
      rootAgentRef: "agent.root",
      nodes: [{
        agentRef: "agent.root",
        threadRef: "thread.portable.ui",
        transcriptRef: "transcript.root",
        activityCursor: 22,
        lifecycle: "running",
        attachmentGeneration: 3,
      }],
    },
    adoptedFromLocalHistory: false,
  }],
  targetDirectories: [{
    sessionRef,
    targets: [
      { targetRef: "target.local", targetClass: "owner_local", adapterRef: "adapter.pylon", ownerRef: "owner.mobile", compatibilityRef: "catalog.1", isolation: "owner_host_process", dataPosture: "owner_device_only", health: "ready" },
      { targetRef: "target.managed", targetClass: "openagents_managed", adapterRef: "adapter.agent-computer", ownerRef: "owner.mobile", compatibilityRef: "catalog.1", isolation: "dedicated_microvm", dataPosture: "openagents_managed_region", health: "ready" },
    ],
  }],
  attachments: [{
    attachmentRef: "attachment.mobile.3",
    sessionRef,
    targetRef: "target.local",
    generation: 3,
    state: "active",
    descendantAgentRefs: ["agent.root"],
    capabilityLeaseRefs: ["lease.provider.3"],
    evidenceRefs: ["receipt.attach.3"],
  }],
  commands: [],
  issues: [],
})

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

const codingBase = {
  directory,
  activeComposer: () => null,
  clearSelection: async () => undefined,
  selectSession: async () => null,
  updateComposerText: async () => null,
  pickComposerAttachments: async () => ({ status: "cancelled" as const }),
}

describe("contract openagents_mobile.portable_controls_ui.v1", () => {
  test("shows confirmed source and explicit destinations, then reports a move as queued", async () => {
    const requested: Array<Readonly<{
      sessionRef: string
      action: MobilePortableControlAction
      destinationTargetRef?: string
    }>> = []
    const initial = snapshot()
    const program = buildHomeProgram({ coding: {
      ...codingBase,
      portableSnapshot: initial,
      requestPortableAction: async input => {
        requested.push(input)
        return {
          state: "queued" as const,
          snapshot: {
            ...initial,
            status: { ...initial.status, pendingCommandCount: 1 },
          },
        }
      },
    } })

    program.controller.inspectSession(sessionRef)
    program.controller.selectPortableDestination("target.managed")
    program.controller.requestPortableControl("move")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const view = JSON.stringify(renderMobileControllerShell(state))

    expect(requested).toEqual([{
      sessionRef,
      action: "move",
      destinationTargetRef: "target.managed",
    }])
    expect(view).toContain("Portable source owner_local · target.local")
    expect(view).toContain("Generation 3 · active")
    expect(view).toContain("Selected · openagents_managed · target.managed")
    expect(view).toContain("move queued for confirmed reconciliation")
    expect(view).toContain("Command queued · awaiting server acceptance")
    expect(view).not.toContain("move completed")
  })

  test("withholds requests when confirmed portable authority is stale", async () => {
    let calls = 0
    const stale = { ...snapshot(), status: { phase: "must_refetch" as const, cursor: 22, pendingCommandCount: 0 } }
    const program = buildHomeProgram({ coding: {
      ...codingBase,
      portableSnapshot: stale,
      requestPortableAction: async () => {
        calls += 1
        return { state: "queued" as const, snapshot: stale }
      },
    } })

    program.controller.inspectSession(sessionRef)
    program.controller.requestPortableControl("checkpoint")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const view = JSON.stringify(renderMobileControllerShell(state))

    expect(calls).toBe(0)
    expect(state.portableNotice?.kind).toBe("rejected")
    expect(view).toContain("Portable authority is not live on this device")
  })
})
