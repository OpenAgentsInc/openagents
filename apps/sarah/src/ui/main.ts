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

import {
  isAvatarCleanupUnconfirmedError,
  startAvatarSession,
  type AvatarHandle,
} from "./avatar-session.ts"
import {
  applyAvatarCleanupObservation,
  makeAvatarSessionAttemptGate,
} from "./avatar-session-attempt-gate.ts"
import {
  beginBoundedAvatarStart,
  type AvatarStartAttempt,
  type AvatarStartDeadlineOutcome,
} from "./avatar-start-deadline.ts"
import { makeAvatarVideoElementLatch } from "./avatar-video-latch.ts"
import {
  beginBoundedAvatarStop,
  type AvatarStopAttempt,
  type AvatarStopDeadlineOutcome,
} from "./avatar-stop-deadline.ts"
import {
  blueprintMapFactFromDelta,
  blueprintMapFactFromProfileFact,
  blueprintMapProjection,
  type BlueprintMapFact,
} from "./blueprint-map-projection.ts"
import { sarahEffectNativeTheme } from "./theme.ts"
import {
  SarahCodingReceiptAction,
  SarahCodingReceiptEvidenceToggle,
  sarahCodingCloseoutReceiptView,
  sarahCodingReceiptIntents,
} from "./coding-closeout-receipt-view.ts"
import {
  SarahFleetApprovalDecisionRequested,
  SarahFleetAuditToggled,
  SarahFleetEvidenceOpened,
  SarahFleetRunControlRequested,
  SarahFleetWorkUnitOpened,
  sarahFleetRunSupervisionView,
  sarahFleetSupervisionIntents,
} from "./fleet-supervision-view.ts"
import type { SarahBlueprintDelta } from "../services/avatar-event-bus.ts"
import type { CustomerBlueprintDraft } from "../services/customer-blueprint.ts"
import {
  fleetContinuityProjection,
  type ConversationObservation,
  type MediaObservation,
  type MediaPresentation,
} from "../contracts/fleet-continuity-projection.ts"
import {
  projectSarahCodingCloseoutReceipts,
  type SarahCodingCloseoutReceipt,
} from "../contracts/coding-closeout-receipt.ts"
import {
  SARAH_OWNER_FLEET_INTERACTIVE,
  SARAH_OWNER_FLEET_READ_ONLY,
  type SarahOwnerFleetInteractionMode,
} from "./owner-fleet-interaction.ts"
import type { SarahFleetOwnerProjection } from "../contracts/fleet-owner-projection.ts"
import type { SarahFleetConnectionState } from "../services/fleet-sync-live-session.ts"
import {
  makeSarahFleetBrowserCoordinator,
  makeSarahFleetBrowserRuntime,
  parseSarahFleetBrowserConfig,
  type SarahFleetBrowserConfig,
  type SarahFleetBrowserViewState,
} from "../services/fleet-browser-host.ts"
import {
  makeSarahFleetStartConfigHandler,
  selectSarahFleetStartConfig,
} from "../services/fleet-start-result.ts"

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

type SarahReceipt = Readonly<{
  id: string
  key: string
  title: string
  body: string
  href?: string
  toolName?: string
  receiptRef?: string
  mode?: "dry_run" | "live"
  ok?: boolean
}>

const sarahPanelIds = ["blueprint", "fleet", "chat", "actions", "receipts"] as const
type SarahPanelId = (typeof sarahPanelIds)[number]

export type SarahOwnerFleetCloseoutState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "error" }>
  | Readonly<{ status: "not_reported" }>
  | Readonly<{
      status: "ready"
      receipts: ReadonlyArray<SarahCodingCloseoutReceipt>
    }>

/**
 * Optional owner-scoped Fleet tab input. Presence means the browser has an
 * exact run scope. Projection null is an honest pre-hydration/reconnect state,
 * never permission to infer a latest/display run.
 */
export type SarahOwnerFleetViewState = Readonly<{
  runRef: SarahFleetBrowserConfig["runRef"]
  scope: SarahFleetBrowserConfig["scope"]
  connection: SarahFleetConnectionState
  projection: SarahFleetOwnerProjection | null
  closeouts: SarahOwnerFleetCloseoutState
  expandedAuditWorkUnitRefs: ReadonlyArray<
    SarahFleetOwnerProjection["workUnits"][number]["workUnitRef"]
  >
  expandedReceiptCardRefs: ReadonlyArray<string>
}>

/**
 * KHS-7 (#8606) in-conversation account linking:
 * unknown → boot probe pending; anonymous → show the sign-in button;
 * linking → popup/poll in flight; linked → email badge.
 */
type AccountPhase = "unknown" | "anonymous" | "linking" | "linked"

export type SarahAvatarStopState =
  | Readonly<{ status: "idle" }>
  | Readonly<{ status: "stopping" }>
  | Readonly<{ status: "timed_out" }>
  | Readonly<{ status: "failed" }>

export type SarahAvatarStartState =
  | Readonly<{ status: "idle" }>
  | Readonly<{ status: "starting" }>
  | Readonly<{ status: "timed_out" }>
  | Readonly<{ status: "cleanup_unconfirmed" }>
  | Readonly<{ status: "failed" }>

export type SarahSurfaceState = Readonly<{
  status: "idle" | "thinking" | "connecting" | "live" | "error"
  /** Browser-observed video movement, independent from conversation state. */
  avatarMedia: MediaObservation
  avatarStart: SarahAvatarStartState
  avatarStop: SarahAvatarStopState
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
  pendingAction: "human_handoff" | null
  blueprintProspectRef: string | null
  blueprintDraft: CustomerBlueprintDraft | null
  blueprintFacts: ReadonlyArray<BlueprintMapFact>
  blueprintContactEmail: string | null
  receiptsProspectRef: string | null
  receipts: ReadonlyArray<SarahReceipt>
  ownerFleet?: SarahOwnerFleetViewState
}>

const initialState: SarahSurfaceState = {
  status: "idle",
  avatarMedia: { status: "not_requested" },
  avatarStart: { status: "idle" },
  avatarStop: { status: "idle" },
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
  pendingAction: null,
  blueprintProspectRef: null,
  blueprintDraft: null,
  blueprintFacts: [],
  blueprintContactEmail: null,
  receiptsProspectRef: null,
  receipts: [],
}

const InputChanged = defineIntent("SarahInputChanged", Schema.String)
const SendText = defineIntent("SarahSendText", Schema.String)
const StartAvatar = defineIntent("SarahStartAvatar", Schema.Null)
const StopAvatar = defineIntent("SarahStopAvatar", Schema.Null)
const ReconnectAvatarMedia = defineIntent("SarahReconnectAvatarMedia", Schema.Null)
const OpenLink = defineIntent("SarahOpenLink", Schema.String)
const ConnectAccount = defineIntent("SarahConnectAccount", Schema.Null)
const BookHumanHandoff = defineIntent("SarahBookHumanHandoff", Schema.Null)
const SelectPanel = defineIntent("SarahSelectPanel", Schema.Literals(sarahPanelIds))

const baseSarahIntents = [
  InputChanged,
  SendText,
  StartAvatar,
  StopAvatar,
  ReconnectAvatarMedia,
  OpenLink,
  ConnectAccount,
  BookHumanHandoff,
  SelectPanel,
] as const

export const sarahOwnerFleetHostIntents = [
  SarahFleetRunControlRequested,
  SarahFleetWorkUnitOpened,
  SarahFleetApprovalDecisionRequested,
  SarahFleetEvidenceOpened,
  SarahCodingReceiptAction,
] as const
export type SarahOwnerFleetHostIntentHandlers = IntentHandlers<
  typeof sarahOwnerFleetHostIntents
>

export type SarahSurfaceMountOptions = Readonly<{
  ownerFleet?: SarahOwnerFleetViewState
  ownerFleetHandlers?: SarahOwnerFleetHostIntentHandlers
  onOwnerFleetRunStarted?: (config: SarahFleetBrowserConfig) => void
}>

const hasCompleteSarahOwnerFleetHostHandlers = (
  handlers: SarahOwnerFleetHostIntentHandlers | undefined,
): handlers is SarahOwnerFleetHostIntentHandlers =>
  handlers !== undefined &&
  typeof handlers.SarahFleetRunControlRequested === "function" &&
  typeof handlers.SarahFleetWorkUnitOpened === "function" &&
  typeof handlers.SarahFleetApprovalDecisionRequested === "function" &&
  typeof handlers.SarahFleetEvidenceOpened === "function" &&
  typeof handlers.SarahCodingReceiptAction === "function"

