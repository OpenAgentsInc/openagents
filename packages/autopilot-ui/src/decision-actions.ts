import { classAttrs } from "@openagentsinc/ui/class-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
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

  return h.div(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.stackSmall),
      h.DataAttribute("autopilot-decision-actions", input.decision.requestId),
    ],
    [
      h.div(classAttrs<AutopilotUiMessage>(domainStyles.wrap), [
        ...decisionVerbs.map((verb) =>
          h.button(
            [
              ...classAttrs<AutopilotUiMessage>(domainStyles.actionButton),
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
