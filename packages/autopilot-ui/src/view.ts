import type {
  DecisionRecord,
  SessionEvent,
  SessionState,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import {
  stylexAttrs,
  stylexFallback,
} from "@openagentsinc/ui/stylex-foldkit"
import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"

export type AutopilotUiMessage = never

export type Staleness = "fresh" | "stale" | "unknown"

export type LagStatus = Readonly<{
  label: string
  value: number
  tone?: ChipTone
}>

export type ChipTone = "neutral" | "success" | "warning" | "danger" | "info"

export type DecisionAction = Readonly<{
  label: string
  verb: "approve" | "deny" | "answer"
  disabled?: boolean
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const sessionStyles = {
  list: stylexFallback("oa-autopilot-session-list"),
  row: stylexFallback("oa-autopilot-session-row"),
  sessionRef: stylexFallback("oa-autopilot-session-ref"),
  adapter: stylexFallback("oa-autopilot-session-adapter"),
  progressRef: stylexFallback("oa-autopilot-session-progress-ref"),
  empty: stylexFallback("oa-autopilot-session-empty"),
} as const

const chipToneStyles = {
  neutral: domainStyles.chipNeutral,
  success: domainStyles.chipSuccess,
  warning: domainStyles.chipWarning,
  danger: domainStyles.chipDanger,
  info: domainStyles.chipInfo,
} as const satisfies Record<ChipTone, (typeof domainStyles)[keyof typeof domainStyles]>

export const sessionStateTone = (state: SessionState): ChipTone => {
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
  }
}

export const statusChip = (input: {
  label: string
  tone?: ChipTone
  attrs?: ReadonlyArray<Attribute<AutopilotUiMessage>>
}): Html =>
  h.span(
    [
      ...(input.attrs ?? []),
      ...stylexAttrs<AutopilotUiMessage>(
        domainStyles.chip,
        chipToneStyles[input.tone ?? "neutral"],
      ),
    ],
    [input.label],
  )

export const sessionStateChip = (state: SessionState): Html =>
  statusChip({
    label: state,
    tone: sessionStateTone(state),
    attrs: [h.DataAttribute("autopilot-state", state)],
  })

export const stalenessChip = (staleness: Staleness): Html =>
  statusChip({
    label: staleness,
    tone:
      staleness === "fresh"
        ? "success"
        : staleness === "stale"
          ? "warning"
          : "neutral",
    attrs: [h.DataAttribute("autopilot-staleness", staleness)],
  })

export const lagChip = (lag: LagStatus): Html =>
  statusChip({
    label: `${lag.label}: ${lag.value}`,
    tone: lag.tone ?? (lag.value === 0 ? "success" : lag.value < 3 ? "warning" : "danger"),
    attrs: [h.DataAttribute("autopilot-lag", lag.label)],
  })

export const SessionRow = (session: SessionSummary): Html => {
  const lastProgressRef = session.lastProgressRef ?? "none"

  return h.article(
    [
      ...stylexAttrs<AutopilotUiMessage>(sessionStyles.row),
      className("session-row"),
      h.DataAttribute("autopilot-session-ref", session.sessionRef),
    ],
    [
      h.code(stylexAttrs<AutopilotUiMessage>(sessionStyles.sessionRef), [session.sessionRef]),
      h.span(stylexAttrs<AutopilotUiMessage>(sessionStyles.adapter), [session.adapter]),
      sessionStateChip(session.state),
      h.code(stylexAttrs<AutopilotUiMessage>(sessionStyles.progressRef), [lastProgressRef]),
    ],
  )
}

export const SessionList = (input: {
  sessions: ReadonlyArray<SessionSummary>
  emptyLabel?: string
}): Html =>
  h.section(
    [
      ...stylexAttrs<AutopilotUiMessage>(sessionStyles.list),
      h.DataAttribute("autopilot-session-list", ""),
    ],
    input.sessions.length === 0
      ? [
          h.p(stylexAttrs<AutopilotUiMessage>(sessionStyles.empty), [
            input.emptyLabel ?? "No sessions",
          ]),
        ]
      : input.sessions.map(SessionRow),
  )

const decisionTone = (state: DecisionRecord["state"]): ChipTone => {
  switch (state) {
    case "pending":
      return "warning"
    case "resolved":
      return "success"
    case "cancelled":
    case "expired":
      return "danger"
  }
}

export const DecisionCard = (input: {
  decision: DecisionRecord
  actions?: ReadonlyArray<DecisionAction>
}): Html => {
  const resolved = input.decision.resolvedVerb === null ? "none" : input.decision.resolvedVerb
  const actions =
    input.actions ??
    ([
      { label: "Approve", verb: "approve" },
      { label: "Deny", verb: "deny" },
      { label: "Answer", verb: "answer" },
    ] satisfies ReadonlyArray<DecisionAction>)
  const disabled = input.decision.state !== "pending"

  return h.article(
    [
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-decision-id", input.decision.requestId),
    ],
    [
      h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stack), [
          h.h3(stylexAttrs<AutopilotUiMessage>(domainStyles.title), [
            input.decision.actionRef,
          ]),
          h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [
            input.decision.requestId,
          ]),
        ]),
        statusChip({
          label: input.decision.state,
          tone: decisionTone(input.decision.state),
          attrs: [h.DataAttribute("autopilot-decision-state", input.decision.state)],
        }),
      ]),
      h.dl(stylexAttrs<AutopilotUiMessage>(domainStyles.metadataGrid), [
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stackSmall), [
          h.dt(stylexAttrs<AutopilotUiMessage>(domainStyles.label), ["Resolved"]),
          h.dd(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [resolved]),
        ]),
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stackSmall), [
          h.dt(stylexAttrs<AutopilotUiMessage>(domainStyles.label), ["Expires"]),
          h.dd(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
            String(input.decision.expiresAtMs),
          ]),
        ]),
      ]),
      h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.wrap), [
        ...actions.map((action) =>
          h.button(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.actionButton),
              h.Type("button"),
              h.Disabled(disabled || action.disabled === true),
              h.DataAttribute("autopilot-decision-action", action.verb),
            ],
            [action.label],
          ),
        ),
      ]),
    ],
  )
}

const eventTone = (event: SessionEvent): ChipTone => {
  switch (event.phase) {
    case "completed":
    case "decision_resolved":
    case "artifact_available":
      return "success"
    case "decision_requested":
    case "progress":
    case "started":
      return "info"
    case "decision_cancelled":
    case "failed":
    case "cancelled":
      return "danger"
  }
}

export const EventTimeline = (input: {
  events: ReadonlyArray<SessionEvent>
  emptyLabel?: string
}): Html =>
  h.ol(
    [
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-event-timeline", ""),
    ],
    input.events.length === 0
      ? [
          h.li(stylexAttrs<AutopilotUiMessage>(domainStyles.empty), [
            input.emptyLabel ?? "No events",
          ]),
        ]
      : input.events.map((event) =>
          h.li(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.eventRow),
              h.DataAttribute("autopilot-event-id", event.eventId),
            ],
            [
              h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [
                `#${event.sequence}`,
              ]),
              statusChip({ label: event.phase, tone: eventTone(event) }),
              h.time(stylexAttrs<AutopilotUiMessage>(domainStyles.muted), [
                event.observedAt,
              ]),
              h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [
                event.detailRef ?? event.sessionRef,
              ]),
            ],
          ),
        ),
  )
