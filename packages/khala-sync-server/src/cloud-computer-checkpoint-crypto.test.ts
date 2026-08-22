import { describe, expect, test } from "vite-plus/test";

import {
  CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA,
  CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS,
  checkpointObjectRef,
  cloudComputerWorkspaceKeyAuthorizer,
  sha256Digest,
  type CloudComputerCheckpointGcs,
  type CloudComputerGcsObject,
  type CloudComputerWorkspaceKeyHandle,
} from "./cloud-computer-checkpoint.js";
import {
  cloudComputerCheckpointService,
  nodeAes256GcmCheckpointCipher,
  type CloudComputerCheckpointDraft,
} from "./cloud-computer-checkpoint-crypto.js";

const plaintext = new TextEncoder().encode("main");

const draft: CloudComputerCheckpointDraft = {
  schema: CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA,
  checkpointRef: "checkpoint.crypto.1",
  idempotencyRef: "operation.crypto.1",
  ownerRef: "owner.alice",
  tenantRef: "tenant.acme",
  workspaceRef: "workspace.crypto.1",
  workspaceGeneration: 1,
  checkpointKind: "full",
  parentCheckpointDigest: null,
  baseImageDigest: `sha256:${"1".repeat(64)}`,
  workspaceKeyRef: "kms.workspace.crypto.1",
  workspaceKeyVersion: 3,
  encryption: "AES-256-GCM",
  entries: [
    {
      path: "README.md",
      kind: "file",
      classification: "workspace",
      mode: 0o100644,
      byteCount: plaintext.byteLength,
      contentDigest: sha256Digest(plaintext),
      linkTarget: null,
    },
  ],
  deletions: [],
  excludedPaths: [...CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS],
  plaintextByteCount: plaintext.byteLength,
  createdAt: "2026-08-22T12:00:00.000Z",
  retention: {
    retainUntil: "2026-08-23T12:00:00.000Z",
    orphanGraceUntil: "2026-08-22T13:00:00.000Z",
    destroyAfter: null,
    legalHold: false,
  },
};

class MemoryStorage implements CloudComputerCheckpointGcs {
  constructor(readonly events: string[] = []) {}

  object: CloudComputerGcsObject | null = null;
  bytes: Uint8Array | null = null;
  pending:
    | { objectRef: string; byteCount: number; ciphertextDigest: `sha256:${string}` }
    | undefined;

  async inspectUpload(objectRef: string) {
    return this.object?.objectRef === objectRef
      ? ({ state: "committed", object: this.object } as const)
      : ({ state: "missing" } as const);
  }

  async startUpload(input: {
    objectRef: string;
    byteCount: number;
    ciphertextDigest: `sha256:${string}`;
  }) {
    this.pending = input;
    return { sessionRef: "session.crypto.1", committedBytes: 0 };
  }

  async appendUpload(input: { sessionRef: string; offset: number; bytes: Uint8Array }) {
    this.bytes = Uint8Array.from(input.bytes);
    return { committedBytes: input.offset + input.bytes.byteLength };
  }

  async finalizeUpload() {
    if (this.pending === undefined || this.bytes === null) throw new Error("upload missing");
    this.object = {
      objectRef: this.pending.objectRef,
      generation: "17",
      byteCount: this.pending.byteCount,
      ciphertextDigest: this.pending.ciphertextDigest,
      state: "committed",
    };
    return this.object;
  }

  async download() {
    this.events.push("download");
    if (this.bytes === null) throw new Error("object missing");
    return Uint8Array.from(this.bytes);
  }

  async tombstone(object: CloudComputerGcsObject) {
    return { ...object, state: "tombstoned" as const };
  }

  async deleteGeneration(object: CloudComputerGcsObject) {
    return {
      schema: "openagents.cloud_computer_gcs_deletion_verification.v1" as const,
      objectRef: object.objectRef,
      objectGeneration: object.generation,
      generationPreconditionMet: true as const,
      allVersionsAbsent: true as const,
    };
  }
}

const keyHarness = (events: string[]) => {
  const wrappingKey = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
  let wrappedInput: Uint8Array | undefined;
  let unwrappedOutput: Uint8Array | undefined;
  const cipher = nodeAes256GcmCheckpointCipher({
    keys: {
      wrapDataEncryptionKey: async ({ dataEncryptionKey }) => {
        events.push("wrap");
        wrappedInput = dataEncryptionKey;
        return dataEncryptionKey.map((byte, index) => byte ^ wrappingKey[index]!);
      },
      unwrapDataEncryptionKey: async ({ wrappedDataEncryptionKey }) => {
        events.push("unwrap");
        unwrappedOutput = wrappedDataEncryptionKey.map((byte, index) => byte ^ wrappingKey[index]!);
        return unwrappedOutput;
      },
    },
  });
  return {
    cipher,
    wrappedInput: () => wrappedInput,
    unwrappedOutput: () => unwrappedOutput,
  };
};

