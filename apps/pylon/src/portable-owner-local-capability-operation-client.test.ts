import { expect, test } from "vite-plus/test";

import { makePylonPortableOwnerLocalCapabilityOperationClient } from "./portable-owner-local-capability-operation-client.js";

test("polls the exact scoped capability-operation route", async () => {
  const calls: Array<{ url: string; authorization: string | null }> = [];
  const request = {
    schema: "openagents.portable_owner_local_capability_operation.v1",
    operationRef: "operation.ide13.client.install",
    action: "install",
    capability: "provider",
    commandExecutionClaimRef: "claim.ide13.client.command",
    ownerRef: "owner.ide13.client",
    pylonRef: "pylon.ide13.client",
    sessionRef: "session.ide13.client",
    attachmentRef: "attachment.ide13.client",
    attachmentGeneration: 1,
    targetRef: "target.ide13.client",
    sourceLeaseRef: "lease.ide13.client.source",
    sourceGrantRef: "grant.ide13.client.source",
    destinationLeaseRef: "lease.ide13.client.destination",
    destinationGrantRef: "grant.ide13.client.destination",
    installationRef: null,
    permissionRefs: ["permission.ide13.client"],
    permissionFingerprint: `sha256:${"2".repeat(64)}`,
    expiresAt: "2026-07-20T12:10:00.000Z",
  } as const;
  const record = {
    request,
    requestFingerprint: `sha256:${"1".repeat(64)}`,
    state: "pending",
    claimRef: null,
    claimFingerprint: null,
    workerInstanceRef: null,
    claimGeneration: null,
    leaseRevision: null,
    claimedAt: null,
    leaseExpiresAt: null,
    resultRef: null,
    resultFingerprint: null,
    resultStatus: null,
    receiptRef: null,
    resultEvidenceRefs: [],
    errorRef: null,
    completedAt: null,
    updatedAt: "2026-07-20T12:00:00.000Z",
  } as const;
  const client = makePylonPortableOwnerLocalCapabilityOperationClient({
    agentToken: "private-test-token",
    baseUrl: "https://openagents.example",
    pylonRef: request.pylonRef,
    targetRef: request.targetRef,
    fetchImpl: async (input, init) => {
      const headers = new Headers(init?.headers);
      calls.push({ url: String(input), authorization: headers.get("authorization") });
      return new Response(
        JSON.stringify({
          schema: "openagents.portable_owner_local_capability_operation_transport.v1",
          operations: [record],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    },
  });

  expect(await client.pending(4)).toEqual([record]);
  expect(calls).toEqual([
    {
      url: "https://openagents.example/api/pylons/pylon.ide13.client/portable-targets/target.ide13.client/capability-operations?limit=4",
      authorization: "Bearer private-test-token",
    },
  ]);
});
