import { canonicalJson, fleetRunScope } from "@openagentsinc/khala-sync"
import {
  FleetSteeringFollowUpCompletionRefKnownAnswer,
  FleetSteeringOutcomeRefKnownAnswer,
} from "@openagentsinc/khala-fleet-intents"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { Effect } from "effect"

import {
  fleetSteeringFollowUpCompletionRef,
  fleetSteeringOutcomeRef,
  makeFleetSteeringExchangeRepository,
} from "./fleet-steering-exchange.js"
import {
  fleetApprovalPostImage,
  fleetRunPostImage,
  projectFleetEntitiesBestEffort,
} from "./fleet-projection.js"
import { makeFleetRunAuthorityRepository } from "./fleet-run-authority.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const FIXED_NOW = Date.parse("2026-07-09T23:00:00.000Z")
const COMMIT = "6896b5cbcc7c7268d77b05f29f64c1d6f8951b18"

test("fleet steering outcome ref matches the shared known-answer vector", async () => {
  expect(
    await fleetSteeringOutcomeRef({
      runRef: "fleet_run.sarah.0123456789abcdef0123",
      claimRef: "claim.sarah_fleet_run.0123456789abcdef01234567",
      pylonRef: "pylon.test.one",
      outcome: {
        seq: 41,
        intentId: "intent.sarah.pause.1",
        outcome: "applied",
        outcomeRef: FleetSteeringOutcomeRefKnownAnswer.outcomeRef,
        observedAt: "2026-07-09T23:00:01.000Z",
      },
    }),
  ).toBe(FleetSteeringOutcomeRefKnownAnswer.outcomeRef)
})

test("fleet steering completion ref matches the shared known-answer vector", async () => {
  expect(
    await fleetSteeringFollowUpCompletionRef({
      runRef: "fleet_run.sarah.0123456789abcdef0123",
      claimRef: "claim.sarah_fleet_run.0123456789abcdef01234567",
      pylonRef: "pylon.test.one",
      completion: {
        seq: 41,
        intentId: "intent.sarah.pause.1",
        state: "applied",
        completionRef:
          FleetSteeringFollowUpCompletionRefKnownAnswer.completionRef,
        completedAt: "2026-07-09T23:00:02.000Z",
      },
    }),
  ).toBe(FleetSteeringFollowUpCompletionRefKnownAnswer.completionRef)
})

const runRequest = (idempotencyKey: string) => ({
  objective: "Prove accepted Pylon steering delivery.",
  repository: {
    owner: "OpenAgentsInc",
    name: "openagents",
    branch: "main",
    commit: COMMIT,
  },
  verifier: { kind: "command" as const, command: "bun test" },
  workSource: { kind: "issue_list" as const, issueRefs: ["#8639"] },
  workerPolicy: {
    workerKind: "auto" as const,
    targetPreference: "owner_local" as const,
  },
  targetConcurrency: 3,
  idempotencyKey,
})

