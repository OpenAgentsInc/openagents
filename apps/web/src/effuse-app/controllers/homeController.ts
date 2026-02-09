import { Effect } from "effect"

import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

export type HomeController = {
  readonly cleanup: () => void
}

function startPrelaunchCountdownTicker(container: Element): () => void {
  const wrapper = container.querySelector("[data-prelaunch-countdown]")
  const display = container.querySelector("[data-countdown-display]")
  const targetAttr = wrapper?.getAttribute("data-countdown-target")
  if (!display || !targetAttr) return () => {}
  const targetMs = Number(targetAttr)
  if (Number.isNaN(targetMs)) return () => {}

  const tick = () => {
    const left = targetMs - Date.now()
    display.textContent = formatCountdown(Math.max(0, left))
  }
  tick()
  const id = setInterval(tick, 1000)
  return () => clearInterval(id)
}

export const mountHomeController = (input: {
  readonly container: Element
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})

  const stopCountdown = startPrelaunchCountdownTicker(input.container)

  return {
    cleanup: () => {
      stopCountdown()
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}

