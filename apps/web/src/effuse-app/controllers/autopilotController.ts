import { Effect } from "effect"
import { boundText, html } from "@openagentsinc/effuse"
import type { EzAction, RouterService, ToolPartStatus } from "@openagentsinc/effuse"

import { cleanupAuthedDotsGridBackground, hydrateAuthedDotsGridBackground } from "../../effuse-pages/authedShell"
import { runAutopilotRoute } from "../../effuse-pages/autopilotRoute"
import { AutopilotChatIsAtBottomAtom, ChatSnapshotAtom } from "../../effect/atoms/chat"
import { AutopilotSidebarCollapsedAtom, AutopilotSidebarUserMenuOpenAtom } from "../../effect/atoms/autopilotUi"
import { SessionAtom } from "../../effect/atoms/session"
import { clearAuthClientCache } from "../../effect/auth"
import { UiBlobStore } from "../blobStore"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { AgentApi, AgentToolContract } from "../../effect/agentApi"
import type { ChatClient } from "../../effect/chat"
import type { TelemetryClient } from "../../effect/telemetry"
import type { UIMessage } from "ai"
import type { AutopilotRouteRenderInput } from "../../effuse-pages/autopilotRoute"
import type { RenderedMessage as EffuseRenderedMessage } from "../../effuse-pages/autopilot"

const ANON_CHAT_STORAGE_KEY = "autopilot-anon-chat-id"

function randomId(size = 12): string {
  let out = ""
  while (out.length < size) out += Math.random().toString(36).slice(2)
  return out.slice(0, size)
}

function getOrCreateAnonChatId(): string {
  if (typeof sessionStorage === "undefined") return `anon-${randomId(8)}`
  let id = sessionStorage.getItem(ANON_CHAT_STORAGE_KEY)
  if (!id) {
    id = `anon-${randomId(12)}`
    sessionStorage.setItem(ANON_CHAT_STORAGE_KEY, id)
  }
  return id
}

function sanitizeBlueprintForDisplay(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sanitizeBlueprintForDisplay)
  }

  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>
    const out: Record<string, unknown> = {}
    for (const [key, child] of Object.entries(record)) {
      const safeKey = key === "addressAs" ? "handle" : key
      out[safeKey] = sanitizeBlueprintForDisplay(child)
    }
    return out
  }

  return value
}

type UiPart = UIMessage["parts"][number]

type RenderPart =
  | { kind: "text"; text: string; state?: "streaming" | "done" }
  | {
      kind: "tool"
      toolName: string
      toolCallId: string
      state: string
      input: unknown
      output?: unknown
      errorText?: string
      preliminary?: boolean
      approval?: { id: string; approved?: boolean; reason?: string }
    }

function isTextPart(part: unknown): part is Extract<UiPart, { type: "text" }> {
  return (
    Boolean(part) &&
    typeof part === "object" &&
    (part as any).type === "text" &&
    typeof (part as any).text === "string"
  )
}

function isToolPart(
  part: unknown,
): part is
  | (Extract<UiPart, { type: `tool-${string}` }> & {
      toolCallId: string
      state: string
      input?: unknown
      output?: unknown
      errorText?: string
      preliminary?: boolean
      approval?: { id: string; approved?: boolean; reason?: string }
      rawInput?: unknown
    })
  | (Extract<UiPart, { type: "dynamic-tool" }> & {
      toolName: string
      toolCallId: string
      state: string
      input?: unknown
      output?: unknown
      errorText?: string
      preliminary?: boolean
      approval?: { id: string; approved?: boolean; reason?: string }
      rawInput?: unknown
    }) {
  if (!part || typeof part !== "object") return false
  const type = (part as any).type
  if (type === "dynamic-tool") {
    return (
      typeof (part as any).toolName === "string" &&
      typeof (part as any).toolCallId === "string" &&
      typeof (part as any).state === "string"
    )
  }
  return (
    typeof type === "string" &&
    type.startsWith("tool-") &&
    typeof (part as any).toolCallId === "string" &&
    typeof (part as any).state === "string"
  )
}

