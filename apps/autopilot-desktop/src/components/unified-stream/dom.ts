import { Effect } from "effect"
import type { ConnectionPhase, RuntimeState } from "./types.js"
import { phaseLabels } from "./constants.js"

const statusClasses: Record<ConnectionPhase, string[]> = {
  connecting: ["bg-status-connecting"],
  connected: ["bg-status-connected"],
  ready: ["bg-status-ready"],
  error: ["bg-status-error"],
}

const statusClassList = [
  "bg-status-idle",
  ...Object.values(statusClasses).flat(),
]

export const setText = (container: Element, selector: string, value: string) =>
  Effect.sync(() => {
    const el = container.querySelector(selector)
    if (el) {
      el.textContent = value
    }
  })

export const setUsagePercent = (
  container: Element,
  kind: "session" | "weekly",
  percent: number | null
) =>
  Effect.sync(() => {
    const valueEl = container.querySelector(
      `[data-role='usage-${kind}-percent']`
    )
    if (valueEl) {
      valueEl.textContent =
        typeof percent === "number" ? `${percent}%` : "--"
    }

    const bar = container.querySelector<HTMLElement>(
      `[data-role='usage-${kind}-bar']`
    )
    if (bar) {
      bar.style.width = `${typeof percent === "number" ? percent : 0}%`
    }
  })

export const setUsageResetLabel = (
  container: Element,
  kind: "session" | "weekly",
  label: string | null
) =>
  setText(container, `[data-role='usage-${kind}-reset']`, label ? `· ${label}` : "")

export const setUsageWeeklyVisible = (container: Element, visible: boolean) =>
  Effect.sync(() => {
    const weekly = container.querySelector("[data-role='usage-weekly']")
    if (!weekly) {
      return
    }
    weekly.classList.toggle("hidden", !visible)
  })

export const setUsageCreditsLabel = (container: Element, label: string | null) =>
  Effect.sync(() => {
    const el = container.querySelector("[data-role='usage-credits']")
    if (!el) {
      return
    }
    el.textContent = label ?? ""
    el.classList.toggle("hidden", !label)
  })

export const setPhase = (
  container: Element,
  state: RuntimeState,
  phase: ConnectionPhase,
  error?: string
) =>
  Effect.sync(() => {
    state.phase = phase

    const dot = container.querySelector("[data-role='status-dot']")
    if (dot) {
      dot.classList.remove(...statusClassList)
      dot.classList.add(...statusClasses[phase])
    }

    const statusText = container.querySelector("[data-role='status-text']")
    if (statusText) {
      statusText.textContent = phaseLabels[phase]
    }

    const errorEl = container.querySelector("[data-role='status-error']")
    if (errorEl) {
      errorEl.textContent = error ?? ""
    }
  })

export const setButtonEnabled = (container: Element, enabled: boolean) =>
  Effect.sync(() => {
    const button = container.querySelector<HTMLButtonElement>(
      "[data-role='send-button']"
    )
    if (button) {
      button.disabled = !enabled
      button.setAttribute("aria-disabled", enabled ? "false" : "true")
    }
  })

export const setButtonLabel = (container: Element, label: string) =>
  setText(container, "[data-role='send-button']", label)