/** Mount-safe mode: projection presence never enables host-bound actions. */
export const sarahOwnerFleetInteractionMode = (
  handlers: SarahOwnerFleetHostIntentHandlers | undefined,
): SarahOwnerFleetInteractionMode =>
  hasCompleteSarahOwnerFleetHostHandlers(handlers)
    ? SARAH_OWNER_FLEET_INTERACTIVE
    : SARAH_OWNER_FLEET_READ_ONLY

const sarahIntents = [
  ...baseSarahIntents,
  ...sarahFleetSupervisionIntents,
  ...sarahCodingReceiptIntents,
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
  const accessibleState =
    state.status === "live"
      ? state.sandbox
        ? "Live, sandbox"
        : "Live"
      : `${state.status[0]!.toUpperCase()}${state.status.slice(1)}`
  return Badge({
    key: "status",
    label,
    tone,
    a11y: { label: `Sarah conversation status: ${accessibleState}` },
  })
}

const avatarConversationObservation = (
  state: SarahSurfaceState,
): ConversationObservation => {
  switch (state.status) {
    case "idle":
      return { status: "idle" }
    case "connecting":
      return { status: "connecting" }
    case "live":
      return { status: "text_live" }
    case "thinking":
      return { status: "busy" }
    case "error":
      return { status: "failed" }
  }
}

/**
 * Reuses FC-3's one LIVE/stale law for the browser surface. The browser does
 * not call the admission projector because it cannot observe reservations,
 * provider capacity, or cost.
 */
export const sarahAvatarContinuityProjection = (
  state: SarahSurfaceState,
  nowMs: number,
) =>
  fleetContinuityProjection(
    {
      conversation: avatarConversationObservation(state),
      media: state.avatarMedia,
      progress: { status: "not_started" },
    },
    nowMs,
  )

const avatarMediaStatusTreatment = (
  state: SarahSurfaceState,
  nowMs: number,
): View | null => {
  if (!state.avatarSessionOpen) return null
  const media: MediaPresentation = sarahAvatarContinuityProjection(
    state,
    nowMs,
  ).media
  const controlsRemainCopy =
    state.ownerFleet === undefined
      ? "Text stays available."
      : "Text and Fleet controls stay available."
  const keepWorkingCopy =
    state.ownerFleet === undefined
      ? "Keep working in text."
      : "Keep working in text; Fleet controls remain available."
  const badge = (
    label: string,
    tone: "neutral" | "info" | "success" | "warn" | "danger",
    accessibleLabel: string,
  ) => Badge({
    key: "avatar-media-status",
    label,
    tone,
    a11y: { label: accessibleLabel },
  })

  if (state.avatarStart.status === "cleanup_unconfirmed") {
    return Stack(
      {
        key: "avatar-media-start-cleanup-unconfirmed",
        direction: "row",
        gap: "2",
        align: "center",
        a11y: {
          role: "group",
          label:
            `Sarah video status: Start and stop unconfirmed. A replacement video will not start. ${keepWorkingCopy}`,
        },
      },
      [
        badge(
          "VIDEO · START/STOP UNCONFIRMED",
          "danger",
          "Sarah video status: Start and stop unconfirmed",
        ),
        text(
          "avatar-media-start-cleanup-unconfirmed-copy",
          `Video cleanup is unconfirmed. ${controlsRemainCopy}`,
          "caption",
          "textMuted",
        ),
      ],
    )
  }
  if (state.avatarStop.status === "stopping") {
    return badge(
      "VIDEO · STOPPING",
      "info",
      "Sarah video status: Stopping the previous video session",
    )
  }
  if (
    state.avatarStop.status === "timed_out" ||
    state.avatarStop.status === "failed"
  ) {
    return Stack(
      {
        key: "avatar-media-stop-unconfirmed",
        direction: "row",
        gap: "2",
        align: "center",
        a11y: {
          role: "group",
          label:
            `Sarah video status: Stop unconfirmed. A replacement video will not start. ${keepWorkingCopy}`,
        },
      },
      [
        badge(
          "VIDEO · STOP UNCONFIRMED",
          "danger",
          "Sarah video status: Stop unconfirmed",
        ),
        text(
          "avatar-media-stop-unconfirmed-copy",
          `Previous video stop is unconfirmed. ${controlsRemainCopy}`,
          "caption",
          "textMuted",
        ),
      ],
    )
  }
  if (state.avatarStart.status === "starting") {
    return badge(
      "VIDEO · STARTING",
      "info",
      "Sarah video status: Starting",
    )
  }
  if (state.avatarStart.status === "timed_out") {
    return Stack(
      {
        key: "avatar-media-start-unconfirmed",
        direction: "row",
        gap: "2",
        align: "center",
        a11y: {
          role: "group",
          label:
            `Sarah video status: Start unconfirmed. A replacement video will not start until the pending start is resolved. ${keepWorkingCopy}`,
        },
      },
      [
        badge(
          "VIDEO · START UNCONFIRMED",
          "danger",
          "Sarah video status: Start unconfirmed",
        ),
        text(
          "avatar-media-start-unconfirmed-copy",
          `Pending video start is unresolved. ${controlsRemainCopy}`,
          "caption",
          "textMuted",
        ),
      ],
    )
  }

  switch (media.status) {
    case "not_requested":
      return null
    case "queued":
      return badge("VIDEO · QUEUED", "info", "Sarah video status: Queued")
    case "connecting":
      return badge(
        "VIDEO · CONNECTING",
        "info",
        "Sarah video status: Connecting",
      )
    case "live":
      return badge(
        "VIDEO · LIVE",
        "success",
        "Sarah video status: Live, moving frames",
      )
    case "stale":
      return Stack(
        {
          key: "avatar-media-reconnecting",
          direction: "row",
          gap: "2",
          align: "center",
          a11y: {
            role: "group",
            label:
              `Sarah video status: Reconnecting. Video paused. ${keepWorkingCopy}`,
          },
        },
        [
          badge(
            "VIDEO · RECONNECTING",
            "warn",
            "Sarah video status: Reconnecting",
          ),
          text(
            "avatar-media-reconnecting-copy",
            `Video paused. ${controlsRemainCopy}`,
            "caption",
            "textMuted",
          ),
          Button({
            key: "avatar-media-reconnect",
            label: "Reconnect video",
            variant: "secondary",
            onPress: IntentRef("SarahReconnectAvatarMedia"),
          }),
        ],
      )
    case "unavailable":
    case "evicted":
      return Stack(
        {
          key: "avatar-media-unavailable",
          direction: "row",
          gap: "2",
          align: "center",
          a11y: {
            role: "group",
            label:
              `Sarah video status: Unavailable. ${keepWorkingCopy}`,
          },
        },
        [
          badge(
            "VIDEO · UNAVAILABLE",
            "danger",
            "Sarah video status: Unavailable",
          ),
          text(
            "avatar-media-unavailable-copy",
            `Video unavailable. ${controlsRemainCopy}`,
            "caption",
            "textMuted",
          ),
          Button({
            key: "avatar-media-reconnect",
            label: "Reconnect video",
            variant: "secondary",
            onPress: IntentRef("SarahReconnectAvatarMedia"),
          }),
        ],
      )
    case "ended":
      return badge("VIDEO · ENDED", "neutral", "Sarah video status: Ended")
  }
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

const avatarControl = (
  state: SarahSurfaceState,
  keySuffix = "overlay",
): View => {
  if (state.avatarStop.status === "stopping") {
    return Button({
      key: `avatar-start-${keySuffix}`,
      label: "Stopping video…",
      variant: "secondary",
      disabled: true,
      onPress: IntentRef("SarahStartAvatar"),
    })
  }
  if (
    state.avatarStop.status !== "idle" ||
    state.avatarStart.status === "cleanup_unconfirmed" ||
    state.avatarStart.status === "timed_out"
  ) {
    return Button({
      key: `avatar-${state.avatarSessionOpen ? "close" : "blocked"}-${keySuffix}`,
      label: state.avatarSessionOpen ? "Close video" : "Video unavailable",
      variant: "secondary",
      disabled: !state.avatarSessionOpen,
      onPress: IntentRef("SarahStopAvatar"),
    })
  }
  if (state.avatarStart.status === "starting") {
    return Button({
      key: `avatar-start-${keySuffix}`,
      label: "Starting video…",
      variant: "secondary",
      disabled: true,
      onPress: IntentRef("SarahStartAvatar"),
    })
  }
  if (state.avatarActive) {
    return Button({
      key: `avatar-stop-${keySuffix}`,
      label: "End",
      variant: "secondary",
      onPress: IntentRef("SarahStopAvatar"),
    })
  }
  return Button({
    key: `avatar-start-${keySuffix}`,
    label:
      state.avatarStart.status === "failed"
        ? "Try video again"
        : state.avatarArmed
          ? "Talk to Sarah"
          : "Avatar offline",
    variant: "primary",
    disabled: !state.avatarArmed || state.status === "connecting",
    onPress: IntentRef("SarahStartAvatar"),
  })
}

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

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}

