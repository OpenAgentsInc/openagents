import {
  PortableCheckpointCustodyObjectManifestSchema,
  type PortableCheckpointCustodyObjectManifest,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationRecord,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema as S } from "effect";

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();
const MAX_SIDECAR_BYTES = 128 * 1024;

const Sha256Digest = S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/));
const PortableRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const Timestamp = S.String.check(S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/));

const PreparedSidecarSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_prepare.v1"),
  state: S.Literal("prepared"),
  manifestDigest: Sha256Digest,
  manifest: PortableCheckpointCustodyObjectManifestSchema,
  operationRef: PortableRef,
  phaseClaimRef: PortableRef,
  phaseClaimGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  phaseLeaseRevision: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  pylonRef: PortableRef,
  targetRef: PortableRef,
  preparedAt: Timestamp,
});
type PreparedSidecar = typeof PreparedSidecarSchema.Type;

const PrepareOperationBindingSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_prepare_binding.v1"),
  operationRef: PortableRef,
  manifestDigest: Sha256Digest,
  objectDigest: Sha256Digest,
  pylonRef: PortableRef,
  targetRef: PortableRef,
  recordedAt: Timestamp,
});
type PrepareOperationBinding = typeof PrepareOperationBindingSchema.Type;

const CommittedSidecarSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_commit.v1"),
  state: S.Literal("committed"),
  manifestDigest: Sha256Digest,
  objectDigest: Sha256Digest,
  operationRef: PortableRef,
  phaseClaimRef: PortableRef,
  phaseClaimGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  phaseLeaseRevision: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  verifiedAt: Timestamp,
});
type CommittedSidecar = typeof CommittedSidecarSchema.Type;

const RedemptionSidecarSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_redemption.v1"),
  state: S.Literal("redeemed"),
  redemptionRef: PortableRef,
  manifestDigest: Sha256Digest,
  objectDigest: Sha256Digest,
  operationRef: PortableRef,
  phaseClaimRef: PortableRef,
  phaseClaimGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  phaseLeaseRevision: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  pylonRef: PortableRef,
  targetRef: PortableRef,
  redeemedAt: Timestamp,
  expiresAt: Timestamp,
});
type RedemptionSidecar = typeof RedemptionSidecarSchema.Type;

const DeletionIntentSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_deletion_intent.v1"),
  state: S.Literal("delete_pending"),
  manifestDigest: Sha256Digest,
  objectDigest: Sha256Digest,
  operationRef: PortableRef,
  phaseClaimRef: PortableRef,
  phaseClaimGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  phaseLeaseRevision: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  pylonRef: PortableRef,
  targetRef: PortableRef,
  recordedAt: Timestamp,
});
type DeletionIntent = typeof DeletionIntentSchema.Type;

const DeletionReceiptSchema = S.Struct({
  schema: S.Literal("openagents.portable_checkpoint_artifact_deletion_receipt.v1"),
  state: S.Literal("deleted"),
  manifestDigest: Sha256Digest,
  objectDigest: Sha256Digest,
  operationRef: PortableRef,
  verifiedAbsent: S.Literal(true),
  deletedAt: Timestamp,
});
type DeletionReceipt = typeof DeletionReceiptSchema.Type;

const decodePrepared = S.decodeUnknownSync(PreparedSidecarSchema);
const decodePrepareOperationBinding = S.decodeUnknownSync(PrepareOperationBindingSchema);
const decodeCommitted = S.decodeUnknownSync(CommittedSidecarSchema);
const decodeRedemption = S.decodeUnknownSync(RedemptionSidecarSchema);
const decodeDeletionIntent = S.decodeUnknownSync(DeletionIntentSchema);
const decodeDeletionReceipt = S.decodeUnknownSync(DeletionReceiptSchema);

export class PortableCheckpointArtifactError extends S.TaggedErrorClass<PortableCheckpointArtifactError>()(
  "PortableCheckpointArtifactError",
  {
    code: S.Literals([
      "invalid",
      "not_found",
      "conflict",
      "expired",
      "stale_claim",
      "too_large",
      "unavailable",
    ]),
    operation: S.String,
  },
) {}

export type PortableCheckpointArtifactObject = Readonly<{
  size: number;
  bytes: () => Promise<Uint8Array>;
}>;

