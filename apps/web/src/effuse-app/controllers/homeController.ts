import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html, renderToolPart } from "@openagentsinc/effuse"
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  DEFAULT_PANE_SYSTEM_THEME,
} from "@openagentsinc/effuse-panes"

import {
  AuthSession,
  AuthSessionUser,
  clearAuthClientCache,
  setClientAuthFromVerify,
} from "../../effect/auth"
import type { ChatSnapshot } from "../../effect/chat"
import { ChatSnapshotAtom } from "../../effect/atoms/chat"
import { SessionAtom, type Session } from "../../effect/atoms/session"
import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"
import {
  renderDseBudgetExceededCard,
  renderDseCompileCard,
  renderDsePromoteCard,
  renderDseRollbackCard,
  renderDseSignatureCard,
} from "../../effuse-pages/autopilot"
import { streamdown } from "../../lib/effuseStreamdown"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { ChatClient } from "../../effect/chat"
import type { AppRuntime } from "../../effect/runtime"
import { toAutopilotRenderParts } from "./autopilotChatParts"

export type HomeController = {
  readonly cleanup: () => void
}

type HomeChatDeps = {
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly navigate: (href: string) => void
  readonly signOut: () => void | Promise<void>
  readonly chat: ChatClient
  readonly refreshConvexAuth?: () => void | Promise<void>
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

    const sessionFromAtoms: Session =
      (deps?.atoms?.get(SessionAtom as any) as Session) ?? { userId: null, user: null }
    const isAuthedFromAtoms = sessionFromAtoms.user != null

    const renderIdentityCard = (userEmail: string) => {
      const existing = overlay.querySelector("[data-oa-home-identity-card]")
      let cardEl: HTMLElement | null = existing instanceof HTMLElement ? existing : null
      if (!cardEl) {
        const next = document.createElement("div")
        next.setAttribute("data-oa-home-identity-card", "1")
        next.style.cssText = "position:fixed;top:12px;left:12px;z-index:10000;pointer-events:auto;"
        overlay.appendChild(next)
        cardEl = next
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
        const btn = cardEl.querySelector("[data-oa-home-identity-logout]")
        if (btn) {
          btn.addEventListener("click", () => {
            void Promise.resolve(deps?.signOut?.()).then(() => closeOverlay())
          })
        }
      }, () => { })
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
    /** Shown when authed but thread not loaded yet; must match Convex FIRST_OPEN_WELCOME_MESSAGE so onboarding isn't skipped. */
    const ONBOARDING_FIRST_MESSAGE = "Autopilot online.\n\nGreetings, user. What shall I call you?"
    const messages: Array<{ role: "user" | "assistant"; text: string }> = isAuthedFromAtoms
      ? [{ role: "assistant", text: ONBOARDING_FIRST_MESSAGE }]
      : [{ role: "assistant", text: "Autopilot initialized. Enter your email address to begin." }]
    let step: Step = isAuthedFromAtoms ? "authed" : "email"
    let email = ""
    let isBusy = false
    let homeThreadId: string | null = null
    let homeSnapshot: ChatSnapshot = { messages: [], status: "ready", errorText: null }
    let unsubHomeChat: (() => void) | null = null
    let dseStrategyId: "direct.v1" | "rlm_lite.v1" = "direct.v1"
    let dseBudgetProfile: "small" | "medium" | "long" = "medium"
    let isRunningDseRecap = false
    let dseErrorText: string | null = null
    let hasScrolledToBottomOnce = false

    const startAuthedChat = (input0: { readonly userId: string; readonly user: Session["user"] | null; readonly token: string | null }) => {
      if (!deps?.atoms || !deps.chat) return

      deps.atoms.set(SessionAtom as any, { userId: input0.userId, user: input0.user })

      // Prime the in-memory auth token cache so Convex can authenticate immediately without waiting
      // for cookie timing (especially important in tests and right after verify/login).
      if (input0.token && input0.user && input0.user.email) {
        try {
          const user = AuthSessionUser.make({
            id: input0.user.id,
            email: input0.user.email ?? null,
            firstName: input0.user.firstName ?? null,
            lastName: input0.user.lastName ?? null,
          })
          const session = AuthSession.make({ userId: input0.userId, sessionId: null, user })
          clearAuthClientCache()
          setClientAuthFromVerify(session, input0.token)
        } catch {
          // ignore
        }
      }

      void Promise.resolve(deps.refreshConvexAuth?.()).catch(() => { })

      if (input0.user?.email) renderIdentityCard(input0.user.email)

      messages.length = 0
      messages.push({ role: "assistant", text: ONBOARDING_FIRST_MESSAGE })
      step = "authed"
      doRender()

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
          doRender()
        },
        () => doRender(),
      )
    }

    const chatInputClass =
      "w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"

    const textFromRenderParts = (parts: ReadonlyArray<{ kind?: string; text?: string }>): string => {
      if (!parts?.length) return ""
      return parts
        .filter((p) => p?.kind === "text" && typeof (p as any).text === "string")
        .map((p) => String((p as any).text ?? ""))
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

      const renderedMessages =
        step === "authed" && homeSnapshot.messages.length > 0
          ? homeSnapshot.messages
            .filter((m) => m && typeof m === "object")
            .filter((m) => String((m as any).role ?? "") === "user" || String((m as any).role ?? "") === "assistant")
            .map((m, i) => {
              const partsRaw = Array.isArray((m as any).parts) ? ((m as any).parts as ReadonlyArray<any>) : []
              let renderParts = toAutopilotRenderParts({ parts: partsRaw, toolContractsByName: null })

              // If the run is streaming but no text has arrived yet, show a stable placeholder to avoid a blank bubble.
              if (
                String((m as any).role) === "assistant" &&
                i === lastAssistantIndex &&
                homeSnapshot.status === "streaming" &&
                renderParts.length === 0
              ) {
                renderParts = [{ kind: "text" as const, text: "...", state: "streaming" as const }]
              }

              return {
                id: String((m as any).id ?? ""),
                role: String((m as any).role ?? "") as "user" | "assistant",
                renderParts,
              }
            })
          : null

      const lastAssistantText =
        step === "authed" && renderedMessages
          ? (() => {
            for (let i = renderedMessages.length - 1; i >= 0; i--) {
              if (renderedMessages[i]?.role === "assistant") {
                return textFromRenderParts(renderedMessages[i].renderParts as any)
              }
            }
            return ""
          })()
          : ""
      const authedPlaceholder = chatPlaceholderFromLastAssistant(lastAssistantText)

      const isLocalhost =
        typeof window !== "undefined" &&
        (window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1")
      const controlsHtml =
        step === "authed" && isLocalhost
          ? html`
              <div
                data-oa-home-chat-controls="1"
                class="flex items-center justify-between gap-2 px-3 py-2 border-b border-white/10 bg-black/40"
              >
                <div class="text-[11px] font-mono text-white/50 truncate">
                  ${homeThreadId ? `thread: ${homeThreadId}` : "thread: (loading...)"}
                </div>
                <div class="flex items-center gap-2">
                  <select
                    data-oa-home-dse-strategy="1"
                    class="h-8 rounded border border-white/15 bg-black/40 px-2 text-[11px] font-mono text-white/80"
                    ${isRunningDseRecap ? "disabled" : ""}
                  >
                    <option value="direct.v1" ${dseStrategyId === "direct.v1" ? "selected" : ""}>direct.v1</option>
                    <option value="rlm_lite.v1" ${dseStrategyId === "rlm_lite.v1" ? "selected" : ""}>rlm_lite.v1</option>
                  </select>
                  <select
                    data-oa-home-dse-budget="1"
                    class="h-8 rounded border border-white/15 bg-black/40 px-2 text-[11px] font-mono text-white/80"
                    ${isRunningDseRecap ? "disabled" : ""}
                  >
                    <option value="small" ${dseBudgetProfile === "small" ? "selected" : ""}>small</option>
                    <option value="medium" ${dseBudgetProfile === "medium" ? "selected" : ""}>medium</option>
                    <option value="long" ${dseBudgetProfile === "long" ? "selected" : ""}>long</option>
                  </select>
                  <button
                    type="button"
                    data-oa-home-dse-recap="1"
                    class="h-8 rounded border border-white/15 bg-white/10 px-2 text-[11px] font-mono text-white/80 hover:bg-white/20 disabled:opacity-60"
                    ${isRunningDseRecap || !homeThreadId ? "disabled" : ""}
                  >
                    ${isRunningDseRecap ? "Running..." : "Run recap"}
                  </button>
                </div>
              </div>
            `
          : null

      const errorHtml =
        step === "authed" && (homeSnapshot.errorText || dseErrorText)
          ? html`
              <div
                data-oa-home-chat-error="1"
                class="mx-4 mt-3 rounded border border-red-400/40 bg-red-500/10 px-3 py-2 text-xs text-red-300"
              >
                <div class="whitespace-pre-wrap">${homeSnapshot.errorText ?? dseErrorText ?? ""}</div>
              </div>
            `
          : null

      const formHtml =
        step === "authed"
          ? html`
              <form data-oa-home-chat-form="1" data-oa-home-chat-step="authed" class="p-2 border-t border-white/10">
                <div class="flex items-center gap-2">
                  <input
                    type="text"
                    name="message"
                    placeholder="${authedPlaceholder}"
                    autocomplete="off"
                    class="${chatInputClass}"
                    data-oa-home-chat-input="1"
                  />
                  ${homeSnapshot.status === "submitted" || homeSnapshot.status === "streaming"
              ? html`<button
                        type="button"
                        data-oa-home-chat-stop="1"
                        class="h-9 rounded border border-white/15 bg-white/5 px-3 text-xs font-mono text-white/70 hover:bg-white/10"
                      >
                        Stop
                      </button>`
              : html`<button
                        type="submit"
                        data-oa-home-chat-send="1"
                        class="h-9 rounded border border-white/15 bg-white/10 px-3 text-xs font-mono text-white/80 hover:bg-white/20"
                      >
                        Send
                      </button>`}
                </div>
              </form>
            `
          : step === "email"
            ? html`
                <form data-oa-home-chat-form="1" data-oa-home-chat-step="email" class="p-2 border-t border-white/10">
                  <div class="flex items-center gap-2">
                    <input
                      type="text"
                      name="email"
                      placeholder="your@email.com"
                      autocomplete="email"
                      class="${chatInputClass}"
                      data-oa-home-chat-input="1"
                    />
                    <button
                      type="submit"
                      data-oa-home-chat-send="1"
                      class="h-9 rounded border border-white/15 bg-white/10 px-3 text-xs font-mono text-white/80 hover:bg-white/20"
                    >
                      Continue
                    </button>
                  </div>
                </form>
              `
            : html`
                <form data-oa-home-chat-form="1" data-oa-home-chat-step="code" class="p-2 border-t border-white/10">
                  <div class="flex items-center gap-2">
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
                    <button
                      type="submit"
                      data-oa-home-chat-send="1"
                      class="h-9 rounded border border-white/15 bg-white/10 px-3 text-xs font-mono text-white/80 hover:bg-white/20"
                    >
                      Verify
                    </button>
                  </div>
                </form>
              `

      const messagesHtml =
        renderedMessages
          ? html`
              <div data-oa-home-chat-messages="1" class="flex flex-col gap-2 overflow-y-auto flex-1 min-h-0 p-4">
                ${renderedMessages.map((m) => {
            if (m.role === "user") {
              const userText = textFromRenderParts(m.renderParts as any)
              return html`<div
                      class="text-sm font-mono text-white/55 text-left max-w-[80%] self-end"
                      data-chat-role="user"
                    >
                      ${userText}
                    </div>`
            }

            const partEls = (m.renderParts as any[]).map((p) => {
              if (p.kind === "text") {
                return streamdown(p.text, {
                  mode: "streaming",
                  isAnimating: p.state === "streaming",
                  caret: "block",
                })
              }
              if (p.kind === "tool") return renderToolPart(p.model)
              if (p.kind === "dse-signature") return renderDseSignatureCard(p.model)
              if (p.kind === "dse-compile") return renderDseCompileCard(p.model)
              if (p.kind === "dse-promote") return renderDsePromoteCard(p.model)
              if (p.kind === "dse-rollback") return renderDseRollbackCard(p.model)
              if (p.kind === "dse-budget-exceeded") return renderDseBudgetExceededCard(p.model)
              return html``
            })

            return html`<div class="text-sm font-mono text-white/90" data-chat-role="assistant">
                    <div class="flex flex-col gap-2">${partEls}</div>
                  </div>`
          })}
              </div>
            `
          : html`
              <div data-oa-home-chat-messages="1" class="flex flex-col gap-2 overflow-y-auto flex-1 min-h-0 p-4">
                ${(step === "authed" ? [{ role: "assistant" as const, text: ONBOARDING_FIRST_MESSAGE }] : messages).map((m) =>
            m.role === "user"
              ? html`<div
                        class="text-sm font-mono text-white/55 text-left max-w-[80%] self-end"
                        data-chat-role="user"
                      >
                        ${m.text}
                      </div>`
              : html`<div class="text-sm font-mono text-white/90" data-chat-role="assistant">
                        ${streamdown(m.text, { mode: "static" })}
                      </div>`,
          )}
              </div>
            `

      const messagesContainer = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
      const savedScrollTop = messagesContainer instanceof HTMLElement ? messagesContainer.scrollTop : 0

      Effect.runPromise(
        Effect.gen(function* () {
          const dom = yield* DomServiceTag
          yield* dom.render(
            paneContentSlot,
            html`
              <div
                class="flex flex-col h-full min-h-0"
                data-oa-home-chat-root="1"
                data-oa-home-chat-status="${homeSnapshot.status}"
              >
                ${controlsHtml}
                ${errorHtml}
                ${messagesHtml}
                ${formHtml}
              </div>
            `,
          )
        }).pipe(Effect.provide(EffuseLive)),
      ).then(
        () => {
          const messagesEl = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
          if (messagesEl instanceof HTMLElement) {
            messagesEl.scrollTop = savedScrollTop
            if (!hasScrolledToBottomOnce) {
              messagesEl.scrollTop = messagesEl.scrollHeight - messagesEl.clientHeight
              hasScrolledToBottomOnce = true
            }
          }

          const strategySel = paneContentSlot.querySelector("[data-oa-home-dse-strategy]")
          if (strategySel instanceof HTMLSelectElement) {
            strategySel.addEventListener("change", () => {
              const v = String(strategySel.value ?? "direct.v1")
              dseStrategyId = v === "rlm_lite.v1" ? "rlm_lite.v1" : "direct.v1"
            })
          }

          const budgetSel = paneContentSlot.querySelector("[data-oa-home-dse-budget]")
          if (budgetSel instanceof HTMLSelectElement) {
            budgetSel.addEventListener("change", () => {
              const v = String(budgetSel.value ?? "medium")
              dseBudgetProfile = v === "small" ? "small" : v === "long" ? "long" : "medium"
            })
          }

          const recapBtn = paneContentSlot.querySelector("[data-oa-home-dse-recap]")
          if (recapBtn instanceof HTMLButtonElement) {
            recapBtn.addEventListener("click", () => {
              const tid = homeThreadId
              if (!tid) return
              if (isRunningDseRecap) return
              isRunningDseRecap = true
              dseErrorText = null
              doRender()
              fetch("/api/autopilot/dse/recap", {
                method: "POST",
                headers: {
                  "content-type": "application/json",
                  ...(String((globalThis as any).__OA_E2E_MODE ?? "") === "stub"
                    ? { "x-oa-e2e-mode": "stub" }
                    : {}),
                },
                credentials: "include",
                cache: "no-store",
                body: JSON.stringify({
                  threadId: tid,
                  strategyId: dseStrategyId,
                  budgetProfile: dseBudgetProfile,
                  question: "Recap this thread.",
                }),
              })
                .then((r) => r.json().catch(() => null) as Promise<{ ok?: boolean; error?: string } | null>)
                .then((data) => {
                  isRunningDseRecap = false
                  if (!data?.ok) {
                    dseErrorText = data?.error ? `DSE recap failed: ${data.error}` : "DSE recap failed."
                  }
                  doRender()
                })
                .catch(() => {
                  isRunningDseRecap = false
                  dseErrorText = "DSE recap failed."
                  doRender()
                })
            })
          }

          const stopBtn = paneContentSlot.querySelector("[data-oa-home-chat-stop]")
          if (stopBtn instanceof HTMLButtonElement) {
            stopBtn.addEventListener("click", () => {
              const tid = homeThreadId
              if (!tid || !deps?.chat) return
              deps.runtime.runPromise(deps.chat.stop(tid)).catch(() => { })
            })
          }

          const authedForm = paneContentSlot.querySelector("[data-oa-home-chat-form][data-oa-home-chat-step=\"authed\"]")
          const messageInput = authedForm?.querySelector("[data-oa-home-chat-input]")
          if (messageInput instanceof HTMLInputElement) {
            let scrollTopToRestore: number | null = null
            messageInput.addEventListener("mousedown", () => {
              const el = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
              if (el instanceof HTMLElement) scrollTopToRestore = el.scrollTop
            }, { capture: true })
            messageInput.addEventListener("focus", () => {
              if (scrollTopToRestore === null) return
              const el = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
              if (el instanceof HTMLElement) {
                const saved = scrollTopToRestore
                scrollTopToRestore = null
                requestAnimationFrame(() => {
                  el.scrollTop = saved
                })
              }
            })
          }

          const form = paneContentSlot.querySelector("[data-oa-home-chat-form]")
          if (!(form instanceof HTMLFormElement)) return
          form.addEventListener("submit", (e: Event) => {
            e.preventDefault()
            const input0 = form.querySelector("[data-oa-home-chat-input]")
            const input = input0 instanceof HTMLInputElement ? input0 : null
            const raw = input?.value?.trim() ?? ""

            if (step === "authed") {
              if (homeSnapshot.status === "submitted" || homeSnapshot.status === "streaming") return
              if (!raw || !deps?.chat) return
              const text = raw
              const ensureThenSend = () => {
                const tid = homeThreadId
                if (tid) {
                  if (input) input.value = ""
                  deps.runtime.runPromise(deps.chat.send(tid, text)).catch(() => { })
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
                      deps.runtime.runPromise(deps.chat.send(id, text)).catch(() => { })
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
                .then((r) => r.json().catch(() => null) as Promise<{ ok?: boolean; error?: string } | null>)
                .then((data) => {
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
              .then((r) =>
                r.json().catch(() => null) as Promise<{
                  ok?: boolean
                  error?: string
                  userId?: string
                  token?: string
                  user?: {
                    id: string
                    email?: string | null
                    firstName?: string | null
                    lastName?: string | null
                  }
                } | null>,
              )
              .then(async (data) => {
                isBusy = false
                if (!data?.ok) {
                  messages.push({
                    role: "assistant",
                    text:
                      data?.error === "invalid_code"
                        ? "Invalid code. Please try again."
                        : "Verification failed. Try again.",
                  })
                  doRender()
                  return
                }

                clearAuthClientCache()
                const token = typeof data.token === "string" ? data.token : null
                const userPayload = data.user && typeof data.user.id === "string" ? data.user : null
                const userId = typeof data.userId === "string" ? data.userId : userPayload?.id ?? null

                if (token && userPayload && userId && deps) {
                  const user = AuthSessionUser.make({
                    id: userPayload.id,
                    email: userPayload.email ?? null,
                    firstName: userPayload.firstName ?? null,
                    lastName: userPayload.lastName ?? null,
                  })
                  const session = AuthSession.make({ userId, sessionId: null, user })
                  setClientAuthFromVerify(session, token)
                  deps.atoms.set(SessionAtom as any, {
                    userId,
                    user: {
                      id: userPayload.id,
                      email: userPayload.email ?? null,
                      firstName: userPayload.firstName ?? null,
                      lastName: userPayload.lastName ?? null,
                    },
                  })

                  try {
                    await deps.refreshConvexAuth?.()
                  } catch {
                    // ignore
                  }

                  if (userPayload.email) renderIdentityCard(userPayload.email)
                  messages.length = 0
                  messages.push({ role: "assistant", text: ONBOARDING_FIRST_MESSAGE })
                  step = "authed"

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
                      messages.push({ role: "assistant", text: ONBOARDING_FIRST_MESSAGE })
                      step = "authed"
                      doRender()
                    },
                    () => {
                      messages.length = 0
                      messages.push({ role: "assistant", text: ONBOARDING_FIRST_MESSAGE })
                      step = "authed"
                      doRender()
                    },
                  )

                  doRender()
                  return
                }

                messages.push({ role: "assistant", text: "Signed in, but couldn't initialize chat. Please try again." })
                step = "authed"
                doRender()
              })
              .catch(() => {
                isBusy = false
                messages.push({ role: "assistant", text: "Verification failed. Try again." })
                doRender()
              })
          })
          const inputEl0 = paneContentSlot.querySelector("[data-oa-home-chat-input]")
          const inputEl = inputEl0 instanceof HTMLInputElement ? inputEl0 : null
          if (inputEl) requestAnimationFrame(() => inputEl.focus())
        },
        () => { },
      )
    }

    if (isAuthedFromAtoms && deps?.atoms) {
      const current = deps.atoms.get(SessionAtom as any) as Session
      startAuthedChat({ userId: current.userId ?? "", user: current.user ?? null, token: null })
    } else if (!isAuthedFromAtoms && deps?.atoms) {
      // If the user already has a valid session cookie (e.g. E2E bypass login), detect it
      // so the home overlay doesn't force re-entering email.
      fetch("/api/auth/session", { method: "GET", cache: "no-store", credentials: "include" })
        .then((r) => r.json().catch(() => null) as Promise<any>)
        .then((data) => {
          if (!data || data.ok !== true) return
          const userId = typeof data.userId === "string" ? data.userId : null
          const token = typeof data.token === "string" && data.token.length > 0 ? data.token : null
          const user =
            data.user && typeof data.user.id === "string"
              ? {
                id: String(data.user.id),
                email: data.user.email ?? null,
                firstName: data.user.firstName ?? null,
                lastName: data.user.lastName ?? null,
              }
              : null
          if (!userId) return
          startAuthedChat({ userId, user, token })
        })
        .catch(() => { })
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
  readonly refreshConvexAuth?: () => void | Promise<void>
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => { })

  const stopCountdown = startPrelaunchCountdownTicker(input.container)
  const deps: HomeChatDeps | undefined =
    input.runtime && input.atoms && input.navigate && input.signOut && input.chat
      ? {
        runtime: input.runtime,
        atoms: input.atoms,
        navigate: input.navigate,
        signOut: input.signOut,
        chat: input.chat,
        refreshConvexAuth: input.refreshConvexAuth,
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
