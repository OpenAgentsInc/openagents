import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  khalaCodeProjectHomeFilteredSessions,
  khalaCodeProjectHomeProjectSummaries,
  mountKhalaCodeProjectHomePanel,
  type KhalaCodeProjectHomeSession,
} from "../src/ui/project-home-panel"

const setGlobal = (key: string, value: unknown): void => {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
    writable: true,
  })
}

const installDom = (): HTMLElement => {
  const window = new Window()
  setGlobal("document", window.document)
  setGlobal("HTMLElement", window.HTMLElement)
  setGlobal("HTMLInputElement", window.HTMLInputElement)
  setGlobal("Element", window.Element)
  setGlobal("Event", window.Event)
  setGlobal("MouseEvent", window.MouseEvent)
  setGlobal("customElements", window.customElements)
  const container = window.document.createElement("section")
  window.document.body.append(container)
  return container as unknown as HTMLElement
}

const flush = async (): Promise<void> => {
  for (let index = 0; index < 20; index += 1) {
    await Promise.resolve()
  }
}

const session = (
  overrides: Partial<KhalaCodeProjectHomeSession> & Pick<KhalaCodeProjectHomeSession, "id" | "title">,
): KhalaCodeProjectHomeSession => ({
  badges: ["Codex"],
  createdAt: 1_700_000_000,
  cwd: "/workspace/openagents",
  forkedFromId: null,
  modelProvider: "codex",
  parentThreadId: null,
  preview: "Preview text",
  projectLabel: "openagents",
  recencyAt: 1_700_000_100,
  resumable: true,
  sessionId: overrides.id,
  source: "codex_app_server_thread_list",
  status: "idle",
  statusLabel: "Idle",
  unavailableReason: null,
  updatedAt: 1_700_000_050,
  ...overrides,
})

describe("khala code project home panel pure helpers", () => {
  test("groups sessions into project summaries sorted by recency, falling back to an unfiled bucket", () => {
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "a", title: "A", projectLabel: "openagents", recencyAt: 100 }),
      session({ id: "b", title: "B", projectLabel: "openagents", recencyAt: 200 }),
      session({ id: "c", title: "C", projectLabel: "", recencyAt: 300, resumable: false }),
      session({ id: "d", title: "D", projectLabel: "autopilot", recencyAt: 50 }),
    ]

    const projects = khalaCodeProjectHomeProjectSummaries(sessions)

    expect(projects).toEqual([
      { key: "unfiled", label: "Unfiled sessions", latestRecencyAt: 300, resumableCount: 0, sessionCount: 1 },
      { key: "openagents", label: "openagents", latestRecencyAt: 200, resumableCount: 2, sessionCount: 2 },
      { key: "autopilot", label: "autopilot", latestRecencyAt: 50, resumableCount: 1, sessionCount: 1 },
    ])
  })

  test("filters sessions by project key and case-insensitive search text, sorted most-recent-first", () => {
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "a", title: "Fix the composer bug", projectLabel: "openagents", recencyAt: 100 }),
      session({ id: "b", title: "Ship the release notes", projectLabel: "openagents", recencyAt: 300 }),
      session({ id: "c", title: "Unrelated autopilot work", projectLabel: "autopilot", recencyAt: 200 }),
    ]

    const byProject = khalaCodeProjectHomeFilteredSessions(sessions, { projectKey: "openagents", searchTerm: "" })
    expect(byProject.map(entry => entry.id)).toEqual(["b", "a"])

    const bySearch = khalaCodeProjectHomeFilteredSessions(sessions, { projectKey: null, searchTerm: "COMPOSER" })
    expect(bySearch.map(entry => entry.id)).toEqual(["a"])

    const byBoth = khalaCodeProjectHomeFilteredSessions(sessions, { projectKey: "autopilot", searchTerm: "composer" })
    expect(byBoth).toEqual([])
  })
})

