import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import type { NodeStateMessage, SessionEventRow } from "../shared/rpc.js"

// CS-A3 (#5363): durable transcript / session-record persistence (desktop side).
//
// Why local persistence (and not a new control verb): the Pylon control server
// only exposes `session.events`, whose `recentEvents` is the node's IN-MEMORY
// event log (bounded to the last ~100 events). That log does NOT survive a node
// restart — after the user restarts `pylon dev` (or the packaged node relaunches
// for an update), `session.list` no longer carries the old session and
// `session.events` returns nothing for it. The node-side durable record
// (apps/pylon/src/session-record-store.ts) is not wired to a control read verb.
//
// Per the CS-A3 brief ("if the node exposes a session-record/transcript read,
// use it; otherwise persist the polled event tail locally keyed by sessionRef
// and document the approach"), and the no-new-wire-verb constraint, the desktop
// persists the per-session event tail it polls, keyed by sessionRef, under the
// node home. On every poll we (1) merge the newly polled events into the on-disk
// transcript (dedup by eventIndex, append-only), and (2) merge the persisted
// transcript BACK into the live NodeStateMessage so a coding session's
// transcript reloads after an app restart / node restart / reconnection — even
// for sessions that have aged out of the node's in-memory tail or dropped off
// `session.list`. Public-safe by construction: only the already-public-safe,
// redaction-scanned event rows the node emits are stored; no tokens, prompts,
// or raw paths beyond what the projection already carries.

const TRANSCRIPT_DIR = "desktop-transcripts"
const SCHEMA = "openagents.autopilot_desktop.session_transcript.v1"
// Bound the stored tail per session so the store never grows without limit.
const MAX_EVENTS_PER_SESSION = 2000
// Bound how many persisted-but-no-longer-listed sessions we re-surface, newest
// first, so a long-lived node home does not inflate every poll's projection.
const MAX_REHYDRATED_SESSIONS = 50

export type SessionTranscriptRecord = {
  schema: typeof SCHEMA
  sessionRef: string
  updatedAt: string
  events: SessionEventRow[]
}

function transcriptDir(nodeHome: string): string {
  return join(nodeHome, TRANSCRIPT_DIR)
}

// sessionRefs are stable public-safe refs (e.g. "session.pylon.control.<hash>").
// Sanitize for a filename so an unexpected ref can never escape the directory.
function transcriptPath(nodeHome: string, sessionRef: string): string {
  const safe = sessionRef.replace(/[^a-zA-Z0-9._-]/g, "-").slice(0, 200)
  return join(transcriptDir(nodeHome), `${safe}.json`)
}

// Merge two event tails into one ordered, deduped-by-eventIndex list. Within a
// single session, eventIndex is the node's monotonic per-session sequence, so it
// is the natural dedup + ordering key. A later poll's row for the same index
// wins (it carries any newly revealed `full` content).
export function mergeEventRows(
  existing: ReadonlyArray<SessionEventRow>,
  incoming: ReadonlyArray<SessionEventRow>,
): SessionEventRow[] {
  const byIndex = new Map<number, SessionEventRow>()
  for (const row of existing) byIndex.set(row.eventIndex, row)
  for (const row of incoming) byIndex.set(row.eventIndex, row)
  const merged = [...byIndex.values()].sort((a, b) => a.eventIndex - b.eventIndex)
  return merged.length > MAX_EVENTS_PER_SESSION
    ? merged.slice(merged.length - MAX_EVENTS_PER_SESSION)
    : merged
}

export function loadSessionTranscript(
  nodeHome: string,
  sessionRef: string,
): SessionTranscriptRecord | null {
  try {
    const raw = readFileSync(transcriptPath(nodeHome, sessionRef), "utf8")
    const parsed = JSON.parse(raw) as Partial<SessionTranscriptRecord>
    if (parsed.sessionRef !== sessionRef || !Array.isArray(parsed.events)) return null
    return {
      schema: SCHEMA,
      sessionRef,
      updatedAt: typeof parsed.updatedAt === "string" ? parsed.updatedAt : "",
      events: parsed.events as SessionEventRow[],
    }
  } catch {
    return null
  }
}

