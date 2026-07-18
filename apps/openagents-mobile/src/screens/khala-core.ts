import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import {
  Badge,
  Composer,
  Button,
  ComponentValueBinding,
  IntentRef,
  Stack,
  Spacer,
  StaticPayload,
  Text,
  Transcript,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import type { ChatMessageImageAttachment, FullAutoRunControlAction } from "@openagentsinc/khala-sync"
import type { MobileRuntimeControlAction } from "../conversation/mobile-conversation"
import type { MobileRuntimeQueueReceipt } from "../conversation/mobile-runtime-queue"
import type { MobileCodingComposerSession } from "../coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import {
  fleetRunActions,
  type LiveAgentGraphPresentation,
  type LiveAgentGraphPresentationRow,
  type LiveAgentGraphTone,
} from "@openagentsinc/khala-sync-client"
import {
  mobileAssistantContentViews,
  sanitizeOwnerConversationResponse,
} from "./mobile-transcript-content"
import {
  renderMobileComposerToolbar,
  type MobileComposerToolbarState,
} from "./mobile-composer-toolbar"
import { renderMobileComposerAttachments } from "./mobile-composer-attachments"
import {
  renderMobilePathAutocomplete,
  renderMobileSlashCommandAutocomplete,
  type MobileComposerPathDiscoveryState,
} from "./mobile-composer-discovery"
import {
  projectMobileComposerRunAdmission,
  renderMobileComposerRunControl,
} from "./mobile-composer-run-control"
import { renderMobileInteractionCard } from "./mobile-interaction-card"
import {
  mobileAttachmentRef,
  renderMobileAttachmentViewer,
  renderMobileTranscriptAttachments,
  type MobileAttachmentPreviewState,
} from "./mobile-transcript-attachment"
import {
  MOBILE_TRANSCRIPT_PAGE_SIZE,
  mobileTranscriptUnreadBoundaryIndex,
  visibleMobileTranscriptEntries,
} from "./mobile-transcript-history"
import {
  renderMobileWorkLog,
  type MobileWorkGroup,
} from "./mobile-work-log"

export type MobileTextScale = "normal" | "large" | "extra_large"

export interface MobileAccessibilityProfile {
  readonly reduceMotion: boolean
  readonly fontScale: number
  readonly textScale: MobileTextScale
  readonly minTouchTarget: number
}

export const defaultMobileAccessibilityProfile: MobileAccessibilityProfile = {
  reduceMotion: false,
  fontScale: 1,
  textScale: "normal",
  minTouchTarget: 44,
}

export const normalizeMobileAccessibilityProfile = (input: Partial<Pick<MobileAccessibilityProfile, "reduceMotion" | "fontScale">> = {}): MobileAccessibilityProfile => {
  const rawFontScale = Number.isFinite(input.fontScale) ? Number(input.fontScale) : 1
  const fontScale = Math.min(2, Math.max(0.85, Math.round(rawFontScale * 100) / 100))
  const textScale: MobileTextScale = fontScale >= 1.55
    ? "extra_large"
    : fontScale >= 1.25 ? "large" : "normal"
  return {
    reduceMotion: input.reduceMotion === true,
    fontScale,
    textScale,
    minTouchTarget: textScale === "extra_large" ? 56 : textScale === "large" ? 52 : 44,
  }
}

export const mobileInteractiveStyle = (accessibility: MobileAccessibilityProfile) => ({
  minHeight: accessibility.minTouchTarget,
  minWidth: accessibility.minTouchTarget,
})

/**
 * The public Khala mode: one conversation over the public orchestration
 * endpoint. It deliberately has no named-persona relationship, FleetRun, account, or
 * backing-model claim. The server owns routing and returns an honest failure.
 */
export type KhalaRole = "user" | "assistant" | "system" | "tool"

export interface KhalaEntry {
  readonly key: string
  readonly role: KhalaRole
  readonly text: string
  readonly status: "thinking" | "pending" | "done" | "failed"
  readonly createdAt?: string
  readonly version?: number
  readonly attachments?: ReadonlyArray<ChatMessageImageAttachment>
  readonly interaction?: KhalaInteraction
  readonly work?: MobileWorkGroup
  /** Server-authored route identity shown separately from assistant prose. */
  readonly provenanceLabel?: string
}

export type KhalaInteractionStatus = "pending" | "resolved" | "expired" | "revoked"
export type KhalaInteraction = Readonly<{
  kind: "provider_question" | "tool_approval" | "plan_review"
  interactionRef: string
  turnRef: string
  status: KhalaInteractionStatus
  title: string
  prompt: string
  questions: ReadonlyArray<Readonly<{
    questionRef: string
    displayText: string
    multiSelect: boolean
    options: ReadonlyArray<Readonly<{
      optionRef: string
      label: string
      description?: string
    }>>
  }>>
  decisionRef?: string
}>

export type KhalaRuntimeTurn = Readonly<{
  runRef: string
  status: "queued" | "running" | "waiting_for_input" | "completed" | "failed" | "canceled"
}>

/**
 * Live Full Auto run state header data (openagents #8982, extended
 * MOB-FA-02 #8994): objective + lifecycle state + rotation/lane/account/cap
 * + Pause/Resume/Stop control affordances + a bounded terminal run-report
 * summary, computed by `fullAutoRunHeaderForState` in `home-core.ts` and
 * rendered here, directly above the existing `Transcript`. `null` renders
 * nothing — the existing default surface is unchanged.
 */
