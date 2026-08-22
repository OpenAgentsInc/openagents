import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";

import {
  CloudComputerCheckpointError,
  assertCloudComputerWorkspaceKeyHandle,
  assertCloudComputerWorkspaceKeyScope,
  checkpointEncryptionBindingDigest,
  createCloudComputerCheckpointManifest,
  downloadCloudComputerCheckpoint,
  sha256Digest,
  uploadCloudComputerCheckpoint,
  verifyCloudComputerCheckpointManifest,
  type CloudComputerCheckpointCipher,
  type CloudComputerCheckpointGcs,
  type CloudComputerCheckpointManifest,
  type CloudComputerCheckpointManifestInput,
  type CloudComputerGcsObject,
  type CloudComputerWorkspaceKeyAuthorizer,
  type CloudComputerWorkspaceKeyHandle,
  type Sha256Digest,
} from "./cloud-computer-checkpoint.js";

const ENVELOPE_SCHEMA = "openagents.cloud_computer_checkpoint_encrypted.v1" as const;
const AES_KEY_BYTES = 32;
const AES_NONCE_BYTES = 12;
const AES_TAG_BYTES = 16;
const ENVELOPE_FIELDS = new Set([
  "algorithm",
  "authTagBase64",
  "ciphertextBase64",
  "keyRef",
  "keyVersion",
  "nonceBase64",
  "schema",
  "wrappedDataEncryptionKeyBase64",
]);

export type CloudComputerCheckpointKeyPort = Readonly<{
  wrapDataEncryptionKey: (
    input: Readonly<{
      key: CloudComputerWorkspaceKeyHandle;
      dataEncryptionKey: Uint8Array;
    }>,
  ) => Promise<Uint8Array>;
  unwrapDataEncryptionKey: (
    input: Readonly<{
      key: CloudComputerWorkspaceKeyHandle;
      wrappedDataEncryptionKey: Uint8Array;
    }>,
  ) => Promise<Uint8Array>;
}>;

export type NodeAes256GcmCheckpointCipherConfig = Readonly<{
  keys: CloudComputerCheckpointKeyPort;
  maxEnvelopeBytes?: number;
}>;

type EncryptedEnvelope = Readonly<{
  schema: typeof ENVELOPE_SCHEMA;
  algorithm: "AES-256-GCM";
  keyRef: string;
  keyVersion: number;
  nonceBase64: string;
  authTagBase64: string;
  wrappedDataEncryptionKeyBase64: string;
  ciphertextBase64: string;
}>;

const encodeBase64 = (bytes: Uint8Array): string => Buffer.from(bytes).toString("base64");

const decodeBase64 = (value: unknown, field: string): Uint8Array => {
  if (typeof value !== "string" || value.length === 0) {
    throw new CloudComputerCheckpointError("integrity_mismatch", field);
  }
  const decoded = Uint8Array.from(Buffer.from(value, "base64"));
  if (encodeBase64(decoded) !== value) {
    decoded.fill(0);
    throw new CloudComputerCheckpointError("integrity_mismatch", field);
  }
  return decoded;
};

const decodeEnvelope = (bytes: Uint8Array, maximum: number): EncryptedEnvelope => {
  if (bytes.byteLength === 0 || bytes.byteLength > maximum) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope.size");
  }
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope");
  }
  const record = value as Readonly<Record<string, unknown>>;
  if (
    Object.keys(record).length !== ENVELOPE_FIELDS.size ||
    Object.keys(record).some((field) => !ENVELOPE_FIELDS.has(field)) ||
    record.schema !== ENVELOPE_SCHEMA ||
    record.algorithm !== "AES-256-GCM" ||
    typeof record.keyRef !== "string" ||
    !Number.isSafeInteger(record.keyVersion)
  ) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope");
  }
  return record as EncryptedEnvelope;
};

