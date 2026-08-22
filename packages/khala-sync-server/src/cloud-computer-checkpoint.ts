import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";

export const CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA =
  "openagents.cloud_computer.checkpoint_manifest.v1" as const;
export const CLOUD_COMPUTER_CHECKPOINT_DELETE_EVIDENCE_SCHEMA =
  "openagents.cloud_computer.checkpoint_delete_evidence.v1" as const;

export type Sha256Digest = `sha256:${string}`;

export const sha256Digest = (value: string | Uint8Array): Sha256Digest =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const SHA256_PATTERN = /^sha256:[0-9a-f]{64}$/u;
const ISO_TIMESTAMP_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u;

export class CloudComputerCheckpointError extends Error {
  override readonly name = "CloudComputerCheckpointError";

  constructor(
    readonly code:
      | "invalid_manifest"
      | "invalid_path"
      | "scope_mismatch"
      | "integrity_mismatch"
      | "upload_regressed"
      | "upload_incomplete"
      | "object_conflict"
      | "object_unavailable",
    readonly field: string,
  ) {
    super(`${code}: ${field}`);
  }
}

export type CloudComputerCheckpointEntry = Readonly<{
  path: string;
  kind: "directory" | "file" | "symlink";
  classification: "workspace" | "git_metadata";
  mode: number;
  byteCount: number;
  contentDigest: Sha256Digest | null;
  linkTarget: string | null;
}>;

export type CloudComputerCheckpointRetention = Readonly<{
  retainUntil: string;
  orphanGraceUntil: string;
  destroyAfter: string | null;
  legalHold: boolean;
}>;

export type CloudComputerCheckpointManifestInput = Readonly<{
  schema: typeof CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA;
  checkpointRef: string;
  idempotencyRef: string;
  ownerRef: string;
  tenantRef: string;
  workspaceRef: string;
  workspaceGeneration: number;
  checkpointKind: "full" | "delta";
  parentCheckpointDigest: Sha256Digest | null;
  baseImageDigest: Sha256Digest;
  workspaceKeyRef: string;
  workspaceKeyVersion: number;
  encryption: "AES-256-GCM";
  entries: ReadonlyArray<CloudComputerCheckpointEntry>;
  deletions: ReadonlyArray<string>;
  excludedPaths: ReadonlyArray<string>;
  plaintextByteCount: number;
  contentManifestDigest: Sha256Digest;
  encryptedByteCount: number;
  ciphertextDigest: Sha256Digest;
  createdAt: string;
  retention: CloudComputerCheckpointRetention;
}>;

export type CloudComputerCheckpointManifest = CloudComputerCheckpointManifestInput &
  Readonly<{ manifestDigest: Sha256Digest }>;

/** Runtime-only state never belongs in a durable workspace checkpoint. */
export const CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS = [
  ".aws/",
  ".codex/auth.json",
  ".config/gcloud/",
  ".env",
  ".env.*",
  ".git/config",
  ".git/config.worktree",
  ".git/credential",
  ".git/credentials",
  ".git/hooks/",
  ".gnupg/",
  ".kube/config",
  ".netrc",
  ".npmrc",
  ".openagents/provider/",
  ".openagents/credentials/",
  ".openagents/runtime/",
  ".openagents/secrets/",
  ".openagents/sockets/",
  ".pypirc",
  ".ssh/",
  ".config/gh/hosts.yml",
  ".docker/config.json",
  "dev/",
  "proc/",
  "run/",
  "sys/",
  "tmp/",
  "var/run/",
] as const;

const normalizedPath = (path: string): string => {
  const normalized = path.replaceAll("\\", "/").replace(/^\.\//u, "");
  if (
    normalized.length === 0 ||
    normalized.startsWith("/") ||
    normalized.endsWith("/") ||
    normalized.includes("\0") ||
    normalized.split("/").some((segment) => segment === "" || segment === "..")
  ) {
    throw new CloudComputerCheckpointError("invalid_path", path);
  }
  return normalized;
};

const exclusionMatches = (path: string, exclusion: string): boolean => {
  if (exclusion.endsWith(".*")) {
    const stem = exclusion.slice(0, -1);
    return path.startsWith(stem);
  }
  if (exclusion.endsWith("/")) return path.startsWith(exclusion);
  return path === exclusion || path.startsWith(`${exclusion}/`);
};

export const checkpointSymlinkStaysInWorkspace = (path: string, target: string): boolean => {
  if (target.length === 0 || target.startsWith("/") || target.includes("\0")) return false;
  const segments = [...path.split("/").slice(0, -1), ...target.replaceAll("\\", "/").split("/")];
  let depth = 0;
  for (const segment of segments) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") {
      if (depth === 0) return false;
      depth -= 1;
    } else {
      depth += 1;
    }
  }
  return true;
};

