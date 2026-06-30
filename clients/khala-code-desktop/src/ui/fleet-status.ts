import type {
  KhalaCodeDesktopFleetAccount,
  KhalaCodeDesktopFleetStatus,
} from "../shared/rpc"

// Fleet status panel for Khala Code Desktop: current Codex fleet state — all
// linked accounts and their readiness, local Pylon health + capacity, active
// assignments, and running codex_exec processes. Backed by the codexFleetStatus
// RPC (which runs inspectCodexFleet on the bun side).

export type FleetPanelHandle = Readonly<{
  refresh: () => Promise<void>
}>

type FleetView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly data: KhalaCodeDesktopFleetStatus }

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

const accountReadinessState = (
  readiness: string,
): "ready" | "missing" | "degraded" => {
  const value = readiness.toLowerCase()
  if (value === "ready") return "ready"
  if (value.includes("credential") || value.includes("missing")) return "missing"
  return "degraded"
}

const titleize = (value: string): string =>
  value.replace(/[_-]+/g, " ").replace(/\b\w/g, char => char.toUpperCase())

const accountBadgeLabel = (readiness: string): string => {
  const state = accountReadinessState(readiness)
  if (state === "ready") return "Ready"
  if (state === "missing") return "Needs reconnect"
  return titleize(readiness)
}

