import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  fleetRunScope,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutatorName,
  PushRequest,
  SyncSchemaVersion,
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
import { readPendingFleetIntents } from "./fleet-intents.js"
import {
  FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME,
  FLEET_PAUSE_RUN_MUTATOR_NAME,
  FLEET_PAUSE_WORKER_MUTATOR_NAME,
  FLEET_RESUME_RUN_MUTATOR_NAME,
  FLEET_RESUME_WORKER_MUTATOR_NAME,
  FLEET_SCOPE_REJECTION,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  FLEET_STOP_CONFIRMATION_REJECTION,
  FLEET_STOP_RUN_MUTATOR_NAME,
  fleetOperatorMutators,
} from "./fleet-mutators.js"
import {
  FLEET_POST_IMAGE_FORBIDDEN_PATTERN,
  fleetWorkerPostImage,
  projectFleetEntitiesBestEffort,
  readScopeOwner,
} from "./fleet-projection.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

/**
 * KS-3.2 (#8292) fleet operator mutators: per-worker pause/resume, inbox
 * flag acknowledgment, and the confirmed terminal stop — through
 * `executePush` against real local Postgres, same discipline as the KS-6.1
 * trio in fleet-projection.test.ts.
 */

const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-fleet2-${clientCounter}`),
    clientId: ClientId.make(`c-fleet2-${clientCounter}`),
    userId: `user-fleet2-${clientCounter}`,
  }
}

const envelope = (id: number, name: string, args: unknown): MutationEnvelope =>
  new MutationEnvelope({
    argsJson: canonicalJson(args),
    mutationId: MutationId.make(id),
    name: MutatorName.make(name),
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

// ---------------------------------------------------------------------------
// Registry surface (no database needed)
// ---------------------------------------------------------------------------

describe("fleet operator mutator set", () => {
  test("carries the full KS-6.1 + KS-3.2 catalog with unique names", () => {
    const names = fleetOperatorMutators.map((m) => String(m.name))
    expect(names).toEqual([
      FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
      FLEET_PAUSE_RUN_MUTATOR_NAME,
      FLEET_RESUME_RUN_MUTATOR_NAME,
      FLEET_PAUSE_WORKER_MUTATOR_NAME,
      FLEET_RESUME_WORKER_MUTATOR_NAME,
      FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME,
      FLEET_STOP_RUN_MUTATOR_NAME,
    ])
    // Registry construction throws on duplicates — building it is the check.
    expect(() => makeMutatorRegistry([...fleetOperatorMutators])).not.toThrow()
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "KS-3.2 fleet mutators against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_fleet2")
      await admin.end()
      const url = pg.urlFor("khala_sync_fleet2")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0005_khala_sync_fleet_intents_v2.sql")
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    const registry = makeMutatorRegistry([...fleetOperatorMutators])
    const nowIso = "2026-07-04T15:20:11.412Z"

    const latestPostImage = async (
      scope: string,
      entityType: string,
      entityId: string,
    ): Promise<Record<string, unknown>> => {
      const rows: Array<{ post_image_json: object | string }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${scope} AND entity_type = ${entityType}
          AND entity_id = ${entityId} AND op = 'upsert'
        ORDER BY version DESC LIMIT 1
      `
      const raw = rows[0]!.post_image_json
      return (
        typeof raw === "string" ? JSON.parse(raw) : raw
      ) as Record<string, unknown>
    }

    test("pauseWorker/resumeWorker: intents carry the worker id; fleet_worker post-images land", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.worker"
      const scope = fleetRunScope(runId)
      const workerId = "dispatch-context.pylon.supervisor.9ab31c44"

      // Seed a projected worker so the mutators patch REAL state (and must
      // preserve the allowlisted fields they do not own).
      const seeded = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetWorkerPostImage({
              accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f",
              currentAssignmentRef: "assignment.public.issue8292.1",
              id: workerId,
              status: "dispatched",
              updatedAt: nowIso,
            }),
            kind: "fleet_worker",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId,
        sql: sql as unknown as SyncSql,
      })
      expect(seeded.ok).toBe(true)

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_PAUSE_WORKER_MUTATOR_NAME, { runId, workerId }),
          envelope(2, FLEET_RESUME_WORKER_MUTATOR_NAME, { runId, workerId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual([
        "applied",
        "applied",
      ])

      const intents: Array<{
        intent: string
        worker_id: string | null
        flag_ref: string | null
        desired_slots: number | null
      }> = await sql`
        SELECT intent, worker_id, flag_ref, desired_slots
        FROM khala_sync_fleet_intents WHERE scope = ${scope} ORDER BY id
      `
      expect(intents.map((i) => i.intent)).toEqual([
        "pause_worker",
        "resume_worker",
      ])
      for (const intent of intents) {
        expect(intent.worker_id).toBe(workerId)
        expect(intent.flag_ref).toBeNull()
        expect(intent.desired_slots).toBeNull()
      }

      // Post-image history: seeded dispatched → paused → idle, preserving
      // the allowlisted fields the mutator does not own.
      const image = await latestPostImage(scope, "fleet_worker", workerId)
      expect(image.phase).toBe("idle")
      expect(image.assignmentRef).toBe("assignment.public.issue8292.1")
      expect(image.accountRefHash).toBe("account.pylon.codex.4e5f6a7b8c9d0e1f")
      const phases: Array<{ post_image_json: object | string }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${scope} AND entity_type = 'fleet_worker'
        ORDER BY version ASC
      `
      expect(
        phases.map((row) => {
          const value =
            typeof row.post_image_json === "string"
              ? (JSON.parse(row.post_image_json) as { phase: string })
              : (row.post_image_json as { phase: string })
          return value.phase
        }),
      ).toEqual(["dispatched", "paused", "idle"])
    })

    test("pauseWorker on a never-projected worker synthesizes a minimal baseline", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.worker-baseline"
      const scope = fleetRunScope(runId)
      const workerId = "dispatch-context.pylon.supervisor.baseline1"
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_PAUSE_WORKER_MUTATOR_NAME, { runId, workerId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results[0]!.status).toBe("applied")
      const image = await latestPostImage(scope, "fleet_worker", workerId)
      expect(image.phase).toBe("paused")
      expect(image.workerId).toBe(workerId)
      expect(image.assignmentRef).toBeUndefined()
    })

    test("acknowledgeInboxFlag: unseen flag records a durable unclassified ack; seen flag preserves kind/openedAt", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.flags"
      const scope = fleetRunScope(runId)

      // Seed one OPEN flag through the projection seam.
      const seeded = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: (await import("@openagentsinc/khala-sync"))
              .decodeFleetInboxFlagEntity({
                flagRef: "inbox-flag.run_blocked.4f2a9c1d",
                kind: "run_blocked",
                openedAt: nowIso,
                status: "open",
                updatedAt: nowIso,
              }),
            kind: "fleet_inbox_flag",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId,
        sql: sql as unknown as SyncSql,
      })
      expect(seeded.ok).toBe(true)

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME, {
            flagRef: "inbox-flag.run_blocked.4f2a9c1d",
            runId,
          }),
          envelope(2, FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME, {
            flagRef: "inbox-flag.never-projected.1",
            runId,
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual([
        "applied",
        "applied",
      ])

      const known = await latestPostImage(
        scope,
        "fleet_inbox_flag",
        "inbox-flag.run_blocked.4f2a9c1d",
      )
      expect(known.status).toBe("acknowledged")
      expect(known.kind).toBe("run_blocked")
      expect(known.openedAt).toBe(nowIso)
      expect(typeof known.acknowledgedAt).toBe("string")

      const unseen = await latestPostImage(
        scope,
        "fleet_inbox_flag",
        "inbox-flag.never-projected.1",
      )
      expect(unseen.status).toBe("acknowledged")
      expect(unseen.kind).toBe("unclassified")

      const intents: Array<{ intent: string; flag_ref: string | null }> =
        await sql`
        SELECT intent, flag_ref FROM khala_sync_fleet_intents
        WHERE scope = ${scope} ORDER BY id
      `
      expect(intents.map((i) => i.intent)).toEqual([
        "acknowledge_inbox_flag",
        "acknowledge_inbox_flag",
      ])
      expect(intents[0]!.flag_ref).toBe("inbox-flag.run_blocked.4f2a9c1d")
    })

    test("stopRun without confirm: true rejects in-band with ZERO writes — not even a scope claim", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.stop-unconfirmed"
      const scope = fleetRunScope(runId)
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_STOP_RUN_MUTATOR_NAME, { confirm: false, runId }),
          // The queue must keep draining behind the rejection.
          envelope(2, FLEET_PAUSE_RUN_MUTATOR_NAME, {
            runId: "fleet-run.mut2.stop-unconfirmed-other",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe(
        FLEET_STOP_CONFIRMATION_REJECTION,
      )
      expect(response.results[1]!.status).toBe("applied")
      expect(Number(response.lastMutationId)).toBe(2)

      // Zero writes: no changelog rows, no intents, no ownership claim.
      const changes: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog WHERE scope = ${scope}
      `
      expect(Number(changes[0]!.count)).toBe(0)
      const intents: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_fleet_intents
        WHERE scope = ${scope}
      `
      expect(Number(intents[0]!.count)).toBe(0)
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBeNull()

      // Missing confirm entirely is an invalid_args rejection (schema gate).
      const missing = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(3, FLEET_STOP_RUN_MUTATOR_NAME, { runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(missing.results[0]!.status).toBe("rejected")
      expect(missing.results[0]!.errorCode).toBe("invalid_args")
    })

    test("stopRun with confirm: true is terminal — status stopped, desiredSlots 0, intent recorded", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.stop"
      const scope = fleetRunScope(runId)
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME, {
            desiredSlots: 5,
            runId,
          }),
          envelope(2, FLEET_STOP_RUN_MUTATOR_NAME, { confirm: true, runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual([
        "applied",
        "applied",
      ])
      const image = await latestPostImage(scope, "fleet_run", runId)
      expect(image.status).toBe("stopped")
      expect(image.desiredSlots).toBe(0)
      const intents: Array<{ intent: string; desired_slots: number | null }> =
        await sql`
        SELECT intent, desired_slots FROM khala_sync_fleet_intents
        WHERE scope = ${scope} ORDER BY id
      `
      expect(intents.map((i) => i.intent)).toEqual([
        "set_desired_slots",
        "stop",
      ])
      // The terminal intent never carries a slot count (0005 CHECK shape).
      expect(intents[1]!.desired_slots).toBeNull()
    })

    test("a FOREIGN user is rejected in-band on every new mutator with zero writes", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const runId = "fleet-run.mut2.foreign"
      const scope = fleetRunScope(runId)
      await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(1, FLEET_PAUSE_RUN_MUTATOR_NAME, { runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      const before: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog WHERE scope = ${scope}
      `
      const response = await executePush({
        registry,
        request: pushRequest(intruder, [
          envelope(1, FLEET_PAUSE_WORKER_MUTATOR_NAME, {
            runId,
            workerId: "dispatch-context.x.1",
          }),
          envelope(2, FLEET_RESUME_WORKER_MUTATOR_NAME, {
            runId,
            workerId: "dispatch-context.x.1",
          }),
          envelope(3, FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME, {
            flagRef: "inbox-flag.x.1",
            runId,
          }),
          envelope(4, FLEET_STOP_RUN_MUTATOR_NAME, { confirm: true, runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual([
        "rejected",
        "rejected",
        "rejected",
        "rejected",
      ])
      for (const result of response.results) {
        expect(result.errorCode).toBe(FLEET_SCOPE_REJECTION)
      }
      const after: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog WHERE scope = ${scope}
      `
      expect(Number(after[0]!.count)).toBe(Number(before[0]!.count))
      const intents: Array<{ requested_by_user_id: string }> = await sql`
        SELECT requested_by_user_id FROM khala_sync_fleet_intents
        WHERE scope = ${scope}
      `
      expect(
        intents.every((i) => i.requested_by_user_id === owner.userId),
      ).toBe(true)
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBe(
        owner.userId,
      )
    })

    test("post-images from the new mutators never carry forbidden material", async () => {
      const scopes = [
        fleetRunScope("fleet-run.mut2.worker"),
        fleetRunScope("fleet-run.mut2.flags"),
        fleetRunScope("fleet-run.mut2.stop"),
      ]
      for (const scope of scopes) {
        const rows: Array<{ post_image_json: object | string | null }> =
          await sql`
          SELECT post_image_json FROM khala_sync_changelog WHERE scope = ${scope}
        `
        for (const row of rows) {
          if (row.post_image_json === null) continue
          const serialized =
            typeof row.post_image_json === "string"
              ? row.post_image_json
              : JSON.stringify(row.post_image_json)
          expect(serialized).not.toMatch(FLEET_POST_IMAGE_FORBIDDEN_PATTERN)
        }
      }
    })

    test("readPendingFleetIntents: watermark resume + scope filter + typed rows", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut2.reader"
      const scope = fleetRunScope(runId)
      await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME, {
            desiredSlots: 3,
            runId,
          }),
          envelope(2, FLEET_PAUSE_WORKER_MUTATOR_NAME, {
            runId,
            workerId: "dispatch-context.reader.1",
          }),
          envelope(3, FLEET_STOP_RUN_MUTATOR_NAME, { confirm: true, runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      const s = sql as unknown as SyncSql
      const all = await readPendingFleetIntents(s, { scope })
      expect(all.map((i) => i.intent)).toEqual([
        "set_desired_slots",
        "pause_worker",
        "stop",
      ])
      expect(all[0]!.desiredSlots).toBe(3)
      expect(all[1]!.workerId).toBe("dispatch-context.reader.1")
      expect(all[1]!.flagRef).toBeNull()
      expect(all[2]!.requestedByUserId).toBe(client.userId)
      expect(all[0]!.mutationRef).toMatch(/^mutation:/)
      expect(all[0]!.createdAt).toMatch(/^\d{4}-\d{2}-\d{2}T/)
      // ids are monotonic — the poller's watermark.
      expect(all[1]!.id).toBeGreaterThan(all[0]!.id)

      // Resume from a watermark: only rows after it.
      const rest = await readPendingFleetIntents(s, {
        afterId: all[0]!.id,
        scope,
      })
      expect(rest.map((i) => i.intent)).toEqual(["pause_worker", "stop"])

      // Scope filter: another run's intents are invisible.
      const other = await readPendingFleetIntents(s, {
        scope: fleetRunScope("fleet-run.mut2.reader-none"),
      })
      expect(other).toEqual([])

      // Limit clamps and pages oldest-first.
      const paged = await readPendingFleetIntents(s, { limit: 1, scope })
      expect(paged).toHaveLength(1)
      expect(paged[0]!.intent).toBe("set_desired_slots")
    })
  },
)
