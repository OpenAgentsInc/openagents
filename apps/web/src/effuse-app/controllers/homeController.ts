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

    const contentSlot = container.querySelector("[data-marketing-slot=\"content\"]")
    if (!(contentSlot instanceof HTMLElement)) return

    const hero = contentSlot.querySelector("[data-oa-home-hero]")
    if (hero instanceof HTMLElement) hero.style.opacity = "0.35"

    const overlay = document.createElement("div")
    overlay.setAttribute("data-oa-home-chat-overlay", "1")
    overlay.style.cssText =
      "position:absolute;inset:0;z-index:10;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.25);pointer-events:auto;"
    const paneWrapper = document.createElement("div")
    paneWrapper.style.cssText = "width:420px;height:320px;min-width:200px;min-height:200px;flex-shrink:0;"
    overlay.appendChild(paneWrapper)
    contentSlot.appendChild(overlay)

    const screen = { width: paneWrapper.clientWidth || 420, height: paneWrapper.clientHeight || 320 }
    const rect = calculateNewPanePosition(undefined, screen, 420, 320)

    const paneSystem = mountPaneSystemDom(paneWrapper, {
      enableDotsBackground: false,
      enableCanvasPan: false,
      enablePaneDrag: true,
      enablePaneResize: true,
      enableKeyboardShortcuts: true,
      enableHotbar: false,
      onPaneClosed: () => {
        paneSystem.destroy()
        overlay.remove()
        if (hero instanceof HTMLElement) hero.style.opacity = ""
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

    const paneContentSlot = paneWrapper.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-content]`)
    if (paneContentSlot instanceof Element) {
      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            paneContentSlot,
            html`
              <div class="flex flex-col h-full min-h-0">
                <p class="p-4 text-sm text-white/90 font-mono flex-shrink-0">Autopilot online.</p>
                <div class="mt-auto p-2 border-t border-white/10">
                  <input
                    type="text"
                    placeholder="Type a message..."
                    class="w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"
                    data-oa-home-chat-input="1"
                  />
                </div>
              </div>
            `,
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

