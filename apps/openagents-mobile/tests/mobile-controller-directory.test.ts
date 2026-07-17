import { describe, expect, test } from "vite-plus/test"

import { projectMobileControllerDirectory } from "../src/coding/mobile-controller-directory"
import type { MobileCodingDirectory } from "../src/coding/mobile-coding-navigation"

const directory = (sessions: MobileCodingDirectory["sessions"]): MobileCodingDirectory => ({
  authority: "confirmed",
  phase: "live",
  cacheState: "current",
  offlineCache: {
    accounting: "live_confirmed",
    ownerScopeRef: "scope.user.owner.fixture",
    cachedRepositoryCount: 0,
    cachedSessionCount: 0,
    lastConfirmedCursor: 12,
  },
  repositories: [
    { repositoryRef: "repository.beta", projectRef: "project.beta", displayName: "Beta", sessionCount: 1 },
    { repositoryRef: "repository.alpha", projectRef: "project.alpha", displayName: "Alpha", sessionCount: 2 },
  ],
  sessions,
})

const session = (
  sessionRef: string,
  repositoryRef: string,
  lastActiveAt: string,
  overrides: Partial<MobileCodingDirectory["sessions"][number]> = {},
): MobileCodingDirectory["sessions"][number] => ({
  sessionRef,
  projectRef: repositoryRef.replace("repository", "project"),
  repositoryRef,
  worktreeRef: `worktree.${sessionRef}`,
  threadRef: `thread.${sessionRef}`,
  runRef: null,
  fleetRef: null,
  currentCheckpointRef: null,
  agentTopologyRef: null,
  canonicalEventCursor: 4,
  provider: { state: "known", providerRef: "provider.codex" },
  runtime: { state: "known", runtimeRef: "runtime.owner-local" },
  state: "active",
  lastActiveAt,
  ...overrides,
})

describe("contract openagents_mobile.controller_directory.v1", () => {
  test("orders recent work deterministically and groups repositories by display name", () => {
    const projection = projectMobileControllerDirectory(directory([
      session("session.alpha-old", "repository.alpha", "2026-07-17T10:00:00.000Z"),
      session("session.beta", "repository.beta", "2026-07-17T12:00:00.000Z"),
      session("session.alpha-new", "repository.alpha", "2026-07-17T12:00:00.000Z"),
    ]))

    expect(projection.summary).toEqual({ repositoryCount: 2, sessionCount: 3, attentionCount: 0 })
    expect(projection.recent.map(item => item.sessionRef)).toEqual([
      "session.alpha-new",
      "session.beta",
      "session.alpha-old",
    ])
    expect(projection.repositories.map(item => item.displayName)).toEqual(["Alpha", "Beta"])
    expect(projection.repositories[0]?.sessions.map(item => item.sessionRef)).toEqual([
      "session.alpha-new",
      "session.alpha-old",
    ])
  })

  test("preserves unavailable provider/runtime facts and isolates recovery attention", () => {
    const projection = projectMobileControllerDirectory(directory([
      session("session.provider", "repository.alpha", "2026-07-17T12:00:00.000Z", {
        provider: { state: "unavailable", reason: "not_projected" },
      }),
      session("session.runtime", "repository.alpha", "2026-07-17T11:00:00.000Z", {
        runtime: { state: "unavailable", reason: "not_attached" },
      }),
      session("session.recovery", "repository.beta", "2026-07-17T10:00:00.000Z", {
        state: "recovery_required",
      }),
    ]))

    expect(projection.recent.map(item => [item.sessionRef, item.targetReadiness])).toEqual([
      ["session.provider", "provider_unavailable"],
      ["session.runtime", "runtime_unavailable"],
      ["session.recovery", "recovery_required"],
    ])
    expect(projection.attention.map(item => item.sessionRef)).toEqual(["session.recovery"])
    expect(JSON.stringify(projection)).not.toContain("/Users/")
    expect(JSON.stringify(projection)).not.toContain("token")
  })

  test("never promotes withheld cached rows into controller authority", () => {
    const projection = projectMobileControllerDirectory({
      authority: "withheld",
      phase: "must_refetch",
      cacheState: "hidden_until_reconnect",
      offlineCache: {
        accounting: "withheld_counted",
        ownerScopeRef: "scope.user.owner.fixture",
        cachedRepositoryCount: 4,
        cachedSessionCount: 9,
        lastConfirmedCursor: 22,
      },
      repositories: [],
      sessions: [],
    })

    expect(projection.summary).toEqual({ repositoryCount: 0, sessionCount: 0, attentionCount: 0 })
    expect(projection.recent).toEqual([])
    expect(projection.repositories).toEqual([])
    expect(projection.attention).toEqual([])
    expect(projection.offlineCache.cachedSessionCount).toBe(9)
  })
})
