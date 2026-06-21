// #5469 (EPIC #5461): swarm batch launch + account-failover/routing visibility
// + sub-agent nesting tree.
//
// This is the SIBLING pure module for the swarm pane (view.ts `swarmPane`,
// helpers.ts `orderSwarmSessions`/`swarm*`). It holds the DOM-free, runtime-free
// derivations the three #5469 additions need so they stay unit-testable without
// a DOM, a node, or any new wire verb:
//
//   1. Batch launch  — a bounded concurrent spawner DRIVEN BY THE REDUCER over
//      the EXISTING `session.spawn` verb. The desktop is the orchestrator: it
//      keeps a queue of objectives and never lets more than `concurrency`
//      sessions be in-flight at once (the audit's "visible concurrency cap").
//      No `sessions batch` wire verb is invented — that CLI path runs in-node;
//      here we reuse the one spawn verb the control protocol already exposes.
//   2. Failover routing — a refs-only per-session routing line derived from the
//      session's bounded event tail. The control-protocol `SessionSummary`
//      carries only the ACTIVE `accountRefHash`; the failover *reasons*
//      (`succeeded|quota_block|skipped_unavailable|failed`) are emitted by the
//      node-side multi-session runner. When a node surfaces a routing reason in
//      a session's event detail, this recognizes the bounded keyword and shows
//      it. This is NOT user-intent routing or retrieval — it reads
//      already-selected session events for bounded enum keywords, which the
//      workspace routing rule allows after the program/event is selected.
//   3. Sub-agent tree — `orderSwarmSessions` already keeps `parentRef` children
//      adjacent; `buildSwarmTree` turns that adjacency into an explicit
//      (session, depth, childCount) list so the grid can render real tree
//      nesting (indent + child-count badge) instead of a flat wall.

// ── 1. Batch launch: bounded-concurrency queue (pure) ────────────────────────

// The hard upper bound on how many sessions a single batch may launch at once.
// Keeps the "visible concurrency cap" honest and prevents a typo'd value from
// fanning out an unbounded swarm.
export const SWARM_BATCH_MAX_CONCURRENCY = 8
export const SWARM_BATCH_DEFAULT_CONCURRENCY = 3
// A bound on the size of a single batch so the textarea cannot launch a
// runaway number of sessions from one click.
export const SWARM_BATCH_MAX_OBJECTIVES = 24

// Parse the batch textarea into a bounded, de-duplicated objective list. One
// objective per line; blank lines and pure-whitespace lines are dropped;
// trimmed; capped at SWARM_BATCH_MAX_OBJECTIVES. Pure so the reducer + tests
// share the exact same parsing.
export function parseSwarmBatchObjectives(raw: string): ReadonlyArray<string> {
  const seen = new Set<string>()
  const out: string[] = []
  for (const line of raw.split("\n")) {
    const objective = line.trim()
    if (objective === "") continue
    if (seen.has(objective)) continue
    seen.add(objective)
    out.push(objective)
    if (out.length >= SWARM_BATCH_MAX_OBJECTIVES) break
  }
  return out
}

// Clamp a requested concurrency into [1, SWARM_BATCH_MAX_CONCURRENCY]. NaN /
// non-positive / over-cap values collapse to a safe bound.
export function clampSwarmBatchConcurrency(value: number): number {
  if (!Number.isFinite(value)) return SWARM_BATCH_DEFAULT_CONCURRENCY
  const floored = Math.floor(value)
  if (floored < 1) return 1
  if (floored > SWARM_BATCH_MAX_CONCURRENCY) return SWARM_BATCH_MAX_CONCURRENCY
  return floored
}

// The bounded batch-launch state the reducer threads. `queue` is the objectives
// not yet dispatched; `active` is how many spawns are currently in flight;
// `concurrency` is the cap; `launched`/`failed` are running counters for the
// honest status line. `total` is the batch size at launch time (for progress).
export type SwarmBatchState = Readonly<{
  queue: ReadonlyArray<string>
  active: number
  concurrency: number
  launched: number
  failed: number
  total: number
}>

