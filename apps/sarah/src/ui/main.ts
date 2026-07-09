/**
 * Sarah — the Effect Native surface at openagents.com/sarah (#8598 AV-5).
 *
 * Authored entirely in the Effect Native component set on the DOM renderer —
 * zero React, no hand-rolled DOM for the UI tree. The avatar pane is an EN
 * `MediaVideo` host (catalog `media-video` kind, effect-native#67, vendored
 * v26): the media-video driver owns the <video> attach target and
 * avatar-session.ts only binds the live stream to it. The transcript is the
 * EN `Transcript` primitive (effect-native#35) — keyed role-tagged messages
 * with pin-to-end, carrying in-place partial-utterance updates once the
 * brain emits partials. Replaces the interim sarah.js shell and closes the
 * open SM-2 item on #8594; SQ-7 catalog gaps tracked in docs/sarah/EN-GAPS.md.
 */

import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  GraphFigure,
  IntentRef,
  StaticPayload,
  List,
  MediaVideo,
  Spacer,
  Stack,
  Tabs,
  Text,
  TextField,
  Transcript,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentHandlers,
  type IntentReporter,
  type TextView,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import { makeDomRenderer, makeMediaVideoDriver } from "@effect-native/render-dom"
import { Effect, Exit, Schema, Scope, SubscriptionRef } from "@effect-native/core/effect"

import { startAvatarSession, type AvatarHandle } from "./avatar-session.ts"
import {
  blueprintMapFactFromDelta,
  blueprintMapFactFromProfileFact,
  blueprintMapProjection,
  type BlueprintMapFact,
} from "./blueprint-map-projection.ts"
import { sarahEffectNativeTheme } from "./theme.ts"
import type { SarahBlueprintDelta } from "../services/avatar-event-bus.ts"
import type { CustomerBlueprintDraft } from "../services/customer-blueprint.ts"

const API = "/sarah/api"

type TranscriptEntry = Readonly<{
  key: string
  role: "user" | "assistant"
  text: string
}>

type SarahCard = Readonly<{
  key: string
  title: string
  body: string
  href?: string
}>

const sarahPanelIds = ["blueprint", "chat", "actions", "receipts"] as const
type SarahPanelId = (typeof sarahPanelIds)[number]

/**
 * KHS-7 (#8606) in-conversation account linking:
 * unknown → boot probe pending; anonymous → show the sign-in button;
 * linking → popup/poll in flight; linked → email badge.
 */
type AccountPhase = "unknown" | "anonymous" | "linking" | "linked"

type SarahSurfaceState = Readonly<{
  status: "idle" | "thinking" | "connecting" | "live" | "error"
  avatarArmed: boolean
  avatarActive: boolean
  /**
   * True from session start until the user ends it (or startup fails) — keeps
   * the EN MediaVideo host mounted for the whole session lifetime, mirroring
   * the previous imperative create-on-start / remove-on-stop element flow.
   */
  avatarSessionOpen: boolean
  sandbox: boolean
  input: string
  transcript: ReadonlyArray<TranscriptEntry>
  cards: ReadonlyArray<SarahCard>
  accountPhase: AccountPhase
  accountEmail: string | null
  activePanel: SarahPanelId
  blueprintProspectRef: string | null
  blueprintDraft: CustomerBlueprintDraft | null
  blueprintFacts: ReadonlyArray<BlueprintMapFact>
  blueprintContactEmail: string | null
}>

const initialState: SarahSurfaceState = {
  status: "idle",
  avatarArmed: false,
  avatarActive: false,
  avatarSessionOpen: false,
  sandbox: false,
  input: "",
  transcript: [
    {
      key: "welcome",
      role: "assistant",
      text: "I'm Sarah, an AI sales employee for OpenAgents. Start the avatar conversation or type below.",
    },
  ],
  cards: [],
  accountPhase: "unknown",
  accountEmail: null,
  activePanel: "blueprint",
  blueprintProspectRef: null,
  blueprintDraft: null,
  blueprintFacts: [],
  blueprintContactEmail: null,
}

