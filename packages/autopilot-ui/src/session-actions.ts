import { classAttrs } from "@openagentsinc/ui/class-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type SessionActionState = "queued" | "running" | "completed" | "failed" | "cancelled" | string

export type ActionSessionSummary = Readonly<{
  sessionRef: string
  adapter: string
  state: SessionActionState
}>

const h = html<AutopilotUiMessage>()

const sessionStateTone = (state: SessionActionState): ChipTone => {
  switch (state) {
    case "completed":
      return "success"
    case "running":
      return "info"
    case "queued":
      return "warning"
    case "failed":
    case "cancelled":
      return "danger"
    default:
      return "neutral"
  }
}

const isTerminalSessionState = (state: SessionActionState): boolean =>
  state === "completed" || state === "failed" || state === "cancelled"

export const SessionDetail = (
  session: ActionSessionSummary,
  input: { events?: ReadonlyArray<unknown> } = {},
): Html =>
  h.article(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-session-ref", session.sessionRef),
    ],
    [
      h.div(classAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(classAttrs<AutopilotUiMessage>(domainStyles.stack), [
          h.code(classAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
            session.sessionRef,
          ]),
          h.span(classAttrs<AutopilotUiMessage>(domainStyles.muted), [
            session.adapter,
          ]),
        ]),
        statusChip({
          label: session.state,
          tone: sessionStateTone(session.state),
          attrs: [h.DataAttribute("autopilot-state", session.state)],
        }),
      ]),
      ...(input.events === undefined
        ? []
        : [
            statusChip({
              label: `events: ${input.events.length}`,
              tone: "neutral",
              attrs: [h.DataAttribute("autopilot-event-count", String(input.events.length))],
            }),
          ]),
    ],
  )

export const SessionActions = (input: {
  session: ActionSessionSummary
  readOnly: boolean
}): Html => {
  const terminal = isTerminalSessionState(input.session.state)

  return h.div(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.wrap),
      h.DataAttribute("autopilot-session-actions", input.session.sessionRef),
    ],
    [
      h.button(
        [
          ...classAttrs<AutopilotUiMessage>(domainStyles.actionButton),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "spawn"),
        ],
        ["Spawn"],
      ),
      h.button(
        [
          ...classAttrs<AutopilotUiMessage>(domainStyles.actionButton),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "cancel"),
        ],
        ["Cancel"],
      ),
    ],
  )
}