export const emptySwarmBatchState: SwarmBatchState = {
  queue: [],
  active: 0,
  concurrency: SWARM_BATCH_DEFAULT_CONCURRENCY,
  launched: 0,
  failed: 0,
  total: 0,
}

// A batch is "running" while there is anything in flight OR anything still
// queued to dispatch.
export function swarmBatchRunning(state: SwarmBatchState): boolean {
  return state.active > 0 || state.queue.length > 0
}

// Given a freshly-parsed objective list and a concurrency cap, produce the
// initial batch state AND the first wave of objectives to dispatch (up to the
// cap). The reducer turns each dispatched objective into one SpawnSession
// command. Pure: (objectives, concurrency) → { state, toDispatch }.
export function startSwarmBatch(
  objectives: ReadonlyArray<string>,
  concurrency: number,
): { state: SwarmBatchState; toDispatch: ReadonlyArray<string> } {
  const cap = clampSwarmBatchConcurrency(concurrency)
  const total = objectives.length
  const toDispatch = objectives.slice(0, cap)
  const queue = objectives.slice(cap)
  return {
    state: {
      queue,
      active: toDispatch.length,
      concurrency: cap,
      launched: 0,
      failed: 0,
      total,
    },
    toDispatch,
  }
}

// One batch session has settled (spawn accepted or rejected). Decrement the
// in-flight count, bump the right counter, then pull the NEXT objective off the
// queue if room remains under the cap. Returns the new state and the objective
// to dispatch next (or null when the queue is drained / still saturated). This
// is the heart of the bounded concurrency: a new spawn only starts as an old
// one settles, so `active` never exceeds `concurrency`.
export function advanceSwarmBatch(
  state: SwarmBatchState,
  outcome: "launched" | "failed",
): { state: SwarmBatchState; next: string | null } {
  const active = Math.max(0, state.active - 1)
  const launched = state.launched + (outcome === "launched" ? 1 : 0)
  const failed = state.failed + (outcome === "failed" ? 1 : 0)
  // Pull the next objective only if there is one AND we are under the cap.
  if (state.queue.length > 0 && active < state.concurrency) {
    const [next, ...rest] = state.queue
    return {
      state: { ...state, queue: rest, active: active + 1, launched, failed },
      next,
    }
  }
  return { state: { ...state, queue: [], active, launched, failed }, next: null }
}

// A one-line, honest status for the batch header: how many launched/failed out
// of the total, how many still queued, and the active/cap. Pure presentational.
export function swarmBatchStatusLine(state: SwarmBatchState): string {
  if (state.total === 0) return ""
  const done = state.launched + state.failed
  const parts = [`${done}/${state.total} launched`]
  if (state.failed > 0) parts.push(`${state.failed} failed`)
  if (state.queue.length > 0) parts.push(`${state.queue.length} queued`)
  if (state.active > 0) parts.push(`${state.active}/${state.concurrency} in flight`)
  else if (state.queue.length === 0) parts.push("done")
  return parts.join(" · ")
}

// ── 2. Failover / routing visibility (pure, refs-only) ───────────────────────

// The bounded routing-reason vocabulary the node-side multi-session runner
// emits (apps/pylon/scripts/multi-session-run.ts `PylonRoutingReason`). The
// desktop only recognizes these exact tokens if a node surfaces them in a
// session's event detail — it never invents a routing reason the node did not
// report.
const ROUTING_REASONS = [
  "quota_block",
  "skipped_unavailable",
  "failed",
  "succeeded",
] as const
export type SwarmRoutingReason = (typeof ROUTING_REASONS)[number]

// A short, public-safe label for a routing reason (refs-only display).
export function swarmRoutingReasonLabel(reason: SwarmRoutingReason): {
  text: string
  toneClass: string
} {
  switch (reason) {
    case "quota_block":
      return { text: "quota block", toneClass: "swarm-route-block" }
    case "skipped_unavailable":
      return { text: "skipped (unavailable)", toneClass: "swarm-route-block" }
    case "failed":
      return { text: "route failed", toneClass: "swarm-route-failed" }
    case "succeeded":
      return { text: "routed", toneClass: "swarm-route-ok" }
  }
}