export const checkpointPathAdmitted = (path: string): boolean => {
  let normalized: string;
  try {
    normalized = normalizedPath(path);
  } catch {
    return false;
  }
  const segments = normalized.split("/");
  if (
    segments.some(
      (segment) =>
        segment === ".env" ||
        (segment.startsWith(".env.") && segment !== ".env.example") ||
        [".netrc", ".npmrc", ".pypirc"].includes(segment),
    ) ||
    segments.some((segment) => [".aws", ".gnupg", ".ssh"].includes(segment)) ||
    segments.some(
      (segment, index) =>
        (segment === ".codex" && segments[index + 1] === "auth.json") ||
        (segment === ".docker" && segments[index + 1] === "config.json") ||
        (segment === ".config" && segments[index + 1] === "gcloud") ||
        (segment === ".config" &&
          segments[index + 1] === "gh" &&
          segments[index + 2] === "hosts.yml") ||
        (segment === ".kube" && segments[index + 1] === "config") ||
        (segment === ".git" &&
          [
            "config",
            "config.worktree",
            "credential",
            "credentials",
            "credential-cache",
            "hooks",
          ].includes(segments[index + 1] ?? "")) ||
        (segment === ".openagents" &&
          ["credentials", "provider", "runtime", "secrets", "sockets"].includes(
            segments[index + 1] ?? "",
          )),
    )
  )
    return false;
  return !CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS.some((exclusion) =>
    exclusionMatches(normalized, exclusion),
  );
};

/** Digests a fully materialized admitted workspace, independent of checkpoint layering. */
export const checkpointWorkspaceStateDigest = (
  entries: ReadonlyArray<CloudComputerCheckpointEntry>,
): Sha256Digest => {
  for (let index = 1; index < entries.length; index += 1) {
    if (entries[index - 1]!.path >= entries[index]!.path)
      throw new CloudComputerCheckpointError("invalid_manifest", "workspaceState.entries");
  }
  return sha256Digest(
    canonicalJson({
      schema: "openagents.cloud_computer_workspace_state.v1",
      entries: entries.map((entry) => ({ ...entry })),
    }),
  );
};

const assertDigest = (value: string | null, field: string): void => {
  if (value !== null && !SHA256_PATTERN.test(value)) {
    throw new CloudComputerCheckpointError("invalid_manifest", field);
  }
};

const assertTimestamp = (value: string, field: string): number => {
  if (!ISO_TIMESTAMP_PATTERN.test(value)) {
    throw new CloudComputerCheckpointError("invalid_manifest", field);
  }
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) {
    throw new CloudComputerCheckpointError("invalid_manifest", field);
  }
  return parsed;
};

const assertInteger = (value: number, field: string, minimum = 0): void => {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new CloudComputerCheckpointError("invalid_manifest", field);
  }
};

