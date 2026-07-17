import {
  Badge,
  Button,
  Card,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"

import type {
  KhalaInteraction,
  MobileAccessibilityProfile,
} from "./khala-core"
import { mobileRichContentViews } from "./mobile-transcript-content"

export interface MobileInteractionCardState {
  readonly selections: Readonly<Record<string, ReadonlyArray<string>>>
  readonly submitting: boolean
  readonly actionsAvailable: boolean
}

const interactionKindLabel = (interaction: KhalaInteraction): string => {
  switch (interaction.kind) {
    case "provider_question": return "Input needed"
    case "tool_approval": return "Approval needed"
    case "plan_review": return "Plan review"
  }
}

const interactionStatusLabel = (interaction: KhalaInteraction): string => {
  if (interaction.status === "pending") return interactionKindLabel(interaction)
  switch (interaction.status) {
    case "resolved": return "Decision confirmed"
    case "expired": return "Expired"
    case "revoked": return "Access revoked"
  }
}

const interactionStatusTone = (
  interaction: KhalaInteraction,
): "info" | "success" | "warn" | "danger" => {
  switch (interaction.status) {
    case "pending": return "info"
    case "resolved": return "success"
    case "expired": return "warn"
    case "revoked": return "danger"
  }
}

const terminalSummary = (interaction: KhalaInteraction): string | null => {
  switch (interaction.status) {
    case "pending": return null
    case "resolved": return interaction.decisionRef === undefined
      ? "The runtime confirmed this interaction is resolved."
      : "Your decision was confirmed by the runtime."
    case "expired": return "This request expired before another decision could be accepted."
    case "revoked": return "Authority for this request was revoked. No action is available."
  }
}

const interactionHeader = (
  entryKey: string,
  interaction: KhalaInteraction,
): ReadonlyArray<View> => [
  Stack({
    key: `${entryKey}-interaction-heading`,
    direction: "row",
    gap: "2",
    align: "center",
    style: { width: "full" },
  }, [
    Badge({
      key: `${entryKey}-interaction-status`,
      label: interactionStatusLabel(interaction),
      tone: interactionStatusTone(interaction),
    }),
    Text({
      key: `${entryKey}-interaction-title`,
      content: interaction.title,
      variant: "heading",
      color: "textPrimary",
      style: { flex: 1 },
    }),
  ]),
]

const questionViews = (
  entryKey: string,
  interaction: KhalaInteraction,
  state: MobileInteractionCardState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => interaction.questions.flatMap((question, questionIndex) => {
  const selected = state.selections[question.questionRef] ?? []
  return [Stack({
    key: `${entryKey}-${question.questionRef}`,
    direction: "column",
    gap: "2",
    padding: "2",
    style: { width: "full", backgroundColor: "surface", borderRadius: "md" },
    a11y: {
      role: "group",
      label: `Question ${questionIndex + 1} of ${interaction.questions.length}. ${question.multiSelect ? "Select one or more answers" : "Select one answer"}`,
    },
  }, [
    Text({
      key: `${entryKey}-${question.questionRef}-mode`,
      content: `Question ${questionIndex + 1} of ${interaction.questions.length} · ${question.multiSelect ? "Select one or more" : "Select one"}`,
      variant: "caption",
      color: "textMuted",
    }),
    Text({
      key: `${entryKey}-${question.questionRef}-prompt`,
      content: question.displayText,
      variant: "body",
      color: "textPrimary",
      weight: "medium",
    }),
    ...question.options.flatMap(option => [
      Button({
        key: `${entryKey}-${question.questionRef}-${option.optionRef}`,
        label: option.label,
        variant: selected.includes(option.optionRef) ? "secondary" : "ghost",
        selected: selected.includes(option.optionRef),
        pill: true,
        size: "sm",
        disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
        onPress: IntentRef("RuntimeInteractionOptionToggled", StaticPayload({
          interactionRef: interaction.interactionRef,
          questionRef: question.questionRef,
          optionRef: option.optionRef,
          multiSelect: question.multiSelect,
        })),
        a11y: {
          label: `${option.label}${option.description === undefined ? "" : `. ${option.description}`}`,
          selected: selected.includes(option.optionRef),
        },
        style: { width: "full", minHeight: accessibility.minTouchTarget },
      }),
      ...(option.description === undefined ? [] : [Text({
        key: `${entryKey}-${question.questionRef}-${option.optionRef}-description`,
        content: option.description,
        variant: "caption",
        color: "textMuted",
      })]),
    ]),
  ])]
})

const questionActions = (
  entryKey: string,
  interaction: KhalaInteraction,
  state: MobileInteractionCardState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const everyQuestionAnswered = interaction.questions.length > 0 &&
    interaction.questions.every(question => (state.selections[question.questionRef]?.length ?? 0) > 0)
  return [Button({
    key: `${entryKey}-submit-answers`,
    label: state.submitting ? "Submitting answers…" : "Submit answers",
    variant: "primary",
    block: true,
    loading: state.submitting,
    disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting || !everyQuestionAnswered,
    onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
      interactionRef: interaction.interactionRef,
      turnRef: interaction.turnRef,
      kind: "provider_question",
    })),
    a11y: {
      label: everyQuestionAnswered ? "Submit selected answers" : "Answer every question before submitting",
    },
    style: { width: "full", minHeight: accessibility.minTouchTarget },
  })]
}

