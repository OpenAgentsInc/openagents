import { Cause, Effect, Exit, Fiber } from "effect"
import { DomServiceTag, EffuseLive, html, rawHtml, renderToolPart } from "@openagentsinc/effuse"
import {
  calculateNewPanePosition,
  DEFAULT_PANE_SYSTEM_THEME,
} from "@openagentsinc/effuse-panes"

import {
  AuthSession,
  AuthSessionUser,
  clearAuthClientCache,
  setClientAuthFromVerify,
} from "../../../effect/auth"
import type { ChatSnapshot } from "../../../effect/chat"
import { ChatSnapshotAtom } from "../../../effect/atoms/chat"
import type { Session } from "../../../effect/atoms/session"
import { HomeApiService } from "../../../effect/homeApi"
import {
  LightningApiService,
  type LightningGatewayDeployment,
  type LightningGatewayEvent,
  type LightningPaywall,
  type LightningSettlement,
} from "../../../effect/lightning"
import { PaneSystemLive, PaneSystemService } from "../../../effect/paneSystem"
import {
  renderDseBudgetExceededCard,
  renderDseCompileCard,
  renderPaymentStateCard,
  renderDsePromoteCard,
  renderDseRollbackCard,
  renderDseSignatureCard,
  type RenderPart,
} from "../../../effuse-pages/autopilot"
import { streamdown } from "../../../lib/effuseStreamdown"

import {
  homeApiRejectedReason,
  isSixDigitCode,
  looksLikeEmail,
  normalizeEmail,
  startCodeErrorMessage,
  verifyCodeErrorMessage,
} from "./authFlow"
import {
  clearCachedSnapshotForUser,
  readCachedSnapshotForUser,
  shouldSkipHydratedPlaceholder,
  writeCachedSnapshotForUser,
} from "./chatSession"
import {
  hasAnyHostedOpsPaneOpen,
  l402PaneRenderBranch,
  makeInitialL402PaneState,
  paneButtonVisualState,
  rejectL402PaneState,
  resolveL402PaneState,
  startL402PaneLoading,
  type L402PaneState,
} from "./l402OpsPaneState"
import {
  clampPaneRectToScreen,
  parseStoredPaneRect,
  readStoredPaneRect,
  writeStoredPaneRect,
} from "./overlayLifecycle"
import {
  BOLT_ICON_SVG,
  BUG_ICON_SVG,
  CHART_ICON_SVG,
  CHECKMARK_ICON_SVG,
  copyTextToClipboard,
  COPY_ICON_SVG,
  METADATA_ICON_SVG,
  TRANSACTIONS_ICON_SVG,
  WALLET_ICON_SVG,
} from "./renderWiring"
import type { HomeChatDeps } from "./types"
import { extractL402PaymentMetadata, type L402PaymentMetadata, toAutopilotRenderParts } from "../autopilotChatParts"

const CHAT_PANE_ID = "home-chat"
const L402_WALLET_PANE_ID = "l402-wallet"
const L402_TRANSACTIONS_PANE_ID = "l402-transactions"
const L402_PAYWALLS_PANE_ID = "l402-paywalls"
const L402_SETTLEMENTS_PANE_ID = "l402-settlements"
const L402_DEPLOYMENTS_PANE_ID = "l402-deployments"
const HOME_CHAT_PANE_RECT_STORAGE_KEY = "oa.home.chat.paneRect.v1"

const toStructuredError = (error: unknown): unknown => {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    }
  }
  return error
}

const logHomeControllerAsyncError = (context: string, error: unknown): void => {
  console.error("[homeController] async_failure", {
    context,
    error: toStructuredError(error),
  })
}

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null

