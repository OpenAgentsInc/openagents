import { agentRunScope } from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  AGENT_RUN_EVENT_PROJECTION_SYSTEM_REF,
  AGENT_RUN_PROJECTION_SYSTEM_REF,
  projectAgentRunBestEffort,
  projectAgentRunEventsBestEffort,
} from "./agent-run-projection.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import type { LocalPostgres } from "./test/local-postgres.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const queuedRun = (
  runId: string,
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  backend: "shc_vm",
  canceledAt: null,
  completedAt: null,
  createdAt: "2026-07-05T12:00:00.000Z",
  failedAt: null,
  goal: "Run a bounded repo cleanup mission.",
  goalContext: {
    goalId: "goal.alpha",
    objective: "Run a bounded repo cleanup mission.",
    remainingTokens: 50_000,
    status: "active",
    timeUsedSeconds: 0,
    tokenBudget: 100_000,
    tokensUsed: 0,
    visibility: "private",
  },
  goalId: "goal.alpha",
  projectId: null,
  repository: {
    owner: "OpenAgentsInc",
    provider: "github",
    ref: "main",
    repo: "openagents",
  },
  routeId: `agent_run_${runId}`,
  runId,
  runtime: "opencode_codex",
  startedAt: null,
  status: "queued",
  teamId: null,
  updatedAt: "2026-07-05T12:00:00.000Z",
  userId: "user.alice",
  ...overrides,
})

// ---------------------------------------------------------------------------
// Fail-soft wrapper (no working database: must return a diagnostic)
// ---------------------------------------------------------------------------

describe("projectAgentRunBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await projectAgentRunBestEffort(
      broken,
      queuedRun("run.broken.alpha"),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("an undecodable raw shape refuses without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectAgentRunBestEffort(neverCalled, {
      ...queuedRun("run.malformed.alpha"),
      status: "exploded",
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
    }
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "agent run projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_agent_run")
      await admin.end()
      const url = pg.urlFor("khala_sync_agent_run")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("projects a queued run + its attached goal into scope.agent_run.<runId>", async () => {
      const outcome = await projectAgentRunBestEffort(
        s(),
        queuedRun("run.web.alpha"),
      )
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return
      expect(String(outcome.entry.scope)).toBe(
        String(agentRunScope("run.web.alpha")),
      )
      expect(String(outcome.entry.entityType)).toBe("agent_run")
      expect(String(outcome.entry.entityId)).toBe("run.web.alpha")
      expect(outcome.entry.mutationRef).toBe(AGENT_RUN_PROJECTION_SYSTEM_REF)
      const postImage = JSON.parse(outcome.entry.postImageJson ?? "{}") as {
        runId?: string
        status?: string
        goalContext?: { goalId?: string; tokensUsed?: number }
      }
      expect(postImage.runId).toBe("run.web.alpha")
      expect(postImage.status).toBe("queued")
      expect(postImage.goalContext?.goalId).toBe("goal.alpha")
    })

    test("re-projecting the same run (continuation) upserts — scope version advances", async () => {
      const first = await projectAgentRunBestEffort(
        s(),
        queuedRun("run.web.beta"),
      )
      expect(first.ok).toBe(true)
      const second = await projectAgentRunBestEffort(
        s(),
        queuedRun("run.web.beta", { status: "running" }),
      )
      expect(second.ok).toBe(true)
      if (!first.ok || !second.ok) return
      expect(Number(second.entry.version)).toBeGreaterThan(
        Number(first.entry.version),
      )
      const latestPostImage = JSON.parse(
        second.entry.postImageJson ?? "{}",
      ) as { status?: string }
      expect(latestPostImage.status).toBe("running")
    })

    test("a run with NO attached goal (goalContext omitted) still projects", async () => {
      const { goalContext: _goalContext, ...withoutGoal } = queuedRun(
        "run.web.gamma",
      )
      const outcome = await projectAgentRunBestEffort(s(), {
        ...withoutGoal,
        goalId: null,
      })
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return
      const postImage = JSON.parse(outcome.entry.postImageJson ?? "{}") as {
        goalId?: string | null
        goalContext?: unknown
      }
      expect(postImage.goalId).toBeNull()
      expect(postImage.goalContext).toBeUndefined()
    })
  },
)

