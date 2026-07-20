import type {
  PortableCommandExecutionClaim,
  PortablePhaseOperationRecord,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vitest";

import type { GoogleCloudKmsDekClient } from "./google-cloud-kms";
import {
  makePortableCheckpointDekRoutes,
  type PortableCheckpointDekCurrentAuthority,
  type PortableCheckpointDekWrapBinding,
} from "./portable-checkpoint-dek-routes";

const pylonRef = "pylon.ide13.kms.1";
const targetRef = "target.ide13.source.1";
const operationRef = "operation.ide13.checkpoint-create.1";
const commandClaimRef = "claim.ide13.command.1";
const phaseClaimRef = "claim.ide13.phase.1";
const leaseExpiresAt = "2026-07-20T15:05:00.000Z";
const objectRef = "checkpoint-custody:ide13-object-1";
const keyRef = "key.portable-checkpoint.production.v1";
const basePath = `/api/pylons/${pylonRef}/portable-targets/${targetRef}/checkpoint-deks/${operationRef}`;

const command: PortableCommandExecutionClaim = {
  schema: "openagents.portable_command_execution.v1",
  claimRef: commandClaimRef,
  commandRef: "command.ide13.1",
  ownerRef: "owner.ide13.1",
  sessionRef: "session.ide13.1",
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.source.1",
  sourceGeneration: 1,
  destinationTargetRef: "target.ide13.destination.1",
  executorEnvironmentRef: targetRef,
  workerInstanceRef: "worker.ide13.command.1",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: "2026-07-20T14:59:00.000Z",
  leaseExpiresAt: "2026-07-20T15:10:00.000Z",
  updatedAt: "2026-07-20T14:59:00.000Z",
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const phase = (
  state: PortablePhaseOperationRecord["state"] = "claimed",
): PortablePhaseOperationRecord => ({
  request: {
    schema: "openagents.portable_phase_operation.v1",
    operationRef,
    commandRef: command.commandRef,
    commandExecutionClaimRef: commandClaimRef,
    ownerRef: command.ownerRef,
    sessionRef: command.sessionRef,
    attachmentRef: command.sourceAttachmentRef,
    attachmentGeneration: command.sourceGeneration,
    targetRef,
    pylonRef,
    kind: "checkpoint-create",
    checkpointRef: "checkpoint.ide13.1",
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: [],
    expiresAt: leaseExpiresAt,
  },
  requestFingerprint: `sha256:${"3".repeat(64)}`,
  state,
  claimRef: phaseClaimRef,
  claimFingerprint: `sha256:${"4".repeat(64)}`,
  workerInstanceRef: "worker.ide13.phase.1",
  claimGeneration: 1,
  leaseRevision: 1,
  claimedAt: "2026-07-20T14:59:00.000Z",
  leaseExpiresAt,
  resultRef: null,
  resultFingerprint: null,
  resultStatus: null,
  resultCheckpointRef: null,
  resultCheckpointObjectRef: null,
  resultCheckpointDigest: null,
  resultCheckpointManifestDigest: null,
  resultDestinationRunnerSessionReservationRef: null,
  resultDestinationActivationReceipt: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: "2026-07-20T14:59:00.000Z",
});

const current = (state: PortablePhaseOperationRecord["state"] = "claimed"):
  PortableCheckpointDekCurrentAuthority => ({ operation: phase(state), commandClaim: command });

const binding: PortableCheckpointDekWrapBinding = {
  schema: "openagents.portable_checkpoint_dek_wrap_binding.v1",
  algorithm: "aes-256-gcm+google-kms-wrapped-dek",
  policy: "openagents_managed",
  operationRef,
  commandExecutionClaimRef: commandClaimRef,
  ownerRef: command.ownerRef,
  pylonRef,
  targetRef,
  sessionRef: command.sessionRef,
  attachmentRef: command.sourceAttachmentRef,
  attachmentGeneration: command.sourceGeneration,
  objectRef,
  keyRef,
};

const unwrapCurrent = (): PortableCheckpointDekCurrentAuthority => ({
  commandClaim: command,
  operation: {
    ...phase(),
    request: {
      ...phase().request,
      operationRef: "operation.ide13.checkpoint-stage.1",
      attachmentRef: "attachment.ide13.destination.1",
      attachmentGeneration: 2,
      targetRef: command.destinationTargetRef,
      kind: "checkpoint-stage",
      checkpointObjectRef: objectRef,
      checkpointDigest: `sha256:${"5".repeat(64)}`,
    },
  },
});

const headers = (): Record<string, string> => ({
  authorization: "Bearer oa_agent_test",
  "content-type": "application/octet-stream",
  "x-openagents-command-claim-ref": commandClaimRef,
  "x-openagents-phase-claim-ref": phaseClaimRef,
  "x-openagents-session-ref": command.sessionRef,
  "x-openagents-attachment-ref": command.sourceAttachmentRef,
  "x-openagents-attachment-generation": String(command.sourceGeneration),
  "x-openagents-worker-instance-ref": "worker.ide13.phase.1",
  "x-openagents-claim-generation": "1",
  "x-openagents-lease-revision": "1",
  "x-openagents-lease-expires-at": leaseExpiresAt,
  "x-openagents-object-ref": objectRef,
  "x-openagents-key-ref": keyRef,
});

const setup = (options: Readonly<{
  authenticated?: boolean;
  registeredOwner?: string;
  registeredOwners?: ReadonlyArray<string | undefined>;
  authorities?: ReadonlyArray<PortableCheckpointDekCurrentAuthority>;
  bindings?: ReadonlyArray<PortableCheckpointDekWrapBinding>;
  kms?: GoogleCloudKmsDekClient;
  configuredKeyRef?: string | undefined;
}> = {}) => {
  let authorityIndex = 0;
  let bindingIndex = 0;
  let registrationIndex = 0;
  const kms = options.kms ?? {
    wrapDek: async () => new Uint8Array([8, 9]),
    unwrapDek: async () => new Uint8Array(32).fill(7),
  };
  const routes = makePortableCheckpointDekRoutes({
    authenticate: async () =>
      options.authenticated === false
        ? undefined
        : { agentUserId: "agent.ide13.1", ownerUserId: command.ownerRef },
    readPylonOwnerAgentUserId: async () =>
      options.registeredOwners?.[registrationIndex++] ??
      options.registeredOwner ??
      "agent.ide13.1",
    resolveExactTarget: async () => "ready",
    readAuthority: async () =>
      options.authorities?.[authorityIndex++] ?? current(),
    resolveWrapBinding: async () =>
      options.bindings?.[bindingIndex++] ?? binding,
    kmsClient: () => kms,
    configuredKeyRef: () =>
      options.configuredKeyRef === undefined ? keyRef : options.configuredKeyRef,
    now: () => new Date("2026-07-20T15:00:00.000Z"),
  });
  return routes.routePortableCheckpointDekRequest;
};

const request = (body: Uint8Array, overrides: HeadersInit = headers()): Request =>
  new Request(`https://openagents.com${basePath}/wrap`, {
    method: "POST",
    headers: overrides,
    body: Uint8Array.from(body).buffer,
  });

describe("portable checkpoint DEK routes", () => {
  test("wraps exactly 32 bytes and returns one-shot binary output", async () => {
    let inputCopy: Uint8Array | undefined;
    const route = setup({
      kms: {
        wrapDek: async input => {
          inputCopy = input.slice();
          return new Uint8Array([5, 6, 7]);
        },
        unwrapDek: vi.fn(),
      },
    });
    const response = await route(request(new Uint8Array(32).fill(3)), {});
    expect(response?.status).toBe(200);
    expect(response?.headers.get("content-type")).toBe("application/octet-stream");
    expect(response?.headers.get("cache-control")).toBe("no-store");
    expect(inputCopy).toEqual(new Uint8Array(32).fill(3));
    expect(new Uint8Array(await response!.arrayBuffer())).toEqual(new Uint8Array([5, 6, 7]));
  });

  test("unwraps bounded ciphertext to exactly 32 private bytes", async () => {
    let wrappedCopy: Uint8Array | undefined;
    const route = setup({
      authorities: [unwrapCurrent(), unwrapCurrent()],
      kms: {
        wrapDek: vi.fn(),
        unwrapDek: async wrapped => {
          wrappedCopy = wrapped.slice();
          return new Uint8Array(32).fill(6);
        },
      },
    });
    const unwrapHeaders = {
      ...headers(),
      "x-openagents-phase-claim-ref": phaseClaimRef,
      "x-openagents-attachment-ref": "attachment.ide13.destination.1",
      "x-openagents-attachment-generation": "2",
    };
    const response = await route(
      new Request(
        `https://openagents.com/api/pylons/${pylonRef}/portable-targets/${command.destinationTargetRef}/checkpoint-deks/operation.ide13.checkpoint-stage.1/unwrap`,
        {
          method: "POST",
          headers: unwrapHeaders,
          body: Uint8Array.from([1, 2, 3]).buffer,
        },
      ),
      {},
    );
    expect(response?.status).toBe(200);
    expect(wrappedCopy).toEqual(new Uint8Array([1, 2, 3]));
    expect(new Uint8Array(await response!.arrayBuffer())).toEqual(new Uint8Array(32).fill(6));
  });

  test("requires the authenticated exact Pylon owner", async () => {
    expect((await setup({ authenticated: false })(request(new Uint8Array(32)), {}))?.status).toBe(401);
    expect(
      (await setup({ registeredOwner: "agent.ide13.other" })(request(new Uint8Array(32)), {}))
        ?.status,
    ).toBe(403);
  });

  test("rejects an invalid DEK size before KMS", async () => {
    const wrapDek = vi.fn(async () => new Uint8Array([1]));
    const route = setup({ kms: { wrapDek, unwrapDek: vi.fn() } });
    const response = await route(request(new Uint8Array(31)), {});
    expect(response?.status).toBe(400);
    expect(wrapDek).not.toHaveBeenCalled();

    const wrongContentType = await route(
      request(new Uint8Array(32), { ...headers(), "content-type": "application/json" }),
      {},
    );
    expect(wrongContentType?.status).toBe(400);
    expect(wrapDek).not.toHaveBeenCalled();
  });

  test("zeroes KMS input when KMS refuses", async () => {
    let captured: Uint8Array | undefined;
    const route = setup({
      kms: {
        wrapDek: async input => {
          captured = input;
          throw new Error("kms unavailable");
        },
        unwrapDek: vi.fn(),
      },
    });
    expect((await route(request(new Uint8Array(32).fill(4)), {}))?.status).toBe(503);
    expect(captured?.every(byte => byte === 0)).toBe(true);
  });

  test("zeroes and refuses KMS output after final authority drift", async () => {
    const output = new Uint8Array([7, 8, 9]);
    const route = setup({
      authorities: [current(), current("completed")],
      kms: { wrapDek: async () => output, unwrapDek: vi.fn() },
    });
    const response = await route(request(new Uint8Array(32).fill(4)), {});
    expect(response?.status).toBe(409);
    expect(output.every(byte => byte === 0)).toBe(true);
  });

  test("zeroes and refuses an oversized wrapped result", async () => {
    const output = new Uint8Array(128 * 1024 + 1).fill(7);
    const route = setup({
      kms: { wrapDek: async () => output, unwrapDek: vi.fn() },
    });
    const response = await route(request(new Uint8Array(32).fill(4)), {});
    expect(response?.status).toBe(409);
    expect(output.every(byte => byte === 0)).toBe(true);
  });

  test("zeroes and refuses KMS output after authenticated Pylon ownership drifts", async () => {
    const output = new Uint8Array([7, 8, 9]);
    const route = setup({
      registeredOwners: ["agent.ide13.1", "agent.ide13.replaced"],
      kms: { wrapDek: async () => output, unwrapDek: vi.fn() },
    });
    const response = await route(request(new Uint8Array(32).fill(4)), {});
    expect(response?.status).toBe(409);
    expect(output.every(byte => byte === 0)).toBe(true);
  });

  test("is unavailable when the key ref or KMS identity configuration is absent", async () => {
    const route = makePortableCheckpointDekRoutes({
      authenticate: async () => ({ agentUserId: "agent.ide13.1", ownerUserId: command.ownerRef }),
      readPylonOwnerAgentUserId: async () => "agent.ide13.1",
      resolveExactTarget: async () => "ready",
      readAuthority: async () => current(),
      resolveWrapBinding: async () => binding,
      kmsClient: () => undefined,
      configuredKeyRef: () => undefined,
    }).routePortableCheckpointDekRequest;
    expect((await route(request(new Uint8Array(32)), {}))?.status).toBe(503);
  });
});