const optionalString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() ? value : null

const toolDisplayName = (toolName: string): string => {
  switch (toolName) {
    case "human_handoff":
      return "Human handoff"
    case "checkout_link_create":
      return "Checkout link"
    case "deal_rules_evaluate":
      return "Deal rules"
    case "account_link":
      return "Account link"
    default:
      return toolName.replaceAll("_", " ")
  }
}

type SarahToolResult = Readonly<{
  toolCallId?: string
  toolName?: string
  ok?: boolean
  output?: unknown
}>

const receiptFromToolResult = (result: SarahToolResult): SarahReceipt | null => {
  const toolName = optionalString(result.toolName)
  if (!toolName) return null
  const output = asRecord(result.output)
  const receiptRef =
    optionalString(output.checkoutRef) ??
    optionalString(output.handoffRef) ??
    optionalString(output.quoteRef) ??
    optionalString(result.toolCallId) ??
    `${toolName}:${JSON.stringify(output).slice(0, 80)}`
  const href = optionalString(output.checkoutUrl) ?? optionalString(output.url)
  const mode = output.mode === "live" || output.mode === "dry_run" ? output.mode : undefined
  const title = `${toolDisplayName(toolName)} ${result.ok === false ? "failed" : "recorded"}`
  const body =
    optionalString(output.message) ??
    optionalString(output.error) ??
    `${toolDisplayName(toolName)} receipt recorded.`
  return {
    id: `tool:${receiptRef}`,
    key: `receipt-tool-${receiptRef}`,
    title,
    body,
    ...(href ? { href } : {}),
    toolName,
    receiptRef,
    ...(mode ? { mode } : {}),
    ...(typeof result.ok === "boolean" ? { ok: result.ok } : {}),
  }
}

const receiptFromStoredTool = (tool: Record<string, unknown>): SarahReceipt | null =>
  receiptFromToolResult({
    toolCallId: optionalString(tool.toolCallId) ?? undefined,
    toolName: optionalString(tool.toolName) ?? undefined,
    ok: typeof tool.ok === "boolean" ? tool.ok : undefined,
    output: {
      checkoutRef: optionalString(tool.checkoutRef),
      checkoutUrl: optionalString(tool.checkoutUrl),
      handoffRef: optionalString(tool.handoffRef),
      quoteRef: optionalString(tool.quoteRef),
      mode: tool.mode,
      message: optionalString(tool.summary),
    },
  })

const accountLinkReceipt = (email: string | null): SarahReceipt => ({
  id: `account_link:${email ?? "linked"}`,
  key: `receipt-account-${email ?? "linked"}`,
  title: "Account link recorded",
  body: email
    ? `Conversation history and credits are linked to ${email}.`
    : "Conversation history and credits are linked to this OpenAgents account.",
  toolName: "account_link",
  receiptRef: "sarah.account_link.v1",
  ok: true,
})

const guardReceipt = (reply: string): SarahReceipt => ({
  id: `guard:${reply.slice(0, 96)}`,
  key: `receipt-guard-${Math.abs(reply.split("").reduce((sum, char) => sum + char.charCodeAt(0), 0))}`,
  title: "Guard refusal recorded",
  body: reply,
  receiptRef: "sarah.guard_refusal.v1",
  ok: true,
})

const cardReceipt = (card: Omit<SarahCard, "key">): SarahReceipt => ({
  id: `card:${card.title}:${card.body}:${card.href ?? ""}`,
  key: `receipt-card-${Math.abs(`${card.title}:${card.body}:${card.href ?? ""}`.split("").reduce((sum, char) => sum + char.charCodeAt(0), 0))}`,
  title: card.title,
  body: card.body,
  ...(card.href ? { href: card.href } : {}),
  receiptRef: "sarah.card_event.v1",
})

const mergeReceipts = (
  current: ReadonlyArray<SarahReceipt>,
  next: ReadonlyArray<SarahReceipt>,
): ReadonlyArray<SarahReceipt> => {
  const byId = new Map<string, SarahReceipt>()
  for (const receipt of [...current, ...next]) byId.set(receipt.id, receipt)
  return [...byId.values()].slice(-30)
}

const receiptItem = (receipt: SarahReceipt): View & { key: string } =>
  keyed(Card(
    {
      key: receipt.key,
      padding: "3",
      radius: "lg",
      style: {
        backgroundColor: "surfaceRaised",
        borderColor: "border",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      Stack(
        { key: `${receipt.key}-head`, direction: "row", gap: "2", align: "center", style: { width: "full" } },
        [
          text(`${receipt.key}-title`, receipt.title, "label", "focus"),
          ...(receipt.mode ? [Badge({ key: `${receipt.key}-mode`, label: receipt.mode, tone: receipt.mode === "live" ? "success" : "neutral" })] : []),
        ],
      ),
      text(`${receipt.key}-body`, receipt.body, "body"),
      ...(receipt.receiptRef
        ? [text(`${receipt.key}-ref`, receipt.receiptRef, "caption", "textMuted")]
        : []),
      ...(receipt.href
        ? [
            Button({
              key: `${receipt.key}-open`,
              label: receipt.toolName === "checkout_link_create" ? "Open checkout" : "Open",
              variant: "secondary",
              onPress: IntentRef("SarahOpenLink", StaticPayload(receipt.href)),
            }),
          ]
        : []),
    ],
  ))

const latestCheckoutReceipt = (state: SarahSurfaceState): SarahReceipt | null =>
  [...state.receipts]
    .reverse()
    .find((receipt) => receipt.toolName === "checkout_link_create" && receipt.href) ?? null

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
        a11y: { label: "Message Sarah" },
        onChange: IntentRef("SarahInputChanged", ComponentValueBinding()),
        onSubmit: IntentRef("SarahSendText", ComponentValueBinding()),
        style: { flex: 1 },
      }),
      Button({
        key: "composer-send",
        label: state.status === "thinking" ? "Sending…" : "Send",
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
  const checkout = latestCheckoutReceipt(state)
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
    [
      accountAction,
      Button({
        key: "actions-book-human",
        label: state.pendingAction === "human_handoff" ? "Booking…" : "Book a human",
        variant: "primary",
        disabled: state.pendingAction !== null,
        onPress: IntentRef("SarahBookHumanHandoff"),
      }),
      checkout
        ? Button({
            key: "actions-open-checkout",
            label: "Open checkout",
            variant: "secondary",
            onPress: IntentRef("SarahOpenLink", StaticPayload(checkout.href ?? "")),
          })
        : Badge({ key: "actions-checkout-empty", label: "No checkout link yet", tone: "neutral" }),
      Spacer({ key: "actions-fill", flex: true }),
    ],
  )
}