export interface FullAutoRunHeaderView {
  readonly runRef: string
  readonly lifecycleLabel: string
  readonly objective: string
  readonly workspaceLabel: string
  /** MOB-FA-02 (#8994): current provider lane/account, or `null` when not
   * yet bound. Short public-safe refs only. */
  readonly laneRef: string | null
  readonly accountRef: string | null
  /** MOB-FA-02 (#8994): continuations-vs-cap, e.g. "6 / 20". */
  readonly turnCap: number
  readonly successfulAttempts: number
  readonly failedAttempts: number
  /** MOB-FA-02 (#8994): count of typed same-pass provider-lane rotations. */
  readonly rotationCount: number
  /** MOB-FA-02 (#8994): Pause/Resume/Stop remote-control affordance state. */
  readonly control: Readonly<{
    /** Actions legal from the run's current lifecycle state, in display
     * order. Empty when the run is terminal or control is unavailable on
     * this build. */
    availableActions: ReadonlyArray<FullAutoRunControlAction>
    /** Set while THIS device has an intent dispatched and awaiting a
     * durable outcome -- the UI must show "Pausing…" etc., never complete
     * from optimistic state. */
    pendingAction: FullAutoRunControlAction | null
    /** A short, honest status line for the most recent dispatched intent's
     * outcome (e.g. "Paused.", "Couldn't pause: illegal transition.",
     * "Still pending — Desktop hasn't responded yet."), or `null` when
     * nothing has been dispatched from this device this session. */
    lastOutcomeLabel: string | null
  }>
  /** MOB-FA-02 (#8994): the bounded run-report summary, present only once
   * the run reaches a terminal lifecycle state. */
  readonly receipt: Readonly<{
    successfulAttempts: number
    failedAttempts: number
    providerIdentities: ReadonlyArray<string>
    livenessGapCount: number
  }> | null
}

export interface KhalaState {
  readonly draft: string
  readonly entries: ReadonlyArray<KhalaEntry>
  readonly pending: boolean
  readonly turnCounter: number
  readonly interactionSelections: Readonly<Record<string, Readonly<Record<string, ReadonlyArray<string>>>>>
  readonly interactionSubmittingRef: string | null
  readonly interactionActionsAvailable: boolean
  readonly expandedWorkGroups: Readonly<Record<string, boolean>>
  readonly expandedWorkItems: Readonly<Record<string, boolean>>
  readonly transcriptVisibleCount: number
  readonly transcriptPinned: boolean
  readonly transcriptUnreadCount: number
  readonly transcriptScrollToKey: string | null
  readonly attachmentPreviewStates: Readonly<Record<string, MobileAttachmentPreviewState>>
  readonly attachmentRetryEpochs: Readonly<Record<string, number>>
  readonly viewingAttachmentRef: string | null
  readonly runtimeTurn: KhalaRuntimeTurn | null
  readonly runtimeControlSubmittingAction: MobileRuntimeControlAction | null
  readonly runtimeControlActionsAvailable: boolean
  readonly runtimeStopConfirmationRunRef: string | null
  readonly runtimeQueueReceipt: MobileRuntimeQueueReceipt | null
  /** Confirmed canonical live-agent hierarchy for the active thread, or null. */
  readonly agentGraph: LiveAgentGraphPresentation | null
  readonly agentGraphExpanded: boolean
  readonly selectedAgentRef: string | null
  readonly threadHistory: Readonly<{
    title: string
    totalMessageCount: number
    retainedMessageCount: number
    retainedEventCount: number
  }> | null
}

export const initialKhalaState: KhalaState = {
  draft: "",
  entries: [],
  pending: false,
  turnCounter: 0,
  interactionSelections: {},
  interactionSubmittingRef: null,
  interactionActionsAvailable: false,
  expandedWorkGroups: {},
  expandedWorkItems: {},
  transcriptVisibleCount: MOBILE_TRANSCRIPT_PAGE_SIZE,
  transcriptPinned: true,
  transcriptUnreadCount: 0,
  transcriptScrollToKey: null,
  attachmentPreviewStates: {},
  attachmentRetryEpochs: {},
  viewingAttachmentRef: null,
  runtimeTurn: null,
  runtimeControlSubmittingAction: null,
  runtimeControlActionsAvailable: false,
  runtimeStopConfirmationRunRef: null,
  runtimeQueueReceipt: null,
  agentGraph: null,
  agentGraphExpanded: false,
  selectedAgentRef: null,
  threadHistory: null,
}

export interface KhalaTurnClient {
  readonly sendTurn: (input: {
    readonly messages: ReadonlyArray<{ readonly role: KhalaRole; readonly content: string }>
  }) => Promise<{ readonly reply: string }>
}

export const KhalaDraftChanged = "KhalaDraftChanged"
export const KhalaTurnSubmitted = "KhalaTurnSubmitted"
export const AgentStackToggled = "AgentStackToggled"
export const AgentRowSelected = "AgentRowSelected"
export const WorkGroupToggled = "WorkGroupToggled"
export const WorkItemToggled = "WorkItemToggled"
export const TranscriptPinnedChanged = "TranscriptPinnedChanged"
export const TranscriptEarlierHistoryRequested = "TranscriptEarlierHistoryRequested"
export const TranscriptJumpToLatestRequested = "TranscriptJumpToLatestRequested"
export const TranscriptAttachmentOpened = "TranscriptAttachmentOpened"
export const TranscriptAttachmentLoadSettled = "TranscriptAttachmentLoadSettled"
export const TranscriptAttachmentRetryRequested = "TranscriptAttachmentRetryRequested"
export const TranscriptAttachmentViewerDismissed = "TranscriptAttachmentViewerDismissed"