const InputChanged = defineIntent("SarahInputChanged", Schema.String)
const SendText = defineIntent("SarahSendText", Schema.String)
const StartAvatar = defineIntent("SarahStartAvatar", Schema.Null)
const StopAvatar = defineIntent("SarahStopAvatar", Schema.Null)
const OpenLink = defineIntent("SarahOpenLink", Schema.String)
const ConnectAccount = defineIntent("SarahConnectAccount", Schema.Null)
const SelectPanel = defineIntent("SarahSelectPanel", Schema.Literals(sarahPanelIds))

const sarahIntents = [
  InputChanged,
  SendText,
  StartAvatar,
  StopAvatar,
  OpenLink,
  ConnectAccount,
  SelectPanel,
] as const

const keyed = <V extends View>(view: V): V & { key: string } =>
  view as V & { key: string }

const text = (
  key: string,
  content: string,
  variant: TextView["variant"] = "body",
  color: TextView["color"] = "textPrimary",
): TextView => Text({ key, content, variant, color })

const statusBadge = (state: SarahSurfaceState): View => {
  const tone =
    state.status === "live"
      ? "success"
      : state.status === "error"
        ? "danger"
        : state.status === "idle"
          ? "neutral"
          : "info"
  const label =
    state.status === "live"
      ? state.sandbox
        ? "LIVE · sandbox"
        : "LIVE"
      : state.status.toUpperCase()
  return Badge({ key: "status", label, tone })
}

/** Account chip (KHS-7): anonymous → sign-in button; linked → email badge. */
const accountChip = (state: SarahSurfaceState): View | null => {
  switch (state.accountPhase) {
    case "unknown":
      return null
    case "linked":
      return Badge({
        key: "account",
        label: state.accountEmail ?? "Account linked",
        tone: "success",
      })
    case "linking":
      return Badge({ key: "account", label: "Linking account…", tone: "info" })
    case "anonymous":
      return Button({
        key: "account-connect",
        label: "Create account / Sign in",
        variant: "secondary",
        onPress: IntentRef("SarahConnectAccount"),
      })
  }
}

const avatarControl = (state: SarahSurfaceState, keySuffix = "overlay"): View =>
  state.avatarActive
    ? Button({
        key: `avatar-stop-${keySuffix}`,
        label: "End",
        variant: "secondary",
        onPress: IntentRef("SarahStopAvatar"),
      })
    : Button({
        key: `avatar-start-${keySuffix}`,
        label: state.avatarArmed ? "Talk to Sarah" : "Avatar offline",
        variant: "primary",
        disabled: !state.avatarArmed || state.status === "connecting",
        onPress: IntentRef("SarahStartAvatar"),
      })

/**
 * A transcript entry as an EN Transcript message (effect-native#35): keyed and
 * role-tagged so partial-utterance text updates replace the message body in
 * place; the Card body keeps the exact visual of the previous List+Card shell.
 */
const transcriptMessage = (entry: TranscriptEntry): TranscriptMessage => ({
  key: entry.key,
  role: entry.role,
  body: [
    Card(
      {
        key: `${entry.key}-card`,
        padding: "3",
        radius: "lg",
        style: {
          backgroundColor: entry.role === "user" ? "surfaceRaised" : "surface",
          borderColor: "border",
          borderWidth: 1,
          width: "full",
        },
      },
      [
        text(`${entry.key}-role`, entry.role === "user" ? "YOU" : "SARAH", "caption", "textMuted"),
        text(`${entry.key}-text`, entry.text, "body"),
      ],
    ),
  ],
})

