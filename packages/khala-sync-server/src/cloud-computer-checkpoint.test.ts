import { describe, expect, test } from "vite-plus/test";

import {
  CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA,
  CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS,
  CloudComputerCheckpointError,
  assertCloudComputerWorkspaceKeyScope,
  checkpointEncryptionBindingDigest,
  checkpointObjectRef,
  checkpointPathAdmitted,
  cloudComputerWorkspaceKeyAuthorizer,
  createCloudComputerCheckpointDeleteEvidence,
  createCloudComputerCheckpointManifest,
  downloadCloudComputerCheckpoint,
  planCloudComputerCheckpointGc,
  sha256Digest,
  uploadCloudComputerCheckpoint,
  verifyCloudComputerCheckpointManifest,
  type CloudComputerCheckpointGcs,
  type CloudComputerCheckpointManifest,
  type CloudComputerCheckpointManifestInput,
  type CloudComputerGcsObject,
  type CloudComputerGcsUploadState,
} from "./cloud-computer-checkpoint.js";

const ciphertext = new TextEncoder().encode("encrypted-checkpoint-fixture");

const manifestFixture = (
  overrides: Partial<CloudComputerCheckpointManifest> = {},
): CloudComputerCheckpointManifest => {
  const draft: CloudComputerCheckpointManifestInput = {
    schema: CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA,
    checkpointRef: "checkpoint.computer05.1",
    idempotencyRef: "operation.computer05.checkpoint.1",
    ownerRef: "owner.alice",
    tenantRef: "tenant.acme",
    workspaceRef: "workspace.computer05",
    workspaceGeneration: 3,
    checkpointKind: "delta" as const,
    parentCheckpointDigest: `sha256:${"1".repeat(64)}`,
    baseImageDigest: `sha256:${"2".repeat(64)}`,
    workspaceKeyRef: "kms.workspace.computer05",
    workspaceKeyVersion: 7,
    encryption: "AES-256-GCM",
    entries: [
      {
        path: ".git/HEAD",
        kind: "file",
        classification: "git_metadata",
        mode: 0o100644,
        byteCount: 4,
        contentDigest: sha256Digest("main"),
        linkTarget: null,
      },
      {
        path: "src",
        kind: "directory",
        classification: "workspace",
        mode: 0o40755,
        byteCount: 0,
        contentDigest: null,
        linkTarget: null,
      },
      {
        path: "src/index.ts",
        kind: "file",
        classification: "workspace",
        mode: 0o100644,
        byteCount: 6,
        contentDigest: sha256Digest("export"),
        linkTarget: null,
      },
    ],
    deletions: ["obsolete.txt"],
    excludedPaths: [...CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS],
    plaintextByteCount: 10,
    contentManifestDigest: `sha256:${"0".repeat(64)}` as const,
    encryptedByteCount: ciphertext.byteLength,
    ciphertextDigest: sha256Digest(ciphertext),
    createdAt: "2026-08-22T15:00:00.000Z",
    retention: {
      retainUntil: "2026-08-29T15:00:00.000Z",
      orphanGraceUntil: "2026-08-22T16:00:00.000Z",
      destroyAfter: "2026-09-22T15:00:00.000Z",
      legalHold: false,
    },
    ...overrides,
  };
  const {
    contentManifestDigest: _,
    ciphertextDigest: __,
    encryptedByteCount: ___,
    retention: ____,
    ...contentManifest
  } = draft;
  return createCloudComputerCheckpointManifest({
    ...draft,
    contentManifestDigest:
      overrides.contentManifestDigest ?? checkpointEncryptionBindingDigest(contentManifest),
  });
};

class MemoryGcs implements CloudComputerCheckpointGcs {
  readonly appendedOffsets: number[] = [];
  readonly objects = new Map<string, CloudComputerGcsObject>();
  readonly partials = new Map<string, { sessionRef: string; bytes: Uint8Array }>();
  finalizeCalls = 0;

  async inspectUpload(objectRef: string): Promise<CloudComputerGcsUploadState> {
    const object = this.objects.get(objectRef);
    if (object) return { state: "committed", object };
    const partial = this.partials.get(objectRef);
    return partial
      ? {
          state: "partial",
          sessionRef: partial.sessionRef,
          committedBytes: partial.bytes.byteLength,
        }
      : { state: "missing" };
  }

