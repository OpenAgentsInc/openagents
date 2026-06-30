import type {
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc"

export type UnifiedInboxItemKind =
  | "approval_required"
  | "run_blocked"
  | "ready_for_review"
  | "mcp_failed"
  | "missing_credential"
  | "memory_update_pending"

export type UnifiedInboxItemAction =
  | "approve"
  | "reject"
  | "edit"
  | "reply"
  | "rerun"
  | "open_file"
  | "resume"
  | "reconnect"
  | "open_fleet"
  | "refresh"

export type UnifiedInboxItem = Readonly<{
  ref: string
  kind: UnifiedInboxItemKind
  title: string
  summary: string
  source: "fleet" | "runtime" | "assignment" | "permission" | "mcp" | "memory"
  severity: "info" | "warning" | "critical"
  observedAt: string
  accountRef?: string
  assignmentRef?: string
  issueRef?: string
  resumeCommand?: string
  actions: readonly UnifiedInboxItemAction[]
}>

export type UnifiedInboxProjection = Readonly<{
  ok: boolean
  observedAt: string
  items: readonly UnifiedInboxItem[]
  coverage: readonly {
    source: string
    status: "connected" | "not_connected"
    summary: string
  }[]
}>

export type UnifiedInboxSource = Readonly<{
  fleet: KhalaCodeDesktopFleetStatus
  pylon: KhalaCodeDesktopRuntimeStatus
  coding: KhalaCodeDesktopRuntimeStatus
  tokenAccounting: KhalaCodeDesktopRuntimeStatus
}>

export type UnifiedInboxPanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type UnifiedInboxPanelOptions = Readonly<{
  fetch: () => Promise<UnifiedInboxSource>
  onOpenFleet: () => void
  onReconnectAccount: (accountRef: string) => void
}>

type InboxView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly data: UnifiedInboxProjection }

type Handlers = Readonly<{
  onRefresh: () => void
  onOpenFleet: () => void
  onReconnectAccount: (accountRef: string) => void
}>

const itemPriority: Record<UnifiedInboxItemKind, number> = {
  approval_required: 0,
  run_blocked: 1,
  missing_credential: 2,
  mcp_failed: 3,
  ready_for_review: 4,
  memory_update_pending: 5,
}

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

const readable = (value: string): string =>
  value.replace(/[_-]+/g, " ").replace(/\b\w/g, char => char.toUpperCase())

const stableRefText = (value: string | null): string =>
  value === null || value.trim() === "" ? "unavailable" : value

const readinessNeedsHuman = (readiness: string): boolean => {
  const value = readiness.toLowerCase()
  return value.includes("credential") || value.includes("missing") || value.includes("error")
}

const assignmentResumeCommand = (assignmentRef: string): string =>
  `khala closeout ${assignmentRef} --json`

