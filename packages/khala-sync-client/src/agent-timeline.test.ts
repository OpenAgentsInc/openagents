import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncVersion,
  SyncVersionWatermark,
  agentRunScope,
  canonicalJson,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  encodeAgentRunEntity,
  encodeAgentRunEventEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import { createKhalaSyncAgentTimeline } from "./agent-timeline.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const NOW = "2026-07-10T20:00:00.000Z"
const RUN = "run.timeline.1"
const scope = agentRunScope(RUN)
const roots: Array<string> = []

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const entry = (
  version: number,
  entityType: string,
  entityId: string,
  postImageJson: string,
) => new ChangelogEntry({
  scope,
  version: SyncVersion.make(version),
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImageJson,
  mutationRef: `mutation.timeline.${version}`,
  committedAt: NOW,
})

const runEntry = entry(
  1,
  AGENT_RUN_ENTITY_TYPE,
  RUN,
  canonicalJson(encodeAgentRunEntity(decodeAgentRunEntity({
    runId: RUN,
    routeId: "thread.timeline.1",
    userId: "owner.private",
    teamId: null,
    projectId: null,
    runtime: "codex",
    backend: "shc_vm",
    status: "running",
    goalId: null,
    goal: "private objective omitted by the client projection",
    repository: { provider: "github", owner: "private-owner", repo: "private-repo", ref: "main" },
    createdAt: NOW,
    updatedAt: NOW,
    startedAt: NOW,
    completedAt: null,
    failedAt: null,
    canceledAt: null,
  }))),
)

const eventEntry = (input: Readonly<{
  version: number
  id: string
  sequence: number
  summary: string
}>) => entry(
  input.version,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  input.id,
  canonicalJson(encodeAgentRunEventEntity(decodeAgentRunEventEntity({
    id: input.id,
    runId: RUN,
    sequence: input.sequence,
    type: "runtime.activity",
    summary: input.summary,
    status: "running",
    source: "provider-private-source",
    payloadJson: JSON.stringify({ rawProviderCallback: "must-not-project" }),
    artifactRefs: [`artifact.${input.sequence}`],
    externalEventId: `external-private-${input.sequence}`,
    createdAt: NOW,
  }))),
)

const session = (
  phase: ScopeSyncState = { phase: "live", cursor: SyncVersionWatermark.make(4) },
  opened: Array<string> = [],
): KhalaSyncSession => ({
  state: () => phase,
  pending: () => [{ mutationId: 1 }],
  subscribe: (requested: SyncScope) => Effect.sync(() => { opened.push(String(requested)) }),
}) as unknown as KhalaSyncSession

describe("contract khala_sync.client.confirmed_agent_timeline.v1", () => {
  test("reconstructs ordered confirmed state after duplicate/out-of-order replay and restart", () => {
    const root = mkdtempSync(join(tmpdir(), "khala-agent-timeline-"))
    roots.push(root)
    const database = join(root, "timeline.sqlite")
    const initial = openKhalaSyncStore(database)
    const replay = [
      runEntry,
      eventEntry({ version: 2, id: "event.3", sequence: 3, summary: "Third" }),
      eventEntry({ version: 3, id: "event.1", sequence: 1, summary: "First" }),
      eventEntry({ version: 4, id: "event.2", sequence: 2, summary: "Second" }),
    ]
    Effect.runSync(initial.applyConfirmed(scope, replay, SyncVersion.make(4)))
    Effect.runSync(initial.applyConfirmed(scope, replay, SyncVersion.make(4)))
    Effect.runSync(initial.close())

    const restarted = openKhalaSyncStore(database)
    try {
      const opened: Array<string> = []
      const timeline = createKhalaSyncAgentTimeline({ store: restarted, session: session(undefined, opened) })
      Effect.runSync(timeline.open(RUN))
      const snapshot = Effect.runSync(timeline.snapshot(RUN))

      expect(opened).toEqual(["scope.agent_run.run.timeline.1"])
      expect(snapshot.status).toEqual({ phase: "live", cursor: 4, pendingMutationCount: 1 })
      expect(snapshot.run).toEqual({
        runRef: RUN,
        routeRef: "thread.timeline.1",
        status: "running",
        createdAt: NOW,
        updatedAt: NOW,
        startedAt: NOW,
        completedAt: null,
        failedAt: null,
        canceledAt: null,
        version: 1,
      })
      expect(snapshot.events.map(event => [event.eventRef, event.sequence, event.version])).toEqual([
        ["event.1", 1, 3],
        ["event.2", 2, 4],
        ["event.3", 3, 2],
      ])
      const serialized = JSON.stringify(snapshot)
      expect(serialized).not.toContain("owner.private")
      expect(serialized).not.toContain("private objective")
      expect(serialized).not.toContain("provider-private-source")
      expect(serialized).not.toContain("rawProviderCallback")
      expect(serialized).not.toContain("external-private")
    } finally {
      Effect.runSync(restarted.close())
    }
  })

  test("hides cached rows unless the exact run scope is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [runEntry], SyncVersion.make(1)))
      const timeline = createKhalaSyncAgentTimeline({
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
      })
      expect(Effect.runSync(timeline.snapshot(RUN))).toEqual({
        status: { phase: "must_refetch", cursor: null, pendingMutationCount: 1 },
        run: null,
        events: [],
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("bounds the public timeline to the newest 500 ordered events", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const events = Array.from({ length: 505 }, (_, index) =>
        eventEntry({
          version: index + 2,
          id: `event.${String(index + 1).padStart(3, "0")}`,
          sequence: index + 1,
          summary: `Event ${index + 1}`,
        }))
      Effect.runSync(store.applyConfirmed(scope, [runEntry, ...events], SyncVersion.make(506)))
      const timeline = createKhalaSyncAgentTimeline({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(506) }),
      })
      const snapshot = Effect.runSync(timeline.snapshot(RUN))
      expect(snapshot.events).toHaveLength(500)
      expect(snapshot.events[0]?.sequence).toBe(6)
      expect(snapshot.events.at(-1)?.sequence).toBe(505)
    } finally {
      Effect.runSync(store.close())
    }
  })
})
