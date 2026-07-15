import { afterEach, describe, expect, test } from "vite-plus/test"
import { Window } from "happy-dom"
import { act, type ReactNode } from "react"
import type { Root } from "react-dom/client"
import { createRoot } from "react-dom/client"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"
import { WorkbenchShell, projectReactSessionRows } from "./react-primitive-adapters.tsx"
import { RedactedSensitiveText, redactedSensitivePlaceholder } from "./react-sensitive-text.tsx"

const restores: Array<() => void> = []
const roots = new Set<Root>()
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
    IS_REACT_ACT_ENVIRONMENT: true,
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
  await act(async () => {
    for (const root of roots) root.unmount()
    roots.clear()
    await new Promise(resolve => setTimeout(resolve, 0))
  })
  while (restores.length > 0) restores.pop()?.()
})

const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 20))
const createTestRoot = (container: HTMLDivElement): Root => {
  const root = createRoot(container)
  roots.add(root)
  return root
}
const render = async (root: Root, node: ReactNode): Promise<void> => {
  await act(async () => {
    root.render(node)
    await settle()
  })
}
const interact = async (interaction: () => void): Promise<void> => {
  await act(async () => {
    interaction()
    await settle()
  })
}

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
  test("shows the dev-stage badge beside the OpenAgents brand", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={() => Effect.void} />)
    const brand = container.querySelector(".oa-react-rail-brand")
    const stage = brand?.querySelector(".oa-react-rail-stage")
    expect(brand?.getAttribute("aria-label")).toBe("OpenAgents Dev")
    expect(stage?.textContent).toBe("Dev")
    expect(stage?.getAttribute("data-app-stage")).toBe("dev")
  })

  test("keeps sensitive account text redacted until an explicit click", async () => {
    const value = "owner.name@example.com"
    const placeholder = redactedSensitivePlaceholder(value)
    expect(placeholder).not.toBe(value)
    expect(placeholder).toHaveLength(value.length)
    expect(placeholder[placeholder.indexOf("@")] ?? null).toBe("@")
    expect(redactedSensitivePlaceholder(value)).toBe(placeholder)

    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <RedactedSensitiveText
      value={value}
      ariaLabel="Toggle account email visibility"
      revealTooltip="Click to reveal email"
      hideTooltip="Click to hide email"
    />)
    const toggle = container.querySelector<HTMLButtonElement>(".oa-react-sensitive-text")
    expect(toggle?.dataset.revealed).toBe("false")
    expect(toggle?.textContent).toBe(placeholder)
    expect(container.textContent).not.toContain(value)

    await interact(() => toggle?.click())
    expect(toggle?.dataset.revealed).toBe("true")
    expect(toggle?.textContent).toBe(value)
    await interact(() => toggle?.click())
    expect(toggle?.dataset.revealed).toBe("false")
    expect(toggle?.textContent).toBe(placeholder)
  })

  test("settings projects only Codex maintenance and Codex account identity", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const base = fixtureState()
    const state: DesktopShellState = {
      ...base,
      workspace: "settings",
      fleet: {
        ...base.fleet,
        phase: "ready",
        accounts: [
          { ref: "codex", provider: "codex", email: "owner@example.com", readiness: "ready" },
          { ref: "claude", provider: "claude_agent", email: "claude@example.com", readiness: "ready" },
        ],
      },
      settings: {
        ...base.settings,
        harnessMaintenance: {
          view: {
            state: "loaded",
            harnesses: [{
              harness: "codex",
              installed: true,
              installedVersion: "0.144.1",
              latestVersion: "0.144.4",
              channel: "npm-global",
              advisory: "behind_latest",
              updateSupported: true,
            }],
          },
          updating: null,
          lastOutcome: null,
          codexReleaseNotes: null,
        },
      },
    }
    await render(root, <WorkbenchShell state={state} report={() => Effect.void} />)
    expect([...container.querySelectorAll("[data-harness]")].map(node => node.getAttribute("data-harness"))).toEqual(["codex"])
    expect([...container.querySelectorAll("[data-provider-account]")].map(node => node.getAttribute("data-provider-account"))).toEqual(["codex"])
    expect(container.textContent).not.toContain("Claude")
    expect(container.textContent).not.toContain("claude@example.com")
    expect(container.querySelector(".oa-react-sensitive-text")?.getAttribute("data-revealed")).toBe("false")
  })

  test("shows the Codex-only update advisory and dispatches the typed update intent", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    const state: DesktopShellState = {
      ...base,
      settings: {
        ...base.settings,
        harnessMaintenance: {
          view: {
            state: "loaded",
            harnesses: [{
              harness: "codex",
              installed: true,
              installedVersion: "0.144.1",
              latestVersion: "0.144.4",
              channel: "npm-global",
              advisory: "behind_latest",
              updateSupported: true,
            }],
          },
          updating: null,
          lastOutcome: null,
          codexReleaseNotes: {
            version: "0.144.4",
            title: "Codex 0.144.4",
            body: "## Changelog\n\nStreaming improvements.",
            publishedAt: "2026-07-15T00:00:00Z",
          },
        },
      },
    }
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={report} />)
    expect(container.querySelector(".oa-react-codex-update-notice")?.textContent).toContain("Codex update available")
    const update = [...container.querySelectorAll<HTMLButtonElement>("button")].find(button => button.textContent?.includes("Update"))
    await interact(() => update?.click())
    expect(received).toContainEqual({ name: "DesktopHarnessUpdateRequested", payload: "codex" })
    expect(container.querySelector(".oa-react-codex-update-notice")).toBeNull()
  })

  test("centers the empty conversation prompt with the current working directory", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const state = {
      ...fixtureState(),
      workingDirectory: "/Users/example/project",
    }
    await render(root, <WorkbenchShell state={state} report={() => Effect.void} />)
    const empty = container.querySelector(".oa-react-timeline-empty")
    expect(empty?.textContent).toContain("Start a conversation with Codex")
    expect(empty?.textContent).toContain("/Users/example/project")
    expect(empty?.querySelector('[data-icon-name="Folder"]')).not.toBeNull()
  })

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
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={report} />)
    expect(container.querySelector('input[type="search"]')).toBeNull()
    const searchTrigger = container.querySelector<HTMLButtonElement>('[aria-label="Search sessions"]')
    expect(searchTrigger?.querySelector('[data-icon-name="Search"]')).not.toBeNull()
    await interact(() => searchTrigger?.click())
    const searchState = fixtureState()
    await render(root, <WorkbenchShell state={{
      ...searchState,
      presentation: { ...searchState.presentation, sessionSearchOpen: true },
    }} report={report} />)
    const sessionRows = [...container.querySelectorAll<HTMLButtonElement>('[data-session-row]')]
    expect(sessionRows.length).toBeGreaterThan(0)
    for (const row of sessionRows) {
      expect(row.classList.contains("justify-start")).toBe(true)
      expect(row.classList.contains("text-left")).toBe(true)
      expect(row.querySelector(".oa-react-session-title")).not.toBeNull()
      expect(row.querySelector(".oa-react-session-meta")).not.toBeNull()
    }
    await interact(() => {
      ;[...container.querySelectorAll("button")].find(button => button.textContent === "New session")?.click()
    })
    const search = container.querySelector('input[type="search"]') as HTMLInputElement
    expect(window.document.activeElement).toBe(search)
    const valueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set
    valueSetter?.call(search, "earlier")
    await interact(() => search.dispatchEvent(new window.Event("input", { bubbles: true })))
    await interact(() => {
      ;(container.querySelector('[data-session-row][data-selected="true"]') as HTMLButtonElement).click()
    })
    expect(received).toEqual(expect.arrayContaining([
      { name: "DesktopSessionSearchDisclosureChanged", payload: true },
      { name: "DesktopNewChat", payload: null },
      { name: "HistorySearchChanged", payload: "earlier" },
      { name: "DesktopChatSelected", payload: "local-1" },
    ]))
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
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={report} />)
    const click = async (label: string): Promise<void> => {
      const button = [...container.querySelectorAll("button")].find(value => value.textContent === label)
      expect(button, label).toBeDefined()
      await interact(() => button?.click())
    }
    await click("Load more sessions")
    await click("Load more workspaces")
    await click("Archive")
    await click("Recover")
    await click("Delete")
    await render(root, <WorkbenchShell state={{ ...state, codingSessionDeleteConfirmRef: session.sessionRef }} report={report} />)
    await click("Confirm delete")
    expect(received).toEqual(expect.arrayContaining([
      { name: "HistoryCatalogMoreRequested", payload: null },
      { name: "DesktopCodingCatalogMoreRequested", payload: null },
      { name: "DesktopCodingSessionArchived", payload: "session-1" },
      { name: "DesktopCodingSessionRecovered", payload: "session-2" },
      { name: "DesktopCodingSessionDeleteRequested", payload: "session-1" },
      { name: "DesktopCodingSessionDeleteConfirmed", payload: "session-1" },
    ]))
  })

  test("the overlay session rail closes on Escape and restores the trigger focus", async () => {
    const { container } = installDom()
    const report: IntentReporter = () => Effect.void
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={report} />)
    const trigger = container.querySelector(".oa-react-sidebar-expand") as HTMLButtonElement
    await interact(() => trigger.click())
    expect(trigger.getAttribute("aria-expanded")).toBe("true")
    await interact(() => window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Escape" })))
    expect(trigger.getAttribute("aria-expanded")).toBe("false")
    expect(window.document.activeElement).toBe(trigger)
  })

  test("uses closed-catalog icon controls and left-aligned sidebar actions", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={() => Effect.void} />)
    expect(container.querySelector('[data-icon-name="Menu"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChatCompose"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="Chats"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChevronLeft"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChevronRight"]')).not.toBeNull()
    const newSession = [...container.querySelectorAll<HTMLButtonElement>("button")]
      .find(button => button.textContent === "New session")
    expect(newSession?.classList.contains("justify-start")).toBe(true)
    expect(newSession?.classList.contains("text-left")).toBe(true)
    expect(container.querySelector('[aria-current="page"]')?.textContent).toContain("Chat")
    expect(container.querySelector(".oa-react-section-label")?.textContent).toBe("Recent")
    expect(container.textContent).not.toContain("Search sessionsSearch Codex sessions")
  })

  test("renders the exact shared destination order, real workspace roots, and recoverable collapse", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const root = createTestRoot(container)
    const chat = fixtureState()
    await render(root, <WorkbenchShell state={chat} report={report} />)
    const destinations = () => [...container.querySelectorAll<HTMLButtonElement>("[data-sidebar-destination-id]")]
    expect(destinations().map(row => row.dataset.sidebarDestinationId)).toEqual([
      "workspace-new-chat",
      "workspace-chat",
      "workspace-home",
      "shell-settings-toggle",
    ])
    expect(destinations().map(row => row.querySelector("[data-icon-name]")?.getAttribute("data-icon-name"))).toEqual([
      "ChatCompose", "Chats", "Home", "Settings",
    ])
    await interact(() => destinations()[2]?.click())
    expect(received.at(-1)).toEqual({ name: "DesktopWorkspaceSelected", payload: "home" })
    await render(root, <WorkbenchShell state={{ ...chat, workspace: "home" }} report={report} />)
    expect(container.querySelector('[data-react-workspace="home"] h1')?.textContent).toBe("Coding sessions")
    expect(container.querySelector('[data-sidebar-destination-id="workspace-home"]')?.getAttribute("aria-current")).toBe("page")
    await interact(() => destinations()[3]?.click())
    expect(received.at(-1)).toEqual({ name: "DesktopSettingsToggled", payload: null })
    await render(root, <WorkbenchShell state={{ ...chat, workspace: "settings" }} report={report} />)
    expect(container.querySelector('[data-react-workspace="settings"] h1')?.textContent).toBe("Settings")

    await interact(() => container.querySelector<HTMLButtonElement>(".oa-react-rail-collapse")?.click())
    expect(received.at(-1)).toEqual({ name: "DesktopSidebarCollapsedChanged", payload: true })
    await render(root, <WorkbenchShell state={{
      ...chat,
      presentation: { sidebarCollapsed: true, sessionSearchOpen: false },
    }} report={report} />)
    expect(container.querySelector(".oa-react-workbench")?.getAttribute("data-rail-collapsed")).toBe("true")
    expect(container.querySelector<HTMLButtonElement>(".oa-react-sidebar-expand")).not.toBeNull()
    expect(container.querySelector('input[type="search"]')).toBeNull()
  })

  test("projects authoritative back/forward availability and dispatches one typed intent per click", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={report} />)
    const disabledBack = container.querySelector<HTMLButtonElement>('[aria-label="Back"]')
    const disabledForward = container.querySelector<HTMLButtonElement>('[aria-label="Forward"]')
    expect(disabledBack?.disabled).toBe(true)
    expect(disabledForward?.disabled).toBe(true)
    await interact(() => disabledBack?.click())
    expect(received).toEqual([])

    const enabled: DesktopShellState = {
      ...fixtureState(),
      navigation: {
        canGoBack: true,
        canGoForward: true,
        backTitle: "Earlier session",
        forwardTitle: "Project home",
      },
    }
    await render(root, <WorkbenchShell state={enabled} report={report} />)
    const back = container.querySelector<HTMLButtonElement>('[aria-label="Back to Earlier session"]')
    const forward = container.querySelector<HTMLButtonElement>('[aria-label="Forward to Project home"]')
    expect(back?.disabled).toBe(false)
    expect(forward?.disabled).toBe(false)
    expect(back?.title).toBe("Back to Earlier session")
    expect(forward?.title).toBe("Forward to Project home")
    await interact(() => back?.click())
    await interact(() => forward?.click())
    expect(received).toEqual([
      { name: "DesktopNavigationBackRequested", payload: null },
      { name: "DesktopNavigationForwardRequested", payload: null },
    ])
  })
})
