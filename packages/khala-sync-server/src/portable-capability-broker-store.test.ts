import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import {
  PortableCapabilityBroker,
  makeOwnerLocalCapabilityAdapter,
  type CapabilityBrokerEvidence,
  type CapabilityBrokerPrivateDurableState,
  type CapabilitySecretVault,
  type SecretMaterial,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import { runMigrations } from "./migrate.js"
import {
  PostgresPortableCapabilityBrokerStore,
  PortableCapabilityBrokerStoreError,
  readPortableCapabilityEvidence,
  type PortableCapabilityBrokerStoreScope,
} from "./portable-capability-broker-store.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"
const ownerRef = "owner.port03.store"
const sessionRef = "session.port03.store"
const scope: PortableCapabilityBrokerStoreScope = {
  ownerRef,
  sessionRef,
  moveClaim: {
    moveRef: "move.port03.store.1",
    commandRef: "command.port03.store.1",
    sourceAttachmentRef: "attachment.port03.store.1",
    sourceGeneration: 1,
    destinationTargetRef: "target.port03.store.managed",
  },
}

const emptyState = (): CapabilityBrokerPrivateDurableState => ({
  schema: "openagents.portable_capability_broker.v1",
  records: [],
  operations: [],
  evidence: [],
  material: "excluded",
})

const evidence = (operationRef: string): CapabilityBrokerEvidence => ({
  schema: "openagents.portable_capability_broker.v1",
  evidenceRef: `evidence.capability.${operationRef}`,
  operationRef,
  operation: "issue",
  status: "completed",
  leaseRef: "lease.port03.store.provider",
  ownerRef,
  sessionRef,
  attachmentRef: scope.moveClaim.sourceAttachmentRef,
  attachmentGeneration: 1,
  targetRef: "target.port03.store.local",
  capability: "provider",
  accountRef: "account.codex-4",
  occurredAt: "2026-07-13T11:30:00.000Z",
  material: "excluded",
})

describe.skipIf(!hasLocalPostgres())("PORT-03 durable capability broker store", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_portable_broker")
    await admin.end()
    const result = await runMigrations({ databaseUrl: pg.urlFor("khala_sync_portable_broker") })
    expect(result.applied).toContain("0069_portable_capability_broker.sql")
    sql = SQL({ url: pg.urlFor("khala_sync_portable_broker"), max: 10 })
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
         event_log_ref, current_projection_ref, command_scope_ref,
         root_agent_ref, state)
      VALUES
        (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`},
         'work.port03.store', 'eventlog.port03.store',
         'current.port03.store', 'commands.port03.store',
         'agent.port03.root', 'active')
    `
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("acquires exactly one move claim and rejects a conflicting claimant", async () => {
    const store = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, scope)
    expect(await store.acquireMoveClaim(0)).toEqual({ revision: 1 })
    expect(await store.acquireMoveClaim(1)).toEqual({ revision: 1 })

    const conflict = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, {
      ...scope,
      moveClaim: { ...scope.moveClaim, moveRef: "move.port03.store.conflict" },
    })
    await expect(conflict.acquireMoveClaim(1)).rejects.toMatchObject({ code: "claim_conflict" })
  })

  test("atomically commits state, evidence, revision CAS, and the exact active claim", async () => {
    const store = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, scope)
    const firstEvidence = evidence("operation.port03.store.first")
    const state = { ...emptyState(), evidence: [firstEvidence] }
    expect(await store.commit({ expectedRevision: 1, state, evidence: firstEvidence })).toEqual({ revision: 2 })
    expect(await store.load()).toEqual({ revision: 2, state })
    expect(await readPortableCapabilityEvidence(sql as unknown as SyncSql, ownerRef, sessionRef)).toEqual([firstEvidence])

    await expect(store.commit({
      expectedRevision: 1,
      state: emptyState(),
      evidence: evidence("operation.port03.store.stale"),
    })).rejects.toMatchObject({ code: "stale_revision" })
    expect((await store.load()).revision).toBe(2)
  })

  test("rolls back state and revision when evidence insertion fails", async () => {
    const store = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, scope)
    const before = await store.load()
    const duplicate = evidence("operation.port03.store.first")
    await expect(store.commit({
      expectedRevision: before.revision,
      state: { ...emptyState(), evidence: [duplicate, duplicate] },
      evidence: duplicate,
    })).rejects.toBeDefined()
    expect(await store.load()).toEqual(before)
  })

  test("restores a fresh broker with byte-identical operation replay", async () => {
    const store = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, scope)
    const loaded = await store.load()
    const released = await store.releaseMoveClaim(loaded.revision)
    const secondScope = {
      ...scope,
      moveClaim: { ...scope.moveClaim, moveRef: "move.port03.store.2", commandRef: "command.port03.store.2" },
    }
    const second = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, secondScope)
    const claimed = await second.acquireMoveClaim(released.revision)
    expect(claimed.revision).toBe(released.revision + 1)

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
    const adapter = makeOwnerLocalCapabilityAdapter("adapter.port03.local", {
      install: async ({ lease }) => ({ installationRef: `installation.${lease.leaseRef}` }),
      wipe: async ({ leaseRef }) => ({ wipeReceiptRef: `receipt.wipe.${leaseRef}` }),
    })
    const config = {
      vault,
      targets: [{
        targetRef: "target.port03.store.local",
        targetClass: "owner_local" as const,
        adapterRef: adapter.adapterRef,
        ready: true,
      }],
      adapters: [adapter],
      atomicStateStore: second,
      clock: { now: () => new Date("2026-07-13T11:30:00.000Z") },
    }
    const broker = await PortableCapabilityBroker.restore(config)
    const input = {
      operationRef: "operation.port03.store.atomic.issue",
      leaseRef: "lease.port03.store.atomic",
      ownerRef,
      sessionRef,
      attachmentRef: scope.moveClaim.sourceAttachmentRef,
      attachmentGeneration: 1,
      targetRef: "target.port03.store.local",
      capability: "provider" as const,
      sourceGrantRef: "grant.port03.store.atomic",
      accountRef: "account.codex-4",
      permissions: ["provider.turn.execute"],
      expiresAt: "2026-07-13T11:40:00.000Z",
    }
    const first = await Effect.runPromise(broker.issue(input))
    const restarted = await PortableCapabilityBroker.restore(config)
    const replay = await Effect.runPromise(restarted.issue(input))
    expect(first.status).toBe("completed")
    expect(replay.status).toBe("replayed")
    expect(restarted.snapshot().leases).toHaveLength(1)
    expect(restarted.snapshot().evidence).toHaveLength(2)
  })

  test("refuses private material before a transaction can mutate durable state", async () => {
    const store = new PostgresPortableCapabilityBrokerStore(sql as unknown as SyncSql, scope)
    const current = await sql`
      SELECT revision FROM khala_sync_portable_capability_brokers
      WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
    `
    const revision = Number(current[0]!.revision)
    const unsafe = {
      ...emptyState(),
      records: [{ apiKey: "must-not-persist" }],
    } as unknown as CapabilityBrokerPrivateDurableState
    try {
      await store.commit({
        expectedRevision: revision,
        state: unsafe,
        evidence: evidence("operation.port03.store.unsafe"),
      })
      throw new Error("expected unsafe state rejection")
    } catch (error) {
      expect(error).toBeInstanceOf(PortableCapabilityBrokerStoreError)
      expect((error as PortableCapabilityBrokerStoreError).code).toBe("unsafe_state")
    }
    const after = await sql`
      SELECT revision FROM khala_sync_portable_capability_brokers
      WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
    `
    expect(Number(after[0]!.revision)).toBe(revision)
  })
})
