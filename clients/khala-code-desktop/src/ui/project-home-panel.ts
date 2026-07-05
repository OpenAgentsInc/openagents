import type { KhalaCodeDesktopCodexThreadSummary } from "../shared/codex-threads"
import { formatCompactThreadTimestamp } from "./thread-time"

/**
 * OpenCode-style project/session home dashboard
 * (docs/khala-code/2026-07-05-opencode-desktop-parity-gap-audit.md, #8443).
 *
 * Reuses `KhalaCodeDesktopCodexThreadSummary` -- the exact projection the
 * Codex thread sidebar already renders -- so project grouping, status
 * badges, and resumability all stay in sync with the same source of truth
 * instead of a second parallel model.
 */
export type KhalaCodeProjectHomeSession = KhalaCodeDesktopCodexThreadSummary

export type KhalaCodeProjectHomeProjectSummary = Readonly<{
  key: string
  label: string
  sessionCount: number
  resumableCount: number
  latestRecencyAt: number | null
}>

const UNFILED_PROJECT_KEY = "unfiled"
const UNFILED_PROJECT_LABEL = "Unfiled sessions"

const projectKeyForSession = (session: KhalaCodeProjectHomeSession): string => {
  const label = session.projectLabel.trim()
  return label === "" ? UNFILED_PROJECT_KEY : label
}

const projectLabelForKey = (key: string): string =>
  key === UNFILED_PROJECT_KEY ? UNFILED_PROJECT_LABEL : key

const recencyRank = (session: KhalaCodeProjectHomeSession): number =>
  session.recencyAt ?? session.updatedAt ?? session.createdAt ?? -Infinity

/**
 * Groups the flat session list into project rows the same way OpenCode's
 * home groups sessions under projects. Pure and unit-testable on its own.
 */
export const khalaCodeProjectHomeProjectSummaries = (
  sessions: readonly KhalaCodeProjectHomeSession[],
): readonly KhalaCodeProjectHomeProjectSummary[] => {
  const byKey = new Map<string, {
    label: string
    sessionCount: number
    resumableCount: number
    latestRecencyAt: number | null
  }>()
  for (const session of sessions) {
    const key = projectKeyForSession(session)
    const existing = byKey.get(key) ?? {
      label: projectLabelForKey(key),
      latestRecencyAt: null,
      resumableCount: 0,
      sessionCount: 0,
    }
    const recency = recencyRank(session)
    byKey.set(key, {
      label: existing.label,
      latestRecencyAt: existing.latestRecencyAt === null
        ? (Number.isFinite(recency) ? recency : null)
        : Number.isFinite(recency)
          ? Math.max(existing.latestRecencyAt, recency)
          : existing.latestRecencyAt,
      resumableCount: existing.resumableCount + (session.resumable === false ? 0 : 1),
      sessionCount: existing.sessionCount + 1,
    })
  }
  return [...byKey.entries()]
    .map(([key, value]) => ({ key, ...value }))
    .sort((left, right) =>
      (right.latestRecencyAt ?? -Infinity) - (left.latestRecencyAt ?? -Infinity) ||
      left.label.localeCompare(right.label),
    )
}

/**
 * Applies the project-home search box and project filter chip to the flat
 * session list, sorted most-recent-first. Pure and unit-testable on its own.
 */
export const khalaCodeProjectHomeFilteredSessions = (
  sessions: readonly KhalaCodeProjectHomeSession[],
  input: Readonly<{ projectKey: string | null; searchTerm: string }>,
): readonly KhalaCodeProjectHomeSession[] => {
  const normalizedSearch = input.searchTerm.trim().toLowerCase()
  return sessions
    .filter(session => input.projectKey === null || projectKeyForSession(session) === input.projectKey)
    .filter(session => {
      if (normalizedSearch === "") return true
      const haystack = [session.title, session.preview, session.projectLabel, session.cwd ?? ""]
        .join("   ")
        .toLowerCase()
      return haystack.includes(normalizedSearch)
    })
    .sort((left, right) => recencyRank(right) - recencyRank(left))
}

export type KhalaCodeProjectHomePanelOptions = Readonly<{
  listSessions: () => Promise<readonly KhalaCodeProjectHomeSession[]>
  onNewSession: () => void
  onOpenSession: (session: KhalaCodeProjectHomeSession) => void
  onOpenSessionInBackground: (session: KhalaCodeProjectHomeSession) => void
}>

export type KhalaCodeProjectHomePanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

type ProjectHomeView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly sessions: readonly KhalaCodeProjectHomeSession[] }

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const countText = (count: number, singular: string, plural: string): string =>
  count === 1 ? `1 ${singular}` : `${count} ${plural}`

