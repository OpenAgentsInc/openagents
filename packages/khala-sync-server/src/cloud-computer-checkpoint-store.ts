import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";

import {
  assertCloudComputerWorkspaceKeyHandle,
  checkpointPathAdmitted,
  checkpointSymlinkStaysInWorkspace,
  checkpointWorkspaceStateDigest,
  CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS,
  type CloudComputerCheckpointEntry,
  type CloudComputerGcsDeletionVerification,
  type CloudComputerWorkspaceKeyHandle,
} from "./cloud-computer-checkpoint.js";
import { assertGoogleCloudStorageDeletionVerification } from "./cloud-computer-checkpoint-gcs.js";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const LOCK = "openagents.cloud-computer.checkpoint";

export type CloudCheckpointBoundary =
  | "explicit"
  | "interval"
  | "stop"
  | "host_replacement"
  | "fork";

export class CloudComputerCheckpointStoreError extends Error {
  constructor(
    readonly code:
      | "conflict"
      | "corrupt_store"
      | "integrity_failed"
      | "invalid"
      | "not_found"
      | "permission_denied"
      | "stale_generation",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerCheckpointStoreError";
  }
}

export type InitializeCloudComputerWorkspaceInput = Readonly<{
  workspaceRef: string;
  computerRef: string;
  runtimeGeneration: number;
  ownerRef: string;
  tenantRef: string;
  conversationRef: string;
  baseImageDigest: string;
  baseImageSignatureRef: string;
  workspaceKeyRef: string;
  workspaceKeyVersion: number;
  createdAt: string;
}>;

export type PrepareCloudCheckpointInput = Readonly<{
  operationRef: string;
  idempotencyRef: string;
  requestDigest: string;
  workspaceRef: string;
  ownerRef: string;
  tenantRef: string;
  computerRef: string;
  expectedRuntimeGeneration: number;
  expectedWorkspaceRevision: number;
  expectedParentCheckpointRef: string | null;
  boundary: CloudCheckpointBoundary;
  createdAt: string;
}>;

export type CloudCheckpointOperation = Readonly<{
  operationRef: string;
  requestDigest: string;
  status: string;
  uploadSessionRef: string | null;
  uploadedByteCount: number;
  objectRef: string | null;
  objectGeneration: number | null;
  checkpointRef: string | null;
  receipt: CloudComputerCheckpointReceipt | null;
}>;

export type RecordVerifiedCloudCheckpointObjectInput = Readonly<{
  operationRef: string;
  requestDigest: string;
  objectRef: string;
  workspaceRef: string;
  ownerRef: string;
  tenantRef: string;
  objectUri: string;
  objectGeneration: number;
  contentDigest: string;
  contentManifestDigest: string;
  storageManifestDigest: string;
  ciphertextDigest: string;
  crc32c: string;
  workspaceKeyRef: string;
  workspaceKeyVersion: number;
  wrappedDekRef: string;
  encryptedByteCount: number;
  createdAt: string;
  verifiedAt: string;
  retainUntil: string;
}>;

export type CommitCloudComputerCheckpointInput = Readonly<{
  operationRef: string;
  requestDigest: string;
  checkpointRef: string;
  objectRef: string;
  contentDigest: string;
  workspaceStateDigest: string;
  contentManifestDigest: string;
  storageManifestDigest: string;
  baseImageDigest: string;
  checkpointKind: "full" | "delta";
  deletedPaths: ReadonlyArray<string>;
  plaintextByteCount: number;
  encryptedByteCount: number;
  retentionPolicyRef: string;
  verifiedAt: string;
  committedAt: string;
  retainUntil: string;
  contentManifest: unknown;
  storageManifest: unknown;
}>;

export type CloudComputerCheckpointReceipt = Readonly<{
  schema: "openagents.cloud_computer_checkpoint_receipt.v1";
  operationRef: string;
  checkpointRef: string;
  workspaceRef: string;
  computerRef: string;
  sourceRuntimeGeneration: number;
  workspaceRevision: number;
  parentCheckpointRef: string | null;
  objectRef: string;
  contentDigest: string;
  contentManifestDigest: string;
  storageManifestDigest: string;
  plaintextByteCount: number;
  encryptedByteCount: number;
  committedAt: string;
  replayed: boolean;
}>;

export type CloudComputerCheckpointRestorePlan = Readonly<{
  schema: "openagents.cloud_computer_restore_plan.v1";
  workspaceRef: string;
  checkpointRef: string;
  workspaceRevision: number;
  objectRef: string;
  objectGeneration: number;
  contentDigest: string;
  contentManifestDigest: string;
  storageManifestDigest: string;
  ciphertextDigest: string;
  workspaceKeyRef: string;
  workspaceKeyVersion: number;
  baseImageDigest: string;
  checkpointKind: "full" | "delta";
  deletedPaths: ReadonlyArray<string>;
  encryptedByteCount: number;
  contentManifest: unknown;
  storageManifest: unknown;
  layers: ReadonlyArray<CloudComputerCheckpointRestoreLayer>;
}>;

export type CloudComputerCheckpointRestoreLayer = Readonly<{
  checkpointRef: string;
  workspaceRevision: number;
  objectRef: string;
  objectGeneration: number;
  contentDigest: string;
  contentManifestDigest: string;
  storageManifestDigest: string;
  ciphertextDigest: string;
  checkpointKind: "full" | "delta";
  deletedPaths: ReadonlyArray<string>;
  encryptedByteCount: number;
  contentManifest: unknown;
  storageManifest: unknown;
}>;

export type CloudCheckpointGcCandidate = Readonly<{
  objectRef: string;
  workspaceRef: string;
  objectGeneration: number;
  ciphertextDigest: string;
  encryptedByteCount: number;
}>;

type WorkspaceRow = Readonly<{
  workspace_ref: string;
  computer_ref: string;
  runtime_generation: string | number;
  owner_ref: string;
  tenant_ref: string;
  conversation_ref: string;
  workspace_revision: string | number;
  current_checkpoint_ref: string | null;
  base_image_digest: string;
  base_image_signature_ref: string;
  workspace_key_ref: string;
  workspace_key_version: string | number;
  state: "active" | "destroying" | "destroyed";
}>;

type OperationRow = Readonly<{
  operation_ref: string;
  request_digest: string;
  workspace_ref: string;
  owner_ref: string;
  tenant_ref: string;
  computer_ref: string;
  expected_runtime_generation: string | number;
  expected_workspace_revision: string | number;
  expected_parent_checkpoint_ref: string | null;
  boundary: CloudCheckpointBoundary;
  status: string;
  upload_session_ref: string | null;
  uploaded_byte_count: string | number;
  object_ref: string | null;
  object_generation: string | number | null;
  checkpoint_ref: string | null;
  receipt_json: unknown | null;
}>;

const assertRef = (value: string, field: string): void => {
  if (!REF.test(value))
    throw new CloudComputerCheckpointStoreError("invalid", `${field} is invalid`);
};
const assertDigest = (value: string, field: string): void => {
  if (!SHA256.test(value))
    throw new CloudComputerCheckpointStoreError("invalid", `${field} is invalid`);
};
const assertInteger = (value: number, field: string, zero = false): void => {
  if (!Number.isSafeInteger(value) || value < (zero ? 0 : 1))
    throw new CloudComputerCheckpointStoreError("invalid", `${field} is invalid`);
};
const instant = (value: string, field: string): number => {
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed))
    throw new CloudComputerCheckpointStoreError("invalid", `${field} is invalid`);
  return parsed;
};
const jsonDigest = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;
const parse = <A>(value: unknown, field: string): A => {
  try {
    return (typeof value === "string" ? JSON.parse(value) : value) as A;
  } catch {
    throw new CloudComputerCheckpointStoreError("corrupt_store", `${field} is invalid JSON`);
  }
};
const record = (value: unknown, field: string): Readonly<Record<string, unknown>> => {
  const parsed = parse<unknown>(value, field);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed))
    throw new CloudComputerCheckpointStoreError("invalid", `${field} must be an object`);
  return parsed as Readonly<Record<string, unknown>>;
};
const assertManifestField = (
  manifest: Readonly<Record<string, unknown>>,
  field: string,
  expected: unknown,
): void => {
  if (canonicalJson(manifest[field]) !== canonicalJson(expected))
    throw new CloudComputerCheckpointStoreError(
      "integrity_failed",
      `manifest ${field} binding differs`,
    );
};
const assertExactFields = (
  manifest: Readonly<Record<string, unknown>>,
  fields: ReadonlyArray<string>,
  name: string,
): void => {
  const actual = Object.keys(manifest);
  if (actual.length !== fields.length || actual.some((field) => !fields.includes(field)))
    throw new CloudComputerCheckpointStoreError("integrity_failed", `${name} fields differ`);
};