const cardItem = (card: SarahCard): View & { key: string } =>
  keyed(Card(
    {
      key: card.key,
      padding: "3",
      radius: "lg",
      style: {
        backgroundColor: "surfaceRaised",
        borderColor: "focus",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      text(`${card.key}-title`, card.title, "label", "focus"),
      text(`${card.key}-body`, card.body, "body"),
      ...(card.href
        ? [
            Button({
              key: `${card.key}-open`,
              label: "Open",
              variant: "secondary",
              onPress: IntentRef("SarahOpenLink", StaticPayload(card.href)),
            }),
          ]
        : []),
    ],
  ))

const blueprintMapPanel = (state: SarahSurfaceState): View => {
  const projection = blueprintMapProjection({
    draft: state.blueprintDraft,
    facts: state.blueprintFacts,
    contactEmail: state.blueprintContactEmail,
    accountLinked: state.accountPhase === "linked",
    live: state.status === "live" || state.status === "thinking",
  })
  return Stack(
    {
      key: "blueprint-map-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [
      GraphFigure({
        key: "blueprint-map-graph",
        nodes: projection.nodes,
        edges: projection.edges,
        layout: "precomputed",
        width: 760,
        height: 430,
        style: {
          width: "full",
          flex: 1,
          minHeight: 0,
          backgroundColor: "surface",
          borderColor: "border",
          borderWidth: 1,
          borderRadius: "lg",
        },
        a11y: { label: "Sarah Blueprint map" },
      }),
    ],
  )
}

const composerView = (state: SarahSurfaceState): View =>
  Stack(
    { key: "composer", direction: "row", gap: "3", align: "center", style: { width: "full" } },
    [
      TextField({
        key: "composer-input",
        value: state.input,
        placeholder: "Type if you prefer text…",
        onChange: IntentRef("SarahInputChanged", ComponentValueBinding()),
        onSubmit: IntentRef("SarahSendText", ComponentValueBinding()),
        style: { flex: 1 },
      }),
      Button({
        key: "composer-send",
        label: state.status === "thinking" ? "…" : "Send",
        variant: "primary",
        disabled: state.status === "thinking",
        onPress: IntentRef("SarahSendText", ComponentValueBinding("input")),
      }),
    ],
  )

const chatPanel = (state: SarahSurfaceState): View =>
  Stack(
    {
      key: "chat-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [
      Transcript({
        key: "transcript",
        pinToEnd: true,
        messages: state.transcript.map(transcriptMessage),
        style: { width: "full", flex: 1, minHeight: 0 },
      }),
      composerView(state),
    ],
  )

const actionsPanel = (state: SarahSurfaceState): View => {
  const accountAction =
    state.accountPhase === "linked"
      ? Badge({
          key: "actions-account-linked",
          label: state.accountEmail ?? "Account linked",
          tone: "success",
        })
      : state.accountPhase === "linking"
        ? Badge({ key: "actions-account-linking", label: "Linking account…", tone: "info" })
        : Button({
            key: "actions-account-connect",
            label: "Create account / Sign in",
            variant: "secondary",
            onPress: IntentRef("SarahConnectAccount"),
          })
  return Stack(
    {
      key: "actions-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [accountAction, Spacer({ key: "actions-fill", flex: true })],
  )
}

const receiptsPanel = (state: SarahSurfaceState): View =>
  Stack(
    {
      key: "receipts-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    state.cards.length
      ? [
          List(
            { key: "receipts-cards", style: { width: "full", flex: 1, minHeight: 0 } },
            state.cards.map(cardItem),
          ),
        ]
      : [text("receipts-empty", "No receipts yet.", "body", "textMuted")],
  )

export const sarahSurfaceView = (state: SarahSurfaceState): View => {
  const account = accountChip(state)
  return Stack(
    {
      key: "sarah-root",
      direction: "column",
      gap: "2",
      padding: "3",
      style: { backgroundColor: "background", height: "full", minHeight: "full", width: "full" },
    },
    [
      Stack(
        { key: "sarah-toolbar", direction: "row", gap: "3", align: "center", style: { width: "full" } },
        [
          Spacer({ key: "toolbar-space", flex: true }),
          statusBadge(state),
          ...(account ? [account] : []),
        ],
      ),
      Tabs({
        key: "sarah-tabs",
        tabs: [
          { id: "blueprint", label: "Blueprint map" },
          { id: "chat", label: "Chat" },
          { id: "actions", label: "Actions" },
          { id: "receipts", label: "Receipts" },
        ],
        panels: [
          { id: "blueprint", content: blueprintMapPanel(state) },
          { id: "chat", content: chatPanel(state) },
          { id: "actions", content: actionsPanel(state) },
          { id: "receipts", content: receiptsPanel(state) },
        ],
        selectedId: state.activePanel,
        keepMounted: true,
        onSelect: IntentRef("SarahSelectPanel", ComponentValueBinding()),
        style: { width: "full", flex: 1, minHeight: 0 },
      }),
    ],
  )
}

/**
 * The avatar pane view: the EN `MediaVideo` host is mounted for the lifetime
 * of a session (effect-native#67). While no session is open the pane renders
 * empty and the #sarah-avatar chrome (data-state CSS) shows the idle overlay.
 */
export const sarahAvatarPaneView = (state: SarahSurfaceState): View =>
  Stack(
    {
      key: "avatar-pane",
      direction: "column",
      style: { width: "full", height: "full", minHeight: "full" },
    },
    [
      ...(state.avatarSessionOpen
        ? [
            MediaVideo({
              key: "avatar-video",
              fit: "cover",
              muted: false,
              style: { width: "full", height: "full" },
              a11y: { label: "Sarah live avatar video" },
            }),
          ]
        : [Stack({ key: "avatar-empty", direction: "column", style: { width: "full", height: "full" } }, [])]),
      Stack(
        { key: "avatar-overlay", direction: "row", gap: "2", align: "center", style: { width: "full" } },
        [
          avatarControl(state),
          statusBadge(state),
        ],
      ),
    ],
  )

let entryCounter = 0
const nextKey = (prefix: string) => `${prefix}-${entryCounter++}`

const appendTranscript = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  role: "user" | "assistant",
  textValue: string,
) =>
  SubscriptionRef.update(state, (current): SarahSurfaceState => ({
    ...current,
    transcript: [...current.transcript, { key: nextKey("t"), role, text: textValue }].slice(-200),
  }))

const appendCard = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  card: Omit<SarahCard, "key">,
) =>
  SubscriptionRef.update(state, (current): SarahSurfaceState => ({
    ...current,
    cards: [...current.cards, { key: nextKey("c"), ...card }].slice(-20),
  }))

const sendTextTurn = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  message: string,
) =>
  Effect.gen(function* () {
    const trimmed = message.trim()
    if (!trimmed) return
    yield* appendTranscript(state, "user", trimmed)
    yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
      ...current,
      input: "",
      status: current.avatarActive ? current.status : "thinking",
    }))
    const reply = yield* Effect.tryPromise({
      try: async () => {
        const response = await fetch(`${API}/eve/turn`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ message: trimmed }),
        })
        const data = (await response.json()) as { reply?: string }
        return data.reply ?? "(no reply)"
      },
      catch: () => new Error("turn_failed"),
    }).pipe(Effect.catch(() => Effect.succeed("I hit a connection problem — try that again in a moment.")))
    yield* appendTranscript(state, "assistant", reply)
    yield* loadBlueprintMapSeed(state)
    yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
      ...current,
      status: current.avatarActive ? current.status : "idle",
    }))
  })

