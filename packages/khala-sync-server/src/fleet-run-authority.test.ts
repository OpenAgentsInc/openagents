import { fleetRunScope } from "@openagentsinc/khala-sync"
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
  decodeFleetRunAuthorityStartRequest,
  FleetRunAuthorityError,
  makeFleetRunAuthorityRepository,
  publicFleetRunAuthorityRecord,
} from "./fleet-run-authority.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const FIXED_NOW = Date.parse("2026-07-09T22:00:00.000Z")
const COMMIT = "57f540bc13922351fee17fdd2c1b866c0fd21e86"

const request = (
  idempotencyKey: string,
  overrides: Readonly<{
    objective?: string
    targetPreference?: "owner_local" | "managed_cloud" | "auto"
    workerKind?: "codex" | "claude" | "grok" | "auto"
    workSource?: unknown
  }> = {},
) => ({
  objective: overrides.objective ?? "Implement one bounded public issue.",
  repository: {
    owner: "OpenAgentsInc",
    name: "openagents",
    branch: "main",
    commit: COMMIT,
  },
  verifier: { kind: "command", command: "bun test" },
  workSource: overrides.workSource ?? {
    kind: "issue_list",
    issueRefs: ["#8637"],
  },
  workerPolicy: {
    workerKind: overrides.workerKind ?? "codex",
    targetPreference: overrides.targetPreference ?? "owner_local",
  },
  targetConcurrency: 2,
  idempotencyKey,
})

describe("FleetRun authority request boundary", () => {
  test("normalizes the public contract and pins a full commit", () => {
    const decoded = decodeFleetRunAuthorityStartRequest(
      request("request-boundary-1"),
    )
    expect(decoded.schema).toBe("sarah.coding_fleet_start.request.v1")
    expect(decoded.repository.commit).toBe(COMMIT)
    expect(decoded.workSource).toEqual({
      kind: "issue_list",
      issueRefs: ["#8637"],
    })
    expect(
      decodeFleetRunAuthorityStartRequest({
        ...request("request-boundary-url"),
        workSource: {
          kind: "issue_list",
          issueRefs: [
            "https://github.com/openagentsinc/OpenAgents/issues/8637",
          ],
        },
      }).workSource,
    ).toEqual({ kind: "issue_list", issueRefs: ["#8637"] })
  })

  test("fails closed on private material, short pins, duplicate issues, and invalid DAGs", () => {
    expect(() =>
      decodeFleetRunAuthorityStartRequest({
        ...request("request-boundary-private"),
        objective: "Read /Users/operator/private before implementation.",
      }),
    ).toThrow(FleetRunAuthorityError)
    expect(() =>
      decodeFleetRunAuthorityStartRequest({
        ...request("request-boundary-pin"),
        repository: {
          ...request("unused").repository,
          commit: "57f540b",
        },
      }),
    ).toThrow(FleetRunAuthorityError)
    expect(() =>
      decodeFleetRunAuthorityStartRequest(
        request("request-boundary-dupe", {
          workSource: {
            kind: "issue_list",
            issueRefs: ["#8637", "#8637"],
          },
        }),
      ),
    ).toThrow(FleetRunAuthorityError)
    expect(() =>
      decodeFleetRunAuthorityStartRequest(
        request("request-boundary-cycle", {
          workSource: {
            kind: "plan_dag",
            planRef: "plan.fc1b",
            units: [
              { unitRef: "a", title: "A", dependsOn: ["b"] },
              { unitRef: "b", title: "B", dependsOn: ["a"] },
            ],
          },
        }),
      ),
    ).toThrow(FleetRunAuthorityError)
  })

  test("rejects cross-repository issue URLs, ambiguous Git refs, placeholder pins, and unsafe verifier argv", () => {
    const invalidRequests = [
      {
        ...request("request-boundary-cross-repo"),
        workSource: {
          kind: "issue_list",
          issueRefs: ["https://github.com/example/another/issues/8637"],
        },
      },
      {
        ...request("request-boundary-ref-prefix"),
        repository: { ...request("unused").repository, branch: "refs/heads/main" },
      },
      {
        ...request("request-boundary-ref-lock"),
        repository: { ...request("unused").repository, branch: "release.lock" },
      },
      {
        ...request("request-boundary-ref-component"),
        repository: { ...request("unused").repository, branch: "feature/.hidden" },
      },
      {
        ...request("request-boundary-placeholder-pin"),
        repository: { ...request("unused").repository, commit: "0".repeat(40) },
      },
      {
        ...request("request-boundary-shell-operator"),
        verifier: { kind: "command", command: "bun test && curl example.com" },
      },
      {
        ...request("request-boundary-absolute-verifier"),
        verifier: { kind: "command", command: "/usr/bin/bun test" },
      },
      {
        ...request("request-boundary-traversal-verifier"),
        verifier: { kind: "command", command: "bun test ../private.test.ts" },
      },
      {
        ...request("request-boundary-secret-verifier"),
        verifier: { kind: "command", command: "bun test secret/token" },
      },
      {
        ...request("request-boundary-unresolved-verifier"),
        verifier: { kind: "ref", ref: "command.public.unresolved" },
      },
    ]

    for (const invalid of invalidRequests) {
      expect(() => decodeFleetRunAuthorityStartRequest(invalid)).toThrow(
        FleetRunAuthorityError,
      )
    }
  })
})

