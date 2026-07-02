import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  mountUnifiedInboxPanel,
  projectUnifiedInbox,
  type UnifiedInboxSource,
} from "../src/ui/inbox"
import type { KhalaCodeDesktopFleetAssignment, KhalaCodeDesktopFleetStatus } from "../src/shared/rpc"

const observedAt = "2026-07-01T18:00:00.000Z"

const tokenRate = {
  source: "token_usage_events",
  status: "pending",
  tokenCountKind: null,
  tokens: null,
  tokensPerMinute: null,
} satisfies KhalaCodeDesktopFleetAssignment["tokenRate"]

const assignment = (
  blockerRefs: readonly string[],
): KhalaCodeDesktopFleetAssignment => ({
  assignmentRef: `assignment.public.${blockerRefs[0]?.replaceAll(".", "_") ?? "ready"}`,
  blockerRefs,
  elapsedMs: 12_000,
  issueRef: "github.issue.openagents.7843",
  runRef: "fleet.run.public.t5_5",
  tokenRate,
  updatedAt: "2026-07-01T18:01:00.000Z",
  workerSession: {
    approvalState: blockerRefs.some(ref => /approval/iu.test(ref))
      ? "approval_required"
      : blockerRefs.length > 0
        ? "blocked"
        : "none",
    blockerRefs: [...blockerRefs],
    closeoutStatus: null,
    executionRuntime: "codex_harness",
    homeRole: "pylon_isolated_worker_codex_home",
    queuePolicy: {
      admission: "pylon_capacity_gate",
      cooldown: blockerRefs.length > 0 ? "unknown" : "ready",
      refill: "pylon_presence_heartbeat",
      queued: null,
    },
    reviewState: blockerRefs.length > 0 ? "blocked" : "active",
    role: "swarm_worker_codex_session",
    transcriptRef: "transcript.public.t5_5",
  },
})

const fleet = (
  overrides: Partial<KhalaCodeDesktopFleetStatus> = {},
): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt,
  pylon: {
    message: "online",
    pylonRef: "pylon.public.t5_5",
    status: "online",
  },
  availableCodexAssignments: 1,
  maxCodexAssignments: 2,
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
  accounts: [{
    accountKey: null,
    accountRef: "codex",
    capacity: {
      available: 1,
      busy: 0,
      queued: 0,
      ready: 1,
    },
    email: null,
    provider: "codex",
    quotaState: "available",
    readiness: "ready",
  }],
  activeAssignments: [],
  processes: [],
  ...overrides,
})

const source = (
  fleetStatus: KhalaCodeDesktopFleetStatus,
): UnifiedInboxSource => ({
  fleet: fleetStatus,
  pylon: {
    ok: true,
    app: "Khala Code Desktop",
    available: true,
    capability: "pylon",
    observedAt,
    reason: "ready",
    status: "ready",
  },
  coding: {
    ok: true,
    app: "Khala Code Desktop",
    available: true,
    capability: "coding",
    observedAt,
    reason: "ready",
    status: "ready",
  },
  tokenAccounting: {
    ok: true,
    app: "Khala Code Desktop",
    available: true,
    capability: "token_accounting",
    observedAt,
    reason: "ready",
    status: "ready",
  },
})

describe("Unified Inbox fleet flags", () => {
  test("routes each T5.5 fleet flag kind to a public-safe Inbox row with responses", () => {
    const cases = [
      {
        kind: "approval_required",
        status: fleet({ activeAssignments: [assignment(["blocker.public.worker.approval_required"])] }),
        actions: ["approve", "reject", "open_fleet", "refresh"],
      },
      {
        kind: "run_blocked",
        status: fleet({ activeAssignments: [assignment(["blocker.public.worker.run_blocked"])] }),
        actions: ["resume", "open_fleet", "refresh"],
      },
      {
        kind: "credentials_missing",
        status: fleet({
          accounts: [{
            accountKey: null,
            accountRef: "codex-2",
            capacity: null,
            email: null,
            provider: "codex",
            quotaState: "credentials_missing",
            readiness: "credentials_missing",
          }],
        }),
        actions: ["reconnect", "open_fleet"],
      },
      {
        kind: "cooldown_all_accounts",
        status: fleet({
          availableCodexAssignments: 0,
          accounts: [{
            accountKey: null,
            accountRef: "codex",
            capacity: { available: 0, busy: 0, queued: 0, ready: 1 },
            email: null,
            provider: "codex",
            queuePolicy: {
              admission: "pylon_capacity_gate",
              cooldown: "cooling_down",
              refill: "pylon_presence_heartbeat",
              queued: 0,
            },
            quotaState: "cooling_down",
            readiness: "ready",
          }],
        }),
        actions: ["open_fleet", "refresh"],
      },
      {
        kind: "merge_conflict_wave",
        status: fleet({ activeAssignments: [assignment(["blocker.public.worker.merge_conflict_wave"])] }),
        actions: ["resume", "open_fleet", "refresh"],
      },
      {
        kind: "claim_expired",
        status: fleet({ activeAssignments: [assignment(["blocker.public.worker.claim_expired"])] }),
        actions: ["rerun", "resume", "open_fleet", "refresh"],
      },
    ] as const

    for (const item of cases) {
      const projection = projectUnifiedInbox(source(item.status))
      expect(projection.items).toContainEqual(expect.objectContaining({
        actions: item.actions,
        kind: item.kind,
        source: item.kind === "credentials_missing" || item.kind === "cooldown_all_accounts"
          ? "fleet"
          : "assignment",
      }))
      expect(JSON.stringify(projection.items)).not.toMatch(/operator@example\.com|\/Users\/|auth\.json|bearer|sk-/i)
    }
  })

  test("resume action invokes the run control hook for fleet flags", async () => {
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })

    const resumed: string[] = []
    try {
      const container = document.createElement("div")
      const panel = mountUnifiedInboxPanel(container, {
        fetch: async () => source(fleet({
          activeAssignments: [assignment(["blocker.public.worker.run_blocked"])],
        })),
        onOpenFleet: () => {},
        onOpenSettings: () => {},
        onReconnectAccount: () => {},
        onResumeRun: runRef => {
          resumed.push(runRef)
        },
      })
      panel.setVisible(true)
      await panel.refresh()

      container.querySelector<HTMLButtonElement>("[data-action=\"resume\"]")?.click()
      await Promise.resolve()

      expect(resumed).toEqual(["fleet.run.public.t5_5"])
    } finally {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    }
  })
})