// --- KHS-7 in-conversation account linking (#8606) -------------------------
// /sarah is path-mounted on openagents.com, so the OpenAuth session cookie is
// first-party. Flow: if already signed in, link immediately; otherwise open
// the existing /login page in a popup (no new auth UI), poll the canonical
// same-origin /api/auth/session until authenticated, then POST the link.

const AUTH_POLL_INTERVAL_MS = 1500
const AUTH_POLL_TIMEOUT_MS = 180_000
/** Grace after the popup closes — the callback may land cookies just before. */
const POPUP_CLOSED_GRACE_MS = 4000

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

const fetchOpenAgentsAuthenticated = async (): Promise<boolean> => {
  try {
    const response = await fetch("/api/auth/session", {
      headers: { accept: "application/json" },
    })
    if (!response.ok) return false
    const data = (await response.json()) as { authenticated?: boolean }
    return data.authenticated === true
  } catch {
    return false
  }
}

const waitForOpenAgentsSession = async (popup: Window | null): Promise<boolean> => {
  const deadline = Date.now() + AUTH_POLL_TIMEOUT_MS
  let popupClosedAt: number | null = null
  while (Date.now() < deadline) {
    if (await fetchOpenAgentsAuthenticated()) return true
    if (popup?.closed) {
      popupClosedAt ??= Date.now()
      if (Date.now() - popupClosedAt > POPUP_CLOSED_GRACE_MS) return false
    }
    await sleep(AUTH_POLL_INTERVAL_MS)
  }
  return false
}

/**
 * Full connect flow. Resolves with the linked email (or null when the link
 * succeeded without a known email); throws when sign-in never completed or
 * the link call failed — the caller reverts the chip to anonymous.
 */