export type PortableCheckpointArtifactBucket = Readonly<{
  get: (key: string) => Promise<PortableCheckpointArtifactObject | null>;
  putIfAbsent: (
    key: string,
    bytes: Uint8Array,
    options: Readonly<{ contentType: string }>,
  ) => Promise<"created" | "exists">;
  delete: (key: string) => Promise<void>;
}>;

export type PortableCheckpointArtifactAuthority = Readonly<{
  operation: PortablePhaseOperationRecord;
  commandClaim: PortableCommandExecutionClaim;
}>;

export type PortableCheckpointArtifactServiceDependencies = Readonly<{
  bucket: PortableCheckpointArtifactBucket;
  readAuthority: (
    pylonRef: string,
    targetRef: string,
    operationRef: string,
  ) => Promise<PortableCheckpointArtifactAuthority>;
  now?: () => Date;
  maximumObjectBytes?: number;
  prefix?: string;
}>;

export type PortableCheckpointArtifactScope = Readonly<{
  ownerRef: string;
  pylonRef: string;
  targetRef: string;
}>;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Readonly<Record<string, unknown>>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const digestBytes = async (bytes: Uint8Array): Promise<`sha256:${string}`> => {
  const copy = Uint8Array.from(bytes);
  const digest = await crypto.subtle.digest("SHA-256", copy.buffer);
  return `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
};

const digestValue = (value: unknown): Promise<`sha256:${string}`> =>
  digestBytes(textEncoder.encode(canonical(value)));

const digestHex = (digest: string): string => digest.slice("sha256:".length);

const sameValue = (left: unknown, right: unknown): boolean => canonical(left) === canonical(right);

const fail = (
  code: PortableCheckpointArtifactError["code"],
  operation: string,
): PortableCheckpointArtifactError => new PortableCheckpointArtifactError({ code, operation });

const parseCiphertextDigest = async (bytes: Uint8Array): Promise<`sha256:${string}`> => {
  let value: unknown;
  try {
    value = JSON.parse(textDecoder.decode(bytes));
  } catch {
    throw fail("invalid", "ciphertext_decode");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw fail("invalid", "ciphertext_decode");
  }
  const encoded = (value as Readonly<Record<string, unknown>>).ciphertextBase64;
  if (typeof encoded !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(encoded)) {
    throw fail("invalid", "ciphertext_decode");
  }
  let decoded: Uint8Array;
  try {
    const binary = atob(encoded);
    decoded = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  } catch {
    throw fail("invalid", "ciphertext_decode");
  }
  if (decoded.byteLength === 0) throw fail("invalid", "ciphertext_decode");
  return digestBytes(decoded);
};

const operationClaim = (operation: PortablePhaseOperationRecord) => {
  if (
    operation.state !== "claimed" ||
    operation.claimRef === null ||
    operation.claimGeneration === null ||
    operation.leaseRevision === null ||
    operation.leaseExpiresAt === null
  ) {
    throw fail("stale_claim", "phase_claim");
  }
  return {
    claimRef: operation.claimRef,
    claimGeneration: operation.claimGeneration,
    leaseRevision: operation.leaseRevision,
    leaseExpiresAt: operation.leaseExpiresAt,
  };
};

const assertCurrentCommandClaim = (
  expected: PortableCommandExecutionClaim,
  actual: PortableCommandExecutionClaim,
  now: Date,
): void => {
  if (
    !sameValue(expected, actual) ||
    actual.state !== "claimed" ||
    actual.terminalStatus !== null ||
    actual.outcomeRef !== null ||
    Date.parse(actual.claimedAt) > now.getTime() ||
    Date.parse(actual.leaseExpiresAt) <= now.getTime()
  ) {
    throw fail("stale_claim", "command_claim");
  }
};

const assertCommonOperation = (
  authority: PortableCheckpointArtifactAuthority,
  scope: PortableCheckpointArtifactScope,
  manifest: PortableCheckpointCustodyObjectManifest,
  now: Date,
) => {
  const { operation, commandClaim } = authority;
  const claim = operationClaim(operation);
  if (
    operation.request.ownerRef !== scope.ownerRef ||
    operation.request.pylonRef !== scope.pylonRef ||
    operation.request.targetRef !== scope.targetRef ||
    operation.request.commandRef !== manifest.commandClaim.commandRef ||
    operation.request.commandExecutionClaimRef !== manifest.commandClaim.claimRef ||
    operation.request.sessionRef !== manifest.sessionRef ||
    operation.request.expiresAt < manifest.expiresAt ||
    Date.parse(operation.request.expiresAt) <= now.getTime() ||
    Date.parse(claim.leaseExpiresAt) <= now.getTime()
  ) {
    throw fail("stale_claim", "phase_binding");
  }
  assertCurrentCommandClaim(manifest.commandClaim, commandClaim, now);
  if (
    manifest.ownerRef !== commandClaim.ownerRef ||
    manifest.sessionRef !== commandClaim.sessionRef ||
    manifest.sourceAttachmentRef !== commandClaim.sourceAttachmentRef ||
    manifest.sourceGeneration !== commandClaim.sourceGeneration ||
    manifest.sourcePylonRef !== commandClaim.executorEnvironmentRef ||
    manifest.targetRef !== commandClaim.destinationTargetRef ||
    Date.parse(manifest.createdAt) > now.getTime() ||
    Date.parse(manifest.expiresAt) <= now.getTime() ||
    manifest.expiresAt > commandClaim.leaseExpiresAt ||
    manifest.retentionSeconds !==
      Math.floor((Date.parse(manifest.expiresAt) - Date.parse(manifest.createdAt)) / 1_000)
  ) {
    throw fail("invalid", "manifest_binding");
  }
  return claim;
};

const assertSourceAuthority = (
  authority: PortableCheckpointArtifactAuthority,
  scope: PortableCheckpointArtifactScope,
  manifest: PortableCheckpointCustodyObjectManifest,
  now: Date,
) => {
  const claim = assertCommonOperation(authority, scope, manifest, now);
  const request = authority.operation.request;
  if (
    request.kind !== "checkpoint-create" ||
    request.attachmentRef !== manifest.sourceAttachmentRef ||
    request.attachmentGeneration !== manifest.sourceGeneration ||
    request.targetRef !== manifest.commandClaim.executorEnvironmentRef ||
    request.checkpointRef !== manifest.checkpointRef ||
    request.checkpointObjectRef !== null ||
    request.checkpointDigest !== null
  ) {
    throw fail("conflict", "source_operation_binding");
  }
  return claim;
};

const assertDestinationAuthority = (
  authority: PortableCheckpointArtifactAuthority,
  scope: PortableCheckpointArtifactScope,
  manifest: PortableCheckpointCustodyObjectManifest,
  now: Date,
) => {
  const claim = assertCommonOperation(authority, scope, manifest, now);
  const request = authority.operation.request;
  if (
    request.kind !== "checkpoint-stage" ||
    request.attachmentGeneration !== manifest.sourceGeneration + 1 ||
    request.targetRef !== manifest.targetRef ||
    request.checkpointRef !== manifest.checkpointRef ||
    request.checkpointObjectRef !== manifest.objectRef ||
    request.checkpointDigest !== manifest.checkpointDigest
  ) {
    throw fail("conflict", "destination_operation_binding");
  }
  return claim;
};

const assertDeleteAuthority = (
  authority: PortableCheckpointArtifactAuthority,
  scope: PortableCheckpointArtifactScope,
  manifest: PortableCheckpointCustodyObjectManifest,
  now: Date,
) => {
  const claim = assertCommonOperation(authority, scope, manifest, now);
  const request = authority.operation.request;
  const sourceCleanup =
    request.kind === "source-cleanup" &&
    request.attachmentRef === manifest.sourceAttachmentRef &&
    request.attachmentGeneration === manifest.sourceGeneration &&
    request.targetRef === manifest.commandClaim.executorEnvironmentRef;
  const stagedAbort =
    request.kind === "staged-abort" &&
    request.attachmentGeneration === manifest.sourceGeneration + 1 &&
    request.targetRef === manifest.targetRef;
  if (!sourceCleanup && !stagedAbort) throw fail("conflict", "delete_operation_binding");
  return claim;
};

const fromBucket = <A>(operation: string, thunk: () => Promise<A>) =>
  Effect.tryPromise({
    try: thunk,
    catch: (error) =>
      error instanceof PortableCheckpointArtifactError ? error : fail("unavailable", operation),
  });

export const makePortableCheckpointArtifactService = (
  dependencies: PortableCheckpointArtifactServiceDependencies,
) => {
  const now = dependencies.now ?? (() => new Date());
  const maximumObjectBytes = dependencies.maximumObjectBytes ?? 64 * 1024 * 1024;
  const prefix = dependencies.prefix ?? "portable-checkpoint-custody/v1/";
  if (!Number.isSafeInteger(maximumObjectBytes) || maximumObjectBytes <= 0) {
    throw new Error("portable checkpoint artifact maximum object bytes is invalid");
  }

  const manifestRoot = (manifestDigest: string) =>
    `${prefix}manifests/sha256/${digestHex(manifestDigest)}/`;
  const preparedKey = (manifestDigest: string) => `${manifestRoot(manifestDigest)}prepared.json`;
  const committedKey = (manifestDigest: string) => `${manifestRoot(manifestDigest)}committed.json`;
  const deletionIntentKey = (manifestDigest: string) =>
    `${manifestRoot(manifestDigest)}deletion-intent.json`;
  const deletionReceiptKey = (manifestDigest: string) =>
    `${manifestRoot(manifestDigest)}deleted.json`;
  const objectKey = (objectDigest: string) => `${prefix}objects/sha256/${digestHex(objectDigest)}`;
  const prepareOperationKey = async (
    scope: PortableCheckpointArtifactScope,
    operationRef: string,
  ) =>
    `${prefix}prepare-operations/sha256/${digestHex(
      await digestValue({
        ownerRef: scope.ownerRef,
        pylonRef: scope.pylonRef,
        targetRef: scope.targetRef,
        operationRef,
      }),
    )}.json`;

  const readJson = <A>(key: string, decode: (input: unknown) => A) =>
    Effect.gen(function* () {
      const object = yield* fromBucket("sidecar_read", () => dependencies.bucket.get(key));
      if (object === null) return null;
      if (object.size <= 0 || object.size > MAX_SIDECAR_BYTES) {
        return yield* fail("conflict", "sidecar_size");
      }
      const bytes = yield* fromBucket("sidecar_read", () => object.bytes());
      if (bytes.byteLength !== object.size || bytes.byteLength > MAX_SIDECAR_BYTES) {
        return yield* fail("conflict", "sidecar_size");
      }
      return yield* Effect.try({
        try: () => decode(JSON.parse(textDecoder.decode(bytes))),
        catch: () => fail("conflict", "sidecar_decode"),
      });
    });

  const putSidecar = <A>(key: string, value: A, decode: (input: unknown) => A) =>
    Effect.gen(function* () {
      const decoded = decode(value);
      const existing = yield* readJson(key, decode);
      if (existing !== null) {
        if (!sameValue(existing, decoded)) {
          return yield* fail("conflict", "sidecar_replay");
        }
        return existing;
      }
      const bytes = textEncoder.encode(canonical(decoded));
      yield* fromBucket("sidecar_write", () =>
        dependencies.bucket.putIfAbsent(key, bytes, { contentType: "application/json" }),
      );
      const verified = yield* readJson(key, decode);
      if (verified === null || !sameValue(verified, decoded)) {
        return yield* fail("conflict", "sidecar_readback");
      }
      return verified;
    });

  const readPrepared = (manifestDigest: string) =>
    Effect.gen(function* () {
      const prepared = yield* readJson(preparedKey(manifestDigest), decodePrepared);
      if (prepared === null) return yield* fail("not_found", "prepare");
      if (prepared.manifestDigest !== manifestDigest) {
        return yield* fail("conflict", "manifest_digest");
      }
      return prepared;
    });

  const readCommitted = (manifestDigest: string) =>
    Effect.gen(function* () {
      const committed = yield* readJson(committedKey(manifestDigest), decodeCommitted);
      if (committed === null) return yield* fail("not_found", "commit");
      const prepared = yield* readPrepared(manifestDigest);
      if (
        committed.manifestDigest !== manifestDigest ||
        committed.objectDigest !== prepared.manifest.objectDigest
      ) {
        return yield* fail("conflict", "commit_binding");
      }
      return { committed, prepared };
    });

  const readVerifiedObject = (manifest: PortableCheckpointCustodyObjectManifest) =>
    Effect.gen(function* () {
      const object = yield* fromBucket("object_read", () =>
        dependencies.bucket.get(objectKey(manifest.objectDigest)),
      );
      if (object === null) return yield* fail("not_found", "object_read");
      if (
        object.size <= 0 ||
        object.size > manifest.byteLimit ||
        object.size > maximumObjectBytes
      ) {
        return yield* fail("too_large", "object_read");
      }
      const bytes = yield* fromBucket("object_read", () => object.bytes());
      if (bytes.byteLength !== object.size) {
        return yield* fail("conflict", "object_size");
      }
      if (
        (yield* Effect.promise(() => digestBytes(bytes))) !== manifest.objectDigest ||
        (yield* Effect.promise(() => parseCiphertextDigest(bytes))) !== manifest.ciphertextDigest
      ) {
        return yield* fail("conflict", "object_digest");
      }
      return bytes;
    });

  const prepare = Effect.fn("PortableCheckpointArtifact.prepare")(function* (
    scope: PortableCheckpointArtifactScope,
    operationRef: string,
    manifestInput: unknown,
  ) {
    const manifest: PortableCheckpointCustodyObjectManifest = yield* Effect.try({
      try: () => S.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(manifestInput),
      catch: () => fail("invalid", "manifest_decode"),
    });
    if (manifest.byteLimit > maximumObjectBytes) {
      return yield* fail("too_large", "manifest_byte_limit");
    }
    const manifestDigest = yield* Effect.promise(() => digestValue(manifest));
    const deleted = yield* readJson(deletionReceiptKey(manifestDigest), decodeDeletionReceipt);
    if (deleted !== null) return yield* fail("conflict", "object_deleted");
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    const phase = assertSourceAuthority(authority, scope, manifest, now());
    const operationKey = yield* Effect.promise(() => prepareOperationKey(scope, operationRef));
    const existingOperation = yield* readJson(operationKey, decodePrepareOperationBinding);
    if (existingOperation !== null && existingOperation.manifestDigest !== manifestDigest) {
      return yield* fail("conflict", "prepare_replay");
    }
    const existingPrepared = yield* readJson(preparedKey(manifestDigest), decodePrepared);
    if (existingPrepared !== null) {
      if (
        existingPrepared.operationRef !== operationRef ||
        existingPrepared.pylonRef !== scope.pylonRef ||
        existingPrepared.targetRef !== scope.targetRef ||
        !sameValue(existingPrepared.manifest, manifest)
      ) {
        return yield* fail("conflict", "prepare_replay");
      }
      return {
        manifestDigest,
        objectDigest: manifest.objectDigest,
        byteLimit: manifest.byteLimit,
        expiresAt: manifest.expiresAt,
      };
    }
    const preparedAt = now().toISOString();
    const operationBinding: PrepareOperationBinding = {
      schema: "openagents.portable_checkpoint_artifact_prepare_binding.v1",
      operationRef,
      manifestDigest,
      objectDigest: manifest.objectDigest,
      pylonRef: scope.pylonRef,
      targetRef: scope.targetRef,
      recordedAt: preparedAt,
    };
    const prepared: PreparedSidecar = {
      schema: "openagents.portable_checkpoint_artifact_prepare.v1",
      state: "prepared",
      manifestDigest,
      manifest,
      operationRef,
      phaseClaimRef: phase.claimRef,
      phaseClaimGeneration: phase.claimGeneration,
      phaseLeaseRevision: phase.leaseRevision,
      pylonRef: scope.pylonRef,
      targetRef: scope.targetRef,
      preparedAt,
    };
    yield* putSidecar(operationKey, operationBinding, decodePrepareOperationBinding);
    yield* putSidecar(preparedKey(manifestDigest), prepared, decodePrepared);
    return {
      manifestDigest,
      objectDigest: manifest.objectDigest,
      byteLimit: manifest.byteLimit,
      expiresAt: manifest.expiresAt,
    };
  });

  const upload = Effect.fn("PortableCheckpointArtifact.upload")(function* (
    scope: PortableCheckpointArtifactScope,
    manifestDigest: string,
    operationRef: string,
    bytes: Uint8Array,
  ) {
    const prepared = yield* readPrepared(manifestDigest);
    if (prepared.operationRef !== operationRef) {
      return yield* fail("conflict", "prepare_operation");
    }
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    assertSourceAuthority(authority, scope, prepared.manifest, now());
    if (
      bytes.byteLength <= 0 ||
      bytes.byteLength > prepared.manifest.byteLimit ||
      bytes.byteLength > maximumObjectBytes
    ) {
      return yield* fail("too_large", "object_upload");
    }
    if ((yield* Effect.promise(() => digestBytes(bytes))) !== prepared.manifest.objectDigest) {
      return yield* fail("conflict", "object_digest");
    }
    const key = objectKey(prepared.manifest.objectDigest);
    const existing = yield* fromBucket("object_read", () => dependencies.bucket.get(key));
    if (existing === null) {
      yield* fromBucket("object_write", () =>
        dependencies.bucket.putIfAbsent(key, bytes, {
          contentType: "application/octet-stream",
        }),
      );
    }
    yield* readVerifiedObject(prepared.manifest);
    return { manifestDigest, objectDigest: prepared.manifest.objectDigest };
  });

  const commit = Effect.fn("PortableCheckpointArtifact.commit")(function* (
    scope: PortableCheckpointArtifactScope,
    manifestDigest: string,
    operationRef: string,
  ) {
    const prepared = yield* readPrepared(manifestDigest);
    if (prepared.operationRef !== operationRef) {
      return yield* fail("conflict", "prepare_operation");
    }
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    const phase = assertSourceAuthority(authority, scope, prepared.manifest, now());
    yield* readVerifiedObject(prepared.manifest);
    const existing = yield* readJson(committedKey(manifestDigest), decodeCommitted);
    if (existing !== null) {
      if (
        existing.operationRef !== operationRef ||
        existing.phaseClaimRef !== phase.claimRef ||
        existing.phaseClaimGeneration !== phase.claimGeneration ||
        existing.phaseLeaseRevision !== phase.leaseRevision
      ) {
        return yield* fail("conflict", "commit_replay");
      }
      return { manifestDigest, objectDigest: prepared.manifest.objectDigest };
    }
    const committed: CommittedSidecar = {
      schema: "openagents.portable_checkpoint_artifact_commit.v1",
      state: "committed",
      manifestDigest,
      objectDigest: prepared.manifest.objectDigest,
      operationRef,
      phaseClaimRef: phase.claimRef,
      phaseClaimGeneration: phase.claimGeneration,
      phaseLeaseRevision: phase.leaseRevision,
      verifiedAt: now().toISOString(),
    };
    yield* putSidecar(committedKey(manifestDigest), committed, decodeCommitted);
    return { manifestDigest, objectDigest: prepared.manifest.objectDigest };
  });

  const redeem = Effect.fn("PortableCheckpointArtifact.redeem")(function* (
    scope: PortableCheckpointArtifactScope,
    manifestDigest: string,
    operationRef: string,
  ) {
    const { prepared } = yield* readCommitted(manifestDigest);
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    const phase = assertDestinationAuthority(authority, scope, prepared.manifest, now());
    yield* readVerifiedObject(prepared.manifest);
    const redemptionDigest = yield* Effect.promise(() =>
      digestValue({
        manifestDigest,
        operationRef,
        phaseClaimRef: phase.claimRef,
        phaseClaimGeneration: phase.claimGeneration,
        phaseLeaseRevision: phase.leaseRevision,
      }),
    );
    const redemptionRef = `redemption.portable-checkpoint.${digestHex(redemptionDigest)}`;
    const expiresAt = new Date(
      Math.min(Date.parse(phase.leaseExpiresAt), Date.parse(prepared.manifest.expiresAt)),
    ).toISOString();
    const redemption: RedemptionSidecar = {
      schema: "openagents.portable_checkpoint_artifact_redemption.v1",
      state: "redeemed",
      redemptionRef,
      manifestDigest,
      objectDigest: prepared.manifest.objectDigest,
      operationRef,
      phaseClaimRef: phase.claimRef,
      phaseClaimGeneration: phase.claimGeneration,
      phaseLeaseRevision: phase.leaseRevision,
      pylonRef: scope.pylonRef,
      targetRef: scope.targetRef,
      redeemedAt: now().toISOString(),
      expiresAt,
    };
    const redemptionKey = `${manifestRoot(manifestDigest)}redemptions/${digestHex(redemptionDigest)}.json`;
    const existing = yield* readJson(redemptionKey, decodeRedemption);
    if (existing === null) {
      yield* putSidecar(redemptionKey, redemption, decodeRedemption);
    } else if (
      existing.redemptionRef !== redemptionRef ||
      existing.pylonRef !== scope.pylonRef ||
      existing.targetRef !== scope.targetRef
    ) {
      return yield* fail("conflict", "redeem_replay");
    }
    return { redemptionRef, expiresAt, manifest: prepared.manifest };
  });

  const download = Effect.fn("PortableCheckpointArtifact.download")(function* (
    scope: PortableCheckpointArtifactScope,
    manifestDigest: string,
    operationRef: string,
    redemptionRef: string,
  ) {
    const { prepared } = yield* readCommitted(manifestDigest);
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    const phase = assertDestinationAuthority(authority, scope, prepared.manifest, now());
    const redemptionDigest = yield* Effect.promise(() =>
      digestValue({
        manifestDigest,
        operationRef,
        phaseClaimRef: phase.claimRef,
        phaseClaimGeneration: phase.claimGeneration,
        phaseLeaseRevision: phase.leaseRevision,
      }),
    );
    const redemptionKey = `${manifestRoot(manifestDigest)}redemptions/${digestHex(redemptionDigest)}.json`;
    const redemption = yield* readJson(redemptionKey, decodeRedemption);
    if (
      redemption === null ||
      redemption.redemptionRef !== redemptionRef ||
      redemption.pylonRef !== scope.pylonRef ||
      redemption.targetRef !== scope.targetRef ||
      Date.parse(redemption.expiresAt) <= now().getTime()
    ) {
      return yield* fail("stale_claim", "redemption");
    }
    return yield* readVerifiedObject(prepared.manifest);
  });

  const deleteObject = Effect.fn("PortableCheckpointArtifact.delete")(function* (
    scope: PortableCheckpointArtifactScope,
    manifestDigest: string,
    operationRef: string,
  ) {
    const { prepared } = yield* readCommitted(manifestDigest);
    const existingReceipt = yield* readJson(
      deletionReceiptKey(manifestDigest),
      decodeDeletionReceipt,
    );
    if (existingReceipt !== null) {
      if (existingReceipt.operationRef !== operationRef) {
        return yield* fail("conflict", "delete_replay");
      }
      const absent = yield* fromBucket("object_read", () =>
        dependencies.bucket.get(objectKey(prepared.manifest.objectDigest)),
      );
      if (absent !== null) return yield* fail("conflict", "delete_readback");
      return existingReceipt;
    }
    const authority = yield* fromBucket("authority_read", () =>
      dependencies.readAuthority(scope.pylonRef, scope.targetRef, operationRef),
    );
    const phase = assertDeleteAuthority(authority, scope, prepared.manifest, now());
    const intent: DeletionIntent = {
      schema: "openagents.portable_checkpoint_artifact_deletion_intent.v1",
      state: "delete_pending",
      manifestDigest,
      objectDigest: prepared.manifest.objectDigest,
      operationRef,
      phaseClaimRef: phase.claimRef,
      phaseClaimGeneration: phase.claimGeneration,
      phaseLeaseRevision: phase.leaseRevision,
      pylonRef: scope.pylonRef,
      targetRef: scope.targetRef,
      recordedAt: now().toISOString(),
    };
    yield* putSidecar(deletionIntentKey(manifestDigest), intent, decodeDeletionIntent);
    yield* fromBucket("object_delete", () =>
      dependencies.bucket.delete(objectKey(prepared.manifest.objectDigest)),
    );
    const remaining = yield* fromBucket("object_read", () =>
      dependencies.bucket.get(objectKey(prepared.manifest.objectDigest)),
    );
    if (remaining !== null) return yield* fail("conflict", "delete_readback");
    const receipt: DeletionReceipt = {
      schema: "openagents.portable_checkpoint_artifact_deletion_receipt.v1",
      state: "deleted",
      manifestDigest,
      objectDigest: prepared.manifest.objectDigest,
      operationRef,
      verifiedAbsent: true,
      deletedAt: now().toISOString(),
    };
    return yield* putSidecar(deletionReceiptKey(manifestDigest), receipt, decodeDeletionReceipt);
  });

  return { commit, deleteObject, download, prepare, redeem, upload };
};
