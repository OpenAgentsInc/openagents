import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type {
  KhalaCodeDesktopFleetRunControlRequest,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetRunStartRequest,
  KhalaCodeDesktopFleetRunStartResult,
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopKhalaSyncFleetMutateRequest,
  KhalaCodeDesktopKhalaSyncFleetStateResult,
} from "../src/shared/rpc"
import {
  buildKhalaFleetThroughputGauges,
  khalaFleetCountdownLabel,
  mountFleetPanel,
} from "../src/ui/fleet-status"

let previousDocument: typeof globalThis.document | undefined
let previousWindow: typeof globalThis.window | undefined
let previousMatchMedia: typeof globalThis.matchMedia | undefined

const status = (): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt: "2026-07-01T18:00:00.000Z",
  pylon: {
    message: "online",
    pylonRef: "pylon.public.fleet",
    status: "online",
  },
  availableCodexAssignments: 3,
  maxCodexAssignments: 4,
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    tokensWindow: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
  accounts: [
    {
      accountKey: null,
      accountRef: "codex-a",
      capacity: {
        available: 2,
        busy: 0,
        queued: 0,
        ready: 2,
      },
      email: "operator-a@example.com",
      paused: false,
      provider: "codex",
      quotaState: "available",
      rateLimits: {
        provider: "codex",
        session: {
          remainingPercent: 75,
          resetDescription: null,
          resetsAtIso: "2026-07-01T19:00:00.000Z",
          usedPercent: 25,
          windowMinutes: 300,
        },
        weekly: {
          remainingPercent: 40,
          resetDescription: null,
          resetsAtIso: "2026-07-03T18:00:00.000Z",
          usedPercent: 60,
          windowMinutes: 10_080,
        },
        rateLimitResetCredits: {
          availableCount: 1,
          nextExpiresAtIso: "2026-07-02T18:00:00.000Z",
        },
        updatedAtIso: "2026-07-01T18:00:00.000Z",
        error: null,
        status: "ok",
      },
      readiness: "ready",
    },
    {
      accountKey: null,
      accountRef: "codex-b",
      capacity: {
        available: 1,
        busy: 0,
        queued: 0,
        ready: 1,
      },
      email: "operator-b@example.com",
      paused: false,
      provider: "codex",
      quotaState: "available",
      readiness: "ready",
    },
    {
      accountKey: null,
      accountRef: "claude-a",
      capacity: {
        available: 1,
        busy: 0,
        queued: 0,
        ready: 1,
      },
      email: "operator-claude@example.com",
      paused: false,
      provider: "claude_agent",
      quotaState: "available",
      readiness: "ready",
    },
  ],
  activeAssignments: [],
  processes: [],
})

const fleetRunProjection = (
  input: Partial<KhalaCodeDesktopFleetRunProjection> = {},
): KhalaCodeDesktopFleetRunProjection => ({
    counters: {
      activeAssignments: 2,
      blockedAssignments: 0,
      completedAssignments: 4,
      failedAssignments: 0,
      workUnitsTotal: 10,
      ...input.counters,
    },
    createdAt: "2026-07-01T18:00:00.000Z",
    dispatchKind: "supervised_dispatch",
    objectiveProjected: false,
    pylonRef: "pylon.public.fleet",
    refillPolicy: {
      cooldownAware: true,
      maxPerAccount: 1,
      stopCondition: "backlog_empty",
    },
    runRef: "fleet.run.public.test",
    startedAt: "2026-07-01T18:00:01.000Z",
    state: "running",
    targetConcurrency: 3,
    updatedAt: "2026-07-01T18:00:31.000Z",
    workerKind: "codex",
    workSource: { kind: "fixture" },
    ...input,
})

const runStartResult = (
  request: KhalaCodeDesktopFleetRunStartRequest,
): KhalaCodeDesktopFleetRunStartResult => ({
  ok: true,
  run: fleetRunProjection({
    targetConcurrency: request.targetConcurrency,
    workerKind: request.workerKind ?? "codex",
    workSource: request.workSource,
  }),
  supervisorStarted: true,
})

