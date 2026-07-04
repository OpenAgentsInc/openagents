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
import {
  FLEET_PAUSE_RUN_MUTATOR_NAME,
  FLEET_RESUME_RUN_MUTATOR_NAME,
  FLEET_SCOPE_REJECTION,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  fleetOperatorMutators,
} from "./fleet-mutators.js"
import {
  canReadScopeV1,
  ensureScopeOwner,
  FLEET_POST_IMAGE_FORBIDDEN_PATTERN,
  FLEET_PROJECTION_SYSTEM_REF,
  fleetAccountPostImage,
  fleetAssignmentPostImage,
  fleetRunPostImage,
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

const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-fleet-${clientCounter}`),
    clientId: ClientId.make(`c-fleet-${clientCounter}`),
    userId: `user-fleet-${clientCounter}`,
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
// Redaction mapping property (no database needed)
// ---------------------------------------------------------------------------

describe("fleet redaction mappings", () => {
  const nowIso = "2026-07-04T15:20:11.412Z"

  test("post-images never carry forbidden material even when raw rows do", () => {
    // Raw rows deliberately polluted with private material on EXTRA fields:
    // the allowlist mapping must not pick any of it up.
    const dirtyRun = {
      apiKey: "sk-live-EXTREMELY-SECRET",
      authorization: "Bearer abc123",
      counters: { activeAssignments: 1 },
      prompt: "implement the thing at /Users/alice/work",
      runRef: "fleet-run.pylon.supervisor.abc123",
      startedAt: nowIso,
      state: "running",
      targetConcurrency: 3,
      updatedAt: nowIso,
      workerKind: "codex",
      worktreePath: "/Users/alice/work/openagents-wt",
    }
    const dirtyWorker = {
      accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f",
      assigneeHandle: "alice@example.com",
      currentAssignmentRef: "assignment.public.issue1.1",
      id: "dispatch-context.pylon.supervisor.9ab31c44",
      sessionToken: "tok_super_secret",
      status: "dispatched",
      updatedAt: nowIso,
      worktreePath: "/Users/alice/work/openagents-wt-x",
    }
    const dirtyAssignment = {
      assignmentRef: "assignment.public.issue8302.1",
      codingAssignment: {
        codex: { authorization: "Bearer xyz" },
        workspace: { path: "/Users/alice/work" },
      },
      issueRef: "#8302",
      state: "accepted_work",
      updatedAt: nowIso,
    }
    const dirtyAccount = {
      accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f",
      email: "alice@example.com",
      readiness: "cooldown",
      refreshToken: "rt_secret",
      updatedAt: nowIso,
    }

    const images = [
      fleetRunPostImage(dirtyRun),
      fleetWorkerPostImage(dirtyWorker),
      fleetAssignmentPostImage(dirtyAssignment),
      fleetAccountPostImage(dirtyAccount),
    ]
    for (const image of images) {
      expect(canonicalJson({ ...image })).not.toMatch(
        FLEET_POST_IMAGE_FORBIDDEN_PATTERN,
      )
      expect(canonicalJson({ ...image })).not.toContain("alice@example.com")
    }
  })

  test("terminal states derive closeoutClass; live states do not", () => {
    const base = {
      assignmentRef: "assignment.public.issue1.1",
      updatedAt: nowIso,
    }
    expect(
      fleetAssignmentPostImage({ ...base, state: "accepted_work" })
        .closeoutClass,
    ).toBe("accepted_work")
    expect(
      fleetAssignmentPostImage({ ...base, state: "rejected" }).closeoutClass,
    ).toBe("rejected")
    expect(
      fleetAssignmentPostImage({ ...base, state: "running" }).closeoutClass,
    ).toBeUndefined()
  })

  test("a raw row whose ALLOWLISTED field carries a path fails to decode", () => {
    expect(() =>
      fleetWorkerPostImage({
        id: "/Users/alice/.pylon/context",
        status: "idle",
        updatedAt: nowIso,
      }),
    ).toThrow()
  })
})

// ---------------------------------------------------------------------------
// Fail-soft wrapper (no working database: must return a diagnostic)
// ---------------------------------------------------------------------------

describe("projectFleetEntitiesBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused")
      },
    } as unknown as SyncSql
    const outcome = await projectFleetEntitiesBestEffort({
      changes: [
        {
          entity: fleetRunPostImage({
            runRef: "fleet-run.test.1",
            startedAt: null,
            state: "running",
            targetConcurrency: 1,
            updatedAt: "2026-07-04T15:20:11.412Z",
            workerKind: "auto",
          }),
          kind: "fleet_run",
          op: "upsert",
        },
      ],
      ownerUserId: "user-1",
      runId: "fleet-run.test.1",
      sql: broken,
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toMatch(
        FLEET_POST_IMAGE_FORBIDDEN_PATTERN,
      )
    }
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "fleet projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_fleet")
      await admin.end()
      const url = pg.urlFor("khala_sync_fleet")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0004_khala_sync_fleet.sql")
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    const nowIso = "2026-07-04T15:20:11.412Z"

    test("ensureScopeOwner is first-writer-wins and idempotent", async () => {
      const scope = fleetRunScope("fleet-run.owner.1")
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBeNull()
      expect(
        await ensureScopeOwner(sql as unknown as SyncSql, scope, "user-a"),
      ).toBe("user-a")
      expect(
        await ensureScopeOwner(sql as unknown as SyncSql, scope, "user-a"),
      ).toBe("user-a")
      // A different user does NOT take over.
      expect(
        await ensureScopeOwner(sql as unknown as SyncSql, scope, "user-b"),
      ).toBe("user-a")
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBe(
        "user-a",
      )
    })

    test("canReadScopeV1: own personal scope + owned fleet scopes only", async () => {
      const scope = fleetRunScope("fleet-run.read.1")
      await ensureScopeOwner(sql as unknown as SyncSql, scope, "reader-1")
      const s = sql as unknown as SyncSql
      expect(await canReadScopeV1(s, "reader-1", scope)).toBe(true)
      expect(await canReadScopeV1(s, "reader-2", scope)).toBe(false)
      expect(
        await canReadScopeV1(
          s,
          "reader-1",
          fleetRunScope("fleet-run.read.unowned"),
        ),
      ).toBe(false)
      const { personalScope } = await import("@openagentsinc/khala-sync")
      expect(
        await canReadScopeV1(s, "reader-1", personalScope("reader-1")),
      ).toBe(true)
      expect(
        await canReadScopeV1(s, "reader-1", personalScope("reader-2")),
      ).toBe(false)
      expect(
        await canReadScopeV1(
          s,
          "reader-1",
          "scope.team.reader-1" as never,
        ),
      ).toBe(false)
    })

    test("projection appends entities + claims the scope owner in one transaction", async () => {
      const runId = "fleet-run.proj.1"
      const scope = fleetRunScope(runId)
      const outcome = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              counters: { activeAssignments: 2, workUnitsTotal: 9 },
              runRef: runId,
              startedAt: nowIso,
              state: "running",
              targetConcurrency: 2,
              updatedAt: nowIso,
              workerKind: "codex",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
          {
            entity: fleetAssignmentPostImage({
              assignmentRef: "assignment.public.issue8302.1",
              issueRef: "#8302",
              state: "offered",
              updatedAt: nowIso,
            }),
            kind: "fleet_assignment",
            op: "upsert",
          },
        ],
        ownerUserId: "proj-owner",
        runId,
        sql: sql as unknown as SyncSql,
      })
      expect(outcome.ok).toBe(true)
      if (outcome.ok) {
        expect(outcome.entries).toHaveLength(2)
        // One transaction ⇒ one scope version for both entries.
        expect(Number(outcome.entries[0]!.version)).toBe(
          Number(outcome.entries[1]!.version),
        )
        for (const entry of outcome.entries) {
          expect(entry.mutationRef).toBe(FLEET_PROJECTION_SYSTEM_REF)
        }
      }
      expect(
        await readScopeOwner(sql as unknown as SyncSql, scope),
      ).toBe("proj-owner")

      const rows: Array<{
        entity_type: string
        entity_id: string
        post_image_json: object | string
      }> = await sql`
        SELECT entity_type, entity_id, post_image_json
        FROM khala_sync_changelog WHERE scope = ${scope}
        ORDER BY entity_type
      `
      expect(rows.map((r) => r.entity_type)).toEqual([
        "fleet_assignment",
        "fleet_run",
      ])
      // Redaction property over what actually landed in the log.
      for (const row of rows) {
        const serialized =
          typeof row.post_image_json === "string"
            ? row.post_image_json
            : JSON.stringify(row.post_image_json)
        expect(serialized).not.toMatch(FLEET_POST_IMAGE_FORBIDDEN_PATTERN)
      }
    })

    test("projection into a scope owned by ANOTHER user refuses with a diagnostic and writes nothing", async () => {
      const runId = "fleet-run.proj.foreign"
      const scope = fleetRunScope(runId)
      await ensureScopeOwner(sql as unknown as SyncSql, scope, "the-owner")
      const outcome = await projectFleetEntitiesBestEffort({
        changes: [
          {
            entity: fleetRunPostImage({
              runRef: runId,
              startedAt: null,
              state: "running",
              targetConcurrency: 1,
              updatedAt: nowIso,
              workerKind: "auto",
            }),
            kind: "fleet_run",
            op: "upsert",
          },
        ],
        ownerUserId: "an-intruder",
        runId,
        sql: sql as unknown as SyncSql,
      })
      expect(outcome.ok).toBe(false)
      if (!outcome.ok) {
        expect(outcome.diagnostic.reason).toBe("scope_owned_by_other_user")
      }
      const rows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog WHERE scope = ${scope}
      `
      expect(Number(rows[0]!.count)).toBe(0)
      // Ownership did not migrate.
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBe(
        "the-owner",
      )
    })

    test("delete changes append tombstones", async () => {
      const runId = "fleet-run.proj.delete"
      const scope = fleetRunScope(runId)
      const outcome = await projectFleetEntitiesBestEffort({
        changes: [
          { entityId: "assignment.public.gone.1", kind: "fleet_assignment", op: "delete" },
        ],
        ownerUserId: "del-owner",
        runId,
        sql: sql as unknown as SyncSql,
      })
      expect(outcome.ok).toBe(true)
      const rows: Array<{ op: string; post_image_json: unknown }> = await sql`
        SELECT op, post_image_json FROM khala_sync_changelog WHERE scope = ${scope}
      `
      expect(rows).toHaveLength(1)
      expect(rows[0]!.op).toBe("delete")
      expect(rows[0]!.post_image_json).toBeNull()
    })

    // -----------------------------------------------------------------------
    // Operator mutators through executePush
    // -----------------------------------------------------------------------

    const registry = makeMutatorRegistry([...fleetOperatorMutators])

    test("owner applies setDesiredSlots/pause/resume; intents + post-images land", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut.1"
      const scope = fleetRunScope(runId)

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME, {
            desiredSlots: 7,
            runId,
          }),
          envelope(2, FLEET_PAUSE_RUN_MUTATOR_NAME, { runId }),
          envelope(3, FLEET_RESUME_RUN_MUTATOR_NAME, { runId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual([
        "applied",
        "applied",
        "applied",
      ])
      expect(Number(response.lastMutationId)).toBe(3)

      // First mutation claimed the scope for the caller.
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBe(
        client.userId,
      )

      // Three intents, in order, attributable to their mutations.
      const intents: Array<{
        intent: string
        desired_slots: number | null
        requested_by_user_id: string
        mutation_ref: string
      }> = await sql`
        SELECT intent, desired_slots, requested_by_user_id, mutation_ref
        FROM khala_sync_fleet_intents WHERE scope = ${scope} ORDER BY id
      `
      expect(intents.map((i) => i.intent)).toEqual([
        "set_desired_slots",
        "pause",
        "resume",
      ])
      expect(intents[0]!.desired_slots).toBe(7)
      expect(intents[1]!.desired_slots).toBeNull()
      for (const intent of intents) {
        expect(intent.requested_by_user_id).toBe(client.userId)
        expect(intent.mutation_ref).toMatch(/^mutation:/)
      }

      // The final post-image reflects the whole intent sequence.
      const rows: Array<{ post_image_json: object | string; version: unknown }> =
        await sql`
        SELECT post_image_json, version FROM khala_sync_changelog
        WHERE scope = ${scope} AND entity_type = 'fleet_run'
        ORDER BY version DESC LIMIT 1
      `
      const image =
        typeof rows[0]!.post_image_json === "string"
          ? (JSON.parse(rows[0]!.post_image_json as string) as {
              desiredSlots: number
              status: string
            })
          : (rows[0]!.post_image_json as { desiredSlots: number; status: string })
      expect(image.desiredSlots).toBe(7)
      expect(image.status).toBe("running")
      // Dense per-scope versions: 3 mutations ⇒ version 3 at the head.
      expect(Number(rows[0]!.version)).toBe(3)
    })

    test("a FOREIGN user is rejected in-band with zero writes; queue never blocks", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const runId = "fleet-run.mut.foreign"
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
          envelope(1, FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME, {
            desiredSlots: 99,
            runId,
          }),
          // The queue must not block behind the rejection: a mutation on the
          // intruder's OWN run applies next.
          envelope(2, FLEET_PAUSE_RUN_MUTATOR_NAME, {
            runId: "fleet-run.mut.intruder-own",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })
      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe(FLEET_SCOPE_REJECTION)
      expect(response.results[1]!.status).toBe("applied")
      expect(Number(response.lastMutationId)).toBe(2)

      // Zero writes on the foreign scope: no new changelog rows, no intents.
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
      // Ownership did not migrate.
      expect(await readScopeOwner(sql as unknown as SyncSql, scope)).toBe(
        owner.userId,
      )
    })

    test("bad args reject in-band without echoing values", async () => {
      const client = freshClient()
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME, {
            desiredSlots: -3,
            runId: "fleet-run.mut.bad",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe("invalid_args")
      expect(response.results[0]!.errorMessageSafe ?? "").not.toContain("-3")
    })

    test("duplicate replay answers from the ledger without re-executing", async () => {
      const client = freshClient()
      const runId = "fleet-run.mut.dup"
      const request = pushRequest(client, [
        envelope(1, FLEET_PAUSE_RUN_MUTATOR_NAME, { runId }),
      ])
      const first = await executePush({
        registry,
        request,
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(first.results[0]!.status).toBe("applied")
      const replay = await executePush({
        registry,
        request,
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(replay.results[0]!.status).toBe("duplicate")
      const intents: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_fleet_intents
        WHERE scope = ${fleetRunScope(runId)}
      `
      expect(Number(intents[0]!.count)).toBe(1)
    })
  },
)
