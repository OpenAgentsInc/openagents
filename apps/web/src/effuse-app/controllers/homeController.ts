import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
} from "@openagentsinc/effuse-panes"

import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

export type HomeController = {
  readonly cleanup: () => void
}

const CHAT_PANE_ID = "home-chat"

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

function openChatPaneOnHome(container: Element): () => void {
  const trigger = container.querySelector("[data-oa-open-chat-pane]")
  if (!trigger) return () => {}

  const handler = (ev: Event): void => {
    ev.preventDefault()
    ev.stopPropagation()

    const shell = container.querySelector("[data-marketing-shell]")
    if (!(shell instanceof HTMLElement)) return

    const overlay = document.createElement("div")
    overlay.style.cssText =
      "position:fixed;inset:0;z-index:9998;pointer-events:auto;display:flex;align-items:stretch;justify-content:stretch;"
    const root = document.createElement("div")
    root.style.cssText = "width:100%;height:100%;"
    overlay.appendChild(root)
    shell.appendChild(overlay)

    const screen = { width: window.innerWidth, height: window.innerHeight }
    const rect = calculateNewPanePosition(undefined, screen, 420, 280)

    const paneSystem = mountPaneSystemDom(root, {
      enableDotsBackground: false,
      enableCanvasPan: false,
      enablePaneDrag: true,
      enablePaneResize: true,
      enableKeyboardShortcuts: true,
      enableHotbar: false,
      onPaneClosed: () => {
        paneSystem.destroy()
        overlay.remove()
      },
    })

    paneSystem.store.addPane({
      id: CHAT_PANE_ID,
      kind: "chat",
      title: "Chat",
      rect,
      dismissable: true,
    })
    paneSystem.store.bringToFront(CHAT_PANE_ID)
    paneSystem.render()

    const contentSlot = root.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-content]`)
    if (contentSlot instanceof Element) {
      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            contentSlot,
            html`<p class="p-4 text-sm text-white/90 font-mono">Autopilot online.</p>`,
          )
        }).pipe(Effect.provide(EffuseLive)),
      ).catch(() => {})
    }
  }

  trigger.addEventListener("click", handler, { capture: true })
  return () => trigger.removeEventListener("click", handler, { capture: true })
}

export const mountHomeController = (input: {
  readonly container: Element
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})

  const stopCountdown = startPrelaunchCountdownTicker(input.container)
  const stopOpenChatPane = openChatPaneOnHome(input.container)

  return {
    cleanup: () => {
      stopCountdown()
      stopOpenChatPane()
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}

