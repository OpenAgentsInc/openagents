import { SQL } from "@openagentsinc/postgres-runtime";
import { PORTABLE_CAPABILITY_BROKER_VERSION } from "@openagentsinc/portable-session-contract";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import {
  HttpPortableCapabilityGrantFactAuthority,
  PostgresPortableCommandCapabilityGrantFactResolver,
} from "./portable-command-capability-grant-resolver.js";
import type { PortableCommandCapabilityGrantFactScope } from "./portable-session-command-runner.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const scope: PortableCommandCapabilityGrantFactScope = {
  commandExecutionClaimRef: "claim.ide13.facts",
  commandLeaseExpiresAt: "2026-07-20T12:07:00.000Z",
  ownerRef: "owner.ide13.facts",
  sessionRef: "session.ide13.facts",
  sourceAttachmentRef: "attachment.ide13.source",
  sourceGeneration: 3,
  sourceTargetRef: "target.ide13.source",
  destinationAttachmentRef: "attachment.ide13.destination",
  destinationGeneration: 4,
  destinationTargetRef: "target.ide13.destination",
  sourceLeaseRefs: ["lease.ide13.source"],
};

const state = {
  schema: PORTABLE_CAPABILITY_BROKER_VERSION,
  records: [
    {
      lease: {
        leaseRef: "lease.ide13.source",
        ownerRef: scope.ownerRef,
        sessionRef: scope.sessionRef,
        attachmentRef: scope.sourceAttachmentRef,
        attachmentGeneration: scope.sourceGeneration,
        targetRef: scope.sourceTargetRef,
        capability: "provider" as const,
        expiresAt: "2026-07-20T12:08:00.000Z",
        state: "issued" as const,
      },
      sourceGrantRef: "grant.ide13.source",
      permissions: ["provider.inference"],
      issuedAt: "2026-07-20T11:00:00.000Z",
      renewalCount: 0,
    },
  ],
  operations: [],
  evidence: [],
  material: "excluded" as const,
};

const sqlWith = (stateJson: unknown): SyncSql =>
  Object.assign(
    async () => [
      {
        state_json: stateJson,
        claim_command_ref: "command.ide13.facts",
        active_move_ref: null,
        active_command_ref: null,
        active_source_attachment_ref: null,
        active_source_generation: null,
        active_destination_target_ref: null,
      },
    ],
    {
      begin: async () => {
        throw new Error("transaction is not expected");
      },
    },
  ) as SyncSql;

