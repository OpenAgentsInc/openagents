/**
 * Sarah — the Effect Native surface at openagents.com/sarah (#8598 AV-5).
 *
 * Authored entirely in the Effect Native component set on the DOM renderer —
 * zero React, no hand-rolled DOM for the UI tree (the avatar <video> lives in
 * a sibling container managed by avatar-session.ts; `media-video` Host kind is
 * filed as upstream demand). Replaces the interim sarah.js shell and closes
 * the open SM-2 item on #8594.
 */

import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  IntentRef,
  StaticPayload,
  List,
  Spacer,
  Stack,
  Text,
  TextField,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentHandlers,
  type IntentReporter,
  type TextView,
  type View,
} from "@effect-native/core"
import { makeDomRenderer } from "@effect-native/render-dom"
import { Effect, Exit, Schema, Scope, SubscriptionRef } from "@effect-native/core/effect"

import { startAvatarSession, type AvatarHandle } from "./avatar-session.ts"
import { sarahEffectNativeTheme } from "./theme.ts"

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
  sandbox: boolean
  input: string
  transcript: ReadonlyArray<TranscriptEntry>
  cards: ReadonlyArray<SarahCard>
  accountPhase: AccountPhase
  accountEmail: string | null
}>

const initialState: SarahSurfaceState = {
  status: "idle",
  avatarArmed: false,
  avatarActive: false,
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
}

const InputChanged = defineIntent("SarahInputChanged", Schema.String)
const SendText = defineIntent("SarahSendText", Schema.String)
const StartAvatar = defineIntent("SarahStartAvatar", Schema.Null)
const StopAvatar = defineIntent("SarahStopAvatar", Schema.Null)
const OpenLink = defineIntent("SarahOpenLink", Schema.String)
const ConnectAccount = defineIntent("SarahConnectAccount", Schema.Null)

const sarahIntents = [
  InputChanged,
  SendText,
  StartAvatar,
  StopAvatar,
  OpenLink,
  ConnectAccount,
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

const transcriptItem = (entry: TranscriptEntry): View & { key: string } =>
  keyed(Card(
    {
      key: entry.key,
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
  ))

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

export const sarahSurfaceView = (state: SarahSurfaceState): View => {
  const account = accountChip(state)
  return Stack(
    {
      key: "sarah-root",
      direction: "column",
      gap: "3",
      padding: "4",
      style: { backgroundColor: "background", minHeight: "full", width: "full" },
    },
    [
      Stack(
        { key: "header", direction: "row", gap: "3", align: "center", style: { width: "full" } },
        [
          text("title", "Sarah", "title"),
          text("subtitle", "OpenAgents sales · openagents.com/sarah", "caption", "textMuted"),
          Spacer({ key: "header-space", flex: true }),
          ...(account ? [account] : []),
          statusBadge(state),
        ],
      ),
      Stack(
        { key: "avatar-controls", direction: "row", gap: "3", style: { width: "full" } },
        [
          state.avatarActive
            ? Button({
                key: "avatar-stop",
                label: "End conversation",
                variant: "secondary",
                onPress: IntentRef("SarahStopAvatar"),
              })
            : Button({
                key: "avatar-start",
                label: state.avatarArmed ? "Talk to Sarah (live avatar)" : "Avatar offline",
                variant: "primary",
                disabled: !state.avatarArmed || state.status === "connecting",
                onPress: IntentRef("SarahStartAvatar"),
              }),
        ],
      ),
      List(
        {
          key: "transcript",
          pinToEnd: true,
          style: { width: "full" },
        },
        state.transcript.map(transcriptItem),
      ),
      ...(state.cards.length
        ? [
            List(
              { key: "cards", style: { width: "full" } },
              state.cards.map(cardItem),
            ),
          ]
        : []),
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
      ),
    ],
  )
}

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

export const mountSarahSurface = (container: HTMLElement, avatarContainer: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialState)
    const program = makeViewProgramFromState(state, sarahSurfaceView)
    const runtime = { avatar: null as AvatarHandle | null }

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
      SarahStartAvatar: () =>
        Effect.gen(function* () {
          yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
            ...current,
            status: "connecting",
          }))
          yield* Effect.tryPromise({
            try: async () => {
              runtime.avatar = await startAvatarSession(avatarContainer, {
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
            status: "idle",
          }))
        }),
    }

    const registry = yield* makeIntentRegistry(sarahIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))

    const surface = yield* makeDomRenderer({ theme: sarahEffectNativeTheme }).mount(
      container,
      program.viewStream,
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
        })),
      ),
      Effect.catch(() =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          accountPhase: "anonymous",
        })),
      ),
    )

    return { unmount: surface.unmount }
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
