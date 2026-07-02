import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import type { KhalaCodeDesktopFleetStatus } from "../shared/rpc"

export type KhalaCodeHotbarValue = "chat" | "fleet" | "settings"

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
    icon: "Robot",
    label: "Fleet",
    slot: 2,
    value: "fleet",
  },
  {
    actionId: "action_bar.slot_3",
    hotkey: "3",
    icon: "Settings",
    label: "Settings",
    slot: 3,
    value: "settings",
  },
]

export type SidebarMountOptions = Readonly<{
  readonly fleetCounts?: KhalaCodeSidebarFleetCounts | null
  readonly selectedValue?: string | null
  readonly onActivate?: (value: string) => void
}>

export type KhalaCodeSidebarFleetCounts = Readonly<{
  accountsReady: number
  workersActive: number
  slotsFree: number
  flags: number
}>

export type KhalaCodeSidebarHandle = Readonly<{
  setFleetCounts: (counts: KhalaCodeSidebarFleetCounts | null) => void
}>

type NavigatorWithUserAgentData = Navigator & {
  readonly userAgentData?: {
    readonly platform?: string
  }
}

type HotbarShortcut = Readonly<{
  ariaModifier: "Alt"
  label: "Alt" | "Option"
  modifierKey: "altKey"
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
        ariaModifier: "Alt",
        label: "Option",
        modifierKey: "altKey",
      }
    : {
        ariaModifier: "Alt",
        label: "Alt",
        modifierKey: "altKey",
      }

const readinessNeedsHuman = (readiness: string): boolean => {
  const value = readiness.toLowerCase()
  return value.includes("auth") || value.includes("missing") || value.includes("error")
}

const blockerNeedsHuman = (refs: readonly string[]): boolean =>
  refs.some(ref => /approval|blocked|claim[_-]?expired|cooldown|merge[_-]?conflict|permission/iu.test(ref))

export const projectKhalaCodeSidebarFleetCounts = (
  status: KhalaCodeDesktopFleetStatus,
): KhalaCodeSidebarFleetCounts => ({
  accountsReady: status.accounts.filter(account => account.readiness.toLowerCase() === "ready").length,
  workersActive: status.activeAssignments.length,
  slotsFree: status.availableCodexAssignments ?? 0,
  flags: status.accounts.filter(account => readinessNeedsHuman(account.readiness)).length +
    status.activeAssignments.filter(assignment =>
      blockerNeedsHuman(assignment.blockerRefs ?? assignment.workerSession?.blockerRefs ?? [])
    ).length +
    (
      status.availableCodexAssignments === 0 &&
      status.accounts.some(account => account.queuePolicy?.cooldown === "cooling_down")
        ? 1
        : 0
    ),
})

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): KhalaCodeSidebarHandle => {
  let selectedValue = options.selectedValue ?? "chat"
  let fleetCounts = options.fleetCounts ?? null
  const shortcut = hotbarShortcut()

  const activate = (slot: KhalaCodeHotbarSlot): void => {
    selectedValue = slot.value
    render()
    options.onActivate?.(slot.value)
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
    if (slot.value === "fleet" && fleetCounts !== null) {
      const counts = document.createElement("span")
      counts.className = "khala-code-hotbar-fleet-counts"
      counts.setAttribute("data-khala-code-fleet-counts", "")
      counts.setAttribute(
        "aria-label",
        `${fleetCounts.accountsReady} accounts ready, ${fleetCounts.workersActive} workers active, ${fleetCounts.slotsFree} slots free, ${fleetCounts.flags} flags`,
      )
      counts.textContent = [
        `${fleetCounts.accountsReady} acct`,
        `${fleetCounts.workersActive} work`,
        `${fleetCounts.slotsFree} free`,
        `${fleetCounts.flags} flag`,
      ].join(" / ")
      button.append(counts)
    }
    button.addEventListener("click", () => activate(slot))
    return button
  }

  function render(): void {
    const nav = document.createElement("nav")
    nav.className = "khala-code-hotbar"
    nav.setAttribute("aria-label", "Khala Code command hotbar")
    nav.dataset.khalaCodeHotbar = ""

    const slots = document.createElement("div")
    slots.className = "khala-code-hotbar-slots"
    slots.append(...KHALA_CODE_HOTBAR_SLOTS.map(hotbarButton))

    nav.append(slots)
    container.replaceChildren(nav)
  }

  window.addEventListener("keydown", event => {
    const slot = KHALA_CODE_HOTBAR_SLOTS.find(item => item.hotkey === event.key)
    if (slot === undefined) return

    const explicitHotkey =
      event[shortcut.modifierKey] &&
      !event.shiftKey &&
      !event.ctrlKey &&
      !event.metaKey
    if (!explicitHotkey) return

    event.preventDefault()
    activate(slot)
  })

  render()
  return {
    setFleetCounts(next: KhalaCodeSidebarFleetCounts | null): void {
      fleetCounts = next
      render()
    },
  }
}