const blueprintDraftCodeCard = (draft: CustomerBlueprintDraft): View & { key: string } =>
  keyed(Card(
    {
      key: "receipts-blueprint-code",
      padding: "3",
      radius: "lg",
      style: {
        backgroundColor: "surfaceRaised",
        borderColor: "border",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      text("receipts-blueprint-code-title", `Blueprint draft v${draft.revision}`, "label", "focus"),
      text(
        "receipts-blueprint-json",
        JSON.stringify(draft, null, 2),
        "caption",
        "textPrimary",
      ),
    ],
  ))

const receiptsPanel = (state: SarahSurfaceState): View => {
  const items: Array<View & { key: string }> = [
    ...(state.blueprintDraft ? [blueprintDraftCodeCard(state.blueprintDraft)] : []),
    ...state.receipts.map(receiptItem),
    ...(state.receipts.length === 0 ? state.cards.map(cardItem) : []),
  ]
  return Stack(
    {
      key: "receipts-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    items.length
      ? [
          List(
            { key: "receipts-cards", style: { width: "full", flex: 1, minHeight: 0 } },
            items,
          ),
        ]
      : [text("receipts-empty", "No receipts yet.", "body", "textMuted")],
  )
}

const ownerFleetCloseoutViews = (
  ownerFleet: SarahOwnerFleetViewState,
  interactionMode: SarahOwnerFleetInteractionMode,
): ReadonlyArray<View> => {
  const closeouts = ownerFleet.closeouts
  if (closeouts.status === "ready") {
    if (closeouts.receipts.length === 0) {
      return [
        text(
          "fleet-closeouts-empty",
          "No coding closeouts in this projection.",
          "body",
          "textMuted",
        ),
      ]
    }
    const expanded = new Set(ownerFleet.expandedReceiptCardRefs)
    return closeouts.receipts.map((receipt) =>
      sarahCodingCloseoutReceiptView(receipt, {
        evidenceExpanded: expanded.has(receipt.cardRef),
        interactionMode,
      }),
    )
  }

  const presentation =
    closeouts.status === "loading"
      ? {
          label: "Closeouts loading",
          tone: "info" as const,
          message: "Coding closeout receipts are loading.",
        }
      : closeouts.status === "error"
        ? {
            label: "Closeouts unavailable",
            tone: "danger" as const,
            message: "Coding closeout receipts are unavailable.",
          }
        : {
            label: "Closeouts not reported",
            tone: "neutral" as const,
            message: "Coding closeout receipts have not been reported.",
          }

  return [
    Stack(
      {
        key: `fleet-closeouts-${closeouts.status}`,
        direction: "column",
        gap: "2",
        a11y: {
          role: "group",
          label: `${presentation.label}. ${presentation.message}`,
        },
        style: { width: "full" },
      },
      [
        Badge({
          key: `fleet-closeouts-${closeouts.status}-badge`,
          label: presentation.label,
          tone: presentation.tone,
        }),
        text(
          `fleet-closeouts-${closeouts.status}-message`,
          presentation.message,
          "body",
          "textMuted",
        ),
      ],
    ),
  ]
}

const fleetConnectionView = (
  connection: SarahFleetConnectionState,
): View & { key: string } => {
  const presentation = (() => {
    switch (connection.phase) {
      case "live":
        return {
          label: "Fleet live",
          tone: "success" as const,
          message: "Owner Fleet projection is live for this exact run.",
        }
      case "reconnecting":
        return {
          label: "Fleet reconnecting",
          tone: "info" as const,
          message: connection.error.messageSafe,
        }
      case "must_refetch":
        return {
          label: "Fleet refreshing",
          tone: "info" as const,
          message: `A fresh exact-run snapshot is required (${connection.reason.replaceAll("_", " ")}).`,
        }
      case "failed":
        return {
          label: "Fleet unavailable",
          tone: "danger" as const,
          message: connection.error.messageSafe,
        }
      case "stopped":
        return {
          label: "Fleet stopped",
          tone: "neutral" as const,
          message: "The exact-run Fleet connection is stopped.",
        }
      case "idle":
      case "catching_up":
      case "connecting":
        return {
          label: "Fleet loading",
          tone: "info" as const,
          message: "Loading the owner-safe projection for this exact run.",
        }
    }
  })()
  return keyed(
    Stack(
      {
        key: `fleet-connection-${connection.phase}`,
        direction: "column",
        gap: "1",
        padding: "2",
        a11y: {
          role: "group",
          label: `${presentation.label}. ${presentation.message}`,
        },
        style: { width: "full" },
      },
      [
        Badge({
          key: `fleet-connection-${connection.phase}-badge`,
          label: presentation.label,
          tone: presentation.tone,
        }),
        text(
          `fleet-connection-${connection.phase}-message`,
          presentation.message,
          "caption",
          "textMuted",
        ),
      ],
    ),
  )
}

const fleetPanel = (
  ownerFleet: SarahOwnerFleetViewState,
  interactionMode: SarahOwnerFleetInteractionMode,
): View =>
  Stack(
    {
      key: "fleet-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", height: "full", minHeight: 0 },
    },
    [
      List(
        {
          key: "fleet-panel-list",
          style: { width: "full", flex: 1, minHeight: 0 },
        },
        [
          fleetConnectionView(ownerFleet.connection),
          ...(interactionMode === SARAH_OWNER_FLEET_READ_ONLY
            ? [
                keyed(
                  Stack(
                    {
                      key: "fleet-controls-unavailable",
                      direction: "column",
                      gap: "1",
                      padding: "2",
                      a11y: {
                        role: "group",
                        label:
                          "Fleet controls unavailable. This surface is read-only; fleet state and evidence references remain visible.",
                      },
                      style: { width: "full" },
                    },
                    [
                      Badge({
                        key: "fleet-controls-unavailable-badge",
                        label: "Read-only",
                        tone: "neutral",
                      }),
                      text(
                        "fleet-controls-unavailable-copy",
                        "Fleet controls unavailable in this surface. State and evidence references remain read-only.",
                        "caption",
                        "textMuted",
                      ),
                    ],
                  ),
                ),
              ]
            : []),
          ...(ownerFleet.projection === null
            ? []
            : [
                keyed(
                  sarahFleetRunSupervisionView(ownerFleet.projection, {
                    expandedAuditWorkUnitRefs:
                      ownerFleet.expandedAuditWorkUnitRefs,
                    interactionMode,
                  }),
                ),
              ]),
          ...(ownerFleet.projection === null
            ? []
            : [
                keyed(
                  Stack(
                    {
                      key: "fleet-closeouts",
                      direction: "column",
                      gap: "3",
                      a11y: {
                        role: "region",
                        label: "Fleet coding closeouts",
                      },
                      style: { width: "full" },
                    },
                    [
                      text(
                        "fleet-closeouts-title",
                        "Coding closeouts",
                        "title",
                      ),
                      ...ownerFleetCloseoutViews(ownerFleet, interactionMode),
                    ],
                  ),
                ),
              ]),
        ],
      ),
    ],
  )

export const sarahSurfaceView = (
  state: SarahSurfaceState,
  interactionMode: SarahOwnerFleetInteractionMode =
    SARAH_OWNER_FLEET_READ_ONLY,
): View => {
  const account = accountChip(state)
  const ownerFleet = state.ownerFleet
  const selectedPanel =
    state.activePanel === "fleet" && ownerFleet === undefined
      ? "blueprint"
      : state.activePanel
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
          ...(ownerFleet === undefined ? [] : [{ id: "fleet", label: "Fleet" }]),
          { id: "chat", label: "Chat" },
          { id: "actions", label: "Actions" },
          { id: "receipts", label: "Receipts" },
        ],
        panels: [
          { id: "blueprint", content: blueprintMapPanel(state) },
          ...(ownerFleet === undefined
            ? []
            : [{ id: "fleet", content: fleetPanel(ownerFleet, interactionMode) }]),
          { id: "chat", content: chatPanel(state) },
          { id: "actions", content: actionsPanel(state) },
          { id: "receipts", content: receiptsPanel(state) },
        ],
        selectedId: selectedPanel,
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
export const sarahAvatarPaneView = (
  state: SarahSurfaceState,
  nowMs = Date.now(),
): View => {
  const mediaStatus = avatarMediaStatusTreatment(state, nowMs)
  return Stack(
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
          ...(mediaStatus === null ? [] : [mediaStatus]),
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

const appendCardReceipt = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  card: Omit<SarahCard, "key">,
) =>
  SubscriptionRef.update(state, (current): SarahSurfaceState => ({
    ...current,
    cards: [...current.cards, { key: nextKey("c"), ...card }].slice(-20),
    receipts: mergeReceipts(current.receipts, [cardReceipt(card)]),
  }))

const appendReceipts = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  receipts: ReadonlyArray<SarahReceipt>,
) =>
  receipts.length === 0
    ? Effect.void
    : SubscriptionRef.update(state, (current): SarahSurfaceState => ({
        ...current,
        receipts: mergeReceipts(current.receipts, receipts),
      }))

const replyLooksLikeGuardRefusal = (reply: string): boolean =>
  reply.includes("won't improvise discounts") ||
  reply.includes("can't share another prospect") ||
  reply.includes("need a human owner")

const sendTextTurn = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
  message: string,
  onOwnerFleetRunStarted?: (config: SarahFleetBrowserConfig) => void,
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
    const turn = yield* Effect.tryPromise({
      try: async () => {
        const response = await fetch(`${API}/eve/turn`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ message: trimmed }),
        })
        const data = (await response.json()) as {
          reply?: string
          modelPath?: string
          toolResults?: unknown
        }
        return {
          reply: data.reply ?? "(no reply)",
          modelPath: data.modelPath ?? null,
          toolResults: Array.isArray(data.toolResults) ? data.toolResults : [],
        }
      },
      catch: () => new Error("turn_failed"),
    }).pipe(Effect.catch(() => Effect.succeed({
      reply: "I hit a connection problem — try that again in a moment.",
      modelPath: null,
      toolResults: [],
    })))
    const selectedFleet = selectSarahFleetStartConfig(turn.toolResults)
    if (selectedFleet !== null && onOwnerFleetRunStarted !== undefined) {
      yield* Effect.sync(() => {
        try {
          onOwnerFleetRunStarted(selectedFleet)
        } catch {
          // The exact Fleet result remains receipted even if browser
          // navigation/runtime boot is temporarily unavailable.
        }
      })
    }
    yield* appendTranscript(state, "assistant", turn.reply)
    yield* appendReceipts(
      state,
      [
        ...turn.toolResults.flatMap((result) => {
          const receipt = receiptFromToolResult(result as SarahToolResult)
          return receipt ? [receipt] : []
        }),
        ...(turn.modelPath === "deterministic_guard" &&
        turn.toolResults.length === 0 &&
        replyLooksLikeGuardRefusal(turn.reply)
          ? [guardReceipt(turn.reply)]
          : []),
      ],
    )
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