  async startUpload(input: {
    objectRef: string;
  }): Promise<{ sessionRef: string; committedBytes: number }> {
    const upload = { sessionRef: `upload:${input.objectRef}`, bytes: new Uint8Array() };
    this.partials.set(input.objectRef, upload);
    return { sessionRef: upload.sessionRef, committedBytes: 0 };
  }

  async appendUpload(input: {
    sessionRef: string;
    offset: number;
    bytes: Uint8Array;
  }): Promise<{ committedBytes: number }> {
    this.appendedOffsets.push(input.offset);
    const entry = [...this.partials.entries()].find(
      ([, value]) => value.sessionRef === input.sessionRef,
    );
    if (!entry) throw new Error("upload missing");
    const [objectRef, partial] = entry;
    if (partial.bytes.byteLength !== input.offset) throw new Error("offset mismatch");
    const bytes = new Uint8Array(input.offset + input.bytes.byteLength);
    bytes.set(partial.bytes);
    bytes.set(input.bytes, input.offset);
    this.partials.set(objectRef, { ...partial, bytes });
    return { committedBytes: bytes.byteLength };
  }

  async finalizeUpload(input: {
    sessionRef: string;
    byteCount: number;
    ciphertextDigest: `sha256:${string}`;
  }): Promise<CloudComputerGcsObject> {
    this.finalizeCalls += 1;
    const entry = [...this.partials.entries()].find(
      ([, value]) => value.sessionRef === input.sessionRef,
    );
    if (!entry) throw new Error("upload missing");
    const [objectRef, partial] = entry;
    const object = {
      objectRef,
      generation: "1700000000000001",
      byteCount: input.byteCount,
      ciphertextDigest: input.ciphertextDigest,
      state: "committed" as const,
    };
    if (partial.bytes.byteLength !== input.byteCount) throw new Error("incomplete upload");
    this.objects.set(objectRef, object);
    this.partials.delete(objectRef);
    return object;
  }

  async download(object: CloudComputerGcsObject): Promise<Uint8Array> {
    if (!this.objects.has(object.objectRef)) throw new Error("object missing");
    return ciphertext.slice();
  }

  async tombstone(object: CloudComputerGcsObject): Promise<CloudComputerGcsObject> {
    const tombstone = { ...object, state: "tombstoned" as const };
    this.objects.set(object.objectRef, tombstone);
    return tombstone;
  }

  async deleteGeneration(object: CloudComputerGcsObject) {
    this.objects.delete(object.objectRef);
    return {
      schema: "openagents.cloud_computer_gcs_deletion_verification.v1" as const,
      objectRef: object.objectRef,
      objectGeneration: object.generation,
      generationPreconditionMet: true as const,
      allVersionsAbsent: true as const,
    };
  }
}