const relativeTime = (iso: string | null): string => {
  if (iso === null) return "—"
  const then = Date.parse(iso)
  if (Number.isNaN(then)) return iso
  const seconds = Math.max(0, Math.round((Date.now() - then) / 1000))
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.round(hours / 24)}d ago`
}

const summaryLine = (parts: ReadonlyArray<string | null>): string =>
  parts.filter((part): part is string => Boolean(part)).join("  ·  ")

const badge = (state: string, label: string): HTMLElement => {
  const node = el("span", "khala-fleet-badge")
  node.dataset.state = state
  node.append(el("span", "khala-fleet-dot"), el("span", undefined, label))
  return node
}

const detailChip = (label: string, value: string): HTMLElement => {
  const chip = el("span", "khala-fleet-chip")
  chip.append(
    el("span", "khala-fleet-chip-label", label),
    el("span", "khala-fleet-chip-value", value),
  )
  return chip
}

const sectionHeader = (title: string, meta?: string): HTMLElement => {
  const header = el("div", "khala-fleet-section-header")
  header.append(el("h3", "khala-fleet-section-title", title))
  if (meta !== undefined) header.append(el("span", "khala-fleet-section-meta", meta))
  return header
}

const accountCard = (account: KhalaCodeDesktopFleetAccount): HTMLElement => {
  const state = accountReadinessState(account.readiness)
  const card = el("article", "khala-fleet-account")
  card.dataset.state = state

  const identity = el("div", "khala-fleet-account-identity")
  identity.append(el("strong", undefined, account.accountRef))
  identity.append(el("span", "khala-fleet-provider", account.provider))
  card.append(identity)

  card.append(badge(state, accountBadgeLabel(account.readiness)))

  if (account.quotaState !== null && account.quotaState.length > 0) {
    const hint = el(
      "p",
      "khala-fleet-account-hint",
      `Quota: ${titleize(account.quotaState)}`,
    )
    card.append(hint)
  } else if (state !== "ready") {
    card.append(
      el(
        "p",
        "khala-fleet-account-hint",
        "Run `khala fleet connect` to reconnect this account.",
      ),
    )
  }
  return card
}

const renderReady = (
  container: HTMLElement,
  data: KhalaCodeDesktopFleetStatus,
): void => {
  const readyAccounts = data.accounts.filter(
    account => accountReadinessState(account.readiness) === "ready",
  ).length
  const needsReconnect = data.accounts.length - readyAccounts
  const capacity =
    data.availableCodexAssignments === null || data.maxCodexAssignments === null
      ? null
      : `${data.availableCodexAssignments}/${data.maxCodexAssignments} Codex slots free`

  // Pylon
  const pylonSection = el("section", "khala-fleet-section")
  pylonSection.append(sectionHeader("Pylon", relativeTime(data.observedAt)))
  const pylonCard = el("article", "khala-fleet-pylon")
  pylonCard.dataset.state = data.pylon.status === "unavailable" ? "stale" : "online"
  const pylonId = el("div", "khala-fleet-pylon-identity")
  pylonId.append(
    el("strong", undefined, data.pylon.pylonRef ?? "local Pylon"),
    el("span", "khala-fleet-pylon-message", data.pylon.message),
  )
  pylonCard.append(pylonId)
  pylonCard.append(badge(
    data.pylon.status === "unavailable" ? "stale" : "online",
    titleize(data.pylon.status),
  ))
  if (capacity !== null) {
    pylonCard.append(el("span", "khala-fleet-capacity", capacity))
  }
  pylonSection.append(pylonCard)
  container.append(pylonSection)

  // Accounts
  const accountsSection = el("section", "khala-fleet-section")
  accountsSection.append(
    sectionHeader(
      "Codex accounts",
      summaryLine([
        `${readyAccounts} ready`,
        needsReconnect > 0 ? `${needsReconnect} need reconnect` : null,
      ]),
    ),
  )
  if (data.accounts.length === 0) {
    accountsSection.append(
      el(
        "p",
        "khala-fleet-empty",
        "No Codex accounts linked. Run `khala fleet connect` to add one.",
      ),
    )
  } else {
    const list = el("div", "khala-fleet-account-list")
    for (const account of data.accounts) list.append(accountCard(account))
    accountsSection.append(list)
  }
  container.append(accountsSection)

  // Active assignments
  const activeSection = el("section", "khala-fleet-section")
  activeSection.append(
    sectionHeader(
      "Active assignments",
      `${data.activeAssignments.length} active`,
    ),
  )
  if (data.activeAssignments.length === 0) {
    activeSection.append(
      el("p", "khala-fleet-empty", "No active Codex assignments right now."),
    )
  } else {
    const list = el("div", "khala-fleet-assignment-list")
    for (const marker of data.activeAssignments) {
      const row = el("article", "khala-fleet-assignment")
      row.dataset.state = "active"
      const chips = el("div", "khala-fleet-chips")
      if (marker.issueRef !== null) chips.append(detailChip("issue", marker.issueRef))
      if (marker.assignmentRef !== null) {
        chips.append(detailChip("assignment", marker.assignmentRef))
      }
      chips.append(detailChip("updated", relativeTime(marker.updatedAt)))
      row.append(chips)
      list.append(row)
    }
    activeSection.append(list)
  }
  container.append(activeSection)

  // Processes
  if (data.processes.length > 0) {
    const procSection = el("section", "khala-fleet-section")
    procSection.append(
      sectionHeader("Codex processes", `${data.processes.length} running`),
    )
    const list = el("div", "khala-fleet-chips")
    for (const process of data.processes) {
      list.append(detailChip(`pid ${process.pid}`, process.elapsed))
    }
    procSection.append(list)
    container.append(procSection)
  }
}

const render = (
  container: HTMLElement,
  view: FleetView,
  onRefresh: () => void,
): void => {
  container.replaceChildren()

  const header = el("header", "khala-fleet-header")
  header.append(el("h2", "khala-fleet-title", "Fleet status"))
  const refresh = el("button", "khala-fleet-refresh", "Refresh")
  refresh.type = "button"
  refresh.disabled = view.phase === "loading"
  refresh.addEventListener("click", onRefresh)
  header.append(refresh)
  container.append(header)

  const body = el("div", "khala-fleet-body")
  if (view.phase === "loading") {
    body.append(el("p", "khala-fleet-empty", "Inspecting Codex fleet…"))
  } else if (view.phase === "error") {
    const error = el("p", "khala-fleet-error", `Could not load fleet status: ${view.message}`)
    body.append(error)
  } else {
    renderReady(body, view.data)
  }
  container.append(body)
}

export const mountFleetPanel = (
  container: HTMLElement,
  options: Readonly<{ fetch: () => Promise<KhalaCodeDesktopFleetStatus> }>,
): FleetPanelHandle => {
  let inFlight = false

  const refresh = async (): Promise<void> => {
    if (inFlight) return
    inFlight = true
    render(container, { phase: "loading" }, () => void refresh())
    try {
      const data = await options.fetch()
      render(container, { phase: "ready", data }, () => void refresh())
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      render(container, { phase: "error", message }, () => void refresh())
    } finally {
      inFlight = false
    }
  }

  render(container, { phase: "loading" }, () => void refresh())
  return { refresh }
}