const validateManifestInput = (manifest: CloudComputerCheckpointManifestInput): void => {
  if (manifest.schema !== CLOUD_COMPUTER_CHECKPOINT_MANIFEST_SCHEMA) {
    throw new CloudComputerCheckpointError("invalid_manifest", "schema");
  }
  if (manifest.encryption !== "AES-256-GCM") {
    throw new CloudComputerCheckpointError("invalid_manifest", "encryption");
  }
  for (const [field, value] of [
    ["checkpointRef", manifest.checkpointRef],
    ["idempotencyRef", manifest.idempotencyRef],
    ["ownerRef", manifest.ownerRef],
    ["tenantRef", manifest.tenantRef],
    ["workspaceRef", manifest.workspaceRef],
    ["workspaceKeyRef", manifest.workspaceKeyRef],
  ] as const) {
    if (value.length === 0 || value.length > 200) {
      throw new CloudComputerCheckpointError("invalid_manifest", field);
    }
  }
  assertInteger(manifest.workspaceGeneration, "workspaceGeneration", 1);
  assertInteger(manifest.workspaceKeyVersion, "workspaceKeyVersion", 1);
  assertInteger(manifest.plaintextByteCount, "plaintextByteCount");
  assertInteger(manifest.encryptedByteCount, "encryptedByteCount");
  assertDigest(manifest.parentCheckpointDigest, "parentCheckpointDigest");
  assertDigest(manifest.baseImageDigest, "baseImageDigest");
  assertDigest(manifest.contentManifestDigest, "contentManifestDigest");
  assertDigest(manifest.ciphertextDigest, "ciphertextDigest");
  const createdAt = assertTimestamp(manifest.createdAt, "createdAt");
  const retainUntil = assertTimestamp(manifest.retention.retainUntil, "retention.retainUntil");
  const orphanGraceUntil = assertTimestamp(
    manifest.retention.orphanGraceUntil,
    "retention.orphanGraceUntil",
  );
  if (retainUntil < createdAt || orphanGraceUntil < createdAt) {
    throw new CloudComputerCheckpointError("invalid_manifest", "retention");
  }
  if (manifest.retention.destroyAfter !== null) {
    const destroyAfter = assertTimestamp(manifest.retention.destroyAfter, "retention.destroyAfter");
    if (destroyAfter < retainUntil) {
      throw new CloudComputerCheckpointError("invalid_manifest", "retention.destroyAfter");
    }
  }
  for (const required of CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS) {
    if (!manifest.excludedPaths.includes(required)) {
      throw new CloudComputerCheckpointError("invalid_manifest", `excludedPaths.${required}`);
    }
  }
  const seen = new Set<string>();
  let totalBytes = 0;
  let previousPath = "";
  for (const entry of manifest.entries) {
    const path = normalizedPath(entry.path);
    if (
      path !== entry.path ||
      !checkpointPathAdmitted(path) ||
      seen.has(path) ||
      path <= previousPath
    ) {
      throw new CloudComputerCheckpointError("invalid_manifest", `entries.${path}`);
    }
    seen.add(path);
    previousPath = path;
    assertInteger(entry.mode, `entries.${path}.mode`);
    if (
      entry.mode > 0o177777 ||
      !["directory", "file", "symlink"].includes(entry.kind) ||
      !["workspace", "git_metadata"].includes(entry.classification)
    ) {
      throw new CloudComputerCheckpointError("invalid_manifest", `entries.${path}`);
    }
    assertInteger(entry.byteCount, `entries.${path}.byteCount`);
    assertDigest(entry.contentDigest, `entries.${path}.contentDigest`);
    if (entry.kind === "file") {
      if (entry.contentDigest === null || entry.linkTarget !== null) {
        throw new CloudComputerCheckpointError("invalid_manifest", `entries.${path}`);
      }
      totalBytes += entry.byteCount;
    } else if (entry.kind === "symlink") {
      if (
        entry.contentDigest !== null ||
        entry.byteCount !== 0 ||
        entry.linkTarget === null ||
        !checkpointSymlinkStaysInWorkspace(path, entry.linkTarget)
      ) {
        throw new CloudComputerCheckpointError("invalid_manifest", `entries.${path}`);
      }
    } else if (entry.contentDigest !== null || entry.byteCount !== 0 || entry.linkTarget !== null) {
      throw new CloudComputerCheckpointError("invalid_manifest", `entries.${path}`);
    }
  }
  let previousDeletion = "";
  for (const deletion of manifest.deletions) {
    const path = normalizedPath(deletion);
    if (
      path !== deletion ||
      !checkpointPathAdmitted(path) ||
      seen.has(path) ||
      path <= previousDeletion
    ) {
      throw new CloudComputerCheckpointError("invalid_manifest", `deletions.${path}`);
    }
    previousDeletion = path;
  }
  if (
    !["full", "delta"].includes(manifest.checkpointKind) ||
    (manifest.checkpointKind === "delta" && manifest.parentCheckpointDigest === null) ||
    (manifest.checkpointKind === "full" && manifest.deletions.length !== 0)
  ) {
    throw new CloudComputerCheckpointError("invalid_manifest", "checkpointKind");
  }
  if (totalBytes !== manifest.plaintextByteCount) {
    throw new CloudComputerCheckpointError("invalid_manifest", "plaintextByteCount");
  }
  if (checkpointEncryptionBindingDigest(manifest) !== manifest.contentManifestDigest) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "contentManifestDigest");
  }
};