export const projectUnifiedInbox = (
  source: UnifiedInboxSource,
): UnifiedInboxProjection => {
  const observedAt = source.fleet.observedAt
  const items: UnifiedInboxItem[] = []

  if (!source.pylon.available || source.fleet.pylon.status === "unavailable") {
    items.push({
      ref: "inbox.runtime.pylon.unavailable",
      kind: "run_blocked",
      title: "Pylon unavailable",
      summary: source.pylon.reason || source.fleet.pylon.message,
      source: "runtime",
      severity: "critical",
      observedAt,
      actions: ["open_fleet", "refresh"],
    })
  }

  for (const account of source.fleet.accounts) {
    if (!readinessNeedsHuman(account.readiness)) continue
    items.push({
      ref: `inbox.credential.${account.accountRef}`,
      kind: "missing_credential",
      title: `${account.accountRef} needs reconnect`,
      summary: account.email === null
        ? `Codex account ${account.accountRef} is not signed in.`
        : `${account.email} is not ready: ${account.readiness}.`,
      source: "fleet",
      severity: "critical",
      observedAt,
      accountRef: account.accountRef,
      actions: ["reconnect", "open_fleet"],
    })
  }

  for (const assignment of source.fleet.activeAssignments) {
    const assignmentRef = assignment.assignmentRef
    items.push({
      ref: `inbox.assignment.${stableRefText(assignmentRef)}.${stableRefText(assignment.issueRef)}`,
      kind: "ready_for_review",
      title: assignment.issueRef === null
        ? "Assignment needs review"
        : `${assignment.issueRef} needs review`,
      summary: assignmentRef === null
        ? "An active assignment was reported without a public assignment ref."
        : "Review the closeout/proof projection before accepting the next step.",
      source: "assignment",
      severity: "info",
      observedAt: assignment.updatedAt ?? observedAt,
      ...(assignmentRef === null ? {} : {
        assignmentRef,
        resumeCommand: assignmentResumeCommand(assignmentRef),
      }),
      ...(assignment.issueRef === null ? {} : { issueRef: assignment.issueRef }),
      actions: assignmentRef === null
        ? ["open_fleet", "refresh"]
        : ["resume", "open_fleet", "refresh"],
    })
  }

  if (source.coding.status === "error" || source.coding.status === "unavailable") {
    items.push({
      ref: "inbox.runtime.coding.blocked",
      kind: "run_blocked",
      title: "Coding runtime blocked",
      summary: source.coding.reason,
      source: "runtime",
      severity: "critical",
      observedAt: source.coding.observedAt,
      actions: ["refresh"],
    })
  }

  const coverage = [
    {
      source: "approval queue",
      status: "not_connected" as const,
      summary: "Pylon permission prompts are not exposed through the desktop RPC yet.",
    },
    {
      source: "MCP failures",
      status: "not_connected" as const,
      summary: "MCP tool failure routing needs a desktop projection before it can create Inbox items.",
    },
    {
      source: "memory and skill updates",
      status: "not_connected" as const,
      summary: "Staged worker memory/skill diffs are not persisted as desktop queue rows yet.",
    },
    {
      source: "fleet readiness",
      status: "connected" as const,
      summary: "Codex account readiness, Pylon state, and assignment markers are projected locally.",
    },
  ]

  return {
    ok: source.fleet.ok && source.pylon.ok && source.coding.ok && source.tokenAccounting.ok,
    observedAt,
    items: items.sort((left, right) =>
      itemPriority[left.kind] - itemPriority[right.kind] ||
      left.title.localeCompare(right.title)
    ),
    coverage,
  }
}

const countByKind = (
  items: readonly UnifiedInboxItem[],
  kind: UnifiedInboxItemKind,
): number => items.filter(item => item.kind === kind).length

const itemKindLabel = (kind: UnifiedInboxItemKind): string => readable(kind)

const renderAction = (
  item: UnifiedInboxItem,
  action: UnifiedInboxItemAction,
  handlers: Handlers,
): HTMLButtonElement => {
  const button = el("button", "khala-inbox-action", readable(action))
  button.type = "button"
  button.dataset.action = action
  button.addEventListener("click", () => {
    if (action === "open_fleet") handlers.onOpenFleet()
    if (action === "reconnect" && item.accountRef !== undefined) {
      handlers.onReconnectAccount(item.accountRef)
    }
    if (action === "refresh") handlers.onRefresh()
    if (action === "resume" && item.resumeCommand !== undefined) {
      void navigator.clipboard?.writeText(item.resumeCommand)
      button.textContent = "Copied"
      window.setTimeout(() => {
        button.textContent = readable(action)
      }, 1400)
    }
  })
  if (
    (action === "reconnect" && item.accountRef === undefined) ||
    (action === "resume" && item.resumeCommand === undefined)
  ) {
    button.disabled = true
  }
  return button
}

const renderItem = (item: UnifiedInboxItem, handlers: Handlers): HTMLElement => {
  const row = el("article", "khala-inbox-item")
  row.dataset.kind = item.kind
  row.dataset.severity = item.severity

  const marker = el("span", "khala-inbox-marker", itemKindLabel(item.kind))
  const body = el("div", "khala-inbox-item-body")
  body.append(el("h3", "khala-inbox-item-title", item.title))
  body.append(el("p", "khala-inbox-item-summary", item.summary))

  const meta = el("div", "khala-inbox-meta")
  meta.append(el("span", undefined, item.source))
  if (item.assignmentRef !== undefined) meta.append(el("span", undefined, item.assignmentRef))
  if (item.accountRef !== undefined) meta.append(el("span", undefined, item.accountRef))
  body.append(meta)

  if (item.resumeCommand !== undefined) {
    const command = el("code", "khala-inbox-command", item.resumeCommand)
    body.append(command)
  }

  const actions = el("div", "khala-inbox-actions")
  for (const action of item.actions) {
    actions.append(renderAction(item, action, handlers))
  }

  row.append(marker, body, actions)
  return row
}

