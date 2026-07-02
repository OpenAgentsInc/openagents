import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  buildKhalaFleetWorkerCards,
  createKhalaFleetWorkerCardThrottler,
  khalaFleetWorkerLifecycleFramesFromNdjson,
} from "../src/ui/fleet-worker-cards"
import { mountFleetPanel } from "../src/ui/fleet-status"
import type {
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopFleetWorkerControlRequest,
} from "../src/shared/rpc"

const assignmentEvent = (patch: Record<string, unknown>): string =>
  JSON.stringify({
    assignmentRef: "assignment.public.worker-card",
    event: "assignment_run.runtime_progress",
    observedAt: "2026-07-01T00:00:00.000Z",
    schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
    ...patch,
  })

const status = (): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt: "2026-07-01T00:00:05.000Z",
  pylon: {
    status: "online",
    pylonRef: "pylon.local.worker_card",
    message: "online",
  },
  availableCodexAssignments: 1,
  maxCodexAssignments: 2,
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "pending",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
  accounts: [],
  activeAssignments: [{
    assignmentRef: "assignment.public.worker-card",
    elapsedMs: 1_000,
    issueRef: "github.issue.openagents.7840",
    tokenRate: {
      source: "fleet.activeAssignments.tokensSoFar",
      status: "pending",
      tokenCountKind: null,
      tokens: null,
      tokensPerMinute: null,
    },
    updatedAt: "2026-07-01T00:00:05.000Z",
  }],
  processes: [],
})

const runProjection = {
  counters: {
    activeAssignments: 1,
    blockedAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    workUnitsTotal: 1,
  },
  createdAt: "2026-07-01T00:00:00.000Z",
  dispatchKind: "supervised_dispatch" as const,
  objectiveProjected: false as const,
  pylonRef: "pylon.local.worker_card",
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 1,
    stopCondition: "backlog_empty" as const,
  },
  runRef: "fleet_run.public.worker_card",
  startedAt: "2026-07-01T00:00:00.000Z",
  state: "running" as const,
  targetConcurrency: 1,
  updatedAt: "2026-07-01T00:00:00.000Z",
  workerKind: "codex" as const,
  workSource: { kind: "fixture" as const },
}

