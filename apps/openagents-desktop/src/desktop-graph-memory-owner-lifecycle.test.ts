import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { runGraphMemoryOwnerLifecycleProof } from "./desktop-graph-memory-owner-lifecycle.js";
import type { SafeStorageLike } from "./desktop-session-vault.js";

const roots: Array<string> = [];
const temporaryRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-owner-lifecycle-"));
  roots.push(root);
  return root;
};

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

const fakeSafeStorage = (): SafeStorageLike => ({
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => "keychain_access",
  encryptString: (plaintext) => Buffer.from(`test-wrapped:${plaintext}`, "utf8"),
  decryptString: (encrypted) => encrypted.toString("utf8").slice("test-wrapped:".length),
});

describe("graph-memory owner lifecycle proof", () => {
  test("proves restart, exact archive validation, idempotent forget, and archive cleanup", async () => {
    const root = temporaryRoot();
    const databasePath = path.join(root, "private", "graph-memory.sqlite");
    const archivePath = path.join(root, "private", "owner-export.graph.json");
    const result = await Effect.runPromise(
      runGraphMemoryOwnerLifecycleProof({
        databasePath,
        archivePath,
        safeStorage: fakeSafeStorage(),
        custodyRung: "test_fake_safe_storage",
      }),
    );

    expect(result).toMatchObject({
      schemaId: "openagents.desktop.graph_memory_owner_lifecycle_proof.v1",
      custodyRung: "test_fake_safe_storage",
      reopenCount: 3,
      put: {
        counts: {
          mentions: 2,
          entities: 2,
          relations: 1,
          vectors: 1,
          summaries: 1,
          rankingRefs: 1,
          rankingSnapshots: 1,
        },
      },
      before: {
        pendingOperationRef: null,
        counts: { vectors: 1, summaries: 1, rankingRefs: 1, rankingSnapshots: 1 },
      },
      incompletePlanRefusal: {
        status: "refused",
        graphDigestUnchanged: true,
      },
      exported: {
        exactSdkValidation: true,
        counts: {
          mentions: 2,
          entities: 2,
          relations: 1,
          vectors: 1,
          summaries: 1,
          rankingRefs: 1,
          rankingSnapshots: 1,
          archives: 1,
        },
      },
      forgotten: {
        before: {
          mentions: 2,
          entities: 2,
          relations: 1,
          vectors: 1,
          summaries: 1,
          rankingRefs: 1,
          rankingSnapshots: 1,
          archives: 1,
        },
        after: {
          mentions: 0,
          entities: 0,
          relations: 0,
          vectors: 0,
          summaries: 0,
          rankingRefs: 0,
          rankingSnapshots: 0,
          archives: 0,
        },
      },
      after: { currentAbsent: true },
      repeated: { sameReceipt: true },
      archiveCleanup: { retainedThroughForget: true, removed: true },
    });
    expect(result.repeated.receiptRef).toBe(result.forgotten.receiptRef);
    expect(result.repeated.receiptDigest).toBe(result.forgotten.receiptDigest);
    expect(result.incompletePlanRefusal.unresolvedCount).toBeGreaterThan(0);
    expect(result.exported.graphDigest).toBe(result.before.graphDigest);
    expect(result.exported.graphManifestDigest).toBe(result.before.graphManifestDigest);
    expect(result.exported.encodedBytes).toBeGreaterThan(0);
    expect(result.before.rankingSnapshotDigests).toHaveLength(1);
    expect(result.before.provenanceRefs.length).toBeGreaterThan(0);
    expect(result.before.provenanceDigest).toMatch(/^[a-f0-9]{64}$/u);
    expect(result.before.sourceMembershipRefs).toHaveLength(5);
    expect(result.before.sourceMembershipDigest).toMatch(/^[a-f0-9]{64}$/u);
    expect(existsSync(archivePath)).toBe(false);
    expect(existsSync(databasePath)).toBe(true);

    const aggregate = JSON.stringify(result);
    expect(aggregate).not.toContain("PRIVATE_SENTINEL");
    expect(aggregate).not.toContain("owner-memory://");
    expect(aggregate).not.toContain(root);
    expect(readFileSync(databasePath).toString("utf8")).not.toContain("PRIVATE_SENTINEL");
  });

  test("records a separate incomplete-plan refusal without changing the graph", async () => {
    const root = temporaryRoot();
    const result = await Effect.runPromise(
      runGraphMemoryOwnerLifecycleProof({
        databasePath: path.join(root, "graph-memory.sqlite"),
        archivePath: path.join(root, "owner-export.graph.json"),
        safeStorage: fakeSafeStorage(),
        custodyRung: "test_fake_safe_storage",
        ownerRef: "owner.incomplete-plan-proof",
        projectRef: "project.incomplete-plan-proof",
      }),
    );

    expect(result.incompletePlanRefusal).toMatchObject({
      status: "refused",
      graphDigestUnchanged: true,
    });
    expect(result.incompletePlanRefusal.unresolvedCount).toBeGreaterThan(0);
    expect(result.before.graphDigest).toBe(result.exported.graphDigest);
    expect(result.forgotten.before).toEqual(result.exported.counts);
  });
});