const connectOpenAgentsAccount = async (): Promise<string | null> => {
  let popup: Window | null = null
  if (!(await fetchOpenAgentsAuthenticated())) {
    popup = window.open("/login", "oa-login", "width=520,height=680,noopener=false")
    const authed = await waitForOpenAgentsSession(popup)
    if (!authed) {
      popup?.close()
      throw new Error("sign_in_not_completed")
    }
  }
  const response = await fetch("/sarah/api/account/link", { method: "POST" })
  popup?.close()
  if (!response.ok) throw new Error(`link_failed_${response.status}`)
  const data = (await response.json()) as { linked?: boolean; email?: string }
  if (data.linked !== true) throw new Error("link_not_confirmed")
  return data.email ?? null
}

type BlueprintSeedResponse = Readonly<{
  prospect?: boolean
  prospectRef?: string
  draft?: CustomerBlueprintDraft | null
  facts?: ReadonlyArray<{ fact?: string; sourceTurnId?: string; at?: string }>
  contact?: { email?: string | null; contactId?: string | null } | null
  storeConfigured?: boolean
}>

const mergeBlueprintFacts = (
  current: ReadonlyArray<BlueprintMapFact>,
  next: ReadonlyArray<BlueprintMapFact>,
): ReadonlyArray<BlueprintMapFact> => {
  const byKey = new Map<string, BlueprintMapFact>()
  for (const fact of [...current, ...next]) {
    byKey.set(`${fact.label}\u0000${fact.sourceTurnId}\u0000${fact.text}`, fact)
  }
  return [...byKey.values()].slice(-40)
}

const blueprintFactsFromSeed = (
  facts: BlueprintSeedResponse["facts"],
): ReadonlyArray<BlueprintMapFact> =>
  (facts ?? []).flatMap((fact) =>
    typeof fact.fact === "string" && typeof fact.sourceTurnId === "string"
      ? [blueprintMapFactFromProfileFact({
          fact: fact.fact,
          sourceTurnId: fact.sourceTurnId,
        })]
      : [],
  )

const applyBlueprintSeed = (
  state: SarahSurfaceState,
  seed: BlueprintSeedResponse,
): SarahSurfaceState => {
  if (seed.prospect === false) {
    return {
      ...state,
      blueprintProspectRef: null,
      blueprintDraft: null,
      blueprintFacts: [],
      blueprintContactEmail: null,
    }
  }
  const prospectRef =
    typeof seed.prospectRef === "string" ? seed.prospectRef : state.blueprintProspectRef
  const nextFacts = blueprintFactsFromSeed(seed.facts)
  const facts =
    prospectRef !== null && prospectRef === state.blueprintProspectRef
      ? mergeBlueprintFacts(state.blueprintFacts, nextFacts)
      : nextFacts
  const contactEmail = seed.contact?.email ?? seed.draft?.contacts.email ?? null
  return {
    ...state,
    blueprintProspectRef: prospectRef,
    blueprintDraft: seed.draft ?? null,
    blueprintFacts: facts,
    blueprintContactEmail: contactEmail,
  }
}

const applyBlueprintDelta = (
  state: SarahSurfaceState,
  delta: SarahBlueprintDelta,
): SarahSurfaceState => {
  const fact = blueprintMapFactFromDelta(delta)
  if (fact) {
    return {
      ...state,
      blueprintFacts: mergeBlueprintFacts(state.blueprintFacts, [fact]),
    }
  }
  if (delta.kind === "contact_linked") {
    return {
      ...state,
      blueprintContactEmail: delta.email ?? state.blueprintContactEmail,
    }
  }
  if (delta.kind === "account_linked") {
    return {
      ...state,
      accountPhase: "linked",
      accountEmail: delta.email ?? state.accountEmail,
      blueprintContactEmail: delta.email ?? state.blueprintContactEmail,
    }
  }
  return state
}

const loadBlueprintMapSeed = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
) =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetch(`${API}/customer-blueprint/current`)
      if (!response.ok) throw new Error(`blueprint_seed_${response.status}`)
      return (await response.json()) as BlueprintSeedResponse
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  }).pipe(
    Effect.flatMap((seed) =>
      SubscriptionRef.update(state, (current): SarahSurfaceState =>
        applyBlueprintSeed(current, seed),
      ),
    ),
    Effect.catch(() => Effect.void),
  )

