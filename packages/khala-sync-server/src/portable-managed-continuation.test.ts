import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, test } from "bun:test"

import {
  createOaCodexControlPortableManagedContinuation,
  PostgresPortableManagedContinuationAuthority,
  PortableManagedContinuationError,
} from "./portable-managed-continuation.js"
import { appendPortableSessionEvent } from "./portable-session-authority.js"
import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"

const graph = {
  rootAgentRef: "agent.continuation.root",
  nodes: [
    {
      agentRef: "agent.continuation.root",
      threadRef: "thread.continuation.root",
      transcriptRef: "transcript.continuation.root",
      activityCursor: 3,
      lifecycle: "waiting" as const,
      attachmentGeneration: 1,
    },
    {
      agentRef: "agent.continuation.child",
      parentAgentRef: "agent.continuation.root",
      threadRef: "thread.continuation.child",
      transcriptRef: "transcript.continuation.child",
      activityCursor: 4,
      lifecycle: "waiting" as const,
      attachmentGeneration: 1,
    },
  ],
}

const expectedThreadCursors = graph.nodes.map((node, index) => ({
  agentRef: node.agentRef,
  threadRef: node.threadRef,
  activityCursor: node.activityCursor,
  eventCursor: index + 8,
}))

const plan = {
  operationRef: "operation.continuation.root-child.1",
  providerLeaseRef: "lease.continuation.provider.1",
  turns: graph.nodes.map(node => ({
    agentRef: node.agentRef,
    turnRef: `turn.continuation.${node.agentRef}`,
    task: `Private bounded task for ${node.agentRef}`,
  })),
}

test("binds exact root/child turns and validates replay-safe cursor advancement", async () => {
  const requests: Array<Record<string, unknown>> = []
  let replay = false
  const continuation = createOaCodexControlPortableManagedContinuation({
    baseUrl: "http://127.0.0.1:8787",
    bearerToken: "fixture-continuation-bearer",
    ownerRef: "owner.continuation",
    targetRef: "target.continuation.managed",
    fetch: async (_request, init) => {
      requests.push(JSON.parse(String(init?.body)))
      const response = {
        acceptedWorkRefs: plan.turns.map(({ agentRef, turnRef }) => ({ agentRef, turnRef })),
        threadCursors: expectedThreadCursors.map(row => ({
          ...row,
          activityCursor: row.activityCursor + 1,
          eventCursor: row.eventCursor + 1,
        })),
        evidenceRefs: ["evidence.continuation.accepted"],
        replay: replay ? "replayed" : "executed",
        material: "excluded",
      }
      replay = true
      return Response.json(response)
    },
  })
  const input = {
    sessionRef: "session.continuation",
    attachmentRef: "attachment.continuation.managed.2",
    generation: 2,
    expectedGraph: graph,
    expectedThreadCursors,
    plan,
  }
  const first = await continuation.run(input)
  const second = await continuation.run(input)
  expect(first.replay).toBe("executed")
  expect(second).toEqual({ ...first, replay: "replayed" })
  expect(requests).toHaveLength(2)
  expect(requests[0]).toMatchObject({
    operationRef: plan.operationRef,
    providerLeaseRef: plan.providerLeaseRef,
    expectedThreadCursors,
    turns: plan.turns,
  })
  expect(JSON.stringify(first)).not.toContain("Private bounded task")
  expect(requests[0]).not.toHaveProperty("resourceRef")
})

test("rejects mismatched accepted refs and private response echo", async () => {
  const base = {
    baseUrl: "https://agent-computer.example",
    bearerToken: "fixture-continuation-bearer",
    ownerRef: "owner.continuation",
    targetRef: "target.continuation.managed",
  }
  const mismatched = createOaCodexControlPortableManagedContinuation({
    ...base,
    fetch: async () => Response.json({
      acceptedWorkRefs: [{ agentRef: graph.rootAgentRef, turnRef: "turn.wrong" }],
      threadCursors: [],
      evidenceRefs: ["evidence.continuation.wrong"],
      replay: "executed",
      material: "excluded",
    }),
  })
  await expect(mismatched.run({
    sessionRef: "session.continuation",
    attachmentRef: "attachment.continuation.managed.2",
    generation: 2,
    expectedGraph: graph,
    expectedThreadCursors,
    plan,
  })).rejects.toBeInstanceOf(PortableManagedContinuationError)

  const echo = createOaCodexControlPortableManagedContinuation({
    ...base,
    fetch: async () => Response.json({ task: "private echo" }),
  })
  await expect(echo.run({
    sessionRef: "session.continuation",
    attachmentRef: "attachment.continuation.managed.2",
    generation: 2,
    expectedGraph: graph,
    expectedThreadCursors,
    plan,
  })).rejects.toMatchObject({ code: "unsafe_response" })
})