describe("Khala Fleet worker cards", () => {
  test("builds cards from fixture lifecycle streams without fabricated frames", async () => {
    const frames = await khalaFleetWorkerLifecycleFramesFromNdjson([
      "human log line",
      assignmentEvent({
        elapsedMs: 2_500,
        phase: "runtime_active",
        tokenCountKind: "exact",
        tokensSoFar: 42,
      }),
      JSON.stringify({ schema: "unknown" }),
    ].join("\n"))
    const cards = buildKhalaFleetWorkerCards(status(), frames)

    expect(frames).toHaveLength(1)
    expect(cards).toHaveLength(1)
    expect(cards[0]?.lifecycle?.line).toBe(
      "assignment_run.runtime_progress phase=runtime_active tokens=42",
    )
    expect(cards[0]?.tokenLabel).toBe("pending exact rows")
    expect(cards[0]?.assignmentRefHash).toMatch(/^ref\.[0-9a-f]{8}$/)
    expect(cards[0]?.claimedWorkUnit).toMatch(/^ref\.[0-9a-f]{8}$/)
  })

  test("throttles worker card updates and flushes the latest frame", () => {
    const callbacks: Array<() => void> = []
    const updates: Array<{ readonly event: string; readonly frames: readonly string[] }> = []
    const throttler = createKhalaFleetWorkerCardThrottler({
      intervalMs: 200,
      onUpdate: update => updates.push({
        event: update.frame.event,
        frames: update.frames.map(frame => `${frame.assignmentRef}:${frame.event}`),
      }),
      setTimeout: callback => {
        callbacks.push(callback)
        return callbacks.length
      },
      clearTimeout: () => undefined,
    })

    throttler.push({
      assignmentRef: "assignment.public.worker-card",
      elapsedMs: null,
      event: "assignment_run.runtime_started",
      line: "assignment_run.runtime_started",
      observedAt: "2026-07-01T00:00:00.000Z",
      tokenCountKind: null,
      tokensSoFar: null,
    })
    throttler.push({
      assignmentRef: "assignment.public.worker-card",
      elapsedMs: null,
      event: "assignment_run.runtime_progress",
      line: "assignment_run.runtime_progress",
      observedAt: "2026-07-01T00:00:01.000Z",
      tokenCountKind: null,
      tokensSoFar: null,
    })
    throttler.push({
      assignmentRef: "assignment.public.worker-card-b",
      elapsedMs: null,
      event: "assignment_run.completed",
      line: "assignment_run.completed",
      observedAt: "2026-07-01T00:00:02.000Z",
      tokenCountKind: null,
      tokensSoFar: null,
    })

    expect(updates).toEqual([])
    callbacks[0]?.()
    expect(updates.map(update => update.event)).toEqual([
      "assignment_run.runtime_progress",
      "assignment_run.completed",
    ])
    expect(updates.at(-1)?.frames).toEqual([
      "assignment.public.worker-card:assignment_run.runtime_progress",
      "assignment.public.worker-card-b:assignment_run.completed",
    ])

    throttler.push({
      assignmentRef: "assignment.public.worker-card",
      elapsedMs: null,
      event: "assignment_run.completed",
      line: "assignment_run.completed",
      observedAt: "2026-07-01T00:00:03.000Z",
      tokenCountKind: null,
      tokensSoFar: null,
    })
    callbacks[1]?.()
    expect(updates.at(-1)?.frames).toEqual([
      "assignment.public.worker-card:assignment_run.completed",
      "assignment.public.worker-card-b:assignment_run.completed",
    ])
  })

  test("renders worker controls through mocked RPC without exposing raw refs in the card", async () => {
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    const previousCrypto = globalThis.crypto
    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
    Object.defineProperty(globalThis, "crypto", {
      configurable: true,
      value: { randomUUID: () => "00000000-0000-4000-8000-000000000001" },
    })
    window.matchMedia = (() => ({ matches: true })) as unknown as typeof window.matchMedia

    const requests: KhalaCodeDesktopFleetWorkerControlRequest[] = []
    const container = document.createElement("div")
    const lifecycleUpdateCallbacks: Array<() => void> = []
    const lifecycleNdjson = async function* (): AsyncIterable<string> {
      yield `${assignmentEvent({
        elapsedMs: 3_000,
        phase: "runtime_active",
        tokenCountKind: "estimated",
        tokensSoFar: 9,
      })}\n`
    }
    const panel = mountFleetPanel(container, {
      connectAccount: async () => ({ ok: false, accountRef: "codex-test", error: "not used", output: "", userCode: null, verificationUrl: null }),
      delegateRun: async () => {
        throw new Error("not used")
      },
      fetch: async () => status(),
      fleetRunControl: async request => ({
        ok: true,
        previousState: "running",
        run: { ...runProjection, runRef: request.runRef },
        supervisorActive: true,
        verb: request.verb,
      }),
      fleetRunList: async () => ({ ok: true, runs: [runProjection] }),
      fleetRunStart: async () => {
        throw new Error("not used")
      },
      fleetWorkerControl: async request => {
        requests.push(request)
        return {
          accepted: true,
          assignmentRef: request.assignmentRef,
          inboxItemRef: request.verb === "flag" ? "inbox.assignment.test" : null,
          ok: true,
          runRef: request.runRef,
          verb: request.verb,
          workerRefHash: request.workerRefHash,
        }
      },
      lifecycleNdjson,
      lifecycleUpdateClock: {
        setTimeout: callback => {
          lifecycleUpdateCallbacks.push(callback)
          return lifecycleUpdateCallbacks.length
        },
        clearTimeout: () => undefined,
      },
      lifecycleUpdateThrottleMs: 0,
      loadGymDemoProof: async () => {
        throw new Error("not used")
      },
      openExternal: async () => true,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("not used")
      },
    })

    try {
      panel.setVisible(true)
      for (let index = 0; index < 50 && lifecycleUpdateCallbacks.length === 0; index += 1) {
        await Promise.resolve()
      }
      lifecycleUpdateCallbacks.splice(0).forEach(callback => callback())
      await Promise.resolve()
      expect(container.innerHTML).toContain("khala-fleet-worker-card")
      const cardHtml = container.querySelector<HTMLElement>(".khala-fleet-worker-card")?.innerHTML ?? ""
      expect(cardHtml).not.toContain("assignment.public.worker-card")
      expect(cardHtml).not.toContain("github.issue.openagents.7840")
      expect(cardHtml).toContain("assignment_run.runtime_progress")
      expect(cardHtml).toContain("tokens=9")
      container.querySelector<HTMLButtonElement>('[data-fleet-worker-control="flag"]')?.click()
      await Promise.resolve()
      expect(requests).toMatchObject([{
        assignmentRef: "assignment.public.worker-card",
        runRef: "fleet_run.public.worker_card",
        verb: "flag",
      }])
    } finally {
      panel.setVisible(false)
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "crypto", { configurable: true, value: previousCrypto })
    }
  })
})
