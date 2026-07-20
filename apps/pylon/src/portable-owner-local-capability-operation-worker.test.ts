import { expect, test } from "vite-plus/test";

import type {
  PortableOwnerLocalCapabilityOperationRecord,
  PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/portable-session-contract";

import type { PylonPortableOwnerLocalCapabilityOperationJournalEntry } from "./portable-owner-local-capability-operation-journal.js";
import { PylonPortableOwnerLocalCapabilityWorker } from "./portable-owner-local-capability-operation-worker.js";

const pylonRef = "pylon.ide13.capability";
const targetRef = "target.ide13.capability";
const workerInstanceRef = "worker.ide13.capability";
const now = new Date("2026-07-20T12:00:00.000Z");

const pending = (): PortableOwnerLocalCapabilityOperationRecord => ({
  request: {
    schema: "openagents.portable_owner_local_capability_operation.v1",
    operationRef: "operation.ide13.capability.install",
    action: "install",
    capability: "provider",
    commandExecutionClaimRef: "claim.ide13.command",
    ownerRef: "owner.ide13.capability",
    pylonRef,
    sessionRef: "session.ide13.capability",
    attachmentRef: "attachment.ide13.capability",
    attachmentGeneration: 1,
    targetRef,
    sourceLeaseRef: "lease.ide13.source",
    sourceGrantRef: "grant.ide13.source",
    destinationLeaseRef: "lease.ide13.destination",
    destinationGrantRef: "grant.ide13.destination",
    installationRef: null,
    permissionRefs: ["permission.ide13.capability"],
    permissionFingerprint: `sha256:${"2".repeat(64)}`,
    expiresAt: "2026-07-20T12:10:00.000Z",
  },
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
  resultInstallationRef: null,
  receiptRef: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: now.toISOString(),
});

test("completes with refs only and zeroizes executor buffers", async () => {
  const entries = new Map<string, PylonPortableOwnerLocalCapabilityOperationJournalEntry>();
  const journalSnapshots: Array<string> = [];
  const raw = new TextEncoder().encode("Bearer private-capability-material");
  let server = pending();
  let completion: PortableOwnerLocalCapabilityOperationResultRequest | undefined;

  const worker = new PylonPortableOwnerLocalCapabilityWorker({
    pylonRef,
    targetRef,
    workerInstanceRef,
    now: () => now,
    client: {
      pending: async () => [server],
      read: async () => server,
      claim: async (request) => {
        server = {
          ...server,
          state: "claimed",
          claimRef: request.claimRef,
          workerInstanceRef,
          claimGeneration: 1,
          leaseRevision: 1,
          claimedAt: now.toISOString(),
          leaseExpiresAt: request.leaseExpiresAt,
        };
        return { operation: server, status: "claimed" };
      },
      renew: async () => ({ operation: server, status: "replayed" }),
      complete: async (request) => {
        completion = request;
        server = {
          ...server,
          state: request.resultStatus,
          leaseRevision: request.expectedLeaseRevision + 1,
          resultRef: request.resultRef,
          resultStatus: request.resultStatus,
          resultInstallationRef: request.resultInstallationRef,
          receiptRef: request.receiptRef,
          resultEvidenceRefs: [...request.evidenceRefs],
          errorRef: request.errorRef,
          completedAt: request.completedAt,
        };
        return { operation: server, status: request.resultStatus };
      },
    },
    journal: {
      entries: async () => [...entries.values()],
      put: async (entry) => {
        const encoded = JSON.stringify(entry);
        journalSnapshots.push(encoded);
        entries.set(entry.record.request.operationRef, entry);
      },
      remove: async (operationRef) => {
        entries.delete(operationRef);
      },
    },
    executor: {
      recoverySemantics: async () => "operation_ref_idempotent",
      execute: async (request, claim) => {
        expect(request.capability).toBe("provider");
        expect(request.installationRef).toBeNull();
        expect(claim.expectedLeaseRevision).toBe(1);
        return {
          outcome: {
            status: "completed",
            resultInstallationRef: "installation.ide13.capability.install",
            receiptRef: "receipt.ide13.capability.install",
            evidenceRefs: ["evidence.ide13.capability.install"],
            errorRef: null,
          },
          privateBuffers: [raw],
        };
      },
    },
  });

  expect(await worker.runPass()).toBe(1);
  expect([...raw]).toEqual(Array.from({ length: raw.length }, () => 0));
  expect(completion).toMatchObject({
    resultStatus: "completed",
    resultInstallationRef: "installation.ide13.capability.install",
    receiptRef: "receipt.ide13.capability.install",
    evidenceRefs: ["evidence.ide13.capability.install"],
    errorRef: null,
  });
  const persistedAndReturned = JSON.stringify({ journalSnapshots, completion });
  expect(persistedAndReturned).not.toContain("private-capability-material");
  expect(persistedAndReturned).not.toContain("privateBuffers");
  expect(entries.size).toBe(0);
});

test("reconciles an exact terminal result after a lost completion acknowledgement", async () => {
  const pendingRecord = pending();
  const requestRecord: PortableOwnerLocalCapabilityOperationRecord = {
    ...pendingRecord,
    request: {
      ...pendingRecord.request,
      capability: "tool",
      executableProfileRef: "profile.ide13.lsp.lost-ack.v1",
    },
  };
  const completion: PortableOwnerLocalCapabilityOperationResultRequest = {
    schema: "openagents.portable_owner_local_capability_operation.v1",
    claimRef: "claim.ide13.capability.worker",
    pylonRef,
    targetRef,
    sessionRef: requestRecord.request.sessionRef,
    attachmentRef: requestRecord.request.attachmentRef,
    attachmentGeneration: 1,
    workerInstanceRef,
    claimGeneration: 1,
    expectedLeaseRevision: 1,
    resultRef: "result.ide13.capability.install",
    resultStatus: "completed",
    resultInstallationRef: "installation.ide13.capability.install",
    executableProfileRef: requestRecord.request.executableProfileRef,
    receiptRef: "receipt.ide13.capability.install",
    evidenceRefs: ["evidence.ide13.capability.install"],
    errorRef: null,
    completedAt: now.toISOString(),
  };
  const claimed = {
    ...requestRecord,
    state: "claimed" as const,
    claimRef: completion.claimRef,
    workerInstanceRef,
    claimGeneration: 1,
    leaseRevision: 1,
    claimedAt: now.toISOString(),
    leaseExpiresAt: "2026-07-20T12:01:00.000Z",
  };
  const terminal: PortableOwnerLocalCapabilityOperationRecord = {
    ...claimed,
    state: "completed",
    leaseRevision: 2,
    resultRef: completion.resultRef,
    resultStatus: completion.resultStatus,
    resultInstallationRef: completion.resultInstallationRef,
    receiptRef: completion.receiptRef,
    resultEvidenceRefs: [...completion.evidenceRefs],
    completedAt: completion.completedAt,
  };
  let removed = false;
  const worker = new PylonPortableOwnerLocalCapabilityWorker({
    pylonRef,
    targetRef,
    workerInstanceRef,
    now: () => now,
    client: {
      pending: async () => [],
      read: async () => terminal,
      claim: async () => {
        throw new Error("unexpected claim");
      },
      renew: async () => {
        throw new Error("unexpected renewal");
      },
      complete: async () => {
        throw new Error("unexpected replay");
      },
    },
    journal: {
      entries: async () => [
        {
          record: claimed,
          claimRequest: {
            schema: "openagents.portable_owner_local_capability_operation.v1",
            operationRef: claimed.request.operationRef,
            claimRef: completion.claimRef,
            pylonRef,
            targetRef,
            sessionRef: claimed.request.sessionRef,
            attachmentRef: claimed.request.attachmentRef,
            attachmentGeneration: 1,
            workerInstanceRef,
            leaseExpiresAt: claimed.leaseExpiresAt,
          },
          claimGeneration: 1,
          leaseRevision: 1,
          leaseExpiresAt: claimed.leaseExpiresAt,
          state: "completion_pending",
          completion,
        },
      ],
      put: async () => {
        throw new Error("unexpected journal write");
      },
      remove: async () => {
        removed = true;
      },
    },
    executor: {
      recoverySemantics: async () => "not_proven",
      execute: async () => {
        throw new Error("unexpected execution");
      },
    },
  });

  expect(await worker.runPass()).toBe(0);
  expect(removed).toBe(true);
});
