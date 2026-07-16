import { afterEach, describe, expect, test } from "vite-plus/test"
import { Window } from "happy-dom"
import { act, StrictMode, type ReactNode } from "react"
import type { Root } from "react-dom/client"
import { createRoot } from "react-dom/client"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"
import { WorkbenchShell, projectReactSessionRows, projectSidebarMeter } from "./react-primitive-adapters.tsx"
import { RedactedSensitiveText, redactedSensitivePlaceholder } from "./react-sensitive-text.tsx"

const restores: Array<() => void> = []
const roots = new Set<Root>()
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Text: window.Text,
    Document: window.Document,
    Range: window.Range,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLDivElement: window.HTMLDivElement,
    HTMLInputElement: window.HTMLInputElement,
    HTMLTextAreaElement: window.HTMLTextAreaElement,
    Event: window.Event,
    InputEvent: window.InputEvent,
    CompositionEvent: window.CompositionEvent,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    MutationObserver: window.MutationObserver,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
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

  test("does not expose repository review in the core workbench", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={() => Effect.void} />)
    expect(container.querySelector('.oa-react-review-trigger')).toBeNull()
    expect(container.querySelector('[data-review-surface]')).toBeNull()
    expect(container.textContent).not.toContain("Repository review")
    expect(container.textContent).not.toContain("Review changes")
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

  test("owner-review settings renders and dispatches the default-off usage consent", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) =>
      Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    await render(root, <WorkbenchShell state={{
      ...base,
      workspace: "settings",
      settings: {
        ...base.settings,
        localCodexUsageControlAvailable: true,
        shareLocalCodexUsage: false,
      },
    }} report={report} />)

    const button = [...container.querySelectorAll("button")]
      .find(node => node.textContent === "Sharing off")
    expect(container.textContent).toContain(
      "never your prompts, responses, files, paths, account names, or credentials",
    )
    expect(button?.getAttribute("aria-pressed")).toBe("false")
    await interact(() => button?.click())
    expect(received.at(-1)).toEqual({
      name: "DesktopLocalCodexUsageSharingToggled",
      payload: true,
    })
  })

  test("renders and dispatches the complete OpenAgents account-linking state model", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) =>
      Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    const renderPhase = async (phase: DesktopShellState["settings"]["openAgentsSession"]) => {
      await render(root, <WorkbenchShell state={{
        ...base,
        workspace: "settings",
        settings: { ...base.settings, openAgentsSession: phase, shareLocalCodexUsage: false },
      }} report={report} />)
    }

    await renderPhase("signed_out")
    expect(container.querySelector('[data-session-phase="signed_out"]')?.textContent).toContain(
      "GitHub password never enters Desktop",
    )
    expect(container.textContent).toContain("linking does not turn on local usage sharing")
    await interact(() => [...container.querySelectorAll("button")]
      .find(node => node.textContent === "Link OpenAgents account")?.click())
    expect(received.at(-1)).toEqual({ name: "DesktopOpenAgentsSignInRequested", payload: null })

    await renderPhase("authenticating")
    expect(container.querySelector('[data-session-phase="authenticating"] button')?.hasAttribute("disabled")).toBe(true)
    expect(container.textContent).toContain("Waiting for secure browser…")

    await renderPhase("cancelled")
    expect(container.querySelector('[data-session-phase="cancelled"] [role="alert"]')?.textContent)
      .toBe("Account linking not completed")
    expect(container.textContent).toContain("No account was linked")

    await renderPhase("unavailable")
    expect(container.querySelector('[data-session-phase="unavailable"] [role="alert"]')?.textContent)
      .toBe("Couldn’t link account")

    await renderPhase("session_ready")
    expect(container.textContent).toContain("Linking never changes the local usage sharing setting")
    await interact(() => [...container.querySelectorAll("button")]
      .find(node => node.textContent === "Disconnect account")?.click())
    expect(received.at(-1)).toEqual({ name: "DesktopOpenAgentsSignOutRequested", payload: null })
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

  test("centers the empty conversation path and dispatches its accessible Change action only while empty", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const state = {
      ...fixtureState(),
      workingDirectory: "/Users/example/project",
    }
    await render(root, <WorkbenchShell state={state} report={report} />)
    const empty = container.querySelector(".oa-react-timeline-empty")
    expect(empty?.textContent).toContain("Start a conversation with Codex")
    expect(empty?.textContent).toContain("/Users/example/project")
    expect(empty?.querySelector('[data-icon-name="Folder"]')).not.toBeNull()
    const change = empty?.querySelector<HTMLButtonElement>('[aria-label="Change working directory"]')
    expect(change?.textContent).toBe("Change")
    await interact(() => change?.click())
    expect(received).toContainEqual({ name: "DesktopWorkspacePickerRequested", payload: null })

    await render(root, <WorkbenchShell state={{
      ...state,
      notes: [{ key: "owner-1", role: "user", text: "Hello", timestamp: "now" }],
    }} report={report} />)
    expect(container.querySelector('[aria-label="Change working directory"]')).toBeNull()
  })

  test("projects metadata before transcript hydration in one deterministic recency order", () => {
    const state = fixtureState()
    const rows = projectReactSessionRows(state, new Date("2026-07-14T12:01:00.000Z"))
    expect(rows.map(row => row.id)).toEqual(["local-1", "history-1"])
    expect(rows[0]).toMatchObject({ title: "Local session", selected: true, meta: "1m" })
    expect(rows[1]).toMatchObject({ title: "Earlier session", meta: "1h" })
    expect(rows.every(row => /^(?:now|\d+[mhd])$/u.test(row.meta))).toBe(true)
    expect(rows.map(row => row.meta).join(" ")).not.toMatch(/completed|running|waiting|title|content/iu)
    expect(state.history.page).toBeNull()

    const hinted = projectReactSessionRows({
      ...state,
      historyShortcutHintsVisible: true,
    }, new Date("2026-07-14T12:01:00.000Z"))
    expect(hinted.map(row => row.meta)).toEqual(["⌘1", "⌘2"])
    expect(hinted.map(row => row.id)).toEqual(rows.map(row => row.id))
  })

  test("projects the sidebar meter shape, nesting token fields under usage (T11 #8868)", () => {
    const state = initialDesktopShellState("electron/darwin", "18:04")
    expect(projectSidebarMeter(state)).toBeUndefined()

    const withMeter = projectSidebarMeter({
      ...state,
      meter: {
        inputTokens: 100,
        outputTokens: 20,
        totalTokens: 120,
        rateLimits: [{ label: "primary", usedPercent: 12 }],
      },
    })
    expect(withMeter).toEqual({
      usage: { inputTokens: 100, outputTokens: 20, totalTokens: 120 },
      rateLimits: [{ label: "primary", usedPercent: 12 }],
    })

    // No rate limits observed yet: the key is absent, not an empty array.
    const tokensOnly = projectSidebarMeter({ ...state, meter: { totalTokens: 42 } })
    expect(tokensOnly).toEqual({ usage: { totalTokens: 42 } })
    expect(tokensOnly).not.toHaveProperty("rateLimits")
  })

  test("mounts the live token meter in the fixed rail footer above Settings", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={{
      ...fixtureState(),
      meter: {
        inputTokens: 100,
        outputTokens: 20,
        totalTokens: 120,
        rateLimits: [{ label: "primary", usedPercent: 12 }],
      },
    }} report={() => Effect.void} />)

    expect(container.querySelector(".oa-react-conversation-header .oa-react-meter")).toBeNull()
    const sidebarMeter = container.querySelector(".oa-react-sidebar-meter .oa-react-meter")
    const settings = container.querySelector('nav[aria-label="Settings"]')
    expect(sidebarMeter).not.toBeNull()
    expect(settings).not.toBeNull()
    if (sidebarMeter === null || settings === null) throw new Error("sidebar meter and Settings must both render")
    expect(sidebarMeter.textContent).toContain("120 TOKENS")
    expect(sidebarMeter.textContent).toContain("PRIMARY")
    expect(sidebarMeter.compareDocumentPosition(settings) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0)
  })

  test("renders exactly one active background across destinations and conversation rows", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const state = fixtureState()
    await render(root, <WorkbenchShell state={{
      ...state,
      history: { ...state.history, pendingThreadRef: "history-1" },
    }} report={() => Effect.void} />)
    const selected = [...container.querySelectorAll<HTMLElement>('[data-selected="true"]')]
    expect(selected).toHaveLength(1)
    expect(selected[0]?.getAttribute("data-session-row")).not.toBeNull()
    expect(selected[0]?.textContent).toContain("Earlier session")
    expect(container.querySelector('[data-sidebar-destination-id="workspace-chat"]')).toBeNull()
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

  test("keeps conversation paging while workspace management stays out of the sidebar", async () => {
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
    expect(received).toEqual([{ name: "HistoryCatalogMoreRequested", payload: null }])
    for (const removedLabel of ["Load more workspaces", "Archive", "Recover", "Delete", "Confirm delete"]) {
      expect([...container.querySelectorAll("button")].some(value => value.textContent === removedLabel)).toBe(false)
    }
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

  test("Command-Enter toggles the same sidebar state as the visible control", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={{
      ...fixtureState(),
      presentation: { sidebarCollapsed: true, sessionSearchOpen: false },
    }} report={report} />)
    const trigger = container.querySelector<HTMLButtonElement>(".oa-react-sidebar-expand")
    await interact(() => window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Enter", metaKey: true, bubbles: true })))
    expect(trigger?.getAttribute("aria-expanded")).toBe("true")
    expect(received.at(-1)).toEqual({ name: "DesktopSidebarCollapsedChanged", payload: false })
    await interact(() => window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Enter", metaKey: true, bubbles: true })))
    expect(trigger?.getAttribute("aria-expanded")).toBe("false")
    expect(received.at(-1)).toEqual({ name: "DesktopSidebarCollapsedChanged", payload: true })
  })

  test("uses closed-catalog icon controls and left-aligned sidebar actions", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={() => Effect.void} />)
    expect(container.querySelector('[data-icon-name="Menu"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChatCompose"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="Chats"]')).toBeNull()
    expect(container.querySelector('[data-icon-name="ChevronLeft"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChevronRight"]')).not.toBeNull()
    const newSession = [...container.querySelectorAll<HTMLButtonElement>("button")]
      .find(button => button.textContent === "New session")
    expect(newSession?.classList.contains("justify-start")).toBe(true)
    expect(newSession?.classList.contains("text-left")).toBe(true)
    expect(container.querySelector('[aria-current="page"]')?.textContent).toContain("Local session")
    expect(container.querySelectorAll('[aria-current="page"]')).toHaveLength(1)
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
      "shell-settings-toggle",
    ])
    expect(destinations().map(row => row.querySelector("[data-icon-name]")?.getAttribute("data-icon-name"))).toEqual([
      "ChatCompose", "Settings",
    ])
    expect(container.querySelector(".oa-react-sidebar-footer [data-sidebar-destination-id=\"shell-settings-toggle\"]")).not.toBeNull()
    expect(container.textContent).not.toContain("Workspaces")
    expect(container.querySelector('[data-react-workspace="home"]')).toBeNull()
    await interact(() => destinations()[1]?.click())
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
    const expand = container.querySelector<HTMLButtonElement>(".oa-react-sidebar-expand")
    expect(expand).not.toBeNull()
    expect(expand?.querySelector('[data-icon-name="Menu"]')).not.toBeNull()
    expect(container.querySelector('input[type="search"]')).toBeNull()
  })

  test("keeps the Settings Khala frame singular while status articles remain semantic and unframed", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const base = fixtureState()
    const state: DesktopShellState = {
      ...base,
      workspace: "settings",
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
          lastOutcome: "Update completed and version re-probed.",
          codexReleaseNotes: null,
        },
      },
    }
    await render(root, <StrictMode><WorkbenchShell state={state} report={() => Effect.void} /></StrictMode>)

    expect(container.querySelectorAll('[data-khala-decoration="settings-frame"]')).toHaveLength(1)
    expect(container.querySelectorAll('[data-khala-decoration="settings-header"]')).toHaveLength(1)
    expect(container.querySelectorAll("#en-khala-desktop-settings-frame")).toHaveLength(1)
    expect(container.querySelectorAll("#en-khala-desktop-settings-header")).toHaveLength(1)
    expect(container.querySelectorAll('[data-react-workspace="settings"] [data-en-khala-decoration]')).toHaveLength(2)
    expect(container.querySelectorAll(".oa-react-settings-status-article")).toHaveLength(1)
    expect(container.querySelector('[data-harness="codex"]')?.getAttribute("data-status")).toBe("behind_latest")
    expect(container.querySelector('[data-harness="codex"] .oa-react-settings-status-label')?.textContent).toBe("Update available")
    expect(container.querySelector('[data-harness="codex"] [data-en-khala-decoration]')).toBeNull()
    expect(container.querySelector('[role="status"]')?.textContent).toContain("Update completed")
    for (const decoration of container.querySelectorAll<HTMLElement>('[data-react-workspace="settings"] .oa-react-khala-decoration')) {
      expect(decoration.getAttribute("aria-hidden")).toBe("true")
      expect(decoration.querySelector("button, a, input, [tabindex]")).toBeNull()
    }

    await render(root, <StrictMode><WorkbenchShell state={state} report={() => Effect.void} /></StrictMode>)
    expect(container.querySelectorAll('[data-react-workspace="settings"] [data-en-khala-decoration]')).toHaveLength(2)
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
        forwardTitle: "Settings",
      },
    }
    await render(root, <WorkbenchShell state={enabled} report={report} />)
    const back = container.querySelector<HTMLButtonElement>('[aria-label="Back to Earlier session"]')
    const forward = container.querySelector<HTMLButtonElement>('[aria-label="Forward to Settings"]')
    expect(back?.disabled).toBe(false)
    expect(forward?.disabled).toBe(false)
    expect(back?.title).toBe("Back to Earlier session")
    expect(forward?.title).toBe("Forward to Settings")
    await interact(() => back?.click())
    await interact(() => forward?.click())
    expect(received).toEqual([
      { name: "DesktopNavigationBackRequested", payload: null },
      { name: "DesktopNavigationForwardRequested", payload: null },
    ])
  })
})
