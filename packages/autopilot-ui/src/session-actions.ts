import { stylexAttrs } from "@openagentsinc/ui/stylex-foldkit"
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
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-session-ref", session.sessionRef),
    ],
    [
      h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stack), [
          h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
            session.sessionRef,
          ]),
          h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.muted), [
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
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.wrap),
      h.DataAttribute("autopilot-session-actions", input.session.sessionRef),
    ],
    [
      h.button(
        [
          ...stylexAttrs<AutopilotUiMessage>(domainStyles.actionButton),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "spawn"),
        ],
        ["Spawn"],
      ),
      h.button(
        [
          ...stylexAttrs<AutopilotUiMessage>(domainStyles.actionButton),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "cancel"),
        ],
        ["Cancel"],
      ),
    ],
  )
}
