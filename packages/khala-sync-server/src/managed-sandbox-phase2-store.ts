import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  type ManagedSandboxCheckpointDeleteReceipt,
  type ManagedSandboxCheckpointStopOutcome,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxForkReceipt,
  type ManagedSandboxPhase2Command,
  type ManagedSandboxPrivateIngressCapability,
  type ManagedSandboxRestoreReceipt,
  SandboxRef,
  decodeManagedSandboxCheckpointDeleteReceipt,
  decodeManagedSandboxCheckpointStopOutcome,
  decodeManagedSandboxContentCheckpoint,
  decodeManagedSandboxForkReceipt,
  decodeManagedSandboxPhase2Command,
  decodeManagedSandboxRestoreReceipt,
  decodeManagedSandboxPrivateIngressCapability,
} from "@openagentsinc/managed-sandbox-contract";
import { Schema as S } from "effect";

import { ManagedSandboxStoreError } from "./managed-sandbox-store.js";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

export type ManagedSandboxPhase2StoredResult =
  | ManagedSandboxContentCheckpoint
  | ManagedSandboxCheckpointStopOutcome
  | ManagedSandboxCheckpointDeleteReceipt
  | ManagedSandboxForkReceipt
  | ManagedSandboxRestoreReceipt
  | ManagedSandboxPrivateIngressCapability;

export type ManagedSandboxPhase2StoredOperation = Readonly<{
  command: ManagedSandboxPhase2Command;
  result: ManagedSandboxPhase2StoredResult;
}>;

export type ManagedSandboxPhase2StoredCheckpointMutation =
  | Readonly<{ _tag: "Put"; checkpoint: ManagedSandboxContentCheckpoint }>
  | Readonly<{ _tag: "Delete"; checkpointRef: string }>
  | Readonly<{ _tag: "PutIngress"; capability: ManagedSandboxPrivateIngressCapability }>
  | Readonly<{ _tag: "None" }>;

type Phase2OperationRow = Readonly<{
  command_ref: string;
  owner_user_id: string;
  tenant_ref: string;
  idempotency_ref: string;
  command_kind: string;
  command_fingerprint: string;
  result_fingerprint: string;
  command_json: unknown;
  result_json: unknown;
}>;

type Phase2CheckpointRow = Readonly<{
  checkpoint_ref: string;
  owner_user_id: string;
  tenant_ref: string;
  source_sandbox_ref: string;
  source_resource_generation: string | number;
  content_digest: string;
  checkpoint_fingerprint: string;
  checkpoint_json: unknown;
  created_by_command_ref: string;
}>;

type Phase2IngressRow = Readonly<{
  capability_ref: string;
  owner_user_id: string;
  tenant_ref: string;
  sandbox_ref: string;
  resource_generation: string | number;
  audience_ref: string;
  access_url_digest: string;
  capability_fingerprint: string;
  capability_json: unknown;
}>;

const decodeRef = S.decodeUnknownSync(SandboxRef);

const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

