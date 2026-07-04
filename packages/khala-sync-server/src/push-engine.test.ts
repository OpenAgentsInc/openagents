import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutationResult,
  MutatorName,
  personalScope,
  PushRequest,
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { KhalaSyncStorageError } from "./errors.js"
import { runMigrations } from "./migrate.js"
import { KhalaSyncClientStateMismatchError } from "./mutation-ledger.js"
import {
  defineMutator,
  executePush,
  INVALID_ARGS_ERROR_CODE,
  makeMutatorRegistry,
  type MutatorRegistry,
  mutationRefFor,
  UNKNOWN_MUTATOR_ERROR_CODE,
} from "./push-engine.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const schemaVersion = SyncSchemaVersion.make(1)
const thingSet = MutatorName.make("thing.set")
const thingExplode = MutatorName.make("thing.explode")
const thingRejectEarly = MutatorName.make("thing.rejectEarly")
const thingEntityType = EntityType.make("thing")

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-push-${clientCounter}`),
    clientId: ClientId.make(`c-push-${clientCounter}`),
    userId: `user-push-${clientCounter}`,
  }
}

interface ThingSetArgs {
  readonly id: string
  readonly value: string
}

const decodeThingSetArgs = (argsJson: string): ThingSetArgs => {
  const raw: unknown = JSON.parse(argsJson)
  if (
    typeof raw !== "object" ||
    raw === null ||
    typeof (raw as { id?: unknown }).id !== "string" ||
    (raw as { id: string }).id.length === 0 ||
    typeof (raw as { value?: unknown }).value !== "string"
  ) {
    throw new Error("thing.set args must be { id, value }")
  }
  return raw as ThingSetArgs
}

const envelope = (
  id: number,
  name: MutatorName,
  args: unknown,
): MutationEnvelope =>
  new MutationEnvelope({
    argsJson: canonicalJson(args),
    mutationId: MutationId.make(id),
    name,
  })

const pushRequest = (
  client: { clientGroupId: ClientGroupId; clientId: ClientId },
  mutations: ReadonlyArray<MutationEnvelope>,
): PushRequest =>
  new PushRequest({
    clientGroupId: client.clientGroupId,
    clientId: client.clientId,
    mutations,
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion,
  })

describe.skipIf(!hasLocalPostgres())("push engine against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL
  let registry: MutatorRegistry
  /** Counts real mutator executions (must never grow on duplicates). */
  let executions = 0

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_push")
    await admin.end()
    const url = pg.urlFor("khala_sync_push")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    sql = new SQL({ url, max: 10 })

    // A business table so atomicity covers real business writes, not just
    // the changelog.
    await sql.unsafe(
      `CREATE TABLE things (
         scope text NOT NULL, id text NOT NULL, value text NOT NULL,
         PRIMARY KEY (scope, id)
       )`,
    )

    registry = makeMutatorRegistry([
      // The realistic mutator: permission check → business write →
      // changelog append, all through the transaction handle.
      defineMutator<ThingSetArgs>({
        decodeArgs: decodeThingSetArgs,
        execute: async (args, ctx) => {
          executions += 1
          const scope = personalScope(ctx.userId)
          await ctx.writer.sql`
            INSERT INTO things (scope, id, value)
            VALUES (${scope}, ${args.id}, ${args.value})
            ON CONFLICT (scope, id) DO UPDATE SET value = EXCLUDED.value
          `
          await ctx.writer.appendChange({
            entityId: EntityId.make(args.id),
            entityType: thingEntityType,
            mutationRef: ctx.mutationRef,
            op: "upsert",
            postImage: { id: args.id, value: args.value },
            scope,
          })
          return new MutationResult({
            mutationId: ctx.mutationId,
            status: "applied",
          })
        },
        name: thingSet,
      }),
      // Writes business row + changelog, THEN throws: the whole
      // transaction (business write, changelog, ledger, scope counter)
      // must roll back.
      defineMutator<ThingSetArgs>({
        decodeArgs: decodeThingSetArgs,
        execute: async (args, ctx) => {
          const scope = personalScope(ctx.userId)
          await ctx.writer.sql`
            INSERT INTO things (scope, id, value)
            VALUES (${scope}, ${args.id}, ${args.value})
          `
          await ctx.writer.appendChange({
            entityId: EntityId.make(args.id),
            entityType: thingEntityType,
            mutationRef: ctx.mutationRef,
            op: "upsert",
            postImage: { id: args.id, value: args.value },
            scope,
          })
          throw new KhalaSyncStorageError(
            "unavailable",
            "simulated storage failure after writes",
          )
        },
        name: thingExplode,
      }),
      // Rejects before writing anything (the documented mutator contract).
      defineMutator<ThingSetArgs>({
        decodeArgs: decodeThingSetArgs,
        execute: async (_args, ctx) =>
          new MutationResult({
            errorCode: "validation_failed",
            errorMessageSafe: "value not allowed",
            mutationId: ctx.mutationId,
            status: "rejected",
          }),
        name: thingRejectEarly,
      }),
    ])
  })

  afterAll(async () => {
    await sql?.end()
    await pg?.stop()
  })

  const changelogRows = async (scope: SyncScope) => {
    const rows: Array<{
      version: string | number | bigint
      entity_id: string
      mutation_ref: string | null
      post_image_json: unknown
    }> = await sql`
      SELECT version, entity_id, mutation_ref, post_image_json
        FROM khala_sync_changelog WHERE scope = ${scope}
       ORDER BY version
    `
    return rows.map((r) => ({ ...r, version: Number(r.version) }))
  }

  const ledgerRows = async (client: {
    clientGroupId: ClientGroupId
    clientId: ClientId
  }) => {
    const rows: Array<{
      mutation_id: string | number | bigint
      status: string
      error_code: string | null
    }> = await sql`
      SELECT mutation_id, status, error_code FROM khala_sync_mutations
       WHERE client_group_id = ${client.clientGroupId}
         AND client_id = ${client.clientId}
       ORDER BY mutation_id
    `
    return rows.map((r) => ({ ...r, mutation_id: Number(r.mutation_id) }))
  }

  const businessRows = async (scope: SyncScope) => {
    const rows: Array<{ id: string; value: string }> = await sql`
      SELECT id, value FROM things WHERE scope = ${scope} ORDER BY id
    `
    return rows
  }

  test("applied flow: business write + changelog + ledger commit together, dense versions", async () => {
    const client = freshClient()
    const scope = personalScope(client.userId)
    const response = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "t1", value: "one" }),
        envelope(2, thingSet, { id: "t2", value: "two" }),
      ]),
      sql,
      userId: client.userId,
    })

    expect(response.protocolVersion).toBe(KHALA_SYNC_PROTOCOL_VERSION)
    expect(response.results.map((r) => r.status)).toEqual([
      "applied",
      "applied",
    ])
    expect(response.results.map((r) => Number(r.mutationId))).toEqual([1, 2])
    expect(response.lastMutationId).toBe(2)

    // Dense, monotonic per-scope versions (one transaction per envelope).
    const log = await changelogRows(scope)
    expect(log.map((r) => r.version)).toEqual([1, 2])
    expect(log.map((r) => r.entity_id)).toEqual(["t1", "t2"])
    expect(log[0]!.mutation_ref).toBe(
      mutationRefFor(client.clientGroupId, client.clientId, MutationId.make(1)),
    )
    expect(await businessRows(scope)).toEqual([
      { id: "t1", value: "one" },
      { id: "t2", value: "two" },
    ])
    expect(await ledgerRows(client)).toEqual([
      { error_code: null, mutation_id: 1, status: "applied" },
      { error_code: null, mutation_id: 2, status: "applied" },
    ])
  })

  test("rejected flow: in-band result, recorded in the ledger, no changelog entry", async () => {
    const client = freshClient()
    const scope = personalScope(client.userId)
    const response = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingRejectEarly, { id: "r1", value: "nope" }),
      ]),
      sql,
      userId: client.userId,
    })

    const result = response.results[0]!
    expect(result.status).toBe("rejected")
    expect(result.errorCode).toBe("validation_failed")
    expect(result.errorMessageSafe).toBe("value not allowed")
    // Rejections ACK the mutation: lastMutationId advances.
    expect(response.lastMutationId).toBe(1)
    expect(await changelogRows(scope)).toEqual([])
    expect(await ledgerRows(client)).toEqual([
      { error_code: "validation_failed", mutation_id: 1, status: "rejected" },
    ])
  })

  test("unknown mutator and bad args are recorded in-band rejections that ack", async () => {
    const client = freshClient()
    const response = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, MutatorName.make("no.suchMutator"), { x: 1 }),
        envelope(2, thingSet, { bogus: true }),
        envelope(3, thingSet, { id: "ok", value: "after-rejections" }),
      ]),
      sql,
      userId: client.userId,
    })

    expect(response.results.map((r) => r.status)).toEqual([
      "rejected",
      "rejected",
      "applied",
    ])
    expect(response.results[0]!.errorCode).toBe(UNKNOWN_MUTATOR_ERROR_CODE)
    expect(response.results[1]!.errorCode).toBe(INVALID_ARGS_ERROR_CODE)
    // Raw args never leak into the safe message.
    expect(response.results[1]!.errorMessageSafe).not.toContain("bogus")
    expect(response.lastMutationId).toBe(3)
    expect((await ledgerRows(client)).map((r) => r.status)).toEqual([
      "rejected",
      "rejected",
      "applied",
    ])
  })

  test("duplicate: replayed envelope answers from the recording without re-executing", async () => {
    const client = freshClient()
    const scope = personalScope(client.userId)
    const first = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "d1", value: "v1" }),
      ]),
      sql,
      userId: client.userId,
    })
    expect(first.results[0]!.status).toBe("applied")
    const executionsAfterFirst = executions

    // Full-batch replay (crash between execute and respond).
    const replay = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "d1", value: "v1" }),
      ]),
      sql,
      userId: client.userId,
    })
    expect(replay.results[0]!.status).toBe("duplicate")
    expect(replay.lastMutationId).toBe(1)
    expect(executions).toBe(executionsAfterFirst)
    expect((await changelogRows(scope)).length).toBe(1)
    expect((await ledgerRows(client)).length).toBe(1)
  })

  test("duplicate of a recorded rejection carries the recorded error in-band", async () => {
    const client = freshClient()
    await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingRejectEarly, { id: "x", value: "x" }),
      ]),
      sql,
      userId: client.userId,
    })
    const replay = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingRejectEarly, { id: "x", value: "x" }),
      ]),
      sql,
      userId: client.userId,
    })
    const result = replay.results[0]!
    expect(result.status).toBe("duplicate")
    expect(result.errorCode).toBe("validation_failed")
    expect(result.errorMessageSafe).toBe("value not allowed")
  })

  test("out_of_order: in-band rejection, acks nothing, gap heals on re-push", async () => {
    const client = freshClient()
    const first = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "o1", value: "v" }),
        envelope(3, thingSet, { id: "o3", value: "v" }),
      ]),
      sql,
      userId: client.userId,
    })
    expect(first.results.map((r) => r.status)).toEqual(["applied", "rejected"])
    expect(first.results[1]!.errorCode).toBe("out_of_order")
    // out_of_order acks NOTHING: no ledger row, watermark stays.
    expect(first.lastMutationId).toBe(1)
    expect((await ledgerRows(client)).map((r) => r.mutation_id)).toEqual([1])

    // The client re-pushes the missing prefix: the gap heals.
    const healed = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(2, thingSet, { id: "o2", value: "v" }),
        envelope(3, thingSet, { id: "o3", value: "v" }),
      ]),
      sql,
      userId: client.userId,
    })
    expect(healed.results.map((r) => r.status)).toEqual(["applied", "applied"])
    expect(healed.lastMutationId).toBe(3)
  })

  test("results come back in request order", async () => {
    const client = freshClient()
    await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "s1", value: "v" }),
      ]),
      sql,
      userId: client.userId,
    })
    const response = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "s1", value: "v" }), // duplicate
        envelope(2, thingRejectEarly, { id: "s2", value: "v" }), // rejected
        envelope(4, thingSet, { id: "s4", value: "v" }), // out_of_order
        envelope(3, thingSet, { id: "s3", value: "v" }), // applied
      ]),
      sql,
      userId: client.userId,
    })
    expect(response.results.map((r) => Number(r.mutationId))).toEqual([
      1, 2, 4, 3,
    ])
    expect(response.results.map((r) => r.status)).toEqual([
      "duplicate",
      "rejected",
      "rejected",
      "applied",
    ])
    expect(response.lastMutationId).toBe(3)
  })

  test("atomicity: a mid-mutator storage failure rolls back business write, changelog, ledger, and version — batch aborts, prefix stays", async () => {
    const client = freshClient()
    const scope = personalScope(client.userId)

    await expect(
      executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, thingSet, { id: "a1", value: "committed" }),
          envelope(2, thingExplode, { id: "a2", value: "rolled-back" }),
          envelope(3, thingSet, { id: "a3", value: "never-reached" }),
        ]),
        sql,
        userId: client.userId,
      }),
    ).rejects.toBeInstanceOf(KhalaSyncStorageError)

    // The committed prefix (mutation 1) stays; the failed transaction left
    // NOTHING behind — no business row, no changelog entry, no ledger row.
    expect(await businessRows(scope)).toEqual([
      { id: "a1", value: "committed" },
    ])
    expect((await changelogRows(scope)).map((r) => r.version)).toEqual([1])
    expect((await ledgerRows(client)).map((r) => r.mutation_id)).toEqual([1])

    // The retry re-acks 1 as duplicate and versions stay DENSE (the rolled
    // back allocation left no gap): the next committed version is 2.
    const retry = await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "a1", value: "committed" }),
        envelope(2, thingSet, { id: "a2", value: "second-try" }),
        envelope(3, thingSet, { id: "a3", value: "reached" }),
      ]),
      sql,
      userId: client.userId,
    })
    expect(retry.results.map((r) => r.status)).toEqual([
      "duplicate",
      "applied",
      "applied",
    ])
    expect((await changelogRows(scope)).map((r) => r.version)).toEqual([
      1, 2, 3,
    ])
    expect(retry.lastMutationId).toBe(3)
  })

  test("client group bound to another user: whole request fails typed, nothing executes", async () => {
    const client = freshClient()
    await executePush({
      registry,
      request: pushRequest(client, [
        envelope(1, thingSet, { id: "m1", value: "v" }),
      ]),
      sql,
      userId: client.userId,
    })

    const executionsBefore = executions
    await expect(
      executePush({
        registry,
        request: pushRequest(client, [
          envelope(2, thingSet, { id: "m2", value: "stolen" }),
        ]),
        sql,
        userId: "some-other-user",
      }),
    ).rejects.toBeInstanceOf(KhalaSyncClientStateMismatchError)
    expect(executions).toBe(executionsBefore)
    expect((await ledgerRows(client)).map((r) => r.mutation_id)).toEqual([1])
  })

  test("empty push still binds the client group and reports the watermark", async () => {
    const client = freshClient()
    const response = await executePush({
      registry,
      request: pushRequest(client, []),
      sql,
      userId: client.userId,
    })
    expect(response.results).toEqual([])
    expect(response.lastMutationId).toBe(0)

    const rows: Array<{ user_id: string }> = await sql`
      SELECT user_id FROM khala_sync_client_state
       WHERE client_group_id = ${client.clientGroupId}
    `
    expect(rows[0]?.user_id).toBe(client.userId)

    // And the empty push STILL enforces the user binding.
    await expect(
      executePush({
        registry,
        request: pushRequest(client, []),
        sql,
        userId: "someone-else",
      }),
    ).rejects.toBeInstanceOf(KhalaSyncClientStateMismatchError)
  })
})
