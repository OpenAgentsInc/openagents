import { describe, expect, test } from "vite-plus/test"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncVersion,
  SyncVersionWatermark,
  canonicalJson,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  encodeAgentRunEntity,
  encodeAgentRunEventEntity,
  threadScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  createKhalaSyncAgentTimeline,
  type ConfirmedEntity,
  type KhalaSyncLocalStore,
  type KhalaSyncSession,
  type ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  openDesktopSyncStore,
  type DesktopSqliteDatabase,
} from "../src/desktop-sync-store.ts"

const NOW = "2026-07-11T16:00:00.000Z"
const THREAD = "thread.timeline.fault.8688"
const RUN = "run.timeline.fault.8688"
const scope = threadScope(THREAD)

const desktopDatabase = (path: string): DesktopSqliteDatabase => {
  const database = new NodeTestDatabase(path, { create: true })
  return {
    close: () => database.close(),
    exec: sql => database.exec(sql),
    prepare: sql => {
      const statement = database.query(sql)
      return {
        all: (...params) => statement.all(...params),
        run: (...params) => statement.run(...params),
      }
    },
  }
}

const mobileDatabase = (path: string): ExpoSqliteDatabase => {
  const database = new NodeTestDatabase(path, { create: true })
  return {
    closeSync: () => database.close(),
    execSync: sql => database.exec(sql),
    getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.query(sql).all(...params) as ReadonlyArray<Row>,
    runSync: (sql, ...params) => database.query(sql).run(...params),
    withTransactionSync: task => database.transaction(task)(),
  }
}

const entry = (
  version: number,
  entityType: string,
  entityId: string,
  postImageJson: string,
): ChangelogEntry => new ChangelogEntry({
  committedAt: NOW,
  entityId: EntityId.make(entityId),
  entityType: EntityType.make(entityType),
  mutationRef: `mutation.timeline.fault.${version}`,
  op: "upsert",
  postImageJson,
  scope,
  version: SyncVersion.make(version),
})

const runImage = (status: "running" | "completed" | "canceled") => canonicalJson(
  encodeAgentRunEntity(decodeAgentRunEntity({
    backend: "pylon",
    canceledAt: status === "canceled" ? NOW : null,
    completedAt: status === "completed" ? NOW : null,
    createdAt: NOW,
    failedAt: null,
    goal: "private objective",
    goalId: null,
    projectId: null,
    repository: {
      owner: "private-owner",
      provider: "github",
      ref: "main",
      repo: "private-repo",
    },
    routeId: THREAD,
    runId: RUN,
    runtime: "codex",
    startedAt: NOW,
    status,
    teamId: null,
    updatedAt: NOW,
    userId: "owner.private",
  })),
)

const lifecycleEventImage = (
  id: string,
  sequence: number,
  type: "turn.started" | "runtime.activity" | "turn.interrupted",
) => canonicalJson(
  encodeAgentRunEventEntity(decodeAgentRunEventEntity({
    artifactRefs: [],
    createdAt: NOW,
    externalEventId: null,
    id,
    payloadJson: null,
    runId: RUN,
    sequence,
    source: "canonical-runtime",
    status: type === "turn.interrupted" ? "interrupted" : "running",
    summary: type === "turn.interrupted" ? "Turn interrupted" : `Event ${sequence}`,
    type,
  })),
)

const eventImage = (id: string, sequence: number) => canonicalJson(
  encodeAgentRunEventEntity(decodeAgentRunEventEntity({
    artifactRefs: [],
    createdAt: NOW,
    externalEventId: null,
    id,
    payloadJson: null,
    runId: RUN,
    sequence,
    source: "canonical-runtime",
    status: sequence === 4 ? "completed" : "running",
    summary: `Event ${sequence}`,
    type: sequence === 4 ? "turn.finished" : "runtime.activity",
  })),
)

const confirmed = (
  version: number,
  entityType: string,
  entityId: string,
  postImageJson: string,
): ConfirmedEntity => ({
  entityId,
  entityType,
  postImageJson,
  version: SyncVersion.make(version),
})

const session = (cursor: () => number): KhalaSyncSession => ({
  pending: () => [],
  state: (_scope: SyncScope): ScopeSyncState => ({
    cursor: SyncVersionWatermark.make(cursor()),
    phase: "live",
  }),
}) as unknown as KhalaSyncSession

