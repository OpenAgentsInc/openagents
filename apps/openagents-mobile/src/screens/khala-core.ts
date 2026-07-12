import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import {
  Badge,
  Composer,
  Button,
  ComponentValueBinding,
  IconButton,
  IntentRef,
  Stack,
  Spacer,
  StaticPayload,
  Text,
  Transcript,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"
import type { MobileRuntimeControlAction } from "../conversation/mobile-conversation"
import type { MobileCodingComposerSession } from "../coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import {
  fleetRunActions,
  type LiveAgentGraphPresentation,
  type LiveAgentGraphPresentationRow,
  type LiveAgentGraphTone,
} from "@openagentsinc/khala-sync-client"

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
export type KhalaRole = "user" | "assistant" | "system"

export interface KhalaEntry {
  readonly key: string
  readonly role: KhalaRole
  readonly text: string
  readonly status: "thinking" | "pending" | "done" | "failed"
  readonly createdAt?: string
  readonly version?: number
  readonly interaction?: KhalaInteraction
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

export interface KhalaState {
  readonly draft: string
  readonly entries: ReadonlyArray<KhalaEntry>
  readonly pending: boolean
  readonly turnCounter: number
  readonly interactionSelections: Readonly<Record<string, Readonly<Record<string, ReadonlyArray<string>>>>>
  readonly interactionSubmittingRef: string | null
  readonly interactionActionsAvailable: boolean
  readonly runtimeTurn: KhalaRuntimeTurn | null
  readonly runtimeControlSubmittingAction: MobileRuntimeControlAction | null
  readonly runtimeControlActionsAvailable: boolean
  /** Confirmed canonical live-agent hierarchy for the active thread, or null. */
  readonly agentGraph: LiveAgentGraphPresentation | null
  readonly agentGraphExpanded: boolean
  readonly selectedAgentRef: string | null
}

export const initialKhalaState: KhalaState = {
  draft: "",
  entries: [],
  pending: false,
  turnCounter: 0,
  interactionSelections: {},
  interactionSubmittingRef: null,
  interactionActionsAvailable: false,
  runtimeTurn: null,
  runtimeControlSubmittingAction: null,
  runtimeControlActionsAvailable: false,
  agentGraph: null,
  agentGraphExpanded: false,
  selectedAgentRef: null,
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

/** Mobile renders at most this many hierarchy rows and names the remainder. */
export const MOBILE_AGENT_GRAPH_MAX_ROWS = 40
export const KHALA_TURN_FAILED_TEXT =
  "Khala could not respond just now. Check your connection and send that again."

const interactionStatusLabel = (status: KhalaInteractionStatus): string => {
  switch (status) {
    case "pending": return "Needs your response"
    case "resolved": return "Resolved"
    case "expired": return "Expired"
    case "revoked": return "Access revoked"
  }
}

const interactionBody = (
  state: KhalaState,
  entry: KhalaEntry,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const interaction = entry.interaction
  if (interaction === undefined) {
    return [Text({
      key: `${entry.key}-text`,
      content: entry.status === "thinking" ? "Khala is thinking…" : entry.text,
      variant: "body",
      color: entry.status === "failed" ? "danger" : "textPrimary",
    })]
  }
  const submitting = state.interactionSubmittingRef === interaction.interactionRef
  const actionable = interaction.status === "pending" &&
    state.interactionActionsAvailable && !submitting
  const selections = state.interactionSelections[interaction.interactionRef] ?? {}
  const questionViews = interaction.kind !== "provider_question" ? [] : interaction.questions.flatMap(question => [
    Text({
      key: `${entry.key}-${question.questionRef}-prompt`,
      content: question.displayText,
      variant: "body",
      color: "textPrimary",
    }),
    ...question.options.flatMap(option => [
      Button({
        key: `${entry.key}-${question.questionRef}-${option.optionRef}`,
        label: option.label,
        variant: selections[question.questionRef]?.includes(option.optionRef)
          ? "secondary"
          : "ghost",
        disabled: !actionable,
        onPress: IntentRef("RuntimeInteractionOptionToggled", StaticPayload({
          interactionRef: interaction.interactionRef,
          questionRef: question.questionRef,
          optionRef: option.optionRef,
          multiSelect: question.multiSelect,
        })),
        style: { width: "full", ...mobileInteractiveStyle(accessibility) },
      }),
      ...(option.description === undefined ? [] : [Text({
        key: `${entry.key}-${question.questionRef}-${option.optionRef}-description`,
        content: option.description,
        variant: "caption",
        color: "textMuted",
      })]),
    ]),
  ])
  const everyQuestionAnswered = interaction.questions.every(question =>
    (selections[question.questionRef]?.length ?? 0) > 0)
  const actionViews = interaction.status !== "pending" ? []
    : interaction.kind === "provider_question"
      ? [Button({
          key: `${entry.key}-submit-answers`,
          label: submitting ? "Submitting…" : "Submit answers",
          variant: "primary",
          disabled: !actionable || !everyQuestionAnswered,
          onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
            interactionRef: interaction.interactionRef,
            turnRef: interaction.turnRef,
            kind: interaction.kind,
          })),
          style: mobileInteractiveStyle(accessibility),
        })]
      : interaction.kind === "tool_approval"
        ? [
            Button({
              key: `${entry.key}-approve`, label: submitting ? "Submitting…" : "Approve",
              variant: "primary", disabled: !actionable,
              onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "approve" })),
              style: mobileInteractiveStyle(accessibility),
            }),
            Button({
              key: `${entry.key}-deny`, label: "Deny", variant: "secondary", disabled: !actionable,
              onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "deny" })),
              style: mobileInteractiveStyle(accessibility),
            }),
          ]
        : [
            Button({ key: `${entry.key}-accept`, label: submitting ? "Submitting…" : "Accept plan", variant: "primary", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "accept" })), style: mobileInteractiveStyle(accessibility) }),
            Button({ key: `${entry.key}-changes`, label: "Request changes", variant: "secondary", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "request_changes" })), style: mobileInteractiveStyle(accessibility) }),
            Button({ key: `${entry.key}-replan`, label: "Replan", variant: "ghost", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "replan" })), style: mobileInteractiveStyle(accessibility) }),
          ]
  return [
    Text({ key: `${entry.key}-title`, content: interaction.title, variant: "heading", color: "textPrimary" }),
    Text({ key: `${entry.key}-status`, content: interactionStatusLabel(interaction.status), variant: "caption", color: interaction.status === "expired" || interaction.status === "revoked" ? "warning" : "textMuted" }),
    Text({ key: `${entry.key}-prompt`, content: interaction.prompt, variant: "body", color: "textPrimary" }),
    ...questionViews,
    ...actionViews,
  ]
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

