import { PORTABLE_CAPABILITY_BROKER_VERSION } from "@openagentsinc/portable-session-contract";
import { describe, expect, it } from "vite-plus/test";

import {
  HttpPortableCapabilityGrantFactAuthority,
  PostgresPortableCommandCapabilityGrantFactResolver,
} from "./portable-command-capability-grant-resolver.js";
import type { PortableCommandCapabilityGrantFactScope } from "./portable-session-command-runner.js";
import type { SyncSql } from "./sql.js";

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
  records: [{
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
  }],
  operations: [],
  evidence: [],
  material: "excluded" as const,
};

const sqlWith = (stateJson: unknown): SyncSql => Object.assign(
  async () => [{
    state_json: stateJson,
    claim_command_ref: "command.ide13.facts",
    active_move_ref: null,
    active_command_ref: null,
    active_source_attachment_ref: null,
    active_source_generation: null,
    active_destination_target_ref: null,
  }],
  { begin: async () => { throw new Error("transaction is not expected"); } },
) as SyncSql;

describe("portable command capability grant facts", () => {
  it("uses the service-authenticated refs-only fact route", async () => {
    let seen: RequestInit | undefined;
    const authority = new HttpPortableCapabilityGrantFactAuthority({
      baseUrl: "https://openagents.example",
      serviceBearer: "service.ide13.facts",
      fetch: async (input, init) => {
        expect(new URL(input instanceof Request ? input.url : input.toString()).pathname)
          .toBe("/api/portable-capability-grants/facts");
        seen = init;
        return Response.json({
          facts: [{
            grantRef: "grant.ide13.source", ownerUserId: scope.ownerRef,
            kind: "github", status: "issued", expiresAt: "2026-07-20T12:09:00.000Z",
          }],
          material: "excluded",
        });
      },
    });
    await expect(authority.resolve({
      ownerUserId: scope.ownerRef,
      grantRefs: ["grant.ide13.source"],
    })).resolves.toHaveLength(1);
    expect(seen?.headers).toMatchObject({ authorization: "Bearer service.ide13.facts" });
  });

  it("binds the exact broker lease to an active owner grant and clamps expiry", async () => {
    const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
      sql: sqlWith(state),
      authority: { resolve: async () => [{
        grantRef: "grant.ide13.source",
        ownerUserId: scope.ownerRef,
        kind: "provider",
        providerAccountRef: "provider-account.ide13",
        runnerSessionId: "runner-session.ide13",
        status: "issued",
        expiresAt: "2026-07-20T12:09:00.000Z",
      }] },
      now: () => "2026-07-20T12:00:00.000Z",
    });
    const result = await resolver.resolve(scope);
    expect(result.facts).toEqual([{
      sourceLeaseRef: "lease.ide13.source",
      destinationSourceGrantRef: expect.stringMatching(/^grant\.portable\.[a-f0-9]{64}$/u),
      expiresAt: "2026-07-20T12:07:00.000Z",
    }]);
    expect(result.bindings).toEqual([{
      grantRef: "grant.ide13.source",
      ownerUserId: scope.ownerRef,
      kind: "provider",
      providerAccountRef: "provider-account.ide13",
      runnerSessionId: "runner-session.ide13",
    }]);
  });

  it("rejects a broker lease-set mismatch before the grant authority call", async () => {
    let called = false;
    const resolver = new PostgresPortableCommandCapabilityGrantFactResolver({
      sql: sqlWith({ ...state, records: [] }),
      authority: { resolve: async () => { called = true; return []; } },
    });
    await expect(resolver.resolve(scope)).rejects.toThrow(/lease does not match/);
    expect(called).toBe(false);
  });
});