const canonicalManifestPayload = (
  input: CloudComputerCheckpointManifestInput,
): CloudComputerCheckpointManifestInput => ({
  schema: input.schema,
  checkpointRef: input.checkpointRef,
  idempotencyRef: input.idempotencyRef,
  ownerRef: input.ownerRef,
  tenantRef: input.tenantRef,
  workspaceRef: input.workspaceRef,
  workspaceGeneration: input.workspaceGeneration,
  checkpointKind: input.checkpointKind,
  parentCheckpointDigest: input.parentCheckpointDigest,
  baseImageDigest: input.baseImageDigest,
  workspaceKeyRef: input.workspaceKeyRef,
  workspaceKeyVersion: input.workspaceKeyVersion,
  encryption: input.encryption,
  entries: input.entries.map((entry) => ({ ...entry })),
  deletions: [...input.deletions],
  excludedPaths: [...input.excludedPaths],
  plaintextByteCount: input.plaintextByteCount,
  contentManifestDigest: input.contentManifestDigest,
  encryptedByteCount: input.encryptedByteCount,
  ciphertextDigest: input.ciphertextDigest,
  createdAt: input.createdAt,
  retention: { ...input.retention },
});

/** The digest covers canonical field order and the sorted admitted entry list. */
export const createCloudComputerCheckpointManifest = (
  input: CloudComputerCheckpointManifestInput,
): CloudComputerCheckpointManifest => {
  validateManifestInput(input);
  const payload = canonicalManifestPayload(input);
  return { ...payload, manifestDigest: sha256Digest(canonicalJson(payload)) };
};

export const verifyCloudComputerCheckpointManifest = (
  manifest: CloudComputerCheckpointManifest,
): void => {
  const { manifestDigest, ...input } = manifest;
  validateManifestInput(input);
  assertDigest(manifestDigest, "manifestDigest");
  if (sha256Digest(canonicalJson(canonicalManifestPayload(input))) !== manifestDigest) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "manifestDigest");
  }
};

declare const workspaceKeyHandleBrand: unique symbol;
const issuedWorkspaceKeyHandles = new WeakSet<object>();

/** An authorized key handle is opaque; raw workspace key bytes never enter this contract. */
export type CloudComputerWorkspaceKeyHandle = Readonly<{
  operation: "checkpoint" | "restore" | "fork";
  actorRef: string;
  ownerRef: string;
  tenantRef: string;
  workspaceRef: string;
  keyRef: string;
  keyVersion: number;
  [workspaceKeyHandleBrand]: true;
}>;

const issueWorkspaceKeyHandle = (input: {
  operation: "checkpoint" | "restore" | "fork";
  actorRef: string;
  ownerRef: string;
  tenantRef: string;
  workspaceRef: string;
  keyRef: string;
  keyVersion: number;
}): CloudComputerWorkspaceKeyHandle => {
  const handle = Object.freeze({ ...input }) as CloudComputerWorkspaceKeyHandle;
  issuedWorkspaceKeyHandles.add(handle);
  return handle;
};

export type CloudComputerWorkspaceKeyAuthorizer = Readonly<{
  authorize: (
    input: Readonly<{
      operation: "checkpoint" | "restore" | "fork";
      actorRef: string;
      ownerRef: string;
      tenantRef: string;
      workspaceRef: string;
      keyRef: string;
      keyVersion: number;
    }>,
  ) => Promise<CloudComputerWorkspaceKeyHandle>;
}>;

export type CloudComputerWorkspaceKeyAuthorityAdapter = Readonly<{
  authorize: (
    input: Readonly<{
      operation: "checkpoint" | "restore" | "fork";
      actorRef: string;
      ownerRef: string;
      tenantRef: string;
      workspaceRef: string;
      keyRef: string;
      keyVersion: number;
    }>,
  ) => Promise<boolean>;
}>;

/** The adapter decides authority; callers never receive a key-handle constructor. */
export const cloudComputerWorkspaceKeyAuthorizer = (
  adapter: CloudComputerWorkspaceKeyAuthorityAdapter,
): CloudComputerWorkspaceKeyAuthorizer => ({
  authorize: async (input) => {
    if (!(await adapter.authorize(input))) {
      throw new CloudComputerCheckpointError("scope_mismatch", "workspaceKey.authority");
    }
    return issueWorkspaceKeyHandle(input);
  },
});