describe("CUT-08 native timeline fault corpus", () => {
  test("Desktop and mobile converge on reordered/duplicate events and the same gap snapshot", () => {
    const root = mkdtempSync(join(tmpdir(), "native-timeline-fault-"))
    const desktop = openDesktopSyncStore(
      join(root, "desktop.sqlite"),
      desktopDatabase,
    )
    const mobile = openExpoKhalaSyncStore(
      join(root, "mobile.sqlite"),
      mobileDatabase,
    )
    const stores: ReadonlyArray<KhalaSyncLocalStore> = [desktop, mobile]
    let cursor = 4
    try {
      const reordered = [
        entry(1, AGENT_RUN_ENTITY_TYPE, RUN, runImage("running")),
        entry(2, AGENT_RUN_EVENT_ENTITY_TYPE, "event.fault.3", eventImage("event.fault.3", 3)),
        entry(3, AGENT_RUN_EVENT_ENTITY_TYPE, "event.fault.1", eventImage("event.fault.1", 1)),
        entry(4, AGENT_RUN_EVENT_ENTITY_TYPE, "event.fault.2", eventImage("event.fault.2", 2)),
      ]
      for (const store of stores) {
        Effect.runSync(store.applyConfirmed(scope, reordered, SyncVersion.make(4)))
        Effect.runSync(store.applyConfirmed(scope, [...reordered].reverse(), SyncVersion.make(4)))
      }

      const before = stores.map(store => Effect.runSync(
        createKhalaSyncAgentTimeline({ store, session: session(() => cursor) })
          .snapshotForThread(THREAD),
      ))
      expect(before[0]).toEqual(before[1])
      expect(before[0]?.events.map(event => [event.eventRef, event.sequence, event.version]))
        .toEqual([
          ["event.fault.1", 1, 3],
          ["event.fault.2", 2, 4],
          ["event.fault.3", 3, 2],
        ])

      cursor = 8
      const replacement = [
        confirmed(8, AGENT_RUN_ENTITY_TYPE, RUN, runImage("completed")),
        confirmed(8, AGENT_RUN_EVENT_ENTITY_TYPE, "event.fault.1", eventImage("event.fault.1", 1)),
        confirmed(8, AGENT_RUN_EVENT_ENTITY_TYPE, "event.fault.4", eventImage("event.fault.4", 4)),
      ]
      for (const store of stores) {
        Effect.runSync(store.resetScope(scope, replacement, SyncVersion.make(8)))
      }
      const after = stores.map(store => Effect.runSync(
        createKhalaSyncAgentTimeline({ store, session: session(() => cursor) })
          .snapshotForThread(THREAD),
      ))
      expect(after[0]).toEqual(after[1])
      expect(after[0]?.status.cursor).toBe(8)
      expect(after[0]?.run?.status).toBe("completed")
      expect(after[0]?.events.map(event => event.eventRef)).toEqual([
        "event.fault.1",
        "event.fault.4",
      ])
      expect(JSON.stringify(after[0])).not.toContain("event.fault.2")
      expect(JSON.stringify(after[0])).not.toContain("event.fault.3")
    } finally {
      Effect.runSync(desktop.close())
      Effect.runSync(mobile.close())
      rmSync(root, { force: true, recursive: true })
    }
  })

  test("Desktop and mobile reconstruct one interrupted terminal after host restart without duplicate output", () => {
    const root = mkdtempSync(join(tmpdir(), "native-lifecycle-fault-"))
    const desktopPath = join(root, "desktop.sqlite")
    const mobilePath = join(root, "mobile.sqlite")
    let desktop = openDesktopSyncStore(desktopPath, desktopDatabase)
    let mobile = openExpoKhalaSyncStore(mobilePath, mobileDatabase)
    let cursor = 3
    const read = (store: KhalaSyncLocalStore) => Effect.runSync(
      createKhalaSyncAgentTimeline({ store, session: session(() => cursor) })
        .snapshotForThread(THREAD),
    )
    try {
      const inFlight = [
        entry(1, AGENT_RUN_ENTITY_TYPE, RUN, runImage("running")),
        entry(2, AGENT_RUN_EVENT_ENTITY_TYPE, "event.lifecycle.started", lifecycleEventImage(
          "event.lifecycle.started", 0, "turn.started",
        )),
        entry(3, AGENT_RUN_EVENT_ENTITY_TYPE, "event.lifecycle.partial", lifecycleEventImage(
          "event.lifecycle.partial", 1, "runtime.activity",
        )),
      ]
      for (const store of [desktop, mobile]) {
        Effect.runSync(store.applyConfirmed(scope, inFlight, SyncVersion.make(3)))
      }
      expect(read(desktop)).toEqual(read(mobile))
      expect(read(desktop)?.run?.status).toBe("running")

      // Renderer/host death: close both native handles and reconstruct only
      // from their durable confirmed stores.
      Effect.runSync(desktop.close())
      Effect.runSync(mobile.close())
      desktop = openDesktopSyncStore(desktopPath, desktopDatabase)
      mobile = openExpoKhalaSyncStore(mobilePath, mobileDatabase)
      expect(read(desktop)).toEqual(read(mobile))
      expect(read(desktop)?.events.map(event => event.eventRef)).toEqual([
        "event.lifecycle.started",
        "event.lifecycle.partial",
      ])

      // The server's lost-generation sweep projects a single terminal. An
      // exact replay is idempotent in both adapters; stale provider output is
      // absent because runtime.recordEvent rejects it at authority.
      cursor = 4
      const terminal = [
        entry(4, AGENT_RUN_ENTITY_TYPE, RUN, runImage("canceled")),
        entry(4, AGENT_RUN_EVENT_ENTITY_TYPE, "event.lifecycle.interrupted", lifecycleEventImage(
          "event.lifecycle.interrupted", 2, "turn.interrupted",
        )),
      ]
      for (const store of [desktop, mobile]) {
        Effect.runSync(store.applyConfirmed(scope, terminal, SyncVersion.make(4)))
        Effect.runSync(store.applyConfirmed(scope, terminal, SyncVersion.make(4)))
      }
      const snapshots = [read(desktop), read(mobile)]
      expect(snapshots[0]).toEqual(snapshots[1])
      expect(snapshots[0]?.run?.status).toBe("canceled")
      expect(snapshots[0]?.events.map(event => [event.eventRef, event.sequence])).toEqual([
        ["event.lifecycle.started", 0],
        ["event.lifecycle.partial", 1],
        ["event.lifecycle.interrupted", 2],
      ])
      expect(JSON.stringify(snapshots[0])).not.toContain("stale provider output")
    } finally {
      Effect.runSync(Effect.ignore(desktop.close()))
      Effect.runSync(Effect.ignore(mobile.close()))
      rmSync(root, { force: true, recursive: true })
    }
  })
})
