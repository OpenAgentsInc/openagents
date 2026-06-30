import type {
  KhalaCodeDesktopConnectStart,
  KhalaCodeDesktopFleetAccount,
  KhalaCodeDesktopFleetStatus,
} from "../shared/rpc"
import { buildKhalaFleetBoardProjection } from "./fleet-board-projection"
import { renderKhalaFleetBoardHtml } from "./fleet-board-renderer"

// Fleet status panel for Khala Code Desktop: current Codex fleet state — all
// linked accounts (with signed-in email + readiness), local Pylon health +
// capacity, active assignments, and running codex_exec processes. Accounts can
// be removed, reconnected, or freshly connected (device-auth) from here.

export type FleetPanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type FleetPanelOptions = Readonly<{
  fetch: () => Promise<KhalaCodeDesktopFleetStatus>
  removeAccount: (
    accountRef: string,
  ) => Promise<{ readonly ok: boolean; readonly error?: string }>
  connectAccount: (accountRef: string) => Promise<KhalaCodeDesktopConnectStart>
  openExternal: (url: string) => Promise<boolean>
}>

type Handlers = Readonly<{
  onRefresh: () => void
  onRemove: (accountRef: string) => void
  onConnect: (accountRef: string) => void
  onOpenUrl: (url: string) => void
  onCancelConnect: () => void
}>

type ConnectView = Readonly<{
  accountRef: string
  start: KhalaCodeDesktopConnectStart | null
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

const appendFleetBoard = (
  container: HTMLElement,
  data: KhalaCodeDesktopFleetStatus,
): void => {
  const projection = buildKhalaFleetBoardProjection({ status: data })
  const template = document.createElement("template")
  template.innerHTML = renderKhalaFleetBoardHtml(projection, {
    reducedMotion:
      typeof matchMedia === "function" &&
      matchMedia("(prefers-reduced-motion: reduce)").matches,
  }).html
  container.append(template.content.cloneNode(true))
}

const accountCard = (
  account: KhalaCodeDesktopFleetAccount,
  handlers: Handlers,
): HTMLElement => {
  const state = accountReadinessState(account.readiness)
  const card = el("article", "khala-fleet-account")
  card.dataset.state = state
  card.dataset.accountRef = account.accountRef

  const identity = el("div", "khala-fleet-account-identity")
  const top = el("div", "khala-fleet-account-top")
  top.append(el("strong", undefined, account.accountRef))
  top.append(el("span", "khala-fleet-provider", account.provider))
  identity.append(top)
  identity.append(el("span", "khala-fleet-email", account.email ?? "not signed in"))
  card.append(identity)

  if (state === "ready") {
    card.append(badge("ready", "Ready"))
  } else {
    const reconnect = el("button", "khala-fleet-reconnect", "Reconnect")
    reconnect.type = "button"
    reconnect.dataset.state = state
    reconnect.title = `Reconnect ${account.accountRef}`
    reconnect.addEventListener("click", () => handlers.onConnect(account.accountRef))
    card.append(reconnect)
  }

  const remove = el("button", "khala-fleet-delete", "✕")
  remove.type = "button"
  remove.title = `Remove ${account.accountRef}`
  remove.setAttribute("aria-label", `Remove account ${account.accountRef}`)
  let armed = false
  let armTimer = 0
  remove.addEventListener("click", () => {
    if (!armed) {
      armed = true
      remove.textContent = "Remove?"
      remove.dataset.armed = "true"
      armTimer = window.setTimeout(() => {
        armed = false
        remove.textContent = "✕"
        delete remove.dataset.armed
      }, 3000)
      return
    }
    window.clearTimeout(armTimer)
    handlers.onRemove(account.accountRef)
  })
  card.append(remove)

  return card
}

const renderReady = (
  container: HTMLElement,
  data: KhalaCodeDesktopFleetStatus,
  handlers: Handlers,
): void => {
  const readyAccounts = data.accounts.filter(
    account => accountReadinessState(account.readiness) === "ready",
  ).length
  const needsReconnect = data.accounts.length - readyAccounts
  const capacity =
    data.availableCodexAssignments === null || data.maxCodexAssignments === null
      ? null
      : `${data.availableCodexAssignments}/${data.maxCodexAssignments} Codex slots free`

  appendFleetBoard(container, data)

  // Pylon
  const pylonSection = el("section", "khala-fleet-section")
  pylonSection.append(sectionHeader("Pylon"))
  const pylonCard = el("article", "khala-fleet-pylon")
  const pylonState = data.pylon.status === "unavailable" ? "stale" : "online"
  pylonCard.dataset.state = pylonState
  const pylonId = el("div", "khala-fleet-pylon-identity")
  pylonId.append(
    el("strong", undefined, data.pylon.pylonRef ?? "local Pylon"),
    el("span", "khala-fleet-pylon-message", data.pylon.message),
  )
  pylonCard.append(pylonId)
  pylonCard.append(badge(pylonState, titleize(data.pylon.status)))
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
      el("p", "khala-fleet-empty", "No Codex accounts linked yet."),
    )
  } else {
    const list = el("div", "khala-fleet-account-list")
    for (const account of data.accounts) list.append(accountCard(account, handlers))
    accountsSection.append(list)
  }
  container.append(accountsSection)

  // Active assignments
  const activeSection = el("section", "khala-fleet-section")
  activeSection.append(
    sectionHeader("Active assignments", `${data.activeAssignments.length} active`),
  )
  if (data.activeAssignments.length === 0) {
    activeSection.append(
      el("p", "khala-fleet-empty", "No active Codex assignments right now."),
    )
  } else {
    const list = el("div", "khala-fleet-assignment-list")
    for (const marker of data.activeAssignments) {
      const row = el("article", "khala-fleet-assignment")
      const chips = el("div", "khala-fleet-chips")
      if (marker.issueRef !== null) chips.append(detailChip("issue", marker.issueRef))
      if (marker.assignmentRef !== null) {
        chips.append(detailChip("assignment", marker.assignmentRef))
      }
      row.append(chips)
      list.append(row)
    }
    activeSection.append(list)
  }
  container.append(activeSection)

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