/** Mobile renders at most this many hierarchy rows and names the remainder. */
export const MOBILE_AGENT_GRAPH_MAX_ROWS = 40
export const KHALA_TURN_FAILED_TEXT =
  "Khala could not respond just now. Check your connection and send that again."

const interactionBody = (
  state: KhalaState,
  entry: KhalaEntry,
  accessibility: MobileAccessibilityProfile,
  showProvenance: boolean,
): ReadonlyArray<View> => {
  if (entry.work !== undefined) {
    return [renderMobileWorkLog(
      entry.work,
      state.expandedWorkGroups[entry.work.groupRef] === true,
      state.expandedWorkItems,
      accessibility,
    )]
  }
  const interaction = entry.interaction
  if (interaction === undefined) {
    const textViews = entry.role === "assistant" && entry.status === "done"
      ? mobileAssistantContentViews(
          entry.key,
          entry.text,
          showProvenance ? entry.provenanceLabel : undefined,
        )
      : [Text({
          key: `${entry.key}-text`,
          content: entry.status === "thinking" ? "Khala is thinking…" : entry.text,
          variant: "body",
          color: entry.status === "failed" ? "danger" : "textPrimary",
        })]
    return [...textViews, ...renderMobileTranscriptAttachments(
      entry.key,
      entry.attachments ?? [],
      state.attachmentPreviewStates,
      state.attachmentRetryEpochs,
      accessibility,
    )]
  }
  const submitting = state.interactionSubmittingRef === interaction.interactionRef
  const selections = state.interactionSelections[interaction.interactionRef] ?? {}
  return [renderMobileInteractionCard(entry.key, interaction, {
    selections,
    submitting,
    actionsAvailable: state.interactionActionsAvailable,
  }, accessibility)]
}

const runtimeControlLabel = (
  action: MobileRuntimeControlAction,
  submitting: boolean,
): string => {
  if (!submitting) {
    switch (action) {
      case "cancel": return "Cancel turn"
      case "close": return "Close turn"
      case "resume": return "Resume"
      case "retry": return "Retry"
    }
  }
  switch (action) {
    case "cancel": return "Canceling…"
    case "close": return "Closing…"
    case "resume": return "Resuming…"
    case "retry": return "Retrying…"
  }
}

const runtimeControlActions = (
  state: KhalaState,
): ReadonlyArray<MobileRuntimeControlAction> => {
  const status = state.runtimeTurn?.status
  if (status === undefined) return []
  // Mobile and Desktop consume the same closed authoritative action table.
  // Pause remains a Desktop-only presentation until mobile has a distinct
  // pause transport; filtering it here avoids pretending cancel and pause are
  // different commands on the current native runtime.
  return fleetRunActions("live", status)
    .filter((action): action is MobileRuntimeControlAction => action !== "pause")
}

const runtimeControlViews = (
  state: KhalaState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const turn = state.runtimeTurn
  if (turn === null) return []
  if (turn.status === "queued" || turn.status === "running" || turn.status === "waiting_for_input") return []
  const actions = runtimeControlActions(state)
  if (actions.length === 0) return []
  return [Stack(
    {
      key: "khala-runtime-controls",
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full" },
    },
    actions.map((action) => {
      const submitting = state.runtimeControlSubmittingAction === action
      return Button({
        key: `khala-runtime-${action}`,
        label: runtimeControlLabel(action, submitting),
        variant: action === "resume" || (action === "retry" && actions.length === 2)
          ? "primary"
          : action === "close"
            ? "ghost"
            : "secondary",
        disabled: !state.runtimeControlActionsAvailable ||
          state.runtimeControlSubmittingAction !== null,
        onPress: IntentRef("RuntimeTurnControlRequested", StaticPayload({
          action,
          runRef: turn.runRef,
        })),
        style: mobileInteractiveStyle(accessibility),
      })
    }),
  )]
}

const compactRuntimeStatusViews = (
  state: KhalaState,
  assistantLabel: string,
): ReadonlyArray<View> => {
  const turn = state.runtimeTurn
  if (turn === null || (turn.status !== "queued" && turn.status !== "running" &&
      turn.status !== "waiting_for_input")) return []
  const work = state.entries.find(entry => entry.work?.runRef === turn.runRef)?.work
  const status = state.runtimeControlSubmittingAction === "cancel"
    ? `Stopping ${assistantLabel}…`
    : turn.status === "queued"
      ? `Starting ${assistantLabel}…`
      : turn.status === "waiting_for_input"
        ? `${assistantLabel} is waiting for you`
        : `${assistantLabel} is thinking…`
  const detail = turn.status === "queued"
    ? "Waiting for the hosted runtime"
    : turn.status === "waiting_for_input"
      ? work?.identityLabel ?? "OpenAgents hosted runtime"
      : work === undefined
        ? "Connecting to the hosted runtime"
        : `Generating with ${work.identityLabel}`
  return [Stack({
    key: "assistant-runtime-status",
    direction: "column",
    gap: "0.5",
    style: { width: "full" },
    a11y: { role: "region", label: `${status} ${detail}` },
  }, [
    Text({
      key: "assistant-runtime-status-label",
      content: status,
      variant: "caption",
      color: "accent",
      weight: "medium",
    }),
    Text({
      key: "assistant-runtime-status-identity",
      content: detail,
      variant: "caption",
      color: "textMuted",
    }),
  ])]
}

