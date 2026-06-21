import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage } from "./view.js"
import { statusChip } from "./view.js"

export type DecisionView = Readonly<{
  requestId: string
  state: "pending" | "resolved" | "cancelled" | "expired"
  resolvedVerb?: string
}>

export type DecisionVerb = "approve" | "deny" | "answer"

export type DecisionActionModel = Readonly<
  Record<
    DecisionVerb,
    Readonly<{
      enabled: boolean
    }>
  >
>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const decisionVerbs = ["approve", "deny", "answer"] as const satisfies ReadonlyArray<DecisionVerb>

const actionLabels = {
  approve: "Approve",
  deny: "Deny",
  answer: "Answer",
} as const satisfies Record<DecisionVerb, string>

export const decisionActionState = (
  decision: DecisionView,
  input: { readOnly: boolean },
): DecisionActionModel => {
  const enabled = decision.state === "pending" && !input.readOnly

  return {
    approve: { enabled },
    deny: { enabled },
    answer: { enabled },
  }
}

export const DecisionActions = (input: { decision: DecisionView; readOnly: boolean }): Html => {
  const actions = decisionActionState(input.decision, { readOnly: input.readOnly })
  const actionClass =
    "inline-flex h-8 items-center rounded-[4px] border border-[var(--outline,#525458)] px-3 font-mono text-xs font-bold text-[var(--primary,#fff)] disabled:opacity-45"

  return h.div(
    [
      className("grid gap-2 text-[var(--text,#d7d8e5)]"),
      h.DataAttribute("autopilot-decision-actions", input.decision.requestId),
    ],
    [
      h.div([className("flex flex-wrap items-center gap-2")], [
        ...decisionVerbs.map((verb) =>
          h.button(
            [
              className(actionClass),
              h.Type("button"),
              h.Disabled(!actions[verb].enabled),
              h.DataAttribute("autopilot-decision-action", verb),
            ],
            [actionLabels[verb]],
          ),
        ),
        ...(input.decision.state === "pending"
          ? []
          : [
              statusChip({
                label: "resolved elsewhere",
                tone: "neutral",
                attrs: [h.DataAttribute("autopilot-decision-resolved-note", input.decision.state)],
              }),
            ]),
      ]),
    ],
  )
}
