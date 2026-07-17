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
  const DOMRectStub = window.DOMRect as typeof DOMRect
  Object.defineProperty(DOMRectStub, "fromRect", {
    configurable: true,
    value: (rect: Partial<DOMRect> = {}): DOMRect =>
      new window.DOMRect(rect.x ?? 0, rect.y ?? 0, rect.width ?? 0, rect.height ?? 0) as unknown as DOMRect,
  })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Text: window.Text,
    Document: window.Document,
    Range: window.Range,
    DOMRect: DOMRectStub,
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

const historyRoot = (threadRef: string, title: string, updatedAt: string, createdAt = updatedAt) => ({
  threadRef,
  parentThreadRef: null,
  title,
  status: "completed" as const,
  createdAt,
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

  test("centers the empty conversation, follows the selected agent, and dispatches its compact directory action only while empty", async () => {
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
    expect(change?.textContent).toBe("")
    expect(change?.querySelector('[data-icon-name="FolderPen"]')).not.toBeNull()
    await interact(() => change?.click())
    expect(received).toContainEqual({ name: "DesktopWorkspacePickerRequested", payload: null })

    await render(root, <WorkbenchShell state={{ ...state, selectedHarness: "fable" }} report={report} />)
    expect(container.querySelector(".oa-react-timeline-empty h2")?.textContent).toBe("Start a conversation with Claude")
    expect(container.querySelector('[data-en-key="shell-provider-select"]')?.textContent).toBe("Claude")

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
    expect(rows[0]).toMatchObject({ title: "Local session", selected: true, meta: "1m", working: false })
    expect(rows[1]).toMatchObject({ title: "Earlier session", meta: "1h", working: false })
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

  test("keeps chats in created-date order when an older chat receives new messages", () => {
    const base = fixtureState()
    const olderCreated = {
      id: "local-older-created",
      title: "Older created chat",
      createdAt: "2026-07-14T09:00:00.000Z",
      updatedAt: "2026-07-14T12:30:00.000Z",
      notes: [],
    }
    const state = {
      ...base,
      threads: [olderCreated],
      activeThreadId: olderCreated.id,
      history: {
        ...base.history,
        catalog: {
          roots: [historyRoot(
            "history-newer-created",
            "Newer created chat",
            "2026-07-14T11:30:00.000Z",
            "2026-07-14T10:00:00.000Z",
          )],
          agents: [],
        },
      },
    }

    expect(projectReactSessionRows(state).map(row => row.id)).toEqual([
      "history-newer-created",
      "local-older-created",
    ])
    expect(projectReactSessionRows({
      ...state,
      threads: [{ ...olderCreated, updatedAt: "2026-07-14T13:30:00.000Z" }],
    }).map(row => row.id)).toEqual([
      "history-newer-created",
      "local-older-created",
    ])
  })

  test("replaces a working chat timestamp with the shared loading icon", async () => {
    const base = fixtureState()
    const state = { ...base, pendingByThread: { "local-1": true } }
    const rows = projectReactSessionRows(state, new Date("2026-07-14T12:01:00.000Z"))
    expect(rows.find(row => row.id === "local-1")?.working).toBe(true)
    expect(rows.find(row => row.id === "history-1")?.working).toBe(false)

    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={() => Effect.void} />)
    const workingRow = container.querySelector('[data-en-key="sidebar-thread-local-1"]')
    expect(workingRow?.querySelector(".oa-react-session-meta")).toBeNull()
    expect(workingRow?.querySelector('[data-en-role="loading"]')?.getAttribute("aria-label")).toBe("Local session is working")
    expect(workingRow?.querySelector('[data-icon-name="LoaderCircle"]')).not.toBeNull()
    expect(container.querySelector('[data-en-key="sidebar-thread-history-1"] .oa-react-session-meta')).not.toBeNull()
  })

  test("an active pending question shows waiting-for-answer instead of the generic timeline worker", async () => {
    const base = fixtureState()
    const state: DesktopShellState = {
      ...base,
      pending: true,
      notes: [{
        key: "pending-question", role: "system", text: "", timestamp: "now",
        question: {
          turnRef: "turn-question", questionRef: "question-waiting", status: "pending",
          questions: [{ question: "Choose a path", header: "Path", multiSelect: false, options: [{ label: "A" }, { label: "B" }] }],
        },
      }],
    }
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={() => Effect.void} />)
    expect(container.querySelector('[aria-label="Waiting for your answer"]')).not.toBeNull()
    expect(container.querySelector('.oa-react-working')).toBeNull()
  })

  test("treats a background Full Auto turn as working sidebar activity", () => {
    const state = fixtureState()
    const rows = projectReactSessionRows({
      ...state,
      fullAutoLiveByThread: { "local-1": { state: "turn_running", turnRef: "turn-1" } },
    }, new Date("2026-07-14T12:01:00.000Z"))
    expect(rows.find(row => row.id === "local-1")?.working).toBe(true)
  })

  test("projects only rate limits into the sidebar footer", () => {
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
      rateLimits: [{ label: "primary", usedPercent: 12 }],
    })

    // Token counts and context usage do not earn a footer on their own.
    const tokensOnly = projectSidebarMeter({ ...state, meter: { totalTokens: 42 } })
    expect(tokensOnly).toBeUndefined()
  })

  test("mounts rate limits below Settings with an explicit divider", async () => {
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
    expect(sidebarMeter.textContent).not.toContain("120 TOKENS")
    expect(sidebarMeter.textContent).not.toContain("INPUT")
    expect(sidebarMeter.textContent).not.toContain("OUTPUT")
    expect(sidebarMeter.textContent).toContain("PRIMARY")
    expect(settings.compareDocumentPosition(sidebarMeter) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0)
    const divider = container.querySelector(".oa-react-sidebar-divider")
    expect(divider).not.toBeNull()
    if (divider === null) throw new Error("sidebar divider must render")
    expect(settings.compareDocumentPosition(divider) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0)
    expect(divider.compareDocumentPosition(sidebarMeter) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0)
  })

  test("settings replaces recent chats with section navigation and a Back footer", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={{
      ...fixtureState(),
      workspace: "settings",
    }} report={() => Effect.void} />)

    expect(container.querySelector('[aria-label="Recent sessions"]')).toBeNull()
    expect(container.querySelector('[aria-label="Search sessions"]')).toBeNull()
    expect(container.querySelector('[data-sidebar-destination-id="settings-general"]')?.textContent).toContain("General")
    expect(container.querySelector('[data-sidebar-destination-id="settings-codex"]')?.textContent).toContain("Codex CLI")
    expect(container.querySelector('[data-sidebar-destination-id="settings-account"]')?.textContent).toContain("Account")
    expect(container.querySelector('[data-sidebar-destination-id="shell-settings-toggle"]')?.textContent).toContain("Back")
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
    await interact(() => {
      container.querySelector<HTMLButtonElement>('[data-en-key="sidebar-thread-history-1"]')?.click()
    })
    expect(received).toEqual(expect.arrayContaining([
      { name: "DesktopSessionSearchDisclosureChanged", payload: true },
      { name: "DesktopNewChat", payload: null },
      { name: "HistorySearchChanged", payload: "earlier" },
      { name: "DesktopChatSelected", payload: "local-1" },
      { name: "HistoryConversationSelected", payload: "history-1" },
    ]))
  })

  test("opens an accessible chat context menu and validates the focused rename prompt", async () => {
    const { window, container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={fixtureState()} report={report} />)
    const localRow = container.querySelector<HTMLButtonElement>('[data-en-key="sidebar-thread-local-1"]')
    if (localRow === null) throw new Error("local session row did not render")
    const localTrigger = localRow
    expect(localTrigger.getAttribute("data-slot")).toBe("context-menu-trigger")

    await interact(() => localTrigger.dispatchEvent(new window.KeyboardEvent("keydown", {
      bubbles: true,
      key: "F10",
      shiftKey: true,
    }) as unknown as Event))
    await interact(() => undefined)
    let renameItem = window.document.querySelector('[data-slot="context-menu-item"]') as unknown as HTMLElement | null
    expect(renameItem?.textContent).toBe("Rename")
    const menu = window.document.querySelector('[data-slot="context-menu-content"]') as unknown as HTMLElement | null
    await interact(() => menu?.dispatchEvent(new window.KeyboardEvent("keydown", {
      bubbles: true,
      key: "Escape",
    }) as unknown as Event))
    expect(window.document.querySelector('[data-slot="context-menu-item"]')).toBeNull()
    await interact(() => localTrigger.dispatchEvent(new window.MouseEvent("contextmenu", {
      bubbles: true,
      button: 2,
      clientX: 40,
      clientY: 40,
    }) as unknown as Event))
    renameItem = window.document.querySelector('[data-slot="context-menu-item"]') as unknown as HTMLElement | null
    expect(renameItem?.textContent).toBe("Rename")
    await interact(() => renameItem?.click())
    await interact(() => undefined)
    expect(received.some(intent => intent.name === "DesktopChatRenameDismissed")).toBe(true)

    const input = window.document.querySelector("#desktop-chat-rename-title") as unknown as HTMLInputElement | null
    expect(window.document.activeElement).toBe(input)
    expect(input?.value).toBe("Local session")
    expect(input?.selectionStart).toBe(0)
    expect(input?.selectionEnd).toBe("Local session".length)

    const valueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set
    valueSetter?.call(input, "   ")
    await interact(() => input?.dispatchEvent(new window.Event("input", { bubbles: true }) as unknown as Event))
    await interact(() => undefined)
    await interact(() => input?.form?.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }) as unknown as Event))
    expect(received.some(intent => intent.name === "DesktopChatRenameRequested")).toBe(false)
    expect(window.document.querySelector("#desktop-chat-rename-error")?.textContent).toBe("Enter a title before saving.")

    valueSetter?.call(input, "  Renamed locally  ")
    await interact(() => input?.dispatchEvent(new window.Event("input", { bubbles: true }) as unknown as Event))
    await interact(() => undefined)
    await interact(() => input?.form?.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }) as unknown as Event))
    expect(received.at(-1)).toEqual({
      name: "DesktopChatRenameRequested",
      payload: { threadRef: "local-1", title: "Renamed locally" },
    })
  })

  test("groups device-local worktrees by project with status, filters, manual order, and typed actions", async () => {
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
        selectedSessionRef: "session-1",
        sessions: [
          session,
          { ...session, sessionRef: "session-2", worktreeRef: "worktree-2", worktreeLabel: "feature/recovery", repositoryLabel: "needs-recovery", state: "recovery_required", recoveryReason: "missing_worktree" },
        ],
        totalSessions: 101,
        nextOffset: 100,
      },
    }
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={report} />)
    expect(received).toEqual([])
    expect(container.querySelector(".oa-react-projects")?.textContent).toContain("Projects")
    expect(container.querySelector(".oa-react-project-group")?.textContent).toContain("OpenAgents")
    expect(container.querySelector(".oa-react-worktree-list")?.textContent).toContain("mainopenagents · idle")
    expect(container.querySelector(".oa-react-worktree-list")?.textContent).not.toContain("feature/recovery")
    const choose = container.querySelector('[aria-label="Choose project or worktree"]') as HTMLButtonElement
    await interact(() => choose.click())
    expect(received).toContainEqual({ name: "DesktopCodingCatalogChooseRequested", payload: null })

    const recoverFilter = [...container.querySelectorAll(".oa-react-project-controls button")].find(button => button.textContent === "Recover") as HTMLButtonElement
    await interact(() => recoverFilter.click())
    expect(received).toContainEqual({ name: "DesktopCodingCatalogFilterSelected", payload: "recovery" })
    await render(root, <WorkbenchShell state={{ ...state, codingSessionFilter: "recovery" }} report={report} />)
    expect(container.querySelector(".oa-react-worktree-list")?.textContent).toContain("feature/recoveryneeds-recovery · needs recovery")
    const checkbox = container.querySelector('[aria-label="Select needs-recovery feature/recovery"]') as HTMLInputElement
    await interact(() => checkbox.click())
    const recoverSelected = [...container.querySelectorAll(".oa-react-project-selection button")].find(button => button.textContent === "Recover") as HTMLButtonElement
    await interact(() => recoverSelected.click())
    expect(received).toContainEqual({ name: "DesktopCodingSessionRecovered", payload: "session-2" })

    await render(root, <WorkbenchShell state={state} report={report} />)
    const sort = container.querySelector('[aria-label="Sort projects"]') as HTMLSelectElement
    await interact(() => {
      const setter = Object.getOwnPropertyDescriptor(window.HTMLSelectElement.prototype, "value")?.set
      setter?.call(sort, "manual")
      sort.dispatchEvent(new window.Event("change", { bubbles: true }) as unknown as Event)
    })
    expect(container.querySelector('[aria-label="Move main down"]')).not.toBeNull()
    const open = container.querySelector(".oa-react-worktree-open") as HTMLButtonElement
    await interact(() => open.click())
    expect(received).toContainEqual({ name: "DesktopCodingSessionOpened", payload: "session-1" })
    const load = [...container.querySelectorAll("button")].find(button => button.textContent === "Load more worktrees") as HTMLButtonElement
    await interact(() => load.click())
    expect(received).toContainEqual({ name: "DesktopCodingCatalogMoreRequested", payload: null })
  })

  test("shows exact worktree context and capability-backed project actions in the header", async () => {
    const { container } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    const session = {
      sessionRef: "session-1", workContextRef: "context-1", grantRef: "grant-1",
      projectRef: "project-1", repositoryRef: "repository-1", worktreeRef: "worktree-1",
      projectLabel: "OpenAgents", repositoryLabel: "openagents", worktreeLabel: "feature/t3-ui",
      state: "active" as const, lastActiveAt: "2026-07-14T12:00:00.000Z", recoveryReason: null,
    }
    const state: DesktopShellState = {
      ...base,
      codingCatalog: { ...base.codingCatalog, selectedSessionRef: "session-1", sessions: [session], totalSessions: 1, activeCount: 1 },
      git: { ...base.git, status: { ok: true, op: "status", branch: "feature/t3-ui", upstream: "origin/feature/t3-ui", detached: false, ahead: 0, behind: 0, staged: [], unstaged: [], untracked: [], truncated: false, repositoryRef: "repository-1", statusRef: "status-1", headRef: "head-1" } },
    }
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={report} />)
    expect(container.querySelector(".oa-react-conversation-meta")?.textContent).toContain("openagents / feature/t3-ui")
    expect(container.querySelector(".oa-react-conversation-actions")?.textContent).toContain("feature/t3-uiFilesReviewChange")
    await interact(() => {
      ;([...container.querySelectorAll(".oa-react-conversation-actions button")].find(button => button.textContent === "Files") as HTMLButtonElement | undefined)?.click()
      ;([...container.querySelectorAll(".oa-react-conversation-actions button")].find(button => button.textContent === "Review") as HTMLButtonElement | undefined)?.click()
      ;([...container.querySelectorAll(".oa-react-conversation-actions button")].find(button => button.textContent === "Change") as HTMLButtonElement | undefined)?.click()
    })
    expect(received).toEqual(expect.arrayContaining([
      { name: "DesktopWorkspaceSelected", payload: "files" },
      { name: "DesktopWorkspaceSelected", payload: "review" },
      { name: "DesktopCodingCatalogChooseRequested", payload: null },
    ]))
  })

  test("keeps the transcript mounted while capability surfaces activate, maximize, close, and persist", async () => {
    const { container, window } = installDom()
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = fixtureState()
    const session = {
      sessionRef: "session-layout", workContextRef: "context-1", grantRef: "grant-1",
      projectRef: "project-1", repositoryRef: "repository-1", worktreeRef: "worktree-1",
      projectLabel: "OpenAgents", repositoryLabel: "openagents", worktreeLabel: "main",
      state: "active" as const, lastActiveAt: "2026-07-14T12:00:00.000Z", recoveryReason: null,
    }
    const state: DesktopShellState = {
      ...base,
      workspace: "files",
      codingCatalog: { ...base.codingCatalog, selectedSessionRef: session.sessionRef, sessions: [session] },
      workspaceBrowser: { ...base.workspaceBrowser, phase: "ready", grantRef: "grant-1" },
    }
    const root = createTestRoot(container)
    await render(root, <WorkbenchShell state={state} report={report} />)
    expect(container.querySelector('[data-react-workspace="chat"]')).not.toBeNull()
    expect(container.querySelector('[aria-label="Files surface"]')).not.toBeNull()
    expect(container.querySelector('[role="tab"][aria-selected="true"]')?.textContent).toContain("Files")

    const maximize = container.querySelector('[aria-label="Maximize panel"]') as HTMLButtonElement
    await interact(() => maximize.click())
    expect(container.querySelector(".oa-react-surface-layout")?.getAttribute("data-maximized")).toBe("true")
    expect(window.localStorage.getItem("openagents.desktop.surface-layout.v1:session-layout")).toContain('"maximized":true')

    const add = container.querySelector('[aria-label="Add surface"]') as HTMLButtonElement
    await interact(() => add.click())
    const review = [...container.querySelectorAll('[role="menuitem"]')].find(button => button.textContent === "Review") as HTMLButtonElement
    await interact(() => review.click())
    expect(received).toContainEqual({ name: "DesktopWorkspaceSelected", payload: "review" })
    expect(container.querySelectorAll('[role="tab"]')).toHaveLength(2)

    const closePanel = container.querySelector('[aria-label="Close panel"]') as HTMLButtonElement
    await interact(() => closePanel.click())
    expect(received.at(-1)).toEqual({ name: "DesktopWorkspaceSelected", payload: "chat" })
    expect(container.querySelector(".oa-react-surface-panel")).toBeNull()
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