export type CloudComputerCheckpointCipher = Readonly<{
  seal: (
    input: Readonly<{
      key: CloudComputerWorkspaceKeyHandle;
      plaintext: Uint8Array;
      authenticatedContentManifestDigest: Sha256Digest;
    }>,
  ) => Promise<Uint8Array>;
  open: (
    input: Readonly<{
      key: CloudComputerWorkspaceKeyHandle;
      ciphertext: Uint8Array;
      authenticatedContentManifestDigest: Sha256Digest;
    }>,
  ) => Promise<Uint8Array>;
}>;

/** Computes encryption AAD before ciphertext fields and the final manifest exist. */
export const checkpointEncryptionBindingDigest = (
  input: Omit<
    CloudComputerCheckpointManifestInput,
    "contentManifestDigest" | "ciphertextDigest" | "encryptedByteCount" | "retention"
  >,
): Sha256Digest =>
  sha256Digest(
    canonicalJson({
      schema: input.schema,
      checkpointRef: input.checkpointRef,
      idempotencyRef: input.idempotencyRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      workspaceRef: input.workspaceRef,
      workspaceGeneration: input.workspaceGeneration,
      checkpointKind: input.checkpointKind,
      parentCheckpointDigest: input.parentCheckpointDigest,
      baseImageDigest: input.baseImageDigest,
      workspaceKeyRef: input.workspaceKeyRef,
      workspaceKeyVersion: input.workspaceKeyVersion,
      encryption: input.encryption,
      entries: input.entries.map((entry) => ({ ...entry })),
      deletions: [...input.deletions],
      excludedPaths: [...input.excludedPaths],
      plaintextByteCount: input.plaintextByteCount,
      createdAt: input.createdAt,
    }),
  );

export const assertCloudComputerWorkspaceKeyScope = (
  key: CloudComputerWorkspaceKeyHandle,
  manifest: CloudComputerCheckpointManifest,
): void => {
  if (
    !issuedWorkspaceKeyHandles.has(key) ||
    key.ownerRef !== manifest.ownerRef ||
    key.tenantRef !== manifest.tenantRef ||
    key.workspaceRef !== manifest.workspaceRef ||
    key.keyRef !== manifest.workspaceKeyRef ||
    key.keyVersion !== manifest.workspaceKeyVersion
  ) {
    throw new CloudComputerCheckpointError("scope_mismatch", "workspaceKey");
  }
};

export const assertCloudComputerWorkspaceKeyHandle = (
  key: CloudComputerWorkspaceKeyHandle,
): void => {
  if (!issuedWorkspaceKeyHandles.has(key)) {
    throw new CloudComputerCheckpointError("scope_mismatch", "workspaceKey.authority");
  }
};

export type CloudComputerGcsObject = Readonly<{
  objectRef: string;
  generation: string;
  byteCount: number;
  ciphertextDigest: Sha256Digest;
  state: "committed" | "tombstoned";
}>;

export type CloudComputerGcsUploadState =
  | Readonly<{ state: "missing" }>
  | Readonly<{ state: "partial"; sessionRef: string; committedBytes: number }>
  | Readonly<{ state: "committed"; object: CloudComputerGcsObject }>;

export type CloudComputerGcsDeletionVerification = Readonly<{
  schema: "openagents.cloud_computer_gcs_deletion_verification.v1";
  objectRef: string;
  objectGeneration: string;
  generationPreconditionMet: true;
  allVersionsAbsent: true;
}>;

/** GCS implementations must use create-only object generations and resumable uploads. */
export type CloudComputerCheckpointGcs = Readonly<{
  inspectUpload: (objectRef: string) => Promise<CloudComputerGcsUploadState>;
  startUpload: (
    input: Readonly<{
      objectRef: string;
      byteCount: number;
      ciphertextDigest: Sha256Digest;
      metadata: Readonly<Record<string, string>>;
    }>,
  ) => Promise<Readonly<{ sessionRef: string; committedBytes: number }>>;
  appendUpload: (
    input: Readonly<{
      sessionRef: string;
      offset: number;
      bytes: Uint8Array;
    }>,
  ) => Promise<Readonly<{ committedBytes: number }>>;
  finalizeUpload: (
    input: Readonly<{
      sessionRef: string;
      byteCount: number;
      ciphertextDigest: Sha256Digest;
    }>,
  ) => Promise<CloudComputerGcsObject>;
  download: (object: CloudComputerGcsObject) => Promise<Uint8Array>;
  tombstone: (
    object: CloudComputerGcsObject,
    evidenceRef: string,
  ) => Promise<CloudComputerGcsObject>;
  deleteGeneration: (
    object: CloudComputerGcsObject,
  ) => Promise<CloudComputerGcsDeletionVerification>;
}>;