describe.skipIf(!hasLocalPostgres())(
  "Fleet steering exchange against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE sarah_fleet_steering_exchange")
      await admin.end()
      const url = pg.urlFor("sarah_fleet_steering_exchange")
      const migrated = await runMigrations({ databaseUrl: url })
      expect(migrated.applied).toContain(
        "0054_sarah_fleet_run_steering_exchange.sql",
      )
      expect(migrated.applied).toContain(
        "0055_sarah_fleet_run_steering_completions.sql",
      )
      sql = new SQL({ url, max: 12 })
    })

    afterAll(async () => {
      if (sql !== undefined) {
        await sql.end()
      }
      if (pg !== undefined) {
        await pg.stop()
      }
    })

    const authority = () =>
      makeFleetRunAuthorityRepository({
        sql: sql as unknown as SyncSql,
        now: Effect.succeed(FIXED_NOW),
      })

    const exchange = () =>
      makeFleetSteeringExchangeRepository({
        sql: sql as unknown as SyncSql,
        now: Effect.succeed(FIXED_NOW + 5_000),
      })

    const seedPylon = async (ownerUserId: string, pylonRef: string) => {
      const nowIso = new Date(FIXED_NOW).toISOString()
      await sql`
        INSERT INTO pylon_registrations
          (id, pylon_ref, owner_agent_user_id, owner_agent_credential_id,
           owner_agent_token_prefix, display_name, status, resource_mode,
           capability_refs_json, wallet_ready, latest_heartbeat_at,
           latest_heartbeat_status, latest_health_refs_json,
           latest_load_refs_json, latest_capacity_refs_json,
           provider_market_relay_refs_json, provider_nip90_lane_refs_json,
           public_projection_json, created_at, updated_at)
        VALUES
          (${`registration.${pylonRef}`}, ${pylonRef}, ${ownerUserId},
           ${`credential.${pylonRef}`}, 'oa_agent', ${pylonRef}, 'active',
           'owner_local', '[]', 0, ${nowIso}, 'online', '[]', '[]', '[]',
           '[]', '[]', '{}', ${nowIso}, ${nowIso})
      `
    }

    const startAndAccept = async (suffix: string) => {
      const ownerUserId = `user-steering-${suffix}`
      const pylonRef = `pylon-steering-${suffix}`
      const run = await Effect.runPromise(
        authority().start({
          ownerUserId,
          request: runRequest(`steering-run-${suffix}`),
        }),
      )
      await seedPylon(ownerUserId, pylonRef)
      const claimed = await Effect.runPromise(
        authority().claim({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          claimIdempotencyKey: `steering-claim-${suffix}`,
          leaseDurationMs: 30_000,
        }),
      )
      await Effect.runPromise(
        authority().acceptClaim({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          claimRef: claimed.claim.claimRef,
        }),
      )
      return {
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimRef: claimed.claim.claimRef,
      }
    }

    const insertIntent = async (
      fixture: Awaited<ReturnType<typeof startAndAccept>>,
      intent: Readonly<Record<string, unknown>>,
    ): Promise<number> => {
      const kind = String(intent.kind)
      const action = kind === "fleet_run_control" ? String(intent.action) : null
      const approvalRef =
        kind === "approval_decision" ? String(intent.approvalRef) : null
      const decision =
        kind === "approval_decision" ? String(intent.decision) : null
      const rows: Array<{ seq: string | number | bigint }> = await sql`
        INSERT INTO khala_sync_fleet_steering_intents
          (intent_id, scope, run_ref, kind, action, approval_ref, decision,
           surface, requested_by_user_id, idempotency_key, intent_json,
           mutation_ref, created_at)
        VALUES
          (${String(intent.intentId)}, ${fleetRunScope(fixture.runRef)},
           ${fixture.runRef}, ${kind}, ${action}, ${approvalRef}, ${decision},
           'web', ${fixture.ownerUserId}, ${String(intent.idempotencyKey)},
           ${canonicalJson(intent)}::jsonb, 'mutation.test.steering',
           ${String(intent.createdAt)})
        RETURNING seq
      `
      return Number(rows[0]!.seq)
    }

    const runControlIntent = (
      fixture: Awaited<ReturnType<typeof startAndAccept>>,
      intentId: string,
      action: "pause" | "resume" | "drain" | "stop",
      createdAt: string,
    ) => ({
      schema: "khala.fleet_intent.v1" as const,
      intentId,
      createdAt,
      origin: { surface: "web" as const },
      idempotencyKey: `${intentId}.idem`,
      runRef: fixture.runRef,
      kind: "fleet_run_control" as const,
      action,
    })

    const outcomeFor = async (
      fixture: Awaited<ReturnType<typeof startAndAccept>>,
      input: Readonly<{
        seq: number
        intentId: string
        outcome:
          | "applied"
          | "queued_follow_up"
          | "skipped_stale"
          | "rejected"
          | "failed"
        observedAt: string
      }>,
    ) => {
      const withoutRef = {
        seq: input.seq,
        intentId: input.intentId,
        outcome: input.outcome,
        observedAt: input.observedAt,
      }
      return {
        ...withoutRef,
        outcomeRef: await fleetSteeringOutcomeRef({
          runRef: fixture.runRef,
          claimRef: fixture.claimRef,
          pylonRef: fixture.pylonRef,
          outcome: {
            ...withoutRef,
            outcomeRef: "outcome.pylon.fleet_steering.000000000000000000000000",
          },
        }),
      }
    }

    const completionFor = async (
      fixture: Awaited<ReturnType<typeof startAndAccept>>,
      input: Readonly<{
        seq: number
        intentId: string
        state: "applied" | "failed" | "stale"
        completedAt: string
      }>,
    ) => {
      const completion = {
        ...input,
        completionRef:
          "completion.pylon.fleet_steering.000000000000000000000000",
      }
      return {
        ...completion,
        completionRef: await fleetSteeringFollowUpCompletionRef({
          runRef: fixture.runRef,
          claimRef: fixture.claimRef,
          pylonRef: fixture.pylonRef,
          completion,
        }),
      }
    }

    const latestRunStatus = async (runRef: string): Promise<string> => {
      const rows: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(runRef)}
          AND entity_type = 'fleet_run'
          AND entity_id = ${runRef}
        ORDER BY version DESC LIMIT 1
      `
      const raw = rows[0]!.post_image_json
      const image =
        typeof raw === "string"
          ? (JSON.parse(raw) as { status: string })
          : (raw as { status: string })
      return image.status
    }

    const commandOutcomes = async (
      runRef: string,
    ): Promise<ReadonlyArray<Record<string, unknown>>> => {
      const rows: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(runRef)}
          AND entity_type = 'fleet_command_outcome'
        ORDER BY version ASC
      `
      return rows.map((row) =>
        typeof row.post_image_json === "string"
          ? (JSON.parse(row.post_image_json) as Record<string, unknown>)
          : (row.post_image_json as Record<string, unknown>),
      )
    }

    const projectRunningRun = async (
      fixture: Awaited<ReturnType<typeof startAndAccept>>,
    ) => {
      const result = await projectFleetEntitiesBestEffort({
        changes: [
          {
            kind: "fleet_run" as const,
            op: "upsert" as const,
            entity: fleetRunPostImage({
              runRef: fixture.runRef,
              state: "running",
              targetConcurrency: 3,
              workerKind: "auto",
              startedAt: "2026-07-09T23:00:00.000Z",
              counters: { workUnitsTotal: 1, activeAssignments: 1 },
              updatedAt: "2026-07-09T23:00:00.000Z",
            }),
          },
        ],
        ownerUserId: fixture.ownerUserId,
        runId: fixture.runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(result.ok).toBe(true)
    }

    test("pages only the exact accepted owner/Pylon/run/claim and validates nested identity", async () => {
      const fixture = await startAndAccept("page")
      const pause = runControlIntent(
        fixture,
        "intent.steering.page.pause",
        "pause",
        "2026-07-09T23:00:01.000Z",
      )
      const resume = runControlIntent(
        fixture,
        "intent.steering.page.resume",
        "resume",
        "2026-07-09T23:00:02.000Z",
      )
      const pauseSeq = await insertIntent(fixture, pause)
      const resumeSeq = await insertIntent(fixture, resume)

      const first = await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 1 }),
      )
      expect(first).toMatchObject({
        ok: true,
        runRef: fixture.runRef,
        claimRef: fixture.claimRef,
        nextAfter: pauseSeq,
        upToDate: false,
      })
      expect(first.intents).toEqual([
        { seq: pauseSeq, intentId: pause.intentId, intent: pause, createdAt: pause.createdAt },
      ])
      const second = await Effect.runPromise(
        exchange().readPage({ ...fixture, after: pauseSeq, limit: 1 }),
      )
      expect(second.intents[0]?.seq).toBe(resumeSeq)
      expect(second.upToDate).toBe(true)

      const foreign = await Effect.runPromise(
        exchange()
          .readPage({
            ...fixture,
            ownerUserId: "user-steering-foreign",
            after: 0,
            limit: 10,
          })
          .pipe(Effect.flip),
      )
      expect(foreign.kind).toBe("claim_conflict")

      await sql`
        UPDATE khala_sync_fleet_steering_intents
        SET intent_json = ${canonicalJson({ ...pause, intentId: "intent.steering.tampered" })}::jsonb
        WHERE intent_id = ${pause.intentId}
      `
      const tampered = await Effect.runPromise(
        exchange()
          .readPage({ ...fixture, after: 0, limit: 10 })
          .pipe(Effect.flip),
      )
      expect(tampered.kind).toBe("storage_unavailable")
    })

    test("byte-bounded pages remain a strict sequence prefix", async () => {
      const fixture = await startAndAccept("byte-prefix")
      const inserted: Array<{ seq: number; intentId: string }> = []
      for (let index = 0; index < 16; index += 1) {
        const intentId = `intent.steering.byte-prefix.${index}`
        const intent = {
          schema: "khala.fleet_intent.v1" as const,
          intentId,
          createdAt: new Date(FIXED_NOW + index * 1_000).toISOString(),
          origin: { surface: "web" as const },
          idempotencyKey: `${intentId}.idem`,
          runRef: fixture.runRef,
          kind: "steer_message" as const,
          targetRef: `work_claim.byte-prefix.${index}`,
          body: "x".repeat(16_000),
        }
        inserted.push({ seq: await insertIntent(fixture, intent), intentId })
      }
      const tail = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.byte-prefix.tail",
        createdAt: new Date(FIXED_NOW + 16_000).toISOString(),
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.byte-prefix.tail.idem",
        runRef: fixture.runRef,
        kind: "steer_message" as const,
        targetRef: "work_claim.byte-prefix.tail",
        body: "tail",
      }
      const tailSeq = await insertIntent(fixture, tail)

      const first = await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 100 }),
      )
      expect(first.intents.length).toBeGreaterThan(0)
      expect(first.intents.length).toBeLessThan(inserted.length)
      expect(first.intents.map((intent) => intent.seq)).toEqual(
        inserted.slice(0, first.intents.length).map((intent) => intent.seq),
      )
      expect(first.nextAfter).toBe(
        inserted[first.intents.length - 1]?.seq,
      )
      expect(first.intents.some((intent) => intent.seq === tailSeq)).toBe(false)
      expect(first.upToDate).toBe(false)

      const second = await Effect.runPromise(
        exchange().readPage({
          ...fixture,
          after: first.nextAfter,
          limit: 100,
        }),
      )
      expect(second.intents.at(-1)?.seq).toBe(tailSeq)
      expect(second.upToDate).toBe(true)
    })

    test("orders same-run outcomes across global gaps and prevents delayed pause from overwriting resume", async () => {
      const fixture = await startAndAccept("ordering")
      const foreign = await startAndAccept("ordering-gap")
      const pause = runControlIntent(
        fixture,
        "intent.steering.order.pause",
        "pause",
        "2026-07-09T23:01:00.000Z",
      )
      const resume = runControlIntent(
        fixture,
        "intent.steering.order.resume",
        "resume",
        "2026-07-09T23:01:02.000Z",
      )
      const pauseSeq = await insertIntent(fixture, pause)
      await insertIntent(
        foreign,
        runControlIntent(
          foreign,
          "intent.steering.other.pause",
          "pause",
          "2026-07-09T23:01:01.000Z",
        ),
      )
      const resumeSeq = await insertIntent(fixture, resume)
      expect(resumeSeq).toBeGreaterThan(pauseSeq + 1)
      await projectRunningRun(fixture)
      const pauseOutcome = await outcomeFor(fixture, {
        seq: pauseSeq,
        intentId: pause.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:02:00.000Z",
      })
      const resumeOutcome = await outcomeFor(fixture, {
        seq: resumeSeq,
        intentId: resume.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:02:01.000Z",
      })
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )

      const outOfOrder = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: { claimRef: fixture.claimRef, outcomes: [resumeOutcome] },
          })
          .pipe(Effect.flip),
      )
      expect(outOfOrder.kind).toBe("claim_conflict")
      expect(await latestRunStatus(fixture.runRef)).toBe("running")

      const unsorted = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: {
              claimRef: fixture.claimRef,
              outcomes: [resumeOutcome, pauseOutcome],
            },
          })
          .pipe(Effect.flip),
      )
      expect(unsorted.kind).toBe("invalid_request")

      const ack = await Effect.runPromise(
        exchange().appendOutcomes({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: {
            claimRef: fixture.claimRef,
            outcomes: [pauseOutcome, resumeOutcome],
          },
        }),
      )
      expect(ack.outcomes.map((outcome) => outcome.seq)).toEqual([
        pauseSeq,
        resumeSeq,
      ])
      expect(ack.storedOutcomeCount).toBe(2)
      expect(await latestRunStatus(fixture.runRef)).toBe("running")
      expect(await commandOutcomes(fixture.runRef)).toEqual([
        expect.objectContaining({
          intentId: pause.intentId,
          seq: pauseSeq,
          kind: "fleet_run_control",
          deliveryOutcome: "applied",
          completionOutcome: "applied",
          effectiveOutcome: "paused",
          completionRef: pauseOutcome.outcomeRef,
          completedAt: "2026-07-09T23:00:05.000Z",
          recordedAt: "2026-07-09T23:00:05.000Z",
        }),
        expect.objectContaining({
          intentId: resume.intentId,
          seq: resumeSeq,
          deliveryOutcome: "applied",
          completionOutcome: "applied",
          effectiveOutcome: "running",
        }),
      ])

      const beforeReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(fixture.runRef)}
      `

      const replay = await Effect.runPromise(
        exchange().appendOutcomes({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: {
            claimRef: fixture.claimRef,
            outcomes: [pauseOutcome, resumeOutcome],
          },
        }),
      )
      expect(replay.duplicateOutcomeCount).toBe(2)
      const afterReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(fixture.runRef)}
      `
      expect(Number(afterReplay[0]!.count)).toBe(
        Number(beforeReplay[0]!.count),
      )

      const conflict = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: {
              claimRef: fixture.claimRef,
              outcomes: [{ ...pauseOutcome, outcome: "failed" }],
            },
          })
          .pipe(Effect.flip),
      )
      expect(conflict.kind).toBe("idempotency_conflict")
    })

    test("applied approval and queued steer project body-free requested-vs-effective outcomes", async () => {
      const fixture = await startAndAccept("privacy")
      const approval = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.approval.allow",
        createdAt: "2026-07-09T23:03:00.000Z",
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.approval.allow.idem",
        runRef: fixture.runRef,
        kind: "approval_decision" as const,
        approvalRef: "approval.steering.tool.1",
        decision: "allow" as const,
      }
      const secret = "PRIVATE-STEER-BODY-MUST-NOT-PROJECT"
      const steer = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.private.body",
        createdAt: "2026-07-09T23:03:01.000Z",
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.private.body.idem",
        runRef: fixture.runRef,
        kind: "steer_message" as const,
        targetRef: "worker.steering.codex.1",
        body: secret,
      }
      const approvalSeq = await insertIntent(fixture, approval)
      const steerSeq = await insertIntent(fixture, steer)
      const projectedApproval = await projectFleetEntitiesBestEffort({
        changes: [
          {
            kind: "fleet_approval",
            op: "upsert",
            entity: fleetApprovalPostImage({
              approvalRef: approval.approvalRef,
              status: "pending",
              workerId: "worker.steering.codex.1",
              toolClass: "bash",
              openedAt: "2026-07-09T23:03:00.000Z",
              updatedAt: "2026-07-09T23:03:00.000Z",
            }),
          },
        ],
        ownerUserId: fixture.ownerUserId,
        runId: fixture.runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(projectedApproval.ok).toBe(true)
      const approvalOutcome = await outcomeFor(fixture, {
        seq: approvalSeq,
        intentId: approval.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:04:00.000Z",
      })
      const steerOutcome = await outcomeFor(fixture, {
        seq: steerSeq,
        intentId: steer.intentId,
        outcome: "queued_follow_up",
        observedAt: "2026-07-09T23:04:01.000Z",
      })
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const ack = await Effect.runPromise(
        exchange().appendOutcomes({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: {
            claimRef: fixture.claimRef,
            outcomes: [approvalOutcome, steerOutcome],
          },
        }),
      )
      expect(canonicalJson(ack)).not.toContain(secret)

      const projections: Array<{
        entity_type: string
        post_image_json: unknown
      }> = await sql`
        SELECT entity_type, post_image_json FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(fixture.runRef)}
        ORDER BY version, entity_type
      `
      expect(projections.some((row) => row.entity_type === "fleet_steer")).toBe(
        false,
      )
      expect(canonicalJson(projections)).not.toContain(secret)
      const projectedOutcomes = await commandOutcomes(fixture.runRef)
      expect(projectedOutcomes).toEqual([
        expect.objectContaining({
          intentId: approval.intentId,
          kind: "approval_decision",
          targetRef: approval.approvalRef,
          deliveryOutcome: "applied",
          completionOutcome: "applied",
          effectiveOutcome: "allowed",
          completionRef: approvalOutcome.outcomeRef,
          completedAt: "2026-07-09T23:00:05.000Z",
          observedAt: approvalOutcome.observedAt,
        }),
        expect.objectContaining({
          intentId: steer.intentId,
          kind: "steer_message",
          targetRef: steer.targetRef,
          deliveryOutcome: "queued_follow_up",
          completionOutcome: null,
          effectiveOutcome: null,
          completionRef: null,
          completedAt: null,
          observedAt: steerOutcome.observedAt,
        }),
      ])
      expect(canonicalJson(projectedOutcomes)).not.toContain(secret)
      const approvalRaw = projections
        .filter((row) => row.entity_type === "fleet_approval")
        .at(-1)?.post_image_json
      const approvalImage =
        typeof approvalRaw === "string"
          ? (JSON.parse(approvalRaw) as { status?: string })
          : (approvalRaw as { status?: string } | undefined)
      expect(approvalImage?.status).toBe("allowed")

      const outcomes: Array<Record<string, unknown>> = await sql`
        SELECT * FROM sarah_fleet_run_steering_outcomes
        WHERE run_ref = ${fixture.runRef}
        ORDER BY seq
      `
      expect(outcomes).toHaveLength(2)
      expect(canonicalJson(outcomes)).not.toContain(secret)
    })

    test("completes queued approval, steer, and stop in order with reconnect-safe receipts", async () => {
      const fixture = await startAndAccept("completion")
      await projectRunningRun(fixture)
      const secret = "PRIVATE-COMPLETION-STEER-BODY"
      const approval = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.completion.approval",
        createdAt: "2026-07-09T23:40:00.000Z",
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.completion.approval.idem",
        runRef: fixture.runRef,
        kind: "approval_decision" as const,
        approvalRef: "approval.steering.completion.1",
        decision: "allow" as const,
      }
      const steer = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.completion.steer",
        createdAt: "2026-07-09T23:40:01.000Z",
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.completion.steer.idem",
        runRef: fixture.runRef,
        kind: "steer_message" as const,
        targetRef: "work_claim.steering.completion.1",
        body: secret,
      }
      const stop = runControlIntent(
        fixture,
        "intent.steering.completion.stop",
        "stop",
        "2026-07-09T23:40:02.000Z",
      )
      const approvalSeq = await insertIntent(fixture, approval)
      const steerSeq = await insertIntent(fixture, steer)
      const stopSeq = await insertIntent(fixture, stop)
      const projectedApproval = await projectFleetEntitiesBestEffort({
        changes: [
          {
            kind: "fleet_approval" as const,
            op: "upsert" as const,
            entity: fleetApprovalPostImage({
              approvalRef: approval.approvalRef,
              status: "pending",
              workerId: "worker.steering.completion.1",
              toolClass: "bash",
              openedAt: approval.createdAt,
              updatedAt: approval.createdAt,
            }),
          },
        ],
        ownerUserId: fixture.ownerUserId,
        runId: fixture.runRef,
        sql: sql as unknown as SyncSql,
      })
      expect(projectedApproval.ok).toBe(true)
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const queuedOutcomes = await Promise.all([
        outcomeFor(fixture, {
          seq: approvalSeq,
          intentId: approval.intentId,
          outcome: "queued_follow_up",
          observedAt: "2026-07-09T23:41:00.000Z",
        }),
        outcomeFor(fixture, {
          seq: steerSeq,
          intentId: steer.intentId,
          outcome: "queued_follow_up",
          observedAt: "2026-07-09T23:41:01.000Z",
        }),
        outcomeFor(fixture, {
          seq: stopSeq,
          intentId: stop.intentId,
          outcome: "queued_follow_up",
          observedAt: "2026-07-09T23:41:02.000Z",
        }),
      ])
      await Effect.runPromise(
        exchange().appendOutcomes({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: { claimRef: fixture.claimRef, outcomes: queuedOutcomes },
        }),
      )
      const completions = await Promise.all([
        completionFor(fixture, {
          seq: approvalSeq,
          intentId: approval.intentId,
          state: "applied",
          completedAt: "2026-07-09T23:42:00.000Z",
        }),
        completionFor(fixture, {
          seq: steerSeq,
          intentId: steer.intentId,
          state: "applied",
          completedAt: "2026-07-09T23:42:01.000Z",
        }),
        completionFor(fixture, {
          seq: stopSeq,
          intentId: stop.intentId,
          state: "applied",
          completedAt: "2026-07-09T23:42:02.000Z",
        }),
      ])
      const ack = await Effect.runPromise(
        exchange().appendCompletions({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: { claimRef: fixture.claimRef, completions },
        }),
      )
      expect(ack.storedCompletionCount).toBe(3)
      expect(canonicalJson(ack)).not.toContain(secret)
      expect(await latestRunStatus(fixture.runRef)).toBe("stopped")
      expect((await commandOutcomes(fixture.runRef)).slice(-3)).toEqual([
        expect.objectContaining({
          intentId: approval.intentId,
          deliveryOutcome: "queued_follow_up",
          completionOutcome: "applied",
          effectiveOutcome: "allowed",
          completionRef: completions[0]!.completionRef,
          completedAt: completions[0]!.completedAt,
        }),
        expect.objectContaining({
          intentId: steer.intentId,
          completionOutcome: "applied",
          effectiveOutcome: "steer_delivered",
          completionRef: completions[1]!.completionRef,
        }),
        expect.objectContaining({
          intentId: stop.intentId,
          completionOutcome: "applied",
          effectiveOutcome: "stopped",
          completionRef: completions[2]!.completionRef,
        }),
      ])
      expect(canonicalJson(await commandOutcomes(fixture.runRef))).not.toContain(
        secret,
      )

      const beforeReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(fixture.runRef)}
      `
      await sql`
        UPDATE sarah_fleet_run_intake_leases SET state = 'released'
        WHERE run_ref = ${fixture.runRef}
      `
      const replay = await Effect.runPromise(
        exchange().appendCompletions({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: { claimRef: fixture.claimRef, completions },
        }),
      )
      expect(replay.duplicateCompletionCount).toBe(3)
      const afterReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${fleetRunScope(fixture.runRef)}
      `
      expect(Number(afterReplay[0]!.count)).toBe(
        Number(beforeReplay[0]!.count),
      )

      const conflicting = await completionFor(fixture, {
        seq: approvalSeq,
        intentId: approval.intentId,
        state: "applied",
        completedAt: "2026-07-09T23:49:00.000Z",
      })
      const conflict = await Effect.runPromise(
        exchange()
          .appendCompletions({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: { claimRef: fixture.claimRef, completions: [conflicting] },
          })
          .pipe(Effect.flip),
      )
      expect(conflict.kind).toBe("idempotency_conflict")
    })

    test("failed and stale completions never claim effective state", async () => {
      const fixture = await startAndAccept("completion-terminal")
      const intents = ["failed", "stale"].map((state, index) => ({
        schema: "khala.fleet_intent.v1" as const,
        intentId: `intent.steering.completion.${state}`,
        createdAt: new Date(FIXED_NOW + 60_000 + index * 1_000).toISOString(),
        origin: { surface: "web" as const },
        idempotencyKey: `intent.steering.completion.${state}.idem`,
        runRef: fixture.runRef,
        kind: "steer_message" as const,
        targetRef: `work_claim.steering.completion.${state}`,
        body: `private-${state}`,
      }))
      const sequences: number[] = []
      for (const intent of intents) {
        sequences.push(await insertIntent(fixture, intent))
      }
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const outcomes = await Promise.all(
        intents.map((intent, index) =>
          outcomeFor(fixture, {
            seq: sequences[index]!,
            intentId: intent.intentId,
            outcome: "queued_follow_up",
            observedAt: new Date(FIXED_NOW + 70_000 + index * 1_000).toISOString(),
          }),
        ),
      )
      await Effect.runPromise(
        exchange().appendOutcomes({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: { claimRef: fixture.claimRef, outcomes },
        }),
      )
      const completions = await Promise.all(
        intents.map((intent, index) =>
          completionFor(fixture, {
            seq: sequences[index]!,
            intentId: intent.intentId,
            state: index === 0 ? "failed" : "stale",
            completedAt: new Date(FIXED_NOW + 80_000 + index * 1_000).toISOString(),
          }),
        ),
      )
      await Effect.runPromise(
        exchange().appendCompletions({
          ownerUserId: fixture.ownerUserId,
          pylonRef: fixture.pylonRef,
          runRef: fixture.runRef,
          batch: { claimRef: fixture.claimRef, completions },
        }),
      )
      expect((await commandOutcomes(fixture.runRef)).slice(-2)).toEqual([
        expect.objectContaining({
          completionOutcome: "failed",
          effectiveOutcome: null,
        }),
        expect.objectContaining({
          completionOutcome: "skipped_stale",
          effectiveOutcome: null,
        }),
      ])
    })

    test("refuses unsupported applied steer ACKs without storing or projecting them", async () => {
      const fixture = await startAndAccept("unsupported-applied-steer")
      const steer = {
        schema: "khala.fleet_intent.v1" as const,
        intentId: "intent.steering.unsupported.applied",
        createdAt: "2026-07-09T23:04:00.000Z",
        origin: { surface: "web" as const },
        idempotencyKey: "intent.steering.unsupported.applied.idem",
        runRef: fixture.runRef,
        kind: "steer_message" as const,
        targetRef: "worker.steering.codex.1",
        body: "private body",
      }
      const seq = await insertIntent(fixture, steer)
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const outcome = await outcomeFor(fixture, {
        seq,
        intentId: steer.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:04:01.000Z",
      })
      const failure = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: { claimRef: fixture.claimRef, outcomes: [outcome] },
          })
          .pipe(Effect.flip),
      )
      expect(failure.kind).toBe("claim_conflict")
      const stored: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_steering_outcomes
        WHERE run_ref = ${fixture.runRef}
      `
      expect(Number(stored[0]!.count)).toBe(0)
      expect(await commandOutcomes(fixture.runRef)).toEqual([])
    })

    test("rolls back an earlier outcome and effective projection when a later row fails", async () => {
      const fixture = await startAndAccept("rollback")
      const pause = runControlIntent(
        fixture,
        "intent.steering.rollback.pause",
        "pause",
        "2026-07-09T23:05:00.000Z",
      )
      const pauseSeq = await insertIntent(fixture, pause)
      await projectRunningRun(fixture)
      const pauseOutcome = await outcomeFor(fixture, {
        seq: pauseSeq,
        intentId: pause.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:06:00.000Z",
      })
      const unknownOutcome = await outcomeFor(fixture, {
        seq: pauseSeq + 100,
        intentId: "intent.steering.rollback.unknown",
        outcome: "failed",
        observedAt: "2026-07-09T23:06:01.000Z",
      })
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const failed = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: {
              claimRef: fixture.claimRef,
              outcomes: [pauseOutcome, unknownOutcome],
            },
          })
          .pipe(Effect.flip),
      )
      expect(failed.kind).toBe("claim_conflict")
      const stored: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_steering_outcomes
        WHERE run_ref = ${fixture.runRef}
      `
      expect(Number(stored[0]!.count)).toBe(0)
      expect(await latestRunStatus(fixture.runRef)).toBe("running")
    })

    test("rejects malicious applied ACKs that do not match effective pre-state", async () => {
      const fixture = await startAndAccept("invalid-state")
      const pause = runControlIntent(
        fixture,
        "intent.steering.invalid.pause",
        "pause",
        "2026-07-09T23:09:00.000Z",
      )
      const seq = await insertIntent(fixture, pause)
      await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const outcome = await outcomeFor(fixture, {
        seq,
        intentId: pause.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:09:01.000Z",
      })
      const invalidRunTransition = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: { claimRef: fixture.claimRef, outcomes: [outcome] },
          })
          .pipe(Effect.flip),
      )
      expect(invalidRunTransition.kind).toBe("claim_conflict")
      const stored: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_steering_outcomes
        WHERE run_ref = ${fixture.runRef}
      `
      expect(Number(stored[0]!.count)).toBe(0)
      expect(await latestRunStatus(fixture.runRef)).toBe("draft")
    })

    test("reserves first delivery to the accepted claim and survives a lost ACK", async () => {
      const fixture = await startAndAccept("reservation")
      const pause = runControlIntent(
        fixture,
        "intent.steering.reserved.pause",
        "pause",
        "2026-07-09T23:07:00.000Z",
      )
      const seq = await insertIntent(fixture, pause)
      const first = await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      const afterLostAck = await Effect.runPromise(
        exchange().readPage({ ...fixture, after: 0, limit: 10 }),
      )
      expect(afterLostAck).toEqual(first)
      const deliveries: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_steering_deliveries
        WHERE run_ref = ${fixture.runRef} AND seq = ${seq}
      `
      expect(Number(deliveries[0]!.count)).toBe(1)

      await sql`
        UPDATE sarah_fleet_run_intake_leases SET state = 'released'
        WHERE run_ref = ${fixture.runRef}
      `
      const outcome = await outcomeFor(fixture, {
        seq,
        intentId: pause.intentId,
        outcome: "applied",
        observedAt: "2026-07-09T23:08:00.000Z",
      })
      const staleAck = await Effect.runPromise(
        exchange()
          .appendOutcomes({
            ownerUserId: fixture.ownerUserId,
            pylonRef: fixture.pylonRef,
            runRef: fixture.runRef,
            batch: { claimRef: fixture.claimRef, outcomes: [outcome] },
          })
          .pipe(Effect.flip),
      )
      expect(staleAck.kind).toBe("claim_conflict")
      const staleRead = await Effect.runPromise(
        exchange()
          .readPage({ ...fixture, after: 0, limit: 10 })
          .pipe(Effect.flip),
      )
      expect(staleRead.kind).toBe("claim_conflict")
      const reservation: Array<{
        intake_claim_ref: string
        pylon_ref: string
      }> = await sql`
        SELECT intake_claim_ref, pylon_ref
        FROM sarah_fleet_run_steering_deliveries
        WHERE run_ref = ${fixture.runRef} AND seq = ${seq}
      `
      expect(reservation).toEqual([
        {
          intake_claim_ref: fixture.claimRef,
          pylon_ref: fixture.pylonRef,
        },
      ])
    })

    test("serializes delivery against a concurrent claim release", async () => {
      const fixture = await startAndAccept("release-race")
      const pause = runControlIntent(
        fixture,
        "intent.steering.release-race.pause",
        "pause",
        "2026-07-09T23:30:00.000Z",
      )
      await insertIntent(fixture, pause)

      let releaseTransaction: (() => void) | undefined
      let markUpdated: (() => void) | undefined
      const mayCommit = new Promise<void>((resolve) => {
        releaseTransaction = resolve
      })
      const updated = new Promise<void>((resolve) => {
        markUpdated = resolve
      })
      const releasing = sql.begin(async (tx) => {
        await tx`
          UPDATE sarah_fleet_run_intake_leases
          SET state = 'released', updated_at = '2026-07-09T23:30:01.000Z'
          WHERE run_ref = ${fixture.runRef}
            AND claim_ref = ${fixture.claimRef}
        `
        markUpdated?.()
        await mayCommit
      })
      await updated

      let settled = false
      const reading = Effect.runPromise(
        exchange()
          .readPage({ ...fixture, after: 0, limit: 10 })
          .pipe(Effect.flip),
      ).then((error) => {
        settled = true
        return error
      })
      await new Promise((resolve) => setTimeout(resolve, 25))
      expect(settled).toBe(false)
      releaseTransaction?.()
      await releasing

      const error = await reading
      expect(error.kind).toBe("claim_conflict")
      const reservations: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count
        FROM sarah_fleet_run_steering_deliveries
        WHERE run_ref = ${fixture.runRef}
      `
      expect(Number(reservations[0]?.count ?? 0)).toBe(0)
    })
  },
)