type ReceiptSeedResponse = Readonly<{
  prospect?: boolean
  prospectRef?: string | null
  receipts?: ReadonlyArray<Record<string, unknown>>
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

const applyReceiptSeed = (
  state: SarahSurfaceState,
  seed: ReceiptSeedResponse,
): SarahSurfaceState => {
  if (seed.prospect === false) {
    return {
      ...state,
      receiptsProspectRef: null,
      receipts: [],
    }
  }
  const prospectRef =
    typeof seed.prospectRef === "string" ? seed.prospectRef : state.receiptsProspectRef
  const receipts = (seed.receipts ?? []).flatMap((tool) => {
    const receipt = receiptFromStoredTool(tool)
    return receipt ? [receipt] : []
  })
  return {
    ...state,
    receiptsProspectRef: prospectRef,
    receipts:
      prospectRef !== null && prospectRef === state.receiptsProspectRef
        ? mergeReceipts(state.receipts, receipts)
        : receipts,
  }
}

const loadCurrentReceipts = (
  state: SubscriptionRef.SubscriptionRef<SarahSurfaceState>,
) =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetch(`${API}/session/receipts/current`)
      if (!response.ok) throw new Error(`receipt_seed_${response.status}`)
      return (await response.json()) as ReceiptSeedResponse
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  }).pipe(
    Effect.flatMap((seed) =>
      SubscriptionRef.update(state, (current): SarahSurfaceState =>
        applyReceiptSeed(current, seed),
      ),
    ),
    Effect.catch(() => Effect.void),
  )

const validEmail = (value: string | null): string | null =>
  value && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) ? value : null

const humanHandoffArgs = (state: SarahSurfaceState) => {
  const summary = state.transcript
    .slice(-8)
    .map((entry) => `${entry.role}: ${entry.text}`)
    .join("\n")
    .slice(0, 1800)
  return {
    reason: "prospect_requested_human_handoff",
    summary: summary || "The prospect requested an operator handoff from the Sarah Actions tab.",
    urgency: "normal",
    prospectName: null,
    contactEmail: validEmail(state.accountEmail ?? state.blueprintContactEmail),
    company: null,
    nextStep: "OpenAgents operator should review the Sarah conversation and follow up.",
    sourceRef: "sarah.actions_tab.v1",
  }
}

const toggledRefList = (
  refs: ReadonlyArray<string>,
  ref: string,
): ReadonlyArray<string> =>
  refs.includes(ref) ? refs.filter((candidate) => candidate !== ref) : [...refs, ref]

