import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  DEFAULT_PANE_SYSTEM_THEME,
} from "@openagentsinc/effuse-panes"

import { AuthService, clearAuthClientCache } from "../../effect/auth"
import type { ChatSnapshot } from "../../effect/chat"
import { ChatSnapshotAtom } from "../../effect/atoms/chat"
import { SessionAtom } from "../../effect/atoms/session"
import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { ChatClient } from "../../effect/chat"
import type { AppRuntime } from "../../effect/runtime"

export type HomeController = {
  readonly cleanup: () => void
}

type HomeChatDeps = {
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly navigate: (href: string) => void
  readonly signOut: () => void | Promise<void>
  readonly chat: ChatClient
}

const CHAT_PANE_ID = "home-chat"

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
      "[data-oa-home-chat-overlay] [data-oa-pane-system][data-oa-pane-dragging=\"1\"] [data-oa-pane-title] { cursor: grabbing; }"
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

    const renderIdentityCard = (userEmail: string) => {
      let cardEl = overlay.querySelector("[data-oa-home-identity-card]")
      if (!cardEl) {
        cardEl = document.createElement("div")
        cardEl.setAttribute("data-oa-home-identity-card", "1")
        cardEl.style.cssText = "position:fixed;top:12px;left:12px;z-index:10000;pointer-events:auto;"
        overlay.appendChild(cardEl)
      }
      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            cardEl as Element,
            html`
              <div class="flex items-center gap-2 rounded-lg border border-white/10 bg-black/80 px-3 py-2 text-xs font-mono text-white/90 shadow-lg backdrop-blur-sm">
                <span class="truncate max-w-[180px]" title="${userEmail}">${userEmail}</span>
                <button
                  type="button"
                  data-oa-home-identity-logout="1"
                  class="shrink-0 rounded px-1.5 py-0.5 text-white/60 hover:bg-white/10 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
                >
                  Log out
                </button>
              </div>
            `,
          )
        }).pipe(Effect.provide(EffuseLive)),
      ).then(() => {
        const btn = (cardEl as Element).querySelector("[data-oa-home-identity-logout]")
        if (btn) {
          btn.addEventListener("click", () => {
            void Promise.resolve(deps?.signOut?.()).then(() => closeOverlay())
          })
        }
      }, () => {})
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
    let homeThreadId: string | null = null
    let homeSnapshot: ChatSnapshot = { messages: [], status: "ready", errorText: null }
    let unsubHomeChat: (() => void) | null = null

    if (isAuthed && deps?.atoms) {
      const currentSession = deps.atoms.get(SessionAtom as any)
      if (currentSession?.user?.email) renderIdentityCard(currentSession.user.email)
      if (deps.chat) {
        deps.runtime.runPromise(deps.chat.getOwnedThreadId()).then(
          (id) => {
            if (id && id.length > 0) {
              homeThreadId = id
              deps.atoms.get(ChatSnapshotAtom(id))
              unsubHomeChat = deps.atoms.subscribe(
                ChatSnapshotAtom(id),
                (snap) => {
                  homeSnapshot = snap
                  doRender()
                },
                { immediate: true },
              )
              doRender()
            }
          },
          () => {},
        )
      }
    }

    const chatInputClass =
      "w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"

    const textFromParts = (parts: ReadonlyArray<{ type?: string; text?: string }>): string => {
      if (!parts?.length) return ""
      return parts
        .filter((p) => p?.type === "text" && typeof (p as any).text === "string")
        .map((p) => (p as any).text)
        .join("")
    }

    /** Placeholder for chat input based on last assistant message (onboarding hints). */
    const chatPlaceholderFromLastAssistant = (
      lastAssistantText: string,
    ): string => {
      const t = lastAssistantText.toLowerCase()
      if (t.includes("what shall i call you")) return "Enter your name or handle"
      if (t.includes("what should you call me")) return "Enter agent name (e.g. Autopilot)"
      if (t.includes("operating vibe") || t.includes("pick one short")) return "Enter a short vibe (e.g. calm, direct)"
      if (t.includes("boundaries or preferences") || t.includes("reply 'none' or list")) return "Reply 'none' or list a few bullets"
      return "Type a message..."
    }

    const doRender = () => {
      const lastAssistantIndex =
        step === "authed" && homeSnapshot.messages.length > 0
          ? (() => {
              for (let i = homeSnapshot.messages.length - 1; i >= 0; i--) {
                if (homeSnapshot.messages[i]?.role === "assistant") return i
              }
              return -1
            })()
          : -1

      const displayMessages: Array<{ role: "user" | "assistant"; text: string }> =
        step === "authed" && homeSnapshot.messages.length > 0
          ? homeSnapshot.messages.map((m, i) => {
              const raw = textFromParts(m.parts ?? [])
              const isLastAssistant = m.role === "assistant" && i === lastAssistantIndex
              const fallback =
                m.role === "assistant"
                  ? homeSnapshot.status === "streaming" && isLastAssistant && !raw
                    ? "..."
                    : "(no text)"
                  : ""
              return {
                role: m.role as "user" | "assistant",
                text: raw || (m.role === "assistant" ? fallback : ""),
              }
            })
          : step === "authed"
            ? [{ role: "assistant" as const, text: "Autopilot online. Awaiting instructions." }]
            : messages

      const lastAssistantText =
        step === "authed"
          ? (() => {
              for (let i = displayMessages.length - 1; i >= 0; i--) {
                if (displayMessages[i]?.role === "assistant") return displayMessages[i].text
              }
              return ""
            })()
          : ""
      const authedPlaceholder = chatPlaceholderFromLastAssistant(lastAssistantText)

      const formHtml =
        step === "authed"
          ? html`
              <form data-oa-home-chat-form="1" data-oa-home-chat-step="authed" class="p-2 border-t border-white/10">
                <input
                  type="text"
                  name="message"
                  placeholder="${authedPlaceholder}"
                  class="${chatInputClass}"
                  data-oa-home-chat-input="1"
                />
              </form>
            `
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
          ${displayMessages.map(
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
            const input = form.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
            const raw = input?.value?.trim() ?? ""

            if (step === "authed") {
              if (homeSnapshot.status === "submitted" || homeSnapshot.status === "streaming") return
              if (!raw || !deps?.chat) return
              const text = raw
              const ensureThenSend = () => {
                let tid = homeThreadId
                if (tid) {
                  if (input) input.value = ""
                  deps.runtime.runPromise(deps.chat.send(tid, text)).catch(() => {})
                  doRender()
                  return
                }
                deps.runtime.runPromise(deps.chat.getOwnedThreadId()).then(
                  (id) => {
                    if (id && id.length > 0) {
                      homeThreadId = id
                      deps.atoms.get(ChatSnapshotAtom(id))
                      if (unsubHomeChat) unsubHomeChat()
                      unsubHomeChat = deps.atoms.subscribe(
                        ChatSnapshotAtom(id),
                        (snap) => {
                          homeSnapshot = snap
                          doRender()
                        },
                        { immediate: true },
                      )
                      if (input) input.value = ""
                      deps.runtime.runPromise(deps.chat.send(id, text)).catch(() => {})
                    }
                    doRender()
                  },
                  () => doRender(),
                )
              }
              ensureThenSend()
              return
            }

            if (isBusy) return
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
                if (deps) {
                  const newSession = deps.atoms.get(SessionAtom as any)
                  if (newSession?.user?.email) renderIdentityCard(newSession.user.email)
                  if (deps.chat) {
                    deps.runtime.runPromise(deps.chat.getOwnedThreadId()).then(
                      (id) => {
                        if (id && id.length > 0) {
                          homeThreadId = id
                          deps.atoms.get(ChatSnapshotAtom(id))
                          if (unsubHomeChat) unsubHomeChat()
                          unsubHomeChat = deps.atoms.subscribe(
                            ChatSnapshotAtom(id),
                            (snap) => {
                              homeSnapshot = snap
                              doRender()
                            },
                            { immediate: true },
                          )
                        }
                        messages.length = 0
                        messages.push({ role: "assistant", text: "Autopilot online. Awaiting instructions." })
                        step = "authed"
                        doRender()
                      },
                      () => {
                        messages.length = 0
                        messages.push({ role: "assistant", text: "Autopilot online. Awaiting instructions." })
                        step = "authed"
                        doRender()
                      },
                    )
                    return
                  }
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
          const inputEl = paneContentSlot.querySelector<HTMLInputElement>("[data-oa-home-chat-input]")
          if (inputEl) requestAnimationFrame(() => inputEl.focus())
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
  readonly chat?: ChatClient
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})

  const stopCountdown = startPrelaunchCountdownTicker(input.container)
  const deps: HomeChatDeps | undefined =
    input.runtime && input.atoms && input.navigate && input.signOut && input.chat
      ? {
          runtime: input.runtime,
          atoms: input.atoms,
          navigate: input.navigate,
          signOut: input.signOut,
          chat: input.chat,
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
