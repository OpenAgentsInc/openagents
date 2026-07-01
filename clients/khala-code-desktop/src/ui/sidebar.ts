import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"

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
  readonly onActivate?: (value: string) => void
}>

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

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): void => {
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
}