const codingComposerContextViews = (
  session: MobileCodingComposerSession | null,
  attachmentStatus: Readonly<{
    kind: "ready" | "failed"
    message: string
  }> | null = null,
  attachmentPicking = false,
  accessibility: MobileAccessibilityProfile = defaultMobileAccessibilityProfile,
  executionTargets: ReadonlyArray<MobileExecutionTargetOption> = [],
): ReadonlyArray<View> => {
  if (session === null) return []
  const target = session.draft.target
  return [Stack(
    {
      key: "khala-coding-composer-context",
      direction: "column",
      gap: "1",
      style: { width: "full" },
    },
    [
      Text({
        key: "khala-coding-composer-location",
        content: `${session.repositoryLabel} · ${session.worktreeLabel}`,
        variant: "caption",
        color: "textPrimary",
      }),
      Text({
        key: "khala-coding-composer-target",
        content: [
          session.targetLabel,
          target.providerRef ?? "Provider not selected",
          target.modelRef ?? "Model not selected",
          target.accountRef ?? "Account not selected",
        ].join(" · "),
        variant: "caption",
        color: target.readiness === "ready" ? "textMuted" : "warning",
      }),
      ...(executionTargets.length === 0
        ? [Text({
            key: "khala-coding-target-catalog-unavailable",
            content: "Execution targets unavailable. Your draft is preserved.",
            variant: "caption",
            color: "warning",
          })]
        : [Stack(
            {
              key: "khala-coding-target-options",
              direction: "row",
              gap: "1",
              style: { width: "full" },
              a11y: { role: "group", label: "Execution target" },
            },
            executionTargets.map(option => {
              const selected = target.executionTargetRef === option.targetId
              return Button({
                key: `khala-coding-target-${option.targetId}`,
                label: option.label,
                variant: selected ? "secondary" : "ghost",
                disabled: option.readiness !== "ready",
                onPress: IntentRef(
                  "CodingExecutionTargetSelected",
                  StaticPayload({ targetId: option.targetId }),
                ),
                a11y: {
                  label: `${option.accessibilityLabel}${selected ? ", selected" : ""}`,
                },
                style: { flex: 1, ...mobileInteractiveStyle(accessibility) },
              })
            }),
          )]),
      ...(attachmentPicking
        ? [Text({
            key: "khala-coding-composer-attachment-picking",
            content: "Choosing files or images…",
            variant: "caption",
            color: "textMuted",
          })]
        : attachmentStatus === null
          ? []
          : [Text({
              key: "khala-coding-composer-attachment-status",
              content: attachmentStatus.message,
              variant: "caption",
              color: attachmentStatus.kind === "failed" ? "danger" : "textMuted",
            })]),
    ],
  )]
}

