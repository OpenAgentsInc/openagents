import { afterEach, describe, expect, test } from "vite-plus/test"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"
import { WorkbenchShell, projectReactSessionRows } from "./react-primitive-adapters.tsx"

const restores: Array<() => void> = []
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    Event: window.Event,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
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

const historyRoot = (threadRef: string, title: string, updatedAt: string) => ({
  threadRef,
  parentThreadRef: null,
  title,
  status: "completed" as const,
  createdAt: updatedAt,
  updatedAt,
  depth: 0,
  descendantCount: 0,
  model: null,
  role: null,
  nickname: null,
  agentPath: null,
  sourceVersion: null,
  reasoning: null,
  source: "codex" as const,
})

const fixtureState = (): DesktopShellState => {
  const base = initialDesktopShellState("electron/darwin")
  const local = {
    id: "local-1",
    title: "Local session",
    updatedAt: "2026-07-14T12:00:00.000Z",
    notes: [],
  }
  return {
    ...base,
    threads: [local],
    activeThreadId: local.id,
    history: {
      ...base.history,
      hydrated: true,
      visibleRootCount: 1,
      catalog: {
        roots: [historyRoot("history-1", "Earlier session", "2026-07-14T11:00:00.000Z")],
        agents: [],
      },
    },
  }
}

describe("React workbench shell", () => {
  test("projects metadata before transcript hydration in one deterministic recency order", () => {
    const state = fixtureState()
    const rows = projectReactSessionRows(state, new Date("2026-07-14T12:01:00.000Z"))
    expect(rows.map(row => row.id)).toEqual(["local-1", "history-1"])
    expect(rows[0]).toMatchObject({ title: "Local session", selected: true, meta: "1m" })
    expect(state.history.page).toBeNull()
  })

  test("dispatches new, search, and select through the existing intent authority", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const root = createRoot(container)
    root.render(<WorkbenchShell state={fixtureState()} report={report} />)
    await new Promise(resolve => setTimeout(resolve, 0))
    ;[...container.querySelectorAll("button")].find(button => button.textContent === "New session")?.click()
    const search = container.querySelector('input[type="search"]') as HTMLInputElement
    const valueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set
    valueSetter?.call(search, "earlier")
    search.dispatchEvent(new window.Event("input", { bubbles: true }))
    ;(container.querySelector('[data-session-row][data-selected="true"]') as HTMLButtonElement).click()
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(received).toEqual(expect.arrayContaining([
      { name: "DesktopNewChat", payload: null },
      { name: "HistorySearchChanged", payload: "earlier" },
      { name: "DesktopChatSelected", payload: "local-1" },
    ]))
    root.unmount()
  })

  test("keeps paging, archive, recovery, and confirmed delete on existing intents", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    const session = {
      sessionRef: "session-1",
      workContextRef: "context-1",
      grantRef: "grant-1",
      projectRef: "project-1",
      repositoryRef: "repository-1",
      worktreeRef: "worktree-1",
      projectLabel: "OpenAgents",
      repositoryLabel: "openagents",
      worktreeLabel: "main",
      state: "idle" as const,
      lastActiveAt: "2026-07-14T12:00:00.000Z",
      recoveryReason: null,
    }
    const state: DesktopShellState = {
      ...base,
      history: {
        ...base.history,
        catalog: {
          ...base.history.catalog,
          roots: [
            ...base.history.catalog.roots,
            historyRoot("history-2", "Paged session", "2026-07-14T10:00:00.000Z"),
          ],
        },
      },
      codingCatalog: {
        ...base.codingCatalog,
        sessions: [
          session,
          { ...session, sessionRef: "session-2", repositoryLabel: "needs-recovery", state: "recovery_required", recoveryReason: "missing_worktree" },
        ],
        totalSessions: 101,
        nextOffset: 100,
      },
    }
    const root = createRoot(container)
    root.render(<WorkbenchShell state={state} report={report} />)
    await new Promise(resolve => setTimeout(resolve, 0))
    const click = (label: string): void => {
      const button = [...container.querySelectorAll("button")].find(value => value.textContent === label)
      expect(button, label).toBeDefined()
      button?.click()
    }
    click("Load more sessions")
    click("Load more workspaces")
    click("Archive")
    click("Recover")
    click("Delete")
    root.render(<WorkbenchShell state={{ ...state, codingSessionDeleteConfirmRef: session.sessionRef }} report={report} />)
    await new Promise(resolve => setTimeout(resolve, 0))
    click("Confirm delete")
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(received).toEqual(expect.arrayContaining([
      { name: "HistoryCatalogMoreRequested", payload: null },
      { name: "DesktopCodingCatalogMoreRequested", payload: null },
      { name: "DesktopCodingSessionArchived", payload: "session-1" },
      { name: "DesktopCodingSessionRecovered", payload: "session-2" },
      { name: "DesktopCodingSessionDeleteRequested", payload: "session-1" },
      { name: "DesktopCodingSessionDeleteConfirmed", payload: "session-1" },
    ]))
    root.unmount()
  })

  test("the overlay session rail closes on Escape and restores the trigger focus", async () => {
    const { window, container } = installDom()
    const report: IntentReporter = () => Effect.void
    const root = createRoot(container)
    root.render(<WorkbenchShell state={fixtureState()} report={report} />)
    await new Promise(resolve => setTimeout(resolve, 0))
    const trigger = container.querySelector(".oa-react-mobile-session-trigger") as HTMLButtonElement
    trigger.click()
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(trigger.getAttribute("aria-expanded")).toBe("true")
    window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Escape" }))
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(trigger.getAttribute("aria-expanded")).toBe("false")
    expect(window.document.activeElement).toBe(trigger)
    root.unmount()
  })
})