function getToolPartName(part: { type: string; toolName?: string }): string {
  if (part.type === "dynamic-tool") return String(part.toolName ?? "tool")
  if (part.type.startsWith("tool-")) return part.type.slice("tool-".length)
  return part.type
}

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value)
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, indent)
  } catch {
    return String(value)
  }
}

function toRenderableParts(parts: ReadonlyArray<UiPart>): Array<RenderPart> {
  const out: Array<RenderPart> = []

  for (const p of parts) {
    if (isTextPart(p)) {
      if (p.text.length === 0) continue
      const prev = out.at(-1)
      if (prev?.kind === "text" && prev.state === p.state) {
        prev.text += p.text
      } else {
        out.push({ kind: "text", text: p.text, state: p.state })
      }
      continue
    }

    if (isToolPart(p)) {
      const toolName = getToolPartName(p as any)
      const state = String((p as any).state ?? "")
      const rawInput = (p as any).rawInput
      const input = (p as any).input ?? rawInput
      out.push({
        kind: "tool",
        toolName,
        toolCallId: String((p as any).toolCallId),
        state,
        input,
        output: (p as any).output,
        errorText: typeof (p as any).errorText === "string" ? (p as any).errorText : undefined,
        preliminary: Boolean((p as any).preliminary),
        approval:
          (p as any).approval && typeof (p as any).approval === "object"
            ? {
                id: String((p as any).approval.id ?? ""),
                approved:
                  typeof (p as any).approval.approved === "boolean"
                    ? (p as any).approval.approved
                    : undefined,
                reason:
                  typeof (p as any).approval.reason === "string"
                    ? (p as any).approval.reason
                    : undefined,
              }
            : undefined,
      })
      continue
    }
  }

  return out
}

const TOOL_IO_MAX_CHARS = 4000

const toToolStatus = (state: string): ToolPartStatus => {
  switch (state) {
    case "output-available":
      return "tool-result"
    case "output-error":
      return "tool-error"
    case "output-denied":
      return "tool-denied"
    case "approval-requested":
    case "approval-responded":
      return "tool-approval"
    default:
      return state.startsWith("input-") ? "tool-call" : "tool-call"
  }
}

export type AutopilotController = {
  readonly cleanup: () => void
}

