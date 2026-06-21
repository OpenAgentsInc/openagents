import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type SessionActionState = "queued" | "running" | "completed" | "failed" | "cancelled" | string

export type ActionSessionSummary = Readonly<{
  sessionRef: string
  adapter: string
  state: SessionActionState
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

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
      className(
        "grid gap-3 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-4 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-session-ref", session.sessionRef),
    ],
    [
      h.div([className("flex flex-wrap items-start justify-between gap-2")], [
        h.div([className("grid min-w-0 gap-1")], [
          h.code([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
            session.sessionRef,
          ]),
          h.span([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
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
  const actionClass =
    "inline-flex h-8 items-center rounded-[4px] border border-[var(--outline,#525458)] px-3 font-mono text-xs font-bold text-[var(--primary,#fff)] disabled:opacity-45"

  return h.div(
    [
      className("flex flex-wrap gap-2"),
      h.DataAttribute("autopilot-session-actions", input.session.sessionRef),
    ],
    [
      h.button(
        [
          className(actionClass),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "spawn"),
        ],
        ["Spawn"],
      ),
      h.button(
        [
          className(actionClass),
          h.Type("button"),
          h.Disabled(input.readOnly || terminal),
          h.DataAttribute("autopilot-action", "cancel"),
        ],
        ["Cancel"],
      ),
    ],
  )
}
