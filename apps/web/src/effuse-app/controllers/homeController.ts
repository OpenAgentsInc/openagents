import { Effect } from "effect"
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
} from "../../effect/auth"
import type { ChatSnapshot } from "../../effect/chat"
import { ChatSnapshotAtom } from "../../effect/atoms/chat"
import { SessionAtom, type Session } from "../../effect/atoms/session"
import { ChatSnapshotCacheLive, ChatSnapshotCacheService } from "../../effect/chatSnapshotCache"
import { PaneSystemLive, PaneSystemService } from "../../effect/paneSystem"
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
  type RenderPart,
} from "../../effuse-pages/autopilot"
import { streamdown } from "../../lib/effuseStreamdown"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { ChatClient } from "../../effect/chat"
import type { AppRuntime } from "../../effect/runtime"
import { toAutopilotRenderParts } from "./autopilotChatParts"

export type HomeController = {
  readonly cleanup: () => void
}

type SessionState = {
  readonly read: () => Session
  readonly write: (session: Session) => void
}

type HomeChatDeps = {
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly sessionState: SessionState
  readonly navigate: (href: string) => void
  readonly signOut: () => void | Promise<void>
  readonly chat: ChatClient
  readonly refreshConvexAuth?: () => void | Promise<void>
}

const CHAT_PANE_ID = "home-chat"
const HOME_CHAT_PANE_RECT_STORAGE_KEY = "oa.home.chat.paneRect.v1"
const HOME_CHAT_SNAPSHOT_CACHE_MAX_AGE_MS = 1000 * 60 * 60 * 24

const COPY_ICON_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>'

const CHECKMARK_ICON_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>'

const CHART_ICON_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>'

const BUG_ICON_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2h8"/><path d="M9 2v2"/><path d="M15 2v2"/><path d="M8 6h8"/><rect x="7" y="6" width="10" height="12" rx="5"/><path d="M3 13h4"/><path d="M17 13h4"/><path d="M5 8l3 2"/><path d="M19 8l-3 2"/><path d="M5 18l3-2"/><path d="M19 18l-3-2"/></svg>'

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null

type StoredPaneRect = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value)

const clampPaneRectToScreen = (rect: StoredPaneRect, screen: { readonly width: number; readonly height: number }): StoredPaneRect => {
  const width = Math.max(320, Math.min(rect.width, Math.max(320, screen.width)))
  const height = Math.max(220, Math.min(rect.height, Math.max(220, screen.height)))
  const maxX = Math.max(0, screen.width - width)
  const maxY = Math.max(0, screen.height - height)
  const x = Math.max(0, Math.min(rect.x, maxX))
  const y = Math.max(0, Math.min(rect.y, maxY))
  return { x, y, width, height }
}

const parseStoredPaneRect = (value: unknown): StoredPaneRect | null => {
  const rec = asRecord(value)
  if (!rec) return null
  const { x, y, width, height } = rec
  if (!isFiniteNumber(x) || !isFiniteNumber(y) || !isFiniteNumber(width) || !isFiniteNumber(height)) return null
  if (width <= 0 || height <= 0) return null
  return { x, y, width, height }
}

const readStoredHomeChatPaneRect = (screen: { readonly width: number; readonly height: number }): StoredPaneRect | null => {
  if (typeof window === "undefined") return null
  try {
    const raw = window.localStorage.getItem(HOME_CHAT_PANE_RECT_STORAGE_KEY)
    if (!raw) return null
    const parsed = parseStoredPaneRect(JSON.parse(raw))
    if (!parsed) return null
    return clampPaneRectToScreen(parsed, screen)
  } catch {
    return null
  }
}

const writeStoredHomeChatPaneRect = (rect: StoredPaneRect): void => {
  if (typeof window === "undefined") return
  try {
    window.localStorage.setItem(HOME_CHAT_PANE_RECT_STORAGE_KEY, JSON.stringify(rect))
  } catch {
    // ignore storage failures
  }
}