const renderConnecting = (
  container: HTMLElement,
  connect: ConnectView,
  handlers: Handlers,
): void => {
  const section = el("section", "khala-fleet-connect")
  section.append(el("h3", "khala-fleet-connect-title", `Connecting ${connect.accountRef}`))

  if (connect.start === null) {
    section.append(
      el("p", "khala-fleet-empty", "Starting Codex device login…"),
    )
    container.append(section)
    return
  }

  if (!connect.start.ok) {
    section.append(
      el(
        "p",
        "khala-fleet-error",
        `Could not start device login: ${connect.start.error ?? "unknown error"}`,
      ),
    )
  } else {
    section.append(
      el(
        "p",
        "khala-fleet-connect-hint",
        "Open the link below in your browser and enter the code to sign in. This updates automatically when it completes.",
      ),
    )
    if (connect.start.verificationUrl !== null) {
      const urlRow = el("div", "khala-fleet-connect-row")
      urlRow.append(el("span", "khala-fleet-chip-label", "url"))
      const verificationUrl = connect.start.verificationUrl
      const link = el("a", "khala-fleet-connect-url", verificationUrl)
      link.href = verificationUrl
      link.addEventListener("click", event => {
        event.preventDefault()
        handlers.onOpenUrl(verificationUrl)
      })
      urlRow.append(link)
      section.append(urlRow)
    }
    if (connect.start.userCode !== null) {
      const codeRow = el("div", "khala-fleet-connect-row")
      codeRow.append(el("span", "khala-fleet-chip-label", "code"))
      codeRow.append(el("code", "khala-fleet-connect-code", connect.start.userCode))
      section.append(codeRow)
    }
    if (connect.start.verificationUrl === null && connect.start.userCode === null) {
      section.append(
        el(
          "pre",
          "khala-fleet-connect-output",
          connect.start.output || "Waiting for the device-login prompt…",
        ),
      )
    }
    section.append(el("p", "khala-fleet-connect-status", "Waiting for authorization…"))
  }

  const close = el("button", "khala-fleet-connect-cancel", "Cancel")
  close.type = "button"
  close.addEventListener("click", handlers.onCancelConnect)
  section.append(close)
  container.append(section)
}