describe("portable command capability grant facts", () => {
  it("uses the service-authenticated refs-only fact route", async () => {
    let seen: RequestInit | undefined;
    const authority = new HttpPortableCapabilityGrantFactAuthority({
      baseUrl: "https://openagents.example",
      serviceBearer: "service.ide13.facts",
      fetch: async (input, init) => {
        expect(new URL(input instanceof Request ? input.url : input.toString()).pathname).toBe(
          "/api/portable-capability-grants/facts",
        );
        seen = init;
        return Response.json({
          facts: [
            {
              grantRef: "grant.ide13.source",
              ownerUserId: scope.ownerRef,
              kind: "github",
              status: "issued",
              expiresAt: "2026-07-20T12:09:00.000Z",
            },
          ],
          material: "excluded",
        });
      },
    });
    await expect(
      authority.resolve({
        ownerUserId: scope.ownerRef,
        grantRefs: ["grant.ide13.source"],
      }),
    ).resolves.toHaveLength(1);
    expect(seen?.headers).toMatchObject({ authorization: "Bearer service.ide13.facts" });
  });

  it("binds the exact broker lease to an active owner grant and clamps expiry", async () => {
    const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
      sql: sqlWith(state),
      authority: {
        resolve: async () => [
          {
            grantRef: "grant.ide13.source",
            ownerUserId: scope.ownerRef,
            kind: "provider",
            providerAccountRef: "provider-account.ide13",
            runnerSessionId: "runner-session.ide13",
            status: "issued",
            expiresAt: "2026-07-20T12:09:00.000Z",
          },
        ],
      },
      now: () => "2026-07-20T12:00:00.000Z",
    });
    const result = await resolver.resolve(scope);
    expect(result.facts).toEqual([
      {
        sourceLeaseRef: "lease.ide13.source",
        destinationSourceGrantRef: expect.stringMatching(/^grant\.portable\.[a-f0-9]{64}$/u),
        expiresAt: "2026-07-20T12:07:00.000Z",
      },
    ]);
    expect(result.bindings).toEqual([
      {
        sourceLeaseRef: "lease.ide13.source",
        grantRef: "grant.ide13.source",
        ownerUserId: scope.ownerRef,
        kind: "provider",
        providerAccountRef: "provider-account.ide13",
        runnerSessionId: "runner-session.ide13",
      },
    ]);
  });

  it("preserves each durable lease-to-grant mapping when authority facts are reordered", async () => {
    const secondLeaseRef = "lease.ide13.source-two";
    const secondGrantRef = "grant.ide13.source-two";
    const twoLeaseScope = {
      ...scope,
      sourceLeaseRefs: [scope.sourceLeaseRefs[0]!, secondLeaseRef],
    };
    const twoLeaseState = {
      ...state,
      records: [
        ...state.records,
        {
          ...state.records[0]!,
          lease: { ...state.records[0]!.lease, leaseRef: secondLeaseRef },
          sourceGrantRef: secondGrantRef,
        },
      ],
    };
    const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
      sql: sqlWith(twoLeaseState),
      authority: {
        resolve: async () => [
          {
            grantRef: secondGrantRef,
            ownerUserId: scope.ownerRef,
            kind: "provider",
            providerAccountRef: "provider-account.ide13.two",
            status: "issued",
            expiresAt: "2026-07-20T12:09:00.000Z",
          },
          {
            grantRef: "grant.ide13.source",
            ownerUserId: scope.ownerRef,
            kind: "provider",
            providerAccountRef: "provider-account.ide13.one",
            status: "issued",
            expiresAt: "2026-07-20T12:09:00.000Z",
          },
        ],
      },
      now: () => "2026-07-20T12:00:00.000Z",
    });

    await expect(resolver.resolve(twoLeaseScope)).resolves.toMatchObject({
      bindings: [
        { sourceLeaseRef: "lease.ide13.source", grantRef: "grant.ide13.source" },
        { sourceLeaseRef: secondLeaseRef, grantRef: secondGrantRef },
      ],
    });
  });

  it("rejects a broker lease-set mismatch before the grant authority call", async () => {
    let called = false;
    const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
      sql: sqlWith({ ...state, records: [] }),
      authority: {
        resolve: async () => {
          called = true;
          return [];
        },
      },
    });
    await expect(resolver.resolve(scope)).rejects.toThrow(/lease does not match/);
    expect(called).toBe(false);
  });
});

