import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  decodeFleetApprovalEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
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
import {
  fleetApprovalPostImage,
  fleetRunPostImage,
  fleetWorkerPostImage,
  projectFleetEntitiesBestEffort,
} from "./fleet-projection.js"
import {
  readPendingFleetSteeringIntents,
} from "./fleet-steering-intents.js"
import {
  FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME,
  FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME,
  FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME,
  FLEET_STEERING_INTENT_EXISTS_REJECTION,
  FLEET_STEERING_KIND_REJECTION,
  FLEET_STEERING_BODY_SHAPE_REJECTION,
  FLEET_STEERING_MAX_INTENTS_PER_RUN,
  FLEET_STEERING_RUN_LIMIT_REJECTION,
  FLEET_STEERING_SCOPE_REJECTION,
  fleetSteeringMutators,
} from "./fleet-steering.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

/**
 * MH-6 (#8585): the three MH-0 typed fleet steering mutators
 * (`khala.fleet_intent.v1`) over Khala Sync, proven against real local
 * Postgres. The integration test is the fixture-form of the issue's five-step
 * phone dogfood receipt (a live device is not reachable in this harness):
 *
 *   (a) desktop starts a mixed fixture FleetRun (codex + claude + grok workers)
 *   (b) phone reads workers + states via the projection within the scope
 *   (c) phone requests pause → effective run state stays unchanged until ACK
 *   (d) phone requests approval → effective approval stays pending until ACK
 *   (e) every request has a durable receipt for the accepted Pylon exchange
 */

