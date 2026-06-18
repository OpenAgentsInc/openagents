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
  TrainingRunsResponse,
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

const genericTryPromiseError = "An error occurred in Effect.tryPromise"

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

export const commandErrorText = (error: unknown, depth = 0): string => {
  if (depth > 5) return String(error)
  if (error instanceof Error) {
    const cause = isRecord(error) ? error.cause : undefined
    if (error.message === genericTryPromiseError && cause !== undefined) {
      return commandErrorText(cause, depth + 1)
    }
    return error.message
  }
  if (isRecord(error)) {
    const message = typeof error.message === "string" ? error.message : ""
    const cause = error.cause
    if (message === genericTryPromiseError && cause !== undefined) {
      return commandErrorText(cause, depth + 1)
    }
    if (message.trim() !== "") return message
  }
  return String(error)
}

const trainingProjectionFailureMeta = (error: string | undefined): string =>
  error === undefined || error === genericTryPromiseError
    ? "waiting for Worker projection"
    : `Worker projection unavailable · ${error}`

export const trainingProjectionMeta = (
  projection: TrainingRunsResponse | null,
): string => {
  if (projection === null) return "waiting for Worker projection"
  const when =
    projection.fetchedAt.length > 0
      ? new Date(projection.fetchedAt).toLocaleTimeString()
      : "unknown time"
  return projection.ok
    ? `${projection.runs.length} runs · fetched ${when}`
    : trainingProjectionFailureMeta(projection.error)
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

// ── #5355: coding composer ────────────────────────────────────────────────────
//
// The control protocol exposes only bounded `session.spawn`/`events`/`cancel`
// (+ approvals); there is no `session.reply` verb. So the composer's reply /
// continue turn is realized as a CONTINUATION spawn: a new bounded session whose
// objective carries the prior turn context, run in the same repo/worktree. This
// keeps the iterative loop on the EXISTING contract (no new wire schema). These
// pure helpers build the continuation objective and decide turn affordances, so
// the loop stays unit-testable without a runtime or a DOM.

// A coding event is "interesting" transcript content (the agent's text, tool
// call, or file change) — used to render a readable turn view rather than the
// raw lifecycle-only timeline.
export function isComposerTranscriptEvent(event: SessionEventRow): boolean {
  return event.detail.trim().length > 0 || (event.full != null && event.full.trim().length > 0)
}

// Build the objective for a continuation turn. The agent gets the prior turns
// as context plus the new follow-up so the conversation is coherent across the
// bounded-session boundary. Bounded so the objective stays a sane size.
export function buildComposerContinuationObjective(
  priorTurns: ReadonlyArray<string>,
  followUp: string,
): string {
  const trimmedFollowUp = followUp.trim()
  const recent = priorTurns
    .map((t) => t.trim())
    .filter((t) => t.length > 0)
    .slice(-4)
  if (recent.length === 0) return trimmedFollowUp
  const context = recent
    .map((t, i) => `${i + 1}. ${t}`)
    .join("\n")
  return [
    "Continue the current coding session. Earlier turns in this thread:",
    context,
    "",
    "Next instruction:",
    trimmedFollowUp,
  ].join("\n")
}

// Is the composer thread accepting a follow-up turn yet? A reply is allowed
// once the active turn reaches a terminal state (the bounded session finished),
// so continuation turns don't stomp a still-running session.
export function composerCanReply(activeSessionState: string | null): boolean {
  if (activeSessionState === null) return false
  return (
    activeSessionState === "completed" ||
    activeSessionState === "failed" ||
    activeSessionState === "cancelled"
  )
}

// One-line status for the composer's active turn, for the pane header.
export function composerTurnSummary(
  activeSessionState: string | null,
  turnCount: number,
): string {
  if (activeSessionState === null) {
    return turnCount === 0 ? "no session yet" : "thread reset"
  }
  const turnLabel = turnCount === 1 ? "1 turn" : `${turnCount} turns`
  switch (activeSessionState) {
    case "queued":
      return `queued · ${turnLabel}`
    case "running":
    case "started":
      return `running · ${turnLabel}`
    case "completed":
      return `done · ${turnLabel} · reply to continue`
    case "failed":
      return `failed · ${turnLabel} · reply to retry`
    case "cancelled":
      return `cancelled · ${turnLabel} · reply to continue`
    default:
      return `${activeSessionState} · ${turnLabel}`
  }
}

// ── CS-A2 (#5362): swarm / multi-session view ────────────────────────────────
//
// The swarm view is a lane/grid over the N concurrent coding sessions the
// runtime can already run (concurrent spawner #4869, control `session.list`,
// external-session `parentRef` nesting). It is a pure read projection over the
// existing node-state (`sessions` + per-session `events` + global `approvals`):
// NO new wire verb. These helpers are the DOM-free derivations each cell and the
// top-level roll-up render, so the swarm view stays unit-testable without a
// runtime or a DOM, the same way the composer/CL-53 helpers do.

// A session is "active" while it has not reached a terminal state. Active
// sessions sort first in the grid so the running swarm is the foreground.
export function isActiveSwarmSession(state: string): boolean {
  return state === "queued" || state === "running" || state === "started"
}

// A short, public-safe status label for a swarm cell (refs only; no raw paths).
export function swarmStatusLabel(state: string): {
  text: string
  toneClass: string
} {
  switch (state) {
    case "queued":
      return { text: "queued", toneClass: "swarm-queued" }
    case "running":
    case "started":
      return { text: "running", toneClass: "swarm-running" }
    case "completed":
      return { text: "done", toneClass: "swarm-completed" }
    case "failed":
      return { text: "failed", toneClass: "swarm-failed" }
    case "cancelled":
      return { text: "cancelled", toneClass: "swarm-cancelled" }
    default:
      return { text: state, toneClass: "swarm-cancelled" }
  }
}

// The account a cell is running under. CS-A1 surfaces accounts as `AccountRow`s
// keyed by a stable `accountRefHash`; a spawned session carries the same
// `accountRefHash`, so the cell can name the account without a raw secret/path.
// Falls back to the provider adapter / "default" when the session has no hash
// (e.g. a default-home spawn) or the row is not in the readiness projection.
export function swarmAccountLabel(
  session: { accountRefHash: string | null; adapter: string },
  accounts: ReadonlyArray<{ accountRefHash: string; provider: string; selector: string }>,
): string {
  if (session.accountRefHash !== null && session.accountRefHash !== "") {
    const match = accounts.find((a) => a.accountRefHash === session.accountRefHash)
    if (match) {
      const which =
        match.selector === "default_home"
          ? "default"
          : session.accountRefHash.slice(-6)
      return `${match.provider} · ${which}`
    }
    // Hash present but no matching readiness row — still name it by its suffix.
    return `${session.adapter} · ${session.accountRefHash.slice(-6)}`
  }
  return `${session.adapter} · default`
}

// The repo / worktree a cell is running in. `workspaceRef` is a public-safe ref
// (no raw path); show its tail, or a neutral placeholder when absent.
export function swarmWorkspaceLabel(session: {
  workspaceRef?: string | null
}): string {
  const ref = session.workspaceRef ?? null
  if (ref === null || ref.trim() === "") return "—"
  return ref.length > 18 ? `…${ref.slice(-16)}` : ref
}

// Per-cell pending-approval count. The node's authoritative pending queue
// (`approvals[]`) is NOT keyed by session, so a per-cell count is derived from
// the session's bounded event tail: a `decision_requested` not yet followed by a
// `decision_resolved`/`decision_cancelled` for the same session. Best-effort,
// public-safe, and matches what the session-detail timeline already shows.
export function swarmSessionPendingApprovals(
  events: ReadonlyArray<{ phase: string }> | undefined,
): number {
  if (!events || events.length === 0) return 0
  let pending = 0
  for (const e of events) {
    if (e.phase === "decision_requested") pending += 1
    else if (e.phase === "decision_resolved" || e.phase === "decision_cancelled") {
      if (pending > 0) pending -= 1
    }
  }
  return pending
}

// Order the swarm grid: active sessions first (by most-recent update), then
// terminal sessions. `parentRef` children are kept adjacent to (and after)
// their parent so nested external/host sessions read as a sub-lane rather than
// scattering across the grid.
export function orderSwarmSessions<
  T extends {
    sessionRef: string
    state: string
    parentRef?: string | null
    updatedAt: string
  },
>(sessions: ReadonlyArray<T>): ReadonlyArray<T> {
  const activeRank = (s: T): number => (isActiveSwarmSession(s.state) ? 0 : 1)
  const byRecency = (a: T, b: T): number => {
    const rank = activeRank(a) - activeRank(b)
    if (rank !== 0) return rank
    return b.updatedAt.localeCompare(a.updatedAt)
  }
  // Roots = sessions with no parent in this set; children attach under them.
  const refs = new Set(sessions.map((s) => s.sessionRef))
  const childrenByParent = new Map<string, T[]>()
  const roots: T[] = []
  for (const s of sessions) {
    const parent = s.parentRef ?? null
    if (parent !== null && parent !== s.sessionRef && refs.has(parent)) {
      const list = childrenByParent.get(parent) ?? []
      list.push(s)
      childrenByParent.set(parent, list)
    } else {
      roots.push(s)
    }
  }
  const ordered: T[] = []
  for (const root of [...roots].sort(byRecency)) {
    ordered.push(root)
    const kids = childrenByParent.get(root.sessionRef)
    if (kids) for (const kid of [...kids].sort(byRecency)) ordered.push(kid)
  }
  return ordered
}

// Top-level swarm summary: how many sessions, broken down by active count, plus
// the total pending approvals across all sessions (the authoritative global
// queue length). One line for the swarm-pane header.
export function swarmSummaryLine(
  sessions: ReadonlyArray<{ state: string }>,
  pendingApprovalCount: number,
): string {
  const total = sessions.length
  if (total === 0) return "no sessions"
  const active = sessions.filter((s) => isActiveSwarmSession(s.state)).length
  const noun = total === 1 ? "session" : "sessions"
  const parts = [`${total} ${noun}`, `${active} active`]
  if (pendingApprovalCount > 0) {
    parts.push(
      `${pendingApprovalCount} pending approval${pendingApprovalCount === 1 ? "" : "s"}`,
    )
  }
  return parts.join(" · ")
}