const validateContentEntries = (
  value: unknown,
): Readonly<{
  plaintextByteCount: number;
  paths: ReadonlySet<string>;
  entries: ReadonlyArray<CloudComputerCheckpointEntry>;
}> => {
  if (!Array.isArray(value))
    throw new CloudComputerCheckpointStoreError("integrity_failed", "content entries differ");
  const seen = new Set<string>();
  const entries: CloudComputerCheckpointEntry[] = [];
  let previousPath = "";
  let plaintextByteCount = 0;
  for (const raw of value) {
    const entry = record(raw, "content entry");
    assertExactFields(
      entry,
      ["path", "kind", "classification", "mode", "byteCount", "contentDigest", "linkTarget"],
      "content entry",
    );
    const path = entry.path;
    if (
      typeof path !== "string" ||
      !checkpointPathAdmitted(path) ||
      seen.has(path) ||
      path <= previousPath ||
      (entry.classification !== "workspace" && entry.classification !== "git_metadata") ||
      !["directory", "file", "symlink"].includes(String(entry.kind)) ||
      !Number.isSafeInteger(entry.mode) ||
      Number(entry.mode) < 0 ||
      Number(entry.mode) > 0o177777 ||
      !Number.isSafeInteger(entry.byteCount) ||
      Number(entry.byteCount) < 0 ||
      (entry.contentDigest !== null &&
        (typeof entry.contentDigest !== "string" || !SHA256.test(entry.contentDigest))) ||
      (entry.linkTarget !== null && typeof entry.linkTarget !== "string")
    )
      throw new CloudComputerCheckpointStoreError(
        "integrity_failed",
        `unsafe content entry ${String(path)}`,
      );
    const kind = entry.kind;
    if (
      (kind === "file" &&
        (entry.contentDigest === null ||
          entry.linkTarget !== null ||
          Number(entry.byteCount) < 0)) ||
      (kind === "directory" &&
        (entry.contentDigest !== null || entry.linkTarget !== null || entry.byteCount !== 0)) ||
      (kind === "symlink" &&
        (entry.contentDigest !== null ||
          entry.byteCount !== 0 ||
          typeof entry.linkTarget !== "string" ||
          !checkpointSymlinkStaysInWorkspace(path, entry.linkTarget)))
    )
      throw new CloudComputerCheckpointStoreError(
        "integrity_failed",
        `invalid content entry ${path}`,
      );
    if (kind === "file") plaintextByteCount += Number(entry.byteCount);
    seen.add(path);
    entries.push(entry as CloudComputerCheckpointEntry);
    previousPath = path;
  }
  return { plaintextByteCount, paths: seen, entries };
};
const operationFrom = (row: OperationRow): CloudCheckpointOperation => ({
  operationRef: row.operation_ref,
  requestDigest: row.request_digest,
  status: row.status,
  uploadSessionRef: row.upload_session_ref,
  uploadedByteCount: Number(row.uploaded_byte_count),
  objectRef: row.object_ref,
  objectGeneration: row.object_generation === null ? null : Number(row.object_generation),
  checkpointRef: row.checkpoint_ref,
  receipt:
    row.receipt_json === null
      ? null
      : { ...parse<CloudComputerCheckpointReceipt>(row.receipt_json, "receipt"), replayed: true },
});

export class PostgresCloudComputerCheckpointStore {
  constructor(private readonly sql: SyncSql) {}

  private async serializable<A>(run: (tx: SyncTransactionSql) => Promise<A>): Promise<A> {
    for (let attempt = 0; attempt < 32; attempt += 1) {
      try {
        // eslint-disable-next-line no-await-in-loop -- each retry observes the prior SQLSTATE.
        return await this.sql.begin("isolation level serializable", run);
      } catch (error) {
        const code =
          typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
        if (code !== "40001" || attempt === 31) throw error;
      }
    }
    throw new CloudComputerCheckpointStoreError("corrupt_store", "serialization retry exhausted");
  }

  private lock(tx: SyncTransactionSql, workspaceRef: string): Promise<unknown> {
    return tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${LOCK}|${workspaceRef}`}, 3105))`;
  }