export const checkpointObjectRef = (
  input: Readonly<{
    tenantRef: string;
    ownerRef: string;
    workspaceRef: string;
    ciphertextDigest: Sha256Digest;
  }>,
): string => {
  assertDigest(input.ciphertextDigest, "ciphertextDigest");
  const scopeDigest = sha256Digest(
    canonicalJson({
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      workspaceRef: input.workspaceRef,
    }),
  ).slice("sha256:".length);
  return `checkpoints/${scopeDigest}/${input.ciphertextDigest.slice("sha256:".length)}`;
};

const assertObjectIntegrity = (
  object: CloudComputerGcsObject,
  expectedRef: string,
  expectedDigest: Sha256Digest,
  expectedBytes: number,
): void => {
  if (
    object.state !== "committed" ||
    object.objectRef !== expectedRef ||
    object.ciphertextDigest !== expectedDigest ||
    object.byteCount !== expectedBytes
  ) {
    throw new CloudComputerCheckpointError("object_conflict", expectedRef);
  }
};

/** Resumes a partial object and treats a prior successful finalize as idempotent success. */
export const uploadCloudComputerCheckpoint = async (
  input: Readonly<{
    storage: CloudComputerCheckpointGcs;
    manifest: CloudComputerCheckpointManifest;
    ciphertext: Uint8Array;
  }>,
): Promise<CloudComputerGcsObject> => {
  verifyCloudComputerCheckpointManifest(input.manifest);
  const digest = sha256Digest(input.ciphertext);
  if (
    digest !== input.manifest.ciphertextDigest ||
    input.ciphertext.byteLength !== input.manifest.encryptedByteCount
  ) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "ciphertext");
  }
  const objectRef = checkpointObjectRef(input.manifest);
  const existing = await input.storage.inspectUpload(objectRef);
  if (existing.state === "committed") {
    assertObjectIntegrity(existing.object, objectRef, digest, input.ciphertext.byteLength);
    return existing.object;
  }
  const upload =
    existing.state === "partial"
      ? existing
      : await input.storage.startUpload({
          objectRef,
          byteCount: input.ciphertext.byteLength,
          ciphertextDigest: digest,
          metadata: {
            manifestDigest: input.manifest.manifestDigest,
            ownerRef: input.manifest.ownerRef,
            tenantRef: input.manifest.tenantRef,
            workspaceRef: input.manifest.workspaceRef,
          },
        });
  if (upload.committedBytes > input.ciphertext.byteLength) {
    throw new CloudComputerCheckpointError("upload_regressed", objectRef);
  }
  let offset = upload.committedBytes;
  if (offset < input.ciphertext.byteLength) {
    const progress = await input.storage.appendUpload({
      sessionRef: upload.sessionRef,
      offset,
      bytes: input.ciphertext.slice(offset),
    });
    if (progress.committedBytes < offset || progress.committedBytes > input.ciphertext.byteLength) {
      throw new CloudComputerCheckpointError("upload_regressed", objectRef);
    }
    offset = progress.committedBytes;
  }
  if (offset !== input.ciphertext.byteLength) {
    throw new CloudComputerCheckpointError("upload_incomplete", objectRef);
  }
  const object = await input.storage.finalizeUpload({
    sessionRef: upload.sessionRef,
    byteCount: input.ciphertext.byteLength,
    ciphertextDigest: digest,
  });
  assertObjectIntegrity(object, objectRef, digest, input.ciphertext.byteLength);
  return object;
};

export const downloadCloudComputerCheckpoint = async (
  input: Readonly<{
    storage: CloudComputerCheckpointGcs;
    object: CloudComputerGcsObject;
  }>,
): Promise<Uint8Array> => {
  if (input.object.state !== "committed") {
    throw new CloudComputerCheckpointError("object_unavailable", input.object.objectRef);
  }
  const bytes = await input.storage.download(input.object);
  if (
    bytes.byteLength !== input.object.byteCount ||
    sha256Digest(bytes) !== input.object.ciphertextDigest
  ) {
    bytes.fill(0);
    throw new CloudComputerCheckpointError("integrity_mismatch", input.object.objectRef);
  }
  return bytes;
};

