import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  DEFAULT_PANE_SYSTEM_THEME,
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

/** Basic email check: non-empty, has @, has domain with at least one dot. */
function looksLikeEmail(value: string): boolean {
  const s = value.trim()
  if (!s) return false
  const at = s.indexOf("@")
  if (at <= 0 || at === s.length - 1) return false
  const after = s.slice(at + 1)
  return after.includes(".") && !after.startsWith(".") && !after.endsWith(".")
}

function startPrelaunchCountdownTicker(container: Element): () => void {
  const wrapper = container.querySelector("[data-prelaunch-countdown]")
  const display = container.querySelector("[data-countdown-display]")
  const targetAttr = wrapper?.getAttribute("data-countdown-target")
  if (!display || !targetAttr) return () => { }
  const targetMs = Number(targetAttr)
  if (Number.isNaN(targetMs)) return () => { }

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
  if (!trigger) return () => { }

  const handler = (ev: Event): void => {
    ev.preventDefault()
    ev.stopPropagation()

    const shell = container.querySelector("[data-marketing-shell]")
    if (!(shell instanceof HTMLElement)) return

    shell.setAttribute("data-oa-home-chat-open", "1")

    const hideStyle = document.createElement("style")
    hideStyle.setAttribute("data-oa-home-chat-hide", "1")
    hideStyle.textContent =
      "[data-marketing-shell][data-oa-home-chat-open] > div:nth-child(2) { visibility: hidden !important; }"
    shell.appendChild(hideStyle)

    const overlay = document.createElement("div")
    overlay.setAttribute("data-oa-home-chat-overlay", "1")
    overlay.style.cssText =
      "position:fixed;inset:0;z-index:9998;pointer-events:auto;"
    const paneStyle = document.createElement("style")
    paneStyle.setAttribute("data-oa-home-chat-pane-style", "1")
    paneStyle.textContent =
      "[data-oa-home-chat-overlay] [data-oa-pane] { background: rgba(0,0,0,0.5) !important; }\n" +
      "[data-oa-home-chat-overlay], [data-oa-home-chat-overlay] [data-oa-pane-system], [data-oa-home-chat-overlay] [data-oa-pane-layer] { user-select: none; -webkit-user-select: none; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane], [data-oa-home-chat-overlay] [data-oa-pane] * { user-select: text; -webkit-user-select: text; }\n" +
      "[data-oa-home-chat-overlay]:focus, [data-oa-home-chat-overlay] [data-oa-pane-system]:focus { outline: none !important; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-title] { cursor: grab; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-title]:active { cursor: grabbing; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-system][data-oa-pane-dragging=\"1\"] [data-oa-pane-title] { cursor: grabbing; }"
    overlay.appendChild(paneStyle)
    const paneRoot = document.createElement("div")
    paneRoot.style.cssText = "width:100%;height:100%;"
    overlay.appendChild(paneRoot)
    shell.appendChild(overlay)

    const screen = { width: window.innerWidth, height: window.innerHeight }
    const rect = calculateNewPanePosition(undefined, screen, 520, 380)

    const paneSystem = mountPaneSystemDom(paneRoot, {
      enableDotsBackground: false,
      enableCanvasPan: false,
      enablePaneDrag: true,
      enablePaneResize: true,
      enableKeyboardShortcuts: true,
      enableHotbar: false,
      theme: { ...DEFAULT_PANE_SYSTEM_THEME, background: "transparent" },
      onPaneClosed: () => {
        paneSystem.destroy()
        overlay.remove()
        hideStyle.remove()
        shell.removeAttribute("data-oa-home-chat-open")
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

    const paneContentSlot = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-content]`)
    if (!(paneContentSlot instanceof Element)) return

    const messages: Array<{ role: "user" | "assistant"; text: string }> = [
      { role: "assistant", text: "Autopilot online." },
    ]

    const chatInputClass =
      "w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"

    const renderContent = () => {
      const messagesHtml = html`
        <div class="flex flex-col gap-2 overflow-y-auto flex-1 min-h-0 p-4">
          ${messages.map(
        (m) =>
          html`<div
                class="text-sm font-mono ${m.role === "user" ? "text-white/80 text-right" : "text-white/90"}"
                data-chat-role="${m.role}"
              >
                ${m.text}
              </div>`,
      )}
        </div>
        <form data-oa-home-chat-form="1" class="p-2 border-t border-white/10">
          <input
            type="text"
            name="email"
            placeholder="Enter your email address"
            autocomplete="email"
            class="${chatInputClass}"
            data-oa-home-chat-input="1"
          />
        </form>
      `
      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            paneContentSlot,
            html`<div class="flex flex-col h-full min-h-0">${messagesHtml}</div>`,
          )
        }).pipe(Effect.provide(EffuseLive)),
      ).then(
        () => {
          const form = paneContentSlot.querySelector("[data-oa-home-chat-form]")
          if (!(form instanceof HTMLFormElement)) return
          form.addEventListener("submit", (e: Event) => {
            e.preventDefault()
            const input = form.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
            const raw = input?.value?.trim() ?? ""
            messages.push({ role: "user", text: raw || "(empty)" })
            if (!raw) {
              messages.push({ role: "assistant", text: "Please enter your email address." })
            } else if (!looksLikeEmail(raw)) {
              messages.push({ role: "assistant", text: "Please enter a valid email address." })
            } else {
              messages.push({ role: "assistant", text: "Logging in..." })
            }
            renderContent()
          })
          const input = paneContentSlot.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
          if (input) requestAnimationFrame(() => input.focus())
        },
        () => { },
      )
    }

    renderContent()
  }

  trigger.addEventListener("click", handler, { capture: true })
  return () => trigger.removeEventListener("click", handler, { capture: true })
}

export const mountHomeController = (input: {
  readonly container: Element
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => { })

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