export const mountKhalaCodeProjectHomePanel = (
  container: HTMLElement,
  options: KhalaCodeProjectHomePanelOptions,
): KhalaCodeProjectHomePanelHandle => {
  let visible = false
  let view: ProjectHomeView = { phase: "loading" }
  let searchTerm = ""
  let selectedProjectKey: string | null = null
  let loading: Promise<void> | null = null

  const refresh = async (): Promise<void> => {
    if (loading !== null) return loading
    view = { phase: "loading" }
    render()
    loading = (async () => {
      try {
        const sessions = await options.listSessions()
        view = { phase: "ready", sessions }
      } catch (error) {
        view = { phase: "error", message: errorMessage(error) }
      }
      render()
    })().finally(() => {
      loading = null
    })
    return loading
  }

  const setSearchTerm = (next: string): void => {
    searchTerm = next
    render()
  }

  const selectProject = (key: string | null): void => {
    selectedProjectKey = selectedProjectKey === key ? null : key
    render()
  }

  const buildHeaderRow = (): HTMLElement => {
    const headerRow = el("div", "khala-project-home-header-row")

    const header = el("header", "khala-project-home-header")
    const titleGroup = el("div", "khala-project-home-title-group")
    titleGroup.append(
      el("div", "khala-project-home-eyebrow", "Project Home"),
      el("h2", "khala-project-home-title", "Projects & Sessions"),
      el(
        "p",
        "khala-project-home-subtitle",
        "Navigate every project and recent session without leaving the home dashboard.",
      ),
    )
    const actions = el("div", "khala-project-home-actions")
    const newSessionButton = el("button", "khala-project-home-action khala-project-home-action-primary", "New Session")
    newSessionButton.type = "button"
    newSessionButton.dataset.khalaProjectHomeAction = "new-session"
    const refreshButton = el("button", "khala-project-home-action", "Refresh")
    refreshButton.type = "button"
    refreshButton.dataset.khalaProjectHomeAction = "refresh"
    actions.append(newSessionButton, refreshButton)
    header.append(titleGroup, actions)

    const search = el("div", "khala-project-home-search")
    const searchInput = Object.assign(el("input", "khala-project-home-search-input") as HTMLInputElement, {
      placeholder: "Search projects and sessions",
      type: "search",
      value: searchTerm,
    })
    searchInput.dataset.khalaProjectHomeSearch = ""
    searchInput.setAttribute("aria-label", "Search projects and sessions")
    search.append(searchInput)

    headerRow.append(header, search)
    return headerRow
  }

  const projectRow = (
    project: KhalaCodeProjectHomeProjectSummary,
  ): HTMLButtonElement => {
    const button = el("button", "khala-project-home-project-row")
    button.type = "button"
    button.dataset.khalaProjectHomeAction = "select-project"
    button.dataset.projectKey = project.key
    const selected = selectedProjectKey === project.key
    button.dataset.selected = selected ? "true" : "false"
    button.setAttribute("aria-pressed", selected ? "true" : "false")
    const main = el("span", "khala-project-home-project-main")
    main.append(
      el("span", "khala-project-home-project-label", project.label),
      el(
        "span",
        "khala-project-home-project-meta",
        countText(project.sessionCount, "session", "sessions"),
      ),
    )
    button.append(main)
    if (project.resumableCount < project.sessionCount) {
      button.append(
        el(
          "span",
          "khala-project-home-badge khala-project-home-badge-warning",
          `${project.sessionCount - project.resumableCount} not resumable`,
        ),
      )
    }
    return button
  }

  const sessionRow = (session: KhalaCodeProjectHomeSession): HTMLElement => {
    const row = el("div", "khala-project-home-session-row")
    row.dataset.sessionId = session.id
    const main = el("div", "khala-project-home-session-main")
    main.append(
      el("span", "khala-project-home-session-title", session.title),
      el("span", "khala-project-home-session-preview", session.preview),
    )
    const meta = el("div", "khala-project-home-session-meta")
    meta.append(el("span", "khala-project-home-session-project", session.projectLabel))
    for (const badge of session.badges) {
      meta.append(el("span", "khala-project-home-badge", badge))
    }
    meta.append(
      el(
        "span",
        "khala-project-home-badge",
        session.resumable === false ? "Not resumable" : session.statusLabel,
      ),
    )
    const timestamp = formatCompactThreadTimestamp(session.recencyAt ?? session.updatedAt ?? session.createdAt)
    if (timestamp !== "") meta.append(el("span", "khala-project-home-session-time", timestamp))
    main.append(meta)
    row.append(main)

    if (session.resumable === false) {
      row.append(
        el(
          "span",
          "khala-project-home-session-unavailable",
          session.unavailableReason ?? "This session cannot be resumed.",
        ),
      )
      return row
    }

    const actions = el("div", "khala-project-home-session-actions")
    const openButton = el("button", "khala-project-home-action khala-project-home-action-primary", "Open")
    openButton.type = "button"
    openButton.dataset.khalaProjectHomeAction = "open-session"
    openButton.dataset.sessionId = session.id
    const backgroundButton = el("button", "khala-project-home-action", "Open in Background")
    backgroundButton.type = "button"
    backgroundButton.dataset.khalaProjectHomeAction = "open-session-background"
    backgroundButton.dataset.sessionId = session.id
    actions.append(openButton, backgroundButton)
    row.append(actions)
    return row
  }

  const renderBody = (): HTMLElement => {
    const body = el("div", "khala-project-home-body")
    if (view.phase === "loading") {
      body.append(el("div", "khala-project-home-empty", "Loading project home..."))
      return body
    }
    if (view.phase === "error") {
      const errorSection = el("section", "khala-project-home-message")
      errorSection.append(
        el("div", "khala-project-home-eyebrow", "Project home unavailable"),
        el("div", "khala-project-home-error", view.message),
      )
      const retryButton = el("button", "khala-project-home-action", "Retry")
      retryButton.type = "button"
      retryButton.dataset.khalaProjectHomeAction = "refresh"
      errorSection.append(retryButton)
      body.append(errorSection)
      return body
    }

    if (view.sessions.length === 0) {
      body.append(
        el(
          "div",
          "khala-project-home-empty",
          "No projects or sessions yet. Start a new session to begin.",
        ),
      )
      return body
    }

    const projects = khalaCodeProjectHomeProjectSummaries(view.sessions)
    const filteredSessions = khalaCodeProjectHomeFilteredSessions(view.sessions, {
      projectKey: selectedProjectKey,
      searchTerm,
    })

    const projectsSection = el("section", "khala-project-home-section")
    projectsSection.append(el("h3", "khala-project-home-section-title", "Projects"))
    const projectList = el("div", "khala-project-home-project-list")
    projectList.append(...projects.map(projectRow))
    projectsSection.append(projectList)
    body.append(projectsSection)

    const sessionsSection = el("section", "khala-project-home-section")
    const sessionsTitle = selectedProjectKey === null
      ? "Recent Sessions"
      : `Sessions in ${projectLabelForKey(selectedProjectKey)}`
    sessionsSection.append(el("h3", "khala-project-home-section-title", sessionsTitle))
    if (filteredSessions.length === 0) {
      sessionsSection.append(
        el(
          "div",
          "khala-project-home-empty",
          searchTerm.trim() === ""
            ? "No sessions in this project yet."
            : `No sessions match "${searchTerm.trim()}".`,
        ),
      )
    } else {
      const sessionList = el("div", "khala-project-home-session-list")
      sessionList.append(...filteredSessions.map(sessionRow))
      sessionsSection.append(sessionList)
    }
    body.append(sessionsSection)
    return body
  }

  // The header row (including the search input) is built exactly once so
  // typing a search query never loses input focus or caret position to a
  // DOM node swap; only the body section below it is replaced on refresh,
  // search, or project-filter changes.
  const headerRowEl = buildHeaderRow()
  const bodyRootEl = el("div", "khala-project-home-body-root")

  function render(): void {
    container.dataset.projectHomeShell = ""
    container.hidden = !visible
    bodyRootEl.replaceChildren(renderBody())
  }

  container.append(headerRowEl, bodyRootEl)

  container.addEventListener("click", event => {
    const target = event.target instanceof Element
      ? event.target.closest<HTMLElement>("[data-khala-project-home-action]")
      : null
    if (target === null) return
    event.preventDefault()
    const action = target.dataset.khalaProjectHomeAction
    if (action === "refresh") void refresh()
    if (action === "new-session") options.onNewSession()
    if (action === "select-project") selectProject(target.dataset.projectKey ?? null)
    if ((action === "open-session" || action === "open-session-background") && view.phase === "ready") {
      const sessionId = target.dataset.sessionId
      const session = view.sessions.find(candidate => candidate.id === sessionId)
      if (session === undefined) return
      if (action === "open-session") options.onOpenSession(session)
      else options.onOpenSessionInBackground(session)
    }
  })

  container.addEventListener("input", event => {
    const target = event.target
    if (!(target instanceof HTMLInputElement)) return
    if (target.dataset.khalaProjectHomeSearch === undefined) return
    setSearchTerm(target.value)
  })

  render()

  return {
    refresh,
    setVisible: nextVisible => {
      const becameVisible = nextVisible && !visible
      visible = nextVisible
      render()
      if (becameVisible && view.phase === "loading") void refresh()
    },
  }
}
