import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"

import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import {
  PortableSessionAuthorityError,
  appendPortableSessionEvent,
  completePortableSessionMove,
  decodePortableRegisterSessionArgs,
  readPortableSessionAuthoritySnapshot,
  recordPortableSessionCommandOutcome,
  registerPortableSession,
  repairPortableSessionCurrentProjection,
  requestPortableSessionCommand,
  purgePortableSessionAuthority,
} from "./portable-session-authority.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const owner = "owner.port01"
const sessionRef = "session.port01"
const sourceAttachmentRef = "attachment.port01.1"
const destinationAttachmentRef = "attachment.port01.2"
const checkpointRef = "checkpoint.port01.1"
const digest = (character: string) => `sha256:${character.repeat(64)}`

const session = {
  schema: "openagents.portable_session.v1" as const,
  sessionRef,
  ownerRef: owner,
  identityBasis: "owner_minted" as const,
  workContextRef: "work.port01",
  eventLogRef: "eventlog.port01",
  currentProjectionRef: "current.port01",
  volatileStreamRef: "stream.port01",
  commandScopeRef: "commands.port01",
  graph: {
    rootAgentRef: "agent.root",
    nodes: [
      {
        agentRef: "agent.root",
        threadRef: "thread.root",
        transcriptRef: "transcript.root",
        activityCursor: 0,
        lifecycle: "running" as const,
        attachmentGeneration: 1,
      },
      {
        agentRef: "agent.child",
        parentAgentRef: "agent.root",
        threadRef: "thread.child",
        transcriptRef: "transcript.child",
        activityCursor: 0,
        lifecycle: "waiting" as const,
        attachmentGeneration: 1,
      },
    ],
  },
  adoptedFromLocalHistory: false,
}

const targets = [
  {
    targetRef: "target.owner.local",
    targetClass: "owner_local" as const,
    adapterRef: "adapter.local.v1",
    ownerRef: owner,
    compatibilityRef: "compat.portable.v1",
    isolation: "owner_host_process" as const,
    dataPosture: "owner_device_only" as const,
    health: "ready" as const,
  },
  {
    targetRef: "target.owner.managed",
    targetClass: "owner_managed" as const,
    adapterRef: "adapter.managed.v1",
    ownerRef: owner,
    compatibilityRef: "compat.portable.v1",
    isolation: "dedicated_microvm" as const,
    dataPosture: "owner_managed_region" as const,
    health: "ready" as const,
  },
]

const sourceAttachment = {
  attachmentRef: sourceAttachmentRef,
  sessionRef,
  targetRef: targets[0]!.targetRef,
  generation: 1,
  state: "active" as const,
  descendantAgentRefs: ["agent.root", "agent.child"],
  capabilityLeaseRefs: ["lease.source.provider"],
  evidenceRefs: ["evidence.source.started"],
}

const command = {
  schema: "openagents.portable_session_command.v1" as const,
  commandRef: "command.port01.move.1",
  idempotencyKey: "idempotency.port01.move.1",
  ownerRef: owner,
  sessionRef,
  kind: "move" as const,
  expectedAttachmentRef: sourceAttachmentRef,
  expectedGeneration: 1,
  destinationTargetRef: targets[1]!.targetRef,
  checkpointRef,
  expiresAt: "2099-01-01T00:00:00.000Z",
}