/** Production Node AES-256-GCM envelope encryption backed by an injected KMS port. */
export const nodeAes256GcmCheckpointCipher = (
  config: NodeAes256GcmCheckpointCipherConfig,
): CloudComputerCheckpointCipher => {
  const maximum = config.maxEnvelopeBytes ?? 2 * 1024 * 1024 * 1024;
  if (!Number.isSafeInteger(maximum) || maximum < 1) {
    throw new CloudComputerCheckpointError("invalid_manifest", "maxEnvelopeBytes");
  }
  return {
    seal: async ({ key, plaintext, authenticatedContentManifestDigest }) => {
      assertCloudComputerWorkspaceKeyHandle(key);
      const dataEncryptionKey = Uint8Array.from(randomBytes(AES_KEY_BYTES));
      const nonce = Uint8Array.from(randomBytes(AES_NONCE_BYTES));
      let wrappedDataEncryptionKey: Uint8Array | undefined;
      let encrypted: Buffer | undefined;
      let authTag: Buffer | undefined;
      try {
        wrappedDataEncryptionKey = await config.keys.wrapDataEncryptionKey({
          key,
          dataEncryptionKey,
        });
        if (wrappedDataEncryptionKey.byteLength === 0) {
          throw new CloudComputerCheckpointError("object_unavailable", "wrappedDataEncryptionKey");
        }
        const cipher = createCipheriv("aes-256-gcm", dataEncryptionKey, nonce, {
          authTagLength: AES_TAG_BYTES,
        });
        cipher.setAAD(Buffer.from(authenticatedContentManifestDigest, "utf8"));
        encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()]);
        authTag = cipher.getAuthTag();
        const envelope: EncryptedEnvelope = {
          schema: ENVELOPE_SCHEMA,
          algorithm: "AES-256-GCM",
          keyRef: key.keyRef,
          keyVersion: key.keyVersion,
          nonceBase64: encodeBase64(nonce),
          authTagBase64: authTag.toString("base64"),
          wrappedDataEncryptionKeyBase64: encodeBase64(wrappedDataEncryptionKey),
          ciphertextBase64: encrypted.toString("base64"),
        };
        const encoded = new TextEncoder().encode(canonicalJson(envelope));
        if (encoded.byteLength > maximum) {
          encoded.fill(0);
          throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope.size");
        }
        return encoded;
      } finally {
        dataEncryptionKey.fill(0);
        nonce.fill(0);
        wrappedDataEncryptionKey?.fill(0);
        encrypted?.fill(0);
        authTag?.fill(0);
      }
    },
    open: async ({ key, ciphertext, authenticatedContentManifestDigest }) => {
      assertCloudComputerWorkspaceKeyHandle(key);
      const envelope = decodeEnvelope(ciphertext, maximum);
      if (envelope.keyRef !== key.keyRef || envelope.keyVersion !== key.keyVersion) {
        throw new CloudComputerCheckpointError("scope_mismatch", "encryptedEnvelope.key");
      }
      const nonce = decodeBase64(envelope.nonceBase64, "encryptedEnvelope.nonce");
      const authTag = decodeBase64(envelope.authTagBase64, "encryptedEnvelope.authTag");
      const wrappedDataEncryptionKey = decodeBase64(
        envelope.wrappedDataEncryptionKeyBase64,
        "encryptedEnvelope.wrappedDataEncryptionKey",
      );
      const encrypted = decodeBase64(envelope.ciphertextBase64, "encryptedEnvelope.ciphertext");
      let dataEncryptionKey: Uint8Array | undefined;
      try {
        if (nonce.byteLength !== AES_NONCE_BYTES || authTag.byteLength !== AES_TAG_BYTES) {
          throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope");
        }
        dataEncryptionKey = await config.keys.unwrapDataEncryptionKey({
          key,
          wrappedDataEncryptionKey,
        });
        if (dataEncryptionKey.byteLength !== AES_KEY_BYTES) {
          throw new CloudComputerCheckpointError("integrity_mismatch", "dataEncryptionKey");
        }
        const decipher = createDecipheriv("aes-256-gcm", dataEncryptionKey, nonce, {
          authTagLength: AES_TAG_BYTES,
        });
        decipher.setAAD(Buffer.from(authenticatedContentManifestDigest, "utf8"));
        decipher.setAuthTag(authTag);
        let decrypted: Buffer | undefined;
        try {
          decrypted = Buffer.concat([decipher.update(encrypted), decipher.final()]);
          return Uint8Array.from(decrypted);
        } catch {
          throw new CloudComputerCheckpointError("integrity_mismatch", "encryptedEnvelope.authTag");
        } finally {
          decrypted?.fill(0);
        }
      } finally {
        dataEncryptionKey?.fill(0);
        nonce.fill(0);
        authTag.fill(0);
        wrappedDataEncryptionKey.fill(0);
        encrypted.fill(0);
      }
    },
  };
};

export type CloudComputerCheckpointDraft = Omit<
  CloudComputerCheckpointManifestInput,
  "contentManifestDigest" | "encryptedByteCount" | "ciphertextDigest"
>;