export const mountSarahSurface = (
  container: HTMLElement,
  avatarContainer: HTMLElement,
  options: SarahSurfaceMountOptions = {},
) =>
  Effect.gen(function* () {
    const interactionMode = sarahOwnerFleetInteractionMode(
      options.ownerFleetHandlers,
    )
    const ownerFleetHandlers = hasCompleteSarahOwnerFleetHostHandlers(
      options.ownerFleetHandlers,
    )
      ? options.ownerFleetHandlers
      : undefined
    const state = yield* SubscriptionRef.make<SarahSurfaceState>(
      options.ownerFleet === undefined
        ? initialState
        : { ...initialState, ownerFleet: options.ownerFleet },
    )
    const program = makeViewProgramFromState(state, (current) =>
      sarahSurfaceView(current, interactionMode),
    )
    const avatarProgram = makeViewProgramFromState(state, sarahAvatarPaneView)
    const runtime = {
      avatar: null as AvatarHandle | null,
      avatarGate: makeAvatarSessionAttemptGate(),
      pendingStart: null as AvatarStartAttempt<AvatarHandle> | null,
      pendingStop: null as AvatarStopAttempt | null,
      disposed: false,
    }

    // The media-video host driver hands us the EN-owned <video> attach
    // target; avatar-session awaits it through acquireVideo (the element
    // appears when avatarSessionOpen mounts the MediaVideo node).
    const videoLatch = makeAvatarVideoElementLatch()
    const mediaVideoDriver = makeMediaVideoDriver({
      onElement: (element) => {
        videoLatch.supply(element)
        return () => {
          videoLatch.clear(element)
        }
      },
    })
    const avatarPane = {
      container: avatarContainer,
      acquireVideo: videoLatch.acquire,
    }

    const runInBackground = <A, E>(effect: Effect.Effect<A, E>) =>
      Effect.runPromise(Effect.catch(effect, () => Effect.void) as Effect.Effect<void, never>)

    const observeLateStop = (attempt: AvatarStopAttempt) => {
      void attempt.completion.then((terminal) => {
        if (runtime.pendingStop !== attempt) return
        runtime.pendingStop = null
        if (terminal === "stopped") runtime.avatarGate.unblockReplacement()
        else runtime.avatarGate.blockReplacement()
        if (runtime.disposed) return
        void runInBackground(
          SubscriptionRef.update(state, (current): SarahSurfaceState => ({
            ...current,
            avatarStop:
              terminal === "stopped"
                ? { status: "idle" }
                : { status: "failed" },
            avatarStart:
              terminal === "stopped" &&
              (current.avatarStart.status === "timed_out" ||
                current.avatarStart.status === "cleanup_unconfirmed")
                ? { status: "idle" }
                : current.avatarStart,
            avatarMedia:
              terminal === "stopped"
                ? current.avatarMedia
                : { status: "unavailable" },
          })),
        )
      })
    }

    const stopAvatarWithinDeadline = (
      handle: AvatarHandle,
    ): Effect.Effect<AvatarStopDeadlineOutcome> =>
      Effect.tryPromise({
        try: async () => {
          const attempt = beginBoundedAvatarStop(() => handle.stop())
          const outcome = await attempt.outcome
          if (outcome === "timed_out") {
            runtime.pendingStop = attempt
            observeLateStop(attempt)
          }
          return outcome
        },
        catch: () => new Error("avatar_stop_deadline_failed"),
      }).pipe(
        Effect.catch(() => Effect.succeed("failed" as const)),
      )

    const recordStopOutcome = (
      outcome: AvatarStopDeadlineOutcome,
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        if (outcome === "stopped") {
          runtime.pendingStop = null
          runtime.avatarGate.unblockReplacement()
        } else {
          runtime.avatarGate.blockReplacement()
        }
        yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          avatarStop:
            outcome === "stopped"
              ? { status: "idle" }
              : outcome === "timed_out"
                ? { status: "timed_out" }
                : { status: "failed" },
          avatarMedia:
            outcome === "stopped"
              ? current.avatarMedia
              : { status: "unavailable" },
        }))
      })

    const cleanupRejectedAvatar = (
      handle: AvatarHandle,
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        const outcome = yield* stopAvatarWithinDeadline(handle)
        if (runtime.disposed) return
        yield* recordStopOutcome(outcome)
        yield* SubscriptionRef.update(
          state,
          (current): SarahSurfaceState => ({
            ...current,
            avatarStart:
              outcome === "stopped"
                ? { status: "idle" }
                : { status: "cleanup_unconfirmed" },
          }),
        )
      })

    const observeLateStart = (
      attempt: AvatarStartAttempt<AvatarHandle>,
    ) => {
      void attempt.completion.then((terminal) => {
        if (runtime.pendingStart !== attempt) {
          if (terminal.status === "started") {
            void runInBackground(cleanupRejectedAvatar(terminal.value))
          }
          return
        }
        runtime.pendingStart = null
        if (terminal.status === "started") {
          void runInBackground(cleanupRejectedAvatar(terminal.value))
          return
        }
        if (terminal.status === "cleanup_unconfirmed") {
          runtime.avatarGate.blockReplacement()
          if (runtime.disposed) return
          void runInBackground(
            SubscriptionRef.update(state, (current): SarahSurfaceState => ({
              ...current,
              avatarStart: { status: "cleanup_unconfirmed" },
              avatarStop: { status: "failed" },
              avatarMedia: { status: "unavailable" },
            })),
          )
          return
        }
        runtime.avatarGate.unblockReplacement()
        if (runtime.disposed) return
        void runInBackground(
          SubscriptionRef.update(state, (current): SarahSurfaceState => ({
            ...current,
            avatarStart: { status: "failed" },
            avatarMedia: { status: "unavailable" },
          })),
        )
      })
    }

    const startAvatar = (keepPaneOnFailure: boolean) =>
      Effect.gen(function* () {
        const generation = runtime.avatarGate.nextAttempt()
        yield* SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          status: keepPaneOnFailure ? "live" : "connecting",
          avatarActive: false,
          avatarMedia: { status: "connecting" },
          avatarStart: { status: "starting" },
          avatarStop: { status: "idle" },
          // Mount the EN MediaVideo host so acquireVideo can resolve.
          avatarSessionOpen: true,
        }))
        const attempt = beginBoundedAvatarStart(
          () => startAvatarSession(avatarPane, {
              onState: (avatarState) => {
                if (!runtime.avatarGate.accepts(generation)) return
                void runInBackground(
                  SubscriptionRef.update(state, (current): SarahSurfaceState => ({
                    ...current,
                    avatarActive: avatarState === "live",
                    avatarSessionOpen:
                      avatarState === "ended"
                        ? false
                        : current.avatarSessionOpen,
                    status:
                      avatarState === "live"
                        ? "live"
                        : avatarState === "error"
                          ? keepPaneOnFailure
                            ? "live"
                            : "error"
                          : avatarState === "ended"
                            ? "idle"
                            : keepPaneOnFailure
                              ? "live"
                              : "connecting",
                    sandbox: runtime.avatar?.sandbox ?? current.sandbox,
                  })),
                )
              },
              onMedia: (avatarMedia) => {
                if (!runtime.avatarGate.accepts(generation)) return
                void runInBackground(
                  SubscriptionRef.update(
                    state,
                    (current): SarahSurfaceState => ({
                      ...current,
                      avatarMedia,
                    }),
                  ),
                )
              },
              onCleanup: (cleanup) => {
                if (!runtime.avatarGate.accepts(generation)) return
                if (cleanup === "pending") {
                  applyAvatarCleanupObservation(runtime.avatarGate, cleanup)
                  void runInBackground(
                    SubscriptionRef.update(
                      state,
                      (current): SarahSurfaceState => ({
                        ...current,
                        avatarActive: false,
                        avatarSessionOpen: true,
                        avatarMedia: { status: "unavailable" },
                        avatarStop: { status: "stopping" },
                      }),
                    ),
                  )
                  return
                }
                if (cleanup === "confirmed") {
                  runtime.avatar = null
                  applyAvatarCleanupObservation(runtime.avatarGate, cleanup)
                  void runInBackground(
                    SubscriptionRef.update(
                      state,
                      (current): SarahSurfaceState => ({
                        ...current,
                        status: "idle",
                        avatarActive: false,
                        avatarSessionOpen: false,
                        avatarMedia: { status: "not_requested" },
                        avatarStart: { status: "idle" },
                        avatarStop: { status: "idle" },
                      }),
                    ),
                  )
                  return
                }
                applyAvatarCleanupObservation(runtime.avatarGate, cleanup)
                void runInBackground(
                  SubscriptionRef.update(
                    state,
                    (current): SarahSurfaceState => ({
                      ...current,
                      status: current.status === "live" ? "live" : "error",
                      avatarActive: false,
                      avatarSessionOpen: true,
                      avatarMedia: { status: "unavailable" },
                      avatarStart: { status: "cleanup_unconfirmed" },
                      avatarStop: { status: "failed" },
                    }),
                  ),
                )
              },
              onTranscript: (role, textValue) => {
                if (!runtime.avatarGate.accepts(generation)) return
                void runInBackground(appendTranscript(state, role, textValue))
              },
              onCard: (card) => {
                if (!runtime.avatarGate.accepts(generation)) return
                void runInBackground(appendCardReceipt(state, card))
              },
              onBlueprintDelta: (delta) => {
                if (!runtime.avatarGate.accepts(generation)) return
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
            }),
          {
            classifyFailure: (error) =>
              isAvatarCleanupUnconfirmedError(error)
                ? "cleanup_unconfirmed"
                : "failed",
          },
        )
        runtime.pendingStart = attempt
        const outcome: AvatarStartDeadlineOutcome<AvatarHandle> =
          yield* Effect.tryPromise({
            try: () => attempt.outcome,
            catch: () => new Error("avatar_start_deadline_failed"),
          }).pipe(
            Effect.catch(() => Effect.succeed({ status: "failed" } as const)),
          )

        if (outcome.status === "timed_out") {
          runtime.avatarGate.supersedeAttempt()
          runtime.avatarGate.blockReplacement()
          if (runtime.disposed) {
            observeLateStart(attempt)
            return
          }
          yield* SubscriptionRef.update(
            state,
            (current): SarahSurfaceState => ({
              ...current,
              status: keepPaneOnFailure ? "live" : "error",
              avatarActive: false,
              avatarSessionOpen: true,
              avatarMedia: { status: "unavailable" },
              avatarStart: { status: "timed_out" },
            }),
          )
          observeLateStart(attempt)
          return
        }

        runtime.pendingStart = null
        if (outcome.status === "cleanup_unconfirmed") {
          runtime.avatar = null
          runtime.avatarGate.supersedeAttempt()
          runtime.avatarGate.blockReplacement()
          yield* appendTranscript(
            state,
            "assistant",
            "Video cleanup is unconfirmed, so another video will not start. Keep working in text.",
          )
          yield* SubscriptionRef.update(
            state,
            (current): SarahSurfaceState => ({
              ...current,
              status: keepPaneOnFailure ? "live" : "error",
              avatarActive: false,
              avatarSessionOpen: true,
              avatarMedia: { status: "unavailable" },
              avatarStart: { status: "cleanup_unconfirmed" },
              avatarStop: { status: "failed" },
            }),
          )
          return
        }
        if (outcome.status === "failed") {
          if (!runtime.avatarGate.accepts(generation)) return
          runtime.avatar = null
          yield* appendTranscript(
            state,
            "assistant",
            "Video couldn't start. Keep working in text or try the video again.",
          )
          yield* SubscriptionRef.update(
            state,
            (current): SarahSurfaceState => ({
              ...current,
              status: keepPaneOnFailure ? "live" : "error",
              avatarActive: false,
              avatarSessionOpen: keepPaneOnFailure,
              avatarMedia: { status: "unavailable" },
              avatarStart: { status: "failed" },
            }),
          )
          return
        }

        const handle = outcome.value
        if (!runtime.avatarGate.accepts(generation)) {
          yield* cleanupRejectedAvatar(handle)
          return
        }
        runtime.avatar = handle
        yield* SubscriptionRef.update(
          state,
          (current): SarahSurfaceState => ({
            ...current,
            sandbox: handle.sandbox,
            avatarStart: { status: "idle" },
          }),
        )
      })

    const withAvatarTransition = (
      transition: Effect.Effect<void>,
      replacement = false,
    ) =>
      Effect.gen(function* () {
        const began = replacement
          ? runtime.avatarGate.tryBeginReplacementTransition()
          : runtime.avatarGate.tryBeginTransition()
        if (!began) return
        yield* transition.pipe(
          Effect.ensuring(
            Effect.sync(() => {
              runtime.avatarGate.finishTransition()
            }),
          ),
        )
      })

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
          yield* sendTextTurn(
            state,
            trimmed,
            options.onOwnerFleetRunStarted,
          )
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
                yield* appendCardReceipt(state, {
                  title: "Account linked",
                  body: "Your conversation history and credits now follow you.",
                })
                yield* appendReceipts(state, [accountLinkReceipt(email)])
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
      SarahBookHumanHandoff: () =>
        Effect.gen(function* () {
          const current = yield* SubscriptionRef.get(state)
          if (current.pendingAction !== null) return
          yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
            ...s2,
            pendingAction: "human_handoff",
          }))
          yield* Effect.tryPromise({
            try: async () => {
              const response = await fetch(`${API}/eve/tool-call`, {
                method: "POST",
                headers: { "content-type": "application/json" },
                body: JSON.stringify({
                  toolName: "human_handoff",
                  toolCallId: `ui.handoff.${crypto.randomUUID()}`,
                  args: humanHandoffArgs(current),
                }),
              })
              if (!response.ok) throw new Error(`handoff_${response.status}`)
              return (await response.json()) as {
                reply?: string
                toolResults?: ReadonlyArray<SarahToolResult>
              }
            },
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }).pipe(
            Effect.flatMap((result) =>
              Effect.gen(function* () {
                yield* appendTranscript(
                  state,
                  "assistant",
                  result.reply ?? "Human handoff recorded.",
                )
                yield* appendReceipts(
                  state,
                  (result.toolResults ?? []).flatMap((tool) => {
                    const receipt = receiptFromToolResult(tool)
                    return receipt ? [receipt] : []
                  }),
                )
                yield* loadCurrentReceipts(state)
                yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
                  ...s2,
                  pendingAction: null,
                  activePanel: "receipts",
                }))
              }),
            ),
            Effect.catch(() =>
              Effect.gen(function* () {
                yield* appendTranscript(
                  state,
                  "assistant",
                  "I couldn't book the human handoff from here. Keep chatting and I'll try again when the tool rail recovers.",
                )
                yield* SubscriptionRef.update(state, (s2): SarahSurfaceState => ({
                  ...s2,
                  pendingAction: null,
                }))
              }),
            ),
          )
        }),
      SarahSelectPanel: (panel) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => ({
          ...current,
          activePanel:
            panel === "fleet" && current.ownerFleet === undefined
              ? current.activePanel
              : panel,
        })),
      SarahFleetRunControlRequested: (payload, intent) =>
        ownerFleetHandlers?.SarahFleetRunControlRequested(
          payload,
          intent,
        ) ?? Effect.void,
      SarahFleetWorkUnitOpened: (payload, intent) =>
        ownerFleetHandlers?.SarahFleetWorkUnitOpened(payload, intent) ??
        Effect.void,
      SarahFleetApprovalDecisionRequested: (payload, intent) =>
        ownerFleetHandlers?.SarahFleetApprovalDecisionRequested(
          payload,
          intent,
        ) ?? Effect.void,
      SarahFleetEvidenceOpened: (payload, intent) =>
        ownerFleetHandlers?.SarahFleetEvidenceOpened(payload, intent) ??
        Effect.void,
      SarahCodingReceiptAction: (payload, intent) =>
        ownerFleetHandlers?.SarahCodingReceiptAction(payload, intent) ??
        Effect.void,
      SarahFleetAuditToggled: ({ runRef, workUnitRef }) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => {
          const ownerFleet = current.ownerFleet
          if (
            ownerFleet === undefined ||
            ownerFleet.projection === null ||
            ownerFleet.projection.run.runRef !== runRef ||
            !ownerFleet.projection.workUnits.some(
              (workUnit) => workUnit.workUnitRef === workUnitRef,
            )
          ) {
            return current
          }
          return {
            ...current,
            ownerFleet: {
              ...ownerFleet,
              expandedAuditWorkUnitRefs: toggledRefList(
                ownerFleet.expandedAuditWorkUnitRefs,
                workUnitRef,
              ),
            },
          }
        }),
      SarahCodingReceiptEvidenceToggle: ({ cardRef }) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => {
          const ownerFleet = current.ownerFleet
          if (
            ownerFleet === undefined ||
            ownerFleet.projection === null ||
            ownerFleet.closeouts.status !== "ready" ||
            !ownerFleet.closeouts.receipts.some(
              (receipt) => receipt.cardRef === cardRef,
            )
          ) {
            return current
          }
          return {
            ...current,
            ownerFleet: {
              ...ownerFleet,
              expandedReceiptCardRefs: toggledRefList(
                ownerFleet.expandedReceiptCardRefs,
                cardRef,
              ),
            },
          }
        }),
      SarahStartAvatar: () =>
        withAvatarTransition(startAvatar(false), true),
      SarahReconnectAvatarMedia: () =>
        withAvatarTransition(
          Effect.gen(function* () {
            const handle = runtime.avatar
            runtime.avatar = null
            runtime.avatarGate.supersedeAttempt()
            yield* SubscriptionRef.update(
              state,
              (current): SarahSurfaceState => ({
                ...current,
                status: current.status === "live" ? "live" : "connecting",
                avatarActive: false,
                avatarSessionOpen: true,
                avatarMedia: { status: "connecting" },
                avatarStop: { status: "stopping" },
              }),
            )
            if (handle !== null) {
              const outcome = yield* stopAvatarWithinDeadline(handle)
              yield* recordStopOutcome(outcome)
              if (outcome !== "stopped") return
            } else {
              yield* SubscriptionRef.update(
                state,
                (current): SarahSurfaceState => ({
                  ...current,
                  avatarStop: { status: "idle" },
                }),
              )
            }
            yield* startAvatar(true)
          }),
          true,
        ),
      SarahStopAvatar: () =>
        withAvatarTransition(
          Effect.gen(function* () {
            const handle = runtime.avatar
            runtime.avatar = null
            runtime.avatarGate.supersedeAttempt()
            if (handle) {
              yield* SubscriptionRef.update(
                state,
                (current): SarahSurfaceState => ({
                  ...current,
                  avatarStop: { status: "stopping" },
                }),
              )
              const outcome = yield* stopAvatarWithinDeadline(handle)
              yield* recordStopOutcome(outcome)
            }
            yield* SubscriptionRef.update(
              state,
              (current): SarahSurfaceState => ({
                ...current,
                avatarActive: false,
                avatarSessionOpen: false,
                avatarMedia: { status: "not_requested" },
                status: "idle",
              }),
            )
          }),
        ),
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
    yield* loadCurrentReceipts(state)

    const openOwnerFleetWorkUnit = (input: Readonly<{
      runRef: string
      workUnitRef: string
      assignmentRef: string
    }>) =>
      SubscriptionRef.update(state, (current): SarahSurfaceState => {
        const ownerFleet = current.ownerFleet
        const workUnit = ownerFleet?.projection?.workUnits.find(
          (candidate) =>
            candidate.workUnitRef === input.workUnitRef &&
            candidate.assignmentRef === input.assignmentRef,
        )
        if (
          ownerFleet === undefined ||
          ownerFleet.runRef !== input.runRef ||
          ownerFleet.projection?.run.runRef !== input.runRef ||
          workUnit === undefined
        ) {
          return current
        }
        return {
          ...current,
          activePanel: "fleet",
          ownerFleet: {
            ...ownerFleet,
            expandedAuditWorkUnitRefs: ownerFleet.expandedAuditWorkUnitRefs.includes(
              workUnit.workUnitRef,
            )
              ? ownerFleet.expandedAuditWorkUnitRefs
              : [...ownerFleet.expandedAuditWorkUnitRefs, workUnit.workUnitRef],
          },
        }
      })

    const openOwnerFleetEvidence = (input: Readonly<{
      runRef: string
      workUnitRef: string
      assignmentRef: string
      evidenceKind: "verification" | "closeout"
      evidenceRef: string
    }>) =>
      SubscriptionRef.update(state, (current): SarahSurfaceState => {
        const ownerFleet = current.ownerFleet
        const workUnit = ownerFleet?.projection?.workUnits.find(
          (candidate) =>
            candidate.workUnitRef === input.workUnitRef &&
            candidate.assignmentRef === input.assignmentRef,
        )
        const expectedRef =
          input.evidenceKind === "verification"
            ? workUnit?.verification.verificationRef
            : workUnit?.closeout.closeoutRef
        if (
          ownerFleet === undefined ||
          ownerFleet.runRef !== input.runRef ||
          ownerFleet.projection?.run.runRef !== input.runRef ||
          workUnit === undefined ||
          expectedRef !== input.evidenceRef
        ) {
          return current
        }
        return {
          ...current,
          activePanel: "fleet",
          ownerFleet: {
            ...ownerFleet,
            expandedAuditWorkUnitRefs: ownerFleet.expandedAuditWorkUnitRefs.includes(
              workUnit.workUnitRef,
            )
              ? ownerFleet.expandedAuditWorkUnitRefs
              : [...ownerFleet.expandedAuditWorkUnitRefs, workUnit.workUnitRef],
          },
        }
      })

    const openOwnerFleetReceiptTarget = (targetRef: string) =>
      SubscriptionRef.update(state, (current): SarahSurfaceState => {
        const ownerFleet = current.ownerFleet
        if (ownerFleet?.closeouts.status !== "ready") return current
        const receipt = ownerFleet.closeouts.receipts.find((candidate) => {
          const next = candidate.sections[5].next
          return candidate.cardRef === targetRef || next.targetRef === targetRef
        })
        if (receipt === undefined) return current
        return {
          ...current,
          activePanel: "fleet",
          ownerFleet: {
            ...ownerFleet,
            expandedReceiptCardRefs: ownerFleet.expandedReceiptCardRefs.includes(
              receipt.cardRef,
            )
              ? ownerFleet.expandedReceiptCardRefs
              : [...ownerFleet.expandedReceiptCardRefs, receipt.cardRef],
          },
        }
      })

    return {
      setOwnerFleetViewState: (
        ownerFleet: SarahOwnerFleetViewState | undefined,
      ) =>
        SubscriptionRef.update(state, (current): SarahSurfaceState => {
          if (ownerFleet !== undefined) {
            const changedScope = current.ownerFleet?.scope !== ownerFleet.scope
            const retainedLocalState =
              !changedScope
                ? {
                    expandedAuditWorkUnitRefs:
                      current.ownerFleet.expandedAuditWorkUnitRefs,
                    expandedReceiptCardRefs:
                      current.ownerFleet.expandedReceiptCardRefs,
                  }
                : {}
            return {
              ...current,
              ownerFleet: { ...ownerFleet, ...retainedLocalState },
              activePanel: changedScope ? "fleet" : current.activePanel,
            }
          }
          const { ownerFleet: _removed, ...withoutOwnerFleet } = current
          return {
            ...withoutOwnerFleet,
            activePanel:
              current.activePanel === "fleet" ? "blueprint" : current.activePanel,
          }
        }),
      openOwnerFleetWorkUnit,
      openOwnerFleetEvidence,
      openOwnerFleetReceiptTarget,
      unmount: Effect.gen(function* () {
        const handle = runtime.avatar
        runtime.avatar = null
        runtime.avatarGate.dispose()
        runtime.disposed = true
        videoLatch.dispose()
        if (handle !== null) {
          yield* stopAvatarWithinDeadline(handle)
        }
        yield* avatarSurface.unmount
        yield* surface.unmount
      }),
    }
  })