const render = (
  container: HTMLElement,
  view: FleetView,
  handlers: Handlers,
  activeConnect: ConnectView | null,
): void => {
  container.replaceChildren()

  const header = el("header", "khala-fleet-header")
  header.append(el("h2", "khala-fleet-title", "Fleet status"))
  const actions = el("div", "khala-fleet-actions")
  const connectBtn = el("button", "khala-fleet-refresh", "Connect account")
  connectBtn.type = "button"
  connectBtn.disabled = activeConnect !== null
  connectBtn.addEventListener("click", () => {
    // Auto-assign a short, unique ref — no name prompt.
    handlers.onConnect(`codex-${crypto.randomUUID().slice(0, 8)}`)
  })
  actions.append(connectBtn)
  const refresh = el("button", "khala-fleet-refresh", "Refresh")
  refresh.type = "button"
  refresh.disabled = view.phase === "loading"
  refresh.addEventListener("click", handlers.onRefresh)
  actions.append(refresh)
  header.append(actions)
  container.append(header)

  const body = el("div", "khala-fleet-body")
  // The connect device-auth card renders inline at the top; the fleet list stays
  // visible and live below it.
  if (activeConnect !== null) renderConnecting(body, activeConnect, handlers)
  if (view.phase === "loading") {
    body.append(el("p", "khala-fleet-empty", "Inspecting Codex fleet…"))
  } else if (view.phase === "error") {
    body.append(
      el("p", "khala-fleet-error", `Could not load fleet status: ${view.message}`),
    )
  } else {
    renderReady(body, view.data, handlers)
  }
  container.append(body)
}

export const mountFleetPanel = (
  container: HTMLElement,
  options: FleetPanelOptions,
): FleetPanelHandle => {
  let inFlight = false
  let lastData: KhalaCodeDesktopFleetStatus | null = null
  let connectPoll = 0
  let activeConnect: ConnectView | null = null
  let visible = false
  let pollTimer = 0

  const setRefreshBusy = (busy: boolean): void => {
    const buttons = container.querySelectorAll<HTMLButtonElement>(".khala-fleet-refresh")
    for (const button of buttons) {
      if (button.textContent === "Refresh") {
        button.disabled = busy
        button.textContent = busy ? "Refreshing…" : "Refresh"
      }
    }
  }

  const currentView = (): FleetView =>
    lastData !== null
      ? { phase: "ready", data: lastData }
      : { phase: "loading" }

  const paint = (): void => render(container, currentView(), handlers, activeConnect)

  const handlers: Handlers = {
    onRefresh: () => void refresh(),
    onRemove: (accountRef: string) => onRemove(accountRef),
    onConnect: (accountRef: string) => onConnect(accountRef),
    onOpenUrl: (url: string) => void options.openExternal(url),
    onCancelConnect: () => {
      window.clearTimeout(connectPoll)
      activeConnect = null
      paint()
    },
  }

  const onRemove = (accountRef: string): void => {
    container
      .querySelector(`[data-account-ref="${CSS.escape(accountRef)}"]`)
      ?.remove()
    void (async () => {
      const result = await options.removeAccount(accountRef)
      if (!result.ok) {
        render(container, { phase: "error", message: result.error ?? "remove failed" }, handlers, activeConnect)
        return
      }
      await refresh()
    })()
  }

  const onConnect = (accountRef: string): void => {
    window.clearTimeout(connectPoll)
    activeConnect = { accountRef, start: null }
    paint()
    void (async () => {
      const start = await options.connectAccount(accountRef)
      // The connect may have been cancelled while we awaited.
      if (activeConnect === null || activeConnect.accountRef !== accountRef) return
      activeConnect = { accountRef, start }
      paint()
      if (!start.ok) return
      const poll = async (): Promise<void> => {
        if (activeConnect === null || activeConnect.accountRef !== accountRef) return
        try {
          const data = await options.fetch()
          lastData = data
          const account = data.accounts.find(item => item.accountRef === accountRef)
          if (account !== undefined && accountReadinessState(account.readiness) === "ready") {
            activeConnect = null
            paint()
            return
          }
        } catch {
          // keep polling
        }
        paint()
        connectPoll = window.setTimeout(() => void poll(), 3000)
      }
      connectPoll = window.setTimeout(() => void poll(), 3000)
    })()
  }

  const refresh = async (): Promise<void> => {
    if (inFlight) return
    inFlight = true
    if (lastData === null && activeConnect === null) {
      paint()
    } else {
      setRefreshBusy(true)
    }
    try {
      const data = await options.fetch()
      lastData = data
      paint()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (lastData === null) {
        render(container, { phase: "error", message }, handlers, activeConnect)
      } else {
        setRefreshBusy(false)
      }
    } finally {
      inFlight = false
    }
  }

  const setVisible = (next: boolean): void => {
    visible = next
    window.clearInterval(pollTimer)
    if (!next) return
    void refresh()
    // Live updates: poll while the panel is visible (skipping in-flight loads).
    // The list stays live even during an active connect.
    pollTimer = window.setInterval(() => {
      if (visible && !inFlight) void refresh()
    }, 5000)
  }

  paint()
  return { refresh, setVisible }
}
