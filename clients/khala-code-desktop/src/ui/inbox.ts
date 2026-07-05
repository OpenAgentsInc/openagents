import type { KhalaCodeDesktopCodexEcosystemSeverity } from "../shared/codex-ecosystem"
import type {
  KhalaCodeDesktopCodexEcosystemReadResult,
  KhalaCodeDesktopCodexHarnessStatus,
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc"

export type UnifiedInboxItemKind =
  | "approval_required"
  | "claim_expired"
  | "cooldown_all_accounts"
  | "credentials_missing"
  | "run_blocked"
  | "merge_conflict_wave"
  | "ready_for_review"
  | "mcp_failed"
  | "codex_ecosystem"
  | "memory_update_pending"

export type UnifiedInboxItemAction =
  | "edit"
  | "reply"
  | "open_file"
  | "resume"
  | "reconnect"
  | "open_fleet"
  | "open_settings"
  | "refresh"

export type UnifiedInboxItem = Readonly<{
  ref: string
  kind: UnifiedInboxItemKind
  title: string
  summary: string
  source: "fleet" | "runtime" | "assignment" | "permission" | "mcp" | "memory" | "codex_ecosystem"
  severity: "info" | "warning" | "critical"
  observedAt: string
  accountRef?: string
  assignmentRef?: string
  issueRef?: string
  resumeRunRef?: string
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
  codexHarness?: KhalaCodeDesktopCodexHarnessStatus
  ecosystem?: KhalaCodeDesktopCodexEcosystemReadResult
  fleet: KhalaCodeDesktopFleetStatus
  pylon: KhalaCodeDesktopRuntimeStatus
  coding: KhalaCodeDesktopRuntimeStatus
  tokenAccounting: KhalaCodeDesktopRuntimeStatus
}>

export type UnifiedInboxPanelHandle = Readonly<{
  destroy: () => void
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type UnifiedInboxPanelOptions = Readonly<{
  fetch: () => Promise<UnifiedInboxSource>
  onOpenFleet: () => void
  onOpenSettings: () => void
  onReconnectAccount: (accountRef: string) => void
  onResumeRun: (runRef: string) => Promise<void> | void
}>

type InboxView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly data: UnifiedInboxProjection }

type Handlers = Readonly<{
  onRefresh: () => void
  onOpenFleet: () => void
  onOpenSettings: () => void
  onReconnectAccount: (accountRef: string) => void
  onResumeRun: (runRef: string) => Promise<void> | void
}>

const itemPriority: Record<UnifiedInboxItemKind, number> = {
  approval_required: 0,
  credentials_missing: 1,
  cooldown_all_accounts: 2,
  merge_conflict_wave: 3,
  claim_expired: 4,
  run_blocked: 5,
  mcp_failed: 6,
  codex_ecosystem: 7,
  ready_for_review: 8,
  memory_update_pending: 9,
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

export const khalaCodeInboxReadinessNeedsHuman = (readiness: string): boolean => {
  const value = readiness.toLowerCase()
  return value.includes("credential") || value.includes("missing") || value.includes("error")
}

const assignmentResumeCommand = (assignmentRef: string): string =>
  `khala closeout ${assignmentRef} --json`

const refIncludes = (refs: readonly string[], pattern: RegExp): boolean =>
  refs.some(ref => pattern.test(ref))

export const khalaCodeInboxFlagKindForAssignment = (
  refs: readonly string[],
  approvalRequired: boolean,
  blocked: boolean,
): UnifiedInboxItemKind | null => {
  if (approvalRequired || refIncludes(refs, /approval[_-]?required|permission/iu)) return "approval_required"
  if (refIncludes(refs, /merge[_-]?(conflict|wave)|merge-wave/iu)) return "merge_conflict_wave"
  if (refIncludes(refs, /claim[_-]?expired|expired[_-]?claim|lease[_-]?expired|claim_not_live/iu)) return "claim_expired"
  if (blocked) return "run_blocked"
  return null
}

export const khalaCodeInboxAssignmentNeedsHuman = (
  refs: readonly string[],
  approvalRequired: boolean,
  blocked: boolean,
): boolean => khalaCodeInboxFlagKindForAssignment(refs, approvalRequired, blocked) !== null

const flagActions = (
  kind: UnifiedInboxItemKind,
  canResumeRun: boolean,
  canReconnect = false,
): readonly UnifiedInboxItemAction[] => {
  if (kind === "approval_required") return ["open_fleet", "refresh"]
  if (kind === "credentials_missing") return canReconnect ? ["reconnect", "open_fleet"] : ["open_fleet", "refresh"]
  if (kind === "claim_expired") return canResumeRun ? ["resume", "open_fleet", "refresh"] : ["open_fleet", "refresh"]
  if (kind === "run_blocked" || kind === "cooldown_all_accounts" || kind === "merge_conflict_wave") {
    return canResumeRun ? ["resume", "open_fleet", "refresh"] : ["open_fleet", "refresh"]
  }
  return ["open_fleet", "refresh"]
}

const allReadyAccountsCoolingDown = (
  fleet: KhalaCodeDesktopFleetStatus,
): boolean => {
  const cooldownAccounts = fleet.accounts.filter(account =>
    account.queuePolicy?.cooldown === "cooling_down" ||
    account.quotaState?.toLowerCase() === "cooling_down"
  )
  return cooldownAccounts.length > 0 &&
    fleet.availableCodexAssignments === 0 &&
    cooldownAccounts.length === fleet.accounts.length
}

const inboxSeverity = (
  severity: KhalaCodeDesktopCodexEcosystemSeverity,
): UnifiedInboxItem["severity"] =>
  severity === "critical" ? "critical" : severity === "warning" ? "warning" : "info"

export const projectUnifiedInbox = (
  source: UnifiedInboxSource,
): UnifiedInboxProjection => {
  const observedAt = source.fleet.observedAt
  const items: UnifiedInboxItem[] = []

  if (source.codexHarness !== undefined && !source.codexHarness.available) {
    items.push({
      ref: "inbox.runtime.codex_harness.unavailable",
      kind: source.codexHarness.auth.state === "credentials_missing"
        ? "credentials_missing"
        : "run_blocked",
      title: "Codex install or sign-in required",
      summary: source.codexHarness.reason,
      source: "runtime",
      severity: "critical",
      observedAt: source.codexHarness.observedAt,
      actions: ["refresh"],
    })
  }

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
    if (!khalaCodeInboxReadinessNeedsHuman(account.readiness)) continue
    items.push({
      ref: `inbox.credential.${account.accountRef}`,
      kind: "credentials_missing",
      title: `${account.accountRef} needs reconnect`,
      summary: account.email === null
        ? `Worker Codex account ${account.accountRef} is not signed in in its isolated Pylon home.`
        : `${account.email} is not ready: ${account.readiness}.`,
      source: "fleet",
      severity: "critical",
      observedAt,
      accountRef: account.accountRef,
      actions: flagActions("credentials_missing", false, true),
    })
  }

  if (allReadyAccountsCoolingDown(source.fleet)) {
    items.push({
      ref: "inbox.fleet.cooldown_all_accounts",
      kind: "cooldown_all_accounts",
      title: "All worker accounts are cooling down",
      summary: "Fleet has no free Codex slots because every ready worker account is in cooldown.",
      source: "fleet",
      severity: "warning",
      observedAt,
      actions: flagActions("cooldown_all_accounts", false),
    })
  }

  for (const assignment of source.fleet.activeAssignments) {
    const assignmentRef = assignment.assignmentRef
    const worker = assignment.workerSession
    const blockerRefs = assignment.blockerRefs ?? worker?.blockerRefs ?? []
    const blocked = blockerRefs.length > 0 ||
      worker?.approvalState === "blocked"
    const approvalRequired = worker?.approvalState === "approval_required"
    const flagKind = khalaCodeInboxFlagKindForAssignment(blockerRefs, approvalRequired, blocked)
    const kind: UnifiedInboxItemKind = flagKind ?? "ready_for_review"
    const runRef = assignment.runRef ?? undefined
    const canResumeRun = runRef !== undefined
    items.push({
      ref: `inbox.assignment.${stableRefText(assignmentRef)}.${stableRefText(assignment.issueRef)}`,
      kind,
      title: assignment.issueRef === null
        ? approvalRequired
          ? "Worker approval required"
          : kind === "merge_conflict_wave"
            ? "Worker merge conflict wave"
            : kind === "claim_expired"
              ? "Worker claim expired"
              : blocked
                ? "Worker run blocked"
            : "Assignment needs review"
        : approvalRequired
          ? `${assignment.issueRef} needs approval`
          : kind === "merge_conflict_wave"
            ? `${assignment.issueRef} has merge conflicts`
            : kind === "claim_expired"
              ? `${assignment.issueRef} claim expired`
              : blocked
                ? `${assignment.issueRef} is blocked`
            : `${assignment.issueRef} needs review`,
      summary: assignmentRef === null
        ? "An active worker assignment was reported without a public assignment ref."
        : approvalRequired
          ? "A worker Codex session is waiting on an approval routed through the fleet projection."
          : kind === "merge_conflict_wave"
            ? "One or more worker sessions reported merge conflicts. Inspect Fleet, resolve the public worktree state, then resume the run."
            : kind === "claim_expired"
              ? "The worker claim expired before closeout. Re-run the work unit or resume the supervised run after checking Fleet."
              : blocked
                ? "A worker Codex session reported blockers; inspect Fleet before resuming."
            : "Review the worker transcript, closeout, and token proof projection before accepting the next step.",
      source: "assignment",
      severity: flagKind === "claim_expired" || flagKind === "merge_conflict_wave" || approvalRequired || blocked ? "critical" : "info",
      observedAt: assignment.updatedAt ?? observedAt,
      ...(assignmentRef === null ? {} : {
        assignmentRef,
        resumeCommand: assignmentResumeCommand(assignmentRef),
      }),
      ...(runRef === undefined ? {} : { resumeRunRef: runRef }),
      ...(assignment.issueRef === null ? {} : { issueRef: assignment.issueRef }),
      actions: assignmentRef === null
        ? ["open_fleet", "refresh"]
        : flagKind === null
          ? ["resume", "open_fleet", "refresh"]
          : flagActions(flagKind, canResumeRun),
    })
  }

  for (const diagnostic of source.ecosystem?.diagnostics ?? []) {
    items.push({
      ref: `inbox.codex_ecosystem.${diagnostic.ref}`,
      kind: diagnostic.source === "mcp" ? "mcp_failed" : "codex_ecosystem",
      title: diagnostic.title,
      summary: diagnostic.detail,
      source: diagnostic.source === "mcp" ? "mcp" : "codex_ecosystem",
      severity: inboxSeverity(diagnostic.severity),
      observedAt: diagnostic.observedAt,
      actions: diagnostic.action === "refresh"
        ? ["refresh"]
        : ["open_settings", "refresh"],
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

  if (source.tokenAccounting.status === "error" || source.tokenAccounting.status === "unavailable") {
    items.push({
      ref: "inbox.runtime.token_accounting.blocked",
      kind: "run_blocked",
      title: "Token accounting needs review",
      summary: source.tokenAccounting.reason,
      source: "runtime",
      severity: "critical",
      observedAt: source.tokenAccounting.observedAt,
      actions: ["refresh"],
    })
  }

  const coverage = [
    {
      source: "Codex harness",
      status: source.codexHarness?.available ? "connected" as const : "not_connected" as const,
      summary: source.codexHarness?.available
        ? "The primary user Codex install and Codex home are ready for wrapper sessions."
        : source.codexHarness?.reason ?? "Codex harness readiness has not been connected to this projection yet.",
    },
    {
      source: "approval queue",
      status: "connected" as const,
      summary: "Worker approval, blocker, and review events are projected from Fleet assignment metadata into Inbox.",
    },
    {
      source: "MCP failures",
      status: source.ecosystem === undefined ? "not_connected" as const : "connected" as const,
      summary: source.ecosystem === undefined
        ? "MCP tool failure routing needs a desktop projection before it can create Inbox items."
        : `${source.ecosystem.sections.mcp.count} Codex MCP servers projected with ${source.ecosystem.sections.mcp.authRequiredCount} auth blockers.`,
    },
    {
      source: "memory and skill updates",
      status: source.ecosystem === undefined ? "not_connected" as const : "connected" as const,
      summary: source.ecosystem === undefined
        ? "Staged worker memory/skill diffs are not persisted as desktop queue rows yet."
        : `${source.ecosystem.notifications.length} recent Codex skill/app/MCP invalidation events are available.`,
    },
    {
      source: "fleet readiness",
      status: "connected" as const,
      summary: "Worker Codex account readiness, Pylon state, and assignment markers are projected locally.",
    },
    {
      source: "token accounting",
      status: source.tokenAccounting.available ? "connected" as const : "not_connected" as const,
      summary: source.tokenAccounting.reason,
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
  resetTimers: Set<number>,
): HTMLButtonElement => {
  const button = el("button", "khala-inbox-action", readable(action))
  button.type = "button"
  button.dataset.action = action
  button.addEventListener("click", () => {
    if (action === "open_fleet") handlers.onOpenFleet()
    if (action === "open_settings") handlers.onOpenSettings()
    if (action === "reconnect" && item.accountRef !== undefined) {
      handlers.onReconnectAccount(item.accountRef)
    }
    if (action === "refresh") handlers.onRefresh()
    if (action === "resume" && item.resumeRunRef !== undefined) {
      void Promise.resolve(handlers.onResumeRun(item.resumeRunRef))
      button.textContent = "Resuming"
      const timer = window.setTimeout(() => {
        resetTimers.delete(timer)
        button.textContent = readable(action)
      }, 1400)
      resetTimers.add(timer)
    } else if (action === "resume" && item.resumeCommand !== undefined) {
      void navigator.clipboard?.writeText(item.resumeCommand)
      button.textContent = "Copied"
      const timer = window.setTimeout(() => {
        resetTimers.delete(timer)
        button.textContent = readable(action)
      }, 1400)
      resetTimers.add(timer)
    }
  })
  if (
    (action === "reconnect" && item.accountRef === undefined) ||
    (action === "resume" && item.resumeCommand === undefined && item.resumeRunRef === undefined)
  ) {
    button.disabled = true
  }
  return button
}

const renderItem = (item: UnifiedInboxItem, handlers: Handlers, resetTimers: Set<number>): HTMLElement => {
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
    actions.append(renderAction(item, action, handlers, resetTimers))
  }

  row.append(marker, body, actions)
  return row
}

const renderReady = (
  container: HTMLElement,
  data: UnifiedInboxProjection,
  handlers: Handlers,
  resetTimers: Set<number>,
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
    "credentials_missing",
    "cooldown_all_accounts",
    "merge_conflict_wave",
    "claim_expired",
    "codex_ecosystem",
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
    for (const item of data.items) list.append(renderItem(item, handlers, resetTimers))
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
  resetTimers: Set<number>,
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
    renderReady(body, view.data, handlers, resetTimers)
  }
  container.append(body)
}

// KS-6.8 (#8418) hot-poll migration finding: this panel's `options.fetch()`
// aggregates SIX independently-sourced device-local RPCs
// (`codexHarnessStatus`, `codexEcosystemRead`, `codexFleetStatus`,
// `pylonStatus`, `codingStatus`, `tokenAccountingStatus` — see the call
// site in `ui/main.ts`), each reading local process/config/file state on
// this machine. None of them are khala-sync scope consumers, and none of
// KS-8.13's product-state entities (`packages/khala-sync/src/khala-code.ts`
// — threads/teams/chat) cover local runtime/harness/account health. This is
// the "device-local codex telemetry" class the cleanup audit's §6.3 already
// excludes from sync consolidation, so the 5s `setVisible`-gated poll below
// is NOT a khala-sync push candidate today (see the doc correction
// alongside this change). It is already bounded to on-screen visibility
// (no interval while the panel is hidden) and explicit actions
// (`onReconnectAccount`/`onResumeRun`) already force an immediate refresh
// rather than waiting for the next tick. Removing the interval outright
// would be a real regression: five of the six sources have no
// change-notification mechanism today, so nothing would tell this panel to
// refresh between user actions. A genuine fix needs a local event bus for
// those five sources (or new sync-scoped entities for them), which is a
// separate, larger follow-up — not a "mirror #8383" cutover.
export const mountUnifiedInboxPanel = (
  container: HTMLElement,
  options: UnifiedInboxPanelOptions,
): UnifiedInboxPanelHandle => {
  let inFlight = false
  let visible = false
  let pollTimer = 0
  let lastData: UnifiedInboxProjection | null = null
  const resetTimers = new Set<number>()

  const currentView = (): InboxView =>
    lastData === null ? { phase: "loading" } : { phase: "ready", data: lastData }

  const handlers: Handlers = {
    onRefresh: () => void refresh(),
    onOpenFleet: options.onOpenFleet,
    onOpenSettings: options.onOpenSettings,
    onReconnectAccount: options.onReconnectAccount,
    onResumeRun: options.onResumeRun,
  }

  const clearResetTimers = (): void => {
    for (const timer of resetTimers) window.clearTimeout(timer)
    resetTimers.clear()
  }

  const paint = (): void => render(container, currentView(), handlers, resetTimers)

  const refresh = async (): Promise<void> => {
    if (inFlight) return
    inFlight = true
    if (lastData === null) paint()
    try {
      lastData = projectUnifiedInbox(await options.fetch())
      paint()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      render(container, { phase: "error", message }, handlers, resetTimers)
    } finally {
      inFlight = false
    }
  }

  const setVisible = (next: boolean): void => {
    visible = next
    window.clearInterval(pollTimer)
    pollTimer = 0
    if (!next) return
    void refresh()
    pollTimer = window.setInterval(() => {
      if (visible && !inFlight) void refresh()
    }, 5000)
  }

  paint()
  return {
    destroy(): void {
      visible = false
      window.clearInterval(pollTimer)
      pollTimer = 0
      clearResetTimers()
      container.replaceChildren()
    },
    refresh,
    setVisible,
  }
}
