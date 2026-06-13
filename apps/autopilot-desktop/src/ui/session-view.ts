import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { SessionEventRow } from "../shared/rpc"

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")
}

function timelineHtml(events: SessionEventRow[]): string {
  if (events.length === 0) return ""
  const items = events
    .map((event) => {
      const label = event.detail.length > 0 ? event.detail : event.phase
      const time = event.observedAt.slice(11, 19)
      return [
        `<li class="event-row event-${escapeHtml(event.state)}">`,
        `<span class="event-detail">${escapeHtml(label)}</span>`,
        `<span class="event-meta">${escapeHtml(event.phase)} · ${escapeHtml(time)}</span>`,
        "</li>",
      ].join("")
    })
    .join("")
  return `<ul class="session-timeline">${items}</ul>`
}

export function sessionRows(
  sessions: SessionSummary[],
  events: Record<string, SessionEventRow[]> = {},
): string {
  return sessions
    .map((session) => {
      const lastProgressRef = session.lastProgressRef ?? "none"
      return [
        '<div class="session-row">',
        `<code>${escapeHtml(session.sessionRef)}</code>`,
        `<span>${escapeHtml(session.adapter)}</span>`,
        `<span class="state-chip state-${escapeHtml(session.state)}">${escapeHtml(session.state)}</span>`,
        `<code>${escapeHtml(lastProgressRef)}</code>`,
        "</div>",
        timelineHtml(events[session.sessionRef] ?? []),
      ].join("")
    })
    .join("")
}

export function nodeStatusLine(state: { ok: boolean; sessions: SessionSummary[] }): string {
  const status = state.ok ? "connected" : "offline"
  const count = state.sessions.length
  const noun = count === 1 ? "session" : "sessions"
  return `${status} · ${count} ${noun}`
}
