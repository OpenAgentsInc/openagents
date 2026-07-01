import { describe, expect, test } from "bun:test"

import {
  buildKhalaFleetBoardProjection,
  KhalaFleetBoardProjectionUnsafe,
} from "../src/ui/fleet-board-projection"
import type { KhalaCodeDesktopFleetStatus } from "../src/shared/rpc"

const fleetStatus = (
  overrides: Partial<KhalaCodeDesktopFleetStatus> = {},
): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt: "2026-06-30T20:00:00.000Z",
  pylon: {
    status: "online",
    pylonRef: "pylon.local.test",
    message: "online",
  },
  availableCodexAssignments: 1,
  maxCodexAssignments: 3,
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
  accounts: [
    {
      accountRef: "codex",
      provider: "codex",
      readiness: "ready",
      quotaState: "available",
      accountKey: "account_key_public",
      capacity: null,
      email: "operator@example.com",
    },
    {
      accountRef: "codex-2",
      provider: "codex",
      readiness: "credentials_missing",
      quotaState: null,
      accountKey: null,
      capacity: null,
      email: null,
    },
  ],
  activeAssignments: [
    {
      assignmentRef: "assignment.public.one",
      elapsedMs: null,
      issueRef: "github.issue.openagents.7768",
      tokenRate: {
        source: "unavailable",
        status: "not_measured",
        tokenCountKind: null,
        tokens: null,
        tokensPerMinute: null,
      },
      updatedAt: "2026-06-30T20:01:00.000Z",
    },
  ],
  processes: [
    {
      pid: "200",
      parentPid: "199",
      elapsed: "00:02:03",
    },
  ],
  ...overrides,
})

describe("Khala Code Fleet board projection", () => {
  test("maps fleet status into a board graph and run timeline", () => {
    const projection = buildKhalaFleetBoardProjection({
      status: fleetStatus(),
      generatedAt: "time.test.fleet_board",
    })

    expect(projection.schemaVersion).toBe(
      "openagents.khala_code.fleet_board_projection.v0",
    )
    expect(projection.summary).toMatchObject({
      readyAccounts: 1,
      totalAccounts: 2,
      activeAssignments: 1,
      runningProcesses: 1,
      availableCodexAssignments: 1,
      maxCodexAssignments: 3,
    })
    expect(projection.graph.status).toBe("active")
    expect(projection.graph.nodes.map(node => node.id)).toEqual([
      "khala-code",
      "main-codex-session",
      "local-pylon",
      "capacity-gate",
      "codex-workers",
      "active-assignments",
      "codex-processes",
      "run-timeline",
    ])
    expect(
      projection.graph.links.map(link => [link.id, link.status]),
    ).toContainEqual(["khala-to-main-codex", "evidence_backed"])
    expect(
      projection.graph.links.map(link => [link.id, link.status]),
    ).toContainEqual(["workers-to-assignments", "evidence_backed"])
    expect(projection.timeline.map(event => event.id)).toEqual([
      "account-1",
      "account-2",
      "capacity",
      "main-codex-session",
      "process-1",
      "pylon-status",
      "assignment-1",
    ])
    expect(projection.caveatRefs).toContain("caveat.khala_fleet.main_session_not_worker")
    expect(projection.timeline.some(event => event.label === "Assignment active"))
      .toBe(true)
    expect(projection.timeline.some(event => event.detail.includes("github.issue.openagents.7768")))
      .toBe(true)
  })

  test("does not project signed-in email, local paths, or raw provider fields", () => {
    const projection = buildKhalaFleetBoardProjection({
      status: fleetStatus({
        pylon: {
          status: "online",
          pylonRef: "pylon.local.test",
          message: "/Users/operator/.codex/auth.json bearer sk-local",
        },
        accounts: [
          {
            accountRef: "/Users/operator/private",
            provider: "codex",
            readiness: "ready",
            quotaState: null,
            accountKey: "credential.provider.token",
            capacity: null,
            email: "operator@example.com",
          },
        ],
        activeAssignments: [
          {
            assignmentRef: "raw_prompt.full",
            elapsedMs: null,
            issueRef: "https://private.example/issue",
            tokenRate: {
              source: "unavailable",
              status: "not_measured",
              tokenCountKind: null,
              tokens: null,
              tokensPerMinute: null,
            },
            updatedAt: "2026-06-30T20:01:00.000Z",
          },
        ],
      }),
    })
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toMatch(
      /operator@example\.com|\/Users\/|auth\.json|bearer|sk-local|credential\.provider\.token|raw_prompt|private\.example/i,
    )
    expect(serialized).toContain("account.khala_fleet.codex.1")
    expect(serialized).toContain("assignment.khala_fleet.pending.1")
  })

  test("marks missing Pylon capacity as blocked with blocker refs", () => {
    const projection = buildKhalaFleetBoardProjection({
      status: fleetStatus({
        pylon: {
          status: "unavailable",
          pylonRef: null,
          message: "offline",
        },
        availableCodexAssignments: 0,
        maxCodexAssignments: 2,
        activeAssignments: [],
        processes: [],
      }),
    })

    expect(projection.graph.status).toBe("blocked")
    expect(projection.blockerRefs).toContain("blocker.khala_fleet.pylon_unavailable")
    expect(
      projection.timeline.find(event => event.id === "pylon-status")?.status,
    ).toBe("blocked")
  })

  test("rejects unsafe generated timestamps before rendering", () => {
    expect(() =>
      buildKhalaFleetBoardProjection({
        status: fleetStatus(),
        generatedAt: "Bearer sk-local",
      }),
    ).toThrow(KhalaFleetBoardProjectionUnsafe)
  })
})