export const mountAutopilotController = (input: {
  readonly container: Element
  readonly ez: Map<string, EzAction>
  readonly atoms: AtomRegistry
  readonly telemetry: TelemetryClient
  readonly api: AgentApi
  readonly chat: ChatClient
  readonly router: RouterService<any>
  readonly navigate: (href: string) => void
}): AutopilotController => {
  const atoms = input.atoms

  const initialSession = atoms.get(SessionAtom)
  const anonChatId = getOrCreateAnonChatId()
  const chatId = initialSession.userId ?? anonChatId

  // Atom-backed UI state
  let session = initialSession
  let collapsed = atoms.get(AutopilotSidebarCollapsedAtom)
  let userMenuOpen = atoms.get(AutopilotSidebarUserMenuOpenAtom)
  let chatSnapshot = atoms.get(ChatSnapshotAtom(chatId))
  let isAtBottom = atoms.get(AutopilotChatIsAtBottomAtom(chatId))

  // Local controller state (was React useState/useRef)
  let isExportingBlueprint = false
  let isResettingAgent = false
  let blueprint: unknown | null = null
  let blueprintError: string | null = null
  let blueprintLoading = false
  let blueprintUpdatedAt: number | null = null
  let isEditingBlueprint = false
  let isSavingBlueprint = false
  type BlueprintDraft = {
    userHandle: string
    agentName: string
    identityVibe: string
    characterVibe: string
    characterBoundaries: string
  }
  let blueprintDraft: BlueprintDraft | null = null
  let toolContractsByName: Record<string, AgentToolContract> | null = null

  // Draft input in a ref-like variable so keystrokes don't force rerenders.
  let inputDraft = ""

  let blueprintPollInterval: number | null = null
  let lastTailKey = ""
  let renderScheduled = false

  const scrollToBottom = (behavior: ScrollBehavior = "auto") => {
    const bottom = document.querySelector("[data-autopilot-bottom]")
    ;(bottom as HTMLElement | null)?.scrollIntoView({ block: "end", behavior })
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

    const renderedMessages = messages
      .filter((m): m is UIMessage & { readonly role: "user" | "assistant" } => m.role === "user" || m.role === "assistant")
      .map((msg) => {
        const parts = Array.isArray((msg as any).parts) ? ((msg as any).parts as ReadonlyArray<UiPart>) : []
        const renderParts = toRenderableParts(parts)
        return { id: msg.id, role: msg.role, renderParts }
      })
      .filter((m) => m.role === "user" || m.renderParts.length > 0)

    const renderedTailKey = (() => {
      const last = renderedMessages.at(-1)
      if (!last) return ""
      const lastPart = last.renderParts.at(-1)
      if (!lastPart) return `${renderedMessages.length}:${last.id}`
      if (lastPart.kind === "text") {
        return `${renderedMessages.length}:${last.id}:text:${lastPart.text.length}:${lastPart.state ?? ""}`
      }
      return `${renderedMessages.length}:${last.id}:tool:${lastPart.toolName}:${lastPart.state}:${safeStableStringify(lastPart.output ?? "").length}`
    })()

    const toolContractsKey = toolContractsByName ? Object.keys(toolContractsByName).sort().join(",") : ""

    const autopilotChatData = {
      messages: renderedMessages.map((m): EffuseRenderedMessage => ({
        id: m.id,
        role: m.role,
        renderParts: m.renderParts.map((p) => {
          if (p.kind === "text") {
            return { kind: "text" as const, text: p.text, state: p.state }
          }
          const meta = toolContractsByName?.[p.toolName]
          const putText = ({ text, mime }: { readonly text: string; readonly mime?: string }) =>
            Effect.sync(() => UiBlobStore.putText({ text, mime }))

          const input = Effect.runSync(
            boundText({
              text: safeStableStringify(p.input),
              maxChars: TOOL_IO_MAX_CHARS,
              putText,
              mime: "application/json",
            }),
          )

          const outputText = p.output !== undefined ? safeStableStringify(p.output) : null
          const output = outputText
            ? Effect.runSync(
                boundText({
                  text: outputText,
                  maxChars: TOOL_IO_MAX_CHARS,
                  putText,
                  mime: "application/json",
                }),
              )
            : undefined

          const error =
            p.errorText && p.errorText.length > 0
              ? Effect.runSync(
                  boundText({
                    text: p.errorText,
                    maxChars: TOOL_IO_MAX_CHARS,
                    putText,
                    mime: "text/plain",
                  }),
                )
              : undefined

          const extra =
            meta?.usage || meta?.description
              ? html`
                  <div data-effuse-tool-meta="1">
                    ${meta.usage ? html`<div data-effuse-tool-usage="1">${meta.usage}</div>` : null}
                    ${meta.description
                      ? html`<div data-effuse-tool-description="1">${meta.description}</div>`
                      : null}
                  </div>
                `
              : null

          return {
            kind: "tool" as const,
            model: {
              status: toToolStatus(p.state),
              toolName: p.toolName,
              toolCallId: p.toolCallId,
              summary: p.state,
              details: {
                extra,
                input,
                output,
                error,
              },
            },
          }
        }),
      })),
      isBusy,
      isAtBottom,
      inputValue: inputDraft,
    }

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

    const blueprintText = (() => {
      if (!blueprint) return null
      try {
        return JSON.stringify(sanitizeBlueprintForDisplay(blueprint), null, 2)
      } catch {
        return null
      }
    })()

    const blueprintPanelModel = {
      updatedAtLabel: blueprintUpdatedAt ? new Date(blueprintUpdatedAt).toLocaleTimeString() : null,
      isLoading: blueprintLoading,
      isEditing: isEditingBlueprint,
      canEdit: Boolean(blueprint),
      isSaving: isSavingBlueprint,
      errorText: blueprintError,
      blueprintText,
      draft: isEditingBlueprint
        ? (blueprintDraft ??
            (blueprint
              ? makeDraftFromBlueprint(blueprint)
              : {
                  userHandle: "",
                  agentName: "",
                  identityVibe: "",
                  characterVibe: "",
                  characterBoundaries: "",
                }))
        : null,
    }

    const controlsModel = {
      isExportingBlueprint,
      isBusy,
      isResettingAgent,
    }

    const sidebarModel = {
      collapsed,
      pathname: window.location.pathname,
      user: session.user
        ? {
            email: session.user.email,
            firstName: session.user.firstName,
            lastName: session.user.lastName,
          }
        : null,
      userMenuOpen,
    }

    const sidebarKey = `${collapsed ? 1 : 0}:${window.location.pathname}:${session.user?.id ?? "null"}:${userMenuOpen ? 1 : 0}`
    const chatKey = `${renderedTailKey}:${isBusy ? 1 : 0}:${isAtBottom ? 1 : 0}:${toolContractsKey}`

    const blueprintKey = (() => {
      if (isEditingBlueprint) {
        return `edit:${isSavingBlueprint ? 1 : 0}:${blueprintError ?? ""}`
      }
      return `view:${isSavingBlueprint ? 1 : 0}:${blueprintLoading ? 1 : 0}:${blueprintError ?? ""}:${blueprintUpdatedAt ?? 0}`
    })()

    const controlsKey = `${isExportingBlueprint ? 1 : 0}:${isBusy ? 1 : 0}:${isResettingAgent ? 1 : 0}`

    const renderInput: AutopilotRouteRenderInput = {
      sidebarModel,
      sidebarKey,
      chatData: autopilotChatData,
      chatKey,
      blueprintModel: blueprintPanelModel,
      blueprintKey,
      controlsModel,
      controlsKey,
    }

    Effect.runPromise(runAutopilotRoute(input.container, renderInput)).catch(() => {})

    // Scroll after render when new content arrives and the user is pinned to bottom.
    if (isAtBottom && renderedMessages.length > 0 && renderedTailKey !== lastTailKey) {
      lastTailKey = renderedTailKey
      setTimeout(() => scrollToBottom(isStreaming ? "auto" : "smooth"), 0)
    }
  }

  const fetchBlueprint = async (options?: { silent?: boolean }) => {
    const silent = options?.silent ?? false
    if (!silent) {
      blueprintLoading = true
      scheduleRender()
    }
    blueprintError = null

    try {
      const json = await Effect.runPromise(input.api.getBlueprint(chatId))
      blueprint = json
      blueprintUpdatedAt = Date.now()
      if (!isEditingBlueprint) {
        // Keep draft in sync when not actively editing.
        const b: any = json ?? {}
        const docs = b?.docs ?? {}
        const identity = docs.identity ?? {}
        const user = docs.user ?? {}
        const character = docs.character ?? {}
        blueprintDraft = {
          userHandle: typeof user.addressAs === "string" ? user.addressAs : "",
          agentName: typeof identity.name === "string" ? identity.name : "",
          identityVibe: typeof identity.vibe === "string" ? identity.vibe : "",
          characterVibe: typeof character.vibe === "string" ? character.vibe : "",
          characterBoundaries: Array.isArray(character.boundaries)
            ? character.boundaries
                .map((s: unknown) => (typeof s === "string" ? s : ""))
                .filter(Boolean)
                .join("\n")
            : "",
        }
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
      const contracts = await Effect.runPromise(input.api.getToolContracts(chatId))
      const map: Record<string, AgentToolContract> = {}
      for (const c of contracts) map[c.name] = c
      toolContractsByName = map
      scheduleRender()
    } catch {
      // Best-effort (tool contract cards degrade).
    }
  }

  const ensureBlueprintPolling = (busy: boolean) => {
    if (busy && blueprintPollInterval == null) {
      blueprintPollInterval = window.setInterval(() => {
        void fetchBlueprint({ silent: true })
      }, 1000)
      return
    }
    if (!busy && blueprintPollInterval != null) {
      window.clearInterval(blueprintPollInterval)
      blueprintPollInterval = null
    }
  }

  // Subscribe to atoms.
  const unsubSession = atoms.subscribe(SessionAtom, (next) => {
    session = next
    scheduleRender()
  }, { immediate: false })

  const unsubCollapsed = atoms.subscribe(AutopilotSidebarCollapsedAtom, (next) => {
    collapsed = next
    scheduleRender()
  }, { immediate: false })

  const unsubUserMenu = atoms.subscribe(AutopilotSidebarUserMenuOpenAtom, (next) => {
    userMenuOpen = next
    scheduleRender()
  }, { immediate: false })

  const unsubIsAtBottom = atoms.subscribe(AutopilotChatIsAtBottomAtom(chatId), (next) => {
    isAtBottom = next
    scheduleRender()
  }, { immediate: false })

  let prevBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"
  const unsubChat = atoms.subscribe(ChatSnapshotAtom(chatId), (next) => {
    chatSnapshot = next
    const busy = next.status === "submitted" || next.status === "streaming"
    ensureBlueprintPolling(busy)
    if (prevBusy && !busy) {
      // Refresh once after finishing.
      void fetchBlueprint({ silent: true })
    }
    prevBusy = busy
    scheduleRender()
  }, { immediate: false })

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
    const distanceFromBottom = target.scrollHeight - (target.scrollTop + target.clientHeight)
    atoms.set(AutopilotChatIsAtBottomAtom(chatId), distanceFromBottom <= thresholdPx)
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

  input.ez.set("autopilot.chat.stop", () => input.chat.stop(chatId))

  input.ez.set("autopilot.chat.send", ({ el, params }) =>
    Effect.gen(function* () {
      const isBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"
      if (isBusy) return

      const form = el instanceof HTMLFormElement ? el : null
      const inputEl = form?.querySelector<HTMLInputElement>('input[name="message"]') ?? null

      const text = String((params as any).message ?? "").trim()
      if (!text) return

      yield* Effect.sync(() => {
        if (inputEl) inputEl.value = ""
        inputDraft = ""
      })

      yield* input.chat.send(chatId, text).pipe(
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

  const onStartEditBlueprint = () => {
    if (!blueprint) return
    // Initialize draft from current blueprint.
    const b: any = blueprint ?? {}
    const docs = b?.docs ?? {}
    const identity = docs.identity ?? {}
    const user = docs.user ?? {}
    const character = docs.character ?? {}
    blueprintDraft = {
      userHandle: typeof user.addressAs === "string" ? user.addressAs : "",
      agentName: typeof identity.name === "string" ? identity.name : "",
      identityVibe: typeof identity.vibe === "string" ? identity.vibe : "",
      characterVibe: typeof character.vibe === "string" ? character.vibe : "",
      characterBoundaries: Array.isArray(character.boundaries)
        ? character.boundaries
            .map((s: unknown) => (typeof s === "string" ? s : ""))
            .filter(Boolean)
            .join("\n")
        : "",
    }
    isEditingBlueprint = true
    scheduleRender()
  }

  const onCancelEditBlueprint = () => {
    isEditingBlueprint = false
    scheduleRender()
  }

  const onSaveBlueprint = async () => {
    if (!blueprintDraft || !blueprint || isSavingBlueprint) return
    isSavingBlueprint = true
    blueprintError = null
    scheduleRender()

    const nowIso = new Date().toISOString()
    const nextBoundaries = blueprintDraft.characterBoundaries
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean)

    const next: any = JSON.parse(JSON.stringify(blueprint))
    next.exportedAt = nowIso

    next.docs.user.addressAs = blueprintDraft.userHandle.trim() || next.docs.user.addressAs
    next.docs.user.updatedAt = nowIso
    next.docs.user.updatedBy = "user"
    next.docs.user.version = Number(next.docs.user.version ?? 1) + 1

    next.docs.identity.name = blueprintDraft.agentName.trim() || next.docs.identity.name
    next.docs.identity.vibe = blueprintDraft.identityVibe.trim() || next.docs.identity.vibe
    next.docs.identity.updatedAt = nowIso
    next.docs.identity.updatedBy = "user"
    next.docs.identity.version = Number(next.docs.identity.version ?? 1) + 1

    next.docs.character.vibe = blueprintDraft.characterVibe.trim() || next.docs.character.vibe
    next.docs.character.boundaries = nextBoundaries
    next.docs.character.updatedAt = nowIso
    next.docs.character.updatedBy = "user"
    next.docs.character.version = Number(next.docs.character.version ?? 1) + 1

    try {
      await Effect.runPromise(
        input.telemetry.withNamespace("ui.blueprint").event("blueprint.save", {
          changed: [
            "user.handle",
            "identity.name",
            "identity.vibe",
            "character.vibe",
            "character.boundaries",
          ],
        })
      )
      await Effect.runPromise(input.api.importBlueprint(chatId, next))
      isEditingBlueprint = false
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

  input.ez.set("autopilot.blueprint.toggleEdit", () =>
    Effect.sync(() => {
      if (isEditingBlueprint) onCancelEditBlueprint()
      else onStartEditBlueprint()
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
    })
  )

  input.ez.set("autopilot.controls.exportBlueprint", () =>
    Effect.sync(() => {
      if (isExportingBlueprint) return
      isExportingBlueprint = true
      scheduleRender()

      void (async () => {
        try {
          const blueprintJson = await Effect.runPromise(input.api.getBlueprint(chatId))
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
      yield* input.chat.clearHistory(chatId)
      yield* Effect.sync(() => {
        inputDraft = ""
        const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]')
        if (inputEl) inputEl.value = ""
      })
    })
  )

  const resetAgent = async () => {
    if (isResettingAgent) return
    const isBusy = chatSnapshot.status === "submitted" || chatSnapshot.status === "streaming"
    if (isBusy) return
    const confirmed = window.confirm("Reset agent?\n\nThis will clear messages and reset your Blueprint to defaults.")
    if (!confirmed) return

    isResettingAgent = true
    blueprintError = null
    scheduleRender()

    try {
      await Effect.runPromise(input.api.resetAgent(chatId))
      inputDraft = ""
      const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]')
      if (inputEl) inputEl.value = ""
      await fetchBlueprint()
      const nextMessages = await Effect.runPromise(input.api.getMessages(chatId)).catch(() => [])
      await Effect.runPromise(input.chat.setMessages(chatId, nextMessages)).catch(() => {})
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

  // Start background work + initial render/hydration.
  Effect.runPromise(hydrateAuthedDotsGridBackground(input.container)).catch(() => {})
  void fetchBlueprint()
  void fetchToolContracts()
  ensureBlueprintPolling(prevBusy)
  scheduleRender()

  return {
    cleanup: () => {
      unsubSession()
      unsubCollapsed()
      unsubUserMenu()
      unsubChat()
      unsubIsAtBottom()

      if (blueprintPollInterval != null) {
        window.clearInterval(blueprintPollInterval)
        blueprintPollInterval = null
      }

      document.removeEventListener("click", onDocClick)
      document.removeEventListener("scroll", onScrollCapture, true)

      input.ez.delete("autopilot.sidebar.toggleCollapse")
      input.ez.delete("autopilot.sidebar.toggleUserMenu")
      input.ez.delete("autopilot.sidebar.logout")
      input.ez.delete("autopilot.chat.input")
      input.ez.delete("autopilot.chat.scrollBottom")
      input.ez.delete("autopilot.chat.stop")
      input.ez.delete("autopilot.chat.send")
      input.ez.delete("autopilot.blueprint.toggleEdit")
      input.ez.delete("autopilot.blueprint.refresh")
      input.ez.delete("autopilot.blueprint.save")
      input.ez.delete("autopilot.blueprint.draft")
      input.ez.delete("autopilot.controls.exportBlueprint")
      input.ez.delete("autopilot.controls.clearMessages")
      input.ez.delete("autopilot.controls.resetAgent")

      cleanupAuthedDotsGridBackground(input.container)
    },
  }
}
