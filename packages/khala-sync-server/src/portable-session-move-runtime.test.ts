import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import {
  makeOwnerLocalCapabilityAdapter,
  type CapabilitySecretVault,
  type SecretMaterial,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import { runMigrations } from "./migrate.js"
import {
  PortableCapabilityBrokerStoreError,
} from "./portable-capability-broker-store.js"
import {
  PostgresPortableSessionMoveRuntime,
  type PortableSessionMoveRuntimeInput,
} from "./portable-session-move-runtime.js"
import type {
  PortableSessionExecutionTarget,
  PortableSessionMoveResult,
} from "./portable-session-move.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"
const ownerRef = "owner.port03.runtime"
const targetRef = "target.port03.runtime.local"

const unreachable = async (): Promise<never> => {
  throw new Error("coordinator test seam must not invoke target")
}

const target: PortableSessionExecutionTarget = {
  targetRef,
  targetClass: "owner_local",
  quiesceGraph: unreachable,
  createCheckpoint: unreachable,
  cleanupSource: unreachable,
  stageCheckpoint: unreachable,
  activate: unreachable,
  abortStaged: unreachable,
}

const input = (suffix: string, moveRef = `move.port03.runtime.${suffix}`): PortableSessionMoveRuntimeInput => ({
  moveRef,
  move: {
    command: {
      schema: "openagents.portable_session_command.v1",
      commandRef: `command.port03.runtime.${suffix}`,
      idempotencyKey: `idempotency.port03.runtime.${suffix}`,
      ownerRef,
      sessionRef: `session.port03.runtime.${suffix}`,
      kind: "move",
      expectedAttachmentRef: `attachment.port03.runtime.${suffix}`,
      expectedGeneration: 1,
      destinationTargetRef: targetRef,
      checkpointRef: `checkpoint.port03.runtime.${suffix}`,
      expiresAt: "2026-07-13T12:30:00.000Z",
    },
    destinationAttachmentRef: `attachment.port03.runtime.${suffix}.next`,
    capabilityTransfers: [],
    source: target,
    destination: target,
  },
  broker: brokerConfig(),
})

const result = (
  runtimeInput: PortableSessionMoveRuntimeInput,
  status: PortableSessionMoveResult["status"],
): PortableSessionMoveResult => ({
  schema: "openagents.portable_session_move.v1",
  status,
  commandRef: runtimeInput.move.command.commandRef,
  sessionRef: runtimeInput.move.command.sessionRef,
  runRef: `run.port03.runtime.${status}`,
  repositoryRef: "repository.port03.runtime",
  pinnedBaseRef: "commit.port03.runtime.base",
  sourceAttachmentRef: runtimeInput.move.command.expectedAttachmentRef,
  sourceGeneration: runtimeInput.move.command.expectedGeneration,
  capabilityLeaseRefs: [],
  acceptedWorkRefs: [],
  evidenceRefs: [`evidence.port03.runtime.${status}`],
})

const vault: CapabilitySecretVault = {
  withSourceGrantMaterial: async ({ use }) => {
    const material = new TextEncoder().encode("fixture-only") as SecretMaterial
    try {
      return await use(material)
    } finally {
      material.fill(0)
    }
  },
  revokeSourceGrant: async () => undefined,
}

const adapter = makeOwnerLocalCapabilityAdapter("adapter.port03.runtime.local", {
  install: async ({ lease }) => ({ installationRef: `installation.${lease.leaseRef}` }),
  wipe: async ({ leaseRef }) => ({ wipeReceiptRef: `receipt.wipe.${leaseRef}` }),
})

const brokerConfig = () => ({
  vault,
  targets: [{
    targetRef,
    targetClass: "owner_local" as const,
    adapterRef: adapter.adapterRef,
    ready: true,
  }],
  adapters: [adapter],
  clock: { now: () => new Date("2026-07-13T12:00:00.000Z") },
})

describe.skipIf(!hasLocalPostgres())("PORT-03 production move runtime", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_portable_move_runtime")
    await admin.end()
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_portable_move_runtime") })
    sql = SQL({ url: pg.urlFor("khala_sync_portable_move_runtime"), max: 10 })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  const insertSession = async (sessionRef: string): Promise<void> => {
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
         event_log_ref, current_projection_ref, command_scope_ref,
         root_agent_ref, state)
      VALUES
        (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`},
         ${`work.${sessionRef}`}, ${`eventlog.${sessionRef}`},
         ${`current.${sessionRef}`}, ${`commands.${sessionRef}`},
         ${`agent.${sessionRef}.root`}, 'active')
    `
  }

  const activeClaim = async (sessionRef: string): Promise<string | null> => {
    const rows = await sql`
      SELECT active_move_ref
      FROM khala_sync_portable_capability_brokers
      WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
    `
    return rows[0]?.active_move_ref ?? null
  }

  test("releases the exact claim at the post-broker-operation revision after a terminal result", async () => {
    const runtimeInput = input("terminal")
    await insertSession(runtimeInput.move.command.sessionRef)
    const runtime = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: broker => ({
        move: async () => {
          await Effect.runPromise(broker.issue({
            operationRef: "operation.port03.runtime.issue",
            leaseRef: "lease.port03.runtime.issue",
            ownerRef,
            sessionRef: runtimeInput.move.command.sessionRef,
            attachmentRef: runtimeInput.move.command.expectedAttachmentRef,
            attachmentGeneration: 1,
            targetRef,
            capability: "provider",
            sourceGrantRef: "grant.port03.runtime.issue",
            accountRef: "account.codex-4",
            permissions: ["provider.turn.execute"],
            expiresAt: "2026-07-13T12:10:00.000Z",
          }))
          return result(runtimeInput, "completed")
        },
      }),
    })

    expect((await runtime.move(runtimeInput)).status).toBe("completed")
    expect(await activeClaim(runtimeInput.move.command.sessionRef)).toBeNull()
    const rows = await sql`
      SELECT revision, state_json
      FROM khala_sync_portable_capability_brokers
      WHERE owner_user_id = ${ownerRef} AND session_ref = ${runtimeInput.move.command.sessionRef}
    `
    expect(Number(rows[0]!.revision)).toBe(3)
    expect(rows[0]!.state_json).toBeDefined()
  })

  test("retains pending reconciliation and lets the same bytes restart and finish", async () => {
    const runtimeInput = input("restart")
    await insertSession(runtimeInput.move.command.sessionRef)
    const pending = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: () => ({ move: async () => result(runtimeInput, "authority_pending_reconcile") }),
    })
    expect((await pending.move(runtimeInput)).status).toBe("authority_pending_reconcile")
    expect(await activeClaim(runtimeInput.move.command.sessionRef)).toBe(runtimeInput.moveRef)

    const resumed = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: () => ({ move: async () => result(runtimeInput, "replayed") }),
    })
    expect((await resumed.move(runtimeInput)).status).toBe("replayed")
    expect(await activeClaim(runtimeInput.move.command.sessionRef)).toBeNull()
  })

  test("rejects a conflicting move before constructing or invoking the coordinator", async () => {
    const first = input("conflict")
    await insertSession(first.move.command.sessionRef)
    const pending = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: () => ({ move: async () => result(first, "activation_pending_reconcile") }),
    })
    await pending.move(first)

    let invoked = false
    const conflicting = { ...first, moveRef: "move.port03.runtime.conflicting" }
    const runtime = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: () => {
        invoked = true
        return { move: async () => result(conflicting, "completed") }
      },
    })
    await expect(runtime.move(conflicting)).rejects.toBeInstanceOf(PortableCapabilityBrokerStoreError)
    expect(invoked).toBeFalse()
    expect(await activeClaim(first.move.command.sessionRef)).toBe(first.moveRef)
  })
})