export type CloudComputerCheckpointGcRecord = Readonly<{
  checkpointRef: string;
  manifestDigest: Sha256Digest;
  object: CloudComputerGcsObject;
  committed: boolean;
  integrityVerified: boolean;
  current: boolean;
  previousOfUnverifiedReplacement: boolean;
  orphanedAt: string | null;
  orphanGraceUntil: string;
  retainUntil: string;
  destroyAfter: string | null;
  legalHold: boolean;
}>;

export type CloudComputerCheckpointGcDecision = Readonly<{
  checkpointRef: string;
  action: "retain" | "tombstone" | "delete";
  reason:
    | "current"
    | "replacement_unverified"
    | "retention"
    | "legal_hold"
    | "orphan_grace"
    | "expired"
    | "destroy";
}>;

/** GC fails closed: current, held, and rollback checkpoints always remain retained. */
export const planCloudComputerCheckpointGc = (
  records: ReadonlyArray<CloudComputerCheckpointGcRecord>,
  now: string,
): ReadonlyArray<CloudComputerCheckpointGcDecision> => {
  const nowMs = assertTimestamp(now, "now");
  return records.map((record) => {
    if (record.current)
      return { checkpointRef: record.checkpointRef, action: "retain", reason: "current" };
    if (record.previousOfUnverifiedReplacement) {
      return {
        checkpointRef: record.checkpointRef,
        action: "retain",
        reason: "replacement_unverified",
      };
    }
    if (record.committed && !record.integrityVerified) {
      return {
        checkpointRef: record.checkpointRef,
        action: "retain",
        reason: "replacement_unverified",
      };
    }
    if (record.legalHold) {
      return { checkpointRef: record.checkpointRef, action: "retain", reason: "legal_hold" };
    }
    if (
      record.destroyAfter !== null &&
      nowMs >= assertTimestamp(record.destroyAfter, "destroyAfter")
    ) {
      return { checkpointRef: record.checkpointRef, action: "delete", reason: "destroy" };
    }
    if (!record.committed && record.orphanedAt !== null) {
      assertTimestamp(record.orphanedAt, "orphanedAt");
      if (nowMs >= assertTimestamp(record.orphanGraceUntil, "orphanGraceUntil")) {
        return { checkpointRef: record.checkpointRef, action: "delete", reason: "expired" };
      }
      return { checkpointRef: record.checkpointRef, action: "retain", reason: "orphan_grace" };
    }
    if (nowMs < assertTimestamp(record.retainUntil, "retainUntil")) {
      return { checkpointRef: record.checkpointRef, action: "retain", reason: "retention" };
    }
    return { checkpointRef: record.checkpointRef, action: "tombstone", reason: "expired" };
  });
};

export type CloudComputerCheckpointDeleteEvidence = Readonly<{
  schema: typeof CLOUD_COMPUTER_CHECKPOINT_DELETE_EVIDENCE_SCHEMA;
  evidenceRef: string;
  checkpointRef: string;
  manifestDigest: Sha256Digest;
  objectRef: string;
  objectGeneration: string;
  action: "tombstoned" | "deleted";
  observedAt: string;
  evidenceDigest: Sha256Digest;
}>;

export const createCloudComputerCheckpointDeleteEvidence = (
  input: Omit<CloudComputerCheckpointDeleteEvidence, "schema" | "evidenceDigest">,
): CloudComputerCheckpointDeleteEvidence => {
  assertDigest(input.manifestDigest, "manifestDigest");
  assertTimestamp(input.observedAt, "observedAt");
  const payload = {
    schema: CLOUD_COMPUTER_CHECKPOINT_DELETE_EVIDENCE_SCHEMA,
    evidenceRef: input.evidenceRef,
    checkpointRef: input.checkpointRef,
    manifestDigest: input.manifestDigest,
    objectRef: input.objectRef,
    objectGeneration: input.objectGeneration,
    action: input.action,
    observedAt: input.observedAt,
  };
  return { ...payload, evidenceDigest: sha256Digest(canonicalJson(payload)) };
};