const agentBadgeTone = (tone: LiveAgentGraphTone): "neutral" | "info" | "success" | "warn" | "danger" =>
  tone === "active"
    ? "info"
    : tone === "attention"
      ? "warn"
      : tone === "success"
        ? "success"
        : tone === "danger"
          ? "danger"
          : "neutral"

export const mobileAgentRowDetailFields = (
  row: LiveAgentGraphPresentationRow,
): ReadonlyArray<Readonly<{ label: string; value: string }>> => [
  { label: "Status", value: row.statusLabel },
  { label: "Provider", value: row.providerLabel },
  { label: "Runtime", value: row.runtimeLabel },
  { label: "Session", value: row.sessionLabel },
  { label: "Worktree", value: row.worktreeLabel },
  { label: "Elapsed", value: row.elapsedLabel },
  { label: "Tokens", value: row.tokensLabel },
  ...(row.toolLabel === null ? [] : [{ label: "Current action", value: row.toolLabel }]),
  ...(row.attentionLabel === null ? [] : [{ label: "Attention", value: row.attentionLabel }]),
  ...(row.terminalLabel === null ? [] : [{ label: "Terminal", value: row.terminalLabel }]),
]

const agentRowAccessibilityLabel = (
  row: LiveAgentGraphPresentationRow,
  selected: boolean,
): string =>
  [
    row.label,
    row.statusLabel,
    row.toolLabel,
    row.attentionLabel,
    row.terminalLabel,
    row.elapsedLabel,
    `Tokens ${row.tokensLabel}`,
    selected ? "Hide agent details" : "Show agent details",
  ].filter((value): value is string => value !== null).join(". ")

const agentDepthSpacing = (depth: number) =>
  (["0", "2", "4", "6", "8", "10"] as const)[Math.min(depth, 5)] ?? "0"

const agentInspectorView = (
  row: LiveAgentGraphPresentationRow,
): View =>
  Stack(
    {
      key: `khala-agent-inspector-${row.agentRef}`,
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        marginLeft: agentDepthSpacing(row.depth + 1),
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "md",
      },
      a11y: { role: "region", label: `Agent details, ${row.label}` },
    },
    mobileAgentRowDetailFields(row).flatMap((field, index) => [
      Text({
        key: `khala-agent-field-${row.agentRef}-${index}`,
        content: `${field.label} · ${field.value}`,
        variant: "caption",
        color: "textMuted",
      }),
    ]),
  )

/**
 * Inline live-agent supervision stack rendered above the transcript. Rows are
 * the shared provider-neutral hierarchy presentation; a tap selects/inspects
 * the exact typed agent ref locally and never issues execution movement.
 * Historical authority is labeled and stays inspection-only.
 */
export const agentStackViews = (
  state: KhalaState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const graph = state.agentGraph
  if (graph === null || graph.rows.length === 0) return []
  const summary = `${graph.totalCount} agent${graph.totalCount === 1 ? "" : "s"} · ${graph.activeCount} active` +
    (graph.attentionCount === 0 ? "" : ` · ${graph.attentionCount} need attention`)
  const authorityDetail = graph.authority === "historical"
    ? " · Historical import · controls unavailable"
    : ""
  return [Stack(
    {
      key: "khala-agent-stack",
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
      a11y: { role: "region", label: `${graph.authorityLabel} agent stack` },
    },
    [
      Stack(
        {
          key: "khala-agent-stack-header",
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Badge({
            key: "khala-agent-stack-authority",
            label: graph.authorityLabel,
            tone: graph.authority === "live" ? "info" : "neutral",
          }),
          Button({
            key: "khala-agent-stack-toggle",
            label: `Agents · ${summary}`,
            variant: "ghost",
            onPress: IntentRef(AgentStackToggled, StaticPayload({})),
            a11y: {
              label: `${state.agentGraphExpanded ? "Collapse" : "Expand"} agent stack. ${summary}${authorityDetail}`,
            },
            style: { flex: 1, ...mobileInteractiveStyle(accessibility) },
          }),
        ],
      ),
      ...(state.agentGraphExpanded
        ? graph.rows.flatMap(row => {
            const selected = row.agentRef === state.selectedAgentRef
            return [
              Stack(
                {
                  key: `khala-agent-row-${row.agentRef}`,
                  direction: "row",
                  gap: "2",
                  align: "center",
                  style: { width: "full", paddingLeft: agentDepthSpacing(row.depth) },
                },
                [
                  Badge({
                    key: `khala-agent-status-${row.agentRef}`,
                    label: row.statusLabel,
                    tone: agentBadgeTone(row.tone),
                  }),
                  Button({
                    key: `khala-agent-select-${row.agentRef}`,
                    label: row.label,
                    variant: selected ? "secondary" : "ghost",
                    onPress: IntentRef(AgentRowSelected, StaticPayload({ agentRef: row.agentRef })),
                    a11y: { label: agentRowAccessibilityLabel(row, selected) },
                    style: { flex: 1, ...mobileInteractiveStyle(accessibility) },
                  }),
                ],
              ),
              ...(selected ? [agentInspectorView(row)] : []),
            ]
          })
        : []),
      ...(state.agentGraphExpanded && graph.hiddenCount > 0
        ? [Text({
            key: "khala-agent-stack-overflow",
            content: `${graph.hiddenCount} more agents hidden by the mobile safety bound`,
            variant: "caption",
            color: "textMuted",
          })]
        : []),
    ],
  )]
}