describe.skipIf(!hasLocalPostgres())(
  "portable command capability grant facts in migrated PostgreSQL",
  () => {
    let pg: LocalPostgres;
    let sql: SQL;

    beforeAll(async () => {
      pg = await startLocalPostgres();
      const admin = SQL({ url: pg.url, max: 1 });
      await admin.unsafe("CREATE DATABASE khala_sync_ide13_capability_facts");
      await admin.end();
      const result = await runMigrations({
        databaseUrl: pg.urlFor("khala_sync_ide13_capability_facts"),
      });
      expect(result.applied).toContain("0083_portable_command_execution.sql");
      sql = SQL({ url: pg.urlFor("khala_sync_ide13_capability_facts"), max: 2 });

      await sql.begin(async (tx) => {
        await tx`
          INSERT INTO khala_sync_portable_sessions
            (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
             event_log_ref, current_projection_ref, command_scope_ref, root_agent_ref,
             state, current_attachment_ref, current_attachment_generation)
          VALUES
            (${scope.sessionRef}, ${scope.ownerRef}, ${`scope.user.${scope.ownerRef}`},
             'work.ide13.facts', 'eventlog.ide13.facts', 'projection.ide13.facts',
             'commands.ide13.facts', 'agent.ide13.facts', 'active',
             ${scope.sourceAttachmentRef}, ${scope.sourceGeneration})
        `;
        await tx`
          INSERT INTO khala_sync_portable_targets
            (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
             isolation, data_posture, health)
          VALUES
            (${scope.sourceTargetRef}, ${scope.ownerRef}, 'owner_local',
             'adapter.ide13.facts.source', 'compat.ide13.facts',
             'owner_host_process', 'owner_device_only', 'ready'),
            (${scope.destinationTargetRef}, ${scope.ownerRef}, 'openagents_managed',
             'adapter.ide13.facts.destination', 'compat.ide13.facts',
             'dedicated_microvm', 'openagents_managed_region', 'ready')
        `;
        await tx`
          INSERT INTO khala_sync_portable_attachments
            (attachment_ref, session_ref, target_ref, generation, state,
             descendant_agent_refs_json, capability_lease_refs_json, evidence_refs_json)
          VALUES
            (${scope.sourceAttachmentRef}, ${scope.sessionRef}, ${scope.sourceTargetRef},
             ${scope.sourceGeneration}, 'active', '[]'::jsonb,
             ${JSON.stringify(scope.sourceLeaseRefs)}::jsonb, '[]'::jsonb)
        `;
        await tx`
          INSERT INTO khala_sync_portable_commands
            (command_ref, idempotency_key, owner_user_id, session_ref, kind,
             expected_attachment_ref, expected_generation, destination_target_ref,
             expires_at, command_json, status)
          VALUES
            ('command.ide13.facts', 'idempotency.ide13.facts', ${scope.ownerRef},
             ${scope.sessionRef}, 'move', ${scope.sourceAttachmentRef},
             ${scope.sourceGeneration}, ${scope.destinationTargetRef},
             ${scope.commandLeaseExpiresAt}, '{}'::jsonb, 'accepted')
        `;
        await tx`
          INSERT INTO khala_sync_portable_capability_brokers
            (owner_user_id, session_ref, revision, state_json)
          VALUES (${scope.ownerRef}, ${scope.sessionRef}, 1, ${JSON.stringify(state)}::jsonb)
        `;
        await tx`
          INSERT INTO khala_sync_portable_command_executions
            (command_ref, claim_ref, owner_user_id, session_ref, command_kind,
             command_fingerprint, claim_fingerprint, source_attachment_ref,
             source_generation, destination_target_ref, executor_environment_ref,
             worker_instance_ref, state, claimed_at, lease_expires_at, updated_at)
          VALUES
            ('command.ide13.facts', ${scope.commandExecutionClaimRef}, ${scope.ownerRef},
             ${scope.sessionRef}, 'move', ${`sha256:${"a".repeat(64)}`},
             ${`sha256:${"b".repeat(64)}`}, ${scope.sourceAttachmentRef},
             ${scope.sourceGeneration}, ${scope.destinationTargetRef},
             ${scope.sourceTargetRef}, 'worker.ide13.facts', 'claimed',
             '2026-07-20T12:00:00.000Z', ${scope.commandLeaseExpiresAt},
             '2026-07-20T12:00:00.000Z')
        `;
      });
    });

    afterAll(async () => {
      if (sql !== undefined) await sql.end();
      if (pg !== undefined) await pg.stop();
    });

    it("resolves through the command execution table created by migrations", async () => {
      const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
        sql: sql as unknown as SyncSql,
        authority: {
          resolve: async () => [
            {
              grantRef: "grant.ide13.source",
              ownerUserId: scope.ownerRef,
              kind: "provider",
              providerAccountRef: "provider-account.ide13",
              status: "issued",
              expiresAt: "2026-07-20T12:09:00.000Z",
            },
          ],
        },
        now: () => "2026-07-20T12:00:00.000Z",
      });

      await expect(resolver.resolve(scope)).resolves.toMatchObject({
        facts: [
          {
            sourceLeaseRef: "lease.ide13.source",
            expiresAt: scope.commandLeaseExpiresAt,
          },
        ],
      });
    });
  },
);
