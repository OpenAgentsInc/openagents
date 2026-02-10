import { Effect } from "effect"
import type { EzAction, RouterService } from "@openagentsinc/effuse"

import { cleanupMarketingDotsGridBackground, hydrateMarketingDotsGridBackground } from "../../effuse-pages/marketingShell"
import { runAutopilotRoute } from "../../effuse-pages/autopilotRoute"
import { AutopilotChatIsAtBottomAtom, ChatSnapshotAtom, OwnedThreadIdAtom } from "../../effect/atoms/chat"
import { AutopilotSidebarCollapsedAtom, AutopilotSidebarUserMenuOpenAtom } from "../../effect/atoms/autopilotUi"
import { SessionAtom } from "../../effect/atoms/session"
import { clearAuthClientCache } from "../../effect/auth"
import { AuthService } from "../../effect/auth"
import { toAutopilotRenderParts } from "./autopilotChatParts"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { AutopilotStore } from "../../effect/autopilotStore"
import type { ContractsApi, ToolContract } from "../../effect/contracts"
import type { ChatClient } from "../../effect/chat"
import type { AppRuntime } from "../../effect/runtime"
import type { TelemetryClient } from "../../effect/telemetry"
import type { ChatMessage } from "../../effect/chatProtocol"
import type { AutopilotRouteRenderInput } from "../../effuse-pages/autopilotRoute"
import type { RenderedMessage as EffuseRenderedMessage } from "../../effuse-pages/autopilot"

function normalizeEmail(raw: string): string {
  return raw.trim().toLowerCase()
}

function normalizeCode(raw: string): string {
  return raw.replace(/\s+/g, "")
}

export type AutopilotController = {
  readonly cleanup: () => void
}

