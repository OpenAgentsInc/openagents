import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
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
  const database = new Database(path, { create: true })
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
  const database = new Database(path, { create: true })
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

const runImage = (status: "running" | "completed") => canonicalJson(
  encodeAgentRunEntity(decodeAgentRunEntity({
    backend: "pylon",
    canceledAt: null,
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
})
