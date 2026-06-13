import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")
}

export function sessionRows(sessions: SessionSummary[]): string {
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
