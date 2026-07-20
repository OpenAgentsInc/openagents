import { mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "vite-plus/test";

import {
  makePylonPortableOwnerLocalCapabilityOperationJournal,
  PylonPortableOwnerLocalCapabilityOperationJournalError,
  type PylonPortableOwnerLocalCapabilityOperationJournalEntry,
} from "./portable-owner-local-capability-operation-journal.js";

const pylonRef = "pylon.ide13.journal";
const targetRef = "target.ide13.journal";
const workerInstanceRef = "worker.ide13.journal";
const now = "2026-07-20T12:00:00.000Z";

const pendingEntry = (): PylonPortableOwnerLocalCapabilityOperationJournalEntry => ({
  record: {
    request: {
      schema: "openagents.portable_owner_local_capability_operation.v1",
      operationRef: "operation.ide13.journal.install",
      action: "install",
      capability: "provider",
      commandExecutionClaimRef: "claim.ide13.journal.command",
      ownerRef: "owner.ide13.journal",
      sessionRef: "session.ide13.journal",
      attachmentRef: "attachment.ide13.journal",
      attachmentGeneration: 1,
      targetRef,
      pylonRef,
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
    updatedAt: now,
  },
  claimRequest: {
    schema: "openagents.portable_owner_local_capability_operation.v1",
    operationRef: "operation.ide13.journal.install",
    claimRef: "claim.ide13.journal.capability",
    sessionRef: "session.ide13.journal",
    attachmentRef: "attachment.ide13.journal",
    attachmentGeneration: 1,
    pylonRef,
    targetRef,
    workerInstanceRef,
    leaseExpiresAt: "2026-07-20T12:01:00.000Z",
  },
  claimGeneration: null,
  leaseRevision: null,
  leaseExpiresAt: null,
  state: "claiming",
  completion: null,
});

test("writes an atomic owner-only refs journal and rejects corrupt or private bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "openagents-capability-journal-"));
  try {
    const journal = makePylonPortableOwnerLocalCapabilityOperationJournal({
      directory,
      pylonRef,
      targetRef,
      workerInstanceRef,
    });
    const entry = pendingEntry();
    await journal.put(entry);
    expect(await journal.entries()).toEqual([entry]);

    const files = await readdir(directory);
    expect(files).toHaveLength(1);
    const path = join(directory, files[0]!);
    expect((await stat(path)).mode & 0o777).toBe(0o600);

    await writeFile(path, '{"secret":"Bearer private-token"}\n', { mode: 0o600 });
    await expect(journal.entries()).rejects.toEqual(
      new PylonPortableOwnerLocalCapabilityOperationJournalError("unsafe_material"),
    );

    await writeFile(path, "not-json\n", { mode: 0o600 });
    await expect(journal.entries()).rejects.toEqual(
      new PylonPortableOwnerLocalCapabilityOperationJournalError("corrupt"),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
