import { createKeyHoldTracker } from "./thread-hotkeys"

export type RecentThreadHotkeyHintTarget = {
  readonly setHotkeyHintsVisible: (visible: boolean) => void
}

export type RecentThreadHotkeyHintOptions = {
  readonly holdDelayMs?: number
}

export type RecentThreadHotkeyHintBinding = {
  readonly dispose: () => void
}

export const bindRecentThreadHotkeyHints = (
  window: unknown,
  target: RecentThreadHotkeyHintTarget,
  options: RecentThreadHotkeyHintOptions = {},
): RecentThreadHotkeyHintBinding => {
  const eventTarget = window as EventTarget
  const hold = createKeyHoldTracker({
    ...(options.holdDelayMs === undefined ? {} : { holdDelayMs: options.holdDelayMs }),
    onHide: () => target.setHotkeyHintsVisible(false),
    onReveal: () => target.setHotkeyHintsVisible(true),
  })

  const keydown = (event: Event): void => {
    const keyboardEvent = event as KeyboardEvent
    if (
      keyboardEvent.key === "Meta" &&
      !keyboardEvent.altKey &&
      !keyboardEvent.ctrlKey &&
      !keyboardEvent.shiftKey
    ) {
      hold.keyDown()
    }
  }
  const keyup = (event: Event): void => {
    if ((event as KeyboardEvent).key === "Meta") hold.keyUp()
  }
  const blur = (): void => hold.keyUp()

  eventTarget.addEventListener("keydown", keydown)
  eventTarget.addEventListener("keyup", keyup)
  eventTarget.addEventListener("blur", blur)

  return {
    dispose: () => {
      hold.keyUp()
      eventTarget.removeEventListener("keydown", keydown)
      eventTarget.removeEventListener("keyup", keyup)
      eventTarget.removeEventListener("blur", blur)
    },
  }
}