export const mountSarahSurface = (container: HTMLElement, avatarContainer: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialState)
    const program = makeViewProgramFromState(state, sarahSurfaceView)
    const avatarProgram = makeViewProgramFromState(state, sarahAvatarPaneView)
    const runtime = { avatar: null as AvatarHandle | null }

    // The media-video host driver hands us the EN-owned <video> attach
    // target; avatar-session awaits it through acquireVideo (the element
    // appears when avatarSessionOpen mounts the MediaVideo node).
    let videoElement: HTMLVideoElement | null = null
    let videoWaiters: Array<(element: HTMLVideoElement) => void> = []
    const mediaVideoDriver = makeMediaVideoDriver({
      onElement: (element) => {
        videoElement = element
        for (const waiter of videoWaiters.splice(0)) waiter(element)
        return () => {
          videoElement = null
        }
      },
    })
    const acquireVideo = (): Promise<HTMLVideoElement> =>
      videoElement
        ? Promise.resolve(videoElement)
        : new Promise((resolve) => {
            videoWaiters.push(resolve)
          })
    const avatarPane = { container: avatarContainer, acquireVideo }

    const runInBackground = <A, E>(effect: Effect.Effect<A, E>) =>
      Effect.runPromise(Effect.catch(effect, () => Effect.void) as Effect.Effect<void, never>)

    const handlers: IntentHandlers<typeof sarahIntents> = {
      SarahInputChanged: (value) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({ ...current, input: value })),
      SarahSendText: (value) =>
        Effect.gen(function* () {
          const current = yield* SubscriptionRef.get(state)
          const message = typeof value === "string" && value.trim() ? value : current.input
          const trimmed = message.trim()
          if (!trimmed) return
          if (current.avatarActive && runtime.avatar) {
            // Route through the avatar loop so Sarah speaks the reply; the
            // transcript arrives via data-channel/SSE events.
            runtime.avatar.message(trimmed)
            yield* appendTranscript(state, "user", trimmed)
            yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({ ...s2, input: "" }))
            return
          }
          yield* sendTextTurn(state, trimmed)
        }),
      SarahOpenLink: (href) =>
        Effect.sync(() => {
          window.open(href, "_blank", "noopener")
        }),
      SarahConnectAccount: () =>
        Effect.gen(function* () {
          const current = yield* SubscriptionRef.get(state)
          if (current.accountPhase === "linking" || current.accountPhase === "linked") return
          yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
            ...s2,
            accountPhase: "linking",
          }))
          yield* Effect.tryPromise({
            try: connectOpenAgentsAccount,
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }).pipe(
            Effect.flatMap((email) =>
              Effect.gen(function* () {
                yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
                  ...s2,
                  accountPhase: "linked",
                  accountEmail: email,
                }))
                yield* appendCard(state, {
                  title: "Account linked",
                  body: "Your conversation history and credits now follow you.",
                })
              }),
            ),
            Effect.catch(() =>
              Effect.gen(function* () {
                yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
                  ...s2,
                  accountPhase: "anonymous",
                }))
                yield* appendTranscript(
                  state,
                  "assistant",
                  "Sign-in didn't complete — no problem. The Create account button is here whenever you want your history and credits to follow you.",
                )
              }),
            ),
          )
        }),
      SarahSelectPanel: (panel) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          activePanel: panel,
        })),
      SarahStartAvatar: () =>
        Effect.gen(function* () {
          yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
            ...current,
            status: "connecting",
            // Mount the EN MediaVideo host so acquireVideo can resolve.
            avatarSessionOpen: true,
          }))
          yield* Effect.tryPromise({
            try: async () => {
              runtime.avatar = await startAvatarSession(avatarPane, {
                onState: (avatarState) => {
                  void runInBackground(
                    SubscriptionRef.update(state, (current): SarahSurfaceState => ({
                      ...current,
                      avatarActive: avatarState === "live" || avatarState === "connecting",
                      status:
                        avatarState === "live"
                          ? "live"
                          : avatarState === "error"
                            ? "error"
                            : avatarState === "ended"
                              ? "idle"
                              : "connecting",
                      sandbox: runtime.avatar?.sandbox ?? current.sandbox,
                    })),
                  )
                },
                onTranscript: (role, textValue) => {
                  void runInBackground(appendTranscript(state, role, textValue))
                },
                onCard: (card) => {
                  void runInBackground(appendCard(state, card))
                },
                onBlueprintDelta: (delta) => {
                  void runInBackground(
                    Effect.gen(function* () {
                      yield* SubscriptionRef.update(
                        state,
                        (current): SarahSurfaceState =>
                          applyBlueprintDelta(current, delta),
                      )
                      if (delta.kind === "draft_revision") {
                        yield* loadBlueprintMapSeed(state)
                      }
                    }),
                  )
                },
              })
            },
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }).pipe(
            Effect.catch((error) =>
              Effect.gen(function* () {
                const busy =
                  error instanceof Error && /busy|429|502/.test(error.message)
                yield* appendTranscript(
                  state,
                  "assistant",
                  busy
                    ? "My avatar line is busy right now — give it a minute and try again, or just type below and I'll answer here."
                    : "I couldn't start the avatar session — type below and I'll answer here while it recovers.",
                )
                yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
                  ...current,
                  status: "error",
                  avatarActive: false,
                  avatarSessionOpen: false,
                }))
              }),
            ),
          )
        }),
      SarahStopAvatar: () =>
        Effect.gen(function* () {
          const handle = runtime.avatar
          runtime.avatar = null
          if (handle) {
            yield* Effect.tryPromise({ try: () => handle.stop(), catch: () => new Error("stop") }).pipe(
              Effect.catch(() => Effect.void),
            )
          }
          yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
            ...current,
            avatarActive: false,
            avatarSessionOpen: false,
            status: "idle",
          }))
        }),
    }

    const registry = yield* makeIntentRegistry(sarahIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))

    const renderer = makeDomRenderer({
      theme: sarahEffectNativeTheme,
      hostDrivers: [mediaVideoDriver],
    })
    const surface = yield* renderer.mount(container, program.viewStream, report)
    const avatarSurface = yield* renderer.mount(
      avatarContainer,
      avatarProgram.viewStream,
      report,
    )

    // Arm state probe — avatar controls light up only when the key is set.
    yield* Effect.tryPromise({
      try: async () => {
        const response = await fetch(`${API}/avatar/status`)
        const status = (await response.json()) as { armed?: boolean; sandbox?: boolean }
        return status
      },
      catch: () => new Error("status_unavailable"),
    }).pipe(
      Effect.flatMap((status) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          avatarArmed: Boolean(status.armed),
          sandbox: Boolean(status.sandbox),
        })),
      ),
      Effect.catch(() => Effect.void),
    )

    // KHS-7 (#8606): account link probe — linked prospects get the email
    // badge; everyone else gets the sign-in button. Fail-soft to anonymous.
    yield* Effect.tryPromise({
      try: async () => {
        const response = await fetch(`${API}/account/status`)
        return (await response.json()) as { linked?: boolean; email?: string }
      },
      catch: () => new Error("account_status_unavailable"),
    }).pipe(
      Effect.flatMap((status) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          accountPhase: status.linked === true ? "linked" : "anonymous",
          accountEmail: status.linked === true ? (status.email ?? null) : null,
          blueprintContactEmail:
            status.linked === true
              ? (status.email ?? current.blueprintContactEmail)
              : current.blueprintContactEmail,
        })),
      ),
      Effect.catch(() =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          accountPhase: "anonymous",
        })),
      ),
    )

    yield* loadBlueprintMapSeed(state)

    return {
      unmount: Effect.gen(function* () {
        yield* avatarSurface.unmount
        yield* surface.unmount
      }),
    }
  })

const boot = () => {
  const root = document.getElementById("sarah-root")
  const avatar = document.getElementById("sarah-avatar")
  if (!root || !avatar) return
  void Effect.runPromise(Scope.make()).then((scope) => {
    void Effect.runPromise(
      Scope.provide(scope)(mountSarahSurface(root, avatar)),
    ).catch((error) => {
      console.error("[sarah] surface mount failed", error)
      void Effect.runPromise(Scope.close(scope, Exit.void))
    })
  })
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot)
} else {
  boot()
}
