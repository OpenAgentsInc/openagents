import { Schema } from "effect";

import { PortableCommandExecutionClaimSchema } from "./portable-command-execution.js";

export const PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION =
  "openagents.portable_checkpoint_custody_object_manifest.v1" as const;
export const PORTABLE_CHECKPOINT_DEK_AUTHORITY_SCHEMA_VERSION =
  "openagents.portable_checkpoint_dek_authority.v1" as const;

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const PortableTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);
const Sha256Digest = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/));
const PositiveInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));
const CanonicalBase64 = Schema.String.check(
  Schema.isPattern(/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/),
);

export const PortableCheckpointCustodyEncryptedV2Schema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_checkpoint_artifact_custody_encrypted.v2"),
  algorithm: Schema.Literal("aes-256-gcm"),
  objectRef: PortableRef,
  policy: Schema.Literal("owner_managed"),
  keyRef: PortableRef,
  nonceBase64: CanonicalBase64,
  authTagBase64: CanonicalBase64,
  ciphertextBase64: CanonicalBase64,
});

export const PortableCheckpointCustodyEncryptedV3Schema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_checkpoint_artifact_custody_encrypted.v3"),
  algorithm: Schema.Literal("aes-256-gcm+google-kms-wrapped-dek"),
  objectRef: PortableRef,
  policy: Schema.Literal("openagents_managed"),
  keyRef: PortableRef,
  wrappedKeyBase64: CanonicalBase64.check(Schema.isMaxLength(128 * 1024)),
  nonceBase64: CanonicalBase64,
  authTagBase64: CanonicalBase64,
  ciphertextBase64: CanonicalBase64,
});

export type PortableCheckpointCustodyEncryptedV3 =
  typeof PortableCheckpointCustodyEncryptedV3Schema.Type;

/** Public-safe current lease facts for one binary DEK wrap or unwrap call. */
export const PortableCheckpointDekAuthoritySchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_CHECKPOINT_DEK_AUTHORITY_SCHEMA_VERSION),
  algorithm: Schema.Literal("aes-256-gcm+google-kms-wrapped-dek"),
  policy: Schema.Literal("openagents_managed"),
  action: Schema.Literals(["wrap", "unwrap"]),
  operationRef: PortableRef,
  commandExecutionClaimRef: PortableRef,
  phaseClaimRef: PortableRef,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  attachmentRef: PortableRef,
  attachmentGeneration: PositiveInt,
  workerInstanceRef: PortableRef,
  claimGeneration: PositiveInt,
  expectedLeaseRevision: PositiveInt,
  expectedLeaseExpiresAt: PortableTimestamp,
  objectRef: PortableRef,
  keyRef: PortableRef,
}).annotate({ identifier: "PortableCheckpointDekAuthority" });
export type PortableCheckpointDekAuthority =
  typeof PortableCheckpointDekAuthoritySchema.Type;

/**
 * Public-safe metadata for one opaque encrypted checkpoint custody object.
 * The encrypted object bytes are a separate private transport value. They
 * must not be added to logs, projections, receipts, or evidence records.
 */
export const PortableCheckpointCustodyObjectManifestSchema = Schema.Struct({
  schema: Schema.Literal(PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION),
  objectRef: PortableRef,
  objectDigest: Sha256Digest,
  artifactRef: PortableRef,
  artifactDigest: Sha256Digest,
  checkpointRef: PortableRef,
  checkpointDigest: Sha256Digest,
  bundleDigest: Sha256Digest,
  ciphertextDigest: Sha256Digest,
  commandClaim: PortableCommandExecutionClaimSchema,
  ownerRef: PortableRef,
  sourcePylonRef: PortableRef,
  targetRef: PortableRef,
  sessionRef: PortableRef,
  sourceAttachmentRef: PortableRef,
  sourceGeneration: PositiveInt,
  custodyPolicy: Schema.Literals(["owner_managed", "openagents_managed"]),
  keyRef: PortableRef,
  byteLimit: PositiveInt,
  createdAt: PortableTimestamp,
  expiresAt: PortableTimestamp,
  retentionSeconds: PositiveInt,
  secretMaterial: Schema.Literal("excluded"),
}).annotate({ identifier: "PortableCheckpointCustodyObjectManifest" });

export type PortableCheckpointCustodyObjectManifest =
  typeof PortableCheckpointCustodyObjectManifestSchema.Type;
