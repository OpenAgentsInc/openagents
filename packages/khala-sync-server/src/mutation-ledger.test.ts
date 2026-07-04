import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutationResult,
  type MutationStatus,
  MutatorName,
  publicScope,
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  checkAndReserve,
  duplicateResultFor,
  getMutation,
  KhalaSyncClientStateMismatchError,
  lastMutationId,
  OUT_OF_ORDER_ERROR_CODE,
  outOfOrderResult,
  recordMutation,
  type RecordedMutation,
  upsertClientState,
} from "./mutation-ledger.js"
import { withSyncTransaction } from "./outbox-writer.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const mutatorName = MutatorName.make("thing.set")
const entityType = EntityType.make("thing")
const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-ledger-${clientCounter}`),
    clientId: ClientId.make(`c-ledger-${clientCounter}`),
    userId: `user-ledger-${clientCounter}`,
    scope: publicScope(`ledger-test-${clientCounter}`),
  }
}

const envelope = (id: number): MutationEnvelope =>
  new MutationEnvelope({
    mutationId: MutationId.make(id),
    name: mutatorName,
    argsJson: canonicalJson({ n: id }),
  })

// ---------------------------------------------------------------------------
// Pure: result builders (no database needed)
// ---------------------------------------------------------------------------

describe("result builders", () => {
  test("outOfOrderResult is an in-band rejection with the typed error code", () => {
    const result = outOfOrderResult(MutationId.make(7), 3)
    expect(result.status).toBe("rejected")
    expect(Number(result.mutationId)).toBe(7)
    expect(result.errorCode).toBe(OUT_OF_ORDER_ERROR_CODE)
    expect(result.errorMessageSafe).toContain("expected 4")
  })

  test("duplicateResultFor carries the recorded errorCode and errorMessageSafe", () => {
    const recorded: RecordedMutation = {
      clientGroupId: ClientGroupId.make("cg"),
      clientId: ClientId.make("c"),
      mutationId: MutationId.make(2),
      name: "thing.set",
      status: "rejected",
      errorCode: "validation_failed",
      resultJson: canonicalJson({
        mutationId: 2,
        status: "rejected",
        errorCode: "validation_failed",
        errorMessageSafe: "title required",
      }),
      committedAt: new Date().toISOString(),
    }
    const result = duplicateResultFor(recorded)
    expect(result.status).toBe("duplicate")
    expect(result.errorCode).toBe("validation_failed")
    expect(result.errorMessageSafe).toBe("title required")
  })

  test("duplicateResultFor on an applied recording has no error fields", () => {
    const recorded: RecordedMutation = {
      clientGroupId: ClientGroupId.make("cg"),
      clientId: ClientId.make("c"),
      mutationId: MutationId.make(1),
      name: "thing.set",
      status: "applied",
      committedAt: new Date().toISOString(),
    }
    const result = duplicateResultFor(recorded)
    expect(result.status).toBe("duplicate")
    expect(result.errorCode).toBeUndefined()
    expect(result.errorMessageSafe).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// Integration: real local Postgres (initdb + pg_ctl throwaway instance)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("mutation ledger against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_ledger")
    await admin.end()
    const url = pg.urlFor("khala_sync_ledger")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    sql = new SQL({ url, max: 10 })
  })

  afterAll(async () => {
    await sql?.end()
    await pg?.stop()
  })

  const changelogCount = async (scope: SyncScope): Promise<number> => {
    const rows = await sql`
      SELECT count(*)::int AS c FROM khala_sync_changelog WHERE scope = ${scope}
    `
    return rows[0].c as number
  }

  const ledgerCount = async (
    clientGroupId: ClientGroupId,
    clientId: ClientId,
  ): Promise<number> => {
    const rows = await sql`
      SELECT count(*)::int AS c FROM khala_sync_mutations
       WHERE client_group_id = ${clientGroupId} AND client_id = ${clientId}
    `
    return rows[0].c as number
  }

  /**
   * Minimal push-engine loop over one batch: one transaction per envelope
   * (upsertClientState → checkAndReserve → execute → recordMutation),
   * exactly the shape the KS-3 mutator engine uses. `outcome` controls
   * what the "mutator" does for envelopes that reach execution.
   */
  const pushBatch = async (
    client: {
      clientGroupId: ClientGroupId
      clientId: ClientId
      userId: string
      scope: SyncScope
    },
    envelopes: ReadonlyArray<MutationEnvelope>,
    options?: {
      readonly outcome?: (env: MutationEnvelope) => {
        readonly status: Exclude<MutationStatus, "duplicate">
        readonly errorCode?: string
        readonly errorMessageSafe?: string
      }
      readonly onExecute?: (env: MutationEnvelope) => void
    },
  ): Promise<Array<MutationResult>> => {
    const results: Array<MutationResult> = []
    for (const env of envelopes) {
      const result = await withSyncTransaction(sql, async (writer) => {
        // Bind the client group (and take its row lock) before gating —
        // this is the engine's per-group serialization point.
        await upsertClientState(writer.sql, {
          clientGroupId: client.clientGroupId,
          userId: client.userId,
          schemaVersion,
        })
        const gate = await checkAndReserve(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          envelope: env,
        })
        if (gate.kind !== "execute") return gate.result

        options?.onExecute?.(env)
        const outcome = options?.outcome?.(env) ?? { status: "applied" as const }
        if (outcome.status === "applied") {
          // The mutator's replicated effect: one changelog append.
          await writer.appendChange({
            scope: client.scope,
            entityType,
            entityId: EntityId.make(`thing-${Number(env.mutationId)}`),
            op: "upsert",
            postImage: { n: Number(env.mutationId) },
            mutationRef: `mutation:${client.clientId}:${Number(env.mutationId)}`,
          })
        }
        const result = new MutationResult({
          mutationId: env.mutationId,
          status: outcome.status,
          ...(outcome.errorCode === undefined
            ? {}
            : { errorCode: outcome.errorCode }),
          ...(outcome.errorMessageSafe === undefined
            ? {}
            : { errorMessageSafe: outcome.errorMessageSafe }),
        })
        await recordMutation(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          mutationId: env.mutationId,
          name: env.name,
          status: outcome.status,
          ...(outcome.errorCode === undefined
            ? {}
            : { errorCode: outcome.errorCode }),
          resultJson: canonicalJson({
            mutationId: Number(env.mutationId),
            status: outcome.status,
            ...(outcome.errorCode === undefined
              ? {}
              : { errorCode: outcome.errorCode }),
            ...(outcome.errorMessageSafe === undefined
              ? {}
              : { errorMessageSafe: outcome.errorMessageSafe }),
          }),
          scope: client.scope,
        })
        return result
      })
      results.push(result)
    }
    return results
  }

  test("sequential progression across batches derives lastMutationId", async () => {
    const client = freshClient()
    const first = await pushBatch(client, [envelope(1), envelope(2), envelope(3)])
    expect(first.map((r) => r.status)).toEqual(["applied", "applied", "applied"])
    expect(await lastMutationId(sql, client)).toBe(3)

    const second = await pushBatch(client, [envelope(4), envelope(5)])
    expect(second.map((r) => r.status)).toEqual(["applied", "applied"])
    expect(await lastMutationId(sql, client)).toBe(5)
    expect(await changelogCount(client.scope)).toBe(5)
    expect(await ledgerCount(client.clientGroupId, client.clientId)).toBe(5)
  })

  test("duplicate replay returns the recorded result and executes NOTHING", async () => {
    const client = freshClient()
    await pushBatch(client, [envelope(1)])
    expect(await changelogCount(client.scope)).toBe(1)

    let executed = 0
    const replay = await pushBatch(client, [envelope(1)], {
      onExecute: () => {
        executed += 1
      },
    })
    expect(replay[0]?.status).toBe("duplicate")
    expect(Number(replay[0]?.mutationId)).toBe(1)
    expect(executed).toBe(0)
    // No changelog side-effects, no new ledger rows, no ack movement.
    expect(await changelogCount(client.scope)).toBe(1)
    expect(await ledgerCount(client.clientGroupId, client.clientId)).toBe(1)
    expect(await lastMutationId(sql, client)).toBe(1)
  })

  test("interleaved duplicates within one batch: only the new envelope executes", async () => {
    const client = freshClient()
    await pushBatch(client, [envelope(1), envelope(2)])
    expect(await changelogCount(client.scope)).toBe(2)

    const executedIds: Array<number> = []
    const results = await pushBatch(
      client,
      [envelope(1), envelope(3), envelope(2)],
      { onExecute: (env) => executedIds.push(Number(env.mutationId)) },
    )
    expect(results.map((r) => r.status)).toEqual([
      "duplicate",
      "applied",
      "duplicate",
    ])
    expect(executedIds).toEqual([3])
    expect(await changelogCount(client.scope)).toBe(3)
    expect(await lastMutationId(sql, client)).toBe(3)
  })

  test("crash between execute and respond: replay answers duplicate with the SAME recorded result", async () => {
    const client = freshClient()
    // The server executed + committed (applied m1, rejected m2), then
    // "crashed" before the response reached the client — the commit IS the
    // recording, so nothing else is needed to simulate the crash.
    const original = await pushBatch(client, [envelope(1), envelope(2)], {
      outcome: (env) =>
        Number(env.mutationId) === 2
          ? {
              status: "rejected",
              errorCode: "validation_failed",
              errorMessageSafe: "title required",
            }
          : { status: "applied" },
    })
    expect(original.map((r) => r.status)).toEqual(["applied", "rejected"])
    expect(await changelogCount(client.scope)).toBe(1)

    // Client never saw the response; it replays the identical batch.
    let executed = 0
    const replay = await pushBatch(client, [envelope(1), envelope(2)], {
      onExecute: () => {
        executed += 1
      },
    })
    expect(executed).toBe(0)
    expect(replay.map((r) => r.status)).toEqual(["duplicate", "duplicate"])
    // The rejected outcome is preserved in-band on the duplicate.
    expect(replay[1]?.errorCode).toBe("validation_failed")
    expect(replay[1]?.errorMessageSafe).toBe("title required")
    expect(replay[0]?.errorCode).toBeUndefined()
    // Still exactly one changelog row and two ledger rows.
    expect(await changelogCount(client.scope)).toBe(1)
    expect(await ledgerCount(client.clientGroupId, client.clientId)).toBe(2)
    expect(await lastMutationId(sql, client)).toBe(2)
  })

  test("out-of-order gap: typed in-band rejection that acks NOTHING", async () => {
    const client = freshClient()
    await pushBatch(client, [envelope(1)])

    let executed = 0
    const results = await pushBatch(client, [envelope(3)], {
      onExecute: () => {
        executed += 1
      },
    })
    expect(executed).toBe(0)
    expect(results[0]?.status).toBe("rejected")
    expect(results[0]?.errorCode).toBe(OUT_OF_ORDER_ERROR_CODE)
    // Nothing recorded, nothing acked, no changelog side-effects.
    expect(await ledgerCount(client.clientGroupId, client.clientId)).toBe(1)
    expect(await lastMutationId(sql, client)).toBe(1)
    expect(await changelogCount(client.scope)).toBe(1)

    // The queue is not poisoned: pushing the missing prefix heals the gap.
    const healed = await pushBatch(client, [envelope(2), envelope(3)])
    expect(healed.map((r) => r.status)).toEqual(["applied", "applied"])
    expect(await lastMutationId(sql, client)).toBe(3)
  })

  test("recordMutation is insert-once: the first recording wins", async () => {
    const client = freshClient()
    const key = {
      clientGroupId: client.clientGroupId,
      clientId: client.clientId,
      mutationId: MutationId.make(1),
    }
    const first = await recordMutation(sql, {
      ...key,
      name: mutatorName,
      status: "applied",
      resultJson: canonicalJson({ mutationId: 1, status: "applied" }),
      scope: client.scope,
    })
    expect(first.inserted).toBe(true)

    const second = await recordMutation(sql, {
      ...key,
      name: mutatorName,
      status: "rejected",
      errorCode: "should_not_win",
    })
    expect(second.inserted).toBe(false)

    const recorded = await getMutation(sql, key)
    expect(recorded?.status).toBe("applied")
    expect(recorded?.errorCode).toBeUndefined()
    expect(recorded?.scope).toBe(String(client.scope))
    expect(recorded?.resultJson).toBe(
      canonicalJson({ mutationId: 1, status: "applied" }),
    )
    expect(new Date(recorded!.committedAt).toISOString()).toBe(
      recorded!.committedAt,
    )
  })

  test("getMutation returns null when nothing was recorded", async () => {
    const client = freshClient()
    expect(
      await getMutation(sql, {
        clientGroupId: client.clientGroupId,
        clientId: client.clientId,
        mutationId: MutationId.make(1),
      }),
    ).toBeNull()
    expect(await lastMutationId(sql, client)).toBe(0)
  })

  test("a ledger hole at or below lastMutationId is a constraint violation", async () => {
    const client = freshClient()
    await upsertClientState(sql, {
      clientGroupId: client.clientGroupId,
      userId: client.userId,
      schemaVersion,
    })
    // Manufacture a corrupt ledger: a recorded id 5 with no 1..4 beneath it
    // (checkAndReserve would never produce this).
    await recordMutation(sql, {
      clientGroupId: client.clientGroupId,
      clientId: client.clientId,
      mutationId: MutationId.make(5),
      name: mutatorName,
      status: "applied",
    })
    await expect(
      withSyncTransaction(sql, (writer) =>
        checkAndReserve(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          envelope: envelope(3),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
  })

  test("checkAndReserve requires the client-state row (upsertClientState first)", async () => {
    const client = freshClient()
    await expect(
      withSyncTransaction(sql, (writer) =>
        checkAndReserve(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          envelope: envelope(1),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
  })

  test("CONCURRENT pushes of the same envelope serialize on the state row: one executes, one answers duplicate", async () => {
    const client = freshClient()
    let executions = 0
    const pushSlow = () =>
      withSyncTransaction(sql, async (writer) => {
        await upsertClientState(writer.sql, {
          clientGroupId: client.clientGroupId,
          userId: client.userId,
          schemaVersion,
        })
        const gate = await checkAndReserve(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          envelope: envelope(1),
        })
        if (gate.kind !== "execute") return gate.result
        executions += 1
        // Hold the transaction open so the racer genuinely overlaps.
        await Bun.sleep(50)
        await recordMutation(writer.sql, {
          clientGroupId: client.clientGroupId,
          clientId: client.clientId,
          mutationId: MutationId.make(1),
          name: mutatorName,
          status: "applied",
        })
        return new MutationResult({
          mutationId: MutationId.make(1),
          status: "applied",
        })
      })
    const [a, b] = await Promise.all([pushSlow(), pushSlow()])
    expect(executions).toBe(1)
    expect([a.status, b.status].sort()).toEqual(["applied", "duplicate"])
    expect(await ledgerCount(client.clientGroupId, client.clientId)).toBe(1)
    expect(await lastMutationId(sql, client)).toBe(1)
  })

  test("client state: insert, refresh (last_seen_at bump), and user-mismatch rejection", async () => {
    const client = freshClient()
    const created = await upsertClientState(sql, {
      clientGroupId: client.clientGroupId,
      userId: "user-a",
      schemaVersion,
    })
    expect(created.created).toBe(true)

    const before = await sql`
      SELECT user_id, schema_version, created_at, last_seen_at
        FROM khala_sync_client_state
       WHERE client_group_id = ${client.clientGroupId}
    `
    await Bun.sleep(15)

    const refreshed = await upsertClientState(sql, {
      clientGroupId: client.clientGroupId,
      userId: "user-a",
      schemaVersion: SyncSchemaVersion.make(2),
    })
    expect(refreshed.created).toBe(false)

    const after = await sql`
      SELECT user_id, schema_version, created_at, last_seen_at
        FROM khala_sync_client_state
       WHERE client_group_id = ${client.clientGroupId}
    `
    expect(Number(after[0].schema_version)).toBe(2)
    expect(new Date(after[0].last_seen_at).getTime()).toBeGreaterThan(
      new Date(before[0].last_seen_at).getTime(),
    )
    expect(new Date(after[0].created_at).getTime()).toBe(
      new Date(before[0].created_at).getTime(),
    )

    // Client groups are user-bound: another user cannot claim the group.
    await expect(
      upsertClientState(sql, {
        clientGroupId: client.clientGroupId,
        userId: "user-b",
        schemaVersion,
      }),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncClientStateMismatchError",
      storedUserId: "user-a",
      requestedUserId: "user-b",
    })
    const mismatchError = await upsertClientState(sql, {
      clientGroupId: client.clientGroupId,
      userId: "user-b",
      schemaVersion,
    }).catch((e: unknown) => e)
    expect(mismatchError).toBeInstanceOf(KhalaSyncClientStateMismatchError)

    // The stored row is untouched by the rejected attempt.
    const untouched = await sql`
      SELECT user_id, schema_version, last_seen_at
        FROM khala_sync_client_state
       WHERE client_group_id = ${client.clientGroupId}
    `
    expect(untouched[0].user_id).toBe("user-a")
    expect(Number(untouched[0].schema_version)).toBe(2)
    expect(new Date(untouched[0].last_seen_at).getTime()).toBe(
      new Date(after[0].last_seen_at).getTime(),
    )
  })

  test("client state upsert works inside the mutator transaction and rolls back with it", async () => {
    const client = freshClient()
    class DomainRollback extends Error {}
    await expect(
      withSyncTransaction(sql, async (writer) => {
        await upsertClientState(writer.sql, {
          clientGroupId: client.clientGroupId,
          userId: "user-a",
          schemaVersion,
        })
        throw new DomainRollback("intentional rollback")
      }),
    ).rejects.toBeInstanceOf(DomainRollback)
    const rows = await sql`
      SELECT count(*)::int AS c FROM khala_sync_client_state
       WHERE client_group_id = ${client.clientGroupId}
    `
    expect(rows[0].c).toBe(0)
  })
})
