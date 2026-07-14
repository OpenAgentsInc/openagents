import { createHash } from "node:crypto"

import {
  canonicalJson,
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetWorkerEntity,
  fleetRunScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"

import {
  decodeFleetRunAuthorityStartRequest,
  decodeFleetRunExecutionBatch,
  decodeFleetRunWorkUnitCloseoutRow,
  FLEET_RUN_EXECUTION_BATCH_SCHEMA,
  FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
  FLEET_RUN_EXECUTION_EVENT_SCHEMA,
  FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
  FleetRunAuthorityError,
  type FleetRunExecutionEventV2,
  makeFleetRunAuthorityRepository,
  publicFleetRunAuthorityRecord,
} from "./fleet-run-authority.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"
const FIXED_NOW = Date.parse("2026-07-09T22:00:00.000Z")
const COMMIT = "57f540bc13922351fee17fdd2c1b866c0fd21e86"
const FIXTURE_RUN_REF = "fleet_run.sarah.0123456789abcdef0123"

const pylonExecutionEvent = <
  const Event extends Readonly<Record<string, unknown>>,
>(
  runRef: string,
  claimRef: string,
  sequence: number,
  event: Event,
) => ({
  ...event,
  sequence,
  eventRef: `event.pylon.fleet_run.${createHash("sha256")
    .update(canonicalJson({ runRef, claimRef, event }))
    .digest("hex")
    .slice(0, 24)}`,
})

const legacyBlockerDigestRef = (blockerRef: string): string =>
  `blocker.pylon.fleet_run.legacy.${createHash("sha256")
    .update(canonicalJson(blockerRef))
    .digest("hex")
    .slice(0, 24)}`

const legacyProjectedDigestRef = (
  safePrefix: string,
  sourceRef: string,
): string =>
  `${safePrefix}.${createHash("sha256")
    .update(canonicalJson(sourceRef))
    .digest("hex")
    .slice(0, 24)}`

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

const exactUsageEvidence = (
  assignmentRef: string,
  pylonRef: string,
  suffix: string,
) => ({
  schema: "openagents.pylon.fleet_run_usage_evidence.v1" as const,
  truth: "exact" as const,
  harnessKind: "codex" as const,
  evidenceRef: `evidence.public.pylon.fleet_run.exact.${suffix}`,
  assignmentRef,
  pylonRef,
  provider: "pylon-codex-own-capacity" as const,
  model: "openagents/pylon-codex" as const,
  demandKind: "own_capacity" as const,
  demandSource: "khala_coding_delegation" as const,
  inputTokens: 8,
  outputTokens: 5,
  reasoningTokens: 2,
  cacheReadTokens: 3,
  totalTokens: 13,
  tokenRows: 1,
  tokenUsageRefs: [`usage_row.${suffix}`],
  proofRefs: [`proof.usage.${suffix}`],
  closeoutChecklistRefs: [`check.closeout.${suffix}`],
  proofChecklistRefs: [`check.proof.${suffix}`],
})

const notMeasuredUsageEvidence = (assignmentRef: string, suffix: string) => ({
  schema: "openagents.pylon.fleet_run_usage_evidence.v1" as const,
  truth: "not_measured" as const,
  harnessKind: "grok" as const,
  evidenceRef: `evidence.public.pylon.fleet_run.not_measured.${suffix}`,
  assignmentRef,
  receiptRef: `receipt.public.pylon.grok.${suffix}`,
  tokenUsageRefs: [] as const,
  caveatRefs: ["caveat.pylon.fleet_run.grok_usage_not_measured"],
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

  test("retains independent per-unit placement authority inside one auto run", () => {
    const placement = (targetPreference: "owner_local" | "managed_cloud") => ({
      targetPreference,
      quotaClass: targetPreference === "owner_local"
        ? "owner_subscription" as const
        : "brokered_credit" as const,
      maxMarginalCostClass: targetPreference === "owner_local"
        ? "subscription" as const
        : "api_metered" as const,
      dataPosture: targetPreference === "owner_local"
        ? "owner_private" as const
        : "broker_safe" as const,
      repositoryConstraint: targetPreference === "owner_local"
        ? "owner_local_allowed" as const
        : "managed_allowed" as const,
      taskConstraint: targetPreference === "owner_local"
        ? "local_ok" as const
        : "managed_required" as const,
    })
    const decoded = decodeFleetRunAuthorityStartRequest(request(
      "request-boundary-hybrid",
      {
        targetPreference: "auto",
        workSource: {
          kind: "plan_dag",
          planRef: "plan.fc4.hybrid",
          units: [
            { unitRef: "unit.local", title: "Local", objective: "Implement the local unit.", placement: placement("owner_local") },
            { unitRef: "unit.managed", title: "Managed", objective: "Implement the managed unit.", placement: placement("managed_cloud") },
          ],
        },
      },
    ))
    expect(decoded.workSource).toEqual({
      kind: "plan_dag",
      planRef: "plan.fc4.hybrid",
      units: [
        { unitRef: "unit.local", title: "Local", objective: "Implement the local unit.", dependsOn: [], placement: placement("owner_local") },
        { unitRef: "unit.managed", title: "Managed", objective: "Implement the managed unit.", dependsOn: [], placement: placement("managed_cloud") },
      ],
    })
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
        repository: {
          ...request("unused").repository,
          branch: "refs/heads/main",
        },
      },
      {
        ...request("request-boundary-ref-lock"),
        repository: { ...request("unused").repository, branch: "release.lock" },
      },
      {
        ...request("request-boundary-ref-component"),
        repository: {
          ...request("unused").repository,
          branch: "feature/.hidden",
        },
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

  test("accepts only bounded, provider-consistent execution evidence", () => {
    const claimRef = `claim.sarah_fleet_run.${"a".repeat(24)}`
    const valid = decodeFleetRunExecutionBatch({
      schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef,
      events: [
        pylonExecutionEvent(FIXTURE_RUN_REF, claimRef, 7, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:00.000Z",
          kind: "work_terminal",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a",
          assignmentRef: "assignment.unit-a",
          workerKind: "codex",
          accountRefHash: `account.pylon.codex.${"a".repeat(24)}`,
          terminalState: "accepted",
          closeoutRef: "closeout.unit-a",
          usageEvidence: {
            truth: "exact",
            tokenUsageRefs: ["token_usage.unit-a.1"],
          },
          blockerRefs: [],
        }),
      ],
    })
    expect(valid.events[0]?.kind).toBe("work_terminal")
    const failedClaimRef = `claim.sarah_fleet_run.${"b".repeat(24)}`
    const unprovenFailure = decodeFleetRunExecutionBatch({
      schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef: failedClaimRef,
      events: [
        pylonExecutionEvent(FIXTURE_RUN_REF, failedClaimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:00.000Z",
          kind: "work_terminal",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a",
          workerKind: "claude",
          terminalState: "failed",
          blockerRefs: ["blocker.dispatch_no_assignment"],
        }),
      ],
    })
    expect(unprovenFailure.events[0]).not.toHaveProperty("assignmentRef")

    const invalid = [
      { ...valid, events: [] },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            usageEvidence: { truth: "not_measured", tokenUsageRefs: [] },
          },
        ],
      },
      {
        ...unprovenFailure,
        events: [
          {
            ...unprovenFailure.events[0],
            blockerRefs: ["blocker.path/Users/operator/worktree"],
          },
        ],
      },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            workClaimRef: "work_claim.path/Users/operator/worktree",
          },
        ],
      },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            assignmentRef: "assignment.path/Users/operator/worktree",
          },
        ],
      },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            closeoutRef: "closeout.path/Users/operator/worktree",
          },
        ],
      },
      {
        ...unprovenFailure,
        events: [
          {
            ...unprovenFailure.events[0],
            assignmentRef: "assignment.partial",
          },
        ],
      },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            workerKind: "grok",
            accountRefHash: `account.pylon.grok.${"b".repeat(24)}`,
            usageEvidence: {
              truth: "exact",
              tokenUsageRefs: ["token_usage.unit-a.1"],
            },
          },
        ],
      },
      {
        ...valid,
        events: [
          valid.events[0],
          {
            ...valid.events[0],
            sequence: 9,
            eventRef: `event.pylon.fleet_run.${"c".repeat(24)}`,
          },
        ],
      },
      {
        ...valid,
        events: [
          {
            ...valid.events[0],
            closeoutRef: "/Users/operator/private/closeout",
          },
        ],
      },
    ]
    for (const batch of invalid) {
      expect(() => decodeFleetRunExecutionBatch(batch)).toThrow(
        FleetRunAuthorityError,
      )
    }
  })

  test("v2 requires complete verifier, artifact, proof, and authority receipts", () => {
    const claimRef = `claim.sarah_fleet_run.${"c".repeat(24)}`
    const accepted = pylonExecutionEvent(FIXTURE_RUN_REF, claimRef, 1, {
      schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
      observedAt: "2026-07-09T22:00:00.000Z",
      kind: "work_terminal",
      unitRef: "unit-a",
      workClaimRef: "work_claim.unit-a.attempt-1",
      assignmentRef: "assignment.unit-a.edge-1",
      workerKind: "codex",
      accountRefHash: `account.pylon.codex.${"a".repeat(24)}`,
      terminalState: "accepted",
      closeoutRef: "closeout.unit-a.attempt-1",
      verification: {
        truth: "passed",
        verifierRef: "verifier.bun-test.1",
        evidenceRefs: ["test.unit-a.1"],
      },
      artifactRefs: ["artifact.patch.unit-a.1"],
      proofRefs: ["proof.unit-a.1"],
      authorityReceiptRefs: ["receipt.authority.unit-a.1"],
      usageEvidence: exactUsageEvidence(
        "assignment.unit-a.edge-1",
        "pylon-fixture-v2",
        "unit-a.1",
      ),
      blockerRefs: [],
    })
    expect(
      decodeFleetRunExecutionBatch({
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef,
        events: [accepted],
      }).events[0],
    ).toMatchObject({ schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2 })
    expect(() =>
      decodeFleetRunExecutionBatch({
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef,
        events: [{
          ...accepted,
          verification: {
            ...accepted.verification,
            evidenceRefs: ["evidence.shared.role"],
          },
          artifactRefs: ["evidence.shared.role"],
        }],
      }),
    ).not.toThrow()

    const managedNotMeasured = {
      ...accepted,
      capacityClass: "managed_cloud" as const,
      marginalCostClass: "api_metered" as const,
      usageEvidence: {
        schema: "openagents.pylon.fleet_run_usage_evidence.v1" as const,
        truth: "not_measured" as const,
        harnessKind: "codex" as const,
        evidenceRef: "evidence.managed.no_token_measurement",
        assignmentRef: "assignment.unit-a.edge-1",
        receiptRef: "receipt.agent_computer.resource_usage",
        tokenUsageRefs: [],
        caveatRefs: ["caveat.agent_computer.token_usage_not_measured"],
      },
    }
    expect(() =>
      decodeFleetRunExecutionBatch({
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef,
        events: [managedNotMeasured],
      }),
    ).not.toThrow()

    const invalid = [
      { ...accepted, proofRefs: undefined },
      { ...accepted, rawPrompt: "must not cross" },
      { ...accepted, artifactRefs: [] },
      { ...accepted, workClaimRef: "work/claim#not-projectable" },
      { ...accepted, blockerRefs: ["blocker.not/projectable"] },
      { ...accepted, blockerRefs: ["blocker.not#projectable"] },
      {
        ...accepted,
        artifactRefs: ["artifact.duplicate", "artifact.duplicate"],
      },
      { ...managedNotMeasured, capacityClass: "owner_local" },
      {
        ...pylonExecutionEvent(FIXTURE_RUN_REF, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:00.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a.duplicate-blocker",
          workerKind: "codex",
        }),
        blockerRefs: ["blocker.duplicate", "blocker.duplicate"],
      },
      {
        ...accepted,
        usageEvidence: {
          ...exactUsageEvidence(
            "assignment.unit-a.edge-1",
            "pylon-fixture-v2",
            "unit-a.invalid-total",
          ),
          totalTokens: 1,
        },
      },
      {
        ...accepted,
        usageEvidence: {
          ...accepted.usageEvidence,
          proofRefs: ["proof.duplicate", "proof.duplicate"],
        },
      },
    ]
    for (const event of invalid) {
      expect(() =>
        decodeFleetRunExecutionBatch({
          schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
          claimRef,
          events: [event],
        }),
      ).toThrow(FleetRunAuthorityError)
    }
  })

  test("v2 approval requests require bounded public-safe exact binding metadata", () => {
    const claimRef = `claim.sarah_fleet_run.${"d".repeat(24)}`
    const approval = pylonExecutionEvent(FIXTURE_RUN_REF, claimRef, 1, {
      schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
      observedAt: "2026-07-09T22:00:00.000Z",
      kind: "approval_requested",
      unitRef: "unit-a",
      workClaimRef: "work_claim.unit-a.attempt-1",
      workerKind: "codex",
      workerRef: "worker.codex.slot-1",
      approvalRef: "approval.unit-a.tool-1",
      toolClass: "write_file",
      blockerRefs: ["blocker.approval_required"],
    })
    expect(
      decodeFleetRunExecutionBatch({
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef,
        events: [approval],
      }).events[0],
    ).toMatchObject({ kind: "approval_requested" })

    for (const invalid of [
      { ...approval, workerRef: undefined },
      { ...approval, blockerRefs: [] },
      { ...approval, rawToolArgs: "PRIVATE TOOL ARGS SENTINEL" },
      { ...approval, workerRef: "/Users/operator/private-worker" },
    ]) {
      expect(() =>
        decodeFleetRunExecutionBatch({
          schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
          claimRef,
          events: [invalid],
        }),
      ).toThrow(FleetRunAuthorityError)
    }
  })

  test("closeout readback rejects owner-local Codex not-measured corruption", () => {
    const row = {
      run_ref: FIXTURE_RUN_REF,
      unit_ref: "unit-a",
      work_claim_ref: "work_claim.unit-a.managed",
      assignment_ref: "assignment.unit-a.managed",
      worker_kind: "codex" as const,
      account_ref_hash: `account.pylon.codex.${"a".repeat(24)}`,
      terminal_state: "accepted" as const,
      closeout_ref: "closeout.unit-a.managed",
      usage_truth: "not_measured" as const,
      token_usage_refs_json: "[]",
      blocker_refs_json: "[]",
      observed_at: "2026-07-09T22:00:00.000Z",
      event_ref: `event.pylon.fleet_run.${"a".repeat(24)}`,
      capacity_class: "managed_cloud" as const,
    }
    expect(() => decodeFleetRunWorkUnitCloseoutRow(row)).not.toThrow()
    expect(() => decodeFleetRunWorkUnitCloseoutRow({
      ...row,
      capacity_class: "owner_local",
    })).toThrow(FleetRunAuthorityError)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "FleetRun authority against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE sarah_fleet_run_authority")
      await admin.end()
      const url = pg.urlFor("sarah_fleet_run_authority")
      const migrated = await runMigrations({ databaseUrl: url })
      expect(migrated.applied).toContain("0052_sarah_fleet_run_authority.sql")
      expect(migrated.applied).toContain(
        "0053_sarah_fleet_run_execution_projection.sql",
      )
      expect(migrated.applied).toContain(
        "0056_sarah_fleet_run_attempts.sql",
      )
      expect(migrated.applied).toContain(
        "0057_sarah_fleet_run_approval_requested.sql",
      )
      expect(migrated.applied).toContain(
        "0060_fleet_attempt_claude_cache_usage.sql",
      )
      expect(migrated.applied).toContain(
        "0065_sarah_fleet_run_managed_cloud_capacity.sql",
      )
      sql = SQL({ url, max: 12 })
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

    const claimAndAccept = async (input: {
      ownerUserId: string
      pylonRef: string
      runRef: string
      claimIdempotencyKey: string
    }) => {
      const claimed = await Effect.runPromise(
        repository().claim({
          ...input,
          leaseDurationMs: 30_000,
        }),
      )
      await Effect.runPromise(
        repository().acceptClaim({
          ownerUserId: input.ownerUserId,
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: claimed.claim.claimRef,
        }),
      )
      return claimed.claim.claimRef
    }

    const settleWithoutHang = async <T>(
      promises: ReadonlyArray<Promise<T>>,
      timeoutMs = 10_000,
    ): Promise<Array<PromiseSettledResult<T>>> => {
      let timeout: ReturnType<typeof setTimeout> | undefined
      try {
        return await Promise.race([
          Promise.allSettled(promises),
          new Promise<never>((_, reject) => {
            timeout = setTimeout(
              () => reject(new Error("concurrent approval writes hung")),
              timeoutMs,
            )
          }),
        ])
      } finally {
        if (timeout !== undefined) clearTimeout(timeout)
      }
    }

    const approvalBatch = (input: {
      runRef: string
      claimRef: string
      suffix: string
      approvalRefs: ReadonlyArray<string>
    }) => {
      let sequence = 1
      const events: Array<FleetRunExecutionEventV2> = [
        pylonExecutionEvent(input.runRef, input.claimRef, sequence++, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:00.000Z",
          kind: "run_started",
        }),
      ]
      for (const [index, approvalRef] of input.approvalRefs.entries()) {
        const unitRef = `unit-${index + 1}`
        const workClaimRef = `work_claim.${input.suffix}.${unitRef}`
        const accountRefHash = `account.pylon.codex.${createHash("sha256")
          .update(input.suffix)
          .digest("hex")
          .slice(0, 24)}`
        events.push(
          pylonExecutionEvent(input.runRef, input.claimRef, sequence++, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
            observedAt: "2026-07-09T22:00:00.000Z",
            kind: "work_progress",
            unitRef,
            workClaimRef,
            assignmentRef: `assignment.${input.suffix}.${unitRef}`,
            workerKind: "codex",
            accountRefHash,
            blockerRefs: [],
          }),
          pylonExecutionEvent(input.runRef, input.claimRef, sequence++, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
            observedAt: "2026-07-09T22:00:00.000Z",
            kind: "approval_requested",
            unitRef,
            workClaimRef,
            workerKind: "codex",
            workerRef: `worker.codex.${input.suffix}.${unitRef}`,
            approvalRef,
            toolClass: "write_file",
            blockerRefs: ["blocker.approval_required"],
          }),
        )
      }
      return {
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef: input.claimRef,
        events,
      } as const
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
      expect(changelog).toHaveLength(2)
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
      expect(changelog[1]!.entity_type).toBe("fleet_work_unit")
      const projectedUnit =
        typeof changelog[1]!.post_image_json === "string"
          ? changelog[1]!.post_image_json
          : JSON.stringify(changelog[1]!.post_image_json)
      expect(projectedUnit).toContain('"workUnitRef":"issue.8637"')
      expect(projectedUnit).toContain('"state":"planned"')

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
      await seedPylon({
        pylonRef: "pylon-claim-a",
        ownerUserId: "user-claim-owner",
      })
      await seedPylon({
        pylonRef: "pylon-claim-b",
        ownerUserId: "user-claim-owner",
      })
      await seedPylon({
        pylonRef: "pylon-foreign",
        ownerUserId: "user-foreign",
      })
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
      const startReplay = await start("user-claim-owner", "claim-exact-run-1")
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

    test("persists gapless execution, exact closeouts, and one owner-scoped Sync post-image transactionally", async () => {
      const ownerUserId = "user-execution-owner"
      const pylonRef = "pylon-execution"
      const run = await start(ownerUserId, "execution-complete-1", {
        workerKind: "auto",
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution",
          units: [
            { unitRef: "unit-a", title: "Unit A", dependsOn: [] },
            { unitRef: "unit-b", title: "Unit B", dependsOn: ["unit-a"] },
          ],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-claim-1",
      })
      const firstBatch = {
        schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
        claimRef,
        events: [
          pylonExecutionEvent(run.record.runRef, claimRef, 1, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
            observedAt: "2026-07-09T22:00:01.000Z",
            kind: "run_started",
          }),
          pylonExecutionEvent(run.record.runRef, claimRef, 2, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
            observedAt: "2026-07-09T22:00:02.000Z",
            kind: "work_progress",
            unitRef: "unit-a",
            workClaimRef: "work_claim.unit-a",
            assignmentRef: "assignment.unit-a",
            workerKind: "codex",
            accountRefHash: `account.pylon.codex.${"a".repeat(24)}`,
            marginalCostClass: "subscription",
            blockerRefs: [],
          }),
          pylonExecutionEvent(run.record.runRef, claimRef, 3, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
            observedAt: "2026-07-09T22:00:03.000Z",
            kind: "work_terminal",
            unitRef: "unit-a",
            workClaimRef: "work_claim.unit-a",
            assignmentRef: "assignment.unit-a",
            workerKind: "codex",
            accountRefHash: `account.pylon.codex.${"a".repeat(24)}`,
            marginalCostClass: "subscription",
            terminalState: "accepted",
            closeoutRef: "closeout.unit-a",
            verification: {
              truth: "passed",
              verifierRef: "verifier.bun-test.1",
              evidenceRefs: ["test.unit-a.1"],
            },
            artifactRefs: ["artifact.patch.unit-a.1"],
            proofRefs: ["proof.unit-a.1"],
            authorityReceiptRefs: ["receipt.authority.unit-a.1"],
            usageEvidence: exactUsageEvidence(
              "assignment.unit-a",
              pylonRef,
              "unit-a.1",
            ),
            blockerRefs: [],
          }),
        ],
      } as const
      const first = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: firstBatch,
        }),
      )
      expect(first.ack.storedEventCount).toBe(3)
      expect(first.ack.duplicateEventCount).toBe(0)
      expect(first.ack.execution).toMatchObject({
        state: "running",
        lastSequence: 3,
        counters: {
          workUnitsTotal: 2,
          activeAssignments: 0,
          acceptedAssignments: 1,
          failedAssignments: 0,
          staleAssignments: 0,
        },
        startedAt: "2026-07-09T22:00:00.000Z",
      })

      const replay = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: firstBatch,
        }),
      )
      expect(replay.ack.storedEventCount).toBe(0)
      expect(replay.ack.duplicateEventCount).toBe(3)

      const completed = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: [
              pylonExecutionEvent(run.record.runRef, claimRef, 4, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:04.000Z",
                kind: "work_terminal",
                unitRef: "unit-b",
                workClaimRef: "work_claim.unit-b",
                assignmentRef: "assignment.unit-b",
                workerKind: "grok",
                accountRefHash: `account.pylon.grok.${"b".repeat(24)}`,
                marginalCostClass: "free",
                terminalState: "accepted",
                closeoutRef: "closeout.unit-b",
                verification: {
                  truth: "passed",
                  verifierRef: "verifier.bun-test.1",
                  evidenceRefs: ["test.unit-b.1"],
                },
                artifactRefs: ["artifact.patch.unit-b.1"],
                proofRefs: ["proof.unit-b.1"],
                authorityReceiptRefs: ["receipt.authority.unit-b.1"],
                usageEvidence: notMeasuredUsageEvidence(
                  "assignment.unit-b",
                  "unit-b.1",
                ),
                blockerRefs: [],
              }),
              pylonExecutionEvent(run.record.runRef, claimRef, 5, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:05.000Z",
                kind: "run_terminal",
                terminalState: "completed",
                blockerRefs: [],
              }),
            ],
          },
        }),
      )
      expect(completed.ack.execution).toMatchObject({
        state: "completed",
        lastSequence: 5,
        counters: {
          workUnitsTotal: 2,
          activeAssignments: 0,
          acceptedAssignments: 2,
          failedAssignments: 0,
          staleAssignments: 0,
        },
      })
      expect(completed.ack.execution.closeouts).toEqual([
        expect.objectContaining({
          unitRef: "unit-a",
          workerKind: "codex",
          usageEvidence: {
            truth: "exact",
            tokenUsageRefs: ["usage_row.unit-a.1"],
          },
        }),
        expect.objectContaining({
          unitRef: "unit-b",
          workerKind: "grok",
          usageEvidence: { truth: "not_measured", tokenUsageRefs: [] },
        }),
      ])

      const eventRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${run.record.runRef}
      `
      expect(Number(eventRows[0]!.count)).toBe(5)
      const changelog: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
        ORDER BY version
      `
      expect(changelog).toHaveLength(11)
      const projected =
        typeof changelog.at(-1)!.post_image_json === "string"
          ? (changelog.at(-1)!.post_image_json as string)
          : JSON.stringify(changelog.at(-1)!.post_image_json)
      expect(projected).toContain('"status":"completed"')
      expect(projected).toContain('"completedAssignments":2')
      expect(projected).not.toContain(ownerUserId)
      expect(projected).not.toContain("work_claim.unit-a")
      const attemptRows: Array<{
        attempt_ref: string
        state: string
        assignment_ref: string | null
        verification_json: string
        token_usage_refs_json: string
      }> = await sql`
        SELECT attempt_ref, state, assignment_ref, verification_json,
               token_usage_refs_json
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
        ORDER BY attempt_ref
      `
      expect(attemptRows).toEqual([
        {
          attempt_ref: "work_claim.unit-a",
          state: "succeeded",
          assignment_ref: "assignment.unit-a",
          verification_json:
            '{"evidenceRefs":["test.unit-a.1"],"truth":"passed","verifierRef":"verifier.bun-test.1"}',
          token_usage_refs_json: '["usage_row.unit-a.1"]',
        },
        {
          attempt_ref: "work_claim.unit-b",
          state: "succeeded",
          assignment_ref: "assignment.unit-b",
          verification_json:
            '{"evidenceRefs":["test.unit-b.1"],"truth":"passed","verifierRef":"verifier.bun-test.1"}',
          token_usage_refs_json: "[]",
        },
      ])
      const workUnitRows: Array<{
        unit_ref: string
        state: string
        latest_attempt_ref: string | null
        accepted_attempt_ref: string | null
      }> = await sql`
        SELECT unit_ref, state, latest_attempt_ref, accepted_attempt_ref
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${run.record.runRef}
        ORDER BY unit_ref
      `
      expect(workUnitRows).toEqual([
        {
          unit_ref: "unit-a",
          state: "succeeded",
          latest_attempt_ref: "work_claim.unit-a",
          accepted_attempt_ref: "work_claim.unit-a",
        },
        {
          unit_ref: "unit-b",
          state: "succeeded",
          latest_attempt_ref: "work_claim.unit-b",
          accepted_attempt_ref: "work_claim.unit-b",
        },
      ])

      const observed = await Effect.runPromise(
        repository().observe({ ownerUserId, runRef: run.record.runRef }),
      )
      expect(observed.record.execution).toEqual(completed.ack.execution)
      const publicRecord = publicFleetRunAuthorityRecord(observed.record)
      expect(publicRecord.execution.state).toBe("completed")
      expect(publicRecord).not.toHaveProperty("ownerUserId")
    })

    test("accepts a retained v1 start followed directly by v2 attempt and run terminals", async () => {
      const ownerUserId = "user-execution-v1-v2-owner"
      const pylonRef = "pylon-execution-v1-v2"
      const run = await start(ownerUserId, "execution-v1-v2-upgrade-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.v1-v2",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-v1-v2-claim-1",
      })
      const started = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
            claimRef,
            events: [
              pylonExecutionEvent(run.record.runRef, claimRef, 1, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
                observedAt: "2026-07-09T22:00:01.000Z",
                kind: "run_started",
              }),
            ],
          },
        }),
      )
      expect(started.ack).toMatchObject({
        acceptedThroughSequence: 1,
        execution: { state: "running" },
      })

      const assignmentRef = "assignment.unit-a.v1-v2"
      const upgraded = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: [
              pylonExecutionEvent(run.record.runRef, claimRef, 2, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:02.000Z",
                kind: "work_progress",
                unitRef: "unit-a",
                workClaimRef: "work_claim.unit-a.v1-v2",
                assignmentRef,
                workerKind: "claude",
                accountRefHash: `account.pylon.claude_agent.${"a".repeat(24)}`,
                marginalCostClass: "subscription",
                blockerRefs: [],
              }),
              pylonExecutionEvent(run.record.runRef, claimRef, 3, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:03.000Z",
                kind: "work_terminal",
                unitRef: "unit-a",
                workClaimRef: "work_claim.unit-a.v1-v2",
                assignmentRef,
                workerKind: "claude",
                accountRefHash: `account.pylon.claude_agent.${"a".repeat(24)}`,
                marginalCostClass: "subscription",
                terminalState: "accepted",
                closeoutRef: "closeout.unit-a.v1-v2",
                verification: {
                  truth: "passed",
                  verifierRef: "verifier.bun-test.v1-v2",
                  evidenceRefs: ["test.unit-a.v1-v2"],
                },
                artifactRefs: ["artifact.patch.unit-a.v1-v2"],
                proofRefs: ["proof.unit-a.v1-v2"],
                authorityReceiptRefs: ["receipt.authority.unit-a.v1-v2"],
                usageEvidence: {
                  ...exactUsageEvidence(
                    assignmentRef,
                    pylonRef,
                    "unit-a.v1-v2",
                  ),
                  harnessKind: "claude",
                  provider: "pylon-claude-own-capacity",
                  model: "openagents/pylon-claude",
                  cacheReadTokens: 579_960,
                },
                blockerRefs: [],
              }),
              pylonExecutionEvent(run.record.runRef, claimRef, 4, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:04.000Z",
                kind: "run_terminal",
                terminalState: "completed",
                blockerRefs: [],
              }),
            ],
          },
        }),
      )
      expect(upgraded.ack).toMatchObject({
        acceptedThroughSequence: 4,
        storedEventCount: 3,
        execution: {
          state: "completed",
          counters: { acceptedAssignments: 1 },
        },
      })
      const eventRows: Array<{ event_kind: string }> = await sql`
        SELECT event_kind FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${run.record.runRef}
        ORDER BY sequence
      `
      expect(eventRows.map(row => row.event_kind)).toEqual([
        "run_started",
        "work_progress",
        "work_terminal",
        "run_terminal",
      ])
    })

    test("v2 persists a proven attempt under its work-claim identity", async () => {
      const ownerUserId = "user-execution-v2-owner"
      const pylonRef = "pylon-execution-v2"
      const run = await start(ownerUserId, "execution-v2-complete-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.v2",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-v2-claim-1",
      })
      const events = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:01.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:02.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a.attempt-1",
          assignmentRef: "assignment.unit-a.edge-99",
          workerKind: "codex",
          accountRefHash: `account.pylon.codex.${"e".repeat(24)}`,
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:03.000Z",
          kind: "work_terminal",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a.attempt-1",
          assignmentRef: "assignment.unit-a.edge-99",
          workerKind: "codex",
          accountRefHash: `account.pylon.codex.${"e".repeat(24)}`,
          terminalState: "accepted",
          closeoutRef: "closeout.unit-a.attempt-1",
          verification: {
            truth: "passed",
            verifierRef: "verifier.bun-test.1",
            evidenceRefs: ["test.unit-a.1"],
          },
          artifactRefs: ["artifact.patch.unit-a.1"],
          proofRefs: ["proof.unit-a.1"],
          authorityReceiptRefs: ["receipt.authority.unit-a.1"],
          marginalCostClass: "subscription",
          usageEvidence: exactUsageEvidence(
            "assignment.unit-a.edge-99",
            pylonRef,
            "unit-a.1",
          ),
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 4, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:04.000Z",
          kind: "run_terminal",
          terminalState: "completed",
          blockerRefs: [],
        }),
      ]
      const result = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events,
          },
        }),
      )
      expect(result.ack.execution.state).toBe("completed")

      const attempts: Array<{
        attempt_ref: string
        work_unit_ref: string
        owner_user_id: string
        intake_claim_ref: string
        pylon_ref: string
        state: string
        assignment_ref: string | null
        verification_json: string
        artifact_refs_json: string
        proof_refs_json: string
        authority_receipt_refs_json: string
        closeout_ref: string | null
        marginal_cost_class: string
        usage_json: string
        usage_truth: string
        usage_provider: string | null
        usage_model: string | null
        usage_total_tokens: string | number | null
        usage_token_rows: string | number | null
        token_usage_refs_json: string
        started_at: string
        last_observed_at: string
        remote_observed_at: string
        updated_at: string
      }> = await sql`
        SELECT attempt_ref, work_unit_ref, owner_user_id, intake_claim_ref,
               pylon_ref, state, assignment_ref, verification_json,
               artifact_refs_json, proof_refs_json,
               authority_receipt_refs_json, closeout_ref,
               marginal_cost_class, usage_json, usage_truth, usage_provider,
               usage_model, usage_total_tokens, usage_token_rows,
               token_usage_refs_json, started_at, last_observed_at,
               remote_observed_at, updated_at
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
      `
      expect(attempts).toEqual([
        {
          attempt_ref: "work_claim.unit-a.attempt-1",
          work_unit_ref: "unit-a",
          owner_user_id: ownerUserId,
          intake_claim_ref: claimRef,
          pylon_ref: pylonRef,
          state: "succeeded",
          assignment_ref: "assignment.unit-a.edge-99",
          verification_json:
            '{"evidenceRefs":["test.unit-a.1"],"truth":"passed","verifierRef":"verifier.bun-test.1"}',
          artifact_refs_json: '["artifact.patch.unit-a.1"]',
          proof_refs_json: '["proof.unit-a.1"]',
          authority_receipt_refs_json: '["receipt.authority.unit-a.1"]',
          closeout_ref: "closeout.unit-a.attempt-1",
          marginal_cost_class: "subscription",
          usage_json: canonicalJson(
            exactUsageEvidence(
              "assignment.unit-a.edge-99",
              pylonRef,
              "unit-a.1",
            ),
          ),
          usage_truth: "exact",
          usage_provider: "pylon-codex-own-capacity",
          usage_model: "openagents/pylon-codex",
          usage_total_tokens: "13",
          usage_token_rows: "1",
          token_usage_refs_json: '["usage_row.unit-a.1"]',
          started_at: new Date(FIXED_NOW).toISOString(),
          last_observed_at: new Date(FIXED_NOW).toISOString(),
          remote_observed_at: "2026-07-09T22:00:03.000Z",
          updated_at: new Date(FIXED_NOW).toISOString(),
        },
      ])
      const units: Array<{
        state: string
        latest_attempt_ref: string | null
        accepted_attempt_ref: string | null
      }> = await sql`
        SELECT state, latest_attempt_ref, accepted_attempt_ref
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `
      expect(units).toEqual([
        {
          state: "succeeded",
          latest_attempt_ref: "work_claim.unit-a.attempt-1",
          accepted_attempt_ref: "work_claim.unit-a.attempt-1",
        },
      ])
      const invalidReasoningSubset = await sql`
        UPDATE sarah_fleet_run_attempts
        SET usage_reasoning_tokens = usage_output_tokens + 1
        WHERE run_ref = ${run.record.runRef}
          AND attempt_ref = 'work_claim.unit-a.attempt-1'
      `.then(
        () => null,
        (error) => error,
      )
      expect(String(invalidReasoningSubset)).toContain(
        "sarah_fleet_run_attempts_usage_columns_coherence",
      )
      const postImages: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
        ORDER BY version
      `
      const serialized = postImages
        .map((row) =>
          typeof row.post_image_json === "string"
            ? row.post_image_json
            : JSON.stringify(row.post_image_json),
        )
        .join("\n")
      expect(serialized).toContain('"attemptRef":"work_claim.unit-a.attempt-1"')
      expect(serialized).toContain(
        '"assignmentRef":"assignment.unit-a.edge-99"',
      )
      expect(serialized).toContain(
        '"acceptedAttemptRef":"work_claim.unit-a.attempt-1"',
      )
      expect(serialized).toContain('"totalTokens":13')
      expect(serialized).toContain('"tokenUsageRefs":["usage_row.unit-a.1"]')
      expect(serialized).not.toContain(ownerUserId)
    })

    test("projects an exact approval against the active attempt using server receipt time", async () => {
      const ownerUserId = "user-execution-approval-owner"
      const pylonRef = "pylon-execution-approval"
      const run = await start(ownerUserId, "execution-approval-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.approval",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-approval-claim-1",
      })
      const assignmentRef = "assignment.unit-a.approval-edge"
      const accountRefHash = `account.pylon.codex.${"f".repeat(24)}`
      const approvalRef = "approval.unit-a.write-file-1"
      const events = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:01.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T21:59:59.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a.approval-attempt-1",
          assignmentRef,
          workerKind: "codex",
          accountRefHash,
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T21:59:58.000Z",
          kind: "approval_requested",
          unitRef: "unit-a",
          workClaimRef: "work_claim.unit-a.approval-attempt-1",
          workerKind: "codex",
          workerRef: "worker.codex.approval-slot-1",
          approvalRef,
          toolClass: "write_file",
          blockerRefs: ["blocker.approval_required"],
        }),
      ]
      const first = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events,
          },
        }),
      )
      expect(first.ack).toMatchObject({
        acceptedThroughSequence: 3,
        storedEventCount: 3,
        execution: { counters: { activeAssignments: 1 } },
      })

      const images: Array<{ post_image_json: string | object }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type = 'fleet_approval'
          AND entity_id = ${approvalRef}
        ORDER BY version DESC
      `
      const raw = images[0]!.post_image_json
      const approval = decodeFleetApprovalEntity(
        typeof raw === "string" ? JSON.parse(raw) : raw,
      )
      expect(approval).toMatchObject({
        approvalRef,
        status: "pending",
        runRef: run.record.runRef,
        workUnitRef: "unit-a",
        attemptRef: "work_claim.unit-a.approval-attempt-1",
        assignmentRef,
        accountRefHash,
        workerId: "worker.codex.approval-slot-1",
        requestEventRef: events[2]!.eventRef,
        toolClass: "write_file",
        openedAt: new Date(FIXED_NOW).toISOString(),
        updatedAt: new Date(FIXED_NOW).toISOString(),
      })
      const exactGraphImages: Array<{
        entity_type: string
        entity_id: string
        post_image_json: string | object
      }> = await sql`
        SELECT DISTINCT ON (entity_type, entity_id)
               entity_type, entity_id, post_image_json
        FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type IN ('fleet_worker', 'fleet_assignment')
        ORDER BY entity_type, entity_id, version DESC
      `
      const workerRaw = exactGraphImages.find(
        (row) => row.entity_type === "fleet_worker",
      )?.post_image_json
      const assignmentRaw = exactGraphImages.find(
        (row) => row.entity_type === "fleet_assignment",
      )?.post_image_json
      expect(
        decodeFleetWorkerEntity(
          typeof workerRaw === "string" ? JSON.parse(workerRaw) : workerRaw,
        ),
      ).toEqual(
        decodeFleetWorkerEntity({
          workerId: "worker.codex.approval-slot-1",
          phase: "blocked",
          harnessKind: "codex",
          assignmentRef,
          accountRefHash,
          lastProgressAt: new Date(FIXED_NOW).toISOString(),
          updatedAt: new Date(FIXED_NOW).toISOString(),
        }),
      )
      expect(
        decodeFleetAssignmentEntity(
          typeof assignmentRaw === "string"
            ? JSON.parse(assignmentRaw)
            : assignmentRaw,
        ),
      ).toEqual(
        decodeFleetAssignmentEntity({
          assignmentRef,
          status: "running",
          updatedAt: new Date(FIXED_NOW).toISOString(),
        }),
      )
      const blockedAttempts: Array<{
        state: string
        progress_class: string
        last_event_ref: string
        last_observed_at: string
        remote_observed_at: string
        terminal_at: string | null
      }> = await sql`
        SELECT state, progress_class, last_event_ref, last_observed_at,
               remote_observed_at, terminal_at
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
          AND attempt_ref = 'work_claim.unit-a.approval-attempt-1'
      `
      expect(blockedAttempts).toEqual([
        {
          state: "running",
          progress_class: "blocked",
          last_event_ref: events[2]!.eventRef,
          last_observed_at: new Date(FIXED_NOW).toISOString(),
          remote_observed_at: "2026-07-09T21:59:58.000Z",
          terminal_at: null,
        },
      ])

      const replay = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events,
          },
        }),
      )
      expect(replay.ack).toMatchObject({
        acceptedThroughSequence: 3,
        storedEventCount: 0,
        duplicateEventCount: 3,
      })
      const graphVersionsAfterReplay: Array<{
        entity_type: string
        count: string | number
      }> = await sql`
        SELECT entity_type, count(*) AS count
        FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type IN ('fleet_worker', 'fleet_assignment')
        GROUP BY entity_type
        ORDER BY entity_type
      `
      expect(graphVersionsAfterReplay.map((row) => ({
        entity_type: row.entity_type,
        count: Number(row.count),
      }))).toEqual([
        { entity_type: "fleet_assignment", count: 1 },
        { entity_type: "fleet_worker", count: 1 },
      ])

      const conflict = await Effect.runPromise(
        repository()
          .appendExecutionEvents({
            ownerUserId,
            pylonRef,
            runRef: run.record.runRef,
            batch: {
              schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
              claimRef,
              events: [
                pylonExecutionEvent(run.record.runRef, claimRef, 4, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:04.000Z",
                  kind: "work_progress",
                  unitRef: "unit-a",
                  workClaimRef: "work_claim.unit-a.approval-attempt-2",
                  workerKind: "codex",
                  blockerRefs: [],
                }),
                pylonExecutionEvent(run.record.runRef, claimRef, 5, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:05.000Z",
                  kind: "approval_requested",
                  unitRef: "unit-a",
                  workClaimRef: "work_claim.unit-a.approval-attempt-2",
                  workerKind: "codex",
                  workerRef: "worker.codex.approval-slot-2",
                  approvalRef,
                  toolClass: "bash",
                  blockerRefs: ["blocker.approval_required"],
                }),
              ],
            },
          })
          .pipe(Effect.flip),
      )
      expect(conflict.kind).toBe("idempotency_conflict")
      const rolledBack: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${run.record.runRef}
      `
      expect(Number(rolledBack[0]!.count)).toBe(3)
    })

    test("keeps worker and assignment identity exact through terminal transitions", async () => {
      const ownerUserId = "user-execution-worker-owner"
      const pylonRef = "pylon-execution-worker"
      const run = await start(ownerUserId, "execution-worker-graph-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.worker",
          units: [
            { unitRef: "unit-a", title: "Unit A", dependsOn: [] },
            { unitRef: "unit-b", title: "Unit B", dependsOn: [] },
          ],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-worker-claim-1",
      })
      const workerA = "worker.codex.execution-slot-a"
      const assignmentA = "assignment.execution.unit-a"
      const accountA = `account.pylon.codex.${"a".repeat(24)}`
      const initialEvents = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:01.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:02.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef: "work_claim.execution.unit-a",
          assignmentRef: assignmentA,
          workerKind: "codex",
          accountRefHash: accountA,
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:03.000Z",
          kind: "approval_requested",
          unitRef: "unit-a",
          workClaimRef: "work_claim.execution.unit-a",
          workerKind: "codex",
          workerRef: workerA,
          approvalRef: "approval.execution.unit-a",
          toolClass: "write_file",
          blockerRefs: ["blocker.approval_required"],
        }),
      ]
      await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: initialEvents,
          },
        }),
      )

      const mismatchedWorkerReuse = await Effect.runPromise(
        repository()
          .appendExecutionEvents({
            ownerUserId,
            pylonRef,
            runRef: run.record.runRef,
            batch: {
              schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
              claimRef,
              events: [
                pylonExecutionEvent(run.record.runRef, claimRef, 4, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:04.000Z",
                  kind: "work_progress",
                  unitRef: "unit-b",
                  workClaimRef: "work_claim.execution.unit-b",
                  assignmentRef: "assignment.execution.unit-b",
                  workerKind: "codex",
                  accountRefHash: `account.pylon.codex.${"b".repeat(24)}`,
                  blockerRefs: [],
                }),
                pylonExecutionEvent(run.record.runRef, claimRef, 5, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:05.000Z",
                  kind: "approval_requested",
                  unitRef: "unit-b",
                  workClaimRef: "work_claim.execution.unit-b",
                  workerKind: "codex",
                  workerRef: workerA,
                  approvalRef: "approval.execution.unit-b.wrong-worker",
                  toolClass: "write_file",
                  blockerRefs: ["blocker.approval_required"],
                }),
              ],
            },
          })
          .pipe(Effect.flip),
      )
      expect(mismatchedWorkerReuse.kind).toBe("idempotency_conflict")
      const afterMismatch: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count
        FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${run.record.runRef}
      `
      expect(Number(afterMismatch[0]!.count)).toBe(3)

      const workerB = "worker.codex.execution-slot-b"
      const assignmentB = "assignment.execution.unit-b"
      const accountB = `account.pylon.codex.${"b".repeat(24)}`
      const terminalEvents = [
        pylonExecutionEvent(run.record.runRef, claimRef, 4, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:04.000Z",
          kind: "work_terminal",
          unitRef: "unit-a",
          workClaimRef: "work_claim.execution.unit-a",
          assignmentRef: assignmentA,
          workerKind: "codex",
          accountRefHash: accountA,
          terminalState: "accepted",
          closeoutRef: "closeout.execution.unit-a",
          verification: {
            truth: "passed",
            verifierRef: "verifier.execution.unit-a",
            evidenceRefs: ["verification.execution.unit-a"],
          },
          artifactRefs: ["artifact.execution.unit-a"],
          proofRefs: ["proof.execution.unit-a"],
          authorityReceiptRefs: ["receipt.execution.unit-a"],
          usageEvidence: exactUsageEvidence(
            assignmentA,
            pylonRef,
            "execution.unit-a",
          ),
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 5, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:05.000Z",
          kind: "work_progress",
          unitRef: "unit-b",
          workClaimRef: "work_claim.execution.unit-b",
          assignmentRef: assignmentB,
          workerKind: "codex",
          accountRefHash: accountB,
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 6, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:06.000Z",
          kind: "approval_requested",
          unitRef: "unit-b",
          workClaimRef: "work_claim.execution.unit-b",
          workerKind: "codex",
          workerRef: workerB,
          approvalRef: "approval.execution.unit-b",
          toolClass: "write_file",
          blockerRefs: ["blocker.approval_required"],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 7, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
          observedAt: "2026-07-09T22:00:07.000Z",
          kind: "work_terminal",
          unitRef: "unit-b",
          workClaimRef: "work_claim.execution.unit-b",
          assignmentRef: assignmentB,
          workerKind: "codex",
          accountRefHash: accountB,
          terminalState: "failed",
          blockerRefs: ["blocker.verification_failed"],
        }),
      ]
      const terminal = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: terminalEvents,
          },
        }),
      )
      expect(terminal.ack).toMatchObject({
        acceptedThroughSequence: 7,
        storedEventCount: 4,
      })

      const currentGraph: Array<{
        entity_type: string
        entity_id: string
        post_image_json: string | object
      }> = await sql`
        SELECT DISTINCT ON (entity_type, entity_id)
               entity_type, entity_id, post_image_json
        FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type IN ('fleet_worker', 'fleet_assignment')
        ORDER BY entity_type, entity_id, version DESC
      `
      const decodedWorkers = currentGraph
        .filter((row) => row.entity_type === "fleet_worker")
        .map((row) =>
          decodeFleetWorkerEntity(
            typeof row.post_image_json === "string"
              ? JSON.parse(row.post_image_json)
              : row.post_image_json,
          ),
        )
      expect(decodedWorkers).toEqual([
        expect.objectContaining({
          workerId: workerA,
          phase: "completed",
          harnessKind: "codex",
          assignmentRef: assignmentA,
          accountRefHash: accountA,
        }),
        expect.objectContaining({
          workerId: workerB,
          phase: "failed",
          harnessKind: "codex",
          assignmentRef: assignmentB,
          accountRefHash: accountB,
        }),
      ])
      const decodedAssignments = currentGraph
        .filter((row) => row.entity_type === "fleet_assignment")
        .map((row) =>
          decodeFleetAssignmentEntity(
            typeof row.post_image_json === "string"
              ? JSON.parse(row.post_image_json)
              : row.post_image_json,
          ),
        )
      expect(decodedAssignments).toEqual([
        expect.objectContaining({
          assignmentRef: assignmentA,
          status: "accepted_work",
          closeoutClass: "accepted_work",
        }),
        expect.objectContaining({
          assignmentRef: assignmentB,
          status: "failed",
          closeoutClass: "failed",
        }),
      ])

      const versionsBeforeReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type IN ('fleet_worker', 'fleet_assignment')
      `
      const replay = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: terminalEvents,
          },
        }),
      )
      expect(replay.ack).toMatchObject({
        acceptedThroughSequence: 7,
        storedEventCount: 0,
        duplicateEventCount: 4,
      })
      const versionsAfterReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND entity_type IN ('fleet_worker', 'fleet_assignment')
      `
      expect(Number(versionsAfterReplay[0]!.count)).toBe(
        Number(versionsBeforeReplay[0]!.count),
      )
    })

    test("indexes global approval identity lookup independently of changelog size", async () => {
      const scope = "scope.fleet_run.approval-index-proof"
      await sql`
        INSERT INTO khala_sync_scopes (scope, last_version)
        VALUES (${scope}, 100001)
      `
      await sql`
        INSERT INTO khala_sync_changelog
          (scope, version, entity_type, entity_id, op, post_image_json,
           mutation_ref, committed_at)
        SELECT ${scope}, n, 'approval_index_noise', 'noise.' || n::text,
               'upsert', '{}'::jsonb, 'test:approval-index-proof',
               '2026-07-09T21:00:00.000Z'::timestamptz
                 + (n * interval '1 microsecond')
        FROM generate_series(1, 100000) AS n
      `
      await sql`
        INSERT INTO khala_sync_changelog
          (scope, version, entity_type, entity_id, op, post_image_json,
           mutation_ref, committed_at)
        VALUES
          (${scope}, 100001, 'fleet_approval', 'approval.index.probe',
           'upsert', '{}', 'test:approval-index-proof',
           '2026-07-09T22:00:00.000Z')
      `
      await sql`ANALYZE khala_sync_changelog`

      const plans: Array<{ "QUERY PLAN": unknown }> = await sql`
        EXPLAIN (FORMAT JSON)
        SELECT scope, post_image_json
        FROM khala_sync_changelog
        WHERE entity_type = 'fleet_approval'
          AND entity_id = 'approval.index.probe'
          AND op = 'upsert'
        ORDER BY committed_at DESC, version DESC
        LIMIT 1
        FOR UPDATE
      `
      expect(JSON.stringify(plans)).toContain(
        "khala_sync_changelog_fleet_approval_latest_idx",
      )
    })

    test("serializes concurrent cross-run claims for one approval ref", async () => {
      const approvalRef = "approval.concurrent.same-ref"
      const runs = await Promise.all([
        start("user-approval-race-a", "approval-race-a", {
          workSource: {
            kind: "plan_dag",
            planRef: "plan.approval.race.a",
            units: [{ unitRef: "unit-1", title: "Unit 1", dependsOn: [] }],
          },
        }),
        start("user-approval-race-b", "approval-race-b", {
          workSource: {
            kind: "plan_dag",
            planRef: "plan.approval.race.b",
            units: [{ unitRef: "unit-1", title: "Unit 1", dependsOn: [] }],
          },
        }),
      ])
      await Promise.all([
        seedPylon({
          ownerUserId: "user-approval-race-a",
          pylonRef: "pylon-approval-race-a",
        }),
        seedPylon({
          ownerUserId: "user-approval-race-b",
          pylonRef: "pylon-approval-race-b",
        }),
      ])
      const claims = await Promise.all([
        claimAndAccept({
          ownerUserId: "user-approval-race-a",
          pylonRef: "pylon-approval-race-a",
          runRef: runs[0]!.record.runRef,
          claimIdempotencyKey: "approval-race-claim-a",
        }),
        claimAndAccept({
          ownerUserId: "user-approval-race-b",
          pylonRef: "pylon-approval-race-b",
          runRef: runs[1]!.record.runRef,
          claimIdempotencyKey: "approval-race-claim-b",
        }),
      ])

      const results = await settleWithoutHang([
        Effect.runPromise(
          repository().appendExecutionEvents({
            ownerUserId: "user-approval-race-a",
            pylonRef: "pylon-approval-race-a",
            runRef: runs[0]!.record.runRef,
            batch: approvalBatch({
              runRef: runs[0]!.record.runRef,
              claimRef: claims[0]!,
              suffix: "race-a",
              approvalRefs: [approvalRef],
            }),
          }),
        ),
        Effect.runPromise(
          repository().appendExecutionEvents({
            ownerUserId: "user-approval-race-b",
            pylonRef: "pylon-approval-race-b",
            runRef: runs[1]!.record.runRef,
            batch: approvalBatch({
              runRef: runs[1]!.record.runRef,
              claimRef: claims[1]!,
              suffix: "race-b",
              approvalRefs: [approvalRef],
            }),
          }),
        ),
      ])
      expect(
        results.filter((result) => result.status === "fulfilled"),
      ).toHaveLength(1)
      const rejected = results.filter(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      )
      expect(rejected).toHaveLength(1)
      expect(rejected[0]!.reason).toMatchObject({
        kind: "idempotency_conflict",
        reason: "fleet approval ref is already bound to another exact attempt",
      })
      const approvals: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE entity_type = 'fleet_approval' AND entity_id = ${approvalRef}
      `
      expect(Number(approvals[0]!.count)).toBe(1)
    })

    test("prelocks reverse-order multi-approval batches without deadlock", async () => {
      const approvalRefs = [
        "approval.concurrent.multi-a",
        "approval.concurrent.multi-b",
      ] as const
      const planUnits = [
        { unitRef: "unit-1", title: "Unit 1", dependsOn: [] },
        { unitRef: "unit-2", title: "Unit 2", dependsOn: [] },
      ]
      const runs = await Promise.all([
        start("user-approval-order-a", "approval-order-a", {
          workSource: {
            kind: "plan_dag",
            planRef: "plan.approval.order.a",
            units: planUnits,
          },
        }),
        start("user-approval-order-b", "approval-order-b", {
          workSource: {
            kind: "plan_dag",
            planRef: "plan.approval.order.b",
            units: planUnits,
          },
        }),
      ])
      await Promise.all([
        seedPylon({
          ownerUserId: "user-approval-order-a",
          pylonRef: "pylon-approval-order-a",
        }),
        seedPylon({
          ownerUserId: "user-approval-order-b",
          pylonRef: "pylon-approval-order-b",
        }),
      ])
      const claims = await Promise.all([
        claimAndAccept({
          ownerUserId: "user-approval-order-a",
          pylonRef: "pylon-approval-order-a",
          runRef: runs[0]!.record.runRef,
          claimIdempotencyKey: "approval-order-claim-a",
        }),
        claimAndAccept({
          ownerUserId: "user-approval-order-b",
          pylonRef: "pylon-approval-order-b",
          runRef: runs[1]!.record.runRef,
          claimIdempotencyKey: "approval-order-claim-b",
        }),
      ])

      const results = await settleWithoutHang([
        Effect.runPromise(
          repository().appendExecutionEvents({
            ownerUserId: "user-approval-order-a",
            pylonRef: "pylon-approval-order-a",
            runRef: runs[0]!.record.runRef,
            batch: approvalBatch({
              runRef: runs[0]!.record.runRef,
              claimRef: claims[0]!,
              suffix: "order-a",
              approvalRefs,
            }),
          }),
        ),
        Effect.runPromise(
          repository().appendExecutionEvents({
            ownerUserId: "user-approval-order-b",
            pylonRef: "pylon-approval-order-b",
            runRef: runs[1]!.record.runRef,
            batch: approvalBatch({
              runRef: runs[1]!.record.runRef,
              claimRef: claims[1]!,
              suffix: "order-b",
              approvalRefs: [...approvalRefs].reverse(),
            }),
          }),
        ),
      ])
      expect(
        results.filter((result) => result.status === "fulfilled"),
      ).toHaveLength(1)
      const rejected = results.filter(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      )
      expect(rejected).toHaveLength(1)
      expect(rejected[0]!.reason).toMatchObject({
        kind: "idempotency_conflict",
        reason: "fleet approval ref is already bound to another exact attempt",
      })
      const approvals: Array<{ entity_id: string; count: string | number }> =
        await sql`
          SELECT entity_id, count(*) AS count FROM khala_sync_changelog
          WHERE entity_type = 'fleet_approval'
            AND entity_id IN (${approvalRefs[0]}, ${approvalRefs[1]})
          GROUP BY entity_id ORDER BY entity_id
        `
      expect(approvals.map((row) => [row.entity_id, Number(row.count)])).toEqual([
        [approvalRefs[0], 1],
        [approvalRefs[1], 1],
      ])
    })

    test("accepts reordered remote clocks while server receipt time remains authoritative", async () => {
      const ownerUserId = "user-execution-clock-owner"
      const pylonRef = "pylon-execution-clock"
      const run = await start(ownerUserId, "execution-clock-order-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.clock-order",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-clock-order-claim",
      })

      const result = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
            claimRef,
            events: [
              pylonExecutionEvent(run.record.runRef, claimRef, 1, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
                observedAt: "2026-07-09T22:00:02.000Z",
                kind: "run_started",
              }),
              pylonExecutionEvent(run.record.runRef, claimRef, 2, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
                observedAt: "2026-07-09T21:59:59.000Z",
                kind: "work_progress",
                unitRef: "unit-a",
                workClaimRef: "work_claim.clock-order.unit-a.attempt-1",
                workerKind: "codex",
                blockerRefs: ["blocker.remote_clock"],
              }),
            ],
          },
        }),
      )
      expect(result.ack).toMatchObject({
        acceptedThroughSequence: 2,
        storedEventCount: 2,
        execution: {
          state: "running",
          counters: { activeAssignments: 1 },
          startedAt: "2026-07-09T22:00:00.000Z",
          updatedAt: "2026-07-09T22:00:00.000Z",
        },
      })

      const attempts: Array<{
        remote_observed_at: string
        last_observed_at: string
        updated_at: string
        blocker_refs_json: string
      }> = await sql`
        SELECT remote_observed_at, last_observed_at, updated_at,
               blocker_refs_json
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
      `
      expect(attempts).toEqual([
        {
          remote_observed_at: "2026-07-09T21:59:59.000Z",
          last_observed_at: "2026-07-09T22:00:00.000Z",
          updated_at: "2026-07-09T22:00:00.000Z",
          blocker_refs_json: '["blocker.remote_clock"]',
        },
      ])
    })

    test("rejects gaps, foreign units, conflicting replay, and incomplete completion without partial writes", async () => {
      const ownerUserId = "user-execution-guard"
      const pylonRef = "pylon-execution-guard"
      const run = await start(ownerUserId, "execution-guards-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.execution.guards",
          units: [
            { unitRef: "unit-a", title: "Unit A", dependsOn: [] },
            { unitRef: "unit-b", title: "Unit B", dependsOn: [] },
          ],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "execution-guards-claim",
      })
      const append = (events: ReadonlyArray<Record<string, unknown>>) =>
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
            claimRef,
            events,
          },
        })
      const startedInput = {
        schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
        observedAt: "2026-07-09T22:00:01.000Z",
        kind: "run_started",
      } as const
      const started = pylonExecutionEvent(
        run.record.runRef,
        claimRef,
        1,
        startedInput,
      )
      await Effect.runPromise(append([started]))

      for (const observedAt of ["2026-07-09T22:06:00.000Z"]) {
        const badClock = await Effect.runPromise(
          append([
            pylonExecutionEvent(run.record.runRef, claimRef, 2, {
              schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
              observedAt,
              kind: "work_progress",
              unitRef: "unit-a",
              workClaimRef: `work_claim.guard.clock.${observedAt.slice(14, 16)}`,
              workerKind: "codex",
              blockerRefs: [],
            }),
          ]).pipe(Effect.flip),
        )
        expect(badClock.kind).toBe("invalid_request")
      }

      const gap = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 3, {
            ...startedInput,
            observedAt: "2026-07-09T22:00:03.000Z",
          }),
        ]).pipe(Effect.flip),
      )
      expect(gap.kind).toBe("claim_conflict")

      const forgedEventRef = await Effect.runPromise(
        append([
          {
            ...pylonExecutionEvent(run.record.runRef, claimRef, 2, {
              schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
              observedAt: "2026-07-09T22:00:02.000Z",
              kind: "work_progress",
              unitRef: "unit-a",
              workClaimRef: "work_claim.forged",
              workerKind: "codex",
              blockerRefs: [],
            }),
            eventRef: `event.pylon.fleet_run.${"f".repeat(24)}`,
          },
        ]).pipe(Effect.flip),
      )
      expect(forgedEventRef.kind).toBe("invalid_request")

      const unknownUnit = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 2, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:02.000Z",
            kind: "work_progress",
            unitRef: "unit-foreign",
            workClaimRef: "work_claim.foreign",
            workerKind: "codex",
            blockerRefs: [],
          }),
        ]).pipe(Effect.flip),
      )
      expect(unknownUnit.kind).toBe("invalid_request")

      const incompleteCompletion = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 2, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:02.000Z",
            kind: "run_terminal",
            terminalState: "completed",
            blockerRefs: [],
          }),
        ]).pipe(Effect.flip),
      )
      expect(incompleteCompletion.kind).toBe("claim_conflict")
      const rowsAfterRollback: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${run.record.runRef}
      `
      expect(Number(rowsAfterRollback[0]!.count)).toBe(1)

      const conflictingReplay = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 1, {
            ...startedInput,
            observedAt: "2026-07-09T22:00:09.000Z",
          }),
        ]).pipe(Effect.flip),
      )
      expect(conflictingReplay.kind).toBe("idempotency_conflict")

      const failed = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 2, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:10.000Z",
            kind: "work_terminal",
            unitRef: "unit-a",
            workClaimRef: "work_claim.guard.unit-a.attempt-1",
            workerKind: "claude",
            terminalState: "failed",
            blockerRefs: ["blocker.dispatch_no_assignment"],
          }),
        ]),
      )
      expect(failed.ack.execution).toMatchObject({
        state: "running",
        lastSequence: 2,
        counters: { acceptedAssignments: 0, failedAssignments: 1 },
      })
      expect(failed.ack.execution.closeouts[0]).toEqual(
        expect.objectContaining({
          unitRef: "unit-a",
          terminalState: "failed",
          blockerRefs: ["blocker.dispatch_no_assignment"],
        }),
      )
      expect(failed.ack.execution.closeouts[0]).not.toHaveProperty(
        "assignmentRef",
      )
      const noSyntheticProof: Array<{
        assignment_ref: string | null
        closeout_ref: string | null
        usage_truth: string | null
      }> = await sql`
        SELECT assignment_ref, closeout_ref, usage_truth
        FROM sarah_fleet_run_work_unit_closeouts
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `
      expect(noSyntheticProof).toEqual([
        { assignment_ref: null, closeout_ref: null, usage_truth: null },
      ])

      const eventAfterTerminalAttempt = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 3, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:11.000Z",
            kind: "work_progress",
            unitRef: "unit-a",
            workClaimRef: "work_claim.guard.unit-a.attempt-1",
            workerKind: "claude",
            blockerRefs: [],
          }),
        ]).pipe(Effect.flip),
      )
      expect(eventAfterTerminalAttempt.kind).toBe("idempotency_conflict")

      const retry = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 3, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:12.000Z",
            kind: "work_progress",
            unitRef: "unit-a",
            workClaimRef: "work_claim.guard.unit-a.attempt-2",
            assignmentRef: "assignment.unit-a.attempt-2",
            workerKind: "codex",
            accountRefHash: `account.pylon.codex.${"c".repeat(24)}`,
            blockerRefs: [],
          }),
          pylonExecutionEvent(run.record.runRef, claimRef, 4, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:13.000Z",
            kind: "work_terminal",
            unitRef: "unit-a",
            workClaimRef: "work_claim.guard.unit-a.attempt-2",
            assignmentRef: "assignment.unit-a.attempt-2",
            workerKind: "codex",
            accountRefHash: `account.pylon.codex.${"c".repeat(24)}`,
            terminalState: "accepted",
            closeoutRef: "closeout.unit-a.attempt-2",
            usageEvidence: {
              truth: "exact",
              tokenUsageRefs: ["token_usage.unit-a.attempt-2"],
            },
            blockerRefs: [],
          }),
          pylonExecutionEvent(run.record.runRef, claimRef, 5, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:14.000Z",
            kind: "work_terminal",
            unitRef: "unit-b",
            workClaimRef: "work_claim.guard.unit-b.attempt-1",
            assignmentRef: "assignment.unit-b.attempt-1",
            workerKind: "grok",
            accountRefHash: `account.pylon.grok.${"d".repeat(24)}`,
            terminalState: "accepted",
            closeoutRef: "closeout.unit-b.attempt-1",
            usageEvidence: { truth: "not_measured", tokenUsageRefs: [] },
            blockerRefs: [],
          }),
        ]),
      )
      expect(retry.ack).toMatchObject({
        schema: "openagents.pylon.fleet_run_execution_ack.v1",
        runRef: run.record.runRef,
        claimRef,
        acceptedThroughSequence: 5,
        storedEventCount: 3,
        execution: {
          state: "running",
          lastSequence: 5,
          counters: {
            workUnitsTotal: 2,
            activeAssignments: 0,
            acceptedAssignments: 0,
            failedAssignments: 1,
            staleAssignments: 0,
          },
          startedAt: "2026-07-09T22:00:00.000Z",
        },
      })
      const unprovenCompletion = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 6, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:15.000Z",
            kind: "run_terminal",
            terminalState: "completed",
            blockerRefs: [],
          }),
        ]).pipe(Effect.flip),
      )
      expect(unprovenCompletion.kind).toBe("claim_conflict")
      const stopped = await Effect.runPromise(
        append([
          pylonExecutionEvent(run.record.runRef, claimRef, 6, {
            schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
            observedAt: "2026-07-09T22:00:15.000Z",
            kind: "run_terminal",
            terminalState: "stopped",
            blockerRefs: ["blocker.operator.stopped"],
          }),
        ]),
      )
      expect(stopped.ack.execution.state).toBe("stopped")
      expect(stopped.ack.execution.closeouts).toHaveLength(3)
      expect(
        stopped.ack.execution.closeouts.map((closeout) => [
          closeout.unitRef,
          closeout.workClaimRef,
          closeout.terminalState,
        ]),
      ).toEqual([
        ["unit-a", "work_claim.guard.unit-a.attempt-1", "failed"],
        ["unit-a", "work_claim.guard.unit-a.attempt-2", "accepted"],
        ["unit-b", "work_claim.guard.unit-b.attempt-1", "accepted"],
      ])
      const persistedAttempts: Array<{
        attempt_ref: string
        work_unit_ref: string
        state: string
      }> = await sql`
        SELECT attempt_ref, work_unit_ref, state
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
        ORDER BY attempt_ref
      `
      expect(persistedAttempts).toEqual([
        {
          attempt_ref: "work_claim.guard.unit-a.attempt-1",
          work_unit_ref: "unit-a",
          state: "failed",
        },
        {
          attempt_ref: "work_claim.guard.unit-a.attempt-2",
          work_unit_ref: "unit-a",
          state: "evidence_pending",
        },
        {
          attempt_ref: "work_claim.guard.unit-b.attempt-1",
          work_unit_ref: "unit-b",
          state: "evidence_pending",
        },
      ])
      const latestPointers: Array<{
        unit_ref: string
        state: string
        latest_attempt_ref: string | null
        accepted_attempt_ref: string | null
      }> = await sql`
        SELECT unit_ref, state, latest_attempt_ref, accepted_attempt_ref
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${run.record.runRef}
        ORDER BY unit_ref
      `
      expect(latestPointers).toEqual([
        {
          unit_ref: "unit-a",
          state: "verification_pending",
          latest_attempt_ref: "work_claim.guard.unit-a.attempt-2",
          accepted_attempt_ref: null,
        },
        {
          unit_ref: "unit-b",
          state: "verification_pending",
          latest_attempt_ref: "work_claim.guard.unit-b.attempt-1",
          accepted_attempt_ref: null,
        },
      ])

      const failedPointer = await sql`
        UPDATE sarah_fleet_run_work_units
        SET state = 'succeeded',
            latest_attempt_ref = 'work_claim.guard.unit-a.attempt-1',
            accepted_attempt_ref = 'work_claim.guard.unit-a.attempt-1'
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `.then(
        () => null,
        (error) => error,
      )
      expect(String(failedPointer)).toContain(
        "accepted fleet attempt must be succeeded",
      )
      const crossUnitPointer = await sql`
        UPDATE sarah_fleet_run_work_units
        SET latest_attempt_ref = 'work_claim.guard.unit-a.attempt-2'
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-b'
      `.then(
        () => null,
        (error) => error,
      )
      expect(String(crossUnitPointer)).toContain(
        "sarah_fleet_run_work_units_latest_attempt_fk",
      )
      const emptyFailedBlockers = await sql`
        UPDATE sarah_fleet_run_attempts
        SET blocker_refs_json = '[]'
        WHERE run_ref = ${run.record.runRef}
          AND attempt_ref = 'work_claim.guard.unit-a.attempt-1'
      `.then(
        () => null,
        (error) => error,
      )
      expect(String(emptyFailedBlockers)).toContain(
        "sarah_fleet_run_attempts_evidence_coherence",
      )
      const duplicateGlobalAttempt = await sql`
        UPDATE sarah_fleet_run_attempts
        SET attempt_ref = 'work_claim.unit-a.attempt-1'
        WHERE run_ref = ${run.record.runRef}
          AND attempt_ref = 'work_claim.guard.unit-a.attempt-2'
      `.then(
        () => null,
        (error) => error,
      )
      expect(String(duplicateGlobalAttempt)).toContain(
        "sarah_fleet_run_attempts_attempt_ref_key",
      )
    })

    test("repairs a completed legacy v1 closeout as evidence pending without rewriting terminal history", async () => {
      const ownerUserId = "user-legacy-completed-owner"
      const pylonRef = "pylon-legacy-completed"
      const run = await start(ownerUserId, "legacy-completed-run-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.legacy.completed",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "legacy-completed-claim",
      })
      const accountRefHash = `account.pylon.codex.${"e".repeat(24)}`
      const workClaimRef = "work_claim.legacy/Users/operator/completed"
      const assignmentRef = "assignment.legacy/Users/operator/completed"
      const closeoutRef = "closeout.legacy/Users/operator/completed"
      const repairedWorkClaimRef = legacyProjectedDigestRef(
        "work_claim.pylon.fleet_run.legacy",
        workClaimRef,
      )
      const repairedAssignmentRef = legacyProjectedDigestRef(
        "assignment.pylon.fleet_run.legacy",
        assignmentRef,
      )
      const repairedCloseoutRef = legacyProjectedDigestRef(
        "closeout.pylon.fleet_run.legacy",
        closeoutRef,
      )
      const legacyEvents = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T21:59:58.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T21:59:57.000Z",
          kind: "work_terminal",
          unitRef: "unit-a",
          workClaimRef,
          assignmentRef,
          workerKind: "codex",
          accountRefHash,
          terminalState: "accepted",
          closeoutRef,
          usageEvidence: {
            truth: "exact",
            tokenUsageRefs: ["token_usage.legacy.completed.unit-a.1"],
          },
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T21:59:56.000Z",
          kind: "run_terminal",
          terminalState: "completed",
          blockerRefs: [],
        }),
      ] as const
      for (const [index, event] of legacyEvents.entries()) {
        await sql`
          INSERT INTO sarah_fleet_run_execution_events
            (run_ref, sequence, event_ref, owner_user_id, pylon_ref,
             intake_claim_ref, event_kind, unit_ref, work_claim_ref,
             event_json, observed_at, recorded_at)
          VALUES
            (${run.record.runRef}, ${event.sequence}, ${event.eventRef},
             ${ownerUserId}, ${pylonRef}, ${claimRef}, ${event.kind},
             ${"unitRef" in event ? event.unitRef : null},
             ${"workClaimRef" in event ? event.workClaimRef : null},
             ${canonicalJson(event)}, ${event.observedAt},
             ${`2026-07-09T22:00:1${index}.000Z`})
        `
      }
      const terminalEvent = legacyEvents[1]
      await sql`
        INSERT INTO sarah_fleet_run_work_unit_closeouts
          (run_ref, unit_ref, work_claim_ref, assignment_ref, worker_kind,
           account_ref_hash, terminal_state, closeout_ref, usage_truth,
           token_usage_refs_json, blocker_refs_json, observed_at, event_ref)
        VALUES
          (${run.record.runRef}, 'unit-a', ${workClaimRef}, ${assignmentRef},
           'codex', ${accountRefHash}, 'accepted',
           ${closeoutRef}, 'exact',
           '["token_usage.legacy.completed.unit-a.1"]', '[]',
           ${terminalEvent.observedAt}, ${terminalEvent.eventRef})
      `
      await sql`
        UPDATE sarah_fleet_run_requests
        SET execution_state = 'completed', execution_last_sequence = 3,
            execution_started_at = '2026-07-09T21:59:58.000Z',
            execution_updated_at = '2026-07-09T21:59:56.000Z'
        WHERE run_ref = ${run.record.runRef}
      `

      const repaired: Array<{ repaired: string | number }> = await sql`
        SELECT sarah_backfill_fleet_run_attempts_v1() AS repaired
      `
      expect(Number(repaired[0]!.repaired)).toBe(1)
      const attempts: Array<{
        attempt_ref: string
        assignment_ref: string | null
        closeout_ref: string | null
        state: string
        marginal_cost_class: string
        verification_json: string
        usage_json: string
        last_observed_at: string
        remote_observed_at: string
      }> = await sql`
        SELECT attempt_ref, assignment_ref, closeout_ref, state,
               marginal_cost_class, verification_json, usage_json,
               last_observed_at, remote_observed_at
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
      `
      expect(attempts).toEqual([
        {
          attempt_ref: repairedWorkClaimRef,
          assignment_ref: repairedAssignmentRef,
          closeout_ref: repairedCloseoutRef,
          state: "evidence_pending",
          marginal_cost_class: "not_measured",
          verification_json: '{"truth":"not_reported"}',
          usage_json: '{"truth":"pending"}',
          last_observed_at: "2026-07-09T22:00:11.000Z",
          remote_observed_at: "2026-07-09T21:59:57.000Z",
        },
      ])
      const units: Array<{
        state: string
        latest_attempt_ref: string | null
        accepted_attempt_ref: string | null
      }> = await sql`
        SELECT state, latest_attempt_ref, accepted_attempt_ref
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `
      expect(units).toEqual([
        {
          state: "verification_pending",
          latest_attempt_ref: repairedWorkClaimRef,
          accepted_attempt_ref: null,
        },
      ])
      const observed = await Effect.runPromise(
        repository().observe({ ownerUserId, runRef: run.record.runRef }),
      )
      expect(observed.record.execution).toMatchObject({
        state: "completed",
        counters: {
          activeAssignments: 0,
          acceptedAssignments: 0,
          staleAssignments: 0,
        },
        startedAt: "2026-07-09T22:00:10.000Z",
        updatedAt: "2026-07-09T22:00:12.000Z",
      })
      expect(observed.record.execution.closeouts[0]).toMatchObject({
        workClaimRef: repairedWorkClaimRef,
        assignmentRef: repairedAssignmentRef,
        closeoutRef: repairedCloseoutRef,
      })
      const repairs: Array<{ entity_type: string; mutation_ref: string }> =
        await sql`
          SELECT entity_type, mutation_ref
          FROM khala_sync_changelog
          WHERE scope = ${run.record.scope}
            AND mutation_ref = 'system:sarah_fleet_run_attempt_backfill.v1'
          ORDER BY entity_type
        `
      expect(repairs).toEqual([
        {
          entity_type: "fleet_attempt",
          mutation_ref: "system:sarah_fleet_run_attempt_backfill.v1",
        },
        {
          entity_type: "fleet_run",
          mutation_ref: "system:sarah_fleet_run_attempt_backfill.v1",
        },
        {
          entity_type: "fleet_work_unit",
          mutation_ref: "system:sarah_fleet_run_attempt_backfill.v1",
        },
      ])
      const repairedPostImages: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND mutation_ref = 'system:sarah_fleet_run_attempt_backfill.v1'
      `
      const serializedRepairedPostImages = JSON.stringify(repairedPostImages)
      expect(serializedRepairedPostImages).toContain(repairedWorkClaimRef)
      expect(serializedRepairedPostImages).toContain(repairedAssignmentRef)
      expect(serializedRepairedPostImages).toContain(repairedCloseoutRef)
      expect(serializedRepairedPostImages).not.toContain("/Users/operator")
      const beforeReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
      `
      const replay: Array<{ repaired: string | number }> = await sql`
        SELECT sarah_backfill_fleet_run_attempts_v1() AS repaired
      `
      expect(Number(replay[0]!.repaired)).toBe(0)
      const afterReplay: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
      `
      expect(afterReplay).toEqual(beforeReplay)
    })

    test("terminalizes a progress-only legacy attempt when its run already stopped", async () => {
      const ownerUserId = "user-legacy-stopped-owner"
      const pylonRef = "pylon-legacy-stopped"
      const run = await start(ownerUserId, "legacy-stopped-run-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.legacy.stopped",
          units: [{ unitRef: "unit-a", title: "Unit A", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "legacy-stopped-claim",
      })
      const workClaimRef = "work_claim.legacy/Users/operator/stopped"
      const assignmentRef = "assignment.legacy/Users/operator/stopped"
      const repairedWorkClaimRef = legacyProjectedDigestRef(
        "work_claim.pylon.fleet_run.legacy",
        workClaimRef,
      )
      const repairedAssignmentRef = legacyProjectedDigestRef(
        "assignment.pylon.fleet_run.legacy",
        assignmentRef,
      )
      const legacyEvents = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:04.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T21:59:58.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef,
          assignmentRef,
          workerKind: "grok",
          blockerRefs: [
            "blocker.path/Users/operator/worktree",
            "blocker.path/Users/operator/worktree",
          ],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:03.000Z",
          kind: "run_terminal",
          terminalState: "stopped",
          blockerRefs: ["blocker.operator.stopped"],
        }),
      ] as const
      for (const [index, event] of legacyEvents.entries()) {
        await sql`
          INSERT INTO sarah_fleet_run_execution_events
            (run_ref, sequence, event_ref, owner_user_id, pylon_ref,
             intake_claim_ref, event_kind, unit_ref, work_claim_ref,
             event_json, observed_at, recorded_at)
          VALUES
            (${run.record.runRef}, ${event.sequence}, ${event.eventRef},
             ${ownerUserId}, ${pylonRef}, ${claimRef}, ${event.kind},
             ${"unitRef" in event ? event.unitRef : null},
             ${"workClaimRef" in event ? event.workClaimRef : null},
             ${canonicalJson(event)}, ${event.observedAt},
             ${`2026-07-09T22:00:2${index}.000Z`})
        `
      }
      await sql`
        UPDATE sarah_fleet_run_requests
        SET execution_state = 'stopped', execution_last_sequence = 3,
            execution_started_at = '2026-07-09T22:00:04.000Z',
            execution_updated_at = '2026-07-09T22:00:03.000Z'
        WHERE run_ref = ${run.record.runRef}
      `

      const repaired: Array<{ repaired: string | number }> = await sql`
        SELECT sarah_backfill_fleet_run_attempts_v1() AS repaired
      `
      expect(Number(repaired[0]!.repaired)).toBe(1)
      const attempts: Array<{
        attempt_ref: string
        assignment_ref: string | null
        state: string
        progress_class: string
        verification_json: string
        blocker_refs_json: string
        terminal_at: string | null
        last_observed_at: string
        remote_observed_at: string
      }> = await sql`
        SELECT attempt_ref, assignment_ref, state, progress_class,
               verification_json, blocker_refs_json, terminal_at,
               last_observed_at, remote_observed_at
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
      `
      expect(attempts).toEqual([
        {
          attempt_ref: repairedWorkClaimRef,
          assignment_ref: repairedAssignmentRef,
          state: "stale",
          progress_class: "terminal",
          verification_json: '{"truth":"not_reported"}',
          blocker_refs_json:
            canonicalJson([
              "blocker.pylon.fleet_run.legacy_identity_unprojectable",
              legacyBlockerDigestRef(
                "blocker.path/Users/operator/worktree",
              ),
            ]),
          terminal_at: "2026-07-09T22:00:21.000Z",
          last_observed_at: "2026-07-09T22:00:21.000Z",
          remote_observed_at: "2026-07-09T21:59:58.000Z",
        },
      ])
      const observed = await Effect.runPromise(
        repository().observe({ ownerUserId, runRef: run.record.runRef }),
      )
      expect(observed.record.execution).toMatchObject({
        state: "stopped",
        counters: {
          activeAssignments: 0,
          acceptedAssignments: 0,
          staleAssignments: 1,
        },
        startedAt: "2026-07-09T22:00:20.000Z",
        updatedAt: "2026-07-09T22:00:22.000Z",
      })
      const units: Array<{
        state: string
        latest_attempt_ref: string | null
        accepted_attempt_ref: string | null
      }> = await sql`
        SELECT state, latest_attempt_ref, accepted_attempt_ref
        FROM sarah_fleet_run_work_units
        WHERE run_ref = ${run.record.runRef} AND unit_ref = 'unit-a'
      `
      expect(units).toEqual([
        {
          state: "stale",
          latest_attempt_ref: repairedWorkClaimRef,
          accepted_attempt_ref: null,
        },
      ])
      const repairImages: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
          AND mutation_ref = 'system:sarah_fleet_run_attempt_backfill.v1'
        ORDER BY entity_type, entity_id
      `
      const serializedRepair = repairImages
        .map((row) =>
          typeof row.post_image_json === "string"
            ? row.post_image_json
            : JSON.stringify(row.post_image_json),
        )
        .join("\n")
      expect(serializedRepair).toContain(
        legacyBlockerDigestRef("blocker.path/Users/operator/worktree"),
      )
      expect(serializedRepair).toContain(repairedWorkClaimRef)
      expect(serializedRepair).toContain(repairedAssignmentRef)
      expect(serializedRepair).not.toContain("/Users/operator/worktree")
      expect(serializedRepair).not.toContain("operator")
      expect(serializedRepair).not.toContain("worktree")
    })

    test("repairs unsafe running progress and failed closeout identities before a safe retry", async () => {
      const ownerUserId = "user-legacy-running-owner"
      const pylonRef = "pylon-legacy-running"
      const run = await start(ownerUserId, "legacy-running-run-1", {
        workSource: {
          kind: "plan_dag",
          planRef: "plan.legacy.running",
          units: [
            { unitRef: "unit-a", title: "Unit A", dependsOn: [] },
            { unitRef: "unit-b", title: "Unit B", dependsOn: [] },
          ],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: run.record.runRef,
        claimIdempotencyKey: "legacy-running-claim",
      })
      const progressWorkClaimRef =
        "work_claim.path/Users/operator/running-progress"
      const progressAssignmentRef =
        "assignment.path/Users/operator/running-progress"
      const failedWorkClaimRef =
        "work_claim.path/Users/operator/running-failed"
      const failedAssignmentRef =
        "assignment.path/Users/operator/running-failed"
      const failedCloseoutRef =
        "closeout.path/Users/operator/running-failed"
      const failedBlockerRef =
        "blocker.path/Users/operator/running-failed"
      const accountRefHash = `account.pylon.codex.${"f".repeat(24)}`
      const legacyEvents = [
        pylonExecutionEvent(run.record.runRef, claimRef, 1, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:01.000Z",
          kind: "run_started",
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 2, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:02.000Z",
          kind: "work_progress",
          unitRef: "unit-a",
          workClaimRef: progressWorkClaimRef,
          assignmentRef: progressAssignmentRef,
          workerKind: "codex",
          accountRefHash,
          blockerRefs: [],
        }),
        pylonExecutionEvent(run.record.runRef, claimRef, 3, {
          schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
          observedAt: "2026-07-09T22:00:03.000Z",
          kind: "work_terminal",
          unitRef: "unit-b",
          workClaimRef: failedWorkClaimRef,
          assignmentRef: failedAssignmentRef,
          workerKind: "codex",
          accountRefHash,
          terminalState: "failed",
          closeoutRef: failedCloseoutRef,
          usageEvidence: {
            truth: "exact",
            tokenUsageRefs: ["token_usage.legacy.running.failed.1"],
          },
          blockerRefs: [failedBlockerRef],
        }),
      ] as const
      for (const [index, event] of legacyEvents.entries()) {
        await sql`
          INSERT INTO sarah_fleet_run_execution_events
            (run_ref, sequence, event_ref, owner_user_id, pylon_ref,
             intake_claim_ref, event_kind, unit_ref, work_claim_ref,
             event_json, observed_at, recorded_at)
          VALUES
            (${run.record.runRef}, ${event.sequence}, ${event.eventRef},
             ${ownerUserId}, ${pylonRef}, ${claimRef}, ${event.kind},
             ${"unitRef" in event ? event.unitRef : null},
             ${"workClaimRef" in event ? event.workClaimRef : null},
             ${canonicalJson(event)}, ${event.observedAt},
             ${`2026-07-09T22:00:3${index}.000Z`})
        `
      }
      const failedEvent = legacyEvents[2]
      await sql`
        INSERT INTO sarah_fleet_run_work_unit_closeouts
          (run_ref, unit_ref, work_claim_ref, assignment_ref, worker_kind,
           account_ref_hash, terminal_state, closeout_ref, usage_truth,
           token_usage_refs_json, blocker_refs_json, observed_at, event_ref)
        VALUES
          (${run.record.runRef}, 'unit-b', ${failedWorkClaimRef},
           ${failedAssignmentRef}, 'codex', ${accountRefHash}, 'failed',
           ${failedCloseoutRef}, 'exact',
           '["token_usage.legacy.running.failed.1"]',
           ${canonicalJson([failedBlockerRef])}, ${failedEvent.observedAt},
           ${failedEvent.eventRef})
      `
      await sql`
        UPDATE sarah_fleet_run_requests
        SET execution_state = 'running', execution_last_sequence = 3,
            execution_started_at = '2026-07-09T22:00:01.000Z',
            execution_updated_at = '2026-07-09T22:00:03.000Z'
        WHERE run_ref = ${run.record.runRef}
      `

      const repaired: Array<{ repaired: string | number }> = await sql`
        SELECT sarah_backfill_fleet_run_attempts_v1() AS repaired
      `
      expect(Number(repaired[0]!.repaired)).toBe(2)
      const repairedProgressRef = legacyProjectedDigestRef(
        "work_claim.pylon.fleet_run.legacy",
        progressWorkClaimRef,
      )
      const repairedFailedRef = legacyProjectedDigestRef(
        "work_claim.pylon.fleet_run.legacy",
        failedWorkClaimRef,
      )
      const attempts: Array<{
        attempt_ref: string
        state: string
        assignment_ref: string | null
        closeout_ref: string | null
        blocker_refs_json: string
      }> = await sql`
        SELECT attempt_ref, state, assignment_ref, closeout_ref,
               blocker_refs_json
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${run.record.runRef}
        ORDER BY work_unit_ref
      `
      expect(attempts).toEqual([
        {
          attempt_ref: repairedProgressRef,
          state: "stale",
          assignment_ref: legacyProjectedDigestRef(
            "assignment.pylon.fleet_run.legacy",
            progressAssignmentRef,
          ),
          closeout_ref: null,
          blocker_refs_json: canonicalJson([
            "blocker.pylon.fleet_run.legacy_identity_unprojectable",
          ]),
        },
        {
          attempt_ref: repairedFailedRef,
          state: "failed",
          assignment_ref: legacyProjectedDigestRef(
            "assignment.pylon.fleet_run.legacy",
            failedAssignmentRef,
          ),
          closeout_ref: legacyProjectedDigestRef(
            "closeout.pylon.fleet_run.legacy",
            failedCloseoutRef,
          ),
          blocker_refs_json: canonicalJson([
            legacyBlockerDigestRef(failedBlockerRef),
          ]),
        },
      ])
      const repairedObservation = await Effect.runPromise(
        repository().observe({ ownerUserId, runRef: run.record.runRef }),
      )
      expect(repairedObservation.record.execution).toMatchObject({
        state: "running",
        counters: {
          activeAssignments: 0,
          failedAssignments: 1,
          staleAssignments: 1,
        },
      })
      expect(JSON.stringify(repairedObservation)).not.toContain(
        "/Users/operator",
      )

      const safeRetry = await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: run.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA,
            claimRef,
            events: [
              pylonExecutionEvent(run.record.runRef, claimRef, 4, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA,
                observedAt: "2026-07-09T22:00:04.000Z",
                kind: "work_progress",
                unitRef: "unit-a",
                workClaimRef: "work_claim.legacy.running.safe-retry",
                assignmentRef: "assignment.legacy.running.safe-retry",
                workerKind: "codex",
                accountRefHash,
                blockerRefs: [],
              }),
            ],
          },
        }),
      )
      expect(safeRetry.ack.execution).toMatchObject({
        state: "running",
        lastSequence: 4,
        counters: {
          activeAssignments: 1,
          failedAssignments: 1,
          staleAssignments: 1,
        },
      })
      const repairImages: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json FROM khala_sync_changelog
        WHERE scope = ${run.record.scope}
      `
      expect(JSON.stringify(repairImages)).not.toContain("/Users/operator")
    })

    test("claim-next skips managed-cloud runs and concurrent exact claims have one winner", async () => {
      await seedPylon({
        pylonRef: "pylon-next-a",
        ownerUserId: "user-next-owner",
      })
      await seedPylon({
        pylonRef: "pylon-next-b",
        ownerUserId: "user-next-owner",
      })
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
      expect(
        attempts.filter((result) => result.status === "fulfilled"),
      ).toHaveLength(1)
      expect(
        attempts.filter((result) => result.status === "rejected"),
      ).toHaveLength(1)
      const leases: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_intake_leases
        WHERE run_ref = ${race.record.runRef} AND state = 'claimed'
      `
      expect(Number(leases[0]!.count)).toBe(1)
    })

    test("claims only an exact owner-scoped managed-cloud Codex run and preserves its capacity", async () => {
      const ownerUserId = "user-managed-cloud-owner"
      const pylonRef = "pylon-managed-cloud-owner"
      const cloud = await start(ownerUserId, "managed-cloud-codex-1", {
        targetPreference: "managed_cloud",
        workerKind: "codex",
        workSource: {
          kind: "plan_dag",
          planRef: "plan.managed-cloud",
          units: [{ unitRef: "unit-cloud", title: "Cloud unit", dependsOn: [] }],
        },
      })
      await seedPylon({ pylonRef, ownerUserId })
      await seedPylon({
        pylonRef: "pylon-managed-cloud-foreign",
        ownerUserId: "user-managed-cloud-foreign",
      })

      const wrongOwner = await Effect.runPromise(
        repository()
          .claim({
            ownerUserId: "user-managed-cloud-foreign",
            pylonRef: "pylon-managed-cloud-foreign",
            runRef: cloud.record.runRef,
            claimIdempotencyKey: "managed-cloud-wrong-owner",
            leaseDurationMs: 30_000,
          })
          .pipe(Effect.flip),
      )
      expect(wrongOwner.kind).toBe("run_not_found")

      for (const workerKind of ["claude", "grok"] as const) {
        const unsupported = await start(
          ownerUserId,
          `managed-cloud-${workerKind}-1`,
          { targetPreference: "managed_cloud", workerKind },
        )
        const error = await Effect.runPromise(
          repository()
            .claim({
              ownerUserId,
              pylonRef,
              runRef: unsupported.record.runRef,
              claimIdempotencyKey: `managed-cloud-${workerKind}-claim`,
              leaseDurationMs: 30_000,
            })
            .pipe(Effect.flip),
        )
        expect(error.kind).toBe("run_not_found")
      }

      const claimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: cloud.record.runRef,
        claimIdempotencyKey: "managed-cloud-codex-claim",
      })
      const wrongHarness = await Effect.runPromise(
        repository()
          .appendExecutionEvents({
            ownerUserId,
            pylonRef,
            runRef: cloud.record.runRef,
            batch: {
              schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
              claimRef,
              events: [
                pylonExecutionEvent(cloud.record.runRef, claimRef, 1, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:01.000Z",
                  kind: "run_started",
                }),
                pylonExecutionEvent(cloud.record.runRef, claimRef, 2, {
                  schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                  observedAt: "2026-07-09T22:00:02.000Z",
                  kind: "work_progress",
                  unitRef: "unit-cloud",
                  workClaimRef: "work_claim.managed-cloud.claude.refused",
                  workerKind: "claude",
                  accountRefHash: `account.pylon.claude_agent.${"d".repeat(24)}`,
                  blockerRefs: [],
                }),
              ],
            },
          })
          .pipe(Effect.flip),
      )
      expect(wrongHarness.kind).toBe("invalid_request")
      const rolledBackEvents: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${cloud.record.runRef}
      `
      expect(Number(rolledBackEvents[0]!.count)).toBe(0)

      const accountRefHash = `account.pylon.codex.${"c".repeat(24)}`
      await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: cloud.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef,
            events: [
              pylonExecutionEvent(cloud.record.runRef, claimRef, 1, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:01.000Z",
                kind: "run_started",
              }),
              pylonExecutionEvent(cloud.record.runRef, claimRef, 2, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:02.000Z",
                kind: "work_progress",
                unitRef: "unit-cloud",
                workClaimRef: "work_claim.managed-cloud.codex.1",
                assignmentRef: "assignment.managed-cloud.codex.1",
                workerKind: "codex",
                accountRefHash,
                marginalCostClass: "not_measured",
                blockerRefs: [],
              }),
              pylonExecutionEvent(cloud.record.runRef, claimRef, 3, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:03.000Z",
                kind: "work_terminal",
                unitRef: "unit-cloud",
                workClaimRef: "work_claim.managed-cloud.codex.1",
                assignmentRef: "assignment.managed-cloud.codex.1",
                workerKind: "codex",
                accountRefHash,
                terminalState: "failed",
                closeoutRef: "closeout.managed-cloud.codex.1",
                verification: {
                  truth: "failed",
                  verifierRef: "verifier.managed-cloud.codex.1",
                  evidenceRefs: ["evidence.managed-cloud.codex.1"],
                },
                blockerRefs: ["blocker.managed_cloud.executor_failed"],
              }),
            ],
          },
        }),
      )

      const attemptRows: Array<{
        capacity_class: string
        worker_kind: string
      }> = await sql`
        SELECT capacity_class, worker_kind
        FROM sarah_fleet_run_attempts
        WHERE run_ref = ${cloud.record.runRef}
      `
      expect(attemptRows).toEqual([
        { capacity_class: "managed_cloud", worker_kind: "codex" },
      ])
      const projectedAttempts: Array<{ post_image_json: unknown }> = await sql`
        SELECT post_image_json
        FROM khala_sync_changelog
        WHERE scope = ${cloud.record.scope} AND entity_type = 'fleet_attempt'
        ORDER BY version
      `
      expect(projectedAttempts).toHaveLength(1)
      const projectedAttempt = projectedAttempts.at(-1)!.post_image_json
      expect(
        typeof projectedAttempt === "string"
          ? projectedAttempt
          : JSON.stringify(projectedAttempt),
      ).toContain('"capacityClass":"managed_cloud"')

      const auto = await start(ownerUserId, "managed-cloud-auto-control-1", {
        targetPreference: "auto",
        workerKind: "codex",
      })
      const autoClaimRef = await claimAndAccept({
        ownerUserId,
        pylonRef,
        runRef: auto.record.runRef,
        claimIdempotencyKey: "managed-cloud-auto-control-claim",
      })
      await Effect.runPromise(
        repository().appendExecutionEvents({
          ownerUserId,
          pylonRef,
          runRef: auto.record.runRef,
          batch: {
            schema: FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2,
            claimRef: autoClaimRef,
            events: [
              pylonExecutionEvent(auto.record.runRef, autoClaimRef, 1, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:01.000Z",
                kind: "run_started",
              }),
              pylonExecutionEvent(auto.record.runRef, autoClaimRef, 2, {
                schema: FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
                observedAt: "2026-07-09T22:00:02.000Z",
                kind: "work_progress",
                unitRef: "issue.8637",
                workClaimRef: "work_claim.auto.owner-local.1",
                workerKind: "codex",
                blockerRefs: [],
              }),
            ],
          },
        }),
      )
      const autoAttempts: Array<{ capacity_class: string }> = await sql`
        SELECT capacity_class FROM sarah_fleet_run_attempts
        WHERE run_ref = ${auto.record.runRef}
      `
      expect(autoAttempts).toEqual([{ capacity_class: "owner_local" }])
    })

    test("an expired claim requires a new idempotency key and can be re-leased", async () => {
      const run = await start("user-expiry-owner", "expiry-run-1")
      await seedPylon({
        pylonRef: "pylon-expiry-a",
        ownerUserId: "user-expiry-owner",
      })
      await seedPylon({
        pylonRef: "pylon-expiry-b",
        ownerUserId: "user-expiry-owner",
      })
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