const checkpoint = {
  schema: "openagents.portable_checkpoint.v1" as const,
  checkpointRef,
  sessionRef,
  sourceAttachmentRef,
  sourceGeneration: 1,
  digest: digest("a"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "revision.port01",
  repositoryPostImageDigest: digest("b"),
  diffDigest: digest("c"),
  eventLogCursor: 2,
  catalogGenerationRef: "catalog.port01.1",
  graphDigest: digest("d"),
  approvalRefs: [],
  artifactRefs: ["artifact.port01.checkpoint"],
  receiptRefs: ["receipt.port01.checkpoint"],
  secretMaterial: "excluded" as const,
  processState: "excluded" as const,
}

const destinationAttachment = {
  attachmentRef: destinationAttachmentRef,
  sessionRef,
  targetRef: targets[1]!.targetRef,
  generation: 2,
  state: "active" as const,
  descendantAgentRefs: ["agent.root", "agent.child"],
  capabilityLeaseRefs: ["lease.destination.provider"],
  checkpointRef,
  evidenceRefs: ["evidence.destination.rehydrated"],
}

const outcome = {
  commandRef: command.commandRef,
  sessionRef,
  status: "completed" as const,
  sourceAttachmentRef,
  sourceGeneration: 1,
  destinationAttachmentRef,
  destinationGeneration: 2,
  checkpointRef,
  evidenceRefs: ["receipt.port01.move"],
}

describe.skipIf(!hasLocalPostgres())("PORT-01 portable session authority against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_portable_session")
    await admin.end()
    const result = await runMigrations({ databaseUrl: pg.urlFor("khala_sync_portable_session") })
    expect(result.applied).toContain("0066_portable_session_authority.sql")
    sql = new SQL({ url: pg.urlFor("khala_sync_portable_session"), max: 10 })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("rejects private host material before any durable write", () => {
    expect(() => decodePortableRegisterSessionArgs(JSON.stringify({
      session,
      targets,
      attachment: sourceAttachment,
      apiKey: "not-allowed",
    }))).toThrow("forbidden private material")
  })

  test("survives a fresh SQL handle and repairs current solely from the durable event log", async () => {
    await withSyncTransaction(sql as unknown as SyncSql, writer =>
      registerPortableSession(writer, { session, targets, attachment: sourceAttachment }, owner, "mutation.register"))

    await withSyncTransaction(sql as unknown as SyncSql, async writer => {
      expect(await appendPortableSessionEvent(writer, {
        eventRef: "event.root.1",
        sessionRef,
        threadRef: "thread.root",
        threadCursor: 1,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 1 },
      }, "mutation.event.1")).toBe(1)
      expect(await appendPortableSessionEvent(writer, {
        eventRef: "event.child.1",
        sessionRef,
        threadRef: "thread.child",
        threadCursor: 1,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "agent_lifecycle",
        current: { lifecycle: "waiting", activityCursor: 0 },
      }, "mutation.event.2")).toBe(2)
    })

    const restarted = new SQL({ url: pg.urlFor("khala_sync_portable_session"), max: 1 })
    const snapshot = await readPortableSessionAuthoritySnapshot(
      restarted as unknown as SyncSql,
      { sessionRef, ownerUserId: owner },
    )
    expect(snapshot?.agents).toHaveLength(2)
    expect(snapshot?.targets).toHaveLength(2)
    expect(snapshot?.current).toHaveLength(2)
    await restarted`DELETE FROM khala_sync_portable_thread_current WHERE session_ref = ${sessionRef}`
    await restarted.end()

    const repaired = await withSyncTransaction(sql as unknown as SyncSql, writer =>
      repairPortableSessionCurrentProjection(writer, sessionRef, owner, "mutation.repair"))
    expect(repaired).toEqual([
      { threadRef: "thread.root", latestCursor: 1, repairedFromEventSeq: 1 },
      { threadRef: "thread.child", latestCursor: 1, repairedFromEventSeq: 2 },
    ])

    await expect(withSyncTransaction(sql as unknown as SyncSql, writer =>
      appendPortableSessionEvent(writer, {
        eventRef: "event.root.gap",
        sessionRef,
        threadRef: "thread.root",
        threadCursor: 3,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 3 },
      }, "mutation.event.gap"))).rejects.toMatchObject({ code: "cursor_gap" })
  })

  test("deduplicates a lost command ACK byte-for-byte and rejects identity reuse", async () => {
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      requestPortableSessionCommand(writer, command, owner, "mutation.command.1"))).toBe("accepted")
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      requestPortableSessionCommand(writer, command, owner, "mutation.command.retry"))).toBe("duplicate")

    await expect(withSyncTransaction(sql as unknown as SyncSql, writer =>
      requestPortableSessionCommand(writer, {
        ...command,
        commandRef: "command.port01.conflict",
      }, owner, "mutation.command.conflict"))).rejects.toMatchObject({
      code: "conflict",
    } satisfies Partial<PortableSessionAuthorityError>)
  })

  test("moves the complete graph once, replays a lost completion ACK, and fences the source", async () => {
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      completePortableSessionMove(writer, {
        commandRef: command.commandRef,
        checkpoint,
        destinationAttachment,
        outcome,
      }, "mutation.move.complete"))).toBe("completed")
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      completePortableSessionMove(writer, {
        commandRef: command.commandRef,
        checkpoint,
        destinationAttachment,
        outcome,
      }, "mutation.move.retry"))).toBe("duplicate")

    const snapshot = await readPortableSessionAuthoritySnapshot(
      sql as unknown as SyncSql,
      { sessionRef, ownerUserId: owner },
    )
    expect(snapshot?.session.current_attachment_ref).toBe(destinationAttachmentRef)
    expect(Number(snapshot?.session.current_attachment_generation)).toBe(2)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["detached", "active"])
    expect(snapshot?.agents.every(row => Number(row.attachment_generation) === 2)).toBe(true)
    expect(snapshot?.checkpoints).toHaveLength(1)
    expect(snapshot?.commands).toHaveLength(1)

    await expect(withSyncTransaction(sql as unknown as SyncSql, writer =>
      appendPortableSessionEvent(writer, {
        eventRef: "event.source.late",
        sessionRef,
        threadRef: "thread.root",
        threadCursor: 2,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 2 },
      }, "mutation.source.late"))).rejects.toMatchObject({ code: "stale_generation" })
  })

  test("records generic command outcomes idempotently and purges every durable authority row", async () => {
    const { destinationTargetRef: _destinationTargetRef, checkpointRef: _checkpointRef, ...commandBase } = command
    const stopCommand = {
      ...commandBase,
      commandRef: "command.port01.stop.2",
      idempotencyKey: "idempotency.port01.stop.2",
      kind: "stop" as const,
      expectedAttachmentRef: destinationAttachmentRef,
      expectedGeneration: 2,
    }
    await withSyncTransaction(sql as unknown as SyncSql, writer =>
      requestPortableSessionCommand(writer, stopCommand, owner, "mutation.stop"))
    const stopOutcome = {
      commandRef: stopCommand.commandRef,
      sessionRef,
      status: "completed" as const,
      sourceAttachmentRef: destinationAttachmentRef,
      sourceGeneration: 2,
      evidenceRefs: ["receipt.port01.stop"],
    }
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      recordPortableSessionCommandOutcome(writer, stopOutcome, "mutation.stop.outcome"))).toBe("recorded")
    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      recordPortableSessionCommandOutcome(writer, stopOutcome, "mutation.stop.outcome.retry"))).toBe("duplicate")

    expect(await withSyncTransaction(sql as unknown as SyncSql, writer =>
      purgePortableSessionAuthority(writer, { sessionRef, ownerUserId: owner }, "mutation.retention.purge"))).toBe(true)
    expect(await readPortableSessionAuthoritySnapshot(
      sql as unknown as SyncSql,
      { sessionRef, ownerUserId: owner },
    )).toBeNull()
    const remaining: Array<{ count: number | string | bigint }> = await sql`
      SELECT count(*) AS count FROM khala_sync_portable_events WHERE session_ref = ${sessionRef}
    `
    expect(Number(remaining[0]?.count)).toBe(0)
  })
})