const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-fleet-steer-${clientCounter}`),
    clientId: ClientId.make(`c-fleet-steer-${clientCounter}`),
    userId: `user-fleet-steer-${clientCounter}`,
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

const SCHEMA = "khala.fleet_intent.v1" as const

const runControlIntent = (input: {
  intentId: string
  runRef: string
  action: "pause" | "resume" | "drain" | "stop"
  surface?: "mobile" | "desktop"
}) => ({
  action: input.action,
  createdAt: "2026-07-08T18:00:00.000Z",
  idempotencyKey: `${input.intentId}.idem`,
  intentId: input.intentId,
  kind: "fleet_run_control" as const,
  origin: { surface: input.surface ?? "mobile" },
  runRef: input.runRef,
  schema: SCHEMA,
})

const approvalIntent = (input: {
  intentId: string
  runRef: string
  approvalRef: string
  decision: "allow" | "deny"
}) => ({
  approvalRef: input.approvalRef,
  createdAt: "2026-07-08T18:00:05.000Z",
  decision: input.decision,
  idempotencyKey: `${input.intentId}.idem`,
  intentId: input.intentId,
  kind: "approval_decision" as const,
  origin: { surface: "mobile" as const },
  runRef: input.runRef,
  schema: SCHEMA,
})

const steerIntent = (input: {
  intentId: string
  runRef: string
  body?: string
  targetRef?: string
}) => ({
  ...(input.body === undefined ? {} : { body: input.body }),
  createdAt: "2026-07-08T18:00:10.000Z",
  idempotencyKey: `${input.intentId}.idem`,
  intentId: input.intentId,
  kind: "steer_message" as const,
  origin: { surface: "mobile" as const },
  runRef: input.runRef,
  schema: SCHEMA,
  ...(input.targetRef === undefined ? {} : { targetRef: input.targetRef }),
})

// ---------------------------------------------------------------------------
// Registry surface (no database needed)
// ---------------------------------------------------------------------------

describe("fleet steering mutator set", () => {
  test("carries the three MH-0 typed intents with unique names", () => {
    const names = fleetSteeringMutators.map((m) => String(m.name))
    expect(names).toEqual([
      FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME,
      FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME,
      FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME,
    ])
    expect(() => makeMutatorRegistry([...fleetSteeringMutators])).not.toThrow()
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "MH-6 fleet steering against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_fleet_steering")
      await admin.end()
      const url = pg.urlFor("khala_sync_fleet_steering")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0050_khala_sync_fleet_steering.sql")
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    const registry = makeMutatorRegistry([...fleetSteeringMutators])
    const nowIso = "2026-07-08T17:59:00.000Z"

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

    test("request-only phone flow keeps effective run and approval state unchanged until Pylon ACK", async () => {
      const client = freshClient()
      const runRef = "fleet.mh6.dogfood"
      const scope = fleetRunScope(runRef)
      const approvalRef = "approval.mh6.claude.bash.1"
      const blockedWorkerId = "worker.mh6.claude.7f3a1b2c"

      // --- (a) desktop starts a mixed fixture FleetRun -------------------
      const started = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              counters: { activeAssignments: 3, workUnitsTotal: 9 },
              runRef,
              startedAt: nowIso,
              state: "running",
              targetConcurrency: 3,
              updatedAt: nowIso,
              // mixed dispatch — the run picks per work-unit
              workerKind: "auto",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
          {
            entity: fleetWorkerPostImage({
              accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f",
              harnessKind: "codex",
              id: "worker.mh6.codex.1a2b3c4d",
              status: "dispatched",
              updatedAt: nowIso,
            }),
            kind: "fleet_worker",
            op: "upsert",
          },
          {
            entity: fleetWorkerPostImage({
              accountRefHash: "account.pylon.claude.aa11bb22cc33dd44",
              harnessKind: "claude",
              id: blockedWorkerId,
              status: "blocked",
              updatedAt: nowIso,
            }),
            kind: "fleet_worker",
            op: "upsert",
          },
          {
            entity: fleetWorkerPostImage({
              accountRefHash: "account.pylon.grok.99887766554433aa",
              harnessKind: "grok",
              id: "worker.mh6.grok.5e6f7a8b",
              status: "dispatched",
              updatedAt: nowIso,
            }),
            kind: "fleet_worker",
            op: "upsert",
          },
          {
            entity: fleetApprovalPostImage({
              approvalRef,
              openedAt: nowIso,
              status: "pending",
              toolClass: "bash",
              updatedAt: nowIso,
              workerId: blockedWorkerId,
            }),
            kind: "fleet_approval",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId: runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(started.ok).toBe(true)

      // --- (b) phone shows workers + states via the projection -----------
      const workerRows: Array<{ post_image_json: object | string }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${scope} AND entity_type = 'fleet_worker' AND op = 'upsert'
        ORDER BY entity_id ASC
      `
      const workers = workerRows.map((r) =>
        decodeFleetWorkerEntity(
          typeof r.post_image_json === "string"
            ? JSON.parse(r.post_image_json)
            : r.post_image_json,
        ),
      )
      expect(workers.map((w) => w.harnessKind).sort()).toEqual([
        "claude",
        "codex",
        "grok",
      ])
      expect(
        workers.find((w) => w.workerId === blockedWorkerId)?.phase,
      ).toBe("blocked")

      // --- (c) phone pauses the run --------------------------------------
      const pause = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, runControlIntent({
            action: "pause",
            intentId: "intent.mh6.pause.1",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(pause.results.map((r) => r.status)).toEqual(["applied"])

      const runImageAfterPause = decodeFleetRunEntity(
        await latestPostImage(scope, "fleet_run", runRef),
      )
      expect(runImageAfterPause.status).toBe("running")

      // ...desktop authority OBSERVES the pause via the watermark reader.
      const observedAfterStart = await readPendingFleetSteeringIntents(
        sql as unknown as SyncSql,
        { scope },
      )
      expect(observedAfterStart.map((i) => i.kind)).toEqual([
        "fleet_run_control",
      ])
      expect(observedAfterStart[0]!.action).toBe("pause")
      expect(observedAfterStart[0]!.intent.kind).toBe("fleet_run_control")
      const pauseSeq = observedAfterStart[0]!.seq

      // --- (d) phone approves the pending tool ---------------------------
      const approve = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(2, FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME, approvalIntent({
            approvalRef,
            decision: "allow",
            intentId: "intent.mh6.approve.1",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(approve.results.map((r) => r.status)).toEqual(["applied"])

      const approvalImage = decodeFleetApprovalEntity(
        await latestPostImage(scope, "fleet_approval", approvalRef),
      )
      expect(approvalImage.status).toBe("pending")
      expect(approvalImage.workerId).toBe(blockedWorkerId)
      expect(approvalImage.toolClass).toBe("bash")
      expect(approvalImage.decidedAt).toBeUndefined()

      // ...authority observes the approval (seq past the pause) and RESUMES
      // the blocked worker — the worker continues.
      const observedAfterPause = await readPendingFleetSteeringIntents(
        sql as unknown as SyncSql,
        { afterSeq: pauseSeq, scope },
      )
      expect(observedAfterPause.map((i) => i.kind)).toEqual([
        "approval_decision",
      ])
      expect(observedAfterPause[0]!.decision).toBe("allow")
      const resumed = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetWorkerPostImage({
              accountRefHash: "account.pylon.claude.aa11bb22cc33dd44",
              harnessKind: "claude",
              id: blockedWorkerId,
              status: "dispatched",
              updatedAt: "2026-07-08T18:01:00.000Z",
            }),
            kind: "fleet_worker",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId: runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(resumed.ok).toBe(true)
      const workerAfter = decodeFleetWorkerEntity(
        await latestPostImage(scope, "fleet_worker", blockedWorkerId),
      )
      expect(workerAfter.phase).toBe("dispatched")

      // --- (e) every event has a receipt row AND a changelog row ---------
      const receipts: Array<{
        kind: string
        action: string | null
        approval_ref: string | null
        decision: string | null
        surface: string
        intent_json: object | string
      }> = await sql`
        SELECT kind, action, approval_ref, decision, surface, intent_json
        FROM khala_sync_fleet_steering_intents
        WHERE scope = ${scope} ORDER BY seq ASC
      `
      expect(receipts.map((r) => r.kind)).toEqual([
        "fleet_run_control",
        "approval_decision",
      ])
      expect(receipts[0]!.action).toBe("pause")
      expect(receipts[1]!.approval_ref).toBe(approvalRef)
      expect(receipts[1]!.decision).toBe("allow")
      for (const r of receipts) expect(r.surface).toBe("mobile")
      // intent_json round-trips as an OBJECT (not a double-encoded scalar)
      const stored =
        typeof receipts[0]!.intent_json === "string"
          ? JSON.parse(receipts[0]!.intent_json as string)
          : receipts[0]!.intent_json
      expect((stored as { schema: string }).schema).toBe(SCHEMA)
    })

    test("steer_message stays private and does not imply delivery before ACK", async () => {
      const client = freshClient()
      const runRef = "fleet.mh6.steer"
      const scope = fleetRunScope(runRef)
      // seed the run so the scope is owned by this client
      await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              counters: {},
              runRef,
              startedAt: nowIso,
              state: "running",
              targetConcurrency: 1,
              updatedAt: nowIso,
              workerKind: "claude",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId: runRef,
        sql: sql as unknown as SyncSql,
      })

      const secret = "PLEASE-DO-NOT-LEAK-THIS-STEER-BODY"
      const steer = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME, steerIntent({
            body: secret,
            intentId: "intent.mh6.steer.1",
            runRef,
            targetRef: "worker.mh6.claude.7f3a1b2c",
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(steer.results.map((r) => r.status)).toEqual(["applied"])

      const projected: Array<{ post_image_json: object | string }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${scope} AND entity_type = 'fleet_steer'
      `
      expect(projected).toEqual([])

      // but the authority CAN read the body from its private durable receipt
      const observed = await readPendingFleetSteeringIntents(
        sql as unknown as SyncSql,
        { scope },
      )
      expect(observed).toHaveLength(1)
      const intent = observed[0]!.intent
      expect(intent.kind).toBe("steer_message")
      if (intent.kind === "steer_message") {
        expect(intent.body).toBe(secret)
      }
    })

    test("rejects kind mismatch, foreign scope, and duplicate idempotency in-band", async () => {
      const owner = freshClient()
      const stranger = freshClient()
      const runRef = "fleet.mh6.reject"
      // owner claims the scope
      await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              counters: {},
              runRef,
              startedAt: nowIso,
              state: "running",
              targetConcurrency: 1,
              updatedAt: nowIso,
              workerKind: "auto",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
        ],
        ownerUserId: owner.userId,
        runId: runRef,
        sql: sql as unknown as SyncSql,
      })

      // kind mismatch: an approval intent sent to the run-control mutator
      const mismatch = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(1, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, approvalIntent({
            approvalRef: "approval.mh6.x.1",
            decision: "allow",
            intentId: "intent.mh6.mismatch.1",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(mismatch.results[0]!.status).toBe("rejected")
      expect(mismatch.results[0]!.errorCode).toBe(FLEET_STEERING_KIND_REJECTION)

      // foreign scope: a stranger cannot steer the owner's run
      const foreign = await executePush({
        registry,
        request: pushRequest(stranger, [
          envelope(1, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, runControlIntent({
            action: "pause",
            intentId: "intent.mh6.foreign.1",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: stranger.userId,
      })
      expect(foreign.results[0]!.status).toBe("rejected")
      expect(foreign.results[0]!.errorCode).toBe(FLEET_STEERING_SCOPE_REJECTION)

      // duplicate idempotency: same intent applied twice → second rejected
      const first = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(2, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, runControlIntent({
            action: "resume",
            intentId: "intent.mh6.dup.1",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(first.results[0]!.status).toBe("applied")
      const second = await executePush({
        registry,
        request: pushRequest(owner, [
          // a fresh mutationId + intentId, but the SAME idempotencyKey
          envelope(3, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, {
            ...runControlIntent({
              action: "resume",
              intentId: "intent.mh6.dup.2",
              runRef,
            }),
            idempotencyKey: "intent.mh6.dup.1.idem",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(second.results[0]!.status).toBe("rejected")
      expect(second.results[0]!.errorCode).toBe(
        FLEET_STEERING_INTENT_EXISTS_REJECTION,
      )

      const oversized = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(4, FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME, steerIntent({
            body: "x".repeat(16 * 1_024 + 1),
            intentId: "intent.mh6.oversized.1",
            runRef,
          })),
          envelope(5, FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME, {
            ...steerIntent({
              intentId: "intent.mh6.bad-ref.1",
              runRef,
            }),
            bodyRef: "private body ref with spaces",
          }),
          envelope(6, FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME, {
            ...approvalIntent({
              approvalRef: "approval.mh6.oversized.1",
              decision: "allow",
              intentId: "intent.mh6.oversized.approval.1",
              runRef,
            }),
            reasonRef: "r".repeat(32 * 1_024),
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(oversized.results.map((result) => result.errorCode)).toEqual([
        FLEET_STEERING_BODY_SHAPE_REJECTION,
        FLEET_STEERING_BODY_SHAPE_REJECTION,
        FLEET_STEERING_BODY_SHAPE_REJECTION,
      ])
    })

    test("bounds durable command receipts per run before Sarah persistence can overflow", async () => {
      const client = freshClient()
      const runRef = "fleet.mh6.command-limit"
      const scope = fleetRunScope(runRef)
      const seeded = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              counters: {},
              runRef,
              startedAt: nowIso,
              state: "running",
              targetConcurrency: 1,
              updatedAt: nowIso,
              workerKind: "auto",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
        ],
        ownerUserId: client.userId,
        runId: runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(seeded.ok).toBe(true)
      await sql`
        INSERT INTO khala_sync_fleet_steering_intents
          (intent_id, scope, run_ref, kind, action, approval_ref, decision,
           surface, requested_by_user_id, idempotency_key, intent_json,
           mutation_ref, created_at)
        SELECT
          'intent.mh6.command-limit.' || series.n,
          ${scope}, ${runRef}, 'fleet_run_control', 'pause', NULL, NULL,
          'mobile', ${client.userId},
          'idem.mh6.command-limit.' || series.n,
          jsonb_build_object(
            'schema', 'khala.fleet_intent.v1',
            'intentId', 'intent.mh6.command-limit.' || series.n,
            'createdAt', ${nowIso}::text,
            'origin', jsonb_build_object('surface', 'mobile'),
            'idempotencyKey', 'idem.mh6.command-limit.' || series.n,
            'runRef', ${runRef}::text,
            'kind', 'fleet_run_control',
            'action', 'pause'
          ),
          'mutation.test.command-limit', ${nowIso}
        FROM generate_series(
          1,
          ${FLEET_STEERING_MAX_INTENTS_PER_RUN}
        ) AS series(n)
      `

      const result = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME, runControlIntent({
            action: "pause",
            intentId: "intent.mh6.command-limit.overflow",
            runRef,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(result.results[0]?.status).toBe("rejected")
      expect(result.results[0]?.errorCode).toBe(
        FLEET_STEERING_RUN_LIMIT_REJECTION,
      )
      const rows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count
        FROM khala_sync_fleet_steering_intents
        WHERE run_ref = ${runRef}
      `
      expect(Number(rows[0]?.count ?? 0)).toBe(
        FLEET_STEERING_MAX_INTENTS_PER_RUN,
      )
    })
  },
)