  async initializeWorkspace(input: InitializeCloudComputerWorkspaceInput): Promise<void> {
    for (const [field, value] of Object.entries({
      workspaceRef: input.workspaceRef,
      computerRef: input.computerRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      conversationRef: input.conversationRef,
      baseImageSignatureRef: input.baseImageSignatureRef,
      workspaceKeyRef: input.workspaceKeyRef,
    }))
      assertRef(value, field);
    assertDigest(input.baseImageDigest, "base image digest");
    assertInteger(input.runtimeGeneration, "runtime generation");
    assertInteger(input.workspaceKeyVersion, "workspace key version");
    instant(input.createdAt, "created at");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const computers: ReadonlyArray<{
        readonly computer_ref: string;
        readonly generation: string | number;
        readonly owner_ref: string;
        readonly tenant_ref: string;
        readonly conversation_ref: string;
      }> = await tx`
        SELECT computer_ref, generation, owner_ref, tenant_ref, conversation_ref
        FROM khala_sync_cloud_computers WHERE computer_ref = ${input.computerRef} FOR UPDATE
      `;
      const computer = computers[0];
      if (computer === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "computer is missing");
      if (
        Number(computer.generation) !== input.runtimeGeneration ||
        computer.owner_ref !== input.ownerRef ||
        computer.tenant_ref !== input.tenantRef ||
        computer.conversation_ref !== input.conversationRef
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "computer scope differs");
      const rows: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${input.workspaceRef} OR computer_ref = ${input.computerRef}
        FOR UPDATE
      `;
      if (rows[0] !== undefined) {
        const row = rows[0];
        if (
          row.workspace_ref !== input.workspaceRef ||
          row.computer_ref !== input.computerRef ||
          Number(row.runtime_generation) !== input.runtimeGeneration ||
          row.owner_ref !== input.ownerRef ||
          row.tenant_ref !== input.tenantRef ||
          row.base_image_digest !== input.baseImageDigest ||
          row.base_image_signature_ref !== input.baseImageSignatureRef ||
          row.workspace_key_ref !== input.workspaceKeyRef ||
          Number(row.workspace_key_version) !== input.workspaceKeyVersion
        )
          throw new CloudComputerCheckpointStoreError("conflict", "workspace bytes conflict");
        return;
      }
      await tx`
        INSERT INTO khala_sync_cloud_computer_workspaces
          (workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
           conversation_ref, base_image_digest, base_image_signature_ref,
           workspace_key_ref, workspace_key_version, created_at, updated_at)
        VALUES
          (${input.workspaceRef}, ${input.computerRef}, ${input.runtimeGeneration},
           ${input.ownerRef}, ${input.tenantRef}, ${input.conversationRef},
           ${input.baseImageDigest}, ${input.baseImageSignatureRef},
           ${input.workspaceKeyRef}, ${input.workspaceKeyVersion},
           ${input.createdAt}, ${input.createdAt})
      `;
    });
  }

  async advanceRuntimeGeneration(
    input: Readonly<{
      workspaceRef: string;
      ownerRef: string;
      tenantRef: string;
      expectedRuntimeGeneration: number;
      nextRuntimeGeneration: number;
      observedAt: string;
    }>,
  ): Promise<void> {
    assertRef(input.workspaceRef, "workspace ref");
    assertRef(input.ownerRef, "owner ref");
    assertRef(input.tenantRef, "tenant ref");
    assertInteger(input.expectedRuntimeGeneration, "expected runtime generation");
    assertInteger(input.nextRuntimeGeneration, "next runtime generation");
    if (input.nextRuntimeGeneration <= input.expectedRuntimeGeneration)
      throw new CloudComputerCheckpointStoreError("invalid", "runtime generation must advance");
    instant(input.observedAt, "observed at");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const computers: ReadonlyArray<{
        readonly generation: string | number;
        readonly owner_ref: string;
        readonly tenant_ref: string;
      }> = await tx`
        SELECT computer.generation, computer.owner_ref, computer.tenant_ref
        FROM khala_sync_cloud_computers AS computer
        JOIN khala_sync_cloud_computer_workspaces AS workspace
          ON workspace.computer_ref = computer.computer_ref
        WHERE workspace.workspace_ref = ${input.workspaceRef}
        FOR UPDATE OF computer
      `;
      const computer = computers[0];
      if (
        computer === undefined ||
        computer.owner_ref !== input.ownerRef ||
        computer.tenant_ref !== input.tenantRef
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "computer scope differs");
      if (Number(computer.generation) !== input.nextRuntimeGeneration)
        throw new CloudComputerCheckpointStoreError(
          "stale_generation",
          "computer generation differs",
        );
      const rows: ReadonlyArray<{ readonly workspace_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_workspaces
        SET runtime_generation = ${input.nextRuntimeGeneration}, version = version + 1,
            updated_at = ${input.observedAt}
        WHERE workspace_ref = ${input.workspaceRef} AND owner_ref = ${input.ownerRef}
          AND tenant_ref = ${input.tenantRef} AND state = 'active'
          AND runtime_generation = ${input.expectedRuntimeGeneration}
        RETURNING workspace_ref
      `;
      if (rows.length !== 1)
        throw new CloudComputerCheckpointStoreError(
          "stale_generation",
          "runtime generation differs",
        );
    });
  }

  /** Authorizes a fork before any source bytes are read; the target always receives a new key. */
  async authorizeFork(
    input: Readonly<{
      forkRef: string;
      sourceWorkspaceRef: string;
      sourceCheckpointRef: string;
      targetWorkspace: InitializeCloudComputerWorkspaceInput;
      targetKeyAuthorization: CloudComputerWorkspaceKeyHandle;
      createdAt: string;
    }>,
  ): Promise<void> {
    try {
      assertCloudComputerWorkspaceKeyHandle(input.targetKeyAuthorization);
    } catch {
      throw new CloudComputerCheckpointStoreError(
        "permission_denied",
        "fork key authorization was not authority-issued",
      );
    }
    for (const [field, value] of Object.entries({
      forkRef: input.forkRef,
      sourceWorkspaceRef: input.sourceWorkspaceRef,
      sourceCheckpointRef: input.sourceCheckpointRef,
      actorRef: input.targetKeyAuthorization.actorRef,
      targetWorkspaceRef: input.targetWorkspace.workspaceRef,
    }))
      assertRef(value, field);
    if (
      input.targetKeyAuthorization.operation !== "fork" ||
      input.targetKeyAuthorization.ownerRef !== input.targetWorkspace.ownerRef ||
      input.targetKeyAuthorization.tenantRef !== input.targetWorkspace.tenantRef ||
      input.targetKeyAuthorization.workspaceRef !== input.targetWorkspace.workspaceRef ||
      input.targetKeyAuthorization.keyRef !== input.targetWorkspace.workspaceKeyRef ||
      input.targetKeyAuthorization.keyVersion !== input.targetWorkspace.workspaceKeyVersion
    )
      throw new CloudComputerCheckpointStoreError(
        "permission_denied",
        "fork key authorization differs",
      );
    const authorizationDigest = jsonDigest({
      operation: input.targetKeyAuthorization.operation,
      actorRef: input.targetKeyAuthorization.actorRef,
      ownerRef: input.targetKeyAuthorization.ownerRef,
      tenantRef: input.targetKeyAuthorization.tenantRef,
      workspaceRef: input.targetKeyAuthorization.workspaceRef,
      keyRef: input.targetKeyAuthorization.keyRef,
      keyVersion: input.targetKeyAuthorization.keyVersion,
      sourceWorkspaceRef: input.sourceWorkspaceRef,
      sourceCheckpointRef: input.sourceCheckpointRef,
    });
    instant(input.createdAt, "created at");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.sourceWorkspaceRef);
      await this.lock(tx, input.targetWorkspace.workspaceRef);
      const sources: ReadonlyArray<{
        readonly workspace_ref: string;
        readonly owner_ref: string;
        readonly tenant_ref: string;
        readonly workspace_key_ref: string;
        readonly checkpoint_ref: string;
        readonly base_image_digest: string;
      }> = await tx`
        SELECT workspace.workspace_ref, workspace.owner_ref, workspace.tenant_ref,
               workspace.workspace_key_ref, checkpoint.checkpoint_ref,
               checkpoint.base_image_digest
        FROM khala_sync_cloud_computer_workspaces AS workspace
        JOIN khala_sync_cloud_computer_checkpoints AS checkpoint
          ON checkpoint.workspace_ref = workspace.workspace_ref
        WHERE workspace.workspace_ref = ${input.sourceWorkspaceRef}
          AND checkpoint.checkpoint_ref = ${input.sourceCheckpointRef}
          AND checkpoint.status IN ('committed', 'superseded')
        FOR UPDATE OF workspace, checkpoint
      `;
      const source = sources[0];
      if (source === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "fork source is missing");
      if (
        source.owner_ref !== input.targetWorkspace.ownerRef ||
        source.tenant_ref !== input.targetWorkspace.tenantRef
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "fork scope differs");
      if (
        source.workspace_key_ref === input.targetWorkspace.workspaceKeyRef ||
        source.base_image_digest !== input.targetWorkspace.baseImageDigest
      )
        throw new CloudComputerCheckpointStoreError(
          "integrity_failed",
          "fork target key or image binding differs",
        );
      const computers: ReadonlyArray<{
        readonly computer_ref: string;
        readonly generation: string | number;
        readonly owner_ref: string;
        readonly tenant_ref: string;
        readonly conversation_ref: string;
      }> = await tx`
        SELECT computer_ref, generation, owner_ref, tenant_ref, conversation_ref
        FROM khala_sync_cloud_computers
        WHERE computer_ref = ${input.targetWorkspace.computerRef}
        FOR UPDATE
      `;
      const computer = computers[0];
      if (
        computer === undefined ||
        Number(computer.generation) !== input.targetWorkspace.runtimeGeneration ||
        computer.owner_ref !== input.targetWorkspace.ownerRef ||
        computer.tenant_ref !== input.targetWorkspace.tenantRef ||
        computer.conversation_ref !== input.targetWorkspace.conversationRef
      )
        throw new CloudComputerCheckpointStoreError(
          "permission_denied",
          "fork target computer scope differs",
        );
      await tx`
        INSERT INTO khala_sync_cloud_computer_workspaces
          (workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
           conversation_ref, base_image_digest, base_image_signature_ref,
           workspace_key_ref, workspace_key_version, created_at, updated_at)
        VALUES
          (${input.targetWorkspace.workspaceRef}, ${input.targetWorkspace.computerRef},
           ${input.targetWorkspace.runtimeGeneration}, ${input.targetWorkspace.ownerRef},
           ${input.targetWorkspace.tenantRef}, ${input.targetWorkspace.conversationRef},
           ${input.targetWorkspace.baseImageDigest}, ${input.targetWorkspace.baseImageSignatureRef},
           ${input.targetWorkspace.workspaceKeyRef}, ${input.targetWorkspace.workspaceKeyVersion},
           ${input.createdAt}, ${input.createdAt})
        ON CONFLICT (workspace_ref) DO NOTHING
      `;
      const targets: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${input.targetWorkspace.workspaceRef}
        FOR UPDATE
      `;
      const target = targets[0];
      if (
        target === undefined ||
        target.computer_ref !== input.targetWorkspace.computerRef ||
        Number(target.runtime_generation) !== input.targetWorkspace.runtimeGeneration ||
        target.owner_ref !== input.targetWorkspace.ownerRef ||
        target.tenant_ref !== input.targetWorkspace.tenantRef ||
        target.workspace_key_ref !== input.targetWorkspace.workspaceKeyRef ||
        Number(target.workspace_key_version) !== input.targetWorkspace.workspaceKeyVersion ||
        target.current_checkpoint_ref !== null
      )
        throw new CloudComputerCheckpointStoreError("conflict", "fork target workspace differs");
      const forks: ReadonlyArray<{ readonly fork_ref: string }> = await tx`
        INSERT INTO khala_sync_cloud_computer_workspace_forks
          (fork_ref, source_workspace_ref, source_checkpoint_ref, target_workspace_ref,
           owner_ref, tenant_ref, actor_ref, authorization_digest, state, created_at)
        VALUES
          (${input.forkRef}, ${input.sourceWorkspaceRef}, ${input.sourceCheckpointRef},
           ${input.targetWorkspace.workspaceRef}, ${input.targetWorkspace.ownerRef},
           ${input.targetWorkspace.tenantRef}, ${input.targetKeyAuthorization.actorRef},
           ${authorizationDigest}, 'authorized', ${input.createdAt})
        ON CONFLICT (fork_ref) DO UPDATE SET fork_ref = EXCLUDED.fork_ref
        WHERE khala_sync_cloud_computer_workspace_forks.authorization_digest = EXCLUDED.authorization_digest
          AND khala_sync_cloud_computer_workspace_forks.source_checkpoint_ref = EXCLUDED.source_checkpoint_ref
          AND khala_sync_cloud_computer_workspace_forks.target_workspace_ref = EXCLUDED.target_workspace_ref
        RETURNING fork_ref
      `;
      if (forks.length !== 1)
        throw new CloudComputerCheckpointStoreError("conflict", "fork bytes conflict");
      await tx`
        INSERT INTO khala_sync_cloud_checkpoint_references
          (reference_ref, workspace_ref, checkpoint_ref, source_checkpoint_ref,
           kind, state, created_at)
        VALUES
          (${`reference.fork-source.${input.forkRef}`}, ${input.targetWorkspace.workspaceRef},
           ${input.sourceCheckpointRef}, ${input.sourceCheckpointRef},
           'fork_source', 'live', ${input.createdAt})
        ON CONFLICT (reference_ref) DO NOTHING
      `;
    });
  }

  async completeFork(
    input: Readonly<{
      forkRef: string;
      targetCheckpointRef: string;
      completedAt: string;
    }>,
  ): Promise<void> {
    instant(input.completedAt, "completed at");
    await this.serializable(async (tx) => {
      const rows: ReadonlyArray<{
        readonly fork_ref: string;
        readonly target_workspace_ref: string;
        readonly source_checkpoint_ref: string;
      }> = await tx`
        UPDATE khala_sync_cloud_computer_workspace_forks AS fork
        SET state = 'resealed', resealed_checkpoint_ref = ${input.targetCheckpointRef},
            completed_at = ${input.completedAt}
        FROM khala_sync_cloud_computer_checkpoints AS checkpoint,
             khala_sync_cloud_computer_checkpoints AS source
        WHERE fork.fork_ref = ${input.forkRef} AND fork.state = 'authorized'
          AND checkpoint.checkpoint_ref = ${input.targetCheckpointRef}
          AND checkpoint.workspace_ref = fork.target_workspace_ref
          AND checkpoint.status = 'committed'
          AND checkpoint.parent_checkpoint_ref IS NULL
          AND source.checkpoint_ref = fork.source_checkpoint_ref
          AND checkpoint.workspace_state_digest = source.workspace_state_digest
          AND checkpoint.base_image_digest = source.base_image_digest
        RETURNING fork.fork_ref, fork.target_workspace_ref, fork.source_checkpoint_ref
      `;
      const fork = rows[0];
      if (fork === undefined)
        throw new CloudComputerCheckpointStoreError("conflict", "fork is not ready to complete");
      await tx`
        UPDATE khala_sync_cloud_checkpoint_references
        SET state = 'released', released_at = ${input.completedAt}
        WHERE reference_ref = ${`reference.fork-source.${input.forkRef}`}
          AND workspace_ref = ${fork.target_workspace_ref}
          AND source_checkpoint_ref = ${fork.source_checkpoint_ref}
          AND kind = 'fork_source' AND state = 'live'
      `;
    });
  }

  async prepare(input: PrepareCloudCheckpointInput): Promise<CloudCheckpointOperation> {
    for (const [field, value] of Object.entries({
      operationRef: input.operationRef,
      idempotencyRef: input.idempotencyRef,
      workspaceRef: input.workspaceRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      computerRef: input.computerRef,
    }))
      assertRef(value, field);
    assertDigest(input.requestDigest, "request digest");
    assertInteger(input.expectedRuntimeGeneration, "runtime generation");
    assertInteger(input.expectedWorkspaceRevision, "workspace revision", true);
    instant(input.createdAt, "created at");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const replay: ReadonlyArray<OperationRow> = await tx`
        SELECT operation_ref, request_digest, workspace_ref, owner_ref, tenant_ref,
               computer_ref, expected_runtime_generation, expected_workspace_revision,
               expected_parent_checkpoint_ref, boundary, status, upload_session_ref,
               uploaded_byte_count, object_ref, object_generation, checkpoint_ref, receipt_json
        FROM khala_sync_cloud_checkpoint_operations
        WHERE workspace_ref = ${input.workspaceRef} AND idempotency_ref = ${input.idempotencyRef}
        FOR UPDATE
      `;
      if (replay[0] !== undefined) {
        if (replay[0].request_digest !== input.requestDigest)
          throw new CloudComputerCheckpointStoreError("conflict", "idempotency bytes conflict");
        return operationFrom(replay[0]);
      }
      const workspaces: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${input.workspaceRef} FOR UPDATE
      `;
      const workspace = workspaces[0];
      if (workspace === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "workspace is missing");
      if (
        workspace.owner_ref !== input.ownerRef ||
        workspace.tenant_ref !== input.tenantRef ||
        workspace.computer_ref !== input.computerRef
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "workspace scope differs");
      if (
        Number(workspace.runtime_generation) !== input.expectedRuntimeGeneration ||
        Number(workspace.workspace_revision) !== input.expectedWorkspaceRevision ||
        workspace.current_checkpoint_ref !== input.expectedParentCheckpointRef
      )
        throw new CloudComputerCheckpointStoreError("stale_generation", "workspace CAS differs");
      if (workspace.state !== "active")
        throw new CloudComputerCheckpointStoreError("conflict", "workspace is not active");
      await tx`
        INSERT INTO khala_sync_cloud_checkpoint_operations
          (operation_ref, idempotency_ref, request_digest, workspace_ref, owner_ref,
           tenant_ref, computer_ref, expected_runtime_generation,
           expected_workspace_revision, expected_parent_checkpoint_ref, boundary,
           status, created_at, updated_at)
        VALUES
          (${input.operationRef}, ${input.idempotencyRef}, ${input.requestDigest},
           ${input.workspaceRef}, ${input.ownerRef}, ${input.tenantRef},
           ${input.computerRef}, ${input.expectedRuntimeGeneration},
           ${input.expectedWorkspaceRevision}, ${input.expectedParentCheckpointRef},
           ${input.boundary}, 'prepared', ${input.createdAt}, ${input.createdAt})
      `;
      return {
        operationRef: input.operationRef,
        requestDigest: input.requestDigest,
        status: "prepared",
        uploadSessionRef: null,
        uploadedByteCount: 0,
        objectRef: null,
        objectGeneration: null,
        checkpointRef: null,
        receipt: null,
      };
    });
  }

  async recordUploadProgress(
    input: Readonly<{
      operationRef: string;
      requestDigest: string;
      uploadSessionRef: string;
      uploadedByteCount: number;
      uncertain: boolean;
      observedAt: string;
    }>,
  ): Promise<void> {
    assertRef(input.operationRef, "operation ref");
    assertRef(input.uploadSessionRef, "upload session ref");
    assertDigest(input.requestDigest, "request digest");
    assertInteger(input.uploadedByteCount, "uploaded byte count", true);
    instant(input.observedAt, "observed at");
    const rows: ReadonlyArray<{ readonly operation_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_checkpoint_operations
      SET status = ${input.uncertain ? "upload_uncertain" : "uploading"},
          upload_session_ref = COALESCE(upload_session_ref, ${input.uploadSessionRef}),
          uploaded_byte_count = GREATEST(uploaded_byte_count, ${input.uploadedByteCount}),
          updated_at = ${input.observedAt}, revision = revision + 1
      WHERE operation_ref = ${input.operationRef} AND request_digest = ${input.requestDigest}
        AND status IN ('prepared', 'uploading', 'upload_uncertain')
        AND (upload_session_ref IS NULL OR upload_session_ref = ${input.uploadSessionRef})
      RETURNING operation_ref
    `;
    if (rows.length !== 1)
      throw new CloudComputerCheckpointStoreError("conflict", "upload progress differs");
  }

  async recordVerifiedObject(input: RecordVerifiedCloudCheckpointObjectInput): Promise<void> {
    for (const [field, value] of Object.entries({
      operationRef: input.operationRef,
      objectRef: input.objectRef,
      workspaceRef: input.workspaceRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      objectUri: input.objectUri,
      workspaceKeyRef: input.workspaceKeyRef,
      wrappedDekRef: input.wrappedDekRef,
    }))
      assertRef(value, field);
    for (const [field, value] of Object.entries({
      requestDigest: input.requestDigest,
      contentDigest: input.contentDigest,
      contentManifestDigest: input.contentManifestDigest,
      storageManifestDigest: input.storageManifestDigest,
      ciphertextDigest: input.ciphertextDigest,
    }))
      assertDigest(value, field);
    assertInteger(input.objectGeneration, "object generation");
    assertInteger(input.workspaceKeyVersion, "workspace key version");
    assertInteger(input.encryptedByteCount, "encrypted byte count");
    const createdAt = instant(input.createdAt, "created at");
    const verifiedAt = instant(input.verifiedAt, "verified at");
    if (verifiedAt < createdAt || instant(input.retainUntil, "retain until") < verifiedAt)
      throw new CloudComputerCheckpointStoreError("invalid", "object timestamps are invalid");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const operations: ReadonlyArray<OperationRow> = await tx`
        SELECT operation_ref, request_digest, workspace_ref, owner_ref, tenant_ref,
               computer_ref, expected_runtime_generation, expected_workspace_revision,
               expected_parent_checkpoint_ref, boundary, status, upload_session_ref,
               uploaded_byte_count, object_ref, object_generation, checkpoint_ref, receipt_json
        FROM khala_sync_cloud_checkpoint_operations
        WHERE operation_ref = ${input.operationRef} FOR UPDATE
      `;
      const operation = operations[0];
      if (operation === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "operation is missing");
      if (operation.request_digest !== input.requestDigest)
        throw new CloudComputerCheckpointStoreError("conflict", "operation bytes conflict");
      if (
        operation.workspace_ref !== input.workspaceRef ||
        operation.owner_ref !== input.ownerRef ||
        operation.tenant_ref !== input.tenantRef
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "object scope differs");
      const workspaces: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${input.workspaceRef} FOR UPDATE
      `;
      const workspace = workspaces[0]!;
      if (
        workspace.workspace_key_ref !== input.workspaceKeyRef ||
        Number(workspace.workspace_key_version) !== input.workspaceKeyVersion
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "workspace key differs");
      const existing: ReadonlyArray<{
        readonly object_ref: string;
        readonly operation_ref: string;
      }> = await tx`
          INSERT INTO khala_sync_cloud_checkpoint_objects
            (object_ref, workspace_ref, operation_ref, owner_ref, tenant_ref, object_uri,
             object_generation, content_digest, content_manifest_digest,
             storage_manifest_digest, ciphertext_digest, crc32c, workspace_key_ref,
             workspace_key_version, wrapped_dek_ref, encrypted_byte_count, state,
             created_at, verified_at, retain_until)
          VALUES
            (${input.objectRef}, ${input.workspaceRef}, ${input.operationRef},
             ${input.ownerRef}, ${input.tenantRef}, ${input.objectUri},
             ${input.objectGeneration}, ${input.contentDigest},
             ${input.contentManifestDigest}, ${input.storageManifestDigest},
             ${input.ciphertextDigest}, ${input.crc32c}, ${input.workspaceKeyRef},
             ${input.workspaceKeyVersion}, ${input.wrappedDekRef},
             ${input.encryptedByteCount}, 'verified', ${input.createdAt},
             ${input.verifiedAt}, ${input.retainUntil})
          ON CONFLICT (operation_ref) DO UPDATE SET operation_ref = EXCLUDED.operation_ref
          WHERE khala_sync_cloud_checkpoint_objects.object_ref = EXCLUDED.object_ref
            AND khala_sync_cloud_checkpoint_objects.object_generation = EXCLUDED.object_generation
            AND khala_sync_cloud_checkpoint_objects.content_manifest_digest = EXCLUDED.content_manifest_digest
            AND khala_sync_cloud_checkpoint_objects.storage_manifest_digest = EXCLUDED.storage_manifest_digest
            AND khala_sync_cloud_checkpoint_objects.ciphertext_digest = EXCLUDED.ciphertext_digest
          RETURNING object_ref, operation_ref
        `;
      if (existing.length !== 1)
        throw new CloudComputerCheckpointStoreError("integrity_failed", "object binding differs");
      await tx`
        UPDATE khala_sync_cloud_checkpoint_operations
        SET status = 'commit_ready', object_ref = ${input.objectRef},
            object_generation = ${input.objectGeneration},
            content_manifest_digest = ${input.contentManifestDigest},
            storage_manifest_digest = ${input.storageManifestDigest},
            uploaded_byte_count = ${input.encryptedByteCount},
            updated_at = ${input.verifiedAt}, revision = revision + 1
        WHERE operation_ref = ${input.operationRef}
          AND status IN ('prepared', 'uploading', 'upload_uncertain', 'uploaded', 'verifying', 'commit_ready')
      `;
    });
  }

  async commit(input: CommitCloudComputerCheckpointInput): Promise<CloudComputerCheckpointReceipt> {
    for (const [field, value] of Object.entries({
      operationRef: input.operationRef,
      checkpointRef: input.checkpointRef,
      objectRef: input.objectRef,
      retentionPolicyRef: input.retentionPolicyRef,
    }))
      assertRef(value, field);
    for (const [field, value] of Object.entries({
      requestDigest: input.requestDigest,
      contentDigest: input.contentDigest,
      workspaceStateDigest: input.workspaceStateDigest,
      contentManifestDigest: input.contentManifestDigest,
      storageManifestDigest: input.storageManifestDigest,
      baseImageDigest: input.baseImageDigest,
    }))
      assertDigest(value, field);
    assertInteger(input.plaintextByteCount, "plaintext byte count", true);
    assertInteger(input.encryptedByteCount, "encrypted byte count");
    const verifiedAt = instant(input.verifiedAt, "verified at");
    const committedAt = instant(input.committedAt, "committed at");
    if (committedAt < verifiedAt || instant(input.retainUntil, "retain until") < committedAt)
      throw new CloudComputerCheckpointStoreError("invalid", "checkpoint timestamps are invalid");
    if (jsonDigest(input.contentManifest) !== input.contentManifestDigest)
      throw new CloudComputerCheckpointStoreError("integrity_failed", "content manifest differs");
    if (jsonDigest(input.storageManifest) !== input.storageManifestDigest)
      throw new CloudComputerCheckpointStoreError("integrity_failed", "storage manifest differs");
    if (
      (input.checkpointKind === "full" && input.deletedPaths.length !== 0) ||
      input.deletedPaths.some((path) => path.startsWith("/") || path.includes(".."))
    )
      throw new CloudComputerCheckpointStoreError("invalid", "deleted paths are invalid");
    return this.serializable(async (tx) => {
      const scopes: ReadonlyArray<{ readonly workspace_ref: string }> = await tx`
        SELECT workspace_ref FROM khala_sync_cloud_checkpoint_operations
        WHERE operation_ref = ${input.operationRef}
      `;
      if (scopes[0] === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "operation is missing");
      await this.lock(tx, scopes[0].workspace_ref);
      const operations: ReadonlyArray<OperationRow> = await tx`
        SELECT operation_ref, request_digest, workspace_ref, owner_ref, tenant_ref,
               computer_ref, expected_runtime_generation, expected_workspace_revision,
               expected_parent_checkpoint_ref, boundary, status, upload_session_ref,
               uploaded_byte_count, object_ref, object_generation, checkpoint_ref, receipt_json
        FROM khala_sync_cloud_checkpoint_operations
        WHERE operation_ref = ${input.operationRef} FOR UPDATE
      `;
      const operation = operations[0];
      if (operation === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "operation is missing");
      if (operation.request_digest !== input.requestDigest)
        throw new CloudComputerCheckpointStoreError("conflict", "operation bytes conflict");
      if (operation.status === "committed" && operation.receipt_json !== null) {
        return {
          ...parse<CloudComputerCheckpointReceipt>(operation.receipt_json, "receipt"),
          replayed: true,
        };
      }
      if (
        operation.status !== "commit_ready" ||
        operation.object_ref !== input.objectRef ||
        operation.object_generation === null
      )
        throw new CloudComputerCheckpointStoreError("conflict", "operation is not commit ready");
      const workspaces: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${operation.workspace_ref} FOR UPDATE
      `;
      const workspace = workspaces[0]!;
      if (
        Number(workspace.runtime_generation) !== Number(operation.expected_runtime_generation) ||
        Number(workspace.workspace_revision) !== Number(operation.expected_workspace_revision) ||
        workspace.current_checkpoint_ref !== operation.expected_parent_checkpoint_ref
      )
        throw new CloudComputerCheckpointStoreError("stale_generation", "workspace CAS differs");
      if (workspace.state !== "active" || workspace.base_image_digest !== input.baseImageDigest)
        throw new CloudComputerCheckpointStoreError(
          "integrity_failed",
          "workspace binding differs",
        );
      const computers: ReadonlyArray<{
        readonly generation: string | number;
        readonly owner_ref: string;
        readonly tenant_ref: string;
      }> = await tx`
        SELECT generation, owner_ref, tenant_ref
        FROM khala_sync_cloud_computers
        WHERE computer_ref = ${workspace.computer_ref}
        FOR UPDATE
      `;
      const computer = computers[0];
      if (
        computer === undefined ||
        Number(computer.generation) !== Number(operation.expected_runtime_generation)
      )
        throw new CloudComputerCheckpointStoreError(
          "stale_generation",
          "computer generation differs",
        );
      if (
        computer.owner_ref !== workspace.owner_ref ||
        computer.tenant_ref !== workspace.tenant_ref
      )
        throw new CloudComputerCheckpointStoreError("permission_denied", "computer scope differs");
      const objects: ReadonlyArray<{
        readonly object_ref: string;
        readonly object_generation: string | number;
        readonly content_digest: string;
        readonly content_manifest_digest: string;
        readonly storage_manifest_digest: string;
        readonly ciphertext_digest: string;
        readonly workspace_key_ref: string;
        readonly workspace_key_version: string | number;
        readonly wrapped_dek_ref: string;
        readonly encrypted_byte_count: string | number;
        readonly retain_until: string | Date;
        readonly state: string;
        readonly verified_at: string | Date;
      }> = await tx`
        SELECT object_ref, object_generation, content_digest, content_manifest_digest,
               storage_manifest_digest, ciphertext_digest, workspace_key_ref,
               workspace_key_version, wrapped_dek_ref, encrypted_byte_count,
               retain_until, state, verified_at
        FROM khala_sync_cloud_checkpoint_objects
        WHERE object_ref = ${input.objectRef} FOR UPDATE
      `;
      const object = objects[0];
      if (
        object === undefined ||
        object.state !== "verified" ||
        object.content_digest !== input.contentDigest ||
        object.content_manifest_digest !== input.contentManifestDigest ||
        object.storage_manifest_digest !== input.storageManifestDigest ||
        Number(object.encrypted_byte_count) !== input.encryptedByteCount
      )
        throw new CloudComputerCheckpointStoreError("integrity_failed", "object binding differs");
      const contentManifest = record(input.contentManifest, "content manifest");
      const storageManifest = record(input.storageManifest, "storage manifest");
      assertExactFields(
        contentManifest,
        [
          "schema",
          "checkpointRef",
          "operationRef",
          "requestDigest",
          "ownerRef",
          "tenantRef",
          "workspaceRef",
          "computerRef",
          "sourceRuntimeGeneration",
          "expectedWorkspaceRevision",
          "parentCheckpointRef",
          "baseImageDigest",
          "checkpointKind",
          "workspaceKeyRef",
          "workspaceKeyVersion",
          "entries",
          "excludedPaths",
          "deletedPaths",
          "plaintextByteCount",
          "contentDigest",
          "workspaceStateDigest",
          "retainUntil",
        ],
        "content manifest",
      );
      const validatedEntries = validateContentEntries(contentManifest.entries);
      if (validatedEntries.plaintextByteCount !== input.plaintextByteCount)
        throw new CloudComputerCheckpointStoreError(
          "integrity_failed",
          "content entry byte count differs",
        );
      let previousDeletion = "";
      for (const deletedPath of input.deletedPaths) {
        if (
          !checkpointPathAdmitted(deletedPath) ||
          deletedPath <= previousDeletion ||
          validatedEntries.paths.has(deletedPath)
        )
          throw new CloudComputerCheckpointStoreError(
            "integrity_failed",
            `invalid deleted path ${deletedPath}`,
          );
        previousDeletion = deletedPath;
      }
      assertManifestField(contentManifest, "excludedPaths", [
        ...CLOUD_COMPUTER_CHECKPOINT_REQUIRED_EXCLUSIONS,
      ]);
      for (const [field, expected] of Object.entries({
        schema: "openagents.cloud_computer_checkpoint_content.v1",
        checkpointRef: input.checkpointRef,
        operationRef: input.operationRef,
        requestDigest: input.requestDigest,
        ownerRef: workspace.owner_ref,
        tenantRef: workspace.tenant_ref,
        workspaceRef: workspace.workspace_ref,
        computerRef: workspace.computer_ref,
        sourceRuntimeGeneration: Number(operation.expected_runtime_generation),
        expectedWorkspaceRevision: Number(operation.expected_workspace_revision),
        parentCheckpointRef: operation.expected_parent_checkpoint_ref,
        baseImageDigest: input.baseImageDigest,
        checkpointKind: input.checkpointKind,
        workspaceKeyRef: workspace.workspace_key_ref,
        workspaceKeyVersion: Number(workspace.workspace_key_version),
        deletedPaths: input.deletedPaths,
        plaintextByteCount: input.plaintextByteCount,
        contentDigest: input.contentDigest,
        workspaceStateDigest: input.workspaceStateDigest,
        retainUntil: input.retainUntil,
      }))
        assertManifestField(contentManifest, field, expected);
      assertExactFields(
        storageManifest,
        [
          "schema",
          "operationRef",
          "requestDigest",
          "workspaceRef",
          "ownerRef",
          "tenantRef",
          "computerRef",
          "objectRef",
          "objectGeneration",
          "contentManifestDigest",
          "ciphertextDigest",
          "encryptedByteCount",
          "workspaceKeyRef",
          "workspaceKeyVersion",
          "wrappedDekRef",
          "retainUntil",
        ],
        "storage manifest",
      );
      for (const [field, expected] of Object.entries({
        schema: "openagents.cloud_computer_checkpoint_storage.v1",
        operationRef: input.operationRef,
        requestDigest: input.requestDigest,
        workspaceRef: workspace.workspace_ref,
        ownerRef: workspace.owner_ref,
        tenantRef: workspace.tenant_ref,
        computerRef: workspace.computer_ref,
        objectRef: input.objectRef,
        objectGeneration: Number(object.object_generation),
        contentManifestDigest: input.contentManifestDigest,
        ciphertextDigest: object.ciphertext_digest,
        encryptedByteCount: input.encryptedByteCount,
        workspaceKeyRef: object.workspace_key_ref,
        workspaceKeyVersion: Number(object.workspace_key_version),
        wrappedDekRef: object.wrapped_dek_ref,
        retainUntil: input.retainUntil,
      }))
        assertManifestField(storageManifest, field, expected);
      const ancestors: ReadonlyArray<{
        readonly checkpoint_kind: "full" | "delta";
        readonly workspace_state_digest: string;
        readonly content_manifest_json: unknown;
        readonly deleted_paths_json: unknown;
      }> =
        operation.expected_parent_checkpoint_ref === null
          ? []
          : await tx`
              WITH RECURSIVE checkpoint_chain AS (
                SELECT checkpoint.*
                FROM khala_sync_cloud_computer_checkpoints AS checkpoint
                WHERE checkpoint.checkpoint_ref = ${operation.expected_parent_checkpoint_ref}
                  AND checkpoint.workspace_ref = ${workspace.workspace_ref}
                UNION ALL
                SELECT parent.*
                FROM khala_sync_cloud_computer_checkpoints AS parent
                JOIN checkpoint_chain AS child
                  ON child.parent_checkpoint_ref = parent.checkpoint_ref
                WHERE parent.workspace_ref = ${workspace.workspace_ref}
              )
              SELECT checkpoint_kind, workspace_state_digest,
                     content_manifest_json, deleted_paths_json
              FROM checkpoint_chain
              ORDER BY workspace_revision
            `;
      if (ancestors.length !== 0 && ancestors[0]?.checkpoint_kind !== "full")
        throw new CloudComputerCheckpointStoreError(
          "corrupt_store",
          "checkpoint ancestry has no full base",
        );
      const materialized = new Map<string, CloudComputerCheckpointEntry>();
      const applyLayer = (
        entries: ReadonlyArray<CloudComputerCheckpointEntry>,
        deletions: ReadonlyArray<string>,
      ): void => {
        for (const deletion of deletions) {
          for (const path of materialized.keys()) {
            if (path === deletion || path.startsWith(`${deletion}/`)) materialized.delete(path);
          }
        }
        for (const entry of entries) materialized.set(entry.path, entry);
      };
      for (const ancestor of ancestors) {
        const ancestorManifest = record(
          ancestor.content_manifest_json,
          "ancestor content manifest",
        );
        const ancestorEntries = validateContentEntries(ancestorManifest.entries).entries;
        const ancestorDeletions = parse<ReadonlyArray<string>>(
          ancestor.deleted_paths_json,
          "ancestor deleted paths",
        );
        applyLayer(ancestorEntries, ancestorDeletions);
        const ordered: CloudComputerCheckpointEntry[] = [];
        for (const entry of materialized.values()) {
          const index = ordered.findIndex((candidate) => candidate.path > entry.path);
          if (index === -1) ordered.push(entry);
          else ordered.splice(index, 0, entry);
        }
        if (checkpointWorkspaceStateDigest(ordered) !== ancestor.workspace_state_digest)
          throw new CloudComputerCheckpointStoreError(
            "corrupt_store",
            "ancestor workspace state digest differs",
          );
      }
      applyLayer(validatedEntries.entries, input.deletedPaths);
      const resolvedEntries: CloudComputerCheckpointEntry[] = [];
      for (const entry of materialized.values()) {
        const index = resolvedEntries.findIndex((candidate) => candidate.path > entry.path);
        if (index === -1) resolvedEntries.push(entry);
        else resolvedEntries.splice(index, 0, entry);
      }
      if (checkpointWorkspaceStateDigest(resolvedEntries) !== input.workspaceStateDigest)
        throw new CloudComputerCheckpointStoreError(
          "integrity_failed",
          "workspace state digest differs from resolved entries",
        );
      const workspaceRevision = Number(operation.expected_workspace_revision) + 1;
      const receipt: CloudComputerCheckpointReceipt = {
        schema: "openagents.cloud_computer_checkpoint_receipt.v1",
        operationRef: input.operationRef,
        checkpointRef: input.checkpointRef,
        workspaceRef: workspace.workspace_ref,
        computerRef: workspace.computer_ref,
        sourceRuntimeGeneration: Number(operation.expected_runtime_generation),
        workspaceRevision,
        parentCheckpointRef: operation.expected_parent_checkpoint_ref,
        objectRef: input.objectRef,
        contentDigest: input.contentDigest,
        contentManifestDigest: input.contentManifestDigest,
        storageManifestDigest: input.storageManifestDigest,
        plaintextByteCount: input.plaintextByteCount,
        encryptedByteCount: input.encryptedByteCount,
        committedAt: input.committedAt,
        replayed: false,
      };
      if (operation.expected_parent_checkpoint_ref !== null) {
        await tx`
          UPDATE khala_sync_cloud_computer_checkpoints
          SET status = 'superseded', revision = revision + 1
          WHERE checkpoint_ref = ${operation.expected_parent_checkpoint_ref}
            AND workspace_ref = ${workspace.workspace_ref} AND status = 'committed'
        `;
        await tx`
          UPDATE khala_sync_cloud_checkpoint_references
          SET state = 'released', released_at = ${input.committedAt}
          WHERE workspace_ref = ${workspace.workspace_ref}
            AND kind = 'current_head' AND state = 'live'
        `;
      }
      await tx`
        INSERT INTO khala_sync_cloud_computer_checkpoints
          (checkpoint_ref, operation_ref, workspace_ref, owner_ref, tenant_ref,
           computer_ref, source_runtime_generation, workspace_revision,
           parent_checkpoint_ref, object_ref, content_digest, content_manifest_digest,
           storage_manifest_digest, workspace_state_digest, base_image_digest, checkpoint_kind,
           deleted_paths_json, plaintext_byte_count, encrypted_byte_count,
           retention_policy_ref, status, verified_at, committed_at, retain_until,
           content_manifest_json, storage_manifest_json)
        VALUES
          (${input.checkpointRef}, ${input.operationRef}, ${workspace.workspace_ref},
           ${workspace.owner_ref}, ${workspace.tenant_ref}, ${workspace.computer_ref},
           ${Number(operation.expected_runtime_generation)}, ${workspaceRevision},
           ${operation.expected_parent_checkpoint_ref}, ${input.objectRef},
           ${input.contentDigest}, ${input.contentManifestDigest},
           ${input.storageManifestDigest}, ${input.workspaceStateDigest}, ${input.baseImageDigest},
           ${input.checkpointKind}, ${input.deletedPaths}::jsonb,
           ${input.plaintextByteCount}, ${input.encryptedByteCount},
           ${input.retentionPolicyRef}, 'committed', ${input.verifiedAt},
           ${input.committedAt}, ${input.retainUntil}, ${input.contentManifest}::jsonb,
           ${input.storageManifest}::jsonb)
      `;
      await tx`
        INSERT INTO khala_sync_cloud_checkpoint_references
          (reference_ref, workspace_ref, checkpoint_ref, source_checkpoint_ref,
           kind, state, created_at)
        VALUES
          (${`reference.checkpoint-head.${input.checkpointRef}`}, ${workspace.workspace_ref},
           ${input.checkpointRef}, NULL, 'current_head', 'live', ${input.committedAt})
      `;
      if (operation.expected_parent_checkpoint_ref !== null) {
        await tx`
          INSERT INTO khala_sync_cloud_checkpoint_references
            (reference_ref, workspace_ref, checkpoint_ref, source_checkpoint_ref,
             kind, state, created_at)
          VALUES
            (${`reference.checkpoint-parent.${input.checkpointRef}`}, ${workspace.workspace_ref},
             ${input.checkpointRef}, ${operation.expected_parent_checkpoint_ref},
             'parent', 'live', ${input.committedAt})
        `;
      }
      await tx`
        UPDATE khala_sync_cloud_checkpoint_objects
        SET state = 'reachable', revision = revision + 1
        WHERE object_ref = ${input.objectRef} AND state = 'verified'
      `;
      const updatedWorkspaces: ReadonlyArray<{ readonly workspace_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_workspaces
        SET workspace_revision = ${workspaceRevision},
            current_checkpoint_ref = ${input.checkpointRef}, version = version + 1,
            updated_at = ${input.committedAt}
        WHERE workspace_ref = ${workspace.workspace_ref}
          AND runtime_generation = ${Number(operation.expected_runtime_generation)}
          AND workspace_revision = ${Number(operation.expected_workspace_revision)}
        RETURNING workspace_ref
      `;
      if (updatedWorkspaces.length !== 1)
        throw new CloudComputerCheckpointStoreError("stale_generation", "workspace CAS failed");
      const updatedComputers: ReadonlyArray<{ readonly computer_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computers
        SET latest_checkpoint_ref = ${input.checkpointRef}, version = version + 1,
            updated_at = ${input.committedAt}
        WHERE computer_ref = ${workspace.computer_ref}
          AND generation = ${Number(operation.expected_runtime_generation)}
        RETURNING computer_ref
      `;
      if (updatedComputers.length !== 1)
        throw new CloudComputerCheckpointStoreError("stale_generation", "computer CAS failed");
      const updatedOperations: ReadonlyArray<{ readonly operation_ref: string }> = await tx`
        UPDATE khala_sync_cloud_checkpoint_operations
        SET status = 'committed', checkpoint_ref = ${input.checkpointRef},
            result_digest = ${jsonDigest(receipt)}, receipt_json = ${receipt}::jsonb,
            completed_at = ${input.committedAt}, updated_at = ${input.committedAt},
            revision = revision + 1
        WHERE operation_ref = ${input.operationRef} AND status = 'commit_ready'
        RETURNING operation_ref
      `;
      if (updatedOperations.length !== 1)
        throw new CloudComputerCheckpointStoreError("conflict", "operation commit CAS failed");
      return receipt;
    });
  }

  /** Marks verified uploads orphaned after a generation or head CAS becomes stale. */
  async reconcileStaleOperations(
    input: Readonly<{ workspaceRef: string; observedAt: string }>,
  ): Promise<number> {
    assertRef(input.workspaceRef, "workspace ref");
    instant(input.observedAt, "observed at");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const operations: ReadonlyArray<{ readonly operation_ref: string }> = await tx`
        UPDATE khala_sync_cloud_checkpoint_operations AS operation
        SET status = 'orphaned', failure_code = 'stale_generation',
            updated_at = ${input.observedAt}, revision = operation.revision + 1
        FROM khala_sync_cloud_computer_workspaces AS workspace,
             khala_sync_cloud_computers AS computer
        WHERE operation.workspace_ref = ${input.workspaceRef}
          AND workspace.workspace_ref = operation.workspace_ref
          AND computer.computer_ref = workspace.computer_ref
          AND operation.status = 'commit_ready'
          AND (
            operation.expected_runtime_generation <> workspace.runtime_generation
            OR operation.expected_runtime_generation <> computer.generation
            OR operation.expected_workspace_revision <> workspace.workspace_revision
            OR operation.expected_parent_checkpoint_ref IS DISTINCT FROM workspace.current_checkpoint_ref
          )
        RETURNING operation.operation_ref
      `;
      if (operations.length !== 0) {
        const operationRefs = operations.map((operation) => operation.operation_ref);
        await tx`
          UPDATE khala_sync_cloud_checkpoint_objects
          SET state = 'tombstoned', tombstoned_at = COALESCE(tombstoned_at, ${input.observedAt}),
              revision = revision + 1
          WHERE operation_ref = ANY(${operationRefs}::text[]) AND state = 'verified'
        `;
      }
      return operations.length;
    });
  }

  async restorePlan(
    input: Readonly<{
      workspaceRef: string;
      checkpointRef?: string | undefined;
      ownerRef: string;
      tenantRef: string;
      expectedRuntimeGeneration: number;
    }>,
  ): Promise<CloudComputerCheckpointRestorePlan> {
    assertInteger(input.expectedRuntimeGeneration, "runtime generation");
    const rows: ReadonlyArray<{
      readonly workspace_ref: string;
      readonly checkpoint_ref: string;
      readonly workspace_revision: string | number;
      readonly object_ref: string;
      readonly object_generation: string | number;
      readonly content_digest: string;
      readonly content_manifest_digest: string;
      readonly storage_manifest_digest: string;
      readonly ciphertext_digest: string;
      readonly workspace_key_ref: string;
      readonly workspace_key_version: string | number;
      readonly base_image_digest: string;
      readonly checkpoint_kind: "full" | "delta";
      readonly deleted_paths_json: unknown;
      readonly encrypted_byte_count: string | number;
      readonly content_manifest_json: unknown;
      readonly storage_manifest_json: unknown;
    }> = await this.sql`
      WITH RECURSIVE checkpoint_chain AS (
        SELECT checkpoint.*
        FROM khala_sync_cloud_computer_workspaces AS workspace
        JOIN khala_sync_cloud_computers AS computer
          ON computer.computer_ref = workspace.computer_ref
        JOIN khala_sync_cloud_computer_checkpoints AS checkpoint
          ON checkpoint.checkpoint_ref = COALESCE(${input.checkpointRef ?? null}, workspace.current_checkpoint_ref)
        WHERE workspace.workspace_ref = ${input.workspaceRef}
          AND workspace.owner_ref = ${input.ownerRef}
          AND workspace.tenant_ref = ${input.tenantRef}
          AND workspace.runtime_generation = ${input.expectedRuntimeGeneration}
          AND computer.generation = ${input.expectedRuntimeGeneration}
          AND workspace.state = 'active'
          AND checkpoint.base_image_digest = workspace.base_image_digest
          AND checkpoint.status IN ('committed', 'superseded')
        UNION ALL
        SELECT parent.*
        FROM khala_sync_cloud_computer_checkpoints AS parent
        JOIN checkpoint_chain AS child ON child.parent_checkpoint_ref = parent.checkpoint_ref
        WHERE parent.workspace_ref = ${input.workspaceRef}
          AND parent.owner_ref = ${input.ownerRef}
          AND parent.tenant_ref = ${input.tenantRef}
          AND parent.status IN ('committed', 'superseded')
          AND parent.base_image_digest = child.base_image_digest
      )
      SELECT ${input.workspaceRef} AS workspace_ref, checkpoint.checkpoint_ref,
             checkpoint.workspace_revision, checkpoint.object_ref,
             object.object_generation, checkpoint.content_digest,
             checkpoint.content_manifest_digest, checkpoint.storage_manifest_digest,
             object.ciphertext_digest, object.workspace_key_ref,
             object.workspace_key_version, checkpoint.base_image_digest,
             checkpoint.checkpoint_kind, checkpoint.deleted_paths_json,
             checkpoint.encrypted_byte_count, checkpoint.content_manifest_json,
             checkpoint.storage_manifest_json
      FROM checkpoint_chain AS checkpoint
      JOIN khala_sync_cloud_checkpoint_objects AS object
        ON object.object_ref = checkpoint.object_ref
      WHERE object.state = 'reachable'
      ORDER BY checkpoint.workspace_revision
    `;
    const row = rows.at(-1);
    if (row === undefined || rows[0]?.checkpoint_kind !== "full")
      throw new CloudComputerCheckpointStoreError(
        row === undefined ? "permission_denied" : "corrupt_store",
        row === undefined
          ? "checkpoint is unavailable for this scope"
          : "checkpoint ancestry has no full base",
      );
    if (rows.some((layer) => layer.base_image_digest !== row.base_image_digest))
      throw new CloudComputerCheckpointStoreError(
        "corrupt_store",
        "checkpoint ancestry has inconsistent base images",
      );
    const layers: ReadonlyArray<CloudComputerCheckpointRestoreLayer> = rows.map((layer) => ({
      checkpointRef: layer.checkpoint_ref,
      workspaceRevision: Number(layer.workspace_revision),
      objectRef: layer.object_ref,
      objectGeneration: Number(layer.object_generation),
      contentDigest: layer.content_digest,
      contentManifestDigest: layer.content_manifest_digest,
      storageManifestDigest: layer.storage_manifest_digest,
      ciphertextDigest: layer.ciphertext_digest,
      checkpointKind: layer.checkpoint_kind,
      deletedPaths: parse(layer.deleted_paths_json, "deleted paths"),
      encryptedByteCount: Number(layer.encrypted_byte_count),
      contentManifest: parse(layer.content_manifest_json, "content manifest"),
      storageManifest: parse(layer.storage_manifest_json, "storage manifest"),
    }));
    return {
      schema: "openagents.cloud_computer_restore_plan.v1",
      workspaceRef: row.workspace_ref,
      checkpointRef: row.checkpoint_ref,
      workspaceRevision: Number(row.workspace_revision),
      objectRef: row.object_ref,
      objectGeneration: Number(row.object_generation),
      contentDigest: row.content_digest,
      contentManifestDigest: row.content_manifest_digest,
      storageManifestDigest: row.storage_manifest_digest,
      ciphertextDigest: row.ciphertext_digest,
      workspaceKeyRef: row.workspace_key_ref,
      workspaceKeyVersion: Number(row.workspace_key_version),
      baseImageDigest: row.base_image_digest,
      checkpointKind: row.checkpoint_kind,
      deletedPaths: parse(row.deleted_paths_json, "deleted paths"),
      encryptedByteCount: Number(row.encrypted_byte_count),
      contentManifest: parse(row.content_manifest_json, "content manifest"),
      storageManifest: parse(row.storage_manifest_json, "storage manifest"),
      layers,
    };
  }

  async beginDestroy(
    input: Readonly<{
      destroyRef: string;
      workspaceRef: string;
      ownerRef: string;
      tenantRef: string;
      expectedRuntimeGeneration: number;
      evidenceDigest: string;
      observedAt: string;
    }>,
  ): Promise<number> {
    assertDigest(input.evidenceDigest, "evidence digest");
    assertInteger(input.expectedRuntimeGeneration, "runtime generation");
    instant(input.observedAt, "observed at");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const workspaces: ReadonlyArray<WorkspaceRow> = await tx`
        SELECT workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
               conversation_ref, workspace_revision, current_checkpoint_ref,
               base_image_digest, base_image_signature_ref, workspace_key_ref,
               workspace_key_version, state
        FROM khala_sync_cloud_computer_workspaces
        WHERE workspace_ref = ${input.workspaceRef} FOR UPDATE
      `;
      const workspace = workspaces[0];
      if (workspace === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "workspace is missing");
      if (workspace.owner_ref !== input.ownerRef || workspace.tenant_ref !== input.tenantRef)
        throw new CloudComputerCheckpointStoreError("permission_denied", "workspace scope differs");
      if (Number(workspace.runtime_generation) !== input.expectedRuntimeGeneration)
        throw new CloudComputerCheckpointStoreError(
          "stale_generation",
          "runtime generation differs",
        );
      if (workspace.state === "destroyed") return 0;
      const externalReferences: ReadonlyArray<{ readonly reference_ref: string }> = await tx`
        SELECT reference.reference_ref
        FROM khala_sync_cloud_checkpoint_references AS reference
        JOIN khala_sync_cloud_computer_checkpoints AS checkpoint
          ON checkpoint.checkpoint_ref = reference.checkpoint_ref
        WHERE checkpoint.workspace_ref = ${input.workspaceRef}
          AND reference.workspace_ref <> ${input.workspaceRef}
          AND reference.state = 'live'
        LIMIT 1
      `;
      if (externalReferences[0] !== undefined)
        throw new CloudComputerCheckpointStoreError(
          "conflict",
          "workspace has a live external checkpoint reference",
        );
      await tx`
        UPDATE khala_sync_cloud_checkpoint_references
        SET state = 'released', released_at = ${input.observedAt}
        WHERE workspace_ref = ${input.workspaceRef} AND state = 'live'
      `;
      const checkpoints: ReadonlyArray<{ readonly checkpoint_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_checkpoints
        SET status = 'tombstoned', tombstoned_at = COALESCE(tombstoned_at, ${input.observedAt}),
            revision = revision + 1
        WHERE workspace_ref = ${input.workspaceRef} AND status IN ('committed', 'superseded')
        RETURNING checkpoint_ref
      `;
      const objects: ReadonlyArray<{
        readonly object_ref: string;
        readonly object_generation: string | number;
      }> = await tx`
        UPDATE khala_sync_cloud_checkpoint_objects
        SET state = 'tombstoned', tombstoned_at = COALESCE(tombstoned_at, ${input.observedAt}),
            revision = revision + 1
        WHERE workspace_ref = ${input.workspaceRef} AND state IN ('verified', 'reachable')
        RETURNING object_ref, object_generation
      `;
      await Promise.all([
        ...checkpoints.map(
          (checkpoint) => tx`
            INSERT INTO khala_sync_cloud_checkpoint_deletion_evidence
              (deletion_ref, workspace_ref, checkpoint_ref, action,
               generation_precondition_met, all_versions_absent, key_disposition,
               evidence_digest, observed_at)
            VALUES
              (${`${input.destroyRef}.${checkpoint.checkpoint_ref}`}, ${input.workspaceRef},
               ${checkpoint.checkpoint_ref}, 'tombstoned', TRUE, FALSE, 'retained',
               ${input.evidenceDigest}, ${input.observedAt})
            ON CONFLICT (deletion_ref) DO NOTHING
          `,
        ),
        ...objects.map(
          (object) => tx`
            INSERT INTO khala_sync_cloud_checkpoint_deletion_evidence
              (deletion_ref, workspace_ref, object_ref, object_generation, action,
               generation_precondition_met, all_versions_absent, key_disposition,
               evidence_digest, observed_at)
            VALUES
              (${`${input.destroyRef}.${object.object_ref}`}, ${input.workspaceRef},
               ${object.object_ref}, ${Number(object.object_generation)}, 'tombstoned',
               TRUE, FALSE, 'retained', ${input.evidenceDigest}, ${input.observedAt})
            ON CONFLICT (deletion_ref) DO NOTHING
          `,
        ),
      ]);
      await tx`
        UPDATE khala_sync_cloud_computer_workspaces
        SET state = 'destroying', current_checkpoint_ref = NULL,
            version = version + 1, updated_at = ${input.observedAt}
        WHERE workspace_ref = ${input.workspaceRef}
      `;
      await tx`
        UPDATE khala_sync_cloud_computers
        SET latest_checkpoint_ref = NULL, version = version + 1,
            updated_at = ${input.observedAt}
        WHERE computer_ref = ${workspace.computer_ref}
      `;
      return objects.length;
    });
  }

  async gcCandidates(now: string, limit = 100): Promise<ReadonlyArray<CloudCheckpointGcCandidate>> {
    instant(now, "now");
    assertInteger(limit, "limit");
    const rows: ReadonlyArray<{
      readonly object_ref: string;
      readonly workspace_ref: string;
      readonly object_generation: string | number;
      readonly ciphertext_digest: string;
      readonly encrypted_byte_count: string | number;
    }> = await this.sql`
      SELECT object.object_ref, object.workspace_ref, object.object_generation,
             object.ciphertext_digest, object.encrypted_byte_count
      FROM khala_sync_cloud_checkpoint_objects AS object
      WHERE object.state = 'tombstoned' AND object.retain_until <= ${now}
        AND NOT EXISTS (
          SELECT 1 FROM khala_sync_cloud_computer_checkpoints AS checkpoint
          JOIN khala_sync_cloud_checkpoint_references AS reference
            ON reference.checkpoint_ref = checkpoint.checkpoint_ref
               OR reference.source_checkpoint_ref = checkpoint.checkpoint_ref
          WHERE checkpoint.object_ref = object.object_ref AND reference.state = 'live'
        )
      ORDER BY object.retain_until, object.object_ref LIMIT ${limit}
    `;
    return rows.map((row) => ({
      objectRef: row.object_ref,
      workspaceRef: row.workspace_ref,
      objectGeneration: Number(row.object_generation),
      ciphertextDigest: row.ciphertext_digest,
      encryptedByteCount: Number(row.encrypted_byte_count),
    }));
  }

  async confirmObjectDeleted(
    input: Readonly<{
      deletionRef: string;
      workspaceRef: string;
      objectRef: string;
      objectGeneration: number;
      verification: CloudComputerGcsDeletionVerification;
      keyDisposition: "retained" | "destroyed" | "not_applicable";
      observedAt: string;
    }>,
  ): Promise<void> {
    assertInteger(input.objectGeneration, "object generation");
    try {
      assertGoogleCloudStorageDeletionVerification(input.verification);
    } catch {
      throw new CloudComputerCheckpointStoreError(
        "integrity_failed",
        "GCS deletion verification was not adapter-issued",
      );
    }
    if (
      input.verification.schema !== "openagents.cloud_computer_gcs_deletion_verification.v1" ||
      input.verification.objectRef !== input.objectRef ||
      input.verification.objectGeneration !== String(input.objectGeneration) ||
      input.verification.generationPreconditionMet !== true ||
      input.verification.allVersionsAbsent !== true
    )
      throw new CloudComputerCheckpointStoreError(
        "integrity_failed",
        "GCS deletion verification differs",
      );
    instant(input.observedAt, "observed at");
    const evidenceDigest = jsonDigest({
      schema: "openagents.cloud_computer_checkpoint_deletion_evidence.v1",
      deletionRef: input.deletionRef,
      workspaceRef: input.workspaceRef,
      objectRef: input.objectRef,
      objectGeneration: input.objectGeneration,
      generationPreconditionMet: input.verification.generationPreconditionMet,
      allVersionsAbsent: input.verification.allVersionsAbsent,
      keyDisposition: input.keyDisposition,
      observedAt: input.observedAt,
    });
    await this.serializable(async (tx) => {
      await this.lock(tx, input.workspaceRef);
      const rows: ReadonlyArray<{
        readonly workspace_ref: string;
        readonly object_generation: string | number;
        readonly state: string;
      }> = await tx`
        SELECT workspace_ref, object_generation, state
        FROM khala_sync_cloud_checkpoint_objects
        WHERE object_ref = ${input.objectRef} FOR UPDATE
      `;
      const object = rows[0];
      if (object === undefined)
        throw new CloudComputerCheckpointStoreError("not_found", "object is missing");
      if (
        object.workspace_ref !== input.workspaceRef ||
        Number(object.object_generation) !== input.objectGeneration
      )
        throw new CloudComputerCheckpointStoreError(
          "stale_generation",
          "object generation differs",
        );
      if (object.state !== "deleted") {
        if (object.state !== "tombstoned")
          throw new CloudComputerCheckpointStoreError("conflict", "object is still reachable");
        await tx`
          UPDATE khala_sync_cloud_checkpoint_objects
          SET state = 'deleted', deleted_at = ${input.observedAt}, revision = revision + 1
          WHERE object_ref = ${input.objectRef}
        `;
        await tx`
          UPDATE khala_sync_cloud_computer_checkpoints
          SET status = 'deleted', deleted_at = ${input.observedAt}, revision = revision + 1
          WHERE object_ref = ${input.objectRef} AND status = 'tombstoned'
        `;
      }
      const evidence: ReadonlyArray<{ readonly deletion_ref: string }> = await tx`
        INSERT INTO khala_sync_cloud_checkpoint_deletion_evidence
          (deletion_ref, workspace_ref, object_ref, object_generation, action,
           generation_precondition_met, all_versions_absent, key_disposition,
           evidence_digest, observed_at)
        VALUES
          (${input.deletionRef}, ${input.workspaceRef}, ${input.objectRef},
           ${input.objectGeneration}, 'deleted', TRUE, TRUE, ${input.keyDisposition},
           ${evidenceDigest}, ${input.observedAt})
        ON CONFLICT (deletion_ref) DO UPDATE SET deletion_ref = EXCLUDED.deletion_ref
        WHERE khala_sync_cloud_checkpoint_deletion_evidence.evidence_digest = EXCLUDED.evidence_digest
          AND khala_sync_cloud_checkpoint_deletion_evidence.object_generation = EXCLUDED.object_generation
        RETURNING deletion_ref
      `;
      if (evidence.length !== 1)
        throw new CloudComputerCheckpointStoreError("conflict", "deletion evidence differs");
      const remaining: ReadonlyArray<{ readonly count: string | number }> = await tx`
        SELECT COUNT(*) AS count FROM khala_sync_cloud_checkpoint_objects
        WHERE workspace_ref = ${input.workspaceRef} AND state <> 'deleted'
      `;
      if (Number(remaining[0]?.count ?? 0) === 0) {
        await tx`
          UPDATE khala_sync_cloud_computer_workspaces
          SET state = 'destroyed', destroyed_at = ${input.observedAt},
              version = version + 1, updated_at = ${input.observedAt}
          WHERE workspace_ref = ${input.workspaceRef} AND state = 'destroying'
        `;
      }
    });
  }

  async recordUsage(
    input: Readonly<{
      eventRef: string;
      workspaceRef: string;
      checkpointRef?: string | undefined;
      operationRef?: string | undefined;
      kind: "uploaded" | "reused" | "restored" | "retained" | "collected" | "storage_age_sample";
      byteCount: number;
      durationMs: number;
      storageAgeMs?: number | undefined;
      objectCount?: number | undefined;
      observedAt: string;
    }>,
  ): Promise<void> {
    assertInteger(input.byteCount, "byte count", true);
    assertInteger(input.durationMs, "duration", true);
    assertInteger(input.storageAgeMs ?? 0, "storage age", true);
    assertInteger(input.objectCount ?? 1, "object count", true);
    instant(input.observedAt, "observed at");
    await this.sql`
      INSERT INTO khala_sync_cloud_checkpoint_usage_events
        (event_ref, workspace_ref, checkpoint_ref, operation_ref, kind, byte_count,
         duration_ms, storage_age_ms, object_count, observed_at)
      VALUES
        (${input.eventRef}, ${input.workspaceRef}, ${input.checkpointRef ?? null},
         ${input.operationRef ?? null}, ${input.kind}, ${input.byteCount},
         ${input.durationMs}, ${input.storageAgeMs ?? 0}, ${input.objectCount ?? 1},
         ${input.observedAt})
      ON CONFLICT (event_ref) DO NOTHING
    `;
  }
}
