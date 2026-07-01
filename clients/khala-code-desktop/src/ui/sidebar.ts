import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"

import type {
  KhalaCodeDesktopFleetAssignment,
  KhalaCodeDesktopFleetStatus,
} from "../shared/rpc"

export type KhalaCodeHotbarValue =
  | "chat"
  | "inbox"
  | "fleet"
  | "gym"
  | "settings"

export type KhalaCodeHotbarSlot = Readonly<{
  actionId: `action_bar.slot_${number}`
  hotkey: `${number}`
  icon: IconName
  label: string
  slot: number
  value: KhalaCodeHotbarValue
}>

export const KHALA_CODE_HOTBAR_SLOTS: ReadonlyArray<KhalaCodeHotbarSlot> = [
  {
    actionId: "action_bar.slot_1",
    hotkey: "1",
    icon: "Chat",
    label: "Chat",
    slot: 1,
    value: "chat",
  },
  {
    actionId: "action_bar.slot_2",
    hotkey: "2",
    icon: "NotificationBell",
    label: "Inbox",
    slot: 2,
    value: "inbox",
  },
  {
    actionId: "action_bar.slot_3",
    hotkey: "3",
    icon: "Robot",
    label: "Fleet",
    slot: 3,
    value: "fleet",
  },
  {
    actionId: "action_bar.slot_4",
    hotkey: "4",
    icon: "Dumbbell",
    label: "Gym",
    slot: 4,
    value: "gym",
  },
  {
    actionId: "action_bar.slot_5",
    hotkey: "5",
    icon: "Settings",
    label: "Settings",
    slot: 5,
    value: "settings",
  },
]

export type SidebarMountOptions = Readonly<{
  readonly selectedValue?: string | null
  readonly fetchFleet?: () => Promise<KhalaCodeDesktopFleetStatus>
  readonly onActivate?: (value: string) => void
  readonly onOpenFleet?: () => void
}>

export type KhalaCodeSidebarFleetSession = Readonly<{
  label: string
  ref: string
  state: string
  tone: "active" | "blocked" | "review"
}>

export type KhalaCodeSidebarFleetSummary = Readonly<{
  activeAssignments: number
  availableSlots: number | null
  busySlots: number
  connectedAccounts: number
  maxSlots: number | null
  message: string
  overflowSessions: number
  pylonStatus: KhalaCodeDesktopFleetStatus["pylon"]["status"]
  queuedSlots: number
  readyAccounts: number
  reconnectAccounts: number
  sessions: readonly KhalaCodeSidebarFleetSession[]
  tone: "degraded" | "offline" | "online"
}>

type FleetSummaryView =
  | { readonly phase: "idle" }
  | { readonly phase: "loading" }
  | { readonly message: string; readonly phase: "error" }
  | {
      readonly phase: "ready"
      readonly summary: KhalaCodeSidebarFleetSummary
    }

type NavigatorWithUserAgentData = Navigator & {
  readonly userAgentData?: {
    readonly platform?: string
  }
}

type HotbarShortcut = Readonly<{
  ariaModifier: "Control" | "Meta"
  label: "Command" | "Ctrl"
  modifierKey: "ctrlKey" | "metaKey"
}>

const platformName = (): string => {
  const navigatorWithUserAgentData = navigator as NavigatorWithUserAgentData
  return (
    navigatorWithUserAgentData.userAgentData?.platform ??
    navigator.platform ??
    ""
  )
}

const isApplePlatform = (): boolean =>
  /Mac|iPhone|iPad|iPod/i.test(platformName())

const hotbarShortcut = (): HotbarShortcut =>
  isApplePlatform()
    ? {
        ariaModifier: "Meta",
        label: "Command",
        modifierKey: "metaKey",
      }
    : {
        ariaModifier: "Control",
        label: "Ctrl",
        modifierKey: "ctrlKey",
      }

const isEditableTarget = (target: EventTarget | null): boolean =>
  target instanceof Element &&
  target.closest("input, textarea, select, [contenteditable='true']") !== null

const readinessState = (
  readiness: string,
): "degraded" | "missing" | "ready" => {
  const value = readiness.toLowerCase()
  if (value === "ready") return "ready"
  if (
    value.includes("auth") ||
    value.includes("cred") ||
    value.includes("expired") ||
    value.includes("missing")
  ) {
    return "missing"
  }
  return "degraded"
}

const finiteNumber = (value: number | null | undefined): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0

const isDisplayOnlyDefaultAccountRef = (accountRef: string): boolean =>
  /^(?:\(default\)|default)$/iu.test(accountRef.trim())