export function saveSessionTranscript(
  nodeHome: string,
  sessionRef: string,
  events: ReadonlyArray<SessionEventRow>,
): void {
  try {
    mkdirSync(transcriptDir(nodeHome), { recursive: true })
    const record: SessionTranscriptRecord = {
      schema: SCHEMA,
      sessionRef,
      updatedAt: new Date().toISOString(),
      events: [...events],
    }
    writeFileSync(transcriptPath(nodeHome, sessionRef), `${JSON.stringify(record)}\n`, "utf8")
  } catch {
    // Persistence is best-effort: a write failure must never break the live poll.
  }
}

// List the sessionRefs that have a persisted transcript on disk.
export function listPersistedSessionRefs(nodeHome: string): string[] {
  try {
    return readdirSync(transcriptDir(nodeHome))
      .filter((name) => name.endsWith(".json"))
      .map((name) => name.slice(0, -".json".length))
  } catch {
    return []
  }
}

// The poll-time merge: given a live NodeStateMessage, (1) persist each listed
// session's polled event tail into its on-disk transcript, and (2) hydrate the
// message's `events` map with the persisted (possibly longer / restart-spanning)
// transcript so the webview always sees the durable history. Sessions that are
// persisted but no longer listed are re-surfaced as cancelled "history" rows so
// their transcript still reloads after a node restart drops them from
// `session.list`. Pure-ish: side effects are bounded to the transcript dir.
export function persistAndMergeTranscripts(
  nodeHome: string | null,
  message: NodeStateMessage,
): NodeStateMessage {
  if (nodeHome === null) return message

  const liveEvents = message.events ?? {}
  const mergedEvents: Record<string, SessionEventRow[]> = {}

  // 1) For each listed session, merge persisted + polled, persist, and surface.
  for (const session of message.sessions) {
    const ref = session.sessionRef
    const polled = liveEvents[ref] ?? []
    const persisted = loadSessionTranscript(nodeHome, ref)
    const merged = mergeEventRows(persisted?.events ?? [], polled)
    if (merged.length > 0) {
      mergedEvents[ref] = merged
      // Only rewrite when the poll actually added/changed something.
      if (polled.length > 0) saveSessionTranscript(nodeHome, ref, merged)
    } else if (polled.length > 0) {
      mergedEvents[ref] = [...polled]
    }
  }

  // 2) Re-surface persisted sessions the node no longer lists (restart/aged out)
  //    as history rows, so their transcript still reloads. Bounded + newest-first.
  const listedRefs = new Set(message.sessions.map((s) => s.sessionRef))
  // Keep live event-only refs, such as proof-linked external Codex/Claude
  // aliases. They are not session.list rows, but reducers may need them.
  for (const [ref, events] of Object.entries(liveEvents)) {
    if (!listedRefs.has(ref) && mergedEvents[ref] === undefined) {
      mergedEvents[ref] = [...events]
    }
  }

  const orphanRecords: SessionTranscriptRecord[] = []
  for (const ref of listPersistedSessionRefs(nodeHome)) {
    if (listedRefs.has(ref)) continue
    const record = loadSessionTranscript(nodeHome, ref)
    if (record && record.events.length > 0) orphanRecords.push(record)
  }
  orphanRecords.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))

  const historySessions = orphanRecords
    .slice(0, MAX_REHYDRATED_SESSIONS)
    .map((record) => {
      mergedEvents[record.sessionRef] = record.events
      return historySessionSummary(record)
    })

  return {
    ...message,
    sessions: [...message.sessions, ...historySessions],
    events: mergedEvents,
  }
}

// Build a minimal, public-safe SessionSummary for a persisted-but-unlisted
// session. The node is not reporting it anymore, so its terminal state is
// reconstructed from the last persisted event's state (falling back to a neutral
// terminal marker). Marked with agentKind "history" so the UI can badge it.
function historySessionSummary(
  record: SessionTranscriptRecord,
): NodeStateMessage["sessions"][number] {
  const last = record.events[record.events.length - 1]
  const lastState = last?.state
  const state =
    lastState === "completed" ||
    lastState === "failed" ||
    lastState === "cancelled" ||
    lastState === "running" ||
    lastState === "queued"
      ? lastState
      : "completed"
  const latest = [...record.events]
    .reverse()
    .find((e) => e.detail.trim().length > 0)
  return {
    sessionRef: record.sessionRef,
    adapter: "codex",
    state,
    accountRefHash: null,
    agentKind: "history",
    updatedAt: record.updatedAt,
    ...(latest ? { latestActivity: latest.detail.slice(0, 160) } : {}),
  }
}
