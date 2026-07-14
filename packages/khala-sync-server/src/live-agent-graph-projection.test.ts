import {
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  decodeLiveAgentGraphPostImageJson,
  emptyLiveAgentGraphEntity,
  liveAgentGraphScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"

import {
  LIVE_AGENT_GRAPH_PROJECTION_SYSTEM_REF,
  projectLiveAgentGraphBestEffort,
} from "./live-agent-graph-projection.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import type { LocalPostgres } from "./test/local-postgres.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
const graph = () => emptyLiveAgentGraphEntity({
  graphRef: "graph.server.1",
  sessionRef: "session.server.1",
  threadRef: "thread.server.1",
  attachmentGeneration: 1,
  updatedAt: "2026-07-11T20:00:00.000Z",
})

describe("live-agent graph projection fail-soft boundary", () => {
  test("refuses malformed graphs before touching storage", async () => {
    const neverCalled = {
      begin: async () => { throw new Error("must not be reached") },
    } as unknown as SyncSql
    const outcome = await projectLiveAgentGraphBestEffort(neverCalled, {
      ...graph(),
      threadRef: "../private/thread",
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) expect(outcome.diagnostic.reason).toBe("projection_failed")
  })

  test("refuses private-shaped material before storage with a bounded diagnostic", async () => {
    const neverCalled = {
      begin: async () => { throw new Error("must not be reached") },
    } as unknown as SyncSql
    const root = {
      agentRef: "agent.codex.root",
      sessionRef: "session.server.1",
      threadRef: "thread.codex.root",
      transcriptRef: "transcript.codex.root",
      runRef: "run.codex.root",
      parent: { kind: "root" as const },
      provider: { state: "known" as const, kind: "codex" as const, providerRef: "provider.codex.owner" },
      runtime: { state: "known" as const, kind: "codex_app_server" as const, runtimeRef: "runtime.codex.owner" },
      worktree: { state: "unknown" as const, reason: "provider_omitted" as const },
      status: "running" as const,
      attention: { state: "none" as const },
      terminal: { state: "active" as const },
      currentTool: { state: "known" as const, toolCallRef: "tool.codex.1", toolName: "Read /Users/alice/private", status: "running" as const },
      attachmentGeneration: 1,
      activityCursor: 1,
      createdAt: "2026-07-11T20:00:00.000Z",
      updatedAt: "2026-07-11T20:00:01.000Z",
      startedAt: "2026-07-11T20:00:00.000Z",
      endedAt: null,
      version: 1,
    }
    const outcome = await projectLiveAgentGraphBestEffort(neverCalled, {
      ...graph(),
      nodes: [root],
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("redaction_refused")
      expect(outcome.diagnostic.messageSafe).not.toContain("alice")
    }
  })

  test("a broken SQL handle returns a safe diagnostic instead of throwing", async () => {
    const broken = {
      begin: async () => { throw new Error("postgres://user:secret@10.0.0.1") },
    } as unknown as SyncSql
    const outcome = await projectLiveAgentGraphBestEffort(broken, graph())
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })
})

describe.skipIf(!hasLocalPostgres())("live-agent graph projection against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL
  const s = () => sql as unknown as SyncSql

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_live_agent_graph")
    await admin.end()
    const url = pg.urlFor("khala_sync_live_agent_graph")
    await runMigrations({ databaseUrl: url })
    sql = SQL({ url, max: 5 })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("appends canonical post-images with dense thread-scope versions", async () => {
    const first = await projectLiveAgentGraphBestEffort(s(), graph())
    const second = await projectLiveAgentGraphBestEffort(s(), graph())
    expect(first.ok).toBe(true)
    expect(second.ok).toBe(true)
    if (!first.ok || !second.ok) return
    expect(String(first.entry.scope)).toBe(String(liveAgentGraphScope("thread.server.1")))
    expect(Number(first.entry.version)).toBe(1)
    expect(Number(second.entry.version)).toBe(2)
    expect(String(first.entry.entityType)).toBe(LIVE_AGENT_GRAPH_ENTITY_TYPE)
    expect(String(first.entry.entityId)).toBe("graph.server.1")
    expect(first.entry.mutationRef).toBe(LIVE_AGENT_GRAPH_PROJECTION_SYSTEM_REF)
    expect(decodeLiveAgentGraphPostImageJson(first.entry.postImageJson!)).toEqual(graph())
  })
})