const ownerFleetViewStateFromBrowser = (
  state: SarahFleetBrowserViewState,
): SarahOwnerFleetViewState => ({
  runRef: state.config.runRef,
  scope: state.config.scope,
  connection: state.connection,
  projection: state.projection,
  closeouts:
    state.projection === null
      ? { status: "not_reported" }
      : {
          status: "ready",
          receipts: projectSarahCodingCloseoutReceipts({
            projection: state.projection,
            evidence: [],
          }),
        },
  expandedAuditWorkUnitRefs: [],
  expandedReceiptCardRefs: [],
})

const boot = () => {
  const root = document.getElementById("sarah-root")
  const avatar = document.getElementById("sarah-avatar")
  if (!root || !avatar) return
  void Effect.runPromise(Scope.make()).then((scope) => {
    let mounted:
      | Effect.Success<ReturnType<typeof mountSarahSurface>>
      | null = null
    let closed = false

    const fleetCoordinator = makeSarahFleetBrowserCoordinator({
      makeRuntime: (config) =>
        makeSarahFleetBrowserRuntime({
          config,
          origin: window.location.origin,
          fetch: (input, init) => fetch(input, init),
        }),
      onState: (state) => {
        if (closed || mounted === null) return
        void Effect.runPromise(
          mounted.setOwnerFleetViewState(
            state === null ? undefined : ownerFleetViewStateFromBrowser(state),
          ),
        )
      },
    })
    const selectStartedFleet = makeSarahFleetStartConfigHandler({
      coordinator: fleetCoordinator,
      currentUrl: () => window.location.href,
      navigate: (url) => {
        const navigation = (
          window as Window & {
            navigation?: {
              navigate: (
                url: string,
                options?: { history?: "push" | "replace" },
              ) => unknown
            }
          }
        ).navigation
        if (navigation !== undefined) {
          navigation.navigate(url, { history: "push" })
          return
        }
        window.location.assign(url)
      },
    })

    const currentCommands = (runRef: string) => {
      const selected = fleetCoordinator.current()
      return selected?.config.runRef === runRef ? selected.commands : null
    }
    const handlers: SarahOwnerFleetHostIntentHandlers = {
      SarahFleetRunControlRequested: ({ runRef, action }) =>
        Effect.tryPromise({
          try: () => {
            const commands = currentCommands(runRef)
            return commands === null
              ? Promise.reject(new Error("fleet_command_unavailable"))
              : commands.runControl({ runRef, action })
          },
          catch: () => new Error("fleet_command_failed"),
        }).pipe(Effect.asVoid, Effect.catch(() => Effect.void)),
      SarahFleetApprovalDecisionRequested: ({
        runRef,
        approvalRef,
        decision,
      }) =>
        Effect.tryPromise({
          try: () => {
            const commands = currentCommands(runRef)
            return commands === null
              ? Promise.reject(new Error("fleet_command_unavailable"))
              : commands.approvalDecision({ runRef, approvalRef, decision })
          },
          catch: () => new Error("fleet_command_failed"),
        }).pipe(Effect.asVoid, Effect.catch(() => Effect.void)),
      SarahFleetWorkUnitOpened: (payload) =>
        mounted?.openOwnerFleetWorkUnit(payload) ?? Effect.void,
      SarahFleetEvidenceOpened: (payload) =>
        mounted?.openOwnerFleetEvidence(payload) ?? Effect.void,
      SarahCodingReceiptAction: (payload) => {
        if (payload.action !== "control_run") {
          return mounted?.openOwnerFleetReceiptTarget(payload.targetRef) ?? Effect.void
        }
        return Effect.tryPromise({
          try: () => {
            const commands = currentCommands(payload.targetRef)
            return commands === null
              ? Promise.reject(new Error("fleet_command_unavailable"))
              : commands.runControl({
                  runRef: payload.targetRef,
                  action: payload.runControl,
                })
          },
          catch: () => new Error("fleet_command_failed"),
        }).pipe(Effect.asVoid, Effect.catch(() => Effect.void))
      },
    }

    const reconcileFleetScope = () => {
      if (closed || mounted === null) return
      let config
      try {
        config = parseSarahFleetBrowserConfig(window.location.href)
      } catch {
        fleetCoordinator.setConfig(null)
        return
      }
      fleetCoordinator.setConfig(config)
    }

    const close = () => {
      if (closed) return
      closed = true
      window.removeEventListener("popstate", reconcileFleetScope)
      window.removeEventListener("pagehide", close)
      fleetCoordinator.dispose()
      const selected = mounted
      mounted = null
      void Effect.runPromise(
        Effect.gen(function* () {
          if (selected !== null) yield* selected.unmount
          yield* Scope.close(scope, Exit.void)
        }),
      )
    }

    void Effect.runPromise(
      Scope.provide(scope)(mountSarahSurface(root, avatar, {
        ownerFleetHandlers: handlers,
        onOwnerFleetRunStarted: selectStartedFleet,
      })),
    )
      .then((handle) => {
        mounted = handle
        window.addEventListener("popstate", reconcileFleetScope)
        window.addEventListener("pagehide", close, { once: true })
        reconcileFleetScope()
      })
      .catch((error) => {
        console.error("[sarah] surface mount failed", error)
        close()
      })
  })
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot)
} else {
  boot()
}
