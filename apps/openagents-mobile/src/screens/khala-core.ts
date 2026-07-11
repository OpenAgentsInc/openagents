import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import {
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

export interface KhalaState {
  readonly draft: string
  readonly entries: ReadonlyArray<KhalaEntry>
  readonly pending: boolean
  readonly turnCounter: number
  readonly interactionSelections: Readonly<Record<string, Readonly<Record<string, ReadonlyArray<string>>>>>
  readonly interactionSubmittingRef: string | null
  readonly interactionActionsAvailable: boolean
}

export const initialKhalaState: KhalaState = {
  draft: "",
  entries: [],
  pending: false,
  turnCounter: 0,
  interactionSelections: {},
  interactionSubmittingRef: null,
  interactionActionsAvailable: false,
}

export interface KhalaTurnClient {
  readonly sendTurn: (input: {
    readonly messages: ReadonlyArray<{ readonly role: KhalaRole; readonly content: string }>
  }) => Promise<{ readonly reply: string }>
}

export const KhalaDraftChanged = "KhalaDraftChanged"
export const KhalaTurnSubmitted = "KhalaTurnSubmitted"
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

const interactionBody = (state: KhalaState, entry: KhalaEntry): ReadonlyArray<View> => {
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
        style: { width: "full" },
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
        })]
      : interaction.kind === "tool_approval"
        ? [
            Button({
              key: `${entry.key}-approve`, label: submitting ? "Submitting…" : "Approve",
              variant: "primary", disabled: !actionable,
              onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "approve" })),
            }),
            Button({
              key: `${entry.key}-deny`, label: "Deny", variant: "secondary", disabled: !actionable,
              onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "deny" })),
            }),
          ]
        : [
            Button({ key: `${entry.key}-accept`, label: submitting ? "Submitting…" : "Accept plan", variant: "primary", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "accept" })) }),
            Button({ key: `${entry.key}-changes`, label: "Request changes", variant: "secondary", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "request_changes" })) }),
            Button({ key: `${entry.key}-replan`, label: "Replan", variant: "ghost", disabled: !actionable, onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({ interactionRef: interaction.interactionRef, turnRef: interaction.turnRef, kind: interaction.kind, outcome: "replan" })) }),
          ]
  return [
    Text({ key: `${entry.key}-title`, content: interaction.title, variant: "heading", color: "textPrimary" }),
    Text({ key: `${entry.key}-status`, content: interactionStatusLabel(interaction.status), variant: "caption", color: interaction.status === "expired" || interaction.status === "revoked" ? "warning" : "textMuted" }),
    Text({ key: `${entry.key}-prompt`, content: interaction.prompt, variant: "body", color: "textPrimary" }),
    ...questionViews,
    ...actionViews,
  ]
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
): View =>
  Stack(
    {
      key: "khala-surface",
      direction: "column",
      gap: "3",
      padding: "4",
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
          body: interactionBody(state, entry),
        })),
        pinToEnd: true,
        style: { width: "full", flex: 1 },
      }),
      Stack(
        {
          key: "khala-composer-bar",
          direction: "row",
          gap: "2",
          align: "center",
          padding: "2",
          style: {
            width: "full",
            minHeight: 54,
            borderRadius: "full",
            surface: "glass",
          },
        },
        [
          IconButton({
            key: "khala-new-chat",
            icon: "Plus",
            accessibilityLabel: "New chat",
            onPress: IntentRef("NewChatPressed", StaticPayload({})),
            surface: "glass",
          }),
          Composer({
            key: "khala-composer",
            doc: state.draft === "" ? [] : [{ kind: "text", text: state.draft }],
            mode: "normal",
            placeholder: authority === "sync" ? "Continue conversation" : "Message Khala",
            submitting: state.pending,
            clearOnSubmit: true,
            onChange: IntentRef(KhalaDraftChanged, ComponentValueBinding()),
            onSubmit: IntentRef(KhalaTurnSubmitted, ComponentValueBinding()),
            style: {
              flex: 1,
              minHeight: 44,
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