const approvalActions = (
  entryKey: string,
  interaction: KhalaInteraction,
  state: MobileInteractionCardState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => [
  Button({
    key: `${entryKey}-approve`,
    label: state.submitting ? "Submitting…" : "Allow once",
    variant: "primary",
    loading: state.submitting,
    disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
    onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
      interactionRef: interaction.interactionRef,
      turnRef: interaction.turnRef,
      kind: "tool_approval",
      outcome: "approve",
    })),
    a11y: { label: "Allow this requested operation once" },
    style: { flex: 1, minHeight: accessibility.minTouchTarget },
  }),
  Button({
    key: `${entryKey}-deny`,
    label: "Deny",
    tone: "danger",
    variant: "soft",
    disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
    onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
      interactionRef: interaction.interactionRef,
      turnRef: interaction.turnRef,
      kind: "tool_approval",
      outcome: "deny",
    })),
    a11y: { label: "Deny this requested operation" },
    style: { flex: 1, minHeight: accessibility.minTouchTarget },
  }),
]

const planActions = (
  entryKey: string,
  interaction: KhalaInteraction,
  state: MobileInteractionCardState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => [
  Button({
    key: `${entryKey}-accept`,
    label: state.submitting ? "Submitting…" : "Accept plan",
    variant: "primary",
    block: true,
    loading: state.submitting,
    disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
    onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
      interactionRef: interaction.interactionRef,
      turnRef: interaction.turnRef,
      kind: "plan_review",
      outcome: "accept",
    })),
    style: { width: "full", minHeight: accessibility.minTouchTarget },
  }),
  Stack({
    key: `${entryKey}-plan-secondary-actions`,
    direction: "row",
    gap: "2",
    style: { width: "full" },
  }, [
    Button({
      key: `${entryKey}-changes`,
      label: "Request changes",
      variant: "secondary",
      disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
      onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
        interactionRef: interaction.interactionRef,
        turnRef: interaction.turnRef,
        kind: "plan_review",
        outcome: "request_changes",
      })),
      style: { flex: 1, minHeight: accessibility.minTouchTarget },
    }),
    Button({
      key: `${entryKey}-replan`,
      label: "Replan",
      variant: "ghost",
      disabled: interaction.status !== "pending" || !state.actionsAvailable || state.submitting,
      onPress: IntentRef("RuntimeInteractionDecisionSubmitted", StaticPayload({
        interactionRef: interaction.interactionRef,
        turnRef: interaction.turnRef,
        kind: "plan_review",
        outcome: "replan",
      })),
      style: { flex: 1, minHeight: accessibility.minTouchTarget },
    }),
  ]),
]

export const renderMobileInteractionCard = (
  entryKey: string,
  interaction: KhalaInteraction,
  state: MobileInteractionCardState,
  accessibility: MobileAccessibilityProfile,
): View => {
  const terminal = terminalSummary(interaction)
  const promptViews = interaction.kind === "plan_review"
    ? mobileRichContentViews(`${entryKey}-plan`, interaction.prompt, "Copy plan")
    : [Text({
        key: `${entryKey}-interaction-prompt`,
        content: interaction.prompt,
        variant: "body",
        color: "textPrimary",
      })]
  const body = interaction.kind === "provider_question"
    ? questionViews(entryKey, interaction, state, accessibility)
    : []
  const actions = interaction.status !== "pending"
    ? []
    : interaction.kind === "provider_question"
      ? questionActions(entryKey, interaction, state, accessibility)
      : interaction.kind === "tool_approval"
        ? approvalActions(entryKey, interaction, state, accessibility)
        : planActions(entryKey, interaction, state, accessibility)

  return Card({
    key: `${entryKey}-interaction-card`,
    padding: "3",
    radius: "lg",
    style: { width: "full", backgroundColor: "surfaceRaised", borderColor: "border", borderWidth: 1 },
    a11y: { role: "region", label: `${interactionKindLabel(interaction)}. ${interaction.title}` },
  }, [
    ...interactionHeader(entryKey, interaction),
    ...promptViews,
    ...(interaction.kind === "tool_approval" && interaction.status === "pending"
      ? [Text({
          key: `${entryKey}-approval-scope`,
          content: "Allow applies to this requested operation once. Review the operation before continuing.",
          variant: "caption",
          color: "textMuted",
        })]
      : []),
    ...body,
    ...(terminal === null ? [] : [Text({
      key: `${entryKey}-interaction-terminal-summary`,
      content: terminal,
      variant: "caption",
      color: interaction.status === "revoked" ? "danger" : interaction.status === "expired" ? "warning" : "textMuted",
    })]),
    ...(actions.length === 0 ? [] : [Stack({
      key: `${entryKey}-interaction-actions`,
      direction: interaction.kind === "tool_approval" ? "row" : "column",
      gap: "2",
      style: { width: "full" },
      a11y: { role: "group", label: `${interactionKindLabel(interaction)} actions` },
    }, actions)]),
  ])
}
