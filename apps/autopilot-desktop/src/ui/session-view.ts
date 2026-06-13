import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { SessionArtifactStats, SessionEventRow } from "../shared/rpc"

function artifactLine(stats: SessionArtifactStats | undefined): string {
  if (!stats) return ""
  const parts = [`artifact: ${stats.outcome ?? stats.kind}`]
  if (stats.editedFileCount !== null) parts.push(`${stats.editedFileCount} files`)
  if (stats.commandCount !== null) parts.push(`${stats.commandCount} cmds`)
  if (stats.totalTokens !== null) parts.push(`${stats.totalTokens} tok`)
  return `<p class="artifact-line">${parts.join(" · ")}</p>`
}

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
  artifacts: Record<string, SessionArtifactStats> = {},
): string {
  const renderOne = (session: SessionSummary, child: boolean): string => {
    const verify =
      session.state === "completed"
        ? "✓ verify passed"
        : session.state === "failed"
          ? "✗ verify failed"
          : session.state === "cancelled"
            ? "cancelled"
            : ""
    const activity = session.latestActivity && session.latestActivity.length > 0 ? session.latestActivity : session.state
    const shortRef = session.sessionRef.slice(-6)
    return [
      `<div class="session-row${child ? " session-child" : ""}">`,
      `<span class="state-chip state-${escapeHtml(session.state)}">${escapeHtml(session.state)}</span>`,
      `<span class="session-activity">${child ? "↳ " : ""}${escapeHtml(activity)}</span>`,
      `<code class="session-short">${escapeHtml(session.adapter)}·${escapeHtml(shortRef)}</code>`,
      "</div>",
      verify.length > 0 ? `<p class="verify-line verify-${escapeHtml(session.state)}">${escapeHtml(verify)}</p>` : "",
      artifactLine(artifacts[session.sessionRef]),
      timelineHtml(events[session.sessionRef] ?? []),
    ].join("")
  }

  // Nest sub-agents (parentRef) under their parent (#4951).
  const childrenOf = (ref: string) => sessions.filter((s) => s.parentRef === ref)
  const isTop = (s: SessionSummary) => !s.parentRef || !sessions.some((p) => p.sessionRef === s.parentRef)
  return sessions
    .filter(isTop)
    .map((s) => renderOne(s, false) + childrenOf(s.sessionRef).map((c) => renderOne(c, true)).join(""))
    .join("")
}

export function nodeStatusLine(state: { ok: boolean; sessions: SessionSummary[] }): string {
  const status = state.ok ? "connected" : "offline"
  const count = state.sessions.length
  const noun = count === 1 ? "session" : "sessions"
  const by: Record<string, number> = {}
  for (const session of state.sessions) by[session.state] = (by[session.state] ?? 0) + 1
  const breakdown = ["running", "queued", "completed", "failed", "cancelled"]
    .filter((k) => by[k])
    .map((k) => `${by[k]} ${k}`)
    .join(" · ")
  return breakdown.length > 0 ? `${status} · ${count} ${noun} · ${breakdown}` : `${status} · ${count} ${noun}`
}