describe("cloud computer checkpoint encryption", () => {
  test("authorizes, seals, uploads, downloads, reauthorizes, opens, and verifies", async () => {
    const events: string[] = [];
    const storage = new MemoryStorage(events);
    const keys = keyHarness(events);
    const authorizer = cloudComputerWorkspaceKeyAuthorizer({
      authorize: async ({ operation }) => {
        events.push(`authorize:${operation}`);
        return true;
      },
    });
    const service = cloudComputerCheckpointService({ authorizer, cipher: keys.cipher, storage });

    const checkpoint = await service.checkpoint({ actorRef: "actor.alice", draft, plaintext });
    expect(checkpoint.object.objectRef).toBe(checkpointObjectRef(checkpoint.manifest));
    expect(events.slice(0, 2)).toEqual(["authorize:checkpoint", "wrap"]);
    expect([...keys.wrappedInput()!]).toEqual(Array(32).fill(0));
    expect(JSON.stringify(checkpoint.receipt)).not.toMatch(/key|wrapped|ciphertext/iu);

    const restored = await service.restore({
      actorRef: "actor.alice",
      manifest: checkpoint.manifest,
      object: checkpoint.object,
    });
    expect(restored).toEqual(plaintext);
    expect(events.slice(-3)).toEqual(["authorize:restore", "download", "unwrap"]);
    expect([...keys.unwrappedOutput()!]).toEqual(Array(32).fill(0));
  });

  test("rejects a forged key handle and authentication-data mismatch", async () => {
    const events: string[] = [];
    const keys = keyHarness(events);
    const forged = {
      ownerRef: draft.ownerRef,
      tenantRef: draft.tenantRef,
      workspaceRef: draft.workspaceRef,
      keyRef: draft.workspaceKeyRef,
      keyVersion: draft.workspaceKeyVersion,
    } as CloudComputerWorkspaceKeyHandle;
    await expect(
      keys.cipher.seal({
        key: forged,
        plaintext,
        authenticatedContentManifestDigest: sha256Digest("binding"),
      }),
    ).rejects.toMatchObject({ code: "scope_mismatch" });

    const authorizer = cloudComputerWorkspaceKeyAuthorizer({ authorize: async () => true });
    const key = await authorizer.authorize({
      operation: "checkpoint",
      actorRef: "actor.alice",
      ownerRef: draft.ownerRef,
      tenantRef: draft.tenantRef,
      workspaceRef: draft.workspaceRef,
      keyRef: draft.workspaceKeyRef,
      keyVersion: draft.workspaceKeyVersion,
    });
    const encrypted = await keys.cipher.seal({
      key,
      plaintext,
      authenticatedContentManifestDigest: sha256Digest("binding.one"),
    });
    await expect(
      keys.cipher.open({
        key,
        ciphertext: encrypted,
        authenticatedContentManifestDigest: sha256Digest("binding.two"),
      }),
    ).rejects.toMatchObject({ code: "integrity_mismatch" });
  });

  test("fails authorization before reading checkpoint bytes", async () => {
    const storage = new MemoryStorage();
    storage.object = {
      objectRef: "checkpoints/missing",
      generation: "1",
      byteCount: 1,
      ciphertextDigest: `sha256:${"f".repeat(64)}`,
      state: "committed",
    };
    const keys = keyHarness([]);
    const service = cloudComputerCheckpointService({
      authorizer: cloudComputerWorkspaceKeyAuthorizer({ authorize: async () => false }),
      cipher: keys.cipher,
      storage,
    });
    const checkpointStorage = new MemoryStorage();
    const allowed = cloudComputerCheckpointService({
      authorizer: cloudComputerWorkspaceKeyAuthorizer({ authorize: async () => true }),
      cipher: keys.cipher,
      storage: checkpointStorage,
    });
    const checkpoint = await allowed.checkpoint({ actorRef: "actor.alice", draft, plaintext });

    await expect(
      service.restore({
        actorRef: "actor.foreign",
        manifest: checkpoint.manifest,
        object: checkpoint.object,
      }),
    ).rejects.toMatchObject({ code: "scope_mismatch" });
    expect(storage.events).toEqual([]);
  });

  test("rejects nested secret paths before authorization or upload", async () => {
    const events: string[] = [];
    const storage = new MemoryStorage(events);
    const keys = keyHarness(events);
    const service = cloudComputerCheckpointService({
      authorizer: cloudComputerWorkspaceKeyAuthorizer({
        authorize: async () => {
          events.push("authorize");
          return true;
        },
      }),
      cipher: keys.cipher,
      storage,
    });
    await expect(
      service.checkpoint({
        actorRef: "actor.alice",
        draft: {
          ...draft,
          entries: [{ ...draft.entries[0]!, path: "packages/app/.env.production" }],
        },
        plaintext,
      }),
    ).rejects.toMatchObject({ code: "invalid_manifest" });
    expect(events).toEqual([]);
  });
});