const boundedText = (value: string): string =>
  value.length > 4_000 ? `${value.slice(0, 4_000)}…` : value

const boundedEntries = (entries: ReadonlyArray<KhalaEntry>): ReadonlyArray<KhalaEntry> =>
  entries.length > 200 ? entries.slice(-200) : entries

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
  accessibility: MobileAccessibilityProfile = defaultMobileAccessibilityProfile,
  executionTargets: ReadonlyArray<MobileExecutionTargetOption> = [],
): View =>
  Stack(
    {
      key: "khala-surface",
      direction: "column",
      gap: "3",
      padding: "4",
      a11y: {
        role: "region",
        label: `Conversation surface, ${accessibility.textScale} text scale, reduced motion ${accessibility.reduceMotion ? "on" : "off"}`,
      },
      style: { width: "full", height: "full" },
    },
    [
      Spacer({ key: "khala-top-space", size: "16" }),
      Text({
        key: "khala-title",
        content: authority === "sync" ? "OpenAgents" : "Khala",
        variant: "title",
        color: "textPrimary",
      }),
      Text({
        key: "khala-subtitle",
        content: authority === "sync"
          ? "Confirmed conversation, continuous across your devices."
          : "One conversation, routed by the OpenAgents orchestrator.",
        variant: "body",
        color: "textMuted",
      }),
      ...agentStackViews(state, accessibility),
      Transcript({
        key: "khala-transcript",
        messages: state.entries.map((entry): TranscriptMessage => ({
          key: entry.key,
          role: entry.role,
          status: entry.status === "thinking" || entry.status === "pending" ? "thinking" : "done",
          senderLabel: entry.role === "user"
            ? entry.status === "pending" ? "YOU · PENDING" : "YOU"
            : entry.role === "assistant" ? "ASSISTANT" : "SYSTEM",
          ...(entry.createdAt === undefined ? {} : { timestamp: entry.createdAt.slice(11, 16) }),
          body: interactionBody(state, entry, accessibility),
        })),
        a11y: {
          role: "list",
          label: `Conversation transcript, reduced motion ${accessibility.reduceMotion ? "on" : "off"}`,
        },
        pinToEnd: true,
        style: { width: "full", flex: 1 },
      }),
      ...runtimeControlViews(state, accessibility),
      ...codingComposerContextViews(
        codingComposer,
        codingAttachmentStatus,
        codingAttachmentPicking,
        accessibility,
        executionTargets,
      ),
      Stack(
        {
          key: "khala-composer-bar",
          direction: "row",
          gap: "2",
          align: "center",
          padding: "2",
          style: {
            width: "full",
            minHeight: Math.max(54, accessibility.minTouchTarget),
            borderRadius: "full",
            surface: "glass",
          },
        },
        [
          IconButton({
            key: "khala-new-chat",
            icon: "Plus",
            accessibilityLabel: codingComposer === null
              ? "New chat"
              : codingAttachmentPicking
                ? "Choosing files or images"
                : "Add file or image",
            disabled: state.pending || codingAttachmentPicking,
            onPress: IntentRef(
              codingComposer === null
                ? "NewChatPressed"
                : "CodingComposerAttachmentsRequested",
              StaticPayload({}),
            ),
            surface: "glass",
            style: mobileInteractiveStyle(accessibility),
          }),
          Composer({
            key: "khala-composer",
            doc: state.draft === "" ? [] : [{ kind: "text", text: state.draft }],
            mode: "normal",
            placeholder: authority === "sync" ? "Continue conversation" : "Message Khala",
            ...(codingComposer === null ? {} : {
              attachments: codingComposer.draft.doc.attachments.map(attachment => ({
                id: attachment.id,
                name: attachment.name,
                mimeType: attachment.mime,
                size: attachment.sizeBytes,
              })),
            }),
            submitting: state.pending,
            clearOnSubmit: true,
            a11y: { label: "Coding message" },
            onChange: IntentRef(KhalaDraftChanged, ComponentValueBinding()),
            ...(codingComposer !== null &&
                codingComposer.draft.target.readiness !== "ready"
              ? {}
              : {
                  onSubmit: IntentRef(
                    KhalaTurnSubmitted,
                    ComponentValueBinding(),
                  ),
                }),
            style: {
              flex: 1,
              minHeight: accessibility.minTouchTarget,
              borderWidth: 0,
              surface: "glass",
            },
          }),
        ],
      ),
    ],
  )

export const khalaIntentDefinitions = [
  { name: KhalaDraftChanged, payload: Schema.String },
  { name: KhalaTurnSubmitted, payload: Schema.String },
] as const