// Derive the per-cell failover/routing summary from a session's bounded event
// tail. We scan the (already-selected) event detail/phase text for the exact
// routing-reason tokens above. The LATEST recognized reason wins (a session
// that was quota-blocked then re-routed shows its current routing state). Any
// recognized reason OTHER than the final one is counted as a `failovers`
// signal so the cell can show "re-routed N×" without leaking account refs.
//
// This is deterministic enum matching over already-selected events (allowed
// post-selection by the workspace routing rule), NOT semantic intent routing.
export function swarmFailoverRouting(
  events: ReadonlyArray<{ phase: string; detail?: string; full?: string }> | undefined,
): { reason: SwarmRoutingReason | null; failovers: number } {
  if (!events || events.length === 0) return { reason: null, failovers: 0 }
  let reason: SwarmRoutingReason | null = null
  let failovers = 0
  for (const e of events) {
    const haystack = `${e.phase} ${e.detail ?? ""} ${e.full ?? ""}`.toLowerCase()
    let matched: SwarmRoutingReason | null = null
    for (const r of ROUTING_REASONS) {
      // Match the underscored token OR a spaced variant of it.
      if (haystack.includes(r) || haystack.includes(r.replace(/_/g, " "))) {
        matched = r
        break
      }
    }
    if (matched === null) continue
    // A block/skip/failed reason that is later superseded counts as a failover.
    if (reason !== null && reason !== "succeeded") failovers += 1
    reason = matched
  }
  return { reason, failovers }
}

// ── 3. Sub-agent nesting tree (pure) ─────────────────────────────────────────

// A node in the swarm tree: the session, its depth from a root (0 = root), and
// how many direct children it has (so a parent can show a child-count badge).
export type SwarmTreeNode<T> = Readonly<{
  session: T
  depth: number
  childCount: number
}>

// Turn an ALREADY-ORDERED swarm session list (the output of
// `orderSwarmSessions`, where children sit adjacent to and after their parent)
// into an explicit depth-annotated tree, supporting arbitrary nesting depth
// (a sub-agent of a sub-agent). The grid renders each node with an indent
// proportional to `depth` and a child-count badge when `childCount > 0`, so the
// `parentRef` hierarchy reads as a tree without becoming a wall.
//
// `orderSwarmSessions` only nests one level (child adjacent to parent); this
// computes the FULL depth chain by walking `parentRef` up through the set, so a
// grandchild renders at depth 2 even though the ordering kept it adjacent.
export function buildSwarmTree<
  T extends { sessionRef: string; parentRef?: string | null | undefined },
>(ordered: ReadonlyArray<T>): ReadonlyArray<SwarmTreeNode<T>> {
  const bySessionRef = new Map<string, T>()
  for (const s of ordered) bySessionRef.set(s.sessionRef, s)

  // Direct-child counts, only counting children whose parent is in the set.
  const childCount = new Map<string, number>()
  for (const s of ordered) {
    const parent = s.parentRef ?? null
    if (parent !== null && parent !== s.sessionRef && bySessionRef.has(parent)) {
      childCount.set(parent, (childCount.get(parent) ?? 0) + 1)
    }
  }

  // Depth = length of the parentRef chain that stays inside the set. Guard
  // against cycles / self-parent with a visited set and a hard depth cap.
  const depthOf = (s: T): number => {
    let depth = 0
    const visited = new Set<string>([s.sessionRef])
    let current: T | undefined = s
    while (current) {
      const parentRef = current.parentRef ?? null
      if (
        parentRef === null ||
        parentRef === current.sessionRef ||
        !bySessionRef.has(parentRef) ||
        visited.has(parentRef)
      ) {
        break
      }
      visited.add(parentRef)
      current = bySessionRef.get(parentRef)
      depth += 1
      if (depth >= 16) break
    }
    return depth
  }

  return ordered.map((session) => ({
    session,
    depth: depthOf(session),
    childCount: childCount.get(session.sessionRef) ?? 0,
  }))
}