const fullAutoRunLifecycleBadgeTone = (label: string): "neutral" | "info" | "success" | "warn" | "danger" => {
  switch (label) {
    case "Running": return "info"
    case "Draft":
    case "Paused":
    case "Stopped": return "neutral"
    case "Pausing":
    case "Retrying": return "info"
    case "Stalled":
    case "Cap reached": return "warn"
    case "Completed": return "success"
    case "Failed": return "danger"
    default: return "neutral"
  }
}

/** MOB-FA-02 (#8994): a short, human label per control action for the
 * button row and the "Pausing…" in-flight label. */
const fullAutoRunControlActionLabel: Readonly<Record<FullAutoRunControlAction, string>> = {
  pause: "Pause",
  resume: "Resume",
  stop: "Stop",
}
const fullAutoRunControlActionPendingLabel: Readonly<Record<FullAutoRunControlAction, string>> = {
  pause: "Pausing…",
  resume: "Resuming…",
  stop: "Stopping…",
}

/**
 * The live Full Auto run state header (openagents #8982, extended
 * MOB-FA-02 #8994): a `Badge` + `Text` block above the transcript, plus a
 * lane/rotation/cap footer, Pause/Resume/Stop buttons, an honest
 * pending/outcome status line, and a bounded run-report summary once the
 * run is terminal. Reuses existing Effect Native primitives per the owner's
 * explicit "even if not all components are ported over yet" instruction —
 * no new primitive was invented for this.
 */
const fullAutoRunHeaderViews = (
  header: FullAutoRunHeaderView | null,
): ReadonlyArray<View> => {
  if (header === null) return []
  const capLabel = `${header.successfulAttempts} / ${header.turnCap} continuations`
  const laneLabel = header.laneRef === null
    ? null
    : header.accountRef === null
      ? header.laneRef
      : `${header.laneRef} (${header.accountRef})`
  const footerParts = [
    ...(laneLabel === null ? [] : [laneLabel]),
    capLabel,
    ...(header.failedAttempts > 0 ? [`${header.failedAttempts} failed`] : []),
    ...(header.rotationCount > 0 ? [`${header.rotationCount} rotation${header.rotationCount === 1 ? "" : "s"}`] : []),
  ]
  return [Stack(
    {
      key: "khala-full-auto-run-header",
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
      a11y: {
        role: "region",
        label: `Full Auto run, ${header.lifecycleLabel}. ${header.objective}`,
      },
    },
    [
      Stack(
        {
          key: "khala-full-auto-run-header-row",
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Badge({
            key: "khala-full-auto-run-lifecycle",
            label: header.lifecycleLabel,
            tone: fullAutoRunLifecycleBadgeTone(header.lifecycleLabel),
          }),
          ...(header.workspaceLabel === ""
            ? []
            : [Text({
                key: "khala-full-auto-run-workspace",
                content: header.workspaceLabel,
                variant: "caption",
                color: "textMuted",
              })]),
        ],
      ),
      Text({
        key: "khala-full-auto-run-objective",
        content: header.objective,
        variant: "body",
        color: "textPrimary",
      }),
      ...(footerParts.length === 0 ? [] : [Text({
        key: "khala-full-auto-run-footer",
        content: footerParts.join(" · "),
        variant: "caption",
        color: "textMuted",
      })]),
      ...fullAutoRunControlViews(header),
      ...fullAutoRunReceiptViews(header),
    ],
  )]
}

/** MOB-FA-02 (#8994): the Pause/Resume/Stop button row plus an honest
 * pending/outcome status line. Renders nothing when control is unavailable
 * on this build (`availableActions` empty and nothing pending/reported). */
const fullAutoRunControlViews = (header: FullAutoRunHeaderView): ReadonlyArray<View> => {
  const { control } = header
  if (control.availableActions.length === 0 && control.pendingAction === null && control.lastOutcomeLabel === null) {
    return []
  }
  return [
    ...(control.availableActions.length === 0 ? [] : [Stack(
      {
        key: "khala-full-auto-run-control-row",
        direction: "row",
        gap: "2",
        align: "center",
        style: { width: "full" },
      },
      control.availableActions.map(action => Button({
        key: `khala-full-auto-run-control-${action}`,
        label: control.pendingAction === action
          ? fullAutoRunControlActionPendingLabel[action]
          : fullAutoRunControlActionLabel[action],
        variant: action === "resume" ? "primary" : action === "stop" ? "ghost" : "secondary",
        disabled: control.pendingAction !== null,
        onPress: IntentRef("FullAutoRunControlRequested", StaticPayload({
          runRef: header.runRef,
          action,
        })),
      })),
    )]),
    ...(control.lastOutcomeLabel === null ? [] : [Text({
      key: "khala-full-auto-run-control-outcome",
      content: control.lastOutcomeLabel,
      variant: "caption",
      color: "textMuted",
    })]),
  ]
}

/** MOB-FA-02 (#8994): the bounded run-report summary, shown once the run
 * reaches a terminal lifecycle state. Only the already-redacted public-safe
 * receipt fields -- never raw report internals. */