const installDom = (): void => {
  const window = new Window()
  previousDocument = globalThis.document
  previousWindow = globalThis.window
  previousMatchMedia = globalThis.matchMedia
  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "matchMedia", {
    configurable: true,
    value: () => ({ matches: false }),
  })
}

const restoreDom = (): void => {
  Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
  Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
  Object.defineProperty(globalThis, "matchMedia", { configurable: true, value: previousMatchMedia })
}

const changeInput = (input: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement, value: string): void => {
  input.value = value
  input.dispatchEvent(new window.Event(input.tagName === "SELECT" ? "change" : "input", { bubbles: true }))
}

const clickButton = (root: HTMLElement, label: string): void => {
  const button = [...root.querySelectorAll<HTMLButtonElement>("button")]
    .find(item => item.textContent?.includes(label))
  expect(button).not.toBeUndefined()
  button!.click()
}

const flushPanelWork = async (): Promise<void> => {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

describe("Fleet status panel", () => {
  beforeEach(installDom)
  afterEach(restoreDom)

  test("computes throughput gauges from exact token rows", () => {
    expect(buildKhalaFleetThroughputGauges({
      activeAdjustedTokensPerMinute: 1_001,
      completedStatus: "exact",
      completedTokenRows: 3,
      completedTokensPerMinute: 250,
      tokensWindow: 2_500,
      inFlightTokens: 9_999,
      inFlightTokensPerMinute: 999,
      source: "pylon_khala_apm",
      unavailableReason: null,
    })).toEqual([
      {
        detail: "Exact token_usage_events window, 3 exact row(s).",
        label: "Tokens/min",
        state: "exact",
        value: "250/min",
      },
      {
        detail: "Exact token_usage_events window, 3 exact row(s).",
        label: "10-min total",
        state: "exact",
        value: "2,500",
      },
      {
        detail: "Exact token_usage_events window, 3 exact row(s).",
        label: "Projected/day",
        state: "exact",
        value: "360K",
      },
    ])
  })

  test("renders pending and not_measured throughput honesty states", () => {
    expect(buildKhalaFleetThroughputGauges({
      activeAdjustedTokensPerMinute: 3_000,
      completedStatus: "pending",
      completedTokenRows: null,
      completedTokensPerMinute: null,
      tokensWindow: null,
      inFlightTokens: 8_000,
      inFlightTokensPerMinute: 800,
      source: "pylon_khala_apm",
      unavailableReason: null,
    }).map(gauge => `${gauge.label}:${gauge.state}:${gauge.value}`)).toEqual([
      "Tokens/min:pending:pending",
      "10-min total:pending:pending",
      "Projected/day:pending:pending",
    ])

    expect(buildKhalaFleetThroughputGauges({
      activeAdjustedTokensPerMinute: null,
      completedStatus: "not_measured",
      completedTokenRows: null,
      completedTokensPerMinute: null,
      tokensWindow: null,
      inFlightTokens: null,
      inFlightTokensPerMinute: null,
      source: "unavailable",
      unavailableReason: "missing OPENAGENTS_AGENT_TOKEN",
    }).map(gauge => `${gauge.label}:${gauge.state}:${gauge.value}`)).toEqual([
      "Tokens/min:not_measured:not measured",
      "10-min total:not_measured:not measured",
      "Projected/day:not_measured:not measured",
    ])
  })

  test("does not synthesize gauges from active progress or in-flight estimates", () => {
    const gauges = buildKhalaFleetThroughputGauges({
      activeAdjustedTokensPerMinute: 50_000,
      completedStatus: "pending",
      completedTokenRows: null,
      completedTokensPerMinute: null,
      tokensWindow: null,
      inFlightTokens: 500_000,
      inFlightTokensPerMinute: 50_000,
      source: "pylon_khala_apm",
      unavailableReason: null,
    })

    expect(gauges.every(gauge => gauge.state === "pending")).toBe(true)
    expect(gauges.map(gauge => gauge.value)).not.toContain("50K/min")
    expect(gauges.map(gauge => gauge.value)).not.toContain("72M")
    expect(gauges.map(gauge => gauge.value)).not.toContain("500K")
  })

  test("renders account cards with rate-limit countdown data and account actions", async () => {
    expect(khalaFleetCountdownLabel(
      "2026-07-01T19:30:00.000Z",
      new Date("2026-07-01T18:00:00.000Z"),
    )).toBe("1h 30m")

    const root = document.createElement("div")
    const connected: string[] = []
    const paused: { accountRef: string; paused: boolean }[] = []
    const resetCredits: string[] = []
    let data = status()
    data = {
      ...data,
      accounts: data.accounts.map(account =>
        account.accountRef === "codex-b"
          ? {
              ...account,
              readiness: "credentials_missing",
              quotaState: "credentials_missing",
              email: null,
            }
          : account,
      ),
    }

    const panel = mountFleetPanel(root, {
      connectAccount: async accountRef => {
        connected.push(accountRef)
        return {
          ok: true,
          accountRef,
          output: "device auth started",
          userCode: "ABCD-1234",
          verificationUrl: "https://example.com/device",
        }
      },
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => data,
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => ({ ok: true, runs: [] }),
      fleetRunStart: async request => runStartResult(request),
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => true,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async request => {
        paused.push(request)
        data = {
          ...data,
          accounts: data.accounts.map(account =>
            account.accountRef === request.accountRef
              ? { ...account, paused: request.paused }
              : account,
          ),
        }
        return { ok: true }
      },
      consumeResetCredit: async request => {
        resetCredits.push(request.accountRef)
        return { ok: true }
      },
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()

    expect(root.textContent).toContain("codex-a")
    expect(root.textContent).toContain("2/2 free")
    expect(root.textContent).toContain("25% used / 75% remaining")
    expect(root.textContent).toContain("60% used / 40% remaining")
    expect(root.textContent).toContain("reset credits1")
    expect(root.textContent).toContain("Pause account")
    expect(root.textContent).toContain("Reconnect")

    clickButton(root, "Pause account")
    await flushPanelWork()
    expect(paused).toEqual([{ accountRef: "codex-a", paused: true }])
    expect(root.textContent).toContain("Resume planning")

    clickButton(root, "Reset credits")
    await flushPanelWork()
    expect(resetCredits).toEqual(["codex-a"])

    clickButton(root, "Reconnect")
    await flushPanelWork()
    expect(connected).toEqual(["codex-b"])
    expect(root.textContent).toContain("ABCD-1234")
    clickButton(root, "Cancel")
    await flushPanelWork()
  })

  test("previews the first FleetRun wave and starts through the mocked RPC", async () => {
    const root = document.createElement("div")
    const requests: KhalaCodeDesktopFleetRunStartRequest[] = []
    let activeRun: KhalaCodeDesktopFleetRunProjection | null = null
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({
        ok: false,
        accountRef: "codex-test",
        error: "disabled",
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => ({ ok: true, runs: activeRun === null ? [] : [activeRun] }),
      fleetRunStart: async request => {
        requests.push(request)
        const result = runStartResult(request)
        activeRun = result.run
        return result
      },
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()

    const form = root.querySelector<HTMLFormElement>('form[aria-label="Start fleet run"]')
    expect(form).not.toBeNull()
    changeInput(form!.elements.namedItem("objective") as HTMLTextAreaElement, "Burn down public fixture tasks.")
    changeInput(form!.elements.namedItem("targetConcurrency") as HTMLInputElement, "3")
    changeInput(form!.elements.namedItem("workSource") as HTMLSelectElement, "fixture")
    changeInput(form!.elements.namedItem("workerKind") as HTMLSelectElement, "codex")

    const previewButton = [...root.querySelectorAll<HTMLButtonElement>("button")]
      .find(button => button.textContent?.includes("Preview first wave"))
    expect(previewButton).not.toBeUndefined()
    previewButton!.click()

    expect(root.textContent).toContain("Planned first wave before starting.")
    expect(root.textContent).toContain("planned claim #1 (fixture)")
    expect(root.textContent).toContain("planned claim #2 (fixture)")
    expect(root.textContent).toContain("planned claim #3 (fixture)")
    expect(root.textContent).toContain("codex-a")
    expect(root.textContent).toContain("codex-b")

    form!.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }))
    await flushPanelWork()

    expect(requests).toEqual([
      {
        objective: "Burn down public fixture tasks.",
        targetConcurrency: 3,
        workerKind: "codex",
        workSource: { kind: "fixture" },
      },
    ])
    expect(root.textContent).toContain("fleet.run.public.test")
    expect(root.textContent).toContain("Running")
    expect(root.textContent).toContain("Burn down public fixture tasks.")
    expect(root.textContent).toContain("target3")
    expect(root.textContent).toContain("actual2")
    expect(root.textContent).toContain("remaining4")
    expect(root.textContent).toContain("claimed2")
    expect(root.textContent).toContain("done4")
    expect(root.textContent).toContain("elapsed30s")
  })

  test("starts Claude FleetRuns from the form without falling back to Codex", async () => {
    const root = document.createElement("div")
    const requests: KhalaCodeDesktopFleetRunStartRequest[] = []
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({
        ok: false,
        accountRef: "codex-test",
        error: "disabled",
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => ({ ok: true, runs: [] }),
      fleetRunStart: async request => {
        requests.push(request)
        return runStartResult(request)
      },
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()

    const form = root.querySelector<HTMLFormElement>('form[aria-label="Start fleet run"]')
    expect(form).not.toBeNull()
    changeInput(form!.elements.namedItem("objective") as HTMLTextAreaElement, "Burn down Claude fixture tasks.")
    changeInput(form!.elements.namedItem("targetConcurrency") as HTMLInputElement, "1")
    changeInput(form!.elements.namedItem("workSource") as HTMLSelectElement, "fixture")
    changeInput(form!.elements.namedItem("workerKind") as HTMLSelectElement, "claude")

    const previewButton = [...root.querySelectorAll<HTMLButtonElement>("button")]
      .find(button => button.textContent?.includes("Preview first wave"))
    expect(previewButton).not.toBeUndefined()
    previewButton!.click()

    const previewRows = [...root.querySelectorAll<HTMLElement>(".khala-fleet-run-preview-slot")]
      .map(row => row.textContent ?? "")
    expect(previewRows).toHaveLength(1)
    expect(previewRows[0]).toContain("claude-a")
    expect(previewRows[0]).toContain("workerclaude")

    form!.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }))
    await flushPanelWork()

    expect(requests).toEqual([
      {
        objective: "Burn down Claude fixture tasks.",
        targetConcurrency: 1,
        workerKind: "claude",
        workSource: { kind: "fixture" },
      },
    ])
  })

  test("renders the active FleetRun header from fleetRunList without fabricated objective text", async () => {
    const root = document.createElement("div")
    const activeRun = fleetRunProjection({
      counters: {
        activeAssignments: 1,
        blockedAssignments: 1,
        completedAssignments: 6,
        failedAssignments: 1,
        workUnitsTotal: 12,
      },
      state: "draining",
      targetConcurrency: 5,
      updatedAt: "2026-07-01T18:01:01.000Z",
    })
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({
        ok: false,
        accountRef: "codex-test",
        error: "disabled",
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => ({ ok: true, runs: [activeRun] }),
      fleetRunStart: async request => runStartResult(request),
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()

    expect(root.textContent).toContain("Active FleetRun")
    expect(root.textContent).toContain("Draining")
    expect(root.textContent).toContain("fleet.run.public.test")
    expect(root.textContent).toContain("Objective is not projected by the public-safe run status.")
    expect(root.textContent).toContain("target5")
    expect(root.textContent).toContain("actual1")
    expect(root.textContent).toContain("remaining3")
    expect(root.textContent).toContain("claimed1")
    expect(root.textContent).toContain("done6")
    expect(root.textContent).toContain("blocked1")
    expect(root.textContent).toContain("failed1")
  })

  test("keeps account data visible when fleetRunList fails and retries that section", async () => {
    const root = document.createElement("div")
    let listCalls = 0
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({
        ok: false,
        accountRef: "codex-test",
        error: "disabled",
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async () => {
        throw new Error("fleet run control should not be called")
      },
      fleetRunList: async () => {
        listCalls += 1
        if (listCalls === 1) throw new Error("fleetRunList failed with 500")
        return { ok: true, runs: [fleetRunProjection()] }
      },
      fleetRunStart: async request => runStartResult(request),
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()

    expect(root.textContent).toContain("Worker Codex accounts")
    expect(root.textContent).toContain("codex-a")
    expect(root.textContent).toContain("FleetRun list degraded")
    expect(root.textContent).toContain("fleetRunList failed with 500")
    expect(root.textContent).toContain("Retry")
    expect(root.textContent).not.toContain("Could not load fleet status")

    clickButton(root, "Retry")
    await flushPanelWork()

    expect(listCalls).toBe(2)
    expect(root.textContent).toContain("fleet.run.public.test")
    expect(root.textContent).not.toContain("FleetRun list degraded")
  })

  test("wires every FleetRun control transition through mocked RPC and renders returned state", async () => {
    const root = document.createElement("div")
    let currentRun = fleetRunProjection({ state: "running" })
    const controls: KhalaCodeDesktopFleetRunControlRequest[] = []
    const panel = mountFleetPanel(root, {
      connectAccount: async () => ({
        ok: false,
        accountRef: "codex-test",
        error: "disabled",
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async request => {
        controls.push(request)
        const nextState = request.verb === "pause"
          ? "paused"
          : request.verb === "resume"
            ? "running"
            : request.verb === "drain"
              ? "draining"
              : "stopped"
        currentRun = fleetRunProjection({
          counters: {
            activeAssignments: request.verb === "stop" ? 0 : 1,
            blockedAssignments: 0,
            completedAssignments: 7,
            failedAssignments: 0,
            workUnitsTotal: 10,
          },
          state: nextState,
          updatedAt: `2026-07-01T18:02:0${controls.length}.000Z`,
        })
        return {
          ok: true,
          previousState: request.verb === "resume" ? "paused" : "running",
          run: currentRun,
          supervisorActive: request.verb !== "stop",
          verb: request.verb,
        }
      },
      fleetRunList: async () => ({
        ok: true,
        runs: currentRun.state === "stopped" ? [] : [currentRun],
      }),
      fleetRunStart: async request => runStartResult(request),
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })

    await panel.refresh()
    expect(root.textContent).toContain("running")

    clickButton(root, "Pause")
    await flushPanelWork()
    expect(root.textContent).toContain("Paused")

    clickButton(root, "Resume")
    await flushPanelWork()
    expect(root.textContent).toContain("Running")

    clickButton(root, "Drain")
    await flushPanelWork()
    expect(root.textContent).toContain("Draining")

    clickButton(root, "Stop")
    await flushPanelWork()
    expect(root.textContent).not.toContain("Active FleetRun")

    expect(controls).toEqual([
      { runRef: "fleet.run.public.test", verb: "pause" },
      { runRef: "fleet.run.public.test", verb: "resume" },
      { runRef: "fleet.run.public.test", verb: "drain" },
      { runRef: "fleet.run.public.test", verb: "stop" },
    ])
  })

  // KS-6.2 (#8303): flag-gated Khala Sync consumption for the Fleet screen.
  const syncState = (
    input: Partial<KhalaCodeDesktopKhalaSyncFleetStateResult> = {},
  ): KhalaCodeDesktopKhalaSyncFleetStateResult => ({
    accounts: [],
    assignments: [],
    authState: "connected",
    cursor: 7,
    enabled: true,
    ok: true,
    pendingMutations: 0,
    phase: "live",
    reason: null,
    rejections: [],
    run: {
      counters: {
        activeAssignments: 5,
        blockedAssignments: 0,
        completedAssignments: 6,
        failedAssignments: 1,
        workUnitsTotal: 12,
      },
      desiredSlots: 8,
      runId: "fleet.run.public.test",
      startedAt: "2026-07-01T18:00:01.000Z",
      status: "running",
      updatedAt: "2026-07-01T18:05:00.000Z",
      workerKind: "codex",
    },
    workers: [],
    ...input,
  })

  const mountSyncPanel = (input: {
    readonly state: () => Promise<KhalaCodeDesktopKhalaSyncFleetStateResult>
    readonly mutations?: KhalaCodeDesktopKhalaSyncFleetMutateRequest[]
    readonly controls?: KhalaCodeDesktopFleetRunControlRequest[]
  }) => {
    const root = document.createElement("div")
    const panel = mountFleetPanel(root, {
      connectAccount: async accountRef => ({
        ok: true,
        accountRef,
        output: "",
        userCode: null,
        verificationUrl: null,
      }),
      delegateRun: async () => {
        throw new Error("delegate runner should not be called")
      },
      fetch: async () => status(),
      fleetRunControl: async request => {
        input.controls?.push(request)
        return {
          ok: true,
          previousState: "running",
          run: fleetRunProjection(),
          supervisorActive: true,
          verb: request.verb,
        }
      },
      fleetRunList: async () => ({ ok: true, runs: [fleetRunProjection()] }),
      fleetRunStart: async request => runStartResult(request),
      fleetWorkerControl: async () => {
        throw new Error("fleet worker control should not be called")
      },
      khalaSyncFleetState: async () => input.state(),
      khalaSyncFleetMutate: async request => {
        input.mutations?.push(request)
        return { ok: true }
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
      setAccountPaused: async () => ({ ok: true }),
      consumeResetCredit: async () => ({ ok: true }),
      startDelegationOptimization: async () => {
        throw new Error("optimization should not be called")
      },
    })
    return { root, panel }
  }

  test("khala sync enabled: active run header renders server truth from the synced fleet_run", async () => {
    const { root, panel } = mountSyncPanel({ state: async () => syncState() })
    await panel.refresh()
    await flushPanelWork()

    const chip = root.querySelector<HTMLElement>(".khala-fleet-sync-indicator")
    expect(chip).not.toBeNull()
    expect(chip!.dataset.khalaSyncPhase).toBe("live")
    expect(chip!.dataset.khalaSyncLive).toBe("true")
    expect(chip!.textContent).toBe("Khala Sync: Live")
    // Server truth wins over the local polling projection: desiredSlots 8
    // (local targetConcurrency was 3) and the synced counters.
    expect(root.textContent).toContain("target8")
    expect(root.textContent).toContain("khala sync")
  })

  test("khala sync disabled result: polling path renders untouched, no sync indicator", async () => {
    const { root, panel } = mountSyncPanel({
      state: async () => syncState({ enabled: false, phase: "disabled", run: null }),
    })
    await panel.refresh()
    await flushPanelWork()

    expect(root.querySelector(".khala-fleet-sync-indicator")).toBeNull()
    expect(root.textContent).toContain("target3")
    expect(root.textContent).toContain("orchestration store")
  })

  test("khala sync enabled: pause routes the operator intent through khalaSyncFleetMutate", async () => {
    const mutations: KhalaCodeDesktopKhalaSyncFleetMutateRequest[] = []
    const controlRequests: KhalaCodeDesktopFleetRunControlRequest[] = []
    const { root, panel } = mountSyncPanel({
      state: async () => syncState(),
      mutations,
      controls: controlRequests,
    })
    await panel.refresh()
    await flushPanelWork()

    clickButton(root, "Pause")
    await flushPanelWork()
    await flushPanelWork()

    expect(mutations).toEqual([
      { action: "pause", runId: "fleet.run.public.test" },
    ])
    // The local supervisor stays the enforcement path until the Pylon-side
    // intent consumer lands (#8302 honest v1 contract).
    expect(controlRequests[0]).toEqual({ runRef: "fleet.run.public.test", verb: "pause" })
  })

  test("khala sync in-band rejection is surfaced in the header state", async () => {
    const { root, panel } = mountSyncPanel({
      state: async () =>
        syncState({
          rejections: [
            {
              errorCode: "unauthorized_scope",
              messageSafe: "this fleet run scope belongs to a different user",
              mutationId: 3,
              mutatorName: "fleet.pauseRun",
              observedAt: "2026-07-01T18:06:00.000Z",
              runId: "fleet.run.public.test",
            },
          ],
        }),
    })
    await panel.refresh()
    await flushPanelWork()

    const rejection = root.querySelector<HTMLElement>(".khala-fleet-sync-rejection")
    expect(rejection).not.toBeNull()
    expect(rejection!.dataset.khalaSyncRejection).toBe("unauthorized_scope")
    expect(rejection!.textContent).toContain("fleet.pauseRun")
    expect(rejection!.textContent).toContain("unauthorized_scope")
  })
})
