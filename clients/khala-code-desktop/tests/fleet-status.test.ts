import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type {
  KhalaCodeDesktopFleetRunStartRequest,
  KhalaCodeDesktopFleetRunStartResult,
  KhalaCodeDesktopFleetStatus,
} from "../src/shared/rpc"
import { mountFleetPanel } from "../src/ui/fleet-status"

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
      provider: "codex",
      quotaState: "available",
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
      provider: "codex",
      quotaState: "available",
      readiness: "ready",
    },
  ],
  activeAssignments: [],
  processes: [],
})

const runStartResult = (
  request: KhalaCodeDesktopFleetRunStartRequest,
): KhalaCodeDesktopFleetRunStartResult => ({
  ok: true,
  run: {
    counters: {
      activeAssignments: 0,
      blockedAssignments: 0,
      completedAssignments: 0,
      failedAssignments: 0,
      workUnitsTotal: 0,
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
    targetConcurrency: request.targetConcurrency,
    updatedAt: "2026-07-01T18:00:01.000Z",
    workerKind: "codex",
    workSource: request.workSource,
  },
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

describe("Fleet status panel", () => {
  beforeEach(installDom)
  afterEach(restoreDom)

  test("previews the first FleetRun wave and starts through the mocked RPC", async () => {
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
      fleetRunStart: async request => {
        requests.push(request)
        return runStartResult(request)
      },
      loadGymDemoProof: () => {
        throw new Error("gym proof should not be called")
      },
      openExternal: async () => false,
      removeAccount: async () => ({ ok: true }),
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
    expect(root.textContent).toContain("claim.fixture.01")
    expect(root.textContent).toContain("claim.fixture.02")
    expect(root.textContent).toContain("claim.fixture.03")
    expect(root.textContent).toContain("codex-a")
    expect(root.textContent).toContain("codex-b")

    form!.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }))
    await Promise.resolve()
    await Promise.resolve()

    expect(requests).toEqual([
      {
        objective: "Burn down public fixture tasks.",
        targetConcurrency: 3,
        tickImmediately: false,
        workerKind: "codex",
        workSource: { kind: "fixture" },
      },
    ])
    expect(root.textContent).toContain("fleet.run.public.test")
    expect(root.textContent).toContain("running")
  })
})