function copyTextToClipboard(text: string, _source: "pane" | "message" | "metadata-pane"): void {
  if (!text || typeof text !== "string") return
  const execCopy = (): boolean => {
    const ta = document.createElement("textarea")
    ta.value = text
    ta.style.position = "fixed"
    ta.style.left = "0"
    ta.style.top = "0"
    ta.style.width = "2px"
    ta.style.height = "2px"
    ta.style.padding = "0"
    ta.style.border = "none"
    ta.style.outline = "none"
    ta.style.boxShadow = "none"
    ta.style.background = "transparent"
    ta.style.opacity = "0.01"
    ta.style.zIndex = "-1"
    document.body.appendChild(ta)
    ta.focus()
    ta.setSelectionRange(0, text.length)
    let ok = false
    try {
      ok = document.execCommand("copy")
    } catch {
      // ignore
    }
    document.body.removeChild(ta)
    return ok
  }
  if (typeof navigator !== "undefined" && navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
    navigator.clipboard.writeText(text).then(
      () => { },
      () => {
        execCopy()
      }
    )
  } else {
    execCopy()
  }
}

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
    const storedRect = readStoredHomeChatPaneRect(screen)
    const rect = storedRect ?? calculateNewPanePosition(undefined, screen, 640, 480)
    let closeOverlay = (): void => { }
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
      },
    } as const

    const runPaneSystemEffectSync = <A>(effect: Effect.Effect<A, never, PaneSystemService>): A => {
      if (deps?.runtime) return deps.runtime.runSync(effect)
      return Effect.runSync(effect.pipe(Effect.provide(PaneSystemLive)))
    }

    const runChatSnapshotCacheEffectSync = <A>(effect: Effect.Effect<A, never, ChatSnapshotCacheService>): A => {
      if (deps?.runtime) return deps.runtime.runSync(effect)
      return Effect.runSync(effect.pipe(Effect.provide(ChatSnapshotCacheLive)))
    }

    const readCachedSnapshotForUser = (userId: string): { readonly threadId: string; readonly snapshot: ChatSnapshot } | null => {
      if (!userId) return null
      const cached = runChatSnapshotCacheEffectSync(
        Effect.gen(function* () {
          const cache = yield* ChatSnapshotCacheService
          return yield* cache.readLatestForUser({
            userId,
            maxAgeMs: HOME_CHAT_SNAPSHOT_CACHE_MAX_AGE_MS,
          })
        }),
      )
      if (!cached) return null
      return { threadId: cached.threadId, snapshot: cached.snapshot }
    }

    const writeCachedSnapshotForUser = (userId: string, threadId: string, snapshot: ChatSnapshot): void => {
      if (!userId || !threadId) return
      runChatSnapshotCacheEffectSync(
        Effect.gen(function* () {
          const cache = yield* ChatSnapshotCacheService
          yield* cache.writeLatestForUser({ userId, threadId, snapshot })
        }),
      )
    }

    const clearCachedSnapshotForUser = (userId: string): void => {
      if (!userId) return
      runChatSnapshotCacheEffectSync(
        Effect.gen(function* () {
          const cache = yield* ChatSnapshotCacheService
          yield* cache.clearForUser(userId)
        }),
      )
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
      const closedRectRaw = paneSystem.store.closedPositions.get(CHAT_PANE_ID)?.rect ?? paneSystem.store.pane(CHAT_PANE_ID)?.rect
      const closedRect = parseStoredPaneRect(closedRectRaw)
      const currentScreen =
        typeof window !== "undefined"
          ? { width: window.innerWidth, height: window.innerHeight }
          : screen
      if (closedRect) writeStoredHomeChatPaneRect(clampPaneRectToScreen(closedRect, currentScreen))
      releasePaneSystemSync()
      overlay.remove()
      hideStyle.remove()
      shell.removeAttribute("data-oa-home-chat-open")
    }

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
            const sessionUserId = readSessionFromAtoms().userId ?? ""
            clearCachedSnapshotForUser(sessionUserId)
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
    let hasAddedPaneCopyButton = false
    let hasAddedPaneDebugButton = false
    let showDebugCards = false
    let paneDebugButton: HTMLButtonElement | null = null
    let previousRenderedMessageCount = 0
    let forceScrollToBottomOnNextRender = false

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
      if (unsubHomeChat) unsubHomeChat()

      let skippedHydratedPlaceholder = false
      const hydratedSnapshot = input.hydratedSnapshot ?? null
      unsubHomeChat = deps.atoms.subscribe(
        ChatSnapshotAtom(threadId),
        (snap) => {
          const shouldSkipHydratedPlaceholder =
            !skippedHydratedPlaceholder &&
            hydratedSnapshot != null &&
            hydratedSnapshot.messages.length > 0 &&
            snap.messages.length === 0 &&
            snap.status === "ready" &&
            snap.errorText == null
          if (shouldSkipHydratedPlaceholder) {
            skippedHydratedPlaceholder = true
            return
          }
          homeSnapshot = snap
          writeCachedSnapshotForUser(input.userId, threadId, snap)
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

      void Promise.resolve(deps.refreshConvexAuth?.()).catch(() => { })

      if (input0.user?.email) renderIdentityCard(input0.user.email)

      const cached = readCachedSnapshotForUser(input0.userId)
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

      deps.runtime.runPromise(deps.chat.getOwnedThreadId()).then(
        (id) => {
          if (id && id.length > 0) {
            attachHomeThreadSubscription({
              threadId: id,
              userId: input0.userId,
              hydratedSnapshot: cached?.threadId === id ? cached.snapshot : null,
            })
          }
          doRender()
        },
        () => doRender(),
      )
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
              if (p.kind === "dse-signature") return showDebugCards ? renderDseSignatureCard(p.model) : html``
              if (p.kind === "dse-compile") return showDebugCards ? renderDseCompileCard(p.model) : html``
              if (p.kind === "dse-promote") return showDebugCards ? renderDsePromoteCard(p.model) : html``
              if (p.kind === "dse-rollback") return showDebugCards ? renderDseRollbackCard(p.model) : html``
              if (p.kind === "dse-budget-exceeded") return showDebugCards ? renderDseBudgetExceededCard(p.model) : html``
              return html``
            })
            const copyText = textFromRenderParts(m.renderParts)
            const metadataJson = messageMetadataJson(
              rawMsg ?? { id: m.id, role: m.role, parts: [], runId: null },
              homeThreadId
            )
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
                    <div class="mt-0.5 w-fit flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none group-hover:pointer-events-auto">
                      <button type="button" data-oa-home-chat-telemetry class="inline-flex items-center justify-center p-0.5 text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 rounded" aria-label="Open trace">${rawHtml(CHART_ICON_SVG)}</button>
                      <button type="button" data-oa-home-chat-copy class="text-[11px] font-mono text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer p-0 focus:outline-none focus-visible:underline">Copy</button>
                      <button type="button" data-oa-home-chat-metadata class="text-[11px] font-mono text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer p-0 focus:outline-none focus-visible:underline">Metadata</button>
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
                          <button type="button" data-oa-home-chat-copy class="text-[11px] font-mono text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer p-0 focus:outline-none focus-visible:underline">Copy</button>
                          <button type="button" data-oa-home-chat-metadata class="text-[11px] font-mono text-white/50 hover:text-white/70 bg-transparent border-0 cursor-pointer p-0 focus:outline-none focus-visible:underline">Metadata</button>
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

      Effect.runPromise(
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
        }).pipe(Effect.provide(EffuseLive)),
      ).then(
        () => {
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
              if (!tid) return
              if (isRunningDseRecap) return
              isRunningDseRecap = true
              dseErrorText = null
              doRender()
              fetch("/api/autopilot/dse/recap", {
                method: "POST",
                headers: {
                  "content-type": "application/json",
                  ...(String((globalThis as { readonly __OA_E2E_MODE?: unknown }).__OA_E2E_MODE ?? "") === "stub"
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

          paneContentSlot.querySelectorAll("[data-oa-home-chat-copy]").forEach((btn) => {
            if (!(btn instanceof HTMLElement)) return
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
                btn.textContent = "Copied"
                setTimeout(() => {
                  btn.textContent = "Copy"
                }, 1000)
              },
              { capture: true }
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
                const metadataPaneEl = paneRoot.querySelector(`[data-pane-id="${paneId}"]`)
                if (metadataPaneEl instanceof HTMLElement) metadataPaneEl.style.background = "#000"
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
                  Effect.runPromise(
                    Effect.gen(function* () {
                      const dom = yield* DomServiceTag
                      yield* dom.render(
                        slot,
                        html`<div class="p-4 h-full overflow-auto bg-black"><pre class="text-xs font-mono text-white/80 whitespace-pre-wrap break-all">${metaJson}</pre></div>`
                      )
                    }).pipe(Effect.provide(EffuseLive))
                  ).catch(() => { })
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
                const slot = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`)
                if (slot instanceof HTMLElement) {
                  const traceJson = JSON.stringify(trace, null, 2)
                  Effect.runPromise(
                    Effect.gen(function* () {
                      const dom = yield* DomServiceTag
                      yield* dom.render(
                        slot,
                        html`<div class="p-4 h-full overflow-auto"><pre class="text-xs font-mono text-white/80 whitespace-pre-wrap break-all">${traceJson}</pre></div>`
                      )
                    }).pipe(Effect.provide(EffuseLive))
                  ).catch(() => { })
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
                  deps.runtime.runPromise(deps.chat.send(tid, text)).catch(() => { })
                  doRender()
                  return
                }
                deps.runtime.runPromise(deps.chat.getOwnedThreadId()).then(
                  (id) => {
                    if (id && id.length > 0) {
                      const userIdForCache = readSessionFromAtoms().userId ?? ""
                      attachHomeThreadSubscription({
                        threadId: id,
                        userId: userIdForCache,
                      })
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
              .then((data) => {
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
      const current = readSessionFromAtoms()
      startAuthedChat({ userId: current.userId ?? "", user: current.user ?? null, token: null })
    } else if (!isAuthedFromAtoms && deps?.atoms) {
      // If the user already has a valid session cookie (e.g. E2E bypass login), detect it
      // so the home overlay doesn't force re-entering email.
      type AuthSessionResponse = {
        readonly ok?: boolean
        readonly userId?: string
        readonly token?: string
        readonly user?: {
          readonly id: string
          readonly email?: string | null
          readonly firstName?: string | null
          readonly lastName?: string | null
        } | null
      }
      fetch("/api/auth/session", { method: "GET", cache: "no-store", credentials: "include" })
        .then((r) => r.json().catch(() => null) as Promise<AuthSessionResponse | null>)
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
    trigger.removeEventListener("click", handler, { capture: true })
    window.removeEventListener("keydown", onGlobalEnter, { capture: true })
  }
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
  let deps: HomeChatDeps | undefined
  if (input.runtime && input.atoms && input.navigate && input.signOut && input.chat) {
    const atoms = input.atoms
    deps = {
      runtime: input.runtime,
      atoms,
      sessionState: {
        read: () => atoms.get(SessionAtom),
        write: (session) => atoms.set(SessionAtom, session),
      },
      navigate: input.navigate,
      signOut: input.signOut,
      chat: input.chat,
      refreshConvexAuth: input.refreshConvexAuth,
    }
  }
  const stopOpenChatPane = openChatPaneOnHome(input.container, deps)

  return {
    cleanup: () => {
      stopCountdown()
      stopOpenChatPane()
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}