export type CloudComputerCheckpointServiceReceipt = Readonly<{
  schema: "openagents.cloud_computer_checkpoint_service_receipt.v1";
  checkpointRef: string;
  manifestDigest: Sha256Digest;
  objectRef: string;
  objectGeneration: string;
  encryptedByteCount: number;
}>;

export type CloudComputerCheckpointService = Readonly<{
  checkpoint: (
    input: Readonly<{
      actorRef: string;
      draft: CloudComputerCheckpointDraft;
      plaintext: Uint8Array;
    }>,
  ) => Promise<
    Readonly<{
      manifest: CloudComputerCheckpointManifest;
      object: CloudComputerGcsObject;
      receipt: CloudComputerCheckpointServiceReceipt;
    }>
  >;
  restore: (
    input: Readonly<{
      actorRef: string;
      manifest: CloudComputerCheckpointManifest;
      object: CloudComputerGcsObject;
    }>,
  ) => Promise<Uint8Array>;
}>;

/** Runs the authorization, encryption, storage, download, and restore integrity sequence. */
export const cloudComputerCheckpointService = (
  input: Readonly<{
    authorizer: CloudComputerWorkspaceKeyAuthorizer;
    cipher: CloudComputerCheckpointCipher;
    storage: CloudComputerCheckpointGcs;
  }>,
): CloudComputerCheckpointService => ({
  checkpoint: async ({ actorRef, draft, plaintext }) => {
    const { retention, ...binding } = draft;
    const contentManifestDigest = checkpointEncryptionBindingDigest(binding);
    const provisionalManifest = createCloudComputerCheckpointManifest({
      ...draft,
      contentManifestDigest,
      encryptedByteCount: 0,
      ciphertextDigest: sha256Digest(new Uint8Array()),
    });
    if (plaintext.byteLength !== draft.plaintextByteCount) {
      throw new CloudComputerCheckpointError("integrity_mismatch", "plaintextByteCount");
    }
    const key = await input.authorizer.authorize({
      operation: "checkpoint",
      actorRef,
      ownerRef: draft.ownerRef,
      tenantRef: draft.tenantRef,
      workspaceRef: draft.workspaceRef,
      keyRef: draft.workspaceKeyRef,
      keyVersion: draft.workspaceKeyVersion,
    });
    assertCloudComputerWorkspaceKeyScope(key, provisionalManifest);
    const ciphertext = await input.cipher.seal({
      key,
      plaintext,
      authenticatedContentManifestDigest: contentManifestDigest,
    });
    try {
      const manifest = createCloudComputerCheckpointManifest({
        ...draft,
        retention,
        contentManifestDigest,
        encryptedByteCount: ciphertext.byteLength,
        ciphertextDigest: sha256Digest(ciphertext),
      });
      const object = await uploadCloudComputerCheckpoint({
        storage: input.storage,
        manifest,
        ciphertext,
      });
      return {
        manifest,
        object,
        receipt: {
          schema: "openagents.cloud_computer_checkpoint_service_receipt.v1",
          checkpointRef: manifest.checkpointRef,
          manifestDigest: manifest.manifestDigest,
          objectRef: object.objectRef,
          objectGeneration: object.generation,
          encryptedByteCount: object.byteCount,
        },
      };
    } finally {
      ciphertext.fill(0);
    }
  },
  restore: async ({ actorRef, manifest, object }) => {
    verifyCloudComputerCheckpointManifest(manifest);
    if (
      object.state !== "committed" ||
      object.ciphertextDigest !== manifest.ciphertextDigest ||
      object.byteCount !== manifest.encryptedByteCount
    ) {
      throw new CloudComputerCheckpointError("object_conflict", object.objectRef);
    }
    const key = await input.authorizer.authorize({
      operation: "restore",
      actorRef,
      ownerRef: manifest.ownerRef,
      tenantRef: manifest.tenantRef,
      workspaceRef: manifest.workspaceRef,
      keyRef: manifest.workspaceKeyRef,
      keyVersion: manifest.workspaceKeyVersion,
    });
    assertCloudComputerWorkspaceKeyScope(key, manifest);
    const ciphertext = await downloadCloudComputerCheckpoint({ storage: input.storage, object });
    try {
      const plaintext = await input.cipher.open({
        key,
        ciphertext,
        authenticatedContentManifestDigest: manifest.contentManifestDigest,
      });
      if (plaintext.byteLength !== manifest.plaintextByteCount) {
        plaintext.fill(0);
        throw new CloudComputerCheckpointError("integrity_mismatch", "plaintextByteCount");
      }
      return plaintext;
    } finally {
      ciphertext.fill(0);
    }
  },
});