const renderReady = (
  container: HTMLElement,
  data: UnifiedInboxProjection,
  handlers: Handlers,
): void => {
  const summary = el("section", "khala-inbox-summary")
  summary.append(
    el("span", "khala-inbox-count", String(data.items.length)),
    el("span", undefined, data.items.length === 1 ? "item needs a human" : "items need a human"),
  )
  const chips = el("div", "khala-inbox-chips")
  for (const kind of [
    "approval_required",
    "run_blocked",
    "missing_credential",
    "ready_for_review",
  ] as const) {
    chips.append(el("span", "khala-inbox-chip", `${readable(kind)} ${countByKind(data.items, kind)}`))
  }
  summary.append(chips)
  container.append(summary)

  if (data.items.length === 0) {
    container.append(
      el("p", "khala-inbox-empty", "No projected Inbox items need action right now."),
    )
  } else {
    const list = el("div", "khala-inbox-list")
    for (const item of data.items) list.append(renderItem(item, handlers))
    container.append(list)
  }

  const coverage = el("section", "khala-inbox-coverage")
  coverage.append(el("h3", "khala-inbox-section-title", "Source coverage"))
  for (const source of data.coverage) {
    const row = el("div", "khala-inbox-coverage-row")
    row.dataset.status = source.status
    row.append(
      el("strong", undefined, source.source),
      el("span", undefined, source.summary),
    )
    coverage.append(row)
  }
  container.append(coverage)
}

const render = (
  container: HTMLElement,
  view: InboxView,
  handlers: Handlers,
): void => {
  container.replaceChildren()

  const header = el("header", "khala-inbox-header")
  const title = el("div")
  title.append(el("h2", "khala-inbox-title", "Unified Inbox"))
  title.append(
    el("p", "khala-inbox-subtitle", "Approvals, blockers, review-ready runs, and fleet fixes in one queue."),
  )
  header.append(title)
  const refresh = el("button", "khala-inbox-refresh", view.phase === "loading" ? "Loading..." : "Refresh")
  refresh.type = "button"
  refresh.disabled = view.phase === "loading"
  refresh.addEventListener("click", handlers.onRefresh)
  header.append(refresh)
  container.append(header)

  const body = el("div", "khala-inbox-body")
  if (view.phase === "loading") {
    body.append(el("p", "khala-inbox-empty", "Projecting queue items..."))
  } else if (view.phase === "error") {
    body.append(el("p", "khala-inbox-error", `Could not load Inbox: ${view.message}`))
  } else {
    renderReady(body, view.data, handlers)
  }
  container.append(body)
}

export const mountUnifiedInboxPanel = (
  container: HTMLElement,
  options: UnifiedInboxPanelOptions,
): UnifiedInboxPanelHandle => {
  let inFlight = false
  let visible = false
  let pollTimer = 0
  let lastData: UnifiedInboxProjection | null = null

  const currentView = (): InboxView =>
    lastData === null ? { phase: "loading" } : { phase: "ready", data: lastData }

  const handlers: Handlers = {
    onRefresh: () => void refresh(),
    onOpenFleet: options.onOpenFleet,
    onReconnectAccount: options.onReconnectAccount,
  }

  const paint = (): void => render(container, currentView(), handlers)

  const refresh = async (): Promise<void> => {
    if (inFlight) return
    inFlight = true
    if (lastData === null) paint()
    try {
      lastData = projectUnifiedInbox(await options.fetch())
      paint()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      render(container, { phase: "error", message }, handlers)
    } finally {
      inFlight = false
    }
  }

  const setVisible = (next: boolean): void => {
    visible = next
    window.clearInterval(pollTimer)
    if (!next) return
    void refresh()
    pollTimer = window.setInterval(() => {
      if (visible && !inFlight) void refresh()
    }, 5000)
  }

  paint()
  return { refresh, setVisible }
}