describe("khala code project home panel DOM", () => {
  test("renders a loading state, then an empty state when no sessions exist", async () => {
    const container = installDom()
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => [],
      onNewSession: () => {},
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })

    panel.setVisible(true)
    expect(container.textContent).toContain("Loading project home")
    await flush()
    expect(container.textContent).toContain("No projects or sessions yet")
    expect(container.querySelector(".khala-project-home-search-input")).not.toBeNull()
  })

  test("renders a populated dashboard with project rows, recent sessions, and status badges", async () => {
    const container = installDom()
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "s1", title: "Fix the composer bug", projectLabel: "openagents", recencyAt: 300 }),
      session({
        id: "s2",
        title: "Legacy session",
        projectLabel: "openagents",
        recencyAt: 200,
        resumable: false,
        unavailableReason: "Stored local record only.",
      }),
    ]
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => sessions,
      onNewSession: () => {},
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })
    panel.setVisible(true)
    await flush()

    expect(container.textContent).toContain("openagents")
    expect(container.textContent).toContain("Fix the composer bug")
    expect(container.textContent).toContain("Legacy session")
    expect(container.textContent).toContain("Stored local record only.")
    expect(container.querySelectorAll(".khala-project-home-project-row")).toHaveLength(1)
    expect(container.querySelectorAll(".khala-project-home-session-row")).toHaveLength(2)
    // The non-resumable row shows its reason instead of Open actions.
    const rows = [...container.querySelectorAll(".khala-project-home-session-row")]
    const legacyRow = rows.find(row => row.textContent?.includes("Legacy session"))
    expect(legacyRow?.querySelector("[data-khala-project-home-action='open-session']")).toBeNull()
  })

  test("search box filters the recent sessions list without losing input focus", async () => {
    const container = installDom()
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "s1", title: "Fix the composer bug", recencyAt: 300 }),
      session({ id: "s2", title: "Ship release notes", recencyAt: 200 }),
    ]
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => sessions,
      onNewSession: () => {},
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })
    panel.setVisible(true)
    await flush()

    const searchInput = container.querySelector<HTMLInputElement>("[data-khala-project-home-search]")
    expect(searchInput).not.toBeNull()
    searchInput!.focus()
    searchInput!.value = "release"
    searchInput!.dispatchEvent(new Event("input", { bubbles: true }))

    expect(container.textContent).toContain("Ship release notes")
    expect(container.textContent).not.toContain("Fix the composer bug")
    expect(document.activeElement).toBe(searchInput)
  })

  test("opens a resumable session in the foreground and supports opening in the background", async () => {
    const container = installDom()
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "s1", title: "Fix the composer bug", recencyAt: 300 }),
    ]
    const openedForeground: string[] = []
    const openedBackground: string[] = []
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => sessions,
      onNewSession: () => {},
      onOpenSession: sessionEntry => openedForeground.push(sessionEntry.id),
      onOpenSessionInBackground: sessionEntry => openedBackground.push(sessionEntry.id),
    })
    panel.setVisible(true)
    await flush()

    container.querySelector<HTMLButtonElement>("[data-khala-project-home-action='open-session']")?.click()
    expect(openedForeground).toEqual(["s1"])

    container.querySelector<HTMLButtonElement>("[data-khala-project-home-action='open-session-background']")?.click()
    expect(openedBackground).toEqual(["s1"])
  })

  test("new session action and refresh both route through the provided options", async () => {
    const container = installDom()
    let calls = 0
    let newSessionCalls = 0
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => {
        calls += 1
        return []
      },
      onNewSession: () => {
        newSessionCalls += 1
      },
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })
    panel.setVisible(true)
    await flush()
    expect(calls).toBe(1)

    container.querySelector<HTMLButtonElement>("[data-khala-project-home-action='new-session']")?.click()
    expect(newSessionCalls).toBe(1)

    await panel.refresh()
    expect(calls).toBe(2)
  })

  test("shows an honest error state with a retry action when the session list fails to load", async () => {
    const container = installDom()
    let attempt = 0
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => {
        attempt += 1
        if (attempt === 1) throw new Error("session catalog unavailable")
        return [session({ id: "s1", title: "Recovered session" })]
      },
      onNewSession: () => {},
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })
    panel.setVisible(true)
    await flush()

    expect(container.textContent).toContain("Project home unavailable")
    expect(container.textContent).toContain("session catalog unavailable")

    container.querySelector<HTMLButtonElement>("[data-khala-project-home-action='refresh']")?.click()
    await flush()
    expect(container.textContent).toContain("Recovered session")
  })

  test("clicking a project row filters recent sessions to that project and can be toggled off", async () => {
    const container = installDom()
    const sessions: KhalaCodeProjectHomeSession[] = [
      session({ id: "s1", title: "Openagents work", projectLabel: "openagents", recencyAt: 300 }),
      session({ id: "s2", title: "Autopilot work", projectLabel: "autopilot", recencyAt: 200 }),
    ]
    const panel = mountKhalaCodeProjectHomePanel(container, {
      listSessions: async () => sessions,
      onNewSession: () => {},
      onOpenSession: () => {},
      onOpenSessionInBackground: () => {},
    })
    panel.setVisible(true)
    await flush()

    const projectRowSelector =
      "[data-khala-project-home-action='select-project'][data-project-key='openagents']"
    const projectRow = container.querySelector<HTMLButtonElement>(projectRowSelector)
    expect(projectRow).not.toBeNull()
    projectRow!.click()

    // Re-query after the click: the panel re-renders its body section, so
    // the earlier button reference is now a detached node.
    expect(container.textContent).toContain("Openagents work")
    expect(container.textContent).not.toContain("Autopilot work")
    expect(container.querySelector(projectRowSelector)?.getAttribute("aria-pressed")).toBe("true")

    container.querySelector<HTMLButtonElement>(projectRowSelector)?.click()
    expect(container.textContent).toContain("Autopilot work")
    expect(container.querySelector(projectRowSelector)?.getAttribute("aria-pressed")).toBe("false")
  })
})
