import type { SecretMaterial } from "@openagentsinc/portable-session-contract";
import { describe, expect, test } from "vite-plus/test";

import {
  makePylonOwnerLocalCapabilityTransportHandler,
  pylonOwnerLocalCapabilityOperationRef,
} from "../src/portable-owner-local-capability-transport.js";

const authority = {
  commandExecutionClaimRef: "claim.ide13.pylon-capability",
  ownerRef: "owner.ide13.pylon-capability",
  pylonRef: "pylon.ide13.pylon-capability",
  sessionRef: "session.ide13.pylon-capability",
  attachmentRef: "attachment.ide13.pylon-capability",
  attachmentGeneration: 2,
  targetRef: "target.ide13.pylon-capability",
} as const;
const leaseRef = "lease.ide13.pylon-capability";
const permissions = ["provider.turn.execute"];

const request = (token: string, operationRef: string, body: Uint8Array) => new Request(
  "https://pylon.example/v1/portable-owner-local-capabilities/install",
  { method: "POST", headers: { authorization: `Bearer ${token}`, "idempotency-key": operationRef,
    "x-openagents-operation-ref": operationRef,
    "x-openagents-command-claim-ref": authority.commandExecutionClaimRef,
    "x-openagents-owner-ref": authority.ownerRef, "x-openagents-pylon-ref": authority.pylonRef,
    "x-openagents-session-ref": authority.sessionRef, "x-openagents-attachment-ref": authority.attachmentRef,
    "x-openagents-attachment-generation": String(authority.attachmentGeneration),
    "x-openagents-target-ref": authority.targetRef, "x-openagents-lease-ref": leaseRef,
    "x-openagents-capability": "provider", "x-openagents-expires-at": "2026-07-20T13:00:00.000Z",
    "x-openagents-permissions": permissions.join(","), "content-type": "application/octet-stream",
    "content-length": String(body.byteLength) }, body: Buffer.from(body) },
);

describe("Pylon owner-local capability transport", () => {
  test("authorizes before custody and zeroes transient request material", async () => {
    let observed: SecretMaterial | undefined;
    let calls = 0;
    const handler = makePylonOwnerLocalCapabilityTransportHandler({
      bearerToken: "private-pylon-bearer-ide13",
      authorize: async input => JSON.stringify(input) === JSON.stringify(authority),
      port: {
        install: async input => { calls += 1; observed = input.material; return {
          installationRef: "installation.ide13.pylon-capability",
          evidenceRef: "evidence.ide13.pylon-capability",
        }; },
        wipe: async () => ({ wipeReceiptRef: "receipt.ide13.pylon-capability" }),
      },
    });
    const operationRef = pylonOwnerLocalCapabilityOperationRef({ action: "install", authority,
      leaseRef, permissions });
    const denied = await handler(request("wrong-private-token", operationRef, new Uint8Array([9])));
    expect(denied.status).toBe(401);
    expect(calls).toBe(0);
    const response = await handler(request("private-pylon-bearer-ide13", operationRef,
      new TextEncoder().encode("private material")));
    expect(response.status).toBe(200);
    const responseBody = await response.json();
    expect(responseBody).toMatchObject({ material: "excluded", operationRef });
    expect(JSON.stringify(responseBody)).not.toContain("private material");
    expect(calls).toBe(1);
    expect(observed).toBeDefined();
    expect([...observed!].every(byte => byte === 0)).toBe(true);
  });
});