describe.skipIf(!hasLocalPostgres())(
  "FleetRun authority against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE sarah_fleet_run_authority")
      await admin.end()
      const url = pg.urlFor("sarah_fleet_run_authority")
      const migrated = await runMigrations({ databaseUrl: url })
      expect(migrated.applied).toContain(
        "0052_sarah_fleet_run_authority.sql",
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

    const repository = (nowMs = FIXED_NOW) =>
      makeFleetRunAuthorityRepository({
        sql: sql as unknown as SyncSql,
        now: Effect.succeed(nowMs),
      })

    const start = async (
      ownerUserId: string,
      idempotencyKey: string,
      overrides: Parameters<typeof request>[1] = {},
    ) =>
      Effect.runPromise(
        repository().start({
          ownerUserId,
          request: request(idempotencyKey, overrides),
        }),
      )

    const seedPylon = async (input: {
      pylonRef: string
      ownerUserId: string
      heartbeatAt?: string
      heartbeatStatus?: string
      status?: string
    }) => {
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
          (${`registration.${input.pylonRef}`}, ${input.pylonRef},
           ${input.ownerUserId}, ${`credential.${input.pylonRef}`}, 'oa_agent',
           ${input.pylonRef}, ${input.status ?? "active"}, 'owner_local', '[]',
           0, ${input.heartbeatAt ?? nowIso},
           ${input.heartbeatStatus ?? "online"}, '[]', '[]', '[]', '[]', '[]',
           '{}', ${nowIso}, ${nowIso})
      `
    }

    const seedOwnerAgentLink = async (input: {
      ownerUserId: string
      agentUserId: string
    }) => {
      const nowIso = new Date(FIXED_NOW).toISOString()
      await sql`
        INSERT INTO openauth_agent_links
          (id, openauth_user_id, agent_user_id, agent_credential_id,
           link_kind, status, created_at, updated_at, revoked_at)
        VALUES
          (${`link.${input.ownerUserId}.${input.agentUserId}`},
           ${input.ownerUserId}, ${input.agentUserId},
           ${`credential.${input.agentUserId}`}, 'credential_anchor', 'active',
           ${nowIso}, ${nowIso}, NULL)
      `
    }

    test("creates run, work units, owner scope, and draft projection atomically", async () => {
      const first = await start("user-owner-a", "create-atomic-1")
      const replay = await start("user-owner-a", "create-atomic-1")
      const secondOwner = await start("user-owner-b", "create-atomic-1")

      expect(first.duplicate).toBe(false)
      expect(replay.duplicate).toBe(true)
      expect(replay.record.runRef).toBe(first.record.runRef)
      expect(secondOwner.record.runRef).not.toBe(first.record.runRef)
      expect(first.record.scope).toBe(fleetRunScope(first.record.runRef))

      const observed = await Effect.runPromise(
        repository().observe({
          ownerUserId: "user-owner-a",
          runRef: first.record.runRef,
        }),
      )
      expect(observed.record).toEqual(first.record)
      const crossOwnerObservation = await Effect.runPromise(
        repository()
          .observe({
            ownerUserId: "user-owner-b",
            runRef: first.record.runRef,
          })
          .pipe(Effect.flip),
      )
      expect(crossOwnerObservation.kind).toBe("run_not_found")

      const runRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_requests
        WHERE idempotency_key = 'create-atomic-1'
      `
      expect(Number(runRows[0]!.count)).toBe(2)
      const units: Array<{
        owner_user_id: string
        unit_ref: string
        issue_ref: string
        depends_on_refs_json: string
      }> = await sql`
        SELECT owner_user_id, unit_ref, issue_ref, depends_on_refs_json
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${first.record.runRef}
      `
      expect(units).toEqual([
        {
          owner_user_id: "user-owner-a",
          unit_ref: "issue.8637",
          issue_ref: "#8637",
          depends_on_refs_json: "[]",
        },
      ])
      const owners: Array<{ owner_user_id: string }> = await sql`
        SELECT owner_user_id FROM khala_sync_scope_owners
        WHERE scope = ${first.record.scope}
      `
      expect(owners).toEqual([{ owner_user_id: "user-owner-a" }])
      const changelog: Array<{
        entity_type: string
        post_image_json: unknown
        mutation_ref: string
      }> = await sql`
        SELECT entity_type, post_image_json, mutation_ref
        FROM khala_sync_changelog
        WHERE scope = ${first.record.scope}
      `
      expect(changelog).toHaveLength(1)
      expect(changelog[0]!.entity_type).toBe("fleet_run")
      expect(changelog[0]!.mutation_ref).toBe(
        "system:sarah_fleet_run_authority.create.v1",
      )
      const projected =
        typeof changelog[0]!.post_image_json === "string"
          ? changelog[0]!.post_image_json
          : JSON.stringify(changelog[0]!.post_image_json)
      expect(projected).toContain('"status":"draft"')
      expect(projected).not.toContain("user-owner-a")
      expect(projected).not.toContain("Implement one bounded public issue")

      const publicRecord = publicFleetRunAuthorityRecord(first.record)
      expect(publicRecord).not.toHaveProperty("ownerUserId")
      expect(publicRecord).not.toHaveProperty("requestFingerprint")
      expect(publicRecord).not.toHaveProperty("idempotencyKey")
    })

    test("same owner idempotency key with changed request is a fixed conflict", async () => {
      await start("user-owner-a", "create-conflict-1")
      const error = await Effect.runPromise(
        repository()
          .start({
            ownerUserId: "user-owner-a",
            request: request("create-conflict-1", {
              objective: "Implement a different bounded public issue.",
            }),
          })
          .pipe(Effect.flip),
      )
      expect(error.kind).toBe("idempotency_conflict")
      expect(error.reason).toBe(
        "fleet run idempotency key is already bound to another request",
      )
    })

    test("claim fails closed when normalized work-unit authority is tampered", async () => {
      const run = await start("user-integrity-owner", "integrity-run-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.integrity",
          units: [{ unitRef: "unit-a", title: "Original", dependsOn: [] }],
        },
      })
      await seedPylon({
        pylonRef: "pylon-integrity",
        ownerUserId: "user-integrity-owner",
      })
      await sql`
        UPDATE sarah_fleet_run_work_units SET title = 'Tampered'
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `
      const error = await Effect.runPromise(
        repository()
          .claim({
            ownerUserId: "user-integrity-owner",
            pylonRef: "pylon-integrity",
            runRef: run.record.runRef,
            claimIdempotencyKey: "claim-integrity-1",
            leaseDurationMs: 30_000,
          })
          .pipe(Effect.flip),
      )
      expect(error.kind).toBe("storage_unavailable")
      expect(error.reason).toBe(
        "fleet run work units failed integrity validation",
      )
      const leases: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_intake_leases
        WHERE run_ref = ${run.record.runRef}
      `
      expect(Number(leases[0]!.count)).toBe(0)
    })

    test("only an active, fresh, same-owner Pylon can claim one exact run", async () => {
      const run = await start("user-claim-owner", "claim-exact-run-1")
      await seedPylon({ pylonRef: "pylon-claim-a", ownerUserId: "user-claim-owner" })
      await seedPylon({ pylonRef: "pylon-claim-b", ownerUserId: "user-claim-owner" })
      await seedPylon({ pylonRef: "pylon-foreign", ownerUserId: "user-foreign" })
      await seedPylon({
        pylonRef: "pylon-stale",
        ownerUserId: "user-claim-owner",
        heartbeatAt: new Date(FIXED_NOW - 10 * 60 * 1_000).toISOString(),
      })

      const claimInput = {
        ownerUserId: "user-claim-owner",
        pylonRef: "pylon-claim-a",
        runRef: run.record.runRef,
        claimIdempotencyKey: "claim-exact-1",
        leaseDurationMs: 30_000,
      }
      const first = await Effect.runPromise(repository().claim(claimInput))
      const replay = await Effect.runPromise(repository().claim(claimInput))
      expect(first.duplicate).toBe(false)
      expect(replay.duplicate).toBe(true)
      expect(replay.claim.claimRef).toBe(first.claim.claimRef)
      expect(first.run.runRef).toBe(run.record.runRef)

      const busy = await Effect.runPromise(
        repository()
          .claim({
            ...claimInput,
            pylonRef: "pylon-claim-b",
            claimIdempotencyKey: "claim-exact-2",
          })
          .pipe(Effect.flip),
      )
      expect(busy.kind).toBe("claim_conflict")

      const wrongOwner = await Effect.runPromise(
        repository()
          .claim({
            ...claimInput,
            ownerUserId: "user-foreign",
            pylonRef: "pylon-foreign",
            claimIdempotencyKey: "claim-foreign-1",
          })
          .pipe(Effect.flip),
      )
      expect(wrongOwner.kind).toBe("run_not_found")

      const foreignPylon = await Effect.runPromise(
        repository()
          .claim({
            ...claimInput,
            pylonRef: "pylon-foreign",
            claimIdempotencyKey: "claim-foreign-2",
          })
          .pipe(Effect.flip),
      )
      expect(foreignPylon.kind).toBe("pylon_not_authorized")

      const stalePylon = await Effect.runPromise(
        repository()
          .claim({
            ...claimInput,
            pylonRef: "pylon-stale",
            claimIdempotencyKey: "claim-stale-1",
          })
          .pipe(Effect.flip),
      )
      expect(stalePylon.kind).toBe("pylon_unavailable")

      const acceptInput = {
        ownerUserId: "user-claim-owner",
        pylonRef: "pylon-claim-a",
        runRef: run.record.runRef,
        claimRef: first.claim.claimRef,
      }
      const accepted = await Effect.runPromise(
        repository().acceptClaim(acceptInput),
      )
      const acceptedReplay = await Effect.runPromise(
        repository().acceptClaim(acceptInput),
      )
      expect(accepted.duplicate).toBe(false)
      expect(acceptedReplay.duplicate).toBe(true)
      expect(accepted.claim.state).toBe("accepted")
      expect(accepted.run.status).toBe("claimed_by_pylon")

      const claimAfterAcceptance = await Effect.runPromise(
        repository()
          .claim({
            ...claimInput,
            pylonRef: "pylon-claim-b",
            claimIdempotencyKey: "claim-after-acceptance",
          })
          .pipe(Effect.flip),
      )
      expect(claimAfterAcceptance.kind).toBe("run_not_found")
      const startReplay = await start(
        "user-claim-owner",
        "claim-exact-run-1",
      )
      expect(startReplay.duplicate).toBe(true)
      expect(startReplay.record.status).toBe("claimed_by_pylon")
    })

    test("a browser owner can claim through an actively linked Pylon agent only", async () => {
      const run = await start("user-browser-owner", "claim-linked-run-1")
      await seedPylon({
        pylonRef: "pylon-linked-agent",
        ownerUserId: "agent-linked-owner",
      })
      await seedOwnerAgentLink({
        ownerUserId: "user-browser-owner",
        agentUserId: "agent-linked-owner",
      })
      const claimed = await Effect.runPromise(
        repository().claim({
          ownerUserId: "user-browser-owner",
          pylonRef: "pylon-linked-agent",
          runRef: run.record.runRef,
          claimIdempotencyKey: "claim-linked-1",
          leaseDurationMs: 30_000,
        }),
      )
      expect(claimed.run.ownerUserId).toBe("user-browser-owner")
      expect(claimed.claim.pylonRef).toBe("pylon-linked-agent")

      await sql`
        UPDATE openauth_agent_links SET status = 'revoked', revoked_at = ${new Date(FIXED_NOW).toISOString()}
        WHERE openauth_user_id = 'user-browser-owner'
          AND agent_user_id = 'agent-linked-owner'
      `
      const replayAfterRevocation = await Effect.runPromise(
        repository()
          .claim({
            ownerUserId: "user-browser-owner",
            pylonRef: "pylon-linked-agent",
            runRef: run.record.runRef,
            claimIdempotencyKey: "claim-linked-1",
            leaseDurationMs: 30_000,
          })
          .pipe(Effect.flip),
      )
      expect(replayAfterRevocation.kind).toBe("pylon_not_authorized")
    })

    test("claim-next skips managed-cloud runs and concurrent exact claims have one winner", async () => {
      await seedPylon({ pylonRef: "pylon-next-a", ownerUserId: "user-next-owner" })
      await seedPylon({ pylonRef: "pylon-next-b", ownerUserId: "user-next-owner" })
      const cloud = await start("user-next-owner", "next-cloud-1", {
        targetPreference: "managed_cloud",
      })
      const local = await start("user-next-owner", "next-local-1", {
        targetPreference: "owner_local",
      })
      const next = await Effect.runPromise(
        repository().claim({
          ownerUserId: "user-next-owner",
          pylonRef: "pylon-next-a",
          claimIdempotencyKey: "claim-next-1",
          leaseDurationMs: 30_000,
        }),
      )
      expect(next.run.runRef).toBe(local.record.runRef)
      expect(next.run.runRef).not.toBe(cloud.record.runRef)

      const race = await start("user-next-owner", "next-race-1")
      const attempts = await Promise.allSettled([
        Effect.runPromise(
          repository().claim({
            ownerUserId: "user-next-owner",
            pylonRef: "pylon-next-a",
            runRef: race.record.runRef,
            claimIdempotencyKey: "claim-race-a",
            leaseDurationMs: 30_000,
          }),
        ),
        Effect.runPromise(
          repository().claim({
            ownerUserId: "user-next-owner",
            pylonRef: "pylon-next-b",
            runRef: race.record.runRef,
            claimIdempotencyKey: "claim-race-b",
            leaseDurationMs: 30_000,
          }),
        ),
      ])
      expect(attempts.filter(result => result.status === "fulfilled")).toHaveLength(1)
      expect(attempts.filter(result => result.status === "rejected")).toHaveLength(1)
      const leases: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_intake_leases
        WHERE run_ref = ${race.record.runRef} AND state = 'claimed'
      `
      expect(Number(leases[0]!.count)).toBe(1)
    })

    test("an expired claim requires a new idempotency key and can be re-leased", async () => {
      const run = await start("user-expiry-owner", "expiry-run-1")
      await seedPylon({ pylonRef: "pylon-expiry-a", ownerUserId: "user-expiry-owner" })
      await seedPylon({ pylonRef: "pylon-expiry-b", ownerUserId: "user-expiry-owner" })
      const originalInput = {
        ownerUserId: "user-expiry-owner",
        pylonRef: "pylon-expiry-a",
        runRef: run.record.runRef,
        claimIdempotencyKey: "claim-expiry-a",
        leaseDurationMs: 5_000,
      }
      await Effect.runPromise(repository().claim(originalInput))
      const afterExpiry = repository(FIXED_NOW + 6_000)
      const expiredReplay = await Effect.runPromise(
        afterExpiry.claim(originalInput).pipe(Effect.flip),
      )
      expect(expiredReplay.kind).toBe("claim_expired")
      const reclaimed = await Effect.runPromise(
        afterExpiry.claim({
          ...originalInput,
          pylonRef: "pylon-expiry-b",
          claimIdempotencyKey: "claim-expiry-b",
        }),
      )
      expect(reclaimed.duplicate).toBe(false)
      expect(reclaimed.claim.pylonRef).toBe("pylon-expiry-b")
    })
  },
)

test("storage errors collapse to fixed public-safe diagnostics", async () => {
  const broken = {
    begin: async () => {
      throw new Error("postgres://operator:secret@private-host/database")
    },
  } as unknown as SyncSql
  const repository = makeFleetRunAuthorityRepository({
    sql: broken,
    now: Effect.succeed(FIXED_NOW),
  })
  const error = await Effect.runPromise(
    repository
      .start({ ownerUserId: "user-storage", request: request("storage-1") })
      .pipe(Effect.flip),
  )
  expect(error.kind).toBe("storage_unavailable")
  expect(error.reason).toBe("fleet run authority is unavailable")
  expect(JSON.stringify(error)).not.toContain("private-host")
  expect(JSON.stringify(error)).not.toContain("secret")
})
