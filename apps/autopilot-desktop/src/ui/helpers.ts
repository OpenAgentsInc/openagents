// CL-53: pure presentational helpers for the Foldkit desktop view.
//
// These are the DOM-free derivations the panes/cards used before the Foldkit
// rewrite, lifted out of the deleted hand-DOM files so the view stays a thin
// mapping and the logic stays unit-testable without a runtime or a DOM.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type {
  AssignmentRow,
  SessionArtifactStats,
  SessionEventRow,
  WalletStatusRow,
} from "../shared/rpc"

// ── Node status line (Nodes pane) ───────────────────────────────────────────
export function nodeStatusLine(state: {
  ok: boolean
  sessions: ReadonlyArray<SessionSummary>
}): string {
  const status = state.ok ? "connected" : "offline"
  const count = state.sessions.length
  const noun = count === 1 ? "session" : "sessions"
  const by: Record<string, number> = {}
  for (const session of state.sessions) by[session.state] = (by[session.state] ?? 0) + 1
  const breakdown = ["running", "queued", "completed", "failed", "cancelled"]
    .filter((k) => by[k])
    .map((k) => `${by[k]} ${k}`)
    .join(" · ")
  return breakdown.length > 0
    ? `${status} · ${count} ${noun} · ${breakdown}`
    : `${status} · ${count} ${noun}`
}

// ── Sessions-pane state breakdown ────────────────────────────────────────────
export function stateBreakdown(sessions: ReadonlyArray<{ state: string }>): string {
  const counts: Record<string, number> = {}
  for (const s of sessions) counts[s.state] = (counts[s.state] ?? 0) + 1
  return Object.entries(counts)
    .map(([state, n]) => `${n} ${state}`)
    .join(" · ")
}

// ── Approvals (Needs you / Decisions) ────────────────────────────────────────
export function approvalLabel(row: { prompt: string; kind: string }): string {
  return row.prompt.trim() !== "" ? row.prompt : row.kind
}

// ── Ask Autopilot ship-status → short label ──────────────────────────────────
export type ShipStatusLine = {
  readonly text: string
  readonly terminal: boolean
  readonly dotColor: string
}

export function shipStatusLine(status: string): ShipStatusLine {
  switch (status) {
    case "received":
      return { text: "received", terminal: false, dotColor: "#8b949e" }
    case "planning":
      return { text: "planning…", terminal: false, dotColor: "#58a6ff" }
    case "fanning_out":
      return { text: "agents working…", terminal: false, dotColor: "#58a6ff" }
    case "shipping":
      return { text: "shipping…", terminal: false, dotColor: "#d29922" }
    case "shipped":
      return { text: "✓ shipped", terminal: true, dotColor: "#3fb950" }
    case "failed":
      return { text: "✗ failed", terminal: true, dotColor: "#f85149" }
    default:
      return { text: status, terminal: false, dotColor: "#8b949e" }
  }
}

// ── Balance (wallet) ─────────────────────────────────────────────────────────
export function walletSummary(wallet: WalletStatusRow): {
  value: string
  summary: string
} {
  const value =
    typeof wallet.balanceSats === "number"
      ? wallet.balanceSats.toLocaleString() + " sats"
      : "—"
  const onlineLabel = wallet.daemonOnline ? "wallet online" : "wallet offline"
  let summary = `${onlineLabel} · ${wallet.readiness}`
  if (wallet.receiveReady) summary += " · receive ✓"
  return { value, summary }
}

// ── Assignments ──────────────────────────────────────────────────────────────
export function assignmentMeta(row: AssignmentRow): { goal: string; meta: string } {
  const goal = row.goal.trim() !== "" ? row.goal : row.assignmentRef.slice(-8)
  const datePart = row.expiresAt ? ` · expires ${row.expiresAt.slice(0, 10)}` : ""
  const refSuffix = row.assignmentRef.slice(-6)
  const meta = `${row.paymentMode}${datePart} · ${refSuffix}`
  return { goal, meta }
}

// ── Settings: connection summary ──────────────────────────────────────────────
export function connectionSummary(node: { ok: boolean } | null): string {
  if (node === null) return "connecting…"
  return node.ok ? "online" : "offline"
}

// ── Coordinator toggle label ──────────────────────────────────────────────────
export function coordinatorToggleLabel(paused: boolean): string {
  return paused ? "▶ Resume" : "⏸ Pause"
}

// ── Spawn: split a verify textarea into trimmed non-empty lines ───────────────
export function parseVerifyLines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
}

// ── Session-detail verify line ────────────────────────────────────────────────
export function verifyLineText(session: SessionSummary): {
  text: string
  toneClass: string
} {
  const ext = session as unknown as {
    artifactRef?: string | null
    errorClass?: string | null
  }
  if (session.state === "completed") {
    const suffix = ext.artifactRef ? ` · artifact ${ext.artifactRef.slice(-12)}` : ""
    return { text: `✓ verify passed${suffix}`, toneClass: "verify-completed" }
  }
  if (session.state === "failed") {
    const suffix = ext.errorClass ? ` · ${ext.errorClass}` : ""
    return { text: `✗ verify failed${suffix}`, toneClass: "verify-failed" }
  }
  if (session.state === "cancelled") {
    return { text: "cancelled", toneClass: "verify-cancelled" }
  }
  return { text: `${session.state}…`, toneClass: "verify-cancelled" }
}

// ── Session-detail artifact line ──────────────────────────────────────────────
export function artifactLineText(
  stats: SessionArtifactStats | undefined | null,
): string {
  if (!stats) return ""
  const parts = [`artifact: ${stats.outcome ?? stats.kind}`]
  if (stats.editedFileCount != null) parts.push(`${stats.editedFileCount} files`)
  if (stats.commandCount != null) parts.push(`${stats.commandCount} cmds`)
  if (stats.totalTokens != null) parts.push(`${stats.totalTokens} tok`)
  return parts.join(" · ")
}

// ── Session-detail: is an event row expandable / cancellable session ──────────
export function eventExpandable(event: SessionEventRow): boolean {
  return (event.full != null && event.full.length > 0) || event.detail.length > 30
}

export function sessionCancellable(state: string): boolean {
  return state === "running" || state === "queued" || state === "started"
}

// ── Event timeline row label/time/meta ────────────────────────────────────────
export function eventRowText(
  event: SessionEventRow,
  expanded: boolean,
): { label: string; meta: string } {
  const expandable = eventExpandable(event)
  const label = expanded
    ? event.full || event.detail || event.phase
    : event.detail || event.phase
  const time = event.observedAt.slice(11, 19)
  let meta = `${event.phase} · #${event.eventIndex} · ${time}`
  if (expandable) meta += expanded ? " · tap to collapse" : " · tap to expand"
  return { label, meta }
}
