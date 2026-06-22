import type {
  DecisionRecord,
  SessionEvent,
  SessionState,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import {
  stylexAttrs,
  stylexFallback,
  stylexRuntimeFallbackEnabled,
} from "@openagentsinc/ui/stylex-foldkit"
import * as stylex from "@stylexjs/stylex"
import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"

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

const sessionStyles = stylexRuntimeFallbackEnabled()
  ? {
      list: stylexFallback("oa-autopilot-session-list"),
      row: stylexFallback("oa-autopilot-session-row"),
      sessionRef: stylexFallback("oa-autopilot-session-ref"),
      adapter: stylexFallback("oa-autopilot-session-adapter"),
      progressRef: stylexFallback("oa-autopilot-session-progress-ref"),
      empty: stylexFallback("oa-autopilot-session-empty"),
    }
  : stylex.create({
      list: {
        display: "grid",
        gap: 8,
      },
      row: {
        display: "grid",
        gap: 8,
        borderWidth: 1,
        borderStyle: "solid",
        borderColor: "var(--outline,#525458)",
        backgroundColor: "var(--bg-secondary,#151515)",
        padding: 12,
        color: "var(--text,#d7d8e5)",
        gridTemplateColumns: {
          default: null,
          "@media (min-width: 640px)":
            "minmax(0,1.7fr) 7rem 7rem minmax(0,1fr)",
        },
        alignItems: {
          default: null,
          "@media (min-width: 640px)": "center",
        },
      },
      sessionRef: {
        minWidth: 0,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
        fontSize: 14,
        color: "var(--primary,#fff)",
      },
      adapter: {
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
        fontSize: 12,
        color: "var(--text-secondary,#8a8c93)",
      },
      progressRef: {
        minWidth: 0,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
        fontSize: 12,
        color: "var(--text-secondary,#8a8c93)",
      },
      empty: {
        margin: 0,
        fontSize: 14,
        color: "var(--text-secondary,#8a8c93)",
      },
    })

const toneClasses = (tone: ChipTone): string => {
  switch (tone) {
    case "success":
      return "border-[#00c853]/60 bg-[#00c853]/10 text-[#86efac]"
    case "warning":
      return "border-[#ffb400]/60 bg-[#ffb400]/10 text-[#facc15]"
    case "danger":
      return "border-[#d32f2f]/70 bg-[#d32f2f]/10 text-[#fca5a5]"
    case "info":
      return "border-sky-400/60 bg-sky-400/10 text-sky-200"
    case "neutral":
      return "border-[var(--outline,#525458)] bg-transparent text-[var(--text-secondary,#8a8c93)]"
  }
}

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
      className(
        [
          "inline-flex h-6 items-center rounded-[4px] border px-2 font-mono text-xs font-bold leading-none whitespace-nowrap",
          toneClasses(input.tone ?? "neutral"),
        ].join(" "),
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
      className(
        "grid gap-3 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-4 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-decision-id", input.decision.requestId),
    ],
    [
      h.div([className("flex flex-wrap items-start justify-between gap-2")], [
        h.div([className("grid min-w-0 gap-1")], [
          h.h3([className("m-0 font-mono text-sm font-bold text-[var(--primary,#fff)]")], [
            input.decision.actionRef,
          ]),
          h.code([className("min-w-0 truncate font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
            input.decision.requestId,
          ]),
        ]),
        statusChip({
          label: input.decision.state,
          tone: decisionTone(input.decision.state),
          attrs: [h.DataAttribute("autopilot-decision-state", input.decision.state)],
        }),
      ]),
      h.dl([className("grid gap-2 text-xs sm:grid-cols-2")], [
        h.div([className("grid gap-1")], [
          h.dt([className("font-mono uppercase text-[var(--text-secondary,#8a8c93)]")], ["Resolved"]),
          h.dd([className("m-0 font-mono text-[var(--primary,#fff)]")], [resolved]),
        ]),
        h.div([className("grid gap-1")], [
          h.dt([className("font-mono uppercase text-[var(--text-secondary,#8a8c93)]")], ["Expires"]),
          h.dd([className("m-0 font-mono text-[var(--primary,#fff)]")], [
            String(input.decision.expiresAtMs),
          ]),
        ]),
      ]),
      h.div([className("flex flex-wrap gap-2")], [
        ...actions.map((action) =>
          h.button(
            [
              className(
                "inline-flex h-8 items-center rounded-[4px] border border-[var(--outline,#525458)] px-3 font-mono text-xs font-bold text-[var(--primary,#fff)] disabled:opacity-45",
              ),
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
    [className("grid gap-2"), h.DataAttribute("autopilot-event-timeline", "")],
    input.events.length === 0
      ? [
          h.li([className("text-sm text-[var(--text-secondary,#8a8c93)]")], [
            input.emptyLabel ?? "No events",
          ]),
        ]
      : input.events.map((event) =>
          h.li(
            [
              className(
                "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 sm:grid-cols-[4rem_10rem_minmax(0,1fr)_minmax(0,1fr)] sm:items-center",
              ),
              h.DataAttribute("autopilot-event-id", event.eventId),
            ],
            [
              h.code([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
                `#${event.sequence}`,
              ]),
              statusChip({ label: event.phase, tone: eventTone(event) }),
              h.time([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
                event.observedAt,
              ]),
              h.code([className("min-w-0 truncate font-mono text-xs text-[var(--primary,#fff)]")], [
                event.detailRef ?? event.sessionRef,
              ]),
            ],
          ),
        ),
  )