const isSidebarWorkerFleetAccount = (
  account: KhalaCodeDesktopFleetStatus["accounts"][number],
): boolean =>
  account.sessionRole !== "main_local_codex_session" &&
  account.homeRole !== "main_user_codex_home_display_only" &&
  !isDisplayOnlyDefaultAccountRef(account.accountRef)

const compactRef = (ref: string): string => {
  const parts = ref.split(/[.:/]+/u).filter(Boolean)
  const last = parts[parts.length - 1] ?? ref
  return last.length > 10 ? `${last.slice(0, 7)}...` : last
}

const knownSlotTotal = (
  value: number | null,
  fallback: number,
  hasCapacity: boolean,
): number | null => (value !== null ? value : hasCapacity ? fallback : null)

const fleetSessionSummary = (
  assignment: KhalaCodeDesktopFleetAssignment,
): KhalaCodeSidebarFleetSession => {
  const ref =
    assignment.issueRef ??
    assignment.assignmentRef ??
    assignment.workerSession?.transcriptRef ??
    "worker session"
  const reviewState = assignment.workerSession?.reviewState ?? "active"
  const approvalState = assignment.workerSession?.approvalState ?? "none"
  const closeoutStatus = assignment.closeoutStatus ?? assignment.workerSession?.closeoutStatus
  const state =
    approvalState === "approval_required"
      ? "approval"
      : reviewState === "ready_for_review"
        ? "review"
        : reviewState === "pending_closeout"
          ? "closeout"
          : reviewState === "blocked" || approvalState === "blocked"
            ? "blocked"
            : closeoutStatus ?? "active"
  const tone =
    state === "approval" || state === "blocked"
      ? "blocked"
      : state === "review" || state === "closeout"
        ? "review"
        : "active"

  return {
    label: compactRef(ref),
    ref,
    state,
    tone,
  }
}

