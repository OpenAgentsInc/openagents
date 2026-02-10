import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  DEFAULT_PANE_SYSTEM_THEME,
} from "@openagentsinc/effuse-panes"

import { AuthService, clearAuthClientCache } from "../../effect/auth"
import { SessionAtom } from "../../effect/atoms/session"
import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { AppRuntime } from "../../effect/runtime"

export type HomeController = {
  readonly cleanup: () => void
}

type HomeChatDeps = {
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly navigate: (href: string) => void
  readonly signOut: () => void | Promise<void>
}

const CHAT_PANE_ID = "home-chat"
const IDENTITY_PANE_ID = "home-identity"
const IDENTITY_PANE_RECT = { x: 16, y: 16, width: 260, height: 52 }

const normalizeEmail = (raw: string): string => raw.trim().toLowerCase()

/** Basic email check: non-empty, has @, has domain with at least one dot. */
function looksLikeEmail(value: string): boolean {
  const s = value.trim()
  if (!s) return false
  const at = s.indexOf("@")
  if (at <= 0 || at === s.length - 1) return false
  const after = s.slice(at + 1)
  return after.includes(".") && !after.startsWith(".") && !after.endsWith(".")
}

/** Exactly 6 digits. */
function isSixDigitCode(value: string): boolean {
  return /^[0-9]{6}$/.test(value.replace(/\s+/g, ""))
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

function openChatPaneOnHome(container: Element, deps: HomeChatDeps | undefined): () => void {
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
      "[data-oa-home-chat-overlay] [data-oa-pane-system][data-oa-pane-dragging=\"1\"] [data-oa-pane-title] { cursor: grabbing; }\n" +
      "[data-oa-home-chat-overlay] [data-pane-id=\"home-identity\"] [data-oa-pane-title] { display: none; }\n" +
      "[data-oa-home-chat-overlay] [data-pane-id=\"home-identity\"] [data-oa-pane-content] { padding: 0; }"
    overlay.appendChild(paneStyle)
    const paneRoot = document.createElement("div")
    paneRoot.style.cssText = "width:100%;height:100%;"
    overlay.appendChild(paneRoot)
    shell.appendChild(overlay)

    const screen = { width: window.innerWidth, height: window.innerHeight }
    const rect = calculateNewPanePosition(undefined, screen, 640, 480)

    const closeOverlay = () => {
      paneSystem.destroy()
      overlay.remove()
      hideStyle.remove()
      shell.removeAttribute("data-oa-home-chat-open")
    }

    const paneSystem = mountPaneSystemDom(paneRoot, {
      enableDotsBackground: false,
      enableCanvasPan: false,
      enablePaneDrag: true,
      enablePaneResize: true,
      enableKeyboardShortcuts: true,
      enableHotbar: false,
      theme: { ...DEFAULT_PANE_SYSTEM_THEME, background: "transparent" },
      onPaneClosed: closeOverlay,
    })

    const session = deps?.atoms?.get(SessionAtom as any) ?? { userId: null, user: null }
    const isAuthed = session.user != null

    if (isAuthed && deps) {
      paneSystem.store.addPane({
        id: IDENTITY_PANE_ID,
        kind: "generic",
        title: "",
        rect: IDENTITY_PANE_RECT,
        dismissable: false,
      })
    }

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

    type Step = "email" | "code" | "authed"
    const messages: Array<{ role: "user" | "assistant"; text: string }> = isAuthed
      ? [{ role: "assistant", text: "Autopilot online. Awaiting instructions." }]
      : [{ role: "assistant", text: "Autopilot initialized. Enter your email address to begin." }]
    let step: Step = isAuthed ? "authed" : "email"
    let email = ""
    let isBusy = false
    let identityPaneAdded = isAuthed

    const chatInputClass =
      "w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"

    const renderIdentityPaneContent = (userEmail: string) => {
      const identitySlot = paneRoot.querySelector(`[data-pane-id="${IDENTITY_PANE_ID}"] [data-oa-pane-content]`)
      if (!(identitySlot instanceof Element)) return
      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            identitySlot,
            html`
              <div class="flex h-full items-center justify-between gap-2 px-3 text-xs font-mono text-white/90">
                <span class="truncate" title="${userEmail}">${userEmail}</span>
                <button
                  type="button"
                  data-oa-home-identity-logout="1"
                  class="shrink-0 rounded px-2 py-1 text-white/60 hover:bg-white/10 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
                >
                  Log out
                </button>
              </div>
            `,
          )
        }).pipe(Effect.provide(EffuseLive)),
      ).then(() => {
        const btn = identitySlot.querySelector("[data-oa-home-identity-logout]")
        if (btn) {
          btn.addEventListener("click", () => {
            void Promise.resolve(deps?.signOut?.()).then(() => closeOverlay())
          })
        }
      }, () => {})
    }

    if (identityPaneAdded && deps?.atoms) {
      const currentSession = deps.atoms.get(SessionAtom as any)
      if (currentSession?.user?.email) renderIdentityPaneContent(currentSession.user.email)
    }

    const doRender = () => {
      const formHtml =
        step === "authed"
          ? html``
          : step === "email"
            ? html`
                <form data-oa-home-chat-form="1" data-oa-home-chat-step="email" class="p-2 border-t border-white/10">
                  <input
                    type="text"
                    name="email"
                    placeholder="your@email.com"
                    autocomplete="email"
                    class="${chatInputClass}"
                    data-oa-home-chat-input="1"
                  />
                </form>
              `
            : html`
                <form data-oa-home-chat-form="1" data-oa-home-chat-step="code" class="p-2 border-t border-white/10">
                  <input
                    type="text"
                    name="code"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    placeholder="123456"
                    maxlength="6"
                    class="${chatInputClass}"
                    data-oa-home-chat-input="1"
                  />
                </form>
              `

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
        ${formHtml}
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
            if (isBusy) return
            const input = form.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
            const raw = input?.value?.trim() ?? ""

            if (step === "email") {
              messages.push({ role: "user", text: raw || "(empty)" })
              if (!raw) {
                messages.push({ role: "assistant", text: "Please enter your email address." })
                doRender()
                return
              }
              if (!looksLikeEmail(raw)) {
                messages.push({ role: "assistant", text: "Please enter a valid email address." })
                doRender()
                return
              }
              const nextEmail = normalizeEmail(raw)
              isBusy = true
              messages.push({ role: "assistant", text: "Sending code…" })
              doRender()
              fetch("/api/auth/start", {
                method: "POST",
                headers: { "content-type": "application/json" },
                credentials: "include",
                body: JSON.stringify({ email: nextEmail }),
              })
                .then((r) => r.json().catch(() => null))
                .then((data: { ok?: boolean; error?: string }) => {
                  isBusy = false
                  if (data?.ok) {
                    email = nextEmail
                    step = "code"
                    messages.push({
                      role: "assistant",
                      text: `Check ${email}. Enter six digit verification code:`,
                    })
                  } else {
                    messages.push({
                      role: "assistant",
                      text: data?.error === "invalid_email" ? "Please enter a valid email address." : "Failed to send code. Try again.",
                    })
                  }
                  doRender()
                })
                .catch(() => {
                  isBusy = false
                  messages.push({ role: "assistant", text: "Failed to send code. Try again." })
                  doRender()
                })
              return
            }

            // step === "code"
            messages.push({ role: "user", text: raw ? "••••••" : "(empty)" })
            if (!raw) {
              messages.push({ role: "assistant", text: "Please enter the 6-digit code from your email." })
              doRender()
              return
            }
            if (!isSixDigitCode(raw)) {
              messages.push({ role: "assistant", text: "Please enter the 6-digit code from your email." })
              doRender()
              return
            }
            const code = raw.replace(/\s+/g, "")
            isBusy = true
            messages.push({ role: "assistant", text: "Verifying…" })
            doRender()
            fetch("/api/auth/verify", {
              method: "POST",
              headers: { "content-type": "application/json" },
              credentials: "include",
              body: JSON.stringify({ email, code }),
            })
              .then((r) => r.json().catch(() => null))
              .then(async (data: { ok?: boolean; error?: string }) => {
                isBusy = false
                if (!data?.ok) {
                  messages.push({
                    role: "assistant",
                    text: data?.error === "invalid_code" ? "Invalid code. Please try again." : "Verification failed. Try again.",
                  })
                  doRender()
                  return
                }
                clearAuthClientCache()
                if (!deps) {
                  messages.push({ role: "assistant", text: "You're signed in." })
                  doRender()
                  return
                }
                const sessionExit = await deps.runtime.runPromiseExit(
                  Effect.flatMap(AuthService, (auth) => auth.getSession()),
                )
                if (sessionExit._tag === "Success") {
                  const session = sessionExit.value
                  deps.atoms.set(SessionAtom as any, {
                    userId: session.userId,
                    user: session.user
                      ? {
                          id: session.user.id,
                          email: session.user.email,
                          firstName: session.user.firstName,
                          lastName: session.user.lastName,
                        }
                      : null,
                  })
                }
                messages.push({ role: "assistant", text: "You're signed in." })
                if (deps && !identityPaneAdded) {
                  identityPaneAdded = true
                  paneSystem.store.addPane({
                    id: IDENTITY_PANE_ID,
                    kind: "generic",
                    title: "",
                    rect: IDENTITY_PANE_RECT,
                    dismissable: false,
                  })
                  paneSystem.store.bringToFront(CHAT_PANE_ID)
                  paneSystem.render()
                  const newSession = deps.atoms.get(SessionAtom as any)
                  if (newSession?.user?.email) renderIdentityPaneContent(newSession.user.email)
                }
                messages.length = 0
                messages.push({ role: "assistant", text: "Autopilot online. Awaiting instructions." })
                step = "authed"
                doRender()
              })
              .catch(() => {
                isBusy = false
                messages.push({ role: "assistant", text: "Verification failed. Try again." })
                doRender()
              })
          })
          if (step !== "authed") {
            const inputEl = paneContentSlot.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
            if (inputEl) requestAnimationFrame(() => inputEl.focus())
          }
        },
        () => {},
      )
    }

    doRender()
  }

  trigger.addEventListener("click", handler, { capture: true })
  return () => trigger.removeEventListener("click", handler, { capture: true })
}

export const mountHomeController = (input: {
  readonly container: Element
  readonly runtime?: AppRuntime
  readonly atoms?: AtomRegistry
  readonly navigate?: (href: string) => void
  readonly signOut?: () => void | Promise<void>
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})

  const stopCountdown = startPrelaunchCountdownTicker(input.container)
  const deps: HomeChatDeps | undefined =
    input.runtime && input.atoms && input.navigate && input.signOut
      ? {
          runtime: input.runtime,
          atoms: input.atoms,
          navigate: input.navigate,
          signOut: input.signOut,
        }
      : undefined
  const stopOpenChatPane = openChatPaneOnHome(input.container, deps)

  return {
    cleanup: () => {
      stopCountdown()
      stopOpenChatPane()
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}
