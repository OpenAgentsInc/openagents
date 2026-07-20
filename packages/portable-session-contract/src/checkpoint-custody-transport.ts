import { Schema } from "effect";

import { PortableCommandExecutionClaimSchema } from "./portable-command-execution.js";

export const PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION =
  "openagents.portable_checkpoint_custody_object_manifest.v1" as const;

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