describe.skipIf(!hasLocalPostgres())("durable managed continuation authority", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_continuation")
    await admin.end()
    const databaseUrl = pg.urlFor("khala_sync_continuation")
    await runMigrations({ databaseUrl })
    sql = new SQL({ url: databaseUrl, max: 4 })
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
         event_log_ref, current_projection_ref, command_scope_ref,
         root_agent_ref, state, current_attachment_ref, current_attachment_generation)
      VALUES
        ('session.continuation', 'owner.continuation', 'scope.user.owner.continuation',
         'work.continuation', 'eventlog.continuation', 'current.continuation',
         'commands.continuation', ${graph.rootAgentRef}, 'active',
         'attachment.continuation.managed.2', 2)
    `
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
         isolation, data_posture, health)
      VALUES
        ('target.continuation.managed', 'owner.continuation', 'openagents_managed',
         'adapter.continuation.managed', 'compatibility.continuation.1',
         'dedicated_microvm', 'openagents_managed_region', 'ready')
    `
    await sql`
      INSERT INTO khala_sync_portable_session_targets (session_ref, target_ref)
      VALUES ('session.continuation', 'target.continuation.managed')
    `
    await sql`
      INSERT INTO khala_sync_portable_attachments
        (attachment_ref, session_ref, target_ref, generation, state,
         descendant_agent_refs_json, capability_lease_refs_json, evidence_refs_json)
      VALUES
        ('attachment.continuation.managed.2', 'session.continuation',
         'target.continuation.managed', 2, 'active',
         ${JSON.stringify(graph.nodes.map(node => node.agentRef))}::jsonb,
         '[]'::jsonb, '["evidence.continuation.attachment"]'::jsonb)
    `
    for (const node of graph.nodes) {
      await sql`
        INSERT INTO khala_sync_portable_agent_nodes
          (session_ref, agent_ref, parent_agent_ref, thread_ref, transcript_ref,
           activity_cursor, lifecycle, attachment_generation)
        VALUES
          ('session.continuation', ${node.agentRef}, ${"parentAgentRef" in node ? node.parentAgentRef : null},
           ${node.threadRef}, ${node.transcriptRef}, ${node.activityCursor}, 'waiting', 2)
      `
    }
    await withSyncTransaction(sql as unknown as SyncSql, async writer => {
      for (const [index, node] of graph.nodes.entries()) {
        await appendPortableSessionEvent(writer, {
          eventRef: `event.continuation.initial.${node.agentRef}`,
          sessionRef: "session.continuation",
          threadRef: node.threadRef,
          threadCursor: 1,
          attachmentRef: "attachment.continuation.managed.2",
          attachmentGeneration: 2,
          eventKind: "activity_cursor",
          current: { lifecycle: "waiting", activityCursor: node.activityCursor, seed: index },
        }, `mutation.continuation.initial.${node.agentRef}`)
      }
    })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("atomically advances exact agent/event cursors and replays without duplicates", async () => {
    const authority = new PostgresPortableManagedContinuationAuthority({
      sql: sql as unknown as SyncSql,
      transaction: run => withSyncTransaction(sql as unknown as SyncSql, run),
    })
    const expected = await authority.readExpectedCursors({
      ownerRef: "owner.continuation",
      sessionRef: "session.continuation",
      attachmentRef: "attachment.continuation.managed.2",
      generation: 2,
      expectedGraph: graph,
    })
    expect(expected.map(row => row.eventCursor)).toEqual([1, 1])
    const receipt = {
      acceptedWorkRefs: plan.turns.map(({ agentRef, turnRef }) => ({ agentRef, turnRef })),
      threadCursors: expected.map(row => ({
        ...row,
        activityCursor: row.activityCursor + 1,
        eventCursor: row.eventCursor + 1,
      })),
      evidenceRefs: ["evidence.continuation.durable"],
      replay: "executed" as const,
    }
    const commit = () => authority.commit({
      ownerRef: "owner.continuation",
      sessionRef: "session.continuation",
      attachmentRef: "attachment.continuation.managed.2",
      generation: 2,
      expectedGraph: graph,
      expectedThreadCursors: expected,
      plan,
      receipt,
    })
    await commit()
    await commit()
    const events = await sql`
      SELECT event_ref FROM khala_sync_portable_events
      WHERE session_ref = 'session.continuation' AND event_ref LIKE 'event.operation.continuation.%'
    `
    expect(events).toHaveLength(2)
    const agents = await sql`
      SELECT agent_ref, activity_cursor, lifecycle FROM khala_sync_portable_agent_nodes
      WHERE session_ref = 'session.continuation' ORDER BY agent_ref
    `
    expect(agents.map((row: Record<string, unknown>) => [row.agent_ref, Number(row.activity_cursor), row.lifecycle])).toEqual([
      ["agent.continuation.child", 5, "waiting"],
      ["agent.continuation.root", 4, "waiting"],
    ])
  })
})