export const projectKhalaCodeSidebarFleetSummary = (
  status: KhalaCodeDesktopFleetStatus,
): KhalaCodeSidebarFleetSummary => {
  const accounts = status.accounts.filter(isSidebarWorkerFleetAccount)
  const hasCapacity = accounts.some(account => account.capacity !== null)
  const fallbackAvailable = accounts.reduce(
    (total, account) => total + finiteNumber(account.capacity?.available),
    0,
  )
  const fallbackReady = accounts.reduce(
    (total, account) => total + finiteNumber(account.capacity?.ready),
    0,
  )
  const busySlots = accounts.reduce(
    (total, account) => total + finiteNumber(account.capacity?.busy),
    0,
  )
  const queuedSlots = accounts.reduce(
    (total, account) => total + finiteNumber(account.capacity?.queued),
    0,
  )
  const readyAccounts = accounts.filter(
    account => readinessState(account.readiness) === "ready",
  ).length
  const reconnectAccounts = accounts.filter(
    account => readinessState(account.readiness) !== "ready",
  ).length
  const availableSlots = knownSlotTotal(
    status.availableCodexAssignments,
    fallbackAvailable,
    hasCapacity,
  )
  const maxSlots = knownSlotTotal(
    status.maxCodexAssignments,
    fallbackReady,
    hasCapacity,
  )
  const sessions = status.activeAssignments.slice(0, 3).map(fleetSessionSummary)
  const tone =
    !status.ok || status.pylon.status === "unavailable"
      ? "offline"
      : reconnectAccounts > 0
        ? "degraded"
        : "online"

  return {
    activeAssignments: status.activeAssignments.length,
    availableSlots,
    busySlots: busySlots === 0 ? status.activeAssignments.length : busySlots,
    connectedAccounts: accounts.length,
    maxSlots,
    message: status.pylon.message,
    overflowSessions: Math.max(0, status.activeAssignments.length - sessions.length),
    pylonStatus: status.pylon.status,
    queuedSlots,
    readyAccounts,
    reconnectAccounts,
    sessions,
    tone,
  }
}

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): void => {
  let selectedValue = options.selectedValue ?? "chat"
  const shortcut = hotbarShortcut()
  let fleetSummaryView: FleetSummaryView = options.fetchFleet === undefined
    ? { phase: "idle" }
    : { phase: "loading" }
  let fleetRefreshInFlight = false

  const activate = (slot: KhalaCodeHotbarSlot): void => {
    selectedValue = slot.value
    render()
    options.onActivate?.(slot.value)
  }

  const openFleetSummary = (): void => {
    selectedValue = "fleet"
    render()
    if (options.onOpenFleet !== undefined) {
      options.onOpenFleet()
    } else {
      options.onActivate?.("fleet")
    }
  }

  const refreshFleetSummary = async (): Promise<void> => {
    if (options.fetchFleet === undefined || fleetRefreshInFlight) return
    fleetRefreshInFlight = true
    if (fleetSummaryView.phase === "idle") {
      fleetSummaryView = { phase: "loading" }
      render()
    }
    try {
      const status = await options.fetchFleet()
      fleetSummaryView = {
        phase: "ready",
        summary: projectKhalaCodeSidebarFleetSummary(status),
      }
    } catch (error) {
      fleetSummaryView = {
        message: error instanceof Error ? error.message : "Fleet status unavailable",
        phase: "error",
      }
    } finally {
      fleetRefreshInFlight = false
      render()
    }
  }

  const hotbarButton = (slot: KhalaCodeHotbarSlot): HTMLButtonElement => {
    const active = selectedValue === slot.value
    const button = document.createElement("button")
    button.type = "button"
    button.className = [
      "khala-code-hotbar-slot",
      "khala-code-hotbar-slot-filled",
      `khala-code-hotbar-slot-${slot.slot}`,
    ].join(" ")
    button.dataset.hotbarAction = slot.actionId
    button.dataset.khalaCodeHotbarSlot = String(slot.slot)
    button.dataset.khalaCodeHotbarValue = slot.value
    button.dataset.hotkey = slot.hotkey
    button.dataset.active = active ? "true" : "false"
    button.setAttribute("aria-pressed", active ? "true" : "false")
    button.setAttribute(
      "aria-keyshortcuts",
      `${shortcut.ariaModifier}+${slot.hotkey}`,
    )
    button.setAttribute(
      "aria-label",
      `${slot.label}, command slot ${slot.hotkey}`,
    )
    button.title = `${slot.label} (${shortcut.label}+${slot.hotkey})`
    button.append(
      iconElement(slot.icon, {
        className: "khala-code-hotbar-icon",
        dataIcon: slot.value,
      }),
    )

    const key = document.createElement("span")
    key.className = "khala-code-hotbar-key"
    key.setAttribute("aria-hidden", "true")
    key.textContent = slot.hotkey

    const label = document.createElement("span")
    label.className = "khala-code-hotbar-label"
    label.textContent = slot.label

    button.append(key, label)
    button.addEventListener("click", () => activate(slot))
    return button
  }

  const fleetMetric = (
    label: string,
    value: string,
    state?: "alert" | "muted",
  ): HTMLElement => {
    const metric = document.createElement("span")
    metric.className = "khala-code-fleet-metric"
    if (state !== undefined) metric.dataset.state = state

    const valueNode = document.createElement("span")
    valueNode.className = "khala-code-fleet-value"
    valueNode.textContent = value

    const labelNode = document.createElement("span")
    labelNode.className = "khala-code-fleet-label"
    labelNode.textContent = label

    metric.append(valueNode, labelNode)
    return metric
  }

  const slotFraction = (value: number | null, total: number | null): string =>
    value === null || total === null ? "?/?" : `${value}/${total}`

  const fleetSummaryNode = (): HTMLElement | null => {
    if (fleetSummaryView.phase === "idle") return null

    const section = document.createElement("section")
    section.className = "khala-code-fleet-summary"
    section.dataset.active = selectedValue === "fleet" ? "true" : "false"
    section.dataset.phase = fleetSummaryView.phase

    const summaryButton = document.createElement("button")
    summaryButton.type = "button"
    summaryButton.className = "khala-code-fleet-strip"
    summaryButton.addEventListener("click", openFleetSummary)

    const header = document.createElement("span")
    header.className = "khala-code-fleet-header"

    const title = document.createElement("span")
    title.className = "khala-code-fleet-kicker"
    title.textContent = "Fleet"

    const status = document.createElement("span")
    status.className = "khala-code-fleet-status"
    status.setAttribute("aria-hidden", "true")

    header.append(title, status)

    if (fleetSummaryView.phase === "loading") {
      section.dataset.tone = "online"
      summaryButton.setAttribute("aria-label", "Open Fleet. Fleet status loading.")
      summaryButton.title = "Open Fleet"
      summaryButton.append(
        header,
        fleetMetric("sync", "...", "muted"),
      )
      section.append(summaryButton)
      return section
    }

    if (fleetSummaryView.phase === "error") {
      section.dataset.tone = "offline"
      summaryButton.setAttribute("aria-label", "Open Fleet. Fleet status unavailable.")
      summaryButton.title = fleetSummaryView.message
      summaryButton.append(
        header,
        fleetMetric("status", "off", "alert"),
      )
      section.append(summaryButton)
      return section
    }

    const { summary } = fleetSummaryView
    section.dataset.tone = summary.tone
    summaryButton.setAttribute(
      "aria-label",
      `Open Fleet. ${summary.readyAccounts} of ${summary.connectedAccounts} accounts ready, ${summary.activeAssignments} active worker sessions.`,
    )
    summaryButton.title = `Open Fleet: ${summary.message}`

    const metrics = document.createElement("span")
    metrics.className = "khala-code-fleet-metrics"
    metrics.append(
      fleetMetric("acct", `${summary.readyAccounts}/${summary.connectedAccounts}`),
      fleetMetric("free", slotFraction(summary.availableSlots, summary.maxSlots)),
      fleetMetric("run", String(summary.activeAssignments)),
    )
    if (summary.reconnectAccounts > 0) {
      metrics.append(fleetMetric("auth", String(summary.reconnectAccounts), "alert"))
    } else if (summary.queuedSlots > 0) {
      metrics.append(fleetMetric("queue", String(summary.queuedSlots)))
    } else {
      metrics.append(fleetMetric("busy", String(summary.busySlots), "muted"))
    }

    summaryButton.append(header, metrics)
    section.append(summaryButton)

    const sessions = document.createElement("div")
    sessions.className = "khala-code-fleet-sessions"
    sessions.setAttribute("aria-label", "Fleet worker sessions")
    if (summary.sessions.length === 0) {
      const empty = document.createElement("span")
      empty.className = "khala-code-fleet-empty"
      empty.textContent = "idle"
      sessions.append(empty)
    } else {
      for (const session of summary.sessions) {
        const button = document.createElement("button")
        button.type = "button"
        button.className = "khala-code-fleet-session"
        button.dataset.fleetSession = session.ref
        button.dataset.state = session.tone
        button.setAttribute("aria-label", `Open Fleet session ${session.ref}`)
        button.title = `Open Fleet session ${session.ref}`
        button.addEventListener("click", openFleetSummary)

        const label = document.createElement("span")
        label.className = "khala-code-fleet-session-label"
        label.textContent = session.label

        const state = document.createElement("span")
        state.className = "khala-code-fleet-session-state"
        state.textContent = session.state

        button.append(label, state)
        sessions.append(button)
      }
      if (summary.overflowSessions > 0) {
        const overflow = document.createElement("button")
        overflow.type = "button"
        overflow.className = "khala-code-fleet-session khala-code-fleet-session-overflow"
        overflow.setAttribute("aria-label", `Open Fleet with ${summary.overflowSessions} more sessions`)
        overflow.title = `Open Fleet with ${summary.overflowSessions} more sessions`
        overflow.textContent = `+${summary.overflowSessions}`
        overflow.addEventListener("click", openFleetSummary)
        sessions.append(overflow)
      }
    }
    section.append(sessions)
    return section
  }

  function render(): void {
    const nav = document.createElement("nav")
    nav.className = "khala-code-hotbar"
    nav.setAttribute("aria-label", "Khala Code command hotbar")
    nav.dataset.khalaCodeHotbar = ""

    const slots = document.createElement("div")
    slots.className = "khala-code-hotbar-slots"
    slots.append(...KHALA_CODE_HOTBAR_SLOTS.map(hotbarButton))

    const summary = fleetSummaryNode()
    nav.append(slots)
    if (summary !== null) nav.append(summary)
    container.replaceChildren(nav)
  }

  window.addEventListener("keydown", event => {
    const slot = KHALA_CODE_HOTBAR_SLOTS.find(item => item.hotkey === event.key)
    if (slot === undefined) return

    const editable = isEditableTarget(event.target)
    const explicitHotkey =
      event[shortcut.modifierKey] &&
      !event.altKey &&
      !event.shiftKey &&
      (shortcut.modifierKey === "metaKey" ? !event.ctrlKey : !event.metaKey)
    const ambientHotkey =
      !editable &&
      !event.altKey &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.shiftKey
    if (!explicitHotkey && !ambientHotkey) return

    event.preventDefault()
    activate(slot)
  })

  render()
  if (options.fetchFleet !== undefined) {
    void refreshFleetSummary()
    window.setInterval(() => void refreshFleetSummary(), 7000)
  }
}