// ---------------------------------------------------------------------------
// Event-feed companion entity (KS-6.6 follow-up, #8416)
// ---------------------------------------------------------------------------

const runnerEvent = (
  runId: string,
  sequence: number,
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  artifactRefs: [],
  createdAt: `2026-07-05T12:00:0${sequence}.000Z`,
  externalEventId: null,
  id: `omni_event_${runId}_${sequence}`,
  payloadJson: null,
  runId,
  sequence,
  source: "shc",
  status: null,
  summary: `event #${sequence}`,
  type: "runner.progress",
  ...overrides,
})

describe("projectAgentRunEventsBestEffort fail-soft", () => {
  test("an empty batch is a no-op success (never opens a transaction)", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectAgentRunEventsBestEffort(
      neverCalled,
      "run.empty",
      [],
    )
    expect(outcome).toEqual({ entries: [], ok: true })
  })

  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await projectAgentRunEventsBestEffort(broken, "run.broken", [
      runnerEvent("run.broken", 2),
    ])
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("a malformed event in the batch refuses the WHOLE batch without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectAgentRunEventsBestEffort(
      neverCalled,
      "run.malformed",
      [runnerEvent("run.malformed", 2), runnerEvent("run.malformed", -1)],
    )
    expect(outcome.ok).toBe(false)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "agent run event projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_agent_run_events")
      await admin.end()
      const url = pg.urlFor("khala_sync_agent_run_events")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("projects a batch of events into the SAME scope as the run entity", async () => {
      const runId = "run.events.alpha"
      const runOutcome = await projectAgentRunBestEffort(s(), queuedRun(runId))
      expect(runOutcome.ok).toBe(true)

      const outcome = await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 2),
        runnerEvent(runId, 3),
      ])
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return
      expect(outcome.entries).toHaveLength(2)
      for (const entry of outcome.entries) {
        expect(String(entry.scope)).toBe(String(agentRunScope(runId)))
        expect(String(entry.entityType)).toBe("agent_run_event")
        expect(entry.mutationRef).toBe(AGENT_RUN_EVENT_PROJECTION_SYSTEM_REF)
      }
      expect(outcome.entries.map(entry => String(entry.entityId))).toEqual([
        `omni_event_${runId}_2`,
        `omni_event_${runId}_3`,
      ])
    })

    test("fires on EVERY separate append call, not just the first — the scope version keeps advancing", async () => {
      const runId = "run.events.beta"
      await projectAgentRunBestEffort(s(), queuedRun(runId))

      const first = await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 2, { summary: "tool call: read file" }),
      ])
      const second = await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 3, { summary: "tool call: edit file" }),
      ])
      const third = await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 4, { summary: "assistant message: done" }),
      ])
      expect(first.ok && second.ok && third.ok).toBe(true)
      if (!first.ok || !second.ok || !third.ok) return

      // Each append is its own real changelog row (distinct entity ids,
      // scope version strictly increasing) — proves the projector genuinely
      // fires per append rather than only capturing the first event.
      const versions = [
        Number(first.entries[0]?.version),
        Number(second.entries[0]?.version),
        Number(third.entries[0]?.version),
      ]
      expect(versions[1]).toBeGreaterThan(versions[0] ?? 0)
      expect(versions[2]).toBeGreaterThan(versions[1] ?? 0)

      const summaries = [first, second, third].map(outcome =>
        outcome.ok
          ? JSON.parse(outcome.entries[0]?.postImageJson ?? "{}").summary
          : undefined,
      )
      expect(summaries).toEqual([
        "tool call: read file",
        "tool call: edit file",
        "assistant message: done",
      ])
    })

    test("re-projecting the SAME event id upserts (last write wins)", async () => {
      const runId = "run.events.gamma"
      await projectAgentRunBestEffort(s(), queuedRun(runId))

      await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 2, { status: "running" }),
      ])
      const second = await projectAgentRunEventsBestEffort(s(), runId, [
        runnerEvent(runId, 2, { status: "completed" }),
      ])
      expect(second.ok).toBe(true)
      if (!second.ok) return
      const postImage = JSON.parse(
        second.entries[0]?.postImageJson ?? "{}",
      ) as { status?: string }
      expect(postImage.status).toBe("completed")
    })
  },
)