export const mountAutopilotController = (input: {
  readonly container: Element
  readonly ez: Map<string, EzAction>
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly telemetry: TelemetryClient
  readonly store: AutopilotStore
  readonly contracts: ContractsApi
  readonly chat: ChatClient
  readonly router: RouterService<any>
  readonly navigate: (href: string) => void
}): AutopilotController => {
  let disposed = false
  let focusedOnMount = false

  const atoms = input.atoms

  const initialSession = atoms.get(SessionAtom)
  void initialSession
  let ownedThreadId = atoms.get(OwnedThreadIdAtom)
  let chatId = initialSession.userId ? (ownedThreadId ?? "") : ""
  // When session becomes authed but OwnedThreadId isn't resolved yet, actions must surface errors
  // even before ChatSnapshotAtom(chatId) is wired. This prevents "silent stall" on Send.
  let chatInitErrorText: string | null = null

  // Atom-backed UI state
  let session = initialSession
  let _collapsed = atoms.get(AutopilotSidebarCollapsedAtom)
  let userMenuOpen = atoms.get(AutopilotSidebarUserMenuOpenAtom)
  let chatSnapshot = chatId ? atoms.get(ChatSnapshotAtom(chatId)) : { messages: [], status: "ready" as const, errorText: null }
  let isAtBottom = chatId ? atoms.get(AutopilotChatIsAtBottomAtom(chatId)) : true

  // Local controller state (was React useState/useRef)
  type AuthStep = "closed" | "email" | "code"
  let authStep: AuthStep = "closed"
  let authEmail = ""
  let authCode = ""
  let authBusy = false
  let authErrorText: string | null = null

  let isExportingBlueprint = false
  let isResettingAgent = false
  let dseStrategyId: "direct.v1" | "rlm_lite.v1" = "direct.v1"
  let dseBudgetProfile: "small" | "medium" | "long" = "medium"
  let isRunningDseRecap = false
  let dseErrorText: string | null = null
  let blueprint: unknown | null = null
  // When switching from Raw -> Form without saving, we need to preserve unknown keys
  // from the raw draft. Store a parsed base blueprint here and apply form edits on top.
  let blueprintEditBase: unknown | null = null
  let blueprintError: string | null = null
  let blueprintLoading = false
  let blueprintUpdatedAt: number | null = null
  let blueprintMode: "form" | "raw" = "form"
  let isSavingBlueprint = false
  // Slot key freezing: once the user starts typing, keep blueprint slot stable even if
  // background blueprint polling updates blueprintUpdatedAt (avoid clobbering caret).
  let blueprintKeyFrozenAt: number | null = null
  type BlueprintDraft = {
    userHandle: string
    agentName: string
    identityVibe: string
    characterVibe: string
    characterBoundaries: string
  }
  let blueprintDraft: BlueprintDraft | null = null
  let blueprintDraftDirty = false
  let blueprintRawDraft: string | null = null
  let blueprintRawDirty = false
  let blueprintRawError: string | null = null
  let toolContractsByName: Record<string, ToolContract> | null = null

  const makeDraftFromBlueprint = (value: unknown): BlueprintDraft => {
    const b: any = value ?? {}
    const docs = b?.docs ?? {}
    const identity = docs.identity ?? {}
    const user = docs.user ?? {}
    const character = docs.character ?? {}

    const boundaries: string = Array.isArray(character.boundaries)
      ? character.boundaries
          .map((s: unknown) => (typeof s === "string" ? s : ""))
          .filter(Boolean)
          .join("\n")
      : ""

    return {
      userHandle: typeof user.addressAs === "string" ? user.addressAs : "",
      agentName: typeof identity.name === "string" ? identity.name : "",
      identityVibe: typeof identity.vibe === "string" ? identity.vibe : "",
      characterVibe: typeof character.vibe === "string" ? character.vibe : "",
      characterBoundaries: boundaries,
    }
  }

  const cloneJson = (value: unknown): any => {
    try {
      return JSON.parse(JSON.stringify(value ?? {}))
    } catch {
      return {}
    }
  }

  const ensureRecord = (obj: any, key: string): any => {
    if (!obj || typeof obj !== "object" || Array.isArray(obj)) return {}
    const existing = obj[key]
    if (existing && typeof existing === "object" && !Array.isArray(existing)) return existing
    const created: Record<string, unknown> = {}
    obj[key] = created
    return created
  }

  const applyDraftFields = (base: unknown, draft: BlueprintDraft): any => {
    const next = cloneJson(base)
    const nowBoundaries = draft.characterBoundaries
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean)

    const docs = ensureRecord(next, "docs")
    const user = ensureRecord(docs, "user")
    const identity = ensureRecord(docs, "identity")
    const character = ensureRecord(docs, "character")

    const nextHandle = draft.userHandle.trim()
    if (nextHandle) user.addressAs = nextHandle

    const nextName = draft.agentName.trim()
    if (nextName) identity.name = nextName

    const nextIdentityVibe = draft.identityVibe.trim()
    if (nextIdentityVibe) identity.vibe = nextIdentityVibe

    const nextCharacterVibe = draft.characterVibe.trim()
    if (nextCharacterVibe) character.vibe = nextCharacterVibe

    character.boundaries = nowBoundaries

    return next
  }

  const applySaveMetadata = (next: any, nowIso: string) => {
    if (!next || typeof next !== "object" || Array.isArray(next)) return
    next.exportedAt = nowIso

    const docs = ensureRecord(next, "docs")
    const bump = (section: any) => {
      if (!section || typeof section !== "object" || Array.isArray(section)) return
      section.updatedAt = nowIso
      section.updatedBy = "user"
      section.version = Number(section.version ?? 1) + 1
    }

    bump(ensureRecord(docs, "user"))
    bump(ensureRecord(docs, "identity"))
    bump(ensureRecord(docs, "character"))
  }

  // Draft input in a ref-like variable so keystrokes don't force rerenders.
  let inputDraft = ""

  let lastTailKey = ""
  let renderScheduled = false

  const scrollToBottom = (behavior: ScrollBehavior = "auto") => {
    const bottom = document.querySelector("[data-autopilot-bottom]")
    ;(bottom as HTMLElement | null)?.scrollIntoView({ block: "end", behavior })
  }

  const focusChatInput = (options?: { readonly force?: boolean }) => {
    const force = options?.force ?? false
    const active = document.activeElement
    if (!force && active && active !== document.body && active !== document.documentElement) {
      const chatForm = input.container.querySelector("#chat-form")
      // If focus is outside the chat UI (e.g. blueprint editor), don't steal focus.
      if (!chatForm || !chatForm.contains(active)) return
    }

    const inputEl =
      input.container.querySelector<HTMLInputElement>('[data-autopilot-chat-input="1"]') ??
      input.container.querySelector<HTMLInputElement>('input[name="message"]')
    if (!inputEl || inputEl.disabled) return

    try {
      inputEl.focus({ preventScroll: true })
    } catch {
      try {
        inputEl.focus()
      } catch {
        // ignore
      }
    }

    // Keep the caret at the end (useful when we restore focus after send).
    try {
      const len = inputEl.value.length
      inputEl.setSelectionRange(len, len)
    } catch {
      // ignore
    }
  }

  const scheduleRender = () => {
    if (renderScheduled) return
    renderScheduled = true
    queueMicrotask(() => {
      renderScheduled = false
      renderNow()
    })
  }

  const renderNow = () => {
    const isStreaming = chatSnapshot.status === "streaming"
    const isBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"

    const messages = chatSnapshot.messages

    let renderedMessages: Array<EffuseRenderedMessage> = messages
      .filter(
        (m): m is ChatMessage & { readonly role: "user" | "assistant" } =>
          m.role === "user" || m.role === "assistant",
      )
      .map((msg) => {
        const parts = Array.isArray((msg as any).parts)
          ? ((msg as any).parts as ReadonlyArray<ChatMessage["parts"][number]>)
          : []
        const renderParts = toAutopilotRenderParts({ parts, toolContractsByName })
        return { id: msg.id, role: msg.role, renderParts }
      })
      .filter((m) => m.role === "user" || m.renderParts.length > 0)

    const showWelcomeMessage =
      renderedMessages.length === 0 &&
      typeof window !== "undefined" &&
      window.location.search.includes("welcome=1")
    if (showWelcomeMessage) {
      renderedMessages = [
        {
          id: "welcome-initial",
          role: "assistant",
          renderParts: [{ kind: "text", text: "Autopilot online." }],
        },
        ...renderedMessages,
      ]
    }

    const renderedTailKey = (() => {
      const last = renderedMessages.at(-1)
      if (!last) return ""
      const lastPart = last.renderParts.at(-1)
      if (!lastPart) return `${renderedMessages.length}:${last.id}`
      if (lastPart.kind === "text") {
        return `${renderedMessages.length}:${last.id}:text:${lastPart.text.length}:${lastPart.state ?? ""}`
      }
      if (lastPart.kind === "tool") {
        const outputLen = lastPart.model.details?.output?.preview?.length ?? 0
        const errorLen = lastPart.model.details?.error?.preview?.length ?? 0
        return `${renderedMessages.length}:${last.id}:tool:${lastPart.model.toolName}:${lastPart.model.status}:${lastPart.model.summary}:${outputLen}:${errorLen}`
      }
      if (lastPart.kind === "dse-signature") {
        const previewLen = lastPart.model.outputPreview?.preview?.length ?? 0
        const errLen = lastPart.model.errorText?.preview?.length ?? 0
        const strategy = lastPart.model.strategyId ?? ""
        const traceLen = lastPart.model.rlmTrace?.preview?.length ?? 0
        return `${renderedMessages.length}:${last.id}:dse-signature:${lastPart.model.signatureId}:${lastPart.model.state}:${lastPart.model.compiled_id ?? ""}:${strategy}:${previewLen}:${errLen}:${traceLen}`
      }
      if (lastPart.kind === "dse-compile") {
        return `${renderedMessages.length}:${last.id}:dse-compile:${lastPart.model.signatureId}:${lastPart.model.state}:${lastPart.model.jobHash}:${lastPart.model.best?.compiled_id ?? ""}`
      }
      if (lastPart.kind === "dse-promote") {
        return `${renderedMessages.length}:${last.id}:dse-promote:${lastPart.model.signatureId}:${lastPart.model.from ?? ""}:${lastPart.model.to ?? ""}`
      }
      if (lastPart.kind === "dse-rollback") {
        return `${renderedMessages.length}:${last.id}:dse-rollback:${lastPart.model.signatureId}:${lastPart.model.from ?? ""}:${lastPart.model.to ?? ""}`
      }
      if (lastPart.kind === "dse-budget-exceeded") {
        return `${renderedMessages.length}:${last.id}:dse-budget-exceeded:${lastPart.model.state}:${(lastPart.model.message ?? "").length}`
      }

      return `${renderedMessages.length}:${last.id}:${(lastPart as any).kind ?? "part"}`
    })()

    const toolContractsKey = toolContractsByName ? Object.keys(toolContractsByName).sort().join(",") : ""

    const autopilotChatData = {
      messages: renderedMessages,
      isBusy,
      isAtBottom,
      inputValue: inputDraft,
      errorText: chatSnapshot.errorText ?? chatInitErrorText,
      auth: {
        isAuthed: Boolean(session.userId),
        authedEmail: session.user?.email ?? null,
        step: authStep,
        email: authEmail,
        code: authCode,
        isBusy: authBusy,
        errorText: authErrorText,
      },
    }

    const baseBlueprint = blueprintEditBase ?? blueprint
    const formDraft =
      blueprintDraft ??
      (baseBlueprint
        ? makeDraftFromBlueprint(baseBlueprint)
        : {
            userHandle: "",
            agentName: "",
            identityVibe: "",
            characterVibe: "",
            characterBoundaries: "",
          })

    const computedRawDraft = (() => {
      if (blueprintMode !== "raw") return null
      if (blueprintRawDraft != null) return blueprintRawDraft
      if (!baseBlueprint) return null
      try {
        const preview = applyDraftFields(baseBlueprint, formDraft)
        return JSON.stringify(preview, null, 2)
      } catch {
        return null
      }
    })()

    const _blueprintPanelModel = {
      updatedAtLabel: blueprintUpdatedAt ? new Date(blueprintUpdatedAt).toLocaleTimeString() : null,
      isLoading: blueprintLoading,
      canEdit: Boolean(blueprint),
      isSaving: isSavingBlueprint,
      errorText: blueprintError,
      mode: blueprintMode,
      rawErrorText: blueprintRawError,
      rawDraft: computedRawDraft,
      draft: baseBlueprint ? formDraft : null,
    }

    const chatKey = `${renderedTailKey}:${isBusy ? 1 : 0}:${isAtBottom ? 1 : 0}:${toolContractsKey}:${chatSnapshot.errorText ?? ""}:${session.userId ?? "null"}:${authStep}:${authBusy ? 1 : 0}:${authEmail}:${authCode}:${authErrorText ?? ""}`

    const controlsData = {
      isExportingBlueprint,
      isResettingAgent,
      dseStrategyId,
      dseBudgetProfile,
      isRunningDseRecap,
      dseErrorText,
    }
    const controlsKey = `${isExportingBlueprint ? 1 : 0}:${isResettingAgent ? 1 : 0}:${dseStrategyId}:${dseBudgetProfile}:${isRunningDseRecap ? 1 : 0}:${dseErrorText ?? ""}`

    const renderInput: AutopilotRouteRenderInput = {
      chatData: autopilotChatData,
      chatKey,
      controlsData,
      controlsKey,
    }

    Effect.runPromise(runAutopilotRoute(input.container, renderInput))
      .then(() => {
        if (disposed) return
        if (focusedOnMount) return
        if (typeof window !== "undefined" && window.location.pathname !== "/autopilot") return
        focusedOnMount = true
        focusChatInput({ force: true })
      })
      .catch(() => {})

    // Scroll after render when new content arrives and the user is pinned to bottom.
    if (isAtBottom && renderedMessages.length > 0 && renderedTailKey !== lastTailKey) {
      lastTailKey = renderedTailKey
      setTimeout(() => scrollToBottom(isStreaming ? "auto" : "smooth"), 0)
    }
  }

  const fetchBlueprint = async (options?: { silent?: boolean }) => {
    if (!chatId) return
    const silent = options?.silent ?? false
    if (!silent) {
      blueprintLoading = true
      scheduleRender()
    }
    blueprintError = null

    try {
      const json = await input.runtime.runPromise(
        input.store.getBlueprint({ threadId: chatId }),
      )
      blueprint = json
      blueprintUpdatedAt = Date.now()
      // Keep drafts in sync only when the user hasn't started editing locally.
      // Once the user types, we freeze the blueprint slot key to avoid clobbering caret.
      const hasLocalEdits =
        blueprintDraftDirty || blueprintRawDirty || blueprintKeyFrozenAt != null || blueprintEditBase != null
      if (!hasLocalEdits) {
        blueprintEditBase = json
        blueprintDraft = json ? makeDraftFromBlueprint(json) : null
        blueprintDraftDirty = false
        blueprintRawDraft = null
        blueprintRawDirty = false
        blueprintRawError = null
        blueprintKeyFrozenAt = null
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      Effect.runPromise(
        input.telemetry.withNamespace("ui.blueprint").log("error", "blueprint.fetch_failed", { message })
      ).catch(() => {})
      blueprintError = message || "Failed to load Blueprint."
    } finally {
      if (!silent) {
        blueprintLoading = false
      }
      scheduleRender()
    }
  }

  const fetchToolContracts = async () => {
    try {
      const contracts = await input.runtime.runPromise(input.contracts.getToolContracts())
      const map: Record<string, ToolContract> = {}
      for (const c of contracts) map[c.name] = c
      toolContractsByName = map
      scheduleRender()
    } catch {
      // Best-effort (tool contract cards degrade).
    }
  }

  // Subscribe to atoms.
  let subscribedForChatId: string | null = null
  let unsubChat: (() => void) | null = null
  let unsubIsAtBottom: (() => void) | null = null

  const ensureOwnedThreadId = (): void => {
    if (!session.userId) return
    chatInitErrorText = null
    scheduleRender()
    const startedAt = Date.now()
    void Effect.runPromise(
      input.telemetry.withNamespace("chat").event("ensureOwnedThread.start", { userId: session.userId }),
    ).catch(() => {})

    let settled = false
    const timeoutMs = 30_000
    const t = window.setTimeout(() => {
      if (disposed) return
      if (settled) return
      chatInitErrorText = "Timed out initializing chat. Check Convex connection/auth."
      void Effect.runPromise(
        input.telemetry.withNamespace("chat").log("error", "ensureOwnedThread.timeout", {
          userId: session.userId,
          ms: Date.now() - startedAt,
        }),
      ).catch(() => {})
      scheduleRender()
    }, timeoutMs)

    input.runtime.runPromise(input.chat.getOwnedThreadId()).then(
      (id) => {
        settled = true
        window.clearTimeout(t)
        if (disposed) return
        if (id && id.length > 0) {
          atoms.set(OwnedThreadIdAtom as any, id)
          chatInitErrorText = null
          void Effect.runPromise(
            input.telemetry.withNamespace("chat").event("ensureOwnedThread.ok", { threadId: id, ms: Date.now() - startedAt }),
          ).catch(() => {})
        } else {
          chatInitErrorText = "Failed to initialize chat (empty thread id)."
        }
        scheduleRender()
      },
      (err) => {
        settled = true
        window.clearTimeout(t)
        const e = err instanceof Error ? err : new Error(String(err))
        Effect.runPromise(
          input.telemetry.withNamespace("chat").log("error", "ensureOwnedThread.failed", { message: e.message }),
        ).catch(() => {})
        if (disposed) return
        chatInitErrorText = e.message || "Failed to initialize chat."
        scheduleRender()
      },
    )
  }

  const unsubSession = atoms.subscribe(SessionAtom, (next) => {
    session = next
    if (!next.userId) {
      atoms.set(OwnedThreadIdAtom as any, null)
      chatId = ""
      chatInitErrorText = null
    } else {
      if (!atoms.get(OwnedThreadIdAtom)) ensureOwnedThreadId()
      chatId = ownedThreadId ?? ""
    }
    scheduleRender()
  }, { immediate: false })

  const unsubOwnedThreadId = atoms.subscribe(OwnedThreadIdAtom, (next) => {
    ownedThreadId = next
    chatId = session.userId ? (next ?? "") : ""
    if (next) {
      // Prime snapshot so ChatSnapshotAtom is evaluated and ChatService.open starts the WS subscription
      // immediately. Without this, the UI can "silently stall" on an empty chat until a later change.
      chatSnapshot = atoms.get(ChatSnapshotAtom(next))
    } else {
      chatSnapshot = { messages: [], status: "ready" as const, errorText: null }
    }
    if (next && next !== subscribedForChatId) {
      if (unsubChat) unsubChat()
      if (unsubIsAtBottom) unsubIsAtBottom()
      subscribedForChatId = next
      unsubChat = atoms.subscribe(ChatSnapshotAtom(next), (snap) => {
        chatSnapshot = snap
        const busy = snap.status === "submitted" || snap.status === "streaming"
        if (prevBusy && !busy) void fetchBlueprint({ silent: true })
        prevBusy = busy
        scheduleRender()
      }, { immediate: true })
      unsubIsAtBottom = atoms.subscribe(AutopilotChatIsAtBottomAtom(next), (nextIsAtBottom) => {
        isAtBottom = nextIsAtBottom
        scheduleRender()
      }, { immediate: true })
    }
    scheduleRender()
  }, { immediate: false })

  let prevBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"

  const unsubCollapsed = atoms.subscribe(AutopilotSidebarCollapsedAtom, (next) => {
    _collapsed = next
    scheduleRender()
  }, { immediate: false })

  const unsubUserMenu = atoms.subscribe(AutopilotSidebarUserMenuOpenAtom, (next) => {
    userMenuOpen = next
    scheduleRender()
  }, { immediate: false })

  if (session.userId && !atoms.get(OwnedThreadIdAtom)) ensureOwnedThreadId()

  // Event listeners.
  const onDocClick = (e: MouseEvent) => {
    if (!userMenuOpen) return
    const root = document.querySelector("[data-autopilot-sidebar-root]")
    if (!(root instanceof HTMLElement)) return
    if (!(e.target instanceof Node)) return
    if (!root.contains(e.target)) {
      atoms.set(AutopilotSidebarUserMenuOpenAtom, false)
    }
  }
  document.addEventListener("click", onDocClick)

  const onScrollCapture = (e: Event) => {
    const thresholdPx = 96
    const target = e.target
    if (!(target instanceof HTMLElement)) return
    if (target.getAttribute("data-scroll-id") !== "autopilot-chat-scroll") return
    const cid = session.userId ? (ownedThreadId ?? "") : ""
    if (cid) atoms.set(AutopilotChatIsAtBottomAtom(cid), target.scrollHeight - (target.scrollTop + target.clientHeight) <= thresholdPx)
  }
  document.addEventListener("scroll", onScrollCapture, true)

  // Actions.
  input.ez.set("autopilot.sidebar.toggleCollapse", () =>
    Effect.sync(() => {
      atoms.set(AutopilotSidebarUserMenuOpenAtom, false)
      atoms.update(AutopilotSidebarCollapsedAtom, (c) => !c)
    })
  )

  input.ez.set("autopilot.sidebar.toggleUserMenu", () =>
    Effect.sync(() => atoms.update(AutopilotSidebarUserMenuOpenAtom, (o) => !o))
  )

  const signOut = async () => {
    try {
      await fetch("/api/auth/signout", { method: "POST", credentials: "include" })
    } catch {
      // Best-effort
    } finally {
      clearAuthClientCache()
      atoms.set(SessionAtom as any, { userId: null, user: null })
      atoms.set(OwnedThreadIdAtom as any, null)
      input.navigate("/")
    }
  }

  input.ez.set("autopilot.sidebar.logout", () =>
    Effect.sync(() => {
      atoms.set(AutopilotSidebarUserMenuOpenAtom, false)
      void signOut()
    })
  )

  input.ez.set("autopilot.chat.input", ({ params }) =>
    Effect.sync(() => {
      inputDraft = String((params as any).message ?? "")
    })
  )

  input.ez.set("autopilot.chat.scrollBottom", () =>
    Effect.sync(() => scrollToBottom("smooth"))
  )

  input.ez.set("autopilot.chat.stop", () =>
    Effect.promise(() => input.runtime.runPromise(input.chat.stop(chatId)))
  )

  input.ez.set("autopilot.chat.retryInit", () =>
    Effect.sync(() => {
      ensureOwnedThreadId()
    }),
  )

  input.ez.set("autopilot.chat.send", ({ el, params }) =>
    Effect.gen(function* () {
      const isBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"
      if (isBusy) return

      const form = el instanceof HTMLFormElement ? el : null
      const inputEl = form?.querySelector<HTMLInputElement>('input[name="message"]') ?? null

      const text = String((params as any).message ?? "").trim()
      if (!text) return

      // Guard against a common race: session is authed but owned thread id hasn't resolved yet.
      // Without this, send() would target an empty threadId and the UI would appear to do nothing.
      let effectiveChatId = chatId
      if (!effectiveChatId) {
        if (!session.userId) {
          yield* Effect.sync(() => {
            chatInitErrorText = null
            setAuth({ step: "email", errorText: null })
          })
          return
        }

        const ensured = yield* Effect.tryPromise({
          try: () => input.runtime.runPromise(input.chat.getOwnedThreadId()),
          catch: (err) => (err instanceof Error ? err : new Error(String(err))),
        }).pipe(
          Effect.tapError((err) =>
            input.telemetry
              .withNamespace("chat")
              .log("error", "ensureOwnedThread.failed", { message: err.message })
              .pipe(Effect.catchAll(() => Effect.void)),
          ),
          Effect.catchAll((err) =>
            Effect.sync(() => {
              chatInitErrorText = err instanceof Error && err.message ? err.message : "Failed to initialize chat."
              scheduleRender()
              return ""
            }),
          ),
        )

        yield* Effect.sync(() => {
          if (ensured && ensured.length > 0) {
            atoms.set(OwnedThreadIdAtom as any, ensured)
            ownedThreadId = ensured
            chatId = ensured
            // Prime snapshot so error banners can render deterministically even before the
            // OwnedThreadIdAtom subscription callback runs.
            chatSnapshot = atoms.get(ChatSnapshotAtom(ensured))
            chatInitErrorText = null
            scheduleRender()
          }
        })

        effectiveChatId = ensured
        if (!effectiveChatId) return
      }

      yield* Effect.sync(() => {
        if (inputEl) inputEl.value = ""
        inputDraft = ""

        // Clicking the Send button transfers focus to the button; return focus
        // to the chat input so users can keep typing immediately.
        focusChatInput({ force: true })
      })

      yield* Effect.tryPromise({
        try: () => input.runtime.runPromise(input.chat.send(effectiveChatId, text)),
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      }).pipe(
        Effect.catchAll(() =>
          Effect.sync(() => {
            if (inputEl) inputEl.value = text
            inputDraft = text
          })
        )
      )

      yield* Effect.sync(() => setTimeout(() => scrollToBottom("auto"), 0))
    })
  )

  const setAuth = (next: Partial<{
    step: AuthStep
    email: string
    code: string
    isBusy: boolean
    errorText: string | null
  }>) => {
    if (next.step !== undefined) authStep = next.step
    if (next.email !== undefined) authEmail = next.email
    if (next.code !== undefined) authCode = next.code
    if (next.isBusy !== undefined) authBusy = next.isBusy
    if (next.errorText !== undefined) authErrorText = next.errorText
    scheduleRender()
  }

  const startMagicCode = async (emailRaw: string) => {
    const email = normalizeEmail(emailRaw)
    if (!email || !email.includes("@") || email.length > 320) {
      setAuth({ errorText: "Enter a valid email address." })
      return
    }

    setAuth({ isBusy: true, errorText: null, email })
    try {
      const res = await fetch("/api/auth/start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        credentials: "include",
        cache: "no-store",
        body: JSON.stringify({ email }),
      })
      const json = await res.json().catch(() => null)
      if (!res.ok || !json || (json as any).ok !== true) {
        setAuth({ errorText: "Failed to send code. Try again." })
        return
      }
      setAuth({ step: "code", errorText: null })
    } catch (err) {
      setAuth({ errorText: err instanceof Error ? err.message : "Failed to send code." })
    } finally {
      setAuth({ isBusy: false })
    }
  }

  const verifyMagicCode = async (emailRaw: string, codeRaw: string) => {
    const email = normalizeEmail(emailRaw)
    const code = normalizeCode(codeRaw)
    if (!email || !email.includes("@") || email.length > 320) {
      setAuth({ errorText: "Enter a valid email address." })
      return
    }
    if (!/^[0-9]{4,10}$/.test(code)) {
      setAuth({ errorText: "Enter the numeric code from your email." })
      return
    }

    setAuth({ isBusy: true, errorText: null, email, code })
    try {
      const res = await fetch("/api/auth/verify", {
        method: "POST",
        headers: { "content-type": "application/json" },
        credentials: "include",
        cache: "no-store",
        body: JSON.stringify({ email, code }),
      })
      const json = await res.json().catch(() => null)
      if (!res.ok || !json || (json as any).ok !== true) {
        setAuth({ errorText: "Invalid code. Try again." })
        return
      }

      // Refresh Auth/Convex token caches and update SessionAtom.
      clearAuthClientCache()

      const authSession = await input.runtime.runPromise(
        Effect.gen(function* () {
          const auth = yield* AuthService
          return yield* auth.getSession()
        }),
      )

      atoms.set(SessionAtom as any, {
        userId: authSession.userId,
        user: authSession.user
          ? {
              id: authSession.user.id,
              email: authSession.user.email,
              firstName: authSession.user.firstName,
              lastName: authSession.user.lastName,
            }
          : null,
      })

      setAuth({ step: "closed", code: "", errorText: null })
    } catch (err) {
      setAuth({ errorText: err instanceof Error ? err.message : "Verification failed." })
    } finally {
      setAuth({ isBusy: false })
    }
  }

  input.ez.set("autopilot.auth.open", () =>
    Effect.sync(() => setAuth({ step: "email", errorText: null })),
  )

  input.ez.set("autopilot.auth.close", () =>
    Effect.sync(() => setAuth({ step: "closed", errorText: null })),
  )

  input.ez.set("autopilot.auth.email.input", ({ params }) =>
    Effect.sync(() => setAuth({ email: String((params as any).email ?? ""), errorText: null })),
  )

  input.ez.set("autopilot.auth.email.submit", ({ params }) =>
    Effect.sync(() => {
      const email = String((params as any).email ?? authEmail ?? "")
      void startMagicCode(email)
    }),
  )

  input.ez.set("autopilot.auth.code.input", ({ params }) =>
    Effect.sync(() => setAuth({ code: String((params as any).code ?? ""), errorText: null })),
  )

  input.ez.set("autopilot.auth.code.back", () =>
    Effect.sync(() => setAuth({ step: "email", code: "", errorText: null })),
  )

  input.ez.set("autopilot.auth.code.resend", () =>
    Effect.sync(() => {
      void startMagicCode(authEmail)
    }),
  )

  input.ez.set("autopilot.auth.code.submit", ({ params }) =>
    Effect.sync(() => {
      const email = String((params as any).email ?? authEmail ?? "")
      const code = String((params as any).code ?? authCode ?? "")
      void verifyMagicCode(email, code)
    }),
  )

  const setBlueprintMode = (mode: "form" | "raw") => {
    if (mode === blueprintMode) return
    if (!blueprint) return

    blueprintRawError = null

    // If switching to Raw, materialize a JSON draft that includes current form edits.
    if (mode === "raw") {
      const base = blueprintEditBase ?? blueprint
      const draft = blueprintDraft ?? makeDraftFromBlueprint(base)
      try {
        const preview = applyDraftFields(base, draft)
        blueprintRawDraft = JSON.stringify(preview, null, 2)
        blueprintRawDirty = false
      } catch {
        blueprintRawDraft = blueprintRawDraft ?? null
      }
    }

    // If switching to Form from Raw, parse the raw JSON once so we preserve unknown keys.
    if (mode === "form" && blueprintMode === "raw") {
      const text = blueprintRawDraft ?? ""
      if (text.trim().length > 0) {
        try {
          const parsed = JSON.parse(text)
          blueprintEditBase = parsed
          blueprintDraft = makeDraftFromBlueprint(parsed)
          // If the user edited raw JSON, treat the form as dirty (unsaved).
          if (blueprintRawDirty) {
            blueprintDraftDirty = true
          }
          blueprintRawDirty = false
        } catch (err) {
          blueprintRawError = err instanceof Error ? err.message : "Invalid JSON."
          // Stay in raw mode until JSON parses.
          scheduleRender()
          return
        }
      }
    }

    blueprintMode = mode
    // Freeze blueprint slot on mode switch to avoid background polling clobbering the editor.
    if (blueprintKeyFrozenAt == null && blueprintUpdatedAt != null) {
      blueprintKeyFrozenAt = blueprintUpdatedAt
    }
    scheduleRender()
  }

  const onSaveBlueprint = async () => {
    if (!blueprint || isSavingBlueprint) return
    isSavingBlueprint = true
    blueprintError = null
    blueprintRawError = null
    scheduleRender()

    const nowIso = new Date().toISOString()
    let next: any
    let changed: ReadonlyArray<string> = []

    try {
      if (blueprintMode === "raw") {
        const text = blueprintRawDraft ?? ""
        if (text.trim().length === 0) {
          blueprintRawError = "Blueprint JSON is empty."
          return
        }
        try {
          next = JSON.parse(text)
        } catch (err) {
          blueprintRawError = err instanceof Error ? err.message : "Invalid JSON."
          return
        }
        applySaveMetadata(next, nowIso)
        changed = ["raw.json"]
      } else {
        const base = blueprintEditBase ?? blueprint
        const draft = blueprintDraft ?? makeDraftFromBlueprint(base)
        next = applyDraftFields(base, draft)
        applySaveMetadata(next, nowIso)
        changed = [
          "user.handle",
          "identity.name",
          "identity.vibe",
          "character.vibe",
          "character.boundaries",
        ]
      }

      await Effect.runPromise(
        input.telemetry.withNamespace("ui.blueprint").event("blueprint.save", {
          changed,
        })
      )
      await input.runtime.runPromise(
        input.store.importBlueprint({ threadId: chatId, blueprint: next }),
      )
      // Clear local edit state so the panel tracks server updates again.
      blueprintEditBase = null
      blueprintDraftDirty = false
      blueprintRawDirty = false
      blueprintRawDraft = null
      blueprintRawError = null
      blueprintKeyFrozenAt = null
      await fetchBlueprint()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      Effect.runPromise(
        input.telemetry.withNamespace("ui.blueprint").log("error", "blueprint.save_failed", { message })
      ).catch(() => {})
      blueprintError = message || "Failed to save Blueprint."
    } finally {
      isSavingBlueprint = false
      scheduleRender()
    }
  }

  input.ez.set("autopilot.blueprint.setMode", ({ params }) =>
    Effect.sync(() => {
      const mode = String((params as any).mode ?? "")
      if (mode === "form" || mode === "raw") {
        setBlueprintMode(mode)
      }
    })
  )

  input.ez.set("autopilot.blueprint.refresh", () =>
    Effect.sync(() => {
      void fetchBlueprint()
    })
  )

  input.ez.set("autopilot.blueprint.save", () =>
    Effect.sync(() => {
      void onSaveBlueprint()
    })
  )

  input.ez.set("autopilot.blueprint.rawDraft", ({ params }) =>
    Effect.sync(() => {
      const p = params as any
      if (p.raw === undefined) return
      blueprintRawDraft = String(p.raw)
      blueprintRawDirty = true
      blueprintRawError = null

      if (blueprintKeyFrozenAt == null && blueprintUpdatedAt != null) {
        blueprintKeyFrozenAt = blueprintUpdatedAt
      }
    })
  )

  input.ez.set("autopilot.blueprint.draft", ({ params }) =>
    Effect.sync(() => {
      const draft =
        blueprintDraft ??
        ({
          userHandle: "",
          agentName: "",
          identityVibe: "",
          characterVibe: "",
          characterBoundaries: "",
        } satisfies BlueprintDraft)
      blueprintDraft = draft

      const p = params as any
      if (p.userHandle !== undefined) draft.userHandle = String(p.userHandle)
      if (p.agentName !== undefined) draft.agentName = String(p.agentName)
      if (p.identityVibe !== undefined) draft.identityVibe = String(p.identityVibe)
      if (p.characterVibe !== undefined) draft.characterVibe = String(p.characterVibe)
      if (p.characterBoundaries !== undefined) draft.characterBoundaries = String(p.characterBoundaries)

      blueprintDraftDirty = true
      blueprintRawError = null
      if (blueprintKeyFrozenAt == null && blueprintUpdatedAt != null) {
        blueprintKeyFrozenAt = blueprintUpdatedAt
      }
    })
  )

  input.ez.set("autopilot.controls.exportBlueprint", () =>
    Effect.sync(() => {
      if (isExportingBlueprint) return
      isExportingBlueprint = true
      scheduleRender()

      void (async () => {
        try {
          const blueprintJson = await input.runtime.runPromise(
            input.store.getBlueprint({ threadId: chatId }),
          )
          const blob = new Blob([JSON.stringify(blueprintJson, null, 2)], { type: "application/json" })
          const url = URL.createObjectURL(blob)
          const link = document.createElement("a")
          link.href = url
          link.download = `autopilot-blueprint-${chatId}.json`
          document.body.appendChild(link)
          link.click()
          link.remove()
          URL.revokeObjectURL(url)
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err)
          Effect.runPromise(
            input.telemetry.withNamespace("ui.blueprint").log("error", "blueprint.export_failed", { message })
          ).catch(() => {})
          window.alert("Failed to export Blueprint JSON.")
        } finally {
          isExportingBlueprint = false
          scheduleRender()
        }
      })()
    })
  )

  input.ez.set("autopilot.controls.clearMessages", () =>
    Effect.gen(function* () {
      if (!chatId) {
        if (!session.userId) {
          yield* Effect.sync(() => setAuth({ step: "email", errorText: null }))
          return
        }
        yield* Effect.sync(() => window.alert("Chat is still initializing. Try again in a moment."))
        return
      }

      // If a run is currently streaming, clear will immediately be repopulated by new parts.
      // Stop first so Clear Messages is a reliable escape hatch.
      yield* Effect.promise(() => input.runtime.runPromise(input.chat.stop(chatId))).pipe(Effect.catchAll(() => Effect.void))
      yield* Effect.promise(() => input.runtime.runPromise(input.chat.clearHistory(chatId)))
      yield* Effect.sync(() => {
        inputDraft = ""
        const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]')
        if (inputEl) inputEl.value = ""
      })
    })
  )

  const resetAgent = async () => {
    if (isResettingAgent) return
    if (!chatId) {
      if (!session.userId) {
        setAuth({ step: "email", errorText: null })
        return
      }
      window.alert("Chat is still initializing. Try again in a moment.")
      return
    }

    const confirmed = window.confirm(
      "Reset agent?\n\nThis will stop any in-progress run, clear messages, and reset your Blueprint to defaults.",
    )
    if (!confirmed) return

    isResettingAgent = true
    blueprintError = null
    scheduleRender()

    try {
      // Best-effort stop so reset is always a working escape hatch (even if a run got stuck).
      await input.runtime.runPromise(input.chat.stop(chatId)).catch(() => {})
      await input.runtime.runPromise(input.store.resetThread({ threadId: chatId }))
      inputDraft = ""
      const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]')
      if (inputEl) inputEl.value = ""
      await fetchBlueprint()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      Effect.runPromise(
        input.telemetry.withNamespace("ui.chat").log("error", "agent.reset_failed", { message })
      ).catch(() => {})
      window.alert("Failed to reset agent.")
    } finally {
      isResettingAgent = false
      scheduleRender()
    }
  }

  input.ez.set("autopilot.controls.resetAgent", () =>
    Effect.sync(() => {
      void resetAgent()
    })
  )

  input.ez.set("autopilot.controls.dse.strategy", ({ params }) =>
    Effect.sync(() => {
      const next = String((params as any).strategyId ?? "")
      if (next === "direct.v1" || next === "rlm_lite.v1") {
        dseStrategyId = next
        dseErrorText = null
        scheduleRender()
      }
    }),
  )

  input.ez.set("autopilot.controls.dse.budget", ({ params }) =>
    Effect.sync(() => {
      const next = String((params as any).budgetProfile ?? "")
      if (next === "small" || next === "medium" || next === "long") {
        dseBudgetProfile = next
        dseErrorText = null
        scheduleRender()
      }
    }),
  )

  input.ez.set("autopilot.controls.dse.recap", () =>
    Effect.gen(function* () {
      if (isRunningDseRecap) return

      // Don't overlap with an in-flight chat run. It makes debugging harder and can interleave cards.
      const chatBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"
      if (chatBusy) {
        yield* Effect.sync(() => {
          dseErrorText = "Wait for the current run to finish (or Stop it) before running the canary recap."
          scheduleRender()
        })
        return
      }

      isRunningDseRecap = true
      dseErrorText = null
      scheduleRender()

      try {
        // Ensure we have a thread id.
        let effectiveChatId = chatId
        if (!effectiveChatId) {
          if (!session.userId) {
            yield* Effect.sync(() => setAuth({ step: "email", errorText: null }))
            return
          }

          const ensured = yield* Effect.tryPromise({
            try: () => input.runtime.runPromise(input.chat.getOwnedThreadId()),
            catch: (err) => (err instanceof Error ? err : new Error(String(err))),
          }).pipe(
            Effect.tapError((err) =>
              input.telemetry
                .withNamespace("chat")
                .log("error", "ensureOwnedThread.failed", { message: err.message })
                .pipe(Effect.catchAll(() => Effect.void)),
            ),
            Effect.catchAll((err) =>
              Effect.sync(() => {
                dseErrorText = err instanceof Error && err.message ? err.message : "Failed to initialize chat."
                scheduleRender()
                return ""
              }),
            ),
          )

          yield* Effect.sync(() => {
            if (ensured && ensured.length > 0) {
              atoms.set(OwnedThreadIdAtom as any, ensured)
              ownedThreadId = ensured
              chatId = ensured
              chatSnapshot = atoms.get(ChatSnapshotAtom(ensured))
              chatInitErrorText = null
              scheduleRender()
            }
          })

          effectiveChatId = ensured
          if (!effectiveChatId) return
        }

        const response = yield* Effect.tryPromise({
          try: () =>
            fetch("/api/autopilot/dse/recap", {
              method: "POST",
              headers: {
                "content-type": "application/json",
                // In production E2E runs, we want deterministic behavior; the server only honors
                // this for E2E-auth sessions (oa-e2e cookie).
                "x-oa-e2e-mode": "stub",
              },
              credentials: "include",
              cache: "no-store",
              body: JSON.stringify({
                threadId: effectiveChatId,
                strategyId: dseStrategyId,
                budgetProfile: dseBudgetProfile,
              }),
            }),
          catch: (err) => (err instanceof Error ? err : new Error(String(err))),
        })

        if (!response.ok) {
          const body = yield* Effect.tryPromise({ try: () => response.text(), catch: () => "" }).pipe(
            Effect.catchAll(() => Effect.succeed("")),
          )
          const msg = body.trim() ? body.trim() : `HTTP ${response.status}`
          yield* Effect.sync(() => {
            dseErrorText = msg
            scheduleRender()
          })
          return
        }
      } finally {
        isRunningDseRecap = false
        scheduleRender()
      }
    }),
  )

  // Start background work + initial render/hydration.
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})
  void fetchBlueprint()
  void fetchToolContracts()
  scheduleRender()

  return {
    cleanup: () => {
      disposed = true
      unsubSession()
      unsubOwnedThreadId()
      unsubCollapsed()
      unsubUserMenu()
      if (unsubChat) unsubChat()
      if (unsubIsAtBottom) unsubIsAtBottom()

      document.removeEventListener("click", onDocClick)
      document.removeEventListener("scroll", onScrollCapture, true)

      input.ez.delete("autopilot.sidebar.toggleCollapse")
      input.ez.delete("autopilot.sidebar.toggleUserMenu")
      input.ez.delete("autopilot.sidebar.logout")
      input.ez.delete("autopilot.chat.input")
      input.ez.delete("autopilot.chat.scrollBottom")
      input.ez.delete("autopilot.chat.stop")
      input.ez.delete("autopilot.chat.send")
      input.ez.delete("autopilot.auth.open")
      input.ez.delete("autopilot.auth.close")
      input.ez.delete("autopilot.auth.email.input")
      input.ez.delete("autopilot.auth.email.submit")
      input.ez.delete("autopilot.auth.code.input")
      input.ez.delete("autopilot.auth.code.back")
      input.ez.delete("autopilot.auth.code.resend")
      input.ez.delete("autopilot.auth.code.submit")
      input.ez.delete("autopilot.blueprint.setMode")
      input.ez.delete("autopilot.blueprint.refresh")
      input.ez.delete("autopilot.blueprint.save")
      input.ez.delete("autopilot.blueprint.rawDraft")
      input.ez.delete("autopilot.blueprint.draft")
      input.ez.delete("autopilot.controls.exportBlueprint")
      input.ez.delete("autopilot.controls.clearMessages")
      input.ez.delete("autopilot.controls.resetAgent")
      input.ez.delete("autopilot.controls.dse.strategy")
      input.ez.delete("autopilot.controls.dse.budget")
      input.ez.delete("autopilot.controls.dse.recap")

      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}
