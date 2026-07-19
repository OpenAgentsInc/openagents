import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { createRef } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  CODEX_CHIP_REASON_POLICY_DENIED,
  CODEX_CHIP_REASON_QUOTA_EXHAUSTED,
  CODEX_CHIP_REASON_RATE_LIMITED,
  CODEX_CHIP_REASON_VERIFYING,
} from "../codex-local-contract.ts"
import { emptyGitPanelState, type GitPanelState } from "./git-panel.ts"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"

const restores: Array<() => void> = []
const installDom = (wide: boolean) => {
  const window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: () => ({
      matches: wide,
      media: "(min-width: 1120px)",
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    customElements: window.customElements,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    Event: window.Event,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    MutationObserver: window.MutationObserver,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return { window, container }
}

afterEach(async () => {
  await new Promise(resolve => setTimeout(resolve, 0))
  restores.splice(0).reverse().forEach(restore => restore())
})

const readyGit = (extra: Partial<GitPanelState> = {}): GitPanelState => ({
  ...emptyGitPanelState(),
  phase: "ready",
  status: {
    ok: true,
    op: "status",
    branch: "main",
    upstream: "origin/main",
    detached: false,
    ahead: 0,
    behind: 0,
    staged: [],
    unstaged: [{ path: "src/renderer.tsx", status: "modified" }],
    untracked: [{ path: "notes.txt", status: "untracked" }],
    truncated: false,
    repositoryRef: "workspace.repository.test",
    statusRef: "workspace.git-status.test",
    headRef: "a".repeat(40),
  },
  currentBranch: "main",
  ...extra,
})

const fixtureState = (extra: Partial<DesktopShellState> = {}): DesktopShellState => {
  const base = initialDesktopShellState("electron/darwin")
  return {
    ...base,
    selectedHarness: "codex",
    harnessLanes: { ...base.harnessLanes, codex: { available: true, reason: null } },
    git: readyGit(),
    ...extra,
  }
}

const recorder = () => {
  const received: Array<{ name: string; payload: unknown }> = []
  const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
  return { received, report }
}
const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 20))

