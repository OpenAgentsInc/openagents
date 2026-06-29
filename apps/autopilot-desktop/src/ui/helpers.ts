// CL-53: pure presentational helpers for the Foldkit desktop view.
//
// These are the DOM-free derivations the panes/cards used before the Foldkit
// rewrite, lifted out of the deleted hand-DOM files so the view stays a thin
// mapping and the logic stays unit-testable without a runtime or a DOM.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type {
  AssignmentRow,
  IntentRow,
  PylonFleetCapacityState,
  PylonFleetReconciliation,
  SessionArtifactStats,
  SessionEventRow,
  TrainingRunsResponse,
  WalletStatusRow,
} from "../shared/rpc.js"

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
      return { text: "planning…", terminal: false, dotColor: "#fff" }
    case "fanning_out":
      return { text: "agents working…", terminal: false, dotColor: "#fff" }
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

// ── #5467: Autonomous coordinator loop (intent → plan → fanout → reconcile →
// ship) — pure derivations over the REAL `intent.list` projection ────────────
//
// The desktop reads exactly what the control protocol exposes today
// (`intent.list` projections + `coordinator.status` paused flag); it never
// fabricates a per-intent session mapping the node does not publish. The five
// loop stages map onto the node's REAL `IntentStatus` lifecycle so the view is
// a faithful projection of the coordinator runtime, not a mock.
//
//   intent     ← the ask was received / queued        (received)
//   plan       ← the planner is fanning the ask        (planning)
//   fanout     ← a session per part is running         (fanning_out)
//   reconcile  ← parts converged, ship decision next   (shipping)
//   ship       ← terminal: shipped (or failed)         (shipped | failed)

export const LOOP_STAGES = [
  { id: "intent", label: "Intent", statuses: ["received"] },
  { id: "plan", label: "Plan", statuses: ["planning"] },
  { id: "fanout", label: "Fanout", statuses: ["fanning_out"] },
  { id: "reconcile", label: "Reconcile", statuses: ["shipping"] },
  { id: "ship", label: "Ship", statuses: ["shipped", "failed"] },
] as const

export type LoopStageId = (typeof LOOP_STAGES)[number]["id"]

export type LoopStageState = "done" | "active" | "pending" | "failed"

// Ordered status rank used to decide which stages an intent has passed. `failed`
// is terminal but is rendered at the ship stage as a failure, not progress.
const LOOP_STATUS_RANK: Record<string, number> = {
  received: 0,
  planning: 1,
  fanning_out: 2,
  shipping: 3,
  shipped: 4,
  failed: 4,
}

// For one intent, classify each of the five loop stages. Derived from the live
// `status` plus the REAL `statusHistory` (a stage the intent actually passed
// through is `done`; the current stage is `active`; later stages are `pending`;
// a `failed` intent marks its furthest-reached stage as `failed`).
export function loopStageStates(
  intent: Pick<IntentRow, "status" | "statusHistory">,
): ReadonlyArray<{ id: LoopStageId; label: string; state: LoopStageState }> {
  const rank = LOOP_STATUS_RANK[intent.status] ?? 0
  const failed = intent.status === "failed"
  const shipped = intent.status === "shipped"
  const reached = new Set<string>(
    (intent.statusHistory ?? [])
      .map((event: { status: string }) => event.status)
      .concat(intent.status),
  )
  return LOOP_STAGES.map((stage) => {
    const stageRank = LOOP_STATUS_RANK[stage.statuses[0]] ?? 0
    let state: LoopStageState
    if (failed && stageRank === rank) state = "failed"
    else if (stageRank < rank) state = "done"
    // A terminal success (`shipped`) has completed the ship stage; only an
    // in-flight intent's current stage is "active".
    else if (stageRank === rank) state = shipped ? "done" : "active"
    else state = stage.statuses.some((s) => reached.has(s)) ? "done" : "pending"
    return { id: stage.id, label: stage.label, state }
  })
}

// The ship gate, stated HONESTLY (audit §1.2 / issue acceptance): the autonomous
// ship step is triple-gated (spend-gate eligible AND decision auto AND
// OA_SHIP_AUTO_EXECUTE=1) and the spend gate DEFAULTS TO DENY, so an autonomous
// intent escalates to the owner rather than spending. This copy never implies
// autonomous spend.
export type ShipGateLine = {
  readonly text: string
  readonly tone: "neutral" | "active" | "shipped" | "failed"
}

export function shipGateLine(status: string): ShipGateLine {
  switch (status) {
    case "shipped":
      return { text: "Shipped (owner-gated; default-DENY held)", tone: "shipped" }
    case "failed":
      return { text: "Failed before ship", tone: "failed" }
    case "shipping":
      return { text: "Ship gate: default-DENY → escalates to you", tone: "active" }
    default:
      return { text: "Ship gate pending (default-DENY; no autonomous spend)", tone: "neutral" }
  }
}