const fullAutoRunReceiptViews = (header: FullAutoRunHeaderView): ReadonlyArray<View> => {
  if (header.receipt === null) return []
  const parts = [
    `${header.receipt.successfulAttempts} succeeded`,
    ...(header.receipt.failedAttempts > 0 ? [`${header.receipt.failedAttempts} failed`] : []),
    ...(header.receipt.providerIdentities.length > 0 ? [header.receipt.providerIdentities.join(", ")] : []),
    ...(header.receipt.livenessGapCount > 0 ? [`${header.receipt.livenessGapCount} liveness gaps`] : []),
  ]
  return [Stack(
    {
      key: "khala-full-auto-run-receipt",
      direction: "column",
      gap: "0",
      style: { width: "full" },
      a11y: { role: "region", label: `Run report: ${parts.join(", ")}` },
    },
    [
      Text({
        key: "khala-full-auto-run-receipt-title",
        content: "Report",
        variant: "caption",
        color: "textMuted",
      }),
      Text({
        key: "khala-full-auto-run-receipt-summary",
        content: parts.join(" · "),
        variant: "caption",
        color: "textMuted",
      }),
    ],
  )]
}

const codingComposerContextViews = (
  session: MobileCodingComposerSession | null,
  attachmentStatus: Readonly<{
    kind: "ready" | "failed"
    message: string
  }> | null = null,
  attachmentPicking = false,
  accessibility: MobileAccessibilityProfile = defaultMobileAccessibilityProfile,
  executionTargets: ReadonlyArray<MobileExecutionTargetOption> = [],
  toolbarState: MobileComposerToolbarState = { pickerOpen: false, search: "" },
): ReadonlyArray<View> => {
  if (session === null) return []
  return renderMobileComposerToolbar(
    session,
    executionTargets,
    toolbarState,
    accessibility,
    attachmentPicking,
    attachmentStatus,
  )
}

const boundedText = (value: string): string =>
  value.length > 4_000 ? `${value.slice(0, 4_000)}…` : value

const boundedEntries = (entries: ReadonlyArray<KhalaEntry>): ReadonlyArray<KhalaEntry> =>
  entries.length > 200 ? entries.slice(-200) : entries

const localTranscriptTime = (createdAt: string): string => {
  const date = new Date(createdAt)
  if (!Number.isFinite(date.getTime())) return createdAt.slice(11, 16)
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
}

const updateKhala = <State extends { readonly khala: KhalaState }>(
  state: SubscriptionRef.SubscriptionRef<State>,
  update: (khala: KhalaState) => KhalaState,
) =>
  SubscriptionRef.update(state, (current) => ({ ...current, khala: update(current.khala) }))

export const khalaHandlers = <State extends { readonly khala: KhalaState }>(
  state: SubscriptionRef.SubscriptionRef<State>,
  client: KhalaTurnClient | undefined,
) => ({
  [KhalaDraftChanged]: (text: string) =>
    updateKhala(state, (khala) => ({ ...khala, draft: boundedText(text) })),
  [KhalaTurnSubmitted]: (raw: string) =>
    Effect.gen(function* () {
      const message = raw.trim()
      if (message === "") return
      const before = yield* SubscriptionRef.get(state)
      if (before.khala.pending) return
      const turn = before.khala.turnCounter + 1
      const userKey = `khala-${turn}-user`
      const replyKey = `khala-${turn}-reply`
      const history = before.khala.entries
        .filter((entry): entry is KhalaEntry & { readonly role: "user" | "assistant" } =>
          entry.status === "done" && entry.role !== "system")
        .map((entry) => ({ role: entry.role, content: entry.text }))
      yield* updateKhala(state, (khala) => ({
        ...khala,
        draft: "",
        pending: true,
        turnCounter: turn,
        entries: boundedEntries([
          ...khala.entries,
          { key: userKey, role: "user", text: boundedText(message), status: "done" },
          { key: replyKey, role: "assistant", text: "", status: "thinking" },
        ]),
      }))
      const result =
        client === undefined
          ? null
          : yield* Effect.tryPromise({
              try: () => client.sendTurn({ messages: [...history, { role: "user", content: message }] }),
              catch: () => new Error("khala_turn_failed"),
            }).pipe(Effect.catch(() => Effect.succeed(null)))
      yield* updateKhala(state, (khala) => ({
        ...khala,
        pending: false,
        entries: khala.entries.map((entry) =>
          entry.key !== replyKey
            ? entry
            : result === null
              ? { ...entry, text: KHALA_TURN_FAILED_TEXT, status: "failed" as const }
              : { ...entry, text: boundedText(result.reply), status: "done" as const },
        ),
      }))
    }),
})