describe("React status projection", () => {
  test("maps only canonical typed Codex reasons", async () => {
    const { projectReactStatusNotices } = await import("./react-review.tsx")
    const cases = [
      [CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT, "signed_out"],
      [CODEX_CHIP_REASON_QUOTA_EXHAUSTED, "quota_exhausted"],
      [CODEX_CHIP_REASON_RATE_LIMITED, "rate_limited"],
      [CODEX_CHIP_REASON_POLICY_DENIED, "policy_denied"],
    ] as const
    const checkingBase = fixtureState()
    expect(projectReactStatusNotices({
      ...checkingBase,
      harnessLanes: { ...checkingBase.harnessLanes, codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING } },
    })).toEqual([])
    for (const [reason, kind] of cases) {
      const base = fixtureState()
      const notices = projectReactStatusNotices({
        ...base,
        harnessLanes: { ...base.harnessLanes, codex: { available: false, reason } },
      })
      expect(notices[0]?.kind).toBe(kind)
    }
    const base = fixtureState()
    expect(projectReactStatusNotices({
      ...base,
      harnessLanes: { ...base.harnessLanes, codex: { available: false, reason: "opaque host failure" } },
    })[0]?.kind).toBe("failed")
    expect(projectReactStatusNotices({
      ...base,
      harnessLanes: { ...base.harnessLanes, codex: {
        available: false,
        reason: "Codex — configuration error",
        diagnostic: {
          kind: "invalid_config",
          detail: "/Users/me/.codex/config.toml:408:1: invalid transport",
        },
      } },
    })[0]).toMatchObject({
      kind: "invalid_config",
      title: "Codex configuration error",
      detail: "/Users/me/.codex/config.toml:408:1: invalid transport",
    })
  })

  test("projects workspace loss and incomplete history without claiming success", async () => {
    const { projectReactStatusNotices } = await import("./react-review.tsx")
    const base = fixtureState()
    const notices = projectReactStatusNotices({
      ...base,
      workspaceBrowser: { ...base.workspaceBrowser, phase: "unavailable", reason: "Grant unavailable." },
      history: {
        ...base.history,
        page: {
          rootThreadRef: "root",
          selectedThreadRef: "root",
          offset: 0,
          limit: 200,
          totalItems: 0,
          hasPrevious: false,
          hasNext: false,
          completeness: { source: 1, rendered: 0, redactions: 0, gaps: 1, complete: false },
          agents: [{
            threadRef: "root", parentThreadRef: null, title: "Root", status: "interrupted",
            createdAt: "2026-07-14T00:00:00Z", updatedAt: "2026-07-14T00:00:00Z",
            depth: 0, descendantCount: 0, model: null, role: null, nickname: null,
            agentPath: null, sourceVersion: null, reasoning: null, source: "codex",
          }],
          items: [],
        },
      },
    })
    expect(notices.map(notice => notice.kind)).toEqual(["revoked_grant", "stream_gap", "interrupted"])
  })

  test("keeps incompatible, offline, interrupted, and failed dispositions distinct", async () => {
    const { projectReactStatusNotices } = await import("./react-review.tsx")
    for (const kind of ["incompatible", "offline", "interrupted", "failed"] as const) {
      expect(projectReactStatusNotices(fixtureState({ runtimeFailure: kind }))[0]?.kind).toBe(kind)
    }
  })

  test("#8998: a Claude signed-out turn never asks the owner to sign in to Codex", async () => {
    const { projectReactStatusNotices } = await import("./react-review.tsx")
    const notice = projectReactStatusNotices(fixtureState({
      selectedHarness: "fable",
      activeLaneRef: "fable-local",
      runtimeFailure: "signed_out",
    }))[0]
    expect(notice).toMatchObject({
      kind: "signed_out",
      title: "Claude sign-in required",
      detail: "The admitted turn reported that its Claude account is unavailable. No alternate account was selected.",
    })
    expect(`${notice?.title} ${notice?.detail}`).not.toContain("Codex")
  })
})

describe("React repository review", () => {
  test("renders a wide adjacent read-only review and dispatches the exact relative path", async () => {
    const { container } = installDom(true)
    const { ReviewSurface } = await import("./react-review.tsx")
    const { received, report } = recorder()
    const root = createRoot(container)
    root.render(<ReviewSurface state={fixtureState()} report={report} open onOpenChange={() => {}} triggerRef={createRef()} />)
    await settle()
    expect(container.querySelector("aside[aria-label='Repository review']")).not.toBeNull()
    expect(container.textContent).toContain("Read-only review")
    expect(container.textContent).not.toContain("workspace.repository.test")
    const review = [...container.querySelectorAll("button")].find(button => button.textContent === "Review")
    review?.click()
    await settle()
    expect(received).toContainEqual({
      name: "GitPanelDiffRequested",
      payload: { path: "src/renderer.tsx", source: "unstaged" },
    })
    for (const forbidden of ["Stage", "Discard", "Commit", "Push", "Terminal"]) {
      expect([...container.querySelectorAll("button")].some(button => button.textContent === forbidden)).toBe(false)
    }
    root.unmount()
  })

  test("exposes stable refusal copy for every bounded review class", async () => {
    const { reviewFailurePresentation } = await import("./react-review.tsx")
    expect(reviewFailurePresentation("secret_diff")?.detail).toContain("withheld")
    expect(reviewFailurePresentation("stale_status")?.title).toBe("Stale review")
    expect(reviewFailurePresentation("binary_diff")?.title).toBe("Binary file")
    expect(reviewFailurePresentation("diff_too_large")?.title).toBe("Diff too large")
    expect(reviewFailurePresentation("unsafe_state")?.title).toBe("Unsafe repository state")
  })
})
