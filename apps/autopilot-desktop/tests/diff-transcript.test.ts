// CS-A3 (#5363): diff-fidelity + transcript-persistence tests.
//
// Covers the two pure/near-pure pieces of CS-A3 without a DOM or a live node:
//   1. parseChangeSetFromEvents / diffReviewProvenance — derive a structured
//      ChangeSet (the DiffReview UI port of apps/pylon/src/tas/diff-review.ts)
//      from the bounded composer event tail the node emits.
//   2. the transcript store — persist a session's polled event tail keyed by
//      sessionRef and merge it back so the transcript survives a node restart.

import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, beforeEach, describe, expect, test } from "bun:test"

import {
  diffReviewProvenance,
  parseChangeSetFromEvents,
} from "../src/ui/helpers"
import {
  loadSessionTranscript,
  mergeEventRows,
  persistAndMergeTranscripts,
  saveSessionTranscript,
} from "../src/bun/transcript-store"
import type { NodeStateMessage, SessionEventRow } from "../src/shared/rpc"

const ev = (over: Partial<SessionEventRow> = {}): SessionEventRow => ({
  eventIndex: over.eventIndex ?? 0,
  phase: over.phase ?? "progress",
  state: over.state ?? "running",
  observedAt: over.observedAt ?? "2026-06-18T12:00:00.000Z",
  detail: over.detail ?? "",
  ...(over.full !== undefined ? { full: over.full } : {}),
})

describe("parseChangeSetFromEvents (#5363)", () => {
  test("explicit +/- counts: 'edited <ref> (+12 −0)'", () => {
    const cs = parseChangeSetFromEvents([
      ev({ eventIndex: 1, detail: "edited src/health.ts (+12 −0)" }),
    ])
    expect(cs.files).toHaveLength(1)
    expect(cs.files[0]).toMatchObject({
      path: "src/health.ts",
      status: "modified",
      added: 12,
      removed: 0,
    })
    expect(cs.summary).toEqual({ fileCount: 1, totalAdded: 12, totalRemoved: 0 })
  })

  test("ascii dash and 'added' kind: 'added src/new.ts (+5 -1)'", () => {
    const cs = parseChangeSetFromEvents([
      ev({ detail: "added src/new.ts (+5 -1)" }),
    ])
    expect(cs.files[0]).toMatchObject({ path: "src/new.ts", status: "added", added: 5, removed: 1 })
  })

  test("codex file_change summary: '<status>: update a.ts, add b/c.ts'", () => {
    const cs = parseChangeSetFromEvents([
      ev({ detail: "completed: update src/a.ts, add tests/b.test.ts" }),
    ])
    const byPath = Object.fromEntries(cs.files.map((f) => [f.path, f]))
    expect(byPath["src/a.ts"]).toMatchObject({ status: "modified" })
    expect(byPath["tests/b.test.ts"]).toMatchObject({ status: "added" })
    expect(cs.summary.fileCount).toBe(2)
  })

  test("deleted wins as terminal status; later non-zero counts win", () => {
    const cs = parseChangeSetFromEvents([
      ev({ eventIndex: 1, detail: "edited src/x.ts (+1 −0)" }),
      ev({ eventIndex: 2, detail: "edited src/x.ts (+10 −3)" }),
      ev({ eventIndex: 3, detail: "completed: delete src/x.ts" }),
    ])
    expect(cs.files).toHaveLength(1)
    expect(cs.files[0]).toMatchObject({ path: "src/x.ts", status: "deleted", added: 10, removed: 3 })
  })

  test("prose / lifecycle rows produce no phantom files", () => {
    const cs = parseChangeSetFromEvents([
      ev({ detail: "thinking: I will add a route" }),
      ev({ detail: "agent: done" }),
      ev({ phase: "started", detail: "" }),
    ])
    expect(cs.files).toHaveLength(0)
    expect(cs.parsedFromEventCount).toBe(0)
  })

  test("uses full content when present over the truncated detail", () => {
    const cs = parseChangeSetFromEvents([
      ev({ detail: "edited src/x.t…", full: "edited src/x.ts (+3 −1)" }),
    ])
    expect(cs.files[0]).toMatchObject({ path: "src/x.ts", added: 3, removed: 1 })
  })

  test("provenance reflects event count and artifact mismatch", () => {
    const cs = parseChangeSetFromEvents([ev({ detail: "edited a.ts (+1 −0)" })])
    expect(diffReviewProvenance(cs, 1)).toBe("derived from 1 session event")
    expect(diffReviewProvenance(cs, 3)).toContain("artifact reports 3 edited files")
  })
})