describe("cloud computer checkpoint contract", () => {
  test("computes a canonical manifest digest and detects mutation", () => {
    const left = manifestFixture();
    const right = manifestFixture();
    expect(left.manifestDigest).toBe(right.manifestDigest);
    expect(() => verifyCloudComputerCheckpointManifest(left)).not.toThrow();
    expect(() =>
      verifyCloudComputerCheckpointManifest({ ...left, workspaceGeneration: 4 }),
    ).toThrowError(CloudComputerCheckpointError);
  });

  test("requires sorted admitted entries and the complete runtime exclusion set", () => {
    expect(checkpointPathAdmitted(".git/HEAD")).toBe(true);
    expect(checkpointPathAdmitted("src/index.ts")).toBe(true);
    expect(checkpointPathAdmitted("../credentials")).toBe(false);
    expect(checkpointPathAdmitted(".env.production")).toBe(false);
    expect(checkpointPathAdmitted("packages/app/.env.production")).toBe(false);
    expect(checkpointPathAdmitted("vendor/tool/.ssh/id_ed25519")).toBe(false);
    expect(checkpointPathAdmitted("packages/app/.kube/config")).toBe(false);
    expect(checkpointPathAdmitted("vendor/tool/.config/gh/hosts.yml")).toBe(false);
    expect(checkpointPathAdmitted(".git/config")).toBe(false);
    expect(checkpointPathAdmitted(".npmrc")).toBe(false);
    expect(checkpointPathAdmitted(".ssh/id_ed25519")).toBe(false);
    expect(checkpointPathAdmitted("run/provider.sock")).toBe(false);
    expect(() =>
      manifestFixture({ excludedPaths: CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS.slice(1) }),
    ).toThrowError(/excludedPaths/);
    expect(() =>
      manifestFixture({
        entries: [
          {
            path: "z.txt",
            kind: "file",
            classification: "workspace",
            mode: 0o100644,
            byteCount: 10,
            contentDigest: sha256Digest("fixture-z"),
            linkTarget: null,
          },
          {
            path: "a.txt",
            kind: "file",
            classification: "workspace",
            mode: 0o100644,
            byteCount: 0,
            contentDigest: sha256Digest(""),
            linkTarget: null,
          },
        ],
      }),
    ).toThrowError(/entries/);
    expect(() =>
      manifestFixture({
        entries: [
          {
            path: "notes.txt",
            kind: "file",
            classification: "secret" as never,
            mode: 0o100600,
            byteCount: 10,
            contentDigest: sha256Digest("not-secret-fixture"),
            linkTarget: null,
          },
        ],
      }),
    ).toThrowError(/entries/);
    expect(() =>
      manifestFixture({
        entries: [
          {
            path: "escape",
            kind: "symlink",
            classification: "workspace",
            mode: 0o120777,
            byteCount: 0,
            contentDigest: null,
            linkTarget: "../run/provider.sock",
          },
        ],
        plaintextByteCount: 0,
      }),
    ).toThrowError(/entries/);
  });

  test("binds opaque key authority to owner, tenant, workspace, and version", async () => {
    const manifest = manifestFixture();
    const authorizer = cloudComputerWorkspaceKeyAuthorizer({
      authorize: async (input) => input.actorRef === "actor.alice",
    });
    const admitted = await authorizer.authorize({
      operation: "restore",
      actorRef: "actor.alice",
      ownerRef: manifest.ownerRef,
      tenantRef: manifest.tenantRef,
      workspaceRef: manifest.workspaceRef,
      keyRef: manifest.workspaceKeyRef,
      keyVersion: manifest.workspaceKeyVersion,
    });
    expect(() => assertCloudComputerWorkspaceKeyScope(admitted, manifest)).not.toThrow();
    await expect(
      authorizer.authorize({
        operation: "fork",
        actorRef: "actor.mallory",
        ownerRef: "owner.mallory",
        tenantRef: manifest.tenantRef,
        workspaceRef: manifest.workspaceRef,
        keyRef: manifest.workspaceKeyRef,
        keyVersion: manifest.workspaceKeyVersion,
      }),
    ).rejects.toThrowError(/scope_mismatch/);
    const foreignOwnerManifest = manifestFixture({
      ownerRef: "owner.mallory",
    });
    expect(() => assertCloudComputerWorkspaceKeyScope(admitted, foreignOwnerManifest)).toThrowError(
      /scope_mismatch/,
    );
  });

  test("requires explicit full or delta semantics and admitted deletion tombstones", () => {
    expect(manifestFixture().checkpointKind).toBe("delta");
    expect(() =>
      manifestFixture({
        checkpointKind: "full",
        parentCheckpointDigest: null,
      }),
    ).toThrowError(/checkpointKind/);
    expect(() =>
      manifestFixture({
        deletions: [".env"],
      }),
    ).toThrowError(/deletions/);
  });

  test("uses a tenant, owner, and workspace-scoped content address", () => {
    const manifest = manifestFixture();
    expect(checkpointObjectRef(manifest)).toMatch(/^checkpoints\/[0-9a-f]{64}\/[0-9a-f]{64}$/u);
    expect(checkpointObjectRef({ ...manifest, ownerRef: "owner.other" })).not.toBe(
      checkpointObjectRef(manifest),
    );
  });

  test("binds encryption before ciphertext and the final manifest digest exist", () => {
    const {
      ciphertextDigest: _,
      encryptedByteCount: __,
      contentManifestDigest: expected,
      retention: ___,
      manifestDigest: ____,
      ...binding
    } = manifestFixture();
    const digest = checkpointEncryptionBindingDigest(binding);
    expect(digest).toBe(expected);
    expect(digest).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(checkpointEncryptionBindingDigest({ ...binding, workspaceGeneration: 4 })).not.toBe(
      digest,
    );
  });

  test("resumes a partial upload from the durable GCS offset", async () => {
    const storage = new MemoryGcs();
    const manifest = manifestFixture();
    const objectRef = checkpointObjectRef(manifest);
    storage.partials.set(objectRef, {
      sessionRef: `upload:${objectRef}`,
      bytes: ciphertext.slice(0, 8),
    });

    const object = await uploadCloudComputerCheckpoint({ storage, manifest, ciphertext });
    expect(storage.appendedOffsets).toEqual([8]);
    expect(object.ciphertextDigest).toBe(manifest.ciphertextDigest);
    expect(storage.finalizeCalls).toBe(1);
  });

  test("returns the existing object after a lost finalize acknowledgement", async () => {
    const storage = new MemoryGcs();
    const manifest = manifestFixture();
    const first = await uploadCloudComputerCheckpoint({ storage, manifest, ciphertext });
    const second = await uploadCloudComputerCheckpoint({ storage, manifest, ciphertext });
    expect(second).toEqual(first);
    expect(storage.finalizeCalls).toBe(1);
  });

  test("rejects conflicting and corrupt content-addressed objects", async () => {
    const storage = new MemoryGcs();
    const manifest = manifestFixture();
    const objectRef = checkpointObjectRef(manifest);
    storage.objects.set(objectRef, {
      objectRef,
      generation: "bad",
      byteCount: ciphertext.byteLength + 1,
      ciphertextDigest: manifest.ciphertextDigest,
      state: "committed",
    });
    await expect(
      uploadCloudComputerCheckpoint({ storage, manifest, ciphertext }),
    ).rejects.toThrowError(/object_conflict/);

    const corruptStorage = new MemoryGcs();
    const object = await uploadCloudComputerCheckpoint({
      storage: corruptStorage,
      manifest,
      ciphertext,
    });
    corruptStorage.download = async () => new TextEncoder().encode("corrupt");
    await expect(
      downloadCloudComputerCheckpoint({ storage: corruptStorage, object }),
    ).rejects.toThrowError(/integrity_mismatch/);
  });

  test("retains the current and rollback checkpoints and collects expired objects", () => {
    const manifest = manifestFixture();
    const object: CloudComputerGcsObject = {
      objectRef: checkpointObjectRef(manifest),
      generation: "1",
      byteCount: ciphertext.byteLength,
      ciphertextDigest: manifest.ciphertextDigest,
      state: "committed",
    };
    const record = {
      checkpointRef: manifest.checkpointRef,
      manifestDigest: manifest.manifestDigest,
      object,
      committed: true,
      integrityVerified: true,
      current: false,
      previousOfUnverifiedReplacement: false,
      orphanedAt: null,
      orphanGraceUntil: "2026-08-22T16:00:00.000Z",
      retainUntil: "2026-08-22T15:30:00.000Z",
      destroyAfter: null,
      legalHold: false,
    } as const;
    expect(
      planCloudComputerCheckpointGc(
        [
          { ...record, checkpointRef: "current", current: true },
          { ...record, checkpointRef: "rollback", previousOfUnverifiedReplacement: true },
          { ...record, checkpointRef: "unverified", integrityVerified: false },
          { ...record, checkpointRef: "expired" },
          {
            ...record,
            checkpointRef: "orphan",
            committed: false,
            orphanedAt: "2026-08-22T15:00:00.000Z",
          },
        ],
        "2026-08-22T17:00:00.000Z",
      ),
    ).toEqual([
      { checkpointRef: "current", action: "retain", reason: "current" },
      { checkpointRef: "rollback", action: "retain", reason: "replacement_unverified" },
      { checkpointRef: "unverified", action: "retain", reason: "replacement_unverified" },
      { checkpointRef: "expired", action: "tombstone", reason: "expired" },
      { checkpointRef: "orphan", action: "delete", reason: "expired" },
    ]);
  });

  test("makes deletion evidence content-addressed and generation-specific", () => {
    const manifest = manifestFixture();
    const evidence = createCloudComputerCheckpointDeleteEvidence({
      evidenceRef: "evidence.checkpoint05.delete.1",
      checkpointRef: manifest.checkpointRef,
      manifestDigest: manifest.manifestDigest,
      objectRef: checkpointObjectRef(manifest),
      objectGeneration: "1700000000000001",
      action: "deleted",
      observedAt: "2026-09-22T15:00:00.000Z",
    });
    expect(evidence.evidenceDigest).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(
      createCloudComputerCheckpointDeleteEvidence({
        ...evidence,
        objectGeneration: "1700000000000002",
      }).evidenceDigest,
    ).not.toBe(evidence.evidenceDigest);
  });
});