export const renderKhalaSurface = (
  state: KhalaState,
  authority: "local" | "sync" = "local",
  codingComposer: MobileCodingComposerSession | null = null,
  codingAttachmentPicking = false,
  codingAttachmentStatus: Readonly<{
    kind: "ready" | "failed"
    message: string
  }> | null = null,
  codingAttachmentMutatingRef: string | null = null,
  accessibility: MobileAccessibilityProfile = defaultMobileAccessibilityProfile,
  executionTargets: ReadonlyArray<MobileExecutionTargetOption> = [],
  composerToolbarState: MobileComposerToolbarState = { pickerOpen: false, search: "" },
  composerPathDiscovery: MobileComposerPathDiscoveryState = { state: "idle" },
  historyAvailability: "live" | "refreshing" | "unavailable" = "live",
  fullAutoRun: FullAutoRunHeaderView | null = null,
  runtimeDetails: "visible" | "hidden" | Readonly<{
    mode: "compact"
    assistantLabel: string
  }> = "visible",
): View => {
  const compactRuntime = typeof runtimeDetails === "object"
    ? runtimeDetails
    : null
  const presentedEntries = runtimeDetails !== "visible"
    ? state.entries
        .filter(entry => entry.work === undefined)
        .map(entry => entry.role === "assistant"
          ? { ...entry, text: sanitizeOwnerConversationResponse(entry.text) }
          : entry)
    : state.entries
  const visibleEntries = visibleMobileTranscriptEntries(
    presentedEntries,
    state.transcriptVisibleCount,
  )
  const requestedScrollTargetIsVisible = state.transcriptScrollToKey !== null &&
    visibleEntries.some(entry => entry.key === state.transcriptScrollToKey)
  const transcriptScrollToKey = requestedScrollTargetIsVisible
    ? state.transcriptScrollToKey
    : runtimeDetails !== "visible" && state.transcriptPinned
      ? visibleEntries[visibleEntries.length - 1]?.key ?? null
      : state.transcriptScrollToKey
  const unreadBoundaryIndex = mobileTranscriptUnreadBoundaryIndex(
    visibleEntries.length,
    state.transcriptUnreadCount,
  )
  const messages = visibleEntries.flatMap((entry, index): ReadonlyArray<TranscriptMessage> => [
    ...(unreadBoundaryIndex === index
      ? [{
          key: "khala-transcript-unread-boundary",
          role: "system" as const,
          status: "done" as const,
          body: [Text({
            key: "khala-transcript-unread-boundary-label",
            content: `${state.transcriptUnreadCount} ${state.transcriptUnreadCount === 1 ? "unread update" : "unread updates"}`,
            variant: "caption",
            color: "accent",
            style: { width: "full", textAlign: "center" },
          })],
        }]
      : []),
    {
      key: entry.key,
      role: entry.role,
      status: entry.status === "thinking" || entry.status === "pending" ? "thinking" : "done",
      ...(entry.role === "system" ? { senderLabel: "SYSTEM" } : {}),
      ...(entry.role !== "assistant" && entry.createdAt !== undefined
        ? { timestamp: localTranscriptTime(entry.createdAt) }
        : {}),
      body: interactionBody(state, entry, accessibility, compactRuntime !== null),
    },
  ])
  const hiddenRetainedCount = Math.max(0, presentedEntries.length - visibleEntries.length)
  const unavailableEarlierCount = state.threadHistory === null
    ? 0
    : Math.max(0, state.threadHistory.totalMessageCount - state.threadHistory.retainedMessageCount)
  const viewingAttachment = state.viewingAttachmentRef === null
    ? null
    : state.entries.flatMap(entry => (entry.attachments ?? []).map((attachment, index) => ({
        attachment,
        attachmentRef: mobileAttachmentRef(entry.key, index),
      }))).find(candidate => candidate.attachmentRef === state.viewingAttachmentRef) ?? null
  const composerAutocomplete = renderMobileSlashCommandAutocomplete(state.draft, {
    composerAvailable: codingComposer !== null,
    targetCatalogAvailable: executionTargets.length > 0,
    attachmentPickerAvailable: codingComposer !== null && !codingAttachmentPicking,
    activeTurnRef: state.runtimeTurn?.runRef ?? null,
    activeTurnCancelable: state.runtimeTurn?.status === "queued" ||
      state.runtimeTurn?.status === "running" || state.runtimeTurn?.status === "waiting_for_input",
    pendingAction: codingAttachmentPicking || codingAttachmentMutatingRef !== null ||
      state.runtimeControlSubmittingAction !== null,
  }) ?? renderMobilePathAutocomplete(state.draft, composerPathDiscovery)
  const runAdmission = projectMobileComposerRunAdmission({
    turn: state.runtimeTurn,
    controlAvailable: state.runtimeControlActionsAvailable,
    submittingAction: state.runtimeControlSubmittingAction,
    stopConfirmationRunRef: state.runtimeStopConfirmationRunRef,
    queueReceipt: state.runtimeQueueReceipt,
  })
  return Stack(
    {
      key: "khala-surface",
      direction: "column",
      gap: "3",
      padding: "4",
      a11y: {
        role: "region",
        label: `Conversation surface, ${accessibility.textScale} text scale, reduced motion ${accessibility.reduceMotion ? "on" : "off"}`,
      },
      style: {
        width: "full",
        height: "full",
        // Owner-private chat is conversational, so keep its first message
        // close to the compact row instead of reserving dashboard-like space.
        ...(runtimeDetails !== "visible" ? { paddingTop: "2" as const } : {}),
      },
    },
    [
      ...fullAutoRunHeaderViews(fullAutoRun),
      ...(authority === "sync" && historyAvailability === "unavailable"
        ? [Text({
            key: "khala-history-unavailable",
            content: "Confirmed history is unavailable until sync resumes.",
            variant: "caption",
            color: "warning",
          })]
        : []),
      ...(authority === "sync" && state.threadHistory !== null &&
          (historyAvailability === "refreshing" ||
            state.threadHistory.retainedMessageCount < state.threadHistory.totalMessageCount)
        ? [Text({
            key: "khala-history-accounting",
            content: `${historyAvailability === "refreshing" ? "Refreshing · " : ""}${state.threadHistory.retainedMessageCount} of ${state.threadHistory.totalMessageCount} messages`,
            variant: "caption",
            color: "textMuted",
          })]
        : []),
      ...agentStackViews(state, accessibility),
      ...(authority === "sync" && state.threadHistory !== null && state.entries.length === 0
        ? [Text({
            key: "khala-empty-history",
            content: runtimeDetails !== "visible"
              ? "Start the conversation below."
              : "No confirmed messages yet. Start this chat below.",
            variant: "body",
            color: "textMuted",
            style: { width: "full", textAlign: "center" },
          })]
        : []),
      ...(hiddenRetainedCount > 0
        ? [Button({
            key: "khala-load-earlier-history",
            label: `Load ${Math.min(MOBILE_TRANSCRIPT_PAGE_SIZE, hiddenRetainedCount)} earlier`,
            variant: "ghost",
            onPress: IntentRef(TranscriptEarlierHistoryRequested, StaticPayload({})),
            a11y: { label: `Load earlier confirmed transcript entries. ${hiddenRetainedCount} retained entries remain.` },
            style: { width: "full", minHeight: accessibility.minTouchTarget },
          })]
        : []),
      ...(unavailableEarlierCount > 0
        ? [Text({
            key: "khala-unavailable-earlier-history",
            content: `${unavailableEarlierCount} earlier ${unavailableEarlierCount === 1 ? "message is" : "messages are"} not retained on this device.`,
            variant: "caption",
            color: "textMuted",
          })]
        : []),
      Transcript({
        key: "khala-transcript",
        messages,
        a11y: {
          role: "list",
          label: `Conversation transcript, reduced motion ${accessibility.reduceMotion ? "on" : "off"}`,
        },
        pinToEnd: state.transcriptPinned,
        onPinnedChange: IntentRef(TranscriptPinnedChanged, ComponentValueBinding()),
        preserveScrollAnchor: true,
        ...(transcriptScrollToKey === null ? {} : { scrollToKey: transcriptScrollToKey }),
        virtualize: true,
        estimatedItemSize: 180,
        style: { width: "full", flex: 1 },
      }),
      ...(state.transcriptUnreadCount > 0
        ? [Button({
            key: "khala-jump-to-latest",
            label: `Jump to latest · ${state.transcriptUnreadCount} unread`,
            variant: "secondary",
            onPress: IntentRef(TranscriptJumpToLatestRequested, StaticPayload({})),
            style: { width: "full", minHeight: accessibility.minTouchTarget },
          })]
        : []),
      ...(compactRuntime === null
        ? []
        : compactRuntimeStatusViews(state, compactRuntime.assistantLabel)),
      ...(runtimeDetails !== "visible" ? [] : runtimeControlViews(state, accessibility)),
      ...(runtimeDetails !== "visible"
        ? []
        : renderMobileComposerRunControl(state.runtimeTurn, runAdmission, accessibility)),
      ...codingComposerContextViews(
        codingComposer,
        codingAttachmentStatus,
        codingAttachmentPicking,
        accessibility,
        executionTargets,
        composerToolbarState,
      ),
      ...(codingComposer === null
        ? []
        : renderMobileComposerAttachments(
            codingComposer.draft.doc.attachments,
            codingAttachmentMutatingRef,
            accessibility,
          )),
      Composer({
        key: "khala-composer",
        doc: state.draft === "" ? [] : [{ kind: "text", text: state.draft }],
        mode: "normal",
        placeholder: authority === "sync" ? runAdmission.placeholder : "Message Khala",
        ...(compactRuntime === null ? {} : { autoCorrect: false }),
        ...(composerAutocomplete === undefined ? {} : { autocomplete: composerAutocomplete }),
        ...(codingComposer === null ? {} : {
          attachments: codingComposer.draft.doc.attachments.map(attachment => ({
            id: attachment.id,
            name: attachment.name,
            mimeType: attachment.mime,
            size: attachment.sizeBytes,
          })),
        }),
        submitting: state.pending,
        ...(runAdmission.active ? { submitLabel: runAdmission.submitLabel } : {}),
        ...((runAdmission.stopAvailable || runAdmission.stopping) &&
            !runAdmission.confirming && state.runtimeTurn !== null
          ? {
              onStop: IntentRef("RuntimeTurnStopConfirmationRequested", StaticPayload({
                runRef: state.runtimeTurn.runRef,
              })),
              stopping: runAdmission.stopping,
            }
          : {}),
        clearOnSubmit: true,
        a11y: { label: "Coding message" },
        onChange: IntentRef(KhalaDraftChanged, ComponentValueBinding()),
        ...(codingComposer === null ? {} : {
          onAttachmentRequest: IntentRef(
            "CodingComposerAttachmentsRequested",
            StaticPayload({}),
          ),
        }),
        ...(codingComposer !== null && codingComposer.draft.target.readiness !== "ready"
          ? {}
          : {
              onSubmit: IntentRef(
                KhalaTurnSubmitted,
                ComponentValueBinding(),
              ),
            }),
        style: {
          width: "full",
          minHeight: Math.max(54, accessibility.minTouchTarget),
          surface: "glass",
        },
      }),
      ...(viewingAttachment === null
        ? []
        : [renderMobileAttachmentViewer(
            viewingAttachment.attachmentRef,
            viewingAttachment.attachment,
          )]),
    ],
  )
}

export const khalaIntentDefinitions = [
  { name: KhalaDraftChanged, payload: Schema.String },
  { name: KhalaTurnSubmitted, payload: Schema.String },
] as const
