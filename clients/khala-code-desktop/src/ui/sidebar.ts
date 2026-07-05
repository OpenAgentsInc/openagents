import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import type { KhalaCodeDesktopFleetStatus } from "../shared/rpc"
import {
  khalaCodeInboxAssignmentNeedsHuman,
  khalaCodeInboxReadinessNeedsHuman,
} from "./inbox"

export type KhalaCodeHotbarValue =
  | "chat"
  | "fleet"
  | "forum"
  | "inbox"
  | "settings"
  | "editor"

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
    icon: "BookOpen",
    label: "Forum",
    slot: 3,
    value: "forum",
  },
  {
    actionId: "action_bar.slot_4",
    hotkey: "4",
    icon: "NotificationBell",
    label: "Inbox",
    slot: 4,
    value: "inbox",
  },
  {
    actionId: "action_bar.slot_5",
    hotkey: "5",
    icon: "Settings",
    label: "Settings",
    slot: 5,
    value: "settings",
  },
  {
    actionId: "action_bar.slot_6",
    hotkey: "6",
    icon: "Code",
    label: "Editor",
    slot: 6,
    value: "editor",
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
  destroy: () => void
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
  visiblePrefix: "Alt+" | "⌥"
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
        visiblePrefix: "⌥",
        modifierKey: "altKey",
      }
    : {
        ariaModifier: "Alt",
        label: "Alt",
        visiblePrefix: "Alt+",
        modifierKey: "altKey",
      }

const hotbarDigitForKeyboardEvent = (event: KeyboardEvent): string | null => {
  if (/^[0-9]$/u.test(event.key)) return event.key

  const physicalDigit = /^Digit([0-9])$/u.exec(event.code)?.[1]
  if (physicalDigit !== undefined) return physicalDigit

  const numpadDigit = /^Numpad([0-9])$/u.exec(event.code)?.[1]
  return numpadDigit ?? null
}

export const projectKhalaCodeSidebarFleetCounts = (
  status: KhalaCodeDesktopFleetStatus,
): KhalaCodeSidebarFleetCounts => ({
  accountsReady: status.accounts.filter(account => account.readiness.toLowerCase() === "ready").length,
  workersActive: status.activeAssignments.length,
  slotsFree: status.availableCodexAssignments ?? 0,
  flags: status.accounts.filter(account => khalaCodeInboxReadinessNeedsHuman(account.readiness)).length +
    status.activeAssignments.filter(assignment => {
      const refs = assignment.blockerRefs ?? assignment.workerSession?.blockerRefs ?? []
      const approvalRequired = assignment.workerSession?.approvalState === "approval_required"
      const blocked = refs.length > 0 || assignment.workerSession?.approvalState === "blocked"
      return khalaCodeInboxAssignmentNeedsHuman(refs, approvalRequired, blocked)
    }).length +
    (
      status.availableCodexAssignments === 0 &&
      status.accounts.length > 0 &&
      status.accounts.every(account =>
        account.queuePolicy?.cooldown === "cooling_down" ||
        account.quotaState?.toLowerCase() === "cooling_down"
      )
        ? 1
        : 0
    ),
})

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): KhalaCodeSidebarHandle => {
  let selectedValue = options.selectedValue ?? "chat"
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
      `${slot.label}, ${shortcut.label}+${slot.hotkey}`,
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
    key.textContent = `${shortcut.visiblePrefix}${slot.hotkey}`

    const label = document.createElement("span")
    label.className = "khala-code-hotbar-label"
    label.textContent = slot.label

    button.append(key, label)
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

  const onKeydown = (event: KeyboardEvent): void => {
    const explicitHotkey =
      event[shortcut.modifierKey] &&
      !event.shiftKey &&
      !event.ctrlKey &&
      !event.metaKey
    if (!explicitHotkey) return

    const digit = hotbarDigitForKeyboardEvent(event)
    const slot = KHALA_CODE_HOTBAR_SLOTS.find(item => item.hotkey === digit)
    if (slot === undefined) return

    event.preventDefault()
    activate(slot)
  }

  window.addEventListener("keydown", onKeydown)

  render()
  return {
    destroy(): void {
      window.removeEventListener("keydown", onKeydown)
      container.replaceChildren()
    },
    setFleetCounts(next: KhalaCodeSidebarFleetCounts | null): void {
      void next
      render()
    },
  }
}