const fingerprint = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(canonicalJson(value), "utf8").digest("hex")}`;

const publicSafe = <A>(value: A): A => {
  if (FORBIDDEN_PRIVATE_MATERIAL.test(canonicalJson(value))) {
    throw new ManagedSandboxStoreError(
      "unsafe_value",
      "Phase 2 persistence value contains private material",
    );
  }
  return value;
};

const decodeResult = (
  command: ManagedSandboxPhase2Command,
  value: unknown,
): ManagedSandboxPhase2StoredResult => {
  try {
    switch (command["_tag"]) {
      case "CreateCheckpoint":
        return decodeManagedSandboxContentCheckpoint(value);
      case "ArchiveWithCheckpoint":
        return decodeManagedSandboxCheckpointStopOutcome(value);
      case "ForkFromCheckpoint":
        return decodeManagedSandboxForkReceipt(value);
      case "RestoreCheckpoint":
        return decodeManagedSandboxRestoreReceipt(value);
      case "DeleteCheckpoint":
        return decodeManagedSandboxCheckpointDeleteReceipt(value);
      case "CreatePrivateIngress":
      case "RevokePrivateIngress":
      case "ExpirePrivateIngress":
        return decodeManagedSandboxPrivateIngressCapability(value);
    }
  } catch (error) {
    if (error instanceof ManagedSandboxStoreError) throw error;
    throw new ManagedSandboxStoreError("invalid", "Phase 2 result failed schema validation");
  }
};

const decodeOperation = (row: Phase2OperationRow): ManagedSandboxPhase2StoredOperation => {
  let command: ManagedSandboxPhase2Command;
  try {
    command = decodeManagedSandboxPhase2Command(row.command_json);
  } catch {
    throw new ManagedSandboxStoreError("corrupt_store", "stored Phase 2 command is invalid");
  }
  let result: ManagedSandboxPhase2StoredResult;
  try {
    result = decodeResult(command, row.result_json);
  } catch {
    throw new ManagedSandboxStoreError("corrupt_store", "stored Phase 2 result is invalid");
  }
  if (
    command.commandRef !== row.command_ref ||
    command.ownerRef !== row.owner_user_id ||
    command.tenantRef !== row.tenant_ref ||
    command.idempotencyRef !== row.idempotency_ref ||
    command["_tag"] !== row.command_kind ||
    fingerprint(command) !== row.command_fingerprint ||
    fingerprint(result) !== row.result_fingerprint
  ) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "stored Phase 2 operation does not match its indexed identity",
    );
  }
  return publicSafe({ command, result });
};

const selectOperation = async (
  sql: SyncTransactionSql | SyncSql,
  input: {
    ownerRef: string;
    tenantRef: string;
    commandRef: string;
    idempotencyRef: string;
  },
  lock = false,
): Promise<Phase2OperationRow | undefined> => {
  const rows: ReadonlyArray<Phase2OperationRow> = lock
    ? await sql`
        SELECT command_ref, owner_user_id, tenant_ref, idempotency_ref, command_kind,
               command_fingerprint, result_fingerprint, command_json, result_json
        FROM khala_sync_managed_sandbox_phase2_operations
        WHERE command_ref = ${input.commandRef}
           OR (owner_user_id = ${input.ownerRef} AND tenant_ref = ${input.tenantRef}
               AND idempotency_ref = ${input.idempotencyRef})
        FOR UPDATE
      `
    : await sql`
        SELECT command_ref, owner_user_id, tenant_ref, idempotency_ref, command_kind,
               command_fingerprint, result_fingerprint, command_json, result_json
        FROM khala_sync_managed_sandbox_phase2_operations
        WHERE command_ref = ${input.commandRef}
           OR (owner_user_id = ${input.ownerRef} AND tenant_ref = ${input.tenantRef}
               AND idempotency_ref = ${input.idempotencyRef})
      `;
  if (rows.length > 1) {
    throw new ManagedSandboxStoreError(
      "idempotency_conflict",
      "the command and idempotency references resolve to different operations",
    );
  }
  return rows[0];
};

const assertScope = (
  row: Pick<Phase2OperationRow, "owner_user_id" | "tenant_ref">,
  ownerRef: string,
  tenantRef: string,
): void => {
  if (row.owner_user_id !== ownerRef || row.tenant_ref !== tenantRef) {
    throw new ManagedSandboxStoreError("permission_denied", "Phase 2 scope does not match");
  }
};

const assertCheckpointRow = (
  row: Phase2CheckpointRow,
  checkpoint: ManagedSandboxContentCheckpoint,
): void => {
  if (
    row.checkpoint_ref !== checkpoint.checkpointRef ||
    row.owner_user_id !== checkpoint.ownerRef ||
    row.tenant_ref !== checkpoint.tenantRef ||
    row.source_sandbox_ref !== checkpoint.sourceSandboxRef ||
    Number(row.source_resource_generation) !== checkpoint.sourceResourceGeneration ||
    row.content_digest !== checkpoint.contentDigest ||
    row.checkpoint_fingerprint !== fingerprint(checkpoint)
  ) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "stored checkpoint metadata does not match its indexed identity",
    );
  }
};

const observedAt = (result: ManagedSandboxPhase2StoredResult): string => {
  if ("deletedAt" in result) return result.deletedAt;
  if ("cleanedAt" in result) return result.cleanedAt;
  if ("issuedAt" in result) return result.issuedAt;
  if ("observedAt" in result) return result.observedAt;
  return result.verifiedAt;
};

const checkpointFromMutation = (
  operation: ManagedSandboxPhase2StoredOperation,
  checkpoint: ManagedSandboxContentCheckpoint,
): ManagedSandboxContentCheckpoint => {
  const result = operation.result;
  const bound =
    operation.command["_tag"] === "CreateCheckpoint"
      ? result
      : operation.command["_tag"] === "ArchiveWithCheckpoint" &&
          "checkpoint" in result &&
          result["_tag"] === "Archived"
        ? result.checkpoint
        : undefined;
  if (
    bound === undefined ||
    !("checkpointRef" in bound) ||
    canonicalJson(bound) !== canonicalJson(checkpoint)
  ) {
    throw new ManagedSandboxStoreError(
      "command_conflict",
      "checkpoint metadata does not match the settled operation result",
    );
  }
  return checkpoint;
};

const assertIngressRow = (
  row: Phase2IngressRow,
  capability: ManagedSandboxPrivateIngressCapability,
): void => {
  if (
    row.capability_ref !== capability.capabilityRef ||
    row.owner_user_id !== capability.ownerRef ||
    row.sandbox_ref !== capability.sandboxRef ||
    Number(row.resource_generation) !== capability.resourceGeneration ||
    row.audience_ref !== capability.audienceRef ||
    row.access_url_digest !== capability.accessUrlDigest ||
    row.capability_fingerprint !== fingerprint(capability)
  ) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "stored private ingress metadata does not match its indexed identity",
    );
  }
};

const applyIngressMutation = async (
  tx: SyncTransactionSql,
  operation: ManagedSandboxPhase2StoredOperation,
  capability: ManagedSandboxPrivateIngressCapability,
): Promise<void> => {
  if (
    !["CreatePrivateIngress", "RevokePrivateIngress", "ExpirePrivateIngress"].includes(
      operation.command["_tag"],
    ) ||
    canonicalJson(operation.result) !== canonicalJson(capability)
  ) {
    throw new ManagedSandboxStoreError(
      "command_conflict",
      "private ingress mutation does not match its operation result",
    );
  }
  const rows: ReadonlyArray<Phase2IngressRow> = await tx`
    SELECT capability_ref, owner_user_id, tenant_ref, sandbox_ref, resource_generation,
           audience_ref, access_url_digest, capability_fingerprint, capability_json
    FROM khala_sync_managed_sandbox_private_ingress
    WHERE capability_ref = ${capability.capabilityRef}
    FOR UPDATE
  `;
  const existing = rows[0];
  if (operation.command["_tag"] === "CreatePrivateIngress") {
    if (capability["_tag"] !== "Active" || existing !== undefined) {
      throw new ManagedSandboxStoreError(
        "command_conflict",
        "private ingress capability identity is already in use",
      );
    }
    await tx`
      INSERT INTO khala_sync_managed_sandbox_private_ingress
        (capability_ref, owner_user_id, tenant_ref, sandbox_ref, resource_generation,
         audience_ref, access_url_digest, capability_state, capability_fingerprint,
         capability_json, created_by_command_ref, updated_by_command_ref, expires_at,
         created_at, updated_at)
      VALUES
        (${capability.capabilityRef}, ${operation.command.ownerRef},
         ${operation.command.tenantRef}, ${capability.sandboxRef},
         ${capability.resourceGeneration}, ${capability.audienceRef},
         ${capability.accessUrlDigest}, ${capability["_tag"]}, ${fingerprint(capability)},
         ${capability}::jsonb, ${operation.command.commandRef},
         ${operation.command.commandRef}, ${capability.expiresAt}, ${capability.issuedAt},
         ${capability.issuedAt})
    `;
    return;
  }
  if (existing === undefined) {
    throw new ManagedSandboxStoreError("not_found", "private ingress capability does not exist");
  }
  assertScope(existing, operation.command.ownerRef, operation.command.tenantRef);
  let current: ManagedSandboxPrivateIngressCapability;
  try {
    current = decodeManagedSandboxPrivateIngressCapability(existing.capability_json);
  } catch {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "stored private ingress metadata is invalid",
    );
  }
  assertIngressRow(existing, current);
  if (
    current["_tag"] !== "Active" ||
    capability["_tag"] !== "Cleaned" ||
    capability.capabilityRef !== current.capabilityRef ||
    capability.accessUrlDigest !== current.accessUrlDigest
  ) {
    throw new ManagedSandboxStoreError(
      "command_conflict",
      "private ingress terminal mutation does not bind the active capability",
    );
  }
  await tx`
    UPDATE khala_sync_managed_sandbox_private_ingress
    SET capability_state = ${capability["_tag"]},
        capability_fingerprint = ${fingerprint(capability)},
        capability_json = ${capability}::jsonb,
        updated_by_command_ref = ${operation.command.commandRef},
        updated_at = ${capability.cleanedAt}
    WHERE capability_ref = ${capability.capabilityRef}
      AND owner_user_id = ${operation.command.ownerRef}
      AND tenant_ref = ${operation.command.tenantRef}
  `;
};

const applyCheckpointMutation = async (
  tx: SyncTransactionSql,
  operation: ManagedSandboxPhase2StoredOperation,
  mutation: ManagedSandboxPhase2StoredCheckpointMutation,
): Promise<void> => {
  if (mutation["_tag"] === "PutIngress") {
    return applyIngressMutation(tx, operation, mutation.capability);
  }
  if (mutation["_tag"] === "None") {
    if (
      operation.command["_tag"] === "CreateCheckpoint" ||
      (operation.command["_tag"] === "ArchiveWithCheckpoint" &&
        "archiveClaim" in operation.result &&
        operation.result["_tag"] === "Archived") ||
      operation.command["_tag"] === "DeleteCheckpoint" ||
      operation.command["_tag"] === "CreatePrivateIngress" ||
      operation.command["_tag"] === "RevokePrivateIngress" ||
      operation.command["_tag"] === "ExpirePrivateIngress"
    ) {
      throw new ManagedSandboxStoreError(
        "command_conflict",
        "the operation result requires a checkpoint metadata mutation",
      );
    }
    return;
  }

  if (mutation["_tag"] === "Put") {
    const checkpoint = checkpointFromMutation(operation, mutation.checkpoint);
    const existing: ReadonlyArray<Phase2CheckpointRow> = await tx`
      SELECT checkpoint_ref, owner_user_id, tenant_ref, source_sandbox_ref,
             source_resource_generation, content_digest, checkpoint_fingerprint,
             checkpoint_json, created_by_command_ref
      FROM khala_sync_managed_sandbox_checkpoints
      WHERE checkpoint_ref = ${checkpoint.checkpointRef}
      FOR UPDATE
    `;
    if (existing.length > 0) {
      throw new ManagedSandboxStoreError(
        "command_conflict",
        "the checkpoint reference is already bound to another operation",
      );
    }
    await tx`
      INSERT INTO khala_sync_managed_sandbox_checkpoints
        (checkpoint_ref, owner_user_id, tenant_ref, source_sandbox_ref,
         source_resource_generation, content_digest, checkpoint_fingerprint,
         created_by_command_ref, retained_until, checkpoint_json, created_at, updated_at)
      VALUES
        (${checkpoint.checkpointRef}, ${checkpoint.ownerRef}, ${checkpoint.tenantRef},
         ${checkpoint.sourceSandboxRef}, ${checkpoint.sourceResourceGeneration},
         ${checkpoint.contentDigest}, ${fingerprint(checkpoint)},
         ${operation.command.commandRef}, ${checkpoint.retainedUntil}, ${checkpoint}::jsonb,
         ${checkpoint.completedAt}, ${checkpoint.verifiedAt})
    `;
    return;
  }

  if (
    operation.command["_tag"] !== "DeleteCheckpoint" ||
    !("contentDeleted" in operation.result) ||
    operation.result.contentDeleted !== true ||
    operation.result.checkpointRef !== mutation.checkpointRef
  ) {
    throw new ManagedSandboxStoreError(
      "command_conflict",
      "checkpoint delete mutation does not match the deletion receipt",
    );
  }
  const rows: ReadonlyArray<Phase2CheckpointRow> = await tx`
    SELECT checkpoint_ref, owner_user_id, tenant_ref, source_sandbox_ref,
           source_resource_generation, content_digest, checkpoint_fingerprint,
           checkpoint_json, created_by_command_ref
    FROM khala_sync_managed_sandbox_checkpoints
    WHERE checkpoint_ref = ${mutation.checkpointRef}
    FOR UPDATE
  `;
  const row = rows[0];
  if (row === undefined) {
    throw new ManagedSandboxStoreError("not_found", "checkpoint metadata does not exist");
  }
  assertScope(row, operation.command.ownerRef, operation.command.tenantRef);
  let checkpoint: ManagedSandboxContentCheckpoint;
  try {
    checkpoint = decodeManagedSandboxContentCheckpoint(row.checkpoint_json);
  } catch {
    throw new ManagedSandboxStoreError("corrupt_store", "stored checkpoint metadata is invalid");
  }
  assertCheckpointRow(row, checkpoint);
  if (checkpoint.contentDigest !== operation.result.contentDigest) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "checkpoint deletion proof does not match stored content",
    );
  }
  await tx`
    DELETE FROM khala_sync_managed_sandbox_checkpoints
    WHERE checkpoint_ref = ${mutation.checkpointRef}
      AND owner_user_id = ${operation.command.ownerRef}
      AND tenant_ref = ${operation.command.tenantRef}
  `;
};

export class PostgresManagedSandboxPhase2Store {
  constructor(private readonly sql: SyncSql) {}

  async lookupOperation(input: {
    ownerRef: string;
    tenantRef: string;
    commandRef: string;
    idempotencyRef: string;
  }): Promise<ManagedSandboxPhase2StoredOperation | undefined> {
    const scope = {
      ownerRef: decodeRef(input.ownerRef),
      tenantRef: decodeRef(input.tenantRef),
      commandRef: decodeRef(input.commandRef),
      idempotencyRef: decodeRef(input.idempotencyRef),
    };
    const row = await selectOperation(this.sql, scope);
    if (row === undefined) return undefined;
    assertScope(row, scope.ownerRef, scope.tenantRef);
    return decodeOperation(row);
  }

  async readCheckpoint(input: {
    ownerRef: string;
    tenantRef: string;
    checkpointRef: string;
  }): Promise<ManagedSandboxContentCheckpoint | undefined> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const checkpointRef = decodeRef(input.checkpointRef);
    const rows: ReadonlyArray<Phase2CheckpointRow> = await this.sql`
      SELECT checkpoint_ref, owner_user_id, tenant_ref, source_sandbox_ref,
             source_resource_generation, content_digest, checkpoint_fingerprint,
             checkpoint_json, created_by_command_ref
      FROM khala_sync_managed_sandbox_checkpoints
      WHERE checkpoint_ref = ${checkpointRef}
    `;
    const row = rows[0];
    if (row === undefined) return undefined;
    assertScope(row, ownerRef, tenantRef);
    let checkpoint: ManagedSandboxContentCheckpoint;
    try {
      checkpoint = decodeManagedSandboxContentCheckpoint(row.checkpoint_json);
    } catch {
      throw new ManagedSandboxStoreError("corrupt_store", "stored checkpoint metadata is invalid");
    }
    assertCheckpointRow(row, checkpoint);
    return publicSafe(checkpoint);
  }

  async readPrivateIngress(input: {
    ownerRef: string;
    tenantRef: string;
    capabilityRef: string;
  }): Promise<ManagedSandboxPrivateIngressCapability | undefined> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const capabilityRef = decodeRef(input.capabilityRef);
    const rows: ReadonlyArray<Phase2IngressRow> = await this.sql`
      SELECT capability_ref, owner_user_id, tenant_ref, sandbox_ref, resource_generation,
             audience_ref, access_url_digest, capability_fingerprint, capability_json
      FROM khala_sync_managed_sandbox_private_ingress
      WHERE capability_ref = ${capabilityRef}
    `;
    const row = rows[0];
    if (row === undefined) return undefined;
    assertScope(row, ownerRef, tenantRef);
    let capability: ManagedSandboxPrivateIngressCapability;
    try {
      capability = decodeManagedSandboxPrivateIngressCapability(row.capability_json);
    } catch {
      throw new ManagedSandboxStoreError(
        "corrupt_store",
        "stored private ingress metadata is invalid",
      );
    }
    assertIngressRow(row, capability);
    return publicSafe(capability);
  }

  async readPrivateIngressForAudience(input: {
    audienceRef: string;
    capabilityRef: string;
  }): Promise<ManagedSandboxPrivateIngressCapability | undefined> {
    const audienceRef = decodeRef(input.audienceRef);
    const capabilityRef = decodeRef(input.capabilityRef);
    const rows: ReadonlyArray<Phase2IngressRow> = await this.sql`
      SELECT capability_ref, owner_user_id, tenant_ref, sandbox_ref, resource_generation,
             audience_ref, access_url_digest, capability_fingerprint, capability_json
      FROM khala_sync_managed_sandbox_private_ingress
      WHERE capability_ref = ${capabilityRef}
        AND audience_ref = ${audienceRef}
    `;
    const row = rows[0];
    if (row === undefined) return undefined;
    let capability: ManagedSandboxPrivateIngressCapability;
    try {
      capability = decodeManagedSandboxPrivateIngressCapability(row.capability_json);
    } catch {
      throw new ManagedSandboxStoreError(
        "corrupt_store",
        "stored private ingress metadata is invalid",
      );
    }
    assertIngressRow(row, capability);
    return publicSafe(capability);
  }

  async settle(input: {
    operation: ManagedSandboxPhase2StoredOperation;
    checkpointMutation: ManagedSandboxPhase2StoredCheckpointMutation;
  }): Promise<ManagedSandboxPhase2StoredOperation> {
    let command: ManagedSandboxPhase2Command;
    try {
      command = publicSafe(decodeManagedSandboxPhase2Command(input.operation.command));
    } catch (error) {
      if (error instanceof ManagedSandboxStoreError) throw error;
      throw new ManagedSandboxStoreError("invalid", "Phase 2 command failed schema validation");
    }
    const result = publicSafe(decodeResult(command, input.operation.result));
    const operation = { command, result } satisfies ManagedSandboxPhase2StoredOperation;
    const commandFingerprint = fingerprint(command);
    const resultFingerprint = fingerprint(result);

    return this.sql.begin(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${command.ownerRef}|${command.tenantRef}|${command.idempotencyRef}`}, 0))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${command.commandRef}, 1))`;

      const existing = await selectOperation(
        tx,
        {
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          commandRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
        },
        true,
      );
      if (existing !== undefined) {
        assertScope(existing, command.ownerRef, command.tenantRef);
        if (
          existing.command_fingerprint !== commandFingerprint ||
          existing.result_fingerprint !== resultFingerprint
        ) {
          throw new ManagedSandboxStoreError(
            "idempotency_conflict",
            "Phase 2 operation reference is bound to different bytes",
          );
        }
        return decodeOperation(existing);
      }

      await tx`
        INSERT INTO khala_sync_managed_sandbox_phase2_operations
          (command_ref, owner_user_id, tenant_ref, idempotency_ref, command_kind,
           command_fingerprint, result_fingerprint, command_json, result_json,
           requested_at, settled_at)
        VALUES
          (${command.commandRef}, ${command.ownerRef}, ${command.tenantRef},
           ${command.idempotencyRef}, ${command["_tag"]}, ${commandFingerprint},
           ${resultFingerprint}, ${command}::jsonb, ${result}::jsonb,
           ${command.requestedAt}, ${observedAt(result)})
      `;

      await applyCheckpointMutation(tx, operation, input.checkpointMutation);
      return operation;
    });
  }
}