describe("transcript store: mergeEventRows (#5363)", () => {
  test("dedups by eventIndex and orders ascending; later row wins", () => {
    const merged = mergeEventRows(
      [ev({ eventIndex: 0, detail: "a" }), ev({ eventIndex: 1, detail: "b" })],
      [ev({ eventIndex: 1, detail: "b2", full: "full b2" }), ev({ eventIndex: 2, detail: "c" })],
    )
    expect(merged.map((m) => m.eventIndex)).toEqual([0, 1, 2])
    expect(merged[1].detail).toBe("b2")
    expect(merged[1].full).toBe("full b2")
  })
})

describe("transcript store: persistence + restart merge (#5363)", () => {
  let home: string
  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), "cs-a3-transcript-"))
  })
  afterEach(() => {
    rmSync(home, { recursive: true, force: true })
  })

  const liveMessage = (events: SessionEventRow[]): NodeStateMessage => ({
    ok: true,
    schema: "openagents.pylon.control.v0.3",
    sessions: [
      {
        sessionRef: "session.pylon.control.aaa",
        adapter: "codex",
        state: "running",
        accountRefHash: null,
        updatedAt: "2026-06-18T12:00:00.000Z",
      },
    ],
    events: { "session.pylon.control.aaa": events },
  })

  test("save + load round-trips a transcript keyed by sessionRef", () => {
    saveSessionTranscript(home, "session.pylon.control.aaa", [ev({ eventIndex: 0, detail: "edited a.ts (+1 −0)" })])
    const loaded = loadSessionTranscript(home, "session.pylon.control.aaa")
    expect(loaded?.events).toHaveLength(1)
    expect(loaded?.events[0].detail).toBe("edited a.ts (+1 −0)")
  })

  test("poll persists the tail and a later poll merges new events", () => {
    const first = persistAndMergeTranscripts(home, liveMessage([ev({ eventIndex: 0, detail: "started" })]))
    expect(first.events?.["session.pylon.control.aaa"]).toHaveLength(1)
    const second = persistAndMergeTranscripts(
      home,
      liveMessage([ev({ eventIndex: 1, detail: "edited a.ts (+2 −0)" })]),
    )
    // The persisted event 0 is merged back in alongside the newly polled event 1.
    const merged = second.events?.["session.pylon.control.aaa"] ?? []
    expect(merged.map((m) => m.eventIndex)).toEqual([0, 1])
  })

  test("poll preserves live event-only refs such as proof external-session aliases", () => {
    const externalRef = "session.pylon.codex_composer.be4d2b8c1eb3512e70bf59be"
    const merged = persistAndMergeTranscripts(home, {
      ...liveMessage([ev({ eventIndex: 0, detail: "redaction blocked", phase: "redaction_blocked" })]),
      events: {
        "session.pylon.control.aaa": [
          ev({ eventIndex: 0, detail: "", phase: "redaction_blocked" }),
        ],
        [externalRef]: [
          ev({
            eventIndex: 3,
            phase: "agent_message",
            state: "completed",
            detail: "agent: I am Codex.",
          }),
        ],
      },
    })

    expect(merged.events?.[externalRef]?.[0]?.detail).toBe("agent: I am Codex.")
  })

  test("transcript reloads after a node restart drops the session from session.list", () => {
    // 1) A session runs and its transcript is persisted.
    persistAndMergeTranscripts(
      home,
      liveMessage([
        ev({ eventIndex: 0, detail: "started" }),
        ev({ eventIndex: 1, detail: "edited a.ts (+4 −1)", state: "completed", phase: "completed" }),
      ]),
    )
    // 2) The node restarts: session.list is now empty, in-memory tail is gone.
    const afterRestart: NodeStateMessage = {
      ok: true,
      schema: "openagents.pylon.control.v0.3",
      sessions: [],
      events: {},
    }
    const merged = persistAndMergeTranscripts(home, afterRestart)
    // The session is re-surfaced as a history row with its full transcript.
    const history = merged.sessions.find((s) => s.sessionRef === "session.pylon.control.aaa")
    expect(history).toBeDefined()
    expect(history?.agentKind).toBe("history")
    expect(history?.state).toBe("completed")
    const events = merged.events?.["session.pylon.control.aaa"] ?? []
    expect(events).toHaveLength(2)
    // And the diff still parses from the reloaded transcript.
    const cs = parseChangeSetFromEvents(events)
    expect(cs.files[0]).toMatchObject({ path: "a.ts", added: 4, removed: 1 })
  })

  test("null node home is a no-op pass-through", () => {
    const msg = liveMessage([ev({ detail: "x" })])
    expect(persistAndMergeTranscripts(null, msg)).toBe(msg)
  })
})