export function openChatPaneOnHome(container: Element, deps: HomeChatDeps | undefined): () => void {
  const trigger = container.querySelector("[data-oa-open-chat-pane]")
  if (!trigger) return () => { }
  let activeOverlayTeardown: (() => void) | null = null

  const isTextEntryTarget = (target: EventTarget | null): boolean => {
    if (!(target instanceof HTMLElement)) return false
    if (target.isContentEditable) return true
    const tag = target.tagName
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || tag === "BUTTON") return true
    return target.closest("input, textarea, select, button, [contenteditable=\"true\"]") != null
  }

  const handler = (ev: Event): void => {
    ev.preventDefault()
    ev.stopPropagation()

    const shell = container.querySelector("[data-marketing-shell]")
    if (!(shell instanceof HTMLElement)) return
    activeOverlayTeardown?.()

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
      "[data-oa-home-chat-overlay] [data-pane-id^=\"telemetry-\"] { background: #000 !important; opacity: 1 !important; }\n" +
      "[data-oa-home-chat-overlay] [data-pane-id^=\"telemetry-\"] [data-oa-pane-title], [data-oa-home-chat-overlay] [data-pane-id^=\"telemetry-\"] [data-oa-pane-content] { background: #000 !important; opacity: 1 !important; }\n" +
      "[data-oa-home-chat-overlay] [data-pane-id^=\"l402-\"] { background: #000 !important; opacity: 1 !important; }\n" +
      "[data-oa-home-chat-overlay] [data-pane-id^=\"l402-\"] [data-oa-pane-title], [data-oa-home-chat-overlay] [data-pane-id^=\"l402-\"] [data-oa-pane-content] { background: #000 !important; opacity: 1 !important; }\n" +
      "[data-oa-home-chat-overlay], [data-oa-home-chat-overlay] [data-oa-pane-system], [data-oa-home-chat-overlay] [data-oa-pane-layer] { user-select: none; -webkit-user-select: none; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane], [data-oa-home-chat-overlay] [data-oa-pane] * { user-select: text; -webkit-user-select: text; }\n" +
      "[data-oa-home-chat-overlay]:focus, [data-oa-home-chat-overlay] [data-oa-pane-system]:focus { outline: none !important; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-title] { cursor: grab; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-title]:active { cursor: grabbing; }\n" +
      "[data-oa-home-chat-overlay] [data-oa-pane-system][data-oa-pane-dragging=\"1\"] [data-oa-pane-title] { cursor: grabbing; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-part] { margin-top: 2px; border: 1px solid rgba(255,255,255,0.14); border-radius: 8px; background: rgba(255,255,255,0.03); overflow: hidden; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-summary] { list-style: none; display: grid; grid-template-columns: auto auto auto 1fr; align-items: center; gap: 8px; padding: 8px 10px; font-size: 11px; line-height: 1.25; cursor: pointer; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-summary]::-webkit-details-marker { display: none; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-disclosure] { opacity: 0.7; transition: transform 120ms ease; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-part][open] [data-effuse-tool-disclosure] { transform: rotate(90deg); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-status-badge=\"tool-result\"] { color: #8ef3ad; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-status-badge=\"tool-error\"] { color: #ff8f8f; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-status-badge=\"tool-call\"] { color: #9cc9ff; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-status-badge=\"tool-denied\"] { color: #ffd08a; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-status-badge=\"tool-approval\"] { color: #d2b7ff; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-name-label=\"1\"] { color: rgba(255,255,255,0.92); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-call-id-label=\"1\"] { color: rgba(255,255,255,0.58); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-summary-text=\"1\"] { color: rgba(255,255,255,0.72); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-details=\"1\"] { border-top: 1px solid rgba(255,255,255,0.1); background: rgba(0,0,0,0.34); padding: 8px 10px; display: flex; flex-direction: column; gap: 8px; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-overview=\"1\"] { font-size: 11px; color: rgba(255,255,255,0.72); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-empty=\"1\"] { font-size: 11px; color: rgba(255,255,255,0.55); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-field] { display: flex; flex-direction: column; gap: 4px; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-label=\"1\"] { font-size: 11px; color: rgba(255,255,255,0.72); text-transform: uppercase; letter-spacing: 0.02em; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-input=\"1\"] pre, [data-oa-home-chat-overlay] [data-effuse-tool-output=\"1\"] pre, [data-oa-home-chat-overlay] [data-effuse-tool-error=\"1\"] pre { margin: 0; padding: 8px; border-radius: 6px; border: 1px solid rgba(255,255,255,0.14); background: rgba(0,0,0,0.5); color: rgba(255,255,255,0.88); white-space: pre-wrap; word-break: break-word; font-size: 11px; line-height: 1.35; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-input=\"1\"] button, [data-oa-home-chat-overlay] [data-effuse-tool-output=\"1\"] button, [data-oa-home-chat-overlay] [data-effuse-tool-error=\"1\"] button { margin-top: 6px; height: 24px; border-radius: 6px; border: 1px solid rgba(255,255,255,0.18); background: rgba(255,255,255,0.06); color: rgba(255,255,255,0.8); font-size: 11px; padding: 0 8px; }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-meta=\"1\"] { font-size: 11px; color: rgba(255,255,255,0.65); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-usage=\"1\"] { color: rgba(255,255,255,0.8); }\n" +
      "[data-oa-home-chat-overlay] [data-effuse-tool-description=\"1\"] { margin-top: 2px; }"
    overlay.appendChild(paneStyle)
    const paneRoot = document.createElement("div")
    paneRoot.style.cssText = "width:100%;height:100%;"
    overlay.appendChild(paneRoot)
    shell.appendChild(overlay)

    const screen = { width: window.innerWidth, height: window.innerHeight }
    const storedRect = readStoredPaneRect(HOME_CHAT_PANE_RECT_STORAGE_KEY, screen)
    const rect = storedRect ?? calculateNewPanePosition(undefined, screen, 640, 480)
    let hostedPanePollTimer: ReturnType<typeof setInterval> | null = null
    let closeOverlay = (): void => { }
    let overlayDisposed = false
    const trackedFibers = new Set<Fiber.Fiber<unknown, unknown>>()
    const runTrackedFiber = <A, E>(
      input: {
        readonly context: string
        readonly start: () => Fiber.Fiber<A, E>
        readonly onSuccess?: (value: A) => void
        readonly onFailure?: (cause: Cause.Cause<E>) => void
      },
    ): void => {
      if (overlayDisposed) return
      const fiber = input.start()
      trackedFibers.add(fiber)
      void Effect.runPromise(Fiber.await(fiber)).then(
        (exit) => {
          trackedFibers.delete(fiber)
          if (Exit.isSuccess(exit)) {
            input.onSuccess?.(exit.value)
            return
          }
          if (Cause.isInterruptedOnly(exit.cause)) return
          input.onFailure?.(exit.cause)
          logHomeControllerAsyncError(input.context, Cause.pretty(exit.cause))
        },
        (error) => {
          trackedFibers.delete(fiber)
          logHomeControllerAsyncError(input.context, error)
        },
      )
    }
    const interruptTrackedFibers = (): void => {
      if (trackedFibers.size === 0) return
      for (const fiber of trackedFibers) {
        Effect.runFork(Fiber.interrupt(fiber))
      }
      trackedFibers.clear()
    }
    const paneSystemConfig = {
      enableDotsBackground: false,
      enableCanvasPan: false,
      enablePaneDrag: true,
      enablePaneResize: true,
      enableKeyboardShortcuts: true,
      enableHotbar: false,
      theme: { ...DEFAULT_PANE_SYSTEM_THEME, background: "transparent" },
      onPaneClosed: (id: string) => {
        // Only closing the main chat pane should dismiss the whole overlay.
        if (id === CHAT_PANE_ID) closeOverlay()
        syncPaneActionButtonState()
      },
    } as const

    let syncPaneActionButtonState = (): void => { }

    const runPaneSystemEffectSync = <A>(effect: Effect.Effect<A, never, PaneSystemService>): A => {
      if (deps?.runtime) return deps.runtime.runSync(effect)
      return Effect.runSync(effect.pipe(Effect.provide(PaneSystemLive)))
    }

    let unsubHomeChat: (() => void) | null = null
    const clearHomeChatSubscription = (): void => {
      const release = unsubHomeChat
      unsubHomeChat = null
      release?.()
    }

    const { paneSystem, release: releasePaneSystem } = runPaneSystemEffectSync(
      Effect.gen(function* () {
        const paneSystemService = yield* PaneSystemService
        return yield* paneSystemService.mount({
          root: paneRoot,
          config: paneSystemConfig,
        })
      }),
    )

    const releasePaneSystemSync = (): void => {
      runPaneSystemEffectSync(releasePaneSystem)
    }

    closeOverlay = () => {
      if (overlayDisposed) return
      overlayDisposed = true
      interruptTrackedFibers()
      if (hostedPanePollTimer) {
        clearInterval(hostedPanePollTimer)
        hostedPanePollTimer = null
      }
      clearHomeChatSubscription()
      const closedRectRaw = paneSystem.store.closedPositions.get(CHAT_PANE_ID)?.rect ?? paneSystem.store.pane(CHAT_PANE_ID)?.rect
      const closedRect = parseStoredPaneRect(closedRectRaw)
      const currentScreen =
        typeof window !== "undefined"
          ? { width: window.innerWidth, height: window.innerHeight }
          : screen
      if (closedRect) writeStoredPaneRect(HOME_CHAT_PANE_RECT_STORAGE_KEY, clampPaneRectToScreen(closedRect, currentScreen))
      releasePaneSystemSync()
      overlay.remove()
      hideStyle.remove()
      shell.removeAttribute("data-oa-home-chat-open")
      if (activeOverlayTeardown === closeOverlay) activeOverlayTeardown = null
    }
    activeOverlayTeardown = closeOverlay

    const readSessionFromAtoms = (): Session =>
      deps?.sessionState.read() ?? { userId: null, user: null }

    const writeSessionToAtoms = (session: Session): void => {
      deps?.sessionState.write(session)
    }

    const sessionFromAtoms: Session =
      readSessionFromAtoms()
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
      runTrackedFiber({
        context: "home.identity_card.render",
        start: () =>
          Effect.runFork(
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
          ),
        onSuccess: () => {
          const btn = cardEl.querySelector("[data-oa-home-identity-logout]")
          if (btn) {
            btn.addEventListener("click", () => {
              const sessionUserId = readSessionFromAtoms().userId ?? ""
            clearCachedSnapshotForUser({ runtime: deps?.runtime, userId: sessionUserId })
              void Promise.resolve(deps?.signOut?.()).then(() => closeOverlay())
            })
          }
        },
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
    if (!(paneContentSlot instanceof Element)) {
      closeOverlay()
      return
    }

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
    let dseStrategyId: "direct.v1" | "rlm_lite.v1" = "direct.v1"
    let dseBudgetProfile: "small" | "medium" | "long" = "medium"
    let isRunningDseRecap = false
    let dseErrorText: string | null = null
    let hasScrolledToBottomOnce = false
    let hasAddedPaneCopyButton = false
    let hasAddedPaneDebugButton = false
    let hasAddedPaneWalletButton = false
    let hasAddedPaneTransactionsButton = false
    let hasAddedPanePaywallsButton = false
    let hasAddedPaneSettlementsButton = false
    let hasAddedPaneDeploymentsButton = false
    let showDebugCards = false
    let paneDebugButton: HTMLButtonElement | null = null
    let paneWalletButton: HTMLButtonElement | null = null
    let paneTransactionsButton: HTMLButtonElement | null = null
    let panePaywallsButton: HTMLButtonElement | null = null
    let paneSettlementsButton: HTMLButtonElement | null = null
    let paneDeploymentsButton: HTMLButtonElement | null = null
    let paywallsPaneState: L402PaneState<LightningPaywall> = makeInitialL402PaneState()
    let settlementsPaneState: L402PaneState<LightningSettlement> = makeInitialL402PaneState()
    let deploymentsPaneState: L402PaneState<LightningGatewayDeployment> = makeInitialL402PaneState()
    let deploymentEventsPaneState: L402PaneState<LightningGatewayEvent> = makeInitialL402PaneState()
    let paywallsRefreshInFlight = false
    let settlementsRefreshInFlight = false
    let deploymentsRefreshInFlight = false
    let previousRenderedMessageCount = 0
    let forceScrollToBottomOnNextRender = false
    type L402PanePayment = L402PaymentMetadata & {
      readonly messageId: string
      readonly runId: string | null
      readonly messageIndex: number
    }
    let latestL402Payments: ReadonlyArray<L402PanePayment> = []

    const formatMsats = (value: number | undefined): string => {
      if (typeof value !== "number" || !Number.isFinite(value)) return "n/a"
      const sats = value / 1000
      return `${sats.toLocaleString(undefined, { maximumFractionDigits: 3 })} sats (${Math.round(value).toLocaleString()} msats)`
    }

    const statusBadgeClass = (status: L402PaymentMetadata["status"]): string => {
      if (status === "completed" || status === "cached") return "text-emerald-300 border-emerald-400/35 bg-emerald-500/10"
      if (status === "blocked") return "text-amber-300 border-amber-400/35 bg-amber-500/10"
      return "text-red-300 border-red-400/35 bg-red-500/10"
    }

    const parseL402PaymentPayload = (value: unknown): L402PanePayment | null => {
      const rec = asRecord(value)
      if (!rec) return null
      if (
        rec.toolName !== "lightning_l402_fetch" ||
        typeof rec.toolCallId !== "string" ||
        (rec.status !== "completed" &&
          rec.status !== "cached" &&
          rec.status !== "blocked" &&
          rec.status !== "failed") ||
        typeof rec.messageId !== "string" ||
        typeof rec.messageIndex !== "number"
      ) {
        return null
      }
      return {
        toolName: "lightning_l402_fetch",
        toolCallId: rec.toolCallId,
        status: rec.status,
        taskId: typeof rec.taskId === "string" ? rec.taskId : undefined,
        paymentId: typeof rec.paymentId === "string" ? rec.paymentId : undefined,
        amountMsats: typeof rec.amountMsats === "number" && Number.isFinite(rec.amountMsats) ? rec.amountMsats : undefined,
        responseStatusCode:
          typeof rec.responseStatusCode === "number" && Number.isFinite(rec.responseStatusCode) ? rec.responseStatusCode : undefined,
        proofReference: typeof rec.proofReference === "string" ? rec.proofReference : undefined,
        denyReason: typeof rec.denyReason === "string" ? rec.denyReason : undefined,
        url: typeof rec.url === "string" ? rec.url : undefined,
        method: typeof rec.method === "string" ? rec.method : undefined,
        scope: typeof rec.scope === "string" ? rec.scope : undefined,
        maxSpendMsats:
          typeof rec.maxSpendMsats === "number" && Number.isFinite(rec.maxSpendMsats) ? rec.maxSpendMsats : undefined,
        messageId: rec.messageId,
        runId: typeof rec.runId === "string" ? rec.runId : null,
        messageIndex: rec.messageIndex,
      }
    }

    const stylePaneOpaqueBlack = (paneId: string): void => {
      const paneEl = paneRoot.querySelector(`[data-pane-id="${paneId}"]`)
      if (!(paneEl instanceof HTMLElement)) return
      paneEl.style.background = "#000"
      paneEl.style.opacity = "1"
      const titleEl = paneEl.querySelector("[data-oa-pane-title]")
      if (titleEl instanceof HTMLElement) {
        titleEl.style.background = "#000"
        titleEl.style.opacity = "1"
      }
      const contentEl = paneEl.querySelector("[data-oa-pane-content]")
      if (contentEl instanceof HTMLElement) {
        contentEl.style.background = "#000"
        contentEl.style.opacity = "1"
      }
    }

    const collectL402PaymentsFromSnapshot = (): ReadonlyArray<L402PanePayment> => {
      if (step !== "authed" || homeSnapshot.messages.length === 0) return []
      const out: Array<L402PanePayment> = []
      for (let i = 0; i < homeSnapshot.messages.length; i++) {
        const msg = homeSnapshot.messages[i]
        const extracted = extractL402PaymentMetadata(msg.parts)
        for (const payment of extracted) {
          out.push({
            ...payment,
            messageId: msg.id,
            runId: msg.runId ?? null,
            messageIndex: i,
          })
        }
      }
      return out
    }

    const l402WalletSummary = (
      payments: ReadonlyArray<L402PanePayment>,
    ): {
      readonly totalAttempts: number
      readonly statusCounts: Record<L402PaymentMetadata["status"], number>
      readonly totalSpendMsats: number
      readonly maxSpendMsats: number
      readonly lastPaid: L402PanePayment | null
    } => {
      const counts: Record<L402PaymentMetadata["status"], number> = {
        completed: 0,
        cached: 0,
        blocked: 0,
        failed: 0,
      }
      let totalSpendMsats = 0
      let maxSpendMsats = 0
      for (const payment of payments) {
        counts[payment.status] += 1
        if ((payment.status === "completed" || payment.status === "cached") && typeof payment.amountMsats === "number") {
          totalSpendMsats += payment.amountMsats
        }
        if (typeof payment.maxSpendMsats === "number") {
          maxSpendMsats = Math.max(maxSpendMsats, payment.maxSpendMsats)
        }
      }
      const lastPaid =
        [...payments]
          .reverse()
          .find((payment) => payment.status === "completed" || payment.status === "cached") ?? null
      return {
        totalAttempts: payments.length,
        statusCounts: counts,
        totalSpendMsats,
        maxSpendMsats,
        lastPaid,
      }
    }

    const renderL402WalletPane = (): void => {
      if (!paneSystem.store.pane(L402_WALLET_PANE_ID)) return
      const slot = paneRoot.querySelector(`[data-pane-id="${L402_WALLET_PANE_ID}"] [data-oa-pane-content]`)
      if (!(slot instanceof HTMLElement)) return
      const summary = l402WalletSummary(latestL402Payments)
      runTrackedFiber({
        context: "home.chat.l402_wallet_pane.render",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              yield* dom.render(
                slot,
                html`
                  <div class="h-full overflow-auto bg-black p-4 text-sm font-mono text-white/85">
                    <div class="text-[11px] uppercase tracking-wide text-white/55">L402 Wallet Summary</div>
                    <div class="mt-3 grid grid-cols-[140px_1fr] gap-x-3 gap-y-2 text-xs">
                      <div class="text-white/55">attempts</div>
                      <div>${summary.totalAttempts}</div>
                      <div class="text-white/55">completed</div>
                      <div>${summary.statusCounts.completed}</div>
                      <div class="text-white/55">cached</div>
                      <div>${summary.statusCounts.cached}</div>
                      <div class="text-white/55">blocked</div>
                      <div>${summary.statusCounts.blocked}</div>
                      <div class="text-white/55">failed</div>
                      <div>${summary.statusCounts.failed}</div>
                      <div class="text-white/55">spent</div>
                      <div>${formatMsats(summary.totalSpendMsats)}</div>
                      <div class="text-white/55">max request cap</div>
                      <div>${summary.maxSpendMsats > 0 ? formatMsats(summary.maxSpendMsats) : "n/a"}</div>
                      <div class="text-white/55">allowlist/policy</div>
                      <div>enforced via desktop executor + macaroon scope</div>
                    </div>
                    <div class="mt-4 rounded border border-white/15 bg-white/5 p-3 text-xs">
                      <div class="text-white/60">last paid endpoint</div>
                      <div class="mt-1 break-all text-white/90">${summary.lastPaid?.url ?? "none yet"}</div>
                      ${summary.lastPaid
                    ? html`<div class="mt-1 text-white/60">proof: ${summary.lastPaid.proofReference ?? "n/a"}</div>`
                    : null}
                    </div>
                  </div>
                `,
              )
            }).pipe(Effect.provide(EffuseLive)),
          ),
      })
    }

    const renderL402TransactionsPane = (): void => {
      if (!paneSystem.store.pane(L402_TRANSACTIONS_PANE_ID)) return
      const slot = paneRoot.querySelector(`[data-pane-id="${L402_TRANSACTIONS_PANE_ID}"] [data-oa-pane-content]`)
      if (!(slot instanceof HTMLElement)) return
      const rows = [...latestL402Payments].reverse().slice(0, 40)
      runTrackedFiber({
        context: "home.chat.l402_transactions_pane.render",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              yield* dom.render(
                slot,
                html`
                  <div class="h-full overflow-auto bg-black p-3 text-xs font-mono text-white/85">
                    <div class="mb-2 px-1 text-[11px] uppercase tracking-wide text-white/55">Recent L402 Attempts</div>
                    ${rows.length === 0
                    ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No L402 payment attempts yet.</div>`
                    : html`
                        <div class="flex flex-col gap-2">
                          ${rows.map((row) => html`
                              <div class="rounded border border-white/15 bg-white/5 p-2">
                                <div class="flex items-center justify-between gap-2">
                                  <span class="inline-flex items-center rounded border px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${statusBadgeClass(row.status)}">${row.status}</span>
                                  <span class="text-[10px] text-white/55">message ${row.messageId}</span>
                                </div>
                                <div class="mt-1 break-all text-white/90">${row.url ?? "unknown endpoint"}</div>
                                <div class="mt-1 text-[11px] text-white/65">
                                  amount: ${formatMsats(row.amountMsats)} · task: ${row.taskId ?? "n/a"} · proof: ${row.proofReference ?? "n/a"}
                                </div>
                                ${row.denyReason ? html`<div class="mt-1 text-[11px] text-amber-200/90">deny: ${row.denyReason}</div>` : null}
                              </div>
                            `)}
                        </div>
                      `}
                  </div>
                `,
              )
            }).pipe(Effect.provide(EffuseLive)),
          ),
      })
    }

    const formatL402Error = (cause: Cause.Cause<unknown>): string => {
      const rendered = Cause.pretty(cause)
      const pieces = rendered
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.length > 0)
      if (pieces.length === 0) return "request_failed"
      return pieces[pieces.length - 1] ?? pieces[0] ?? "request_failed"
    }

    const renderL402PaywallsPane = (): void => {
      if (!paneSystem.store.pane(L402_PAYWALLS_PANE_ID)) return
      const slot = paneRoot.querySelector(`[data-pane-id="${L402_PAYWALLS_PANE_ID}"] [data-oa-pane-content]`)
      if (!(slot instanceof HTMLElement)) return
      const branch = l402PaneRenderBranch(paywallsPaneState)
      runTrackedFiber({
        context: "home.chat.l402_paywalls_pane.render",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              const header = html`<div class="mb-3 text-[11px] uppercase tracking-wide text-white/55">Hosted Paywalls</div>`
              const meta = html`
                <div class="mb-2 text-[11px] text-white/55">
                  request: ${paywallsPaneState.requestId ?? "n/a"} · rows: ${paywallsPaneState.rows.length}
                </div>
              `
              const loading = html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">Loading paywalls…</div>`
              const error = html`<div class="rounded border border-red-400/40 bg-red-500/10 p-3 text-red-100">${paywallsPaneState.errorText ?? "request_failed"}</div>`
              const empty = html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No paywalls yet.</div>`
              const errorBanner =
                paywallsPaneState.loadState === "error"
                  ? html`<div class="mb-2 rounded border border-red-400/40 bg-red-500/10 p-2 text-red-100">${paywallsPaneState.errorText ?? "request_failed"}</div>`
                  : null
              const data = html`
                <div class="flex flex-col gap-2">
                  ${paywallsPaneState.rows.map((paywall) => html`
                      <div class="rounded border border-white/15 bg-white/5 p-3 text-xs">
                        <div class="flex items-center justify-between gap-2">
                          <div class="text-white/90">${paywall.name}</div>
                          <span class="inline-flex rounded border px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${paywall.status === "active"
                            ? "border-emerald-400/35 bg-emerald-500/10 text-emerald-200"
                            : paywall.status === "paused"
                              ? "border-amber-400/35 bg-amber-500/10 text-amber-200"
                              : "border-red-400/35 bg-red-500/10 text-red-200"}">${paywall.status}</span>
                        </div>
                        <div class="mt-1 break-all text-white/70">${paywall.paywallId}</div>
                        <div class="mt-1 text-white/65">price: ${formatMsats(paywall.policy.fixedAmountMsats)} · routes: ${paywall.routes.length}</div>
                        <div class="mt-1 text-white/55">request: ${paywall.requestId ?? "n/a"}</div>
                      </div>
                    `)}
                </div>
              `

              yield* dom.render(
                slot,
                html`
                  <div class="h-full overflow-auto bg-black p-3 text-xs font-mono text-white/85">
                    ${header}
                    ${meta}
                    ${errorBanner}
                    ${branch === "loading" ? loading : branch === "error" ? error : branch === "empty" ? empty : data}
                  </div>
                `,
              )
            }).pipe(Effect.provide(EffuseLive)),
          ),
      })
    }

    const renderL402SettlementsPane = (): void => {
      if (!paneSystem.store.pane(L402_SETTLEMENTS_PANE_ID)) return
      const slot = paneRoot.querySelector(`[data-pane-id="${L402_SETTLEMENTS_PANE_ID}"] [data-oa-pane-content]`)
      if (!(slot instanceof HTMLElement)) return
      const branch = l402PaneRenderBranch(settlementsPaneState)
      runTrackedFiber({
        context: "home.chat.l402_settlements_pane.render",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              const header = html`<div class="mb-3 text-[11px] uppercase tracking-wide text-white/55">Hosted Settlements</div>`
              const meta = html`
                <div class="mb-2 text-[11px] text-white/55">
                  request: ${settlementsPaneState.requestId ?? "n/a"} · cursor: ${settlementsPaneState.nextCursor ?? "end"}
                </div>
              `
              const loading = html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">Loading settlements…</div>`
              const error = html`<div class="rounded border border-red-400/40 bg-red-500/10 p-3 text-red-100">${settlementsPaneState.errorText ?? "request_failed"}</div>`
              const empty = html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No settlements yet.</div>`
              const errorBanner =
                settlementsPaneState.loadState === "error"
                  ? html`<div class="mb-2 rounded border border-red-400/40 bg-red-500/10 p-2 text-red-100">${settlementsPaneState.errorText ?? "request_failed"}</div>`
                  : null
              const data = html`
                <div class="flex flex-col gap-2">
                  ${settlementsPaneState.rows.map((settlement) => html`
                      <div class="rounded border border-white/15 bg-white/5 p-3 text-xs">
                        <div class="flex items-center justify-between gap-2">
                          <div class="break-all text-white/90">${settlement.settlementId}</div>
                          <div class="text-white/75">${formatMsats(settlement.amountMsats)}</div>
                        </div>
                        <div class="mt-1 break-all text-white/65">paywall: ${settlement.paywallId}</div>
                        <div class="mt-1 break-all text-white/65">proof: ${settlement.paymentProofRef}</div>
                        <div class="mt-1 text-white/55">request: ${settlement.requestId ?? "n/a"}</div>
                      </div>
                    `)}
                </div>
              `

              yield* dom.render(
                slot,
                html`
                  <div class="h-full overflow-auto bg-black p-3 text-xs font-mono text-white/85">
                    ${header}
                    ${meta}
                    ${errorBanner}
                    ${branch === "loading" ? loading : branch === "error" ? error : branch === "empty" ? empty : data}
                  </div>
                `,
              )
            }).pipe(Effect.provide(EffuseLive)),
          ),
      })
    }

    const renderL402DeploymentsPane = (): void => {
      if (!paneSystem.store.pane(L402_DEPLOYMENTS_PANE_ID)) return
      const slot = paneRoot.querySelector(`[data-pane-id="${L402_DEPLOYMENTS_PANE_ID}"] [data-oa-pane-content]`)
      if (!(slot instanceof HTMLElement)) return
      const deploymentBranch = l402PaneRenderBranch(deploymentsPaneState)
      const eventsBranch = l402PaneRenderBranch(deploymentEventsPaneState)
      const latestDeployment = deploymentsPaneState.rows[0] ?? null

      runTrackedFiber({
        context: "home.chat.l402_deployments_pane.render",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              const deploymentsContent =
                deploymentBranch === "loading"
                  ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">Loading deployments…</div>`
                  : deploymentBranch === "error"
                    ? html`<div class="rounded border border-red-400/40 bg-red-500/10 p-3 text-red-100">${deploymentsPaneState.errorText ?? "request_failed"}</div>`
                    : deploymentBranch === "empty"
                      ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No deployments yet.</div>`
                      : html`
                          <div class="flex flex-col gap-2">
                            ${deploymentsPaneState.rows.map((deployment) => html`
                                <div class="rounded border border-white/15 bg-white/5 p-3 text-xs">
                                  <div class="flex items-center justify-between gap-2">
                                    <div class="break-all text-white/90">${deployment.deploymentId}</div>
                                    <span class="inline-flex rounded border px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${deployment.status === "applied"
                                      ? "border-emerald-400/35 bg-emerald-500/10 text-emerald-200"
                                      : deployment.status === "pending"
                                        ? "border-amber-400/35 bg-amber-500/10 text-amber-200"
                                        : "border-red-400/35 bg-red-500/10 text-red-200"}">${deployment.status}</span>
                                  </div>
                                  <div class="mt-1 break-all text-white/65">config: ${deployment.configHash}</div>
                                  <div class="mt-1 break-all text-white/55">paywall: ${deployment.paywallId ?? "n/a"}</div>
                                </div>
                              `)}
                          </div>
                        `

              const eventsContent =
                eventsBranch === "loading"
                  ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">Loading deployment events…</div>`
                  : eventsBranch === "error"
                    ? html`<div class="rounded border border-red-400/40 bg-red-500/10 p-3 text-red-100">${deploymentEventsPaneState.errorText ?? "request_failed"}</div>`
                    : eventsBranch === "empty"
                      ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No deployment events yet.</div>`
                      : html`
                          <div class="flex flex-col gap-2">
                            ${deploymentEventsPaneState.rows.slice(0, 20).map((event) => html`
                                <div class="rounded border border-white/15 bg-white/5 p-2 text-xs">
                                  <div class="flex items-center justify-between gap-2">
                                    <div class="text-white/90">${event.eventType}</div>
                                    <div class="text-white/70">${event.level}</div>
                                  </div>
                                  <div class="mt-1 break-all text-white/60">request: ${event.requestId ?? "n/a"}</div>
                                </div>
                              `)}
                          </div>
                        `

              yield* dom.render(
                slot,
                html`
                  <div class="h-full overflow-auto bg-black p-3 text-xs font-mono text-white/85">
                    <div class="mb-2 text-[11px] uppercase tracking-wide text-white/55">Gateway Deployments</div>
                    <div class="mb-2 text-[11px] text-white/55">
                      deployments request: ${deploymentsPaneState.requestId ?? "n/a"} · events request: ${deploymentEventsPaneState.requestId ?? "n/a"}
                    </div>
                    ${deploymentsPaneState.loadState === "error"
                      ? html`<div class="mb-2 rounded border border-red-400/40 bg-red-500/10 p-2 text-red-100">${deploymentsPaneState.errorText ?? "request_failed"}</div>`
                      : null}
                    ${deploymentEventsPaneState.loadState === "error"
                      ? html`<div class="mb-2 rounded border border-red-400/40 bg-red-500/10 p-2 text-red-100">${deploymentEventsPaneState.errorText ?? "request_failed"}</div>`
                      : null}
                    <div class="mb-3 rounded border border-white/15 bg-white/5 p-3 text-xs">
                      <div class="text-white/60">latest deployment</div>
                      <div class="mt-1 break-all text-white/90">${latestDeployment?.deploymentId ?? "none"}</div>
                      <div class="mt-1 text-white/65">status: ${latestDeployment?.status ?? "n/a"} · config: ${latestDeployment?.configHash ?? "n/a"}</div>
                    </div>
                    ${deploymentsContent}
                    <div class="mt-4 mb-2 text-[11px] uppercase tracking-wide text-white/55">Deployment History</div>
                    ${eventsContent}
                  </div>
                `,
              )
            }).pipe(Effect.provide(EffuseLive)),
          ),
      })
    }

    const renderL402AuxPanes = (): void => {
      renderL402WalletPane()
      renderL402TransactionsPane()
      renderL402PaywallsPane()
      renderL402SettlementsPane()
      renderL402DeploymentsPane()
    }

    const openL402PaymentDetailPane = (payment: L402PanePayment): void => {
      const paneId = `l402-payment-${Date.now()}`
      const screen = { width: paneRoot.clientWidth, height: paneRoot.clientHeight }
      const rect = calculateNewPanePosition(paneSystem.store.lastPanePosition, screen, 560, 380)
      paneSystem.store.addPane({
        id: paneId,
        kind: "l402-payment",
        title: "Payment Detail",
        rect,
        dismissable: true,
      })
      paneSystem.store.bringToFront(paneId)
      paneSystem.render()
      stylePaneOpaqueBlack(paneId)
      const slot = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`)
      const titleActions = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-title-actions]`)
      const payloadJson = JSON.stringify(payment, null, 2)

      if (titleActions instanceof HTMLElement) {
        const copyBtn = document.createElement("button")
        copyBtn.setAttribute("type", "button")
        copyBtn.setAttribute("aria-label", "Copy payment detail")
        copyBtn.innerHTML = COPY_ICON_SVG
        copyBtn.addEventListener(
          "pointerdown",
          (e) => {
            if (e.button !== 0) return
            e.preventDefault()
            e.stopPropagation()
            e.stopImmediatePropagation()
            copyTextToClipboard(payloadJson, "metadata-pane")
            copyBtn.innerHTML = CHECKMARK_ICON_SVG
            setTimeout(() => {
              copyBtn.innerHTML = COPY_ICON_SVG
            }, 1000)
          },
          { capture: true },
        )
        titleActions.appendChild(copyBtn)
      }

      if (slot instanceof HTMLElement) {
        runTrackedFiber({
          context: "home.chat.l402_payment_pane.render",
          start: () =>
            Effect.runFork(
              Effect.gen(function* () {
                const dom = yield* DomServiceTag
                yield* dom.render(
                  slot,
                  html`
                    <div class="h-full overflow-auto bg-black p-4 text-sm font-mono text-white/85">
                      <div class="grid grid-cols-[140px_1fr] gap-x-3 gap-y-2 text-xs">
                        <div class="text-white/55">status</div>
                        <div>${payment.status}</div>
                        <div class="text-white/55">url</div>
                        <div class="break-all">${payment.url ?? "n/a"}</div>
                        <div class="text-white/55">method</div>
                        <div>${payment.method ?? "GET"}</div>
                        <div class="text-white/55">scope</div>
                        <div>${payment.scope ?? "default"}</div>
                        <div class="text-white/55">taskId</div>
                        <div>${payment.taskId ?? "n/a"}</div>
                        <div class="text-white/55">paymentId</div>
                        <div>${payment.paymentId ?? "n/a"}</div>
                        <div class="text-white/55">amount</div>
                        <div>${formatMsats(payment.amountMsats)}</div>
                        <div class="text-white/55">proof</div>
                        <div class="break-all">${payment.proofReference ?? "n/a"}</div>
                        <div class="text-white/55">denyReason</div>
                        <div>${payment.denyReason ?? "n/a"}</div>
                        <div class="text-white/55">responseStatus</div>
                        <div>${payment.responseStatusCode ?? "n/a"}</div>
                        <div class="text-white/55">messageId</div>
                        <div>${payment.messageId}</div>
                        <div class="text-white/55">runId</div>
                        <div>${payment.runId ?? "n/a"}</div>
                      </div>
                    </div>
                  `,
                )
              }).pipe(Effect.provide(EffuseLive)),
            ),
        })
      }
    }

    const togglePersistentL402Pane = (opts: {
      readonly paneId: string
      readonly kind: string
      readonly title: string
      readonly width: number
      readonly height: number
    }): void => {
      const screen = { width: paneRoot.clientWidth, height: paneRoot.clientHeight }
      paneSystem.store.togglePane(opts.paneId, screen, (snapshot) => ({
        id: opts.paneId,
        kind: opts.kind,
        title: opts.title,
        rect: snapshot?.rect ?? calculateNewPanePosition(paneSystem.store.lastPanePosition, screen, opts.width, opts.height),
        dismissable: true,
      }))
      paneSystem.render()
      if (paneSystem.store.pane(opts.paneId)) stylePaneOpaqueBlack(opts.paneId)
      if (opts.paneId === L402_PAYWALLS_PANE_ID || opts.paneId === L402_SETTLEMENTS_PANE_ID || opts.paneId === L402_DEPLOYMENTS_PANE_ID) {
        const hostedOpen = hasAnyHostedOpsPaneOpen({
          paywallsOpen: paneSystem.store.pane(L402_PAYWALLS_PANE_ID) != null,
          settlementsOpen: paneSystem.store.pane(L402_SETTLEMENTS_PANE_ID) != null,
          deploymentsOpen: paneSystem.store.pane(L402_DEPLOYMENTS_PANE_ID) != null,
        })
        if (hostedOpen) {
          if (!hostedPanePollTimer) {
            hostedPanePollTimer = setInterval(() => {
              if (paneSystem.store.pane(L402_PAYWALLS_PANE_ID)) refreshHostedPaywalls()
              if (paneSystem.store.pane(L402_SETTLEMENTS_PANE_ID)) refreshHostedSettlements()
              if (paneSystem.store.pane(L402_DEPLOYMENTS_PANE_ID)) refreshHostedDeployments()
            }, 10_000)
          }
          if (paneSystem.store.pane(L402_PAYWALLS_PANE_ID)) refreshHostedPaywalls()
          if (paneSystem.store.pane(L402_SETTLEMENTS_PANE_ID)) refreshHostedSettlements()
          if (paneSystem.store.pane(L402_DEPLOYMENTS_PANE_ID)) refreshHostedDeployments()
        } else if (hostedPanePollTimer) {
          clearInterval(hostedPanePollTimer)
          hostedPanePollTimer = null
        }
      }
      syncPaneActionButtonState()
      renderL402AuxPanes()
    }

    const refreshHostedPaywalls = (): void => {
      if (!deps) {
        paywallsPaneState = rejectL402PaneState({
          previous: paywallsPaneState,
          errorText: "runtime_unavailable",
          updatedAtMs: Date.now(),
        })
        renderL402PaywallsPane()
        return
      }
      if (paywallsRefreshInFlight) return
      paywallsRefreshInFlight = true
      paywallsPaneState = startL402PaneLoading(paywallsPaneState)
      renderL402PaywallsPane()
      runTrackedFiber({
        context: "home.chat.l402_paywalls_pane.refresh",
        start: () =>
          deps.runtime.runFork(
            Effect.gen(function* () {
              const lightning = yield* LightningApiService
              return yield* lightning.listPaywalls({ limit: 50 })
            }),
          ),
        onSuccess: (value) => {
          paywallsRefreshInFlight = false
          paywallsPaneState = resolveL402PaneState({
            rows: value.paywalls,
            requestId: value.requestId,
            updatedAtMs: Date.now(),
          })
          renderL402PaywallsPane()
        },
        onFailure: (cause) => {
          paywallsRefreshInFlight = false
          paywallsPaneState = rejectL402PaneState({
            previous: paywallsPaneState,
            errorText: formatL402Error(cause),
            updatedAtMs: Date.now(),
          })
          renderL402PaywallsPane()
        },
      })
    }

    const refreshHostedSettlements = (): void => {
      if (!deps) {
        settlementsPaneState = rejectL402PaneState({
          previous: settlementsPaneState,
          errorText: "runtime_unavailable",
          updatedAtMs: Date.now(),
        })
        renderL402SettlementsPane()
        return
      }
      if (settlementsRefreshInFlight) return
      settlementsRefreshInFlight = true
      settlementsPaneState = startL402PaneLoading(settlementsPaneState)
      renderL402SettlementsPane()
      runTrackedFiber({
        context: "home.chat.l402_settlements_pane.refresh",
        start: () =>
          deps.runtime.runFork(
            Effect.gen(function* () {
              const lightning = yield* LightningApiService
              return yield* lightning.listOwnerSettlements({ limit: 50 })
            }),
          ),
        onSuccess: (value) => {
          settlementsRefreshInFlight = false
          settlementsPaneState = resolveL402PaneState({
            rows: value.settlements,
            requestId: value.requestId,
            nextCursor: value.nextCursor,
            updatedAtMs: Date.now(),
          })
          renderL402SettlementsPane()
        },
        onFailure: (cause) => {
          settlementsRefreshInFlight = false
          settlementsPaneState = rejectL402PaneState({
            previous: settlementsPaneState,
            errorText: formatL402Error(cause),
            updatedAtMs: Date.now(),
          })
          renderL402SettlementsPane()
        },
      })
    }

    const refreshHostedDeployments = (): void => {
      if (!deps) {
        deploymentsPaneState = rejectL402PaneState({
          previous: deploymentsPaneState,
          errorText: "runtime_unavailable",
          updatedAtMs: Date.now(),
        })
        deploymentEventsPaneState = rejectL402PaneState({
          previous: deploymentEventsPaneState,
          errorText: "runtime_unavailable",
          updatedAtMs: Date.now(),
        })
        renderL402DeploymentsPane()
        return
      }
      if (deploymentsRefreshInFlight) return
      deploymentsRefreshInFlight = true
      deploymentsPaneState = startL402PaneLoading(deploymentsPaneState)
      deploymentEventsPaneState = startL402PaneLoading(deploymentEventsPaneState)
      renderL402DeploymentsPane()
      runTrackedFiber({
        context: "home.chat.l402_deployments_pane.refresh",
        start: () =>
          deps.runtime.runFork(
            Effect.gen(function* () {
              const lightning = yield* LightningApiService
              const [deployments, events] = yield* Effect.all([
                lightning.listDeployments({ limit: 50 }),
                lightning.listDeploymentEvents({ limit: 50 }),
              ])
              return { deployments, events }
            }),
          ),
        onSuccess: ({ deployments, events }) => {
          deploymentsRefreshInFlight = false
          deploymentsPaneState = resolveL402PaneState({
            rows: deployments.deployments,
            requestId: deployments.requestId,
            nextCursor: deployments.nextCursor,
            updatedAtMs: Date.now(),
          })
          deploymentEventsPaneState = resolveL402PaneState({
            rows: events.events,
            requestId: events.requestId,
            nextCursor: events.nextCursor,
            updatedAtMs: Date.now(),
          })
          renderL402DeploymentsPane()
        },
        onFailure: (cause) => {
          deploymentsRefreshInFlight = false
          const errorText = formatL402Error(cause)
          deploymentsPaneState = rejectL402PaneState({
            previous: deploymentsPaneState,
            errorText,
            updatedAtMs: Date.now(),
          })
          deploymentEventsPaneState = rejectL402PaneState({
            previous: deploymentEventsPaneState,
            errorText,
            updatedAtMs: Date.now(),
          })
          renderL402DeploymentsPane()
        },
      })
    }

    syncPaneActionButtonState = () => {
      const walletOpen = paneSystem.store.pane(L402_WALLET_PANE_ID) != null
      const transactionsOpen = paneSystem.store.pane(L402_TRANSACTIONS_PANE_ID) != null
      const paywallsOpen = paneSystem.store.pane(L402_PAYWALLS_PANE_ID) != null
      const settlementsOpen = paneSystem.store.pane(L402_SETTLEMENTS_PANE_ID) != null
      const deploymentsOpen = paneSystem.store.pane(L402_DEPLOYMENTS_PANE_ID) != null
      if (paneWalletButton instanceof HTMLButtonElement) {
        const style = paneButtonVisualState(walletOpen)
        paneWalletButton.setAttribute("aria-pressed", style.ariaPressed)
        paneWalletButton.style.color = style.color
        paneWalletButton.style.opacity = style.opacity
      }
      if (paneTransactionsButton instanceof HTMLButtonElement) {
        const style = paneButtonVisualState(transactionsOpen)
        paneTransactionsButton.setAttribute("aria-pressed", style.ariaPressed)
        paneTransactionsButton.style.color = style.color
        paneTransactionsButton.style.opacity = style.opacity
      }
      if (panePaywallsButton instanceof HTMLButtonElement) {
        const style = paneButtonVisualState(paywallsOpen)
        panePaywallsButton.setAttribute("aria-pressed", style.ariaPressed)
        panePaywallsButton.style.color = style.color
        panePaywallsButton.style.opacity = style.opacity
      }
      if (paneSettlementsButton instanceof HTMLButtonElement) {
        const style = paneButtonVisualState(settlementsOpen)
        paneSettlementsButton.setAttribute("aria-pressed", style.ariaPressed)
        paneSettlementsButton.style.color = style.color
        paneSettlementsButton.style.opacity = style.opacity
      }
      if (paneDeploymentsButton instanceof HTMLButtonElement) {
        const style = paneButtonVisualState(deploymentsOpen)
        paneDeploymentsButton.setAttribute("aria-pressed", style.ariaPressed)
        paneDeploymentsButton.style.color = style.color
        paneDeploymentsButton.style.opacity = style.opacity
      }

      const hostedOpen = hasAnyHostedOpsPaneOpen({
        paywallsOpen,
        settlementsOpen,
        deploymentsOpen,
      })
      if (!hostedOpen && hostedPanePollTimer) {
        clearInterval(hostedPanePollTimer)
        hostedPanePollTimer = null
      }
    }

    const attachHomeThreadSubscription = (
      input: {
        readonly threadId: string
        readonly userId: string
        readonly hydratedSnapshot?: ChatSnapshot | null
      },
    ): void => {
      if (!deps?.atoms) return
      const threadId = input.threadId
      if (!threadId) return

      homeThreadId = threadId
      deps.atoms.get(ChatSnapshotAtom(threadId))
      clearHomeChatSubscription()

      let skippedHydratedPlaceholder = false
      const hydratedSnapshot = input.hydratedSnapshot ?? null
      unsubHomeChat = deps.atoms.subscribe(
        ChatSnapshotAtom(threadId),
        (snap) => {
          if (
            shouldSkipHydratedPlaceholder({
              skippedHydratedPlaceholder,
              hasHydratedSnapshot: hydratedSnapshot != null,
              hydratedSnapshotMessageCount: hydratedSnapshot?.messages.length ?? 0,
              nextSnapshotMessageCount: snap.messages.length,
              nextSnapshotStatus: snap.status,
              nextSnapshotErrorText: snap.errorText,
            })
          ) {
            skippedHydratedPlaceholder = true
            return
          }
          homeSnapshot = snap
          writeCachedSnapshotForUser({ runtime: deps?.runtime, userId: input.userId, threadId, snapshot: snap })
          doRender()
        },
        { immediate: true },
      )
    }

    const startAuthedChat = (input0: { readonly userId: string; readonly user: Session["user"] | null; readonly token: string | null }) => {
      if (!deps?.atoms || !deps.chat) return

      writeSessionToAtoms({ userId: input0.userId, user: input0.user })

      // Prime the in-memory auth token cache so Convex can authenticate immediately without waiting
      // for cookie timing (especially important in tests and right after verify/login).
      if (input0.token && input0.user) {
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

      void Promise.resolve(deps.refreshConvexAuth?.()).catch((error) => {
        logHomeControllerAsyncError("home.chat.refresh_convex_auth", error)
      })

      if (input0.user?.email) renderIdentityCard(input0.user.email)

      const cached = readCachedSnapshotForUser({ runtime: deps?.runtime, userId: input0.userId })
      messages.length = 0
      step = "authed"
      if (cached) {
        homeThreadId = cached.threadId
        homeSnapshot = cached.snapshot
        forceScrollToBottomOnNextRender = true
        doRender()
        attachHomeThreadSubscription({
          threadId: cached.threadId,
          userId: input0.userId,
          hydratedSnapshot: cached.snapshot,
        })
      } else {
        messages.push({ role: "assistant", text: ONBOARDING_FIRST_MESSAGE })
        doRender()
      }

      runTrackedFiber({
        context: "home.chat.get_owned_thread_id.authed_start",
        start: () => deps.runtime.runFork(deps.chat.getOwnedThreadId()),
        onSuccess: (id) => {
          if (id && id.length > 0) {
            attachHomeThreadSubscription({
              threadId: id,
              userId: input0.userId,
              hydratedSnapshot: cached?.threadId === id ? cached.snapshot : null,
            })
          }
          doRender()
        },
        onFailure: () => {
          doRender()
        },
      })
    }

    const chatInputClass =
      "w-full px-3 py-2 rounded bg-white/5 border border-white/10 text-white/90 text-sm font-mono placeholder-white/40 focus:outline-none focus:border-white/20"

    const textFromRenderParts = (
      parts: ReadonlyArray<{ readonly kind?: unknown; readonly text?: unknown }>,
    ): string => {
      if (!parts?.length) return ""
      return parts
        .filter((p): p is { readonly kind: "text"; readonly text: string } => p?.kind === "text" && typeof p.text === "string")
        .map((p) => p.text)
        .join("")
    }

    const getChatMarkdown = (): string => {
      const blocks: string[] = []
      if (step === "authed" && homeSnapshot.messages.length > 0) {
        for (const m of homeSnapshot.messages) {
          const role = m.role
          const partsRaw = m.parts
          const renderParts = toAutopilotRenderParts({ parts: partsRaw, toolContractsByName: null })
          const text = textFromRenderParts(renderParts)
          blocks.push((role === "user" ? "## User\n\n" : "## Assistant\n\n") + (text || "(no text)"))
        }
      } else {
        for (const m of messages) {
          blocks.push((m.role === "user" ? "## User\n\n" : "## Assistant\n\n") + m.text)
        }
      }
      return blocks.join("\n\n")
    }

    const getThreadTrace = (): { threadId: string | null; entries: unknown[] } => {
      const entries: unknown[] = []
      if (step !== "authed" || !homeSnapshot.messages.length) {
        return { threadId: homeThreadId, entries }
      }
      for (const msg of homeSnapshot.messages) {
        const messageId = msg.id ?? null
        const role = msg.role ?? null
        const parts = msg.parts as ReadonlyArray<Record<string, unknown>>
        for (const p of parts) {
          const t = String(p?.type ?? "")
          if (t.startsWith("dse.") || t.startsWith("tool") || t === "dynamic-tool") {
            entries.push({ messageId, role, type: t, id: p?.id ?? p?.signatureId, state: p?.state, ...sanitizeForTrace(p) })
          }
        }
      }
      return { threadId: homeThreadId, entries }
    }
    const sanitizeForTrace = (p: Record<string, unknown>): Record<string, unknown> => {
      const out: Record<string, unknown> = {}
      if (p?.signatureId != null) out.signatureId = p.signatureId
      if (p?.strategyId != null) out.strategyId = p.strategyId
      if (p?.strategyReason != null) out.strategyReason = p.strategyReason
      if (p?.compiled_id != null) out.compiled_id = p.compiled_id
      if (p?.receiptId != null) out.receiptId = p.receiptId
      if (p?.toolCallId != null) out.toolCallId = p.toolCallId
      if (p?.toolName != null) out.toolName = p.toolName
      if (p?.timing != null) out.timing = p.timing
      if (p?.budget != null) out.budget = p.budget
      if (p?.tsMs != null) out.tsMs = p.tsMs
      if (p?.jobHash != null) out.jobHash = p.jobHash
      if (p?.errorText != null) out.errorText = p.errorText
      if (p?.rlmTrace != null) out.rlmTrace = p.rlmTrace
      if (p?.contextPressure != null) out.contextPressure = p.contextPressure
      if (p?.promptRenderStats != null) out.promptRenderStats = p.promptRenderStats
      if (p?.outputPreview != null) out.outputPreview = p.outputPreview
      if (p?.from != null) out.from = p.from
      if (p?.to != null) out.to = p.to
      if (p?.reason != null) out.reason = p.reason
      if (p?.best != null) out.best = p.best
      if (p?.candidates != null) out.candidates = p.candidates
      return out
    }

    type MessageModelMeta = {
      readonly modelId: string
      readonly provider: string
      readonly modelRoute: string
      readonly modelFallbackId: string
      readonly recordingStatus: "recorded" | "missing_in_finish" | "unavailable"
    }

    type MessageSignatureMeta = {
      readonly id: string | null
      readonly state: string | null
      readonly signatureId: string
      readonly compiled_id?: string
      readonly receiptId?: string
      readonly strategyId?: string
      readonly strategyReason?: string
      readonly durationMs?: number
      readonly budget?: unknown
      readonly modelId?: string
      readonly provider?: string
      readonly modelRoute?: string
      readonly modelFallbackId?: string
    }

    type MessageInferenceMeta = {
      readonly hasFinish: boolean
      readonly finishReason: string | null
      readonly timeToFirstTokenMs: number | null
      readonly timeToCompleteMs: number | null
      readonly usage: {
        readonly inputTokens: number | null
        readonly outputTokens: number | null
        readonly totalTokens: number | null
      }
      readonly textPartChars: number
      readonly partCount: number
      readonly dsePartCount: number
      readonly toolPartCount: number
      readonly partTypes: ReadonlyArray<string>
    }

    type MessageDebugInfo = {
      readonly sourceKind: "dse" | "llm" | "unknown"
      readonly sourceReason: string
      readonly model: MessageModelMeta
      readonly inference: MessageInferenceMeta
      readonly signatures: ReadonlyArray<MessageSignatureMeta>
      readonly primarySignature: MessageSignatureMeta | null
    }

    type MessagePartLike = {
      readonly type?: unknown
      readonly id?: unknown
      readonly signatureId?: unknown
      readonly state?: unknown
      readonly compiled_id?: unknown
      readonly receiptId?: unknown
      readonly strategyId?: unknown
      readonly strategyReason?: unknown
      readonly timing?: unknown
      readonly budget?: unknown
      readonly model?: unknown
      readonly toolCallId?: unknown
      readonly errorText?: unknown
      readonly text?: unknown
    }

    type MessageFinishLike = {
      readonly reason?: unknown
      readonly usage?: unknown
      readonly modelId?: unknown
      readonly provider?: unknown
      readonly modelRoute?: unknown
      readonly modelFallbackId?: unknown
      readonly timeToFirstTokenMs?: unknown
      readonly timeToCompleteMs?: unknown
    }

    type RawMessageLike = {
      readonly id?: unknown
      readonly runId?: unknown
      readonly role?: unknown
      readonly finish?: unknown
      readonly parts?: unknown
    }

    const asStringOrNull = (value: unknown): string | null =>
      typeof value === "string" && value.trim().length > 0 ? value : null

    const asNumberOrNull = (value: unknown): number | null =>
      typeof value === "number" && Number.isFinite(value) ? value : null

    const messagePartsFromUnknown = (value: unknown): ReadonlyArray<MessagePartLike> =>
      Array.isArray(value) ? value.filter((p) => p && typeof p === "object") as ReadonlyArray<MessagePartLike> : []

    const signaturesFromParts = (parts: ReadonlyArray<MessagePartLike>): ReadonlyArray<MessageSignatureMeta> =>
      parts
        .filter((p) => p.type === "dse.signature" && typeof p.signatureId === "string")
        .map((p) => {
          const model = asRecord(p.model)
          const timing = asRecord(p.timing)
          return {
            id: asStringOrNull(p.id),
            state: asStringOrNull(p.state),
            signatureId: String(p.signatureId),
            ...(asStringOrNull(p.compiled_id) ? { compiled_id: String(p.compiled_id) } : {}),
            ...(asStringOrNull(p.receiptId) ? { receiptId: String(p.receiptId) } : {}),
            ...(asStringOrNull(p.strategyId) ? { strategyId: String(p.strategyId) } : {}),
            ...(asStringOrNull(p.strategyReason) ? { strategyReason: String(p.strategyReason) } : {}),
            ...(asNumberOrNull(timing?.durationMs) != null ? { durationMs: Number(timing?.durationMs) } : {}),
            ...(p.budget && typeof p.budget === "object" ? { budget: p.budget } : {}),
            ...(asStringOrNull(model?.modelId) ? { modelId: String(model?.modelId) } : {}),
            ...(asStringOrNull(model?.provider) ? { provider: String(model?.provider) } : {}),
            ...(asStringOrNull(model?.route) ? { modelRoute: String(model?.route) } : {}),
            ...(asStringOrNull(model?.fallbackModelId) ? { modelFallbackId: String(model?.fallbackModelId) } : {}),
          } satisfies MessageSignatureMeta
        })

    const inferenceMetaFromParts = (parts: ReadonlyArray<MessagePartLike>, finish: MessageFinishLike | null): MessageInferenceMeta => {
      const usage = asRecord(finish?.usage)
      const inputTokens = asNumberOrNull(usage?.inputTokens)
      const outputTokens = asNumberOrNull(usage?.outputTokens)
      const totalTokens = asNumberOrNull(usage?.totalTokens)
      const timeToFirstTokenMs = asNumberOrNull(finish?.timeToFirstTokenMs)
      const timeToCompleteMs = asNumberOrNull(finish?.timeToCompleteMs)
      const partTypes = parts.map((p) => String(p.type ?? "?"))
      const textPartChars = parts
        .filter((p) => p.type === "text" && typeof p.text === "string")
        .reduce((sum, p) => sum + String(p.text ?? "").length, 0)
      const dsePartCount = parts.filter((p) => String(p.type ?? "").startsWith("dse.")).length
      const toolPartCount = parts.filter((p) => String(p.type ?? "").startsWith("tool-") || p.type === "dynamic-tool").length
      return {
        hasFinish: Boolean(finish && typeof finish === "object"),
        finishReason: asStringOrNull(finish?.reason),
        timeToFirstTokenMs,
        timeToCompleteMs,
        usage: { inputTokens, outputTokens, totalTokens },
        textPartChars,
        partCount: parts.length,
        dsePartCount,
        toolPartCount,
        partTypes,
      }
    }

    const messageDebugInfo = (rawMessage: RawMessageLike): MessageDebugInfo => {
      const parts = messagePartsFromUnknown(rawMessage.parts)
      const signatures = signaturesFromParts(parts)
      const finish = asRecord(rawMessage.finish) as MessageFinishLike | null
      const latestSignature = signatures.length > 0 ? signatures[signatures.length - 1] : null
      const inference = inferenceMetaFromParts(parts, finish)

      const modelId = asStringOrNull(finish?.modelId) ?? latestSignature?.modelId ?? null
      const provider = asStringOrNull(finish?.provider) ?? latestSignature?.provider ?? null
      const modelRoute = asStringOrNull(finish?.modelRoute) ?? latestSignature?.modelRoute ?? null
      const modelFallbackId = asStringOrNull(finish?.modelFallbackId) ?? latestSignature?.modelFallbackId ?? null
      const hasRecordedModel = Boolean(modelId || provider || modelRoute || modelFallbackId)
      const recordingStatus: MessageModelMeta["recordingStatus"] =
        hasRecordedModel ? "recorded" : inference.hasFinish ? "missing_in_finish" : "unavailable"
      const missingValue = recordingStatus === "missing_in_finish" ? "not_recorded_on_message" : "unavailable"
      const model: MessageModelMeta = {
        modelId: modelId ?? missingValue,
        provider: provider ?? missingValue,
        modelRoute: modelRoute ?? missingValue,
        modelFallbackId: modelFallbackId ?? missingValue,
        recordingStatus,
      }

      const sourceKind: MessageDebugInfo["sourceKind"] =
        signatures.length > 0 ? "dse" : inference.hasFinish ? "llm" : "unknown"

      const sourceReason =
        sourceKind === "dse"
          ? "dse.signature part recorded"
          : sourceKind === "llm"
            ? hasRecordedModel
              ? "LLM finish + model metadata recorded"
              : "LLM finish recorded but model metadata missing on this message"
            : "No signature or finish metadata recorded"

      return {
        sourceKind,
        sourceReason,
        model,
        inference,
        signatures,
        primarySignature: latestSignature,
      }
    }

    const debugRow = (label: string, value: string | number | null | undefined): ReturnType<typeof html> => {
      if (value == null || value === "") return html``
      return html`
        <div class="grid grid-cols-[108px_1fr] gap-2 text-xs leading-relaxed">
          <div class="text-white/60">${label}</div>
          <div class="text-white/90 font-mono break-words">${value}</div>
        </div>
      `
    }

    const debugSourceBadge = (kind: MessageDebugInfo["sourceKind"]): ReturnType<typeof html> => {
      const label = kind === "dse" ? "dse" : kind === "llm" ? "llm" : "unknown"
      const cls =
        kind === "dse"
          ? "border-emerald-400/40 bg-emerald-500/10 text-emerald-300"
          : kind === "llm"
            ? "border-blue-400/40 bg-blue-500/10 text-blue-300"
            : "border-white/30 bg-white/10 text-white/80"
      return html`<span class="inline-flex items-center rounded border px-2 py-0.5 text-[11px] uppercase tracking-wide ${cls}"
        >${label}</span
      >`
    }

    const renderInferenceDebugCard = (opts: {
      readonly messageId: string
      readonly info: MessageDebugInfo
    }): ReturnType<typeof html> => {
      const tokens = opts.info.inference.usage
      const tokenSummary = [
        `input=${tokens.inputTokens ?? "?"}`,
        `output=${tokens.outputTokens ?? "?"}`,
        `total=${tokens.totalTokens ?? "?"}`,
      ].join(" ")
      return html`
        <section
          data-oa-debug-card="1"
          data-oa-debug-card-title="Inference Metadata"
          data-oa-debug-card-state="${opts.info.sourceKind}"
          class="rounded-lg border border-white/15 bg-white/5 px-3 py-3"
        >
          <header class="flex items-center justify-between gap-3">
            <div class="text-xs text-white/60 uppercase tracking-wider">Inference Metadata</div>
            ${debugSourceBadge(opts.info.sourceKind)}
          </header>
          <div class="mt-2 flex flex-col gap-2">
            ${debugRow("recording", opts.info.model.recordingStatus)}
            ${debugRow("reason", opts.info.sourceReason)}
            ${debugRow("modelId", opts.info.model.modelId)}
            ${debugRow("provider", opts.info.model.provider)}
            ${debugRow("modelRoute", opts.info.model.modelRoute)}
            ${debugRow("fallbackModelId", opts.info.model.modelFallbackId)}
            ${debugRow("finishReason", opts.info.inference.finishReason)}
            ${debugRow("timeToFirstTokenMs", opts.info.inference.timeToFirstTokenMs)}
            ${debugRow("timeToCompleteMs", opts.info.inference.timeToCompleteMs)}
            ${debugRow("tokens", tokenSummary)}
            ${debugRow("textChars", opts.info.inference.textPartChars)}
            ${debugRow("parts", opts.info.inference.partCount)}
            ${debugRow("dseParts", opts.info.inference.dsePartCount)}
            ${debugRow("toolParts", opts.info.inference.toolPartCount)}
            ${debugRow("partTypes", opts.info.inference.partTypes.join(", "))}
            ${debugRow("messageId", opts.messageId)}
          </div>
        </section>
      `
    }

    const messageMetadataJson = (rawMessage: RawMessageLike, threadId: string | null, userId?: string | null): string => {
      const parts = messagePartsFromUnknown(rawMessage.parts)
      const l402Payments = extractL402PaymentMetadata(parts)
      const debug = messageDebugInfo(rawMessage)
      const partsSummary = parts.map((p) => {
        const base = { type: p.type ?? "?", id: p.id ?? p.signatureId }
        if (p.type === "dse.signature" || p.signatureId) {
          const model = asRecord(p.model)
          const timing = asRecord(p.timing)
          return {
            ...base,
            signatureId: p.signatureId ?? null,
            strategyId: p.strategyId ?? null,
            strategyReason: p.strategyReason ?? null,
            compiled_id: p.compiled_id ?? null,
            receiptId: p.receiptId ?? null,
            state: p.state ?? null,
            durationMs: asNumberOrNull(timing?.durationMs),
            budget: p.budget,
            ...(asStringOrNull(model?.modelId) ? { modelId: model?.modelId } : {}),
            ...(asStringOrNull(model?.provider) ? { provider: model?.provider } : {}),
            ...(asStringOrNull(model?.route) ? { modelRoute: model?.route } : {}),
            ...(asStringOrNull(model?.fallbackModelId) ? { modelFallbackId: model?.fallbackModelId } : {}),
            errorText: p.errorText ?? null,
          }
        }
        if (p.type === "dse.compile") return { ...base, state: p.state ?? null, errorText: p.errorText ?? null }
        if (p.type === "dse.tool" || p.toolCallId) return { ...base, toolCallId: p.toolCallId ?? null, state: p.state ?? null }
        return base
      })
      const payload: Record<string, unknown> = {
        messageId: rawMessage.id ?? null,
        threadId,
        runId: rawMessage.runId ?? null,
        role: rawMessage.role ?? null,
        source: { kind: debug.sourceKind, reason: debug.sourceReason },
        model: debug.model,
        inference: debug.inference,
        signature: debug.primarySignature,
        signatures: debug.signatures,
        parts: partsSummary,
        l402Payments,
      }
      if (rawMessage.finish != null && typeof rawMessage.finish === "object") {
        payload.llm = rawMessage.finish
      }
      if (userId != null && userId !== "") payload.userId = userId
      return JSON.stringify(payload, null, 2)
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
      if (overlayDisposed) return
      type HomeRenderedMessage = {
        readonly id: string
        readonly role: "user" | "assistant"
        readonly renderParts: ReadonlyArray<RenderPart>
      }

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
            .map((m, i): HomeRenderedMessage => {
              const partsRaw = m.parts
              let renderParts = toAutopilotRenderParts({ parts: partsRaw, toolContractsByName: null })

              // If the run is streaming but no text has arrived yet, show a stable placeholder to avoid a blank bubble.
              if (
                m.role === "assistant" &&
                i === lastAssistantIndex &&
                homeSnapshot.status === "streaming" &&
                renderParts.length === 0
              ) {
                renderParts = [{ kind: "text" as const, text: "...", state: "streaming" as const }]
              }

              return {
                id: m.id,
                role: m.role,
                renderParts,
              }
            })
          : null

      latestL402Payments = collectL402PaymentsFromSnapshot()
      const l402PaymentsByMessageId = new Map<string, ReadonlyArray<L402PanePayment>>()
      for (const payment of latestL402Payments) {
        const arr = l402PaymentsByMessageId.get(payment.messageId) ?? []
        l402PaymentsByMessageId.set(payment.messageId, [...arr, payment])
      }

      const lastAssistantText =
        step === "authed" && renderedMessages
          ? (() => {
            for (let i = renderedMessages.length - 1; i >= 0; i--) {
              if (renderedMessages[i]?.role === "assistant") {
                return textFromRenderParts(renderedMessages[i].renderParts)
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
              const userText = textFromRenderParts(m.renderParts)
              return html`<div
                      class="text-sm font-mono text-white/55 text-left max-w-[80%] self-end"
                      data-chat-role="user"
                    >
                      ${userText}
                    </div>`
            }

            const rawMsg = homeSnapshot.messages.find((msg) => msg.id === m.id)
            const debugInfo = messageDebugInfo(rawMsg ?? { id: m.id, role: m.role, parts: [] })
            const hasDseSignaturePart = m.renderParts.some((p) => p.kind === "dse-signature")
            const partEls = m.renderParts.map((p) => {
              if (p.kind === "text") {
                return streamdown(p.text, {
                  mode: "streaming",
                  isAnimating: p.state === "streaming",
                  caret: "block",
                })
              }
              if (p.kind === "tool") return renderToolPart(p.model)
              if (p.kind === "payment-state") return renderPaymentStateCard(p.model)
              if (p.kind === "dse-signature") return showDebugCards ? renderDseSignatureCard(p.model) : html``
              if (p.kind === "dse-compile") return showDebugCards ? renderDseCompileCard(p.model) : html``
              if (p.kind === "dse-promote") return showDebugCards ? renderDsePromoteCard(p.model) : html``
              if (p.kind === "dse-rollback") return showDebugCards ? renderDseRollbackCard(p.model) : html``
              if (p.kind === "dse-budget-exceeded") return showDebugCards ? renderDseBudgetExceededCard(p.model) : html``
              return html``
            })
            const copyText = textFromRenderParts(m.renderParts)
            const messagePayments = l402PaymentsByMessageId.get(m.id) ?? []
            const paymentPayload = messagePayments.length > 0 ? messagePayments[messagePayments.length - 1] : null
            const metadataJson = messageMetadataJson(
              rawMsg ?? { id: m.id, role: m.role, parts: [], runId: null },
              homeThreadId
            )
            const paymentJson = paymentPayload ? JSON.stringify(paymentPayload, null, 2) : ""
            const debugFallbackCard =
              showDebugCards && !hasDseSignaturePart
                ? renderInferenceDebugCard({ messageId: m.id, info: debugInfo })
                : null
            return html`<div class="group text-sm font-mono text-white/90" data-chat-role="assistant">
                    <div class="flex flex-col gap-2">
                      ${partEls}
                      ${debugFallbackCard}
                    </div>
                    <span data-oa-copy-source style="display:none">${copyText}</span>
                    <span data-oa-message-metadata style="display:none">${metadataJson}</span>
                    <span data-oa-message-payment style="display:none">${paymentJson}</span>
                    <div class="mt-0.5 w-fit flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none group-hover:pointer-events-auto">
                      ${paymentPayload
                ? html`<button type="button" data-oa-home-chat-payment class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open payment detail">${rawHtml(BOLT_ICON_SVG)}</button>`
                : null}
                      <button type="button" data-oa-home-chat-telemetry class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open trace">${rawHtml(CHART_ICON_SVG)}</button>
                      <button type="button" data-oa-home-chat-copy class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Copy message">${rawHtml(COPY_ICON_SVG)}</button>
                      <button type="button" data-oa-home-chat-metadata class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open metadata">${rawHtml(METADATA_ICON_SVG)}</button>
                    </div>
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
              : html`<div class="group text-sm font-mono text-white/90" data-chat-role="assistant">
                        ${streamdown(m.text, { mode: "static" })}
                        ${showDebugCards
                ? renderInferenceDebugCard({
                  messageId: "inline-static",
                  info: messageDebugInfo({ id: null, role: m.role, parts: [], runId: null }),
                })
                : null}
                        <span data-oa-copy-source style="display:none">${m.text}</span>
                        <span data-oa-message-metadata style="display:none">${messageMetadataJson({ id: null, role: m.role, parts: [], runId: null }, homeThreadId)}</span>
                        <div class="mt-0.5 w-fit flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none group-hover:pointer-events-auto">
                          <button type="button" data-oa-home-chat-telemetry class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open trace">${rawHtml(CHART_ICON_SVG)}</button>
                          <button type="button" data-oa-home-chat-copy class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Copy message">${rawHtml(COPY_ICON_SVG)}</button>
                          <button type="button" data-oa-home-chat-metadata class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open metadata">${rawHtml(METADATA_ICON_SVG)}</button>
                        </div>
                      </div>`,
          )}
              </div>
            `

      const messagesContainer = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
      const savedScrollTop = messagesContainer instanceof HTMLElement ? messagesContainer.scrollTop : 0
      const messageCountBeforeRender =
        step === "authed"
          ? homeSnapshot.messages.length
          : messages.length

      runTrackedFiber({
        context: "home.chat.render_dom",
        start: () =>
          Effect.runFork(
            Effect.gen(function* () {
              const dom = yield* DomServiceTag
              yield* dom.render(
                paneContentSlot,
                html`
                  <div
                    class="flex flex-col h-full min-h-0 bg-black"
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
            }).pipe(
              Effect.provide(EffuseLive),
              Effect.tap(() =>
                Effect.sync(() => {
          const syncDebugButtonState = () => {
            if (!(paneDebugButton instanceof HTMLButtonElement)) return
            paneDebugButton.setAttribute("aria-pressed", showDebugCards ? "true" : "false")
            paneDebugButton.style.color = showDebugCards ? "rgba(255,255,255,0.92)" : "rgba(255,255,255,0.55)"
            paneDebugButton.style.opacity = showDebugCards ? "1" : "0.8"
          }

          if (!hasAddedPaneCopyButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const copyBtn = document.createElement("button")
              copyBtn.setAttribute("type", "button")
              copyBtn.setAttribute("aria-label", "Copy entire chat as markdown")
              copyBtn.innerHTML = COPY_ICON_SVG
              copyBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  copyTextToClipboard(getChatMarkdown(), "pane")
                  copyBtn.innerHTML = CHECKMARK_ICON_SVG
                  setTimeout(() => {
                    copyBtn.innerHTML = COPY_ICON_SVG
                  }, 1000)
                },
                { capture: true }
              )
              titleActions.appendChild(copyBtn)
              hasAddedPaneCopyButton = true
            }
          }
          if (!hasAddedPaneDebugButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const debugBtn = document.createElement("button")
              debugBtn.setAttribute("type", "button")
              debugBtn.setAttribute("aria-label", "Toggle debug cards")
              debugBtn.setAttribute("title", "Toggle debug cards")
              debugBtn.innerHTML = BUG_ICON_SVG
              debugBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              debugBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  showDebugCards = !showDebugCards
                  syncDebugButtonState()
                  doRender()
                },
                { capture: true }
              )
              titleActions.appendChild(debugBtn)
              paneDebugButton = debugBtn
              hasAddedPaneDebugButton = true
            }
          }
          syncDebugButtonState()

          if (!hasAddedPaneWalletButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const walletBtn = document.createElement("button")
              walletBtn.setAttribute("type", "button")
              walletBtn.setAttribute("aria-label", "Toggle L402 wallet pane")
              walletBtn.setAttribute("title", "Toggle L402 wallet pane")
              walletBtn.innerHTML = WALLET_ICON_SVG
              walletBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              walletBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  togglePersistentL402Pane({
                    paneId: L402_WALLET_PANE_ID,
                    kind: "l402-wallet",
                    title: "Wallet",
                    width: 460,
                    height: 320,
                  })
                },
                { capture: true },
              )
              titleActions.appendChild(walletBtn)
              paneWalletButton = walletBtn
              hasAddedPaneWalletButton = true
            }
          }

          if (!hasAddedPaneTransactionsButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const transactionsBtn = document.createElement("button")
              transactionsBtn.setAttribute("type", "button")
              transactionsBtn.setAttribute("aria-label", "Toggle L402 transactions pane")
              transactionsBtn.setAttribute("title", "Toggle L402 transactions pane")
              transactionsBtn.innerHTML = TRANSACTIONS_ICON_SVG
              transactionsBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              transactionsBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  togglePersistentL402Pane({
                    paneId: L402_TRANSACTIONS_PANE_ID,
                    kind: "l402-transactions",
                    title: "Transactions",
                    width: 560,
                    height: 420,
                  })
                },
                { capture: true },
              )
              titleActions.appendChild(transactionsBtn)
              paneTransactionsButton = transactionsBtn
              hasAddedPaneTransactionsButton = true
            }
          }

          if (!hasAddedPanePaywallsButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const paywallsBtn = document.createElement("button")
              paywallsBtn.setAttribute("type", "button")
              paywallsBtn.setAttribute("aria-label", "Toggle L402 paywalls pane")
              paywallsBtn.setAttribute("title", "Toggle L402 paywalls pane")
              paywallsBtn.innerHTML = BOLT_ICON_SVG
              paywallsBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              paywallsBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  togglePersistentL402Pane({
                    paneId: L402_PAYWALLS_PANE_ID,
                    kind: "l402-paywalls",
                    title: "L402 Paywalls",
                    width: 620,
                    height: 440,
                  })
                },
                { capture: true },
              )
              titleActions.appendChild(paywallsBtn)
              panePaywallsButton = paywallsBtn
              hasAddedPanePaywallsButton = true
            }
          }

          if (!hasAddedPaneSettlementsButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const settlementsBtn = document.createElement("button")
              settlementsBtn.setAttribute("type", "button")
              settlementsBtn.setAttribute("aria-label", "Toggle L402 settlements pane")
              settlementsBtn.setAttribute("title", "Toggle L402 settlements pane")
              settlementsBtn.innerHTML = TRANSACTIONS_ICON_SVG
              settlementsBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              settlementsBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  togglePersistentL402Pane({
                    paneId: L402_SETTLEMENTS_PANE_ID,
                    kind: "l402-settlements",
                    title: "L402 Settlements",
                    width: 620,
                    height: 440,
                  })
                },
                { capture: true },
              )
              titleActions.appendChild(settlementsBtn)
              paneSettlementsButton = settlementsBtn
              hasAddedPaneSettlementsButton = true
            }
          }

          if (!hasAddedPaneDeploymentsButton) {
            const titleActions = paneRoot.querySelector(`[data-pane-id="${CHAT_PANE_ID}"] [data-oa-pane-title-actions]`)
            if (titleActions instanceof HTMLElement) {
              const deploymentsBtn = document.createElement("button")
              deploymentsBtn.setAttribute("type", "button")
              deploymentsBtn.setAttribute("aria-label", "Toggle L402 deployments pane")
              deploymentsBtn.setAttribute("title", "Toggle L402 deployments pane")
              deploymentsBtn.innerHTML = CHART_ICON_SVG
              deploymentsBtn.style.transition = "color 120ms ease, opacity 120ms ease"
              deploymentsBtn.addEventListener(
                "pointerdown",
                (e) => {
                  if (e.button !== 0) return
                  e.preventDefault()
                  e.stopPropagation()
                  e.stopImmediatePropagation()
                  togglePersistentL402Pane({
                    paneId: L402_DEPLOYMENTS_PANE_ID,
                    kind: "l402-deployments",
                    title: "L402 Deployments",
                    width: 680,
                    height: 480,
                  })
                },
                { capture: true },
              )
              titleActions.appendChild(deploymentsBtn)
              paneDeploymentsButton = deploymentsBtn
              hasAddedPaneDeploymentsButton = true
            }
          }
          syncPaneActionButtonState()
          renderL402AuxPanes()

          const messagesEl = paneContentSlot.querySelector("[data-oa-home-chat-messages]")
          if (messagesEl instanceof HTMLElement) {
            messagesEl.scrollTop = savedScrollTop
            const hasMessages =
              step === "authed"
                ? homeSnapshot.messages.length > 0
                : messages.length > 0
            const messageCountAfterRender =
              step === "authed"
                ? homeSnapshot.messages.length
                : messages.length
            const hasNewMessages = messageCountAfterRender > previousRenderedMessageCount
            const shouldFollowLiveOutput =
              step === "authed" &&
              (homeSnapshot.status === "submitted" || homeSnapshot.status === "streaming")
            const shouldScrollToBottom =
              forceScrollToBottomOnNextRender ||
              shouldFollowLiveOutput ||
              hasNewMessages
            if (shouldScrollToBottom && hasMessages) {
              messagesEl.scrollTop = messagesEl.scrollHeight - messagesEl.clientHeight
              hasScrolledToBottomOnce = true
            } else if (!hasScrolledToBottomOnce && hasMessages) {
              messagesEl.scrollTop = messagesEl.scrollHeight - messagesEl.clientHeight
              hasScrolledToBottomOnce = true
            }
            previousRenderedMessageCount = messageCountAfterRender
            forceScrollToBottomOnNextRender = false
          } else {
            previousRenderedMessageCount = messageCountBeforeRender
            forceScrollToBottomOnNextRender = false
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
              if (!tid || !deps) return
              if (isRunningDseRecap) return
              isRunningDseRecap = true
              dseErrorText = null
              doRender()
              const e2eMode = String((globalThis as { readonly __OA_E2E_MODE?: unknown }).__OA_E2E_MODE ?? "") === "stub"
                ? "stub"
                : "off"
              runTrackedFiber({
                context: "home.dse.recap",
                start: () =>
                  deps.runtime.runFork(
                    Effect.gen(function* () {
                      const homeApi = yield* HomeApiService
                      return yield* homeApi.runDseRecap({
                        threadId: tid,
                        strategyId: dseStrategyId,
                        budgetProfile: dseBudgetProfile,
                        question: "Recap this thread.",
                        e2eMode,
                      })
                    }).pipe(
                      Effect.map(() => ({ ok: true as const })),
                      Effect.catchAll((error) => Effect.succeed({ ok: false as const, error })),
                    ),
                  ),
                onSuccess: (result) => {
                  isRunningDseRecap = false
                  if (!result.ok) {
                    const reason = homeApiRejectedReason(result.error)
                    dseErrorText = reason ? `DSE recap failed: ${reason}` : "DSE recap failed."
                  }
                  doRender()
                },
              })
            })
          }

          const stopBtn = paneContentSlot.querySelector("[data-oa-home-chat-stop]")
          if (stopBtn instanceof HTMLButtonElement) {
            stopBtn.addEventListener("click", () => {
              const tid = homeThreadId
              if (!tid || !deps?.chat) return
              runTrackedFiber({
                context: "home.chat.stop",
                start: () => deps.runtime.runFork(deps.chat.stop(tid)),
              })
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

          paneContentSlot.querySelectorAll("[data-oa-home-chat-copy]").forEach((btn) => {
            if (!(btn instanceof HTMLButtonElement)) return
            btn.addEventListener(
              "pointerdown",
              (e) => {
                if (e.button !== 0) return
                e.preventDefault()
                e.stopPropagation()
                e.stopImmediatePropagation()
                const block = btn.closest("[data-chat-role=\"assistant\"]")
                const sourceEl = block?.querySelector("[data-oa-copy-source]")
                const text = sourceEl?.textContent ?? ""
                copyTextToClipboard(text, "message")
                btn.innerHTML = CHECKMARK_ICON_SVG
                setTimeout(() => {
                  btn.innerHTML = COPY_ICON_SVG
                }, 1000)
              },
              { capture: true }
            )
          })

          paneContentSlot.querySelectorAll("[data-oa-home-chat-payment]").forEach((btn) => {
            if (!(btn instanceof HTMLElement)) return
            btn.addEventListener(
              "pointerdown",
              (e) => {
                if (e.button !== 0) return
                e.preventDefault()
                e.stopPropagation()
                e.stopImmediatePropagation()
                const block = btn.closest("[data-chat-role=\"assistant\"]")
                const paymentEl = block?.querySelector("[data-oa-message-payment]")
                let payload: unknown = null
                try {
                  payload = JSON.parse(paymentEl?.textContent ?? "null")
                } catch {
                  payload = null
                }
                const payment = parseL402PaymentPayload(payload)
                if (!payment) return
                openL402PaymentDetailPane(payment)
              },
              { capture: true },
            )
          })

          paneContentSlot.querySelectorAll("[data-oa-l402-approve]").forEach((btn) => {
            if (!(btn instanceof HTMLButtonElement)) return
            btn.addEventListener(
              "pointerdown",
              (e) => {
                if (e.button !== 0) return
                e.preventDefault()
                e.stopPropagation()
                e.stopImmediatePropagation()
                const taskId = String(btn.getAttribute("data-task-id") ?? "").trim()
                if (!taskId) return
                const text = `lightning_l402_approve(${JSON.stringify({ taskId })})`
                const tid = homeThreadId
                if (!deps?.chat) return
                forceScrollToBottomOnNextRender = true
                if (tid) {
                  runTrackedFiber({
                    context: "home.chat.l402_approve.existing_thread",
                    start: () => deps.runtime.runFork(deps.chat.send(tid, text)),
                  })
                  doRender()
                  return
                }

                runTrackedFiber({
                  context: "home.chat.get_owned_thread_id.before_l402_approve",
                  start: () => deps.runtime.runFork(deps.chat.getOwnedThreadId()),
                  onSuccess: (id) => {
                    if (id && id.length > 0) {
                      const userIdForCache = readSessionFromAtoms().userId ?? ""
                      attachHomeThreadSubscription({
                        threadId: id,
                        userId: userIdForCache,
                      })
                      runTrackedFiber({
                        context: "home.chat.l402_approve.newly_attached_thread",
                        start: () => deps.runtime.runFork(deps.chat.send(id, text)),
                      })
                    }
                    doRender()
                  },
                  onFailure: () => {
                    doRender()
                  },
                })
              },
              { capture: true },
            )
          })

          paneContentSlot.querySelectorAll("[data-oa-home-chat-metadata]").forEach((btn) => {
            if (!(btn instanceof HTMLElement)) return
            btn.addEventListener(
              "pointerdown",
              (e) => {
                if (e.button !== 0) return
                e.preventDefault()
                e.stopPropagation()
                e.stopImmediatePropagation()
                const block = btn.closest("[data-chat-role=\"assistant\"]")
                const metaEl = block?.querySelector("[data-oa-message-metadata]")
                let meta: Record<string, unknown> = {}
                try {
                  meta = JSON.parse(metaEl?.textContent ?? "{}") as Record<string, unknown>
                } catch {
                  meta = { parseError: "invalid JSON" }
                }
                const session = readSessionFromAtoms()
                if (session.userId) meta.userId = session.userId
                const paneId = `metadata-${Date.now()}`
                const screen = { width: paneRoot.clientWidth, height: paneRoot.clientHeight }
                const rect = calculateNewPanePosition(paneSystem.store.lastPanePosition, screen, 520, 360)
                paneSystem.store.addPane({
                  id: paneId,
                  kind: "metadata",
                  title: "Message metadata",
                  rect,
                  dismissable: true,
                })
                paneSystem.store.bringToFront(paneId)
                paneSystem.render()
                stylePaneOpaqueBlack(paneId)
                const slot = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`)
                const titleActions = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-title-actions]`)
                const metaJson = JSON.stringify(meta, null, 2)
                if (titleActions instanceof HTMLElement) {
                  const copyBtn = document.createElement("button")
                  copyBtn.setAttribute("type", "button")
                  copyBtn.setAttribute("aria-label", "Copy metadata")
                  copyBtn.innerHTML = COPY_ICON_SVG
                  copyBtn.addEventListener(
                    "pointerdown",
                    (e) => {
                      if (e.button !== 0) return
                      e.preventDefault()
                      e.stopPropagation()
                      e.stopImmediatePropagation()
                      copyTextToClipboard(metaJson, "metadata-pane")
                      copyBtn.innerHTML = CHECKMARK_ICON_SVG
                      setTimeout(() => {
                        copyBtn.innerHTML = COPY_ICON_SVG
                      }, 1000)
                    },
                    { capture: true }
                  )
                  titleActions.appendChild(copyBtn)
                }
                if (slot instanceof HTMLElement) {
                  runTrackedFiber({
                    context: "home.chat.metadata_pane.render",
                    start: () =>
                      Effect.runFork(
                        Effect.gen(function* () {
                          const dom = yield* DomServiceTag
                          yield* dom.render(
                            slot,
                            html`<div class="p-4 h-full overflow-auto bg-black"><pre class="text-xs font-mono text-white/80 whitespace-pre-wrap break-all">${metaJson}</pre></div>`
                          )
                        }).pipe(Effect.provide(EffuseLive))
                      ),
                  })
                }
              },
              { capture: true }
            )
          })

          paneContentSlot.querySelectorAll("[data-oa-home-chat-telemetry]").forEach((btn) => {
            if (!(btn instanceof HTMLElement)) return
            btn.addEventListener(
              "pointerdown",
              (e) => {
                if (e.button !== 0) return
                e.preventDefault()
                e.stopPropagation()
                e.stopImmediatePropagation()
                const trace = getThreadTrace()
                const paneId = `telemetry-${Date.now()}`
                const screen = { width: paneRoot.clientWidth, height: paneRoot.clientHeight }
                const rect = calculateNewPanePosition(paneSystem.store.lastPanePosition, screen, 560, 400)
                paneSystem.store.addPane({
                  id: paneId,
                  kind: "telemetry",
                  title: "Trace",
                  rect,
                  dismissable: true,
                })
                paneSystem.store.bringToFront(paneId)
                paneSystem.render()
                stylePaneOpaqueBlack(paneId)
                const slot = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`)
                if (slot instanceof HTMLElement) {
                  const traceJson = JSON.stringify(trace, null, 2)
                  runTrackedFiber({
                    context: "home.chat.telemetry_pane.render",
                    start: () =>
                      Effect.runFork(
                        Effect.gen(function* () {
                          const dom = yield* DomServiceTag
                          yield* dom.render(
                            slot,
                            html`<div class="p-4 h-full overflow-auto bg-black"><pre class="text-xs font-mono text-white/80 whitespace-pre-wrap break-all">${traceJson}</pre></div>`
                          )
                        }).pipe(Effect.provide(EffuseLive))
                      ),
                  })
                }
              },
              { capture: true }
            )
          })

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
              forceScrollToBottomOnNextRender = true
              const ensureThenSend = () => {
                const tid = homeThreadId
                if (tid) {
                  if (input) input.value = ""
                  runTrackedFiber({
                    context: "home.chat.send.existing_thread",
                    start: () => deps.runtime.runFork(deps.chat.send(tid, text)),
                  })
                  doRender()
                  return
                }
                runTrackedFiber({
                  context: "home.chat.get_owned_thread_id.before_send",
                  start: () => deps.runtime.runFork(deps.chat.getOwnedThreadId()),
                  onSuccess: (id) => {
                    if (id && id.length > 0) {
                      const userIdForCache = readSessionFromAtoms().userId ?? ""
                      attachHomeThreadSubscription({
                        threadId: id,
                        userId: userIdForCache,
                      })
                      if (input) input.value = ""
                      runTrackedFiber({
                        context: "home.chat.send.newly_attached_thread",
                        start: () => deps.runtime.runFork(deps.chat.send(id, text)),
                      })
                    }
                    doRender()
                  },
                  onFailure: () => {
                    doRender()
                  },
                })
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
              if (!deps) {
                isBusy = false
                messages.push({ role: "assistant", text: "Failed to send code. Try again." })
                doRender()
                return
              }
              runTrackedFiber({
                context: "home.auth.start_magic_code",
                start: () =>
                  deps.runtime.runFork(
                    Effect.gen(function* () {
                      const homeApi = yield* HomeApiService
                      yield* homeApi.startMagicCode({ email: nextEmail })
                      return { ok: true as const }
                    }).pipe(
                      Effect.catchAll((error) => Effect.succeed({ ok: false as const, error })),
                    ),
                  ),
                onSuccess: (result) => {
                  isBusy = false
                  if (result.ok) {
                    email = nextEmail
                    step = "code"
                    messages.push({
                      role: "assistant",
                      text: `Check ${email}. Enter six digit verification code:`,
                    })
                  } else {
                    messages.push({
                      role: "assistant",
                      text: startCodeErrorMessage(result.error),
                    })
                  }
                  doRender()
                },
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
            if (!deps) {
              isBusy = false
              messages.push({ role: "assistant", text: "Verification failed. Try again." })
              doRender()
              return
            }
            runTrackedFiber({
              context: "home.auth.verify_magic_code",
              start: () =>
                deps.runtime.runFork(
                  Effect.gen(function* () {
                    const homeApi = yield* HomeApiService
                    const verified = yield* homeApi.verifyMagicCode({ email, code })
                    return { ok: true as const, verified }
                  }).pipe(
                    Effect.catchAll((error) => Effect.succeed({ ok: false as const, error })),
                  ),
                ),
              onSuccess: (result) => {
                isBusy = false
                if (!result.ok) {
                  messages.push({
                    role: "assistant",
                    text: verifyCodeErrorMessage(result.error),
                  })
                  doRender()
                  return
                }

                clearAuthClientCache()
                const token = result.verified.token
                const userPayload = result.verified.user
                const userId = result.verified.userId ?? userPayload?.id ?? null

                if (token && userPayload && userId) {
                  startAuthedChat({
                    userId,
                    user: {
                      id: String(userPayload.id),
                      email: userPayload.email ?? null,
                      firstName: userPayload.firstName ?? null,
                      lastName: userPayload.lastName ?? null,
                    },
                    token,
                  })
                  return
                }

                messages.push({ role: "assistant", text: "Signed in, but couldn't initialize chat. Please try again." })
                step = "authed"
                doRender()
              },
            })
          })
          const inputEl0 = paneContentSlot.querySelector("[data-oa-home-chat-input]")
          const inputEl = inputEl0 instanceof HTMLInputElement ? inputEl0 : null
          if (inputEl) requestAnimationFrame(() => inputEl.focus())
                }),
              ),
            ),
          ),
      })
    }

    if (isAuthedFromAtoms && deps?.atoms) {
      const current = readSessionFromAtoms()
      startAuthedChat({ userId: current.userId ?? "", user: current.user ?? null, token: null })
    } else if (!isAuthedFromAtoms && deps?.atoms) {
      // If the user already has a valid session cookie (e.g. E2E bypass login), detect it
      // so the home overlay doesn't force re-entering email.
      runTrackedFiber({
        context: "home.auth.session_probe",
        start: () =>
          deps.runtime.runFork(
            Effect.gen(function* () {
              const homeApi = yield* HomeApiService
              const session = yield* homeApi.getAuthSession()
              return { ok: true as const, session }
            }).pipe(
              Effect.catchAll((error) => Effect.succeed({ ok: false as const, error })),
            ),
          ),
        onSuccess: (result) => {
          if (!result.ok) {
            logHomeControllerAsyncError("home.auth.session_probe", result.error)
            return
          }
          if (!result.session) return
          startAuthedChat({
            userId: result.session.userId,
            user: result.session.user,
            token: result.session.token,
          })
        },
      })
    }

    doRender()
  }

  const onGlobalEnter = (ev: KeyboardEvent): void => {
    if (ev.key !== "Enter") return
    if (ev.defaultPrevented || ev.repeat) return
    if (ev.metaKey || ev.ctrlKey || ev.altKey || ev.shiftKey) return
    if (isTextEntryTarget(ev.target)) return

    const shell = container.querySelector("[data-marketing-shell]")
    if (shell instanceof HTMLElement && shell.getAttribute("data-oa-home-chat-open") === "1") return

    if (!(trigger instanceof HTMLElement)) return
    if (!trigger.isConnected || trigger.getClientRects().length === 0) return

    ev.preventDefault()
    ev.stopPropagation()
    trigger.dispatchEvent(
      new MouseEvent("click", {
        bubbles: true,
        cancelable: true,
        view: window,
      }),
    )
  }

  trigger.addEventListener("click", handler, { capture: true })
  window.addEventListener("keydown", onGlobalEnter, { capture: true })

  return () => {
    activeOverlayTeardown?.()
    activeOverlayTeardown = null
    trigger.removeEventListener("click", handler, { capture: true })
    window.removeEventListener("keydown", onGlobalEnter, { capture: true })
  }
}