// One-line roll-up for the loop header: how many asks are in-flight vs terminal,
// and whether the coordinator is paused.
export function autonomousLoopSummary(
  intents: ReadonlyArray<Pick<IntentRow, "status">>,
  paused: boolean | null,
): string {
  const active = intents.filter(
    (i) => i.status !== "shipped" && i.status !== "failed",
  ).length
  const shipped = intents.filter((i) => i.status === "shipped").length
  const failed = intents.filter((i) => i.status === "failed").length
  const loopState =
    paused === null ? "coordinator status unknown" : paused ? "paused" : "running"
  if (intents.length === 0) return `${loopState} · no asks yet`
  const parts = [`${active} in-flight`]
  if (shipped > 0) parts.push(`${shipped} shipped`)
  if (failed > 0) parts.push(`${failed} failed`)
  return `${loopState} · ${parts.join(" · ")}`
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

export function pylonFleetCapacityLabel(
  state: PylonFleetCapacityState,
): string {
  switch (state) {
    case "verified":
      return "capacity verified"
    case "stale":
      return "heartbeat stale"
    case "blocked":
      return "capacity blocked"
    case "unknown":
      return "capacity unknown"
  }
}

export function pylonFleetSummary(
  fleet: PylonFleetReconciliation,
): {
  line: string
  capacityLine: string
  tone: "ready" | "watch" | "blocked"
} {
  const parts = [
    `${fleet.counts.pylons} pylons`,
    `${fleet.counts.assigned} assigned`,
    `${fleet.counts.executing} executing`,
    `${fleet.counts.stale} stale`,
    `${fleet.counts.accepted} accepted`,
    `${fleet.counts.rejected} rejected`,
    `${fleet.counts.tokenFailures} token failures`,
  ]
  const age =
    fleet.capacity.ageSeconds === null
      ? "no heartbeat"
      : `${fleet.capacity.ageSeconds}s old`
  const slots =
    fleet.capacity.availableCodexSlots === null
      ? "slots unknown"
      : `${fleet.capacity.availableCodexSlots} slots available`
  const tone =
    fleet.capacity.state === "blocked"
      ? "blocked"
      : fleet.capacity.state === "verified" &&
          fleet.counts.stale === 0 &&
          fleet.counts.tokenFailures === 0
        ? "ready"
        : "watch"
  return {
    capacityLine: `${pylonFleetCapacityLabel(fleet.capacity.state)} · ${age} · ${slots}`,
    line: parts.join(" · "),
    tone,
  }
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

// ── #5470 session-detail artifact & receipt browser ───────────────────────────
//
// Build the inspectable ref rows for a session's retained artifact. Each row is
// a (label, value) pair where the value is ALWAYS a ref/digest/enum the node
// already deemed public-projection-safe — never a seed, token, raw path, or raw
// secret. Null/empty fields are dropped so the browser shows only what exists.
// Pure + DOM-free so it is unit-testable without a runtime.
export type ArtifactBrowserRow = Readonly<{ label: string; value: string }>

export type ArtifactBrowserSection = Readonly<{
  // A stable id for the section (artifact kind / receipts).
  id: string
  title: string
  rows: ReadonlyArray<ArtifactBrowserRow>
}>

export function artifactBrowserSections(
  stats: SessionArtifactStats | undefined | null,
): ReadonlyArray<ArtifactBrowserSection> {
  if (!stats) return []
  const detail = stats.detail
  const sections: Array<ArtifactBrowserSection> = []

  // The artifact section: the proof/failure refs and provenance enums.
  const artifactRows: Array<ArtifactBrowserRow> = []
  const push = (label: string, value: string | null | undefined): void => {
    if (value != null && value.length > 0) artifactRows.push({ label, value })
  }
  push("kind", stats.kind === "none" ? null : stats.kind)
  push("outcome", stats.outcome)
  if (detail) {
    push("schema", detail.schema)
    push("objective", detail.objectiveDigestRef)
    push("verify", detail.verifyRef)
    push("response", detail.responseDigestRef)
    push("external session", detail.externalSessionRef)
    push("execution path", detail.executionPathRef)
    push("execution mode", detail.executionMode)
    push("sandbox", detail.sandboxMode)
    push("permission", detail.permissionMode)
    push("dev-check", detail.devCheckState)
    push("redaction", detail.redactionState)
    push("workspace", detail.workspaceRef)
    push("error class", detail.errorClass)
    push("error digest", detail.errorDigestRef)
    for (const deviation of Array.isArray(detail.deviationRefs) ? detail.deviationRefs : []) {
      artifactRows.push({ label: "deviation", value: deviation })
    }
  }
  if (artifactRows.length > 0) {
    const isFailure = stats.kind === "failure"
    sections.push({
      id: stats.kind,
      title: isFailure ? "Failure artifact" : "Proof artifact",
      rows: artifactRows,
    })
  }

  // The receipts section: the dereferenceable receipt-style refs (digest +
  // verify refs that act as the session's replay receipts). Refs only — the
  // browser never fetches or renders the receipt body.
  if (detail) {
    const receiptRows: Array<ArtifactBrowserRow> = []
    if (detail.responseDigestRef) receiptRows.push({ label: "response digest", value: detail.responseDigestRef })
    if (detail.objectiveDigestRef) receiptRows.push({ label: "objective digest", value: detail.objectiveDigestRef })
    if (detail.errorDigestRef) receiptRows.push({ label: "error digest", value: detail.errorDigestRef })
    if (detail.verifyRef) receiptRows.push({ label: "verify ref", value: detail.verifyRef })
    if (receiptRows.length > 0) {
      sections.push({ id: "receipts", title: "Receipt refs", rows: receiptRows })
    }
  }

  return sections
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
  workspaceRef?: string | null | undefined
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
    parentRef?: string | null | undefined
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

// ── CS-A3 (#5363): diff fidelity — structured diff from the event tail ────────
//
// The composer's coding turns surface file edits as bounded, public-safe
// composer-event messages in the session event tail (the node never streams raw
// patch bodies to a remote client). This derives a structured ChangeSet from
// that tail so the desktop can render the SHARED `DiffReview` component (the UI
// port of apps/pylon/src/tas/diff-review.ts) instead of a flat transcript row.
//
// This is NOT user-intent routing or retrieval — it parses already-selected
// session events for bounded fields (a public-safe file ref, a +/- count, a
// status keyword), which the routing rule explicitly allows after the
// program/event has been selected. It recognizes the message shapes the real
// codex/claude composers and the local Apple FM session emit:
//   - "edited <ref> (+N −M)"          (explicit per-file counts; − or -)
//   - "<status>: <kind> <ref>, …"     (codex file_change summaries)
//   - "<kind> <ref>"                  (claude tool edits / bare file lines)
// Each recognized file becomes a DiffReviewFile; unrecognized rows are ignored
// so the diff view never invents changes the node did not report.

import type {
  DiffFileStatus,
  DiffReviewFile,
  DiffReviewSummary,
} from "@openagentsinc/autopilot-ui"

export type DesktopChangeSet = {
  files: DiffReviewFile[]
  summary: DiffReviewSummary
  // How many events were inspected vs. recognized, for honest provenance.
  parsedFromEventCount: number
}

// Map a status/kind keyword onto the three diff-review file statuses.
function diffStatusFromKeyword(keyword: string): DiffFileStatus | null {
  const k = keyword.toLowerCase()
  if (k === "add" || k === "added" || k === "create" || k === "created" || k === "new") {
    return "added"
  }
  if (
    k === "delete" ||
    k === "deleted" ||
    k === "remove" ||
    k === "removed" ||
    k === "rm"
  ) {
    return "deleted"
  }
  if (
    k === "edit" ||
    k === "edited" ||
    k === "update" ||
    k === "updated" ||
    k === "modify" ||
    k === "modified" ||
    k === "change" ||
    k === "changed"
  ) {
    return "modified"
  }
  return null
}

// A public-safe file ref is a relative file-ish token. Raw absolute local paths,
// parent-directory escapes, Windows paths, and URL/provider payload fragments are
// rejected before they can enter the Diff/Artifacts UI.
export function publicSafeFileRef(token: string): string | null {
  const t = token.replace(/[.,;:)]+$/, "")
  if (t.length === 0 || /\s/.test(t)) return null
  if (
    t.startsWith("/") ||
    t.startsWith("~/") ||
    t.startsWith("../") ||
    t.includes("\\") ||
    t.includes("://") ||
    /^[A-Za-z]:[\\/]/.test(t)
  ) {
    return null
  }
  return t.includes("/") || /^[\w.@-]+\.[\w.@-]+$/.test(t) ? t : null
}

const COUNT_PATTERN = /\(\s*\+?\s*(\d+)\s*[−-]\s*(\d+)\s*\)/

// Parse a single composer-event message into zero or more change entries.
function parseDiffMessage(message: string): Array<{
  path: string
  status: DiffFileStatus
  added: number
  removed: number
}> {
  const text = message.trim()
  if (text.length === 0) return []

  // Strip a leading "<status>: " prefix (codex "completed: …" / "failed: …").
  const afterStatus = text.includes(": ") ? text.slice(text.indexOf(": ") + 2) : text

  // Per-file +/- counts: "edited src/x.ts (+12 −0)" or "src/x.ts (+1 -2)".
  const countMatch = afterStatus.match(COUNT_PATTERN)
  if (countMatch) {
    const before = afterStatus.slice(0, countMatch.index).trim()
    const tokens = before.split(/\s+/)
    // Last path-ish token is the file; an earlier token may be the kind.
    let path: string | null = null
    let status: DiffFileStatus = "modified"
    for (let i = tokens.length - 1; i >= 0; i--) {
      const cleaned = tokens[i].replace(/[.,;:)]+$/, "")
      const publicRef = publicSafeFileRef(cleaned)
      if (path === null && publicRef !== null) {
        path = publicRef
        continue
      }
      if (path !== null) {
        const kind = diffStatusFromKeyword(cleaned)
        if (kind !== null) {
          status = kind
          break
        }
      }
    }
    if (path !== null) {
      return [
        {
          path,
          status,
          added: Number(countMatch[1]),
          removed: Number(countMatch[2]),
        },
      ]
    }
  }

  // Codex file_change summaries: "update src/x.ts, add tests/x.test.ts".
  // Comma-separated "<kind> <ref>" pairs.
  const entries: Array<{ path: string; status: DiffFileStatus; added: number; removed: number }> = []
  for (const part of afterStatus.split(",")) {
    const tokens = part.trim().split(/\s+/)
    if (tokens.length < 2) continue
    const kind = diffStatusFromKeyword(tokens[0])
    const ref = publicSafeFileRef(tokens[1])
    if (kind !== null && ref !== null) {
      entries.push({ path: ref, status: kind, added: 0, removed: 0 })
    }
  }
  return entries
}

// Build a structured ChangeSet from a session's event tail. Later events for the
// same path win on counts (the latest reported +/- for a file), and the status
// escalates sensibly (a file first added then edited stays "added").
export function parseChangeSetFromEvents(
  events: ReadonlyArray<{ detail: string; full?: string }> | undefined,
): DesktopChangeSet {
  const byPath = new Map<string, DiffReviewFile>()
  let parsedFromEventCount = 0
  for (const event of events ?? []) {
    const sources =
      event.full && event.full.trim().length > 0
        ? [event.full, event.detail]
        : [event.detail]
    const parsed = sources
      .map((source) => parseDiffMessage(source))
      .find((entries) => entries.length > 0) ?? []
    if (parsed.length > 0) parsedFromEventCount += 1
    for (const entry of parsed) {
      const prior = byPath.get(entry.path)
      if (prior === undefined) {
        byPath.set(entry.path, {
          path: entry.path,
          status: entry.status,
          added: entry.added,
          removed: entry.removed,
        })
        continue
      }
      // Keep the strongest status: added beats modified; deleted is terminal.
      const status: DiffFileStatus =
        prior.status === "deleted" || entry.status === "deleted"
          ? "deleted"
          : prior.status === "added" || entry.status === "added"
            ? "added"
            : "modified"
      byPath.set(entry.path, {
        path: entry.path,
        status,
        // Prefer a non-zero count; later non-zero counts win.
        added: entry.added > 0 ? entry.added : prior.added,
        removed: entry.removed > 0 ? entry.removed : prior.removed,
      })
    }
  }
  const files = [...byPath.values()].sort((a, b) => a.path.localeCompare(b.path))
  const summary: DiffReviewSummary = files.reduce(
    (acc, f) => ({
      fileCount: acc.fileCount + 1,
      totalAdded: acc.totalAdded + f.added,
      totalRemoved: acc.totalRemoved + f.removed,
    }),
    { fileCount: 0, totalAdded: 0, totalRemoved: 0 },
  )
  return { files, summary, parsedFromEventCount }
}

// Honest provenance line for the diff view: the counts/files came from the
// bounded event tail, optionally corroborated by the artifact's editedFileCount.
export function diffReviewProvenance(
  changeSet: DesktopChangeSet,
  artifactEditedFileCount: number | null | undefined,
): string {
  const base = `derived from ${changeSet.parsedFromEventCount} session event${
    changeSet.parsedFromEventCount === 1 ? "" : "s"
  }`
  if (
    artifactEditedFileCount != null &&
    artifactEditedFileCount !== changeSet.summary.fileCount
  ) {
    return `${base} · artifact reports ${artifactEditedFileCount} edited file${
      artifactEditedFileCount === 1 ? "" : "s"
    }`
  }
  return base
}
