import { createHash } from "node:crypto";

import {
  PortablePhaseOperationClaimRequestSchema,
  PortablePhaseOperationRecordSchema,
  PortablePhaseOperationRenewRequestSchema,
  PortablePhaseOperationRequestSchema,
  PortablePhaseOperationResultRequestSchema,
  validateIdePortableDestinationActivationReceipt,
  type PortablePhaseOperationRecord,
  type PortablePhaseOperationRequest,
} from "@openagentsinc/portable-session-contract";

export {
  PortablePhaseOperationClaimRequestSchema,
  PortablePhaseOperationRenewRequestSchema,
  PortablePhaseOperationResultRequestSchema,
  type PortablePhaseOperationClaimRequest,
  type PortablePhaseOperationRecord,
  type PortablePhaseOperationRenewRequest,
  type PortablePhaseOperationResultRequest,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import type { SyncSql, SyncTransactionSql } from "./sql.js";

type Row = {
  operation_ref: string;
  request_fingerprint: string;
  command_ref: string;
  command_execution_claim_ref: string;
  owner_user_id: string;
  session_ref: string;
  attachment_ref: string;
  attachment_generation: string | number;
  target_ref: string;
  pylon_ref: string;
  kind: string;
  checkpoint_ref: string | null;
  checkpoint_object_ref: string | null;
  checkpoint_digest: string | null;
  request_evidence_refs_json: unknown;
  request_json: unknown;
  expires_at: Date | string;
  state: "pending" | "claimed" | "completed" | "failed" | "expired";
  claim_ref: string | null;
  claim_fingerprint: string | null;
  worker_instance_ref: string | null;
  claim_generation: string | number | null;
  lease_revision: string | number | null;
  claimed_at: Date | string | null;
  lease_expires_at: Date | string | null;
  result_ref: string | null;
  result_fingerprint: string | null;
  result_status: "completed" | "failed" | "expired" | null;
  result_checkpoint_ref: string | null;
  result_checkpoint_object_ref: string | null;
  result_checkpoint_digest: string | null;
  result_checkpoint_manifest_digest: string | null;
  result_destination_activation_receipt_json: unknown;
  result_evidence_refs_json: unknown;
  error_ref: string | null;
  completed_at: Date | string | null;
  updated_at: Date | string;
};

type ExecutionBindingRow = {
  command_ref: string;
  owner_user_id: string;
  session_ref: string;
  source_attachment_ref: string;
  source_generation: string | number;
  destination_target_ref: string;
  executor_environment_ref: string;
  state: string;
  lease_expires_at: Date | string;
};

export class PortablePhaseOperationStoreError extends Error {
  readonly _tag = "PortablePhaseOperationStoreError";
  override readonly name = "PortablePhaseOperationStoreError";

  constructor(
    readonly code:
      | "invalid"
      | "not_found"
      | "conflict"
      | "not_claimable"
      | "expired"
      | "stale_generation"
      | "stale_revision"
      | "unsafe_material",
    message: string,
  ) {
    super(message);
  }
}

const decodeRequest = Schema.decodeUnknownSync(PortablePhaseOperationRequestSchema);
const decodeClaimRequest = Schema.decodeUnknownSync(PortablePhaseOperationClaimRequestSchema);
const decodeRenewRequest = Schema.decodeUnknownSync(PortablePhaseOperationRenewRequestSchema);
const decodeResultRequest = Schema.decodeUnknownSync(PortablePhaseOperationResultRequestSchema);
const decodeRecord = Schema.decodeUnknownSync(PortablePhaseOperationRecordSchema);

const RefSchema = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const decodeRef = Schema.decodeUnknownSync(RefSchema);

const forbiddenPrivateMaterial =
  /"(?:[A-Za-z0-9_]*(?:path|paths|credential|credentials|handle|handles|bytes)|token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|hostname|processId|providerSessionId|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/i;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      // The array is new. Sorting it cannot mutate caller state.
      // eslint-disable-next-line unicorn/no-array-sort
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const assertPublicSafe = (value: unknown): void => {
  if (forbiddenPrivateMaterial.test(canonical(value))) {
    throw new PortablePhaseOperationStoreError(
      "unsafe_material",
      "portable phase operation contains forbidden private material",
    );
  }
};

const fingerprint = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;
const iso = (value: Date | string): string => new Date(value).toISOString();

const positive = (value: string | number | null, field: string): number | null => {
  if (value === null) return null;
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new PortablePhaseOperationStoreError("invalid", `${field} is invalid`);
  }
  return number;
};

const requiredPositive = (value: string | number | null, field: string): number => {
  const number = positive(value, field);
  if (number === null) {
    throw new PortablePhaseOperationStoreError("invalid", `${field} is missing`);
  }
  return number;
};

const requiredRow = (rows: ReadonlyArray<Row>, operation: string): Row => {
  const row = rows[0];
  if (row === undefined) {
    throw new PortablePhaseOperationStoreError("conflict", `${operation} CAS did not update a row`);
  }
  return row;
};

const rowToRecord = (row: Row): PortablePhaseOperationRecord => {
  const request = decodeRequest(parseJson(row.request_json));
  assertPublicSafe(request);
  if (
    fingerprint(request) !== row.request_fingerprint ||
    request.commandRef !== row.command_ref ||
    request.commandExecutionClaimRef !== row.command_execution_claim_ref ||
    request.ownerRef !== row.owner_user_id ||
    request.sessionRef !== row.session_ref ||
    request.attachmentRef !== row.attachment_ref ||
    request.attachmentGeneration !==
      requiredPositive(row.attachment_generation, "attachment generation") ||
    request.targetRef !== row.target_ref ||
    request.pylonRef !== row.pylon_ref ||
    request.kind !== row.kind ||
    request.checkpointRef !== row.checkpoint_ref ||
    request.checkpointObjectRef !== row.checkpoint_object_ref ||
    request.checkpointDigest !== row.checkpoint_digest ||
    canonical(request.evidenceRefs) !== canonical(parseJson(row.request_evidence_refs_json)) ||
    new Date(request.expiresAt).toISOString() !== iso(row.expires_at)
  ) {
    throw new PortablePhaseOperationStoreError(
      "invalid",
      "portable phase persisted request binding is invalid",
    );
  }
  return decodeRecord({
    request,
    requestFingerprint: row.request_fingerprint,
    state: row.state,
    claimRef: row.claim_ref,
    claimFingerprint: row.claim_fingerprint,
    workerInstanceRef: row.worker_instance_ref,
    claimGeneration: positive(row.claim_generation, "claim generation"),
    leaseRevision: positive(row.lease_revision, "lease revision"),
    claimedAt: row.claimed_at === null ? null : iso(row.claimed_at),
    leaseExpiresAt: row.lease_expires_at === null ? null : iso(row.lease_expires_at),
    resultRef: row.result_ref,
    resultFingerprint: row.result_fingerprint,
    resultStatus: row.result_status,
    resultCheckpointRef: row.result_checkpoint_ref,
    resultCheckpointObjectRef: row.result_checkpoint_object_ref,
    resultCheckpointDigest: row.result_checkpoint_digest,
    resultCheckpointManifestDigest: row.result_checkpoint_manifest_digest,
    resultDestinationActivationReceipt: parseJson(row.result_destination_activation_receipt_json),
    resultEvidenceRefs: parseJson(row.result_evidence_refs_json),
    errorRef: row.error_ref,
    completedAt: row.completed_at === null ? null : iso(row.completed_at),
    updatedAt: iso(row.updated_at),
  });
};

const selectByOperation = async (
  sql: SyncTransactionSql,
  operationRef: string,
): Promise<Row | undefined> => {
  const rows: Row[] = await sql`
    SELECT * FROM khala_sync_portable_phase_operations
    WHERE operation_ref = ${operationRef}
    FOR UPDATE
  `;
  return rows[0];
};

const selectByClaim = async (
  sql: SyncTransactionSql,
  claimRef: string,
): Promise<Row | undefined> => {
  const rows: Row[] = await sql`
    SELECT * FROM khala_sync_portable_phase_operations
    WHERE claim_ref = ${claimRef}
    FOR UPDATE
  `;
  return rows[0];
};

const selectByPhase = async (
  sql: SyncTransactionSql,
  commandExecutionClaimRef: string,
  kind: string,
): Promise<Row | undefined> => {
  const rows: Row[] = await sql`
    SELECT * FROM khala_sync_portable_phase_operations
    WHERE command_execution_claim_ref = ${commandExecutionClaimRef} AND kind = ${kind}
    FOR UPDATE
  `;
  return rows[0];
};

const isSourceKind = (kind: string): boolean =>
  kind === "quiesce" || kind === "checkpoint-create" || kind === "source-cleanup";

/** Durable refs-only exchange between canonical movement and one bound Pylon target. */
export class PostgresPortablePhaseOperationStore {
  constructor(
    private readonly sql: SyncSql,
    private readonly now: () => string = () => new Date().toISOString(),
  ) {}

  async enqueue(input: unknown): Promise<{
    readonly status: "enqueued" | "replayed";
    readonly operation: PortablePhaseOperationRecord;
  }> {
    assertPublicSafe(input);
    let request: PortablePhaseOperationRequest;
    try {
      request = decodeRequest(input);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase request is invalid");
    }
    this.assertCheckpointRequestShape(request);
    const now = new Date(this.now());
    if (new Date(request.expiresAt) <= now) {
      throw new PortablePhaseOperationStoreError("expired", "portable phase request expired");
    }
    const requestFingerprint = fingerprint(request);

    return this.sql.begin(async (tx) => {
      const bindings: ExecutionBindingRow[] = await tx`
        SELECT command_ref, owner_user_id, session_ref, source_attachment_ref,
               source_generation, destination_target_ref, executor_environment_ref,
               state, lease_expires_at
        FROM khala_sync_portable_command_executions
        WHERE claim_ref = ${request.commandExecutionClaimRef}
        FOR UPDATE
      `;
      const binding = bindings[0];
      if (!binding) {
        throw new PortablePhaseOperationStoreError(
          "not_found",
          "portable command execution claim does not exist",
        );
      }
      const existing = await selectByOperation(tx, request.operationRef);
      if (existing) {
        if (existing.request_fingerprint === requestFingerprint) {
          return { status: "replayed", operation: rowToRecord(existing) };
        }
        throw new PortablePhaseOperationStoreError(
          "conflict",
          "portable phase operation ref has different request bytes",
        );
      }
      const existingPhase = await selectByPhase(tx, request.commandExecutionClaimRef, request.kind);
      if (existingPhase) {
        throw new PortablePhaseOperationStoreError(
          "conflict",
          "portable command phase already has a different operation ref",
        );
      }
      this.assertExecutionBinding(request, binding, now);

      const inserted: Row[] = await tx`
        INSERT INTO khala_sync_portable_phase_operations
          (operation_ref, request_fingerprint, command_ref,
           command_execution_claim_ref, owner_user_id, session_ref,
           attachment_ref, attachment_generation, target_ref, pylon_ref, kind,
           checkpoint_ref, checkpoint_object_ref, checkpoint_digest,
           request_evidence_refs_json, request_json, expires_at, state,
           created_at, updated_at)
        VALUES
          (${request.operationRef}, ${requestFingerprint}, ${request.commandRef},
           ${request.commandExecutionClaimRef}, ${request.ownerRef}, ${request.sessionRef},
           ${request.attachmentRef}, ${request.attachmentGeneration}, ${request.targetRef},
           ${request.pylonRef}, ${request.kind}, ${request.checkpointRef},
           ${request.checkpointObjectRef}, ${request.checkpointDigest},
           ${JSON.stringify(request.evidenceRefs)}::jsonb,
           ${JSON.stringify(request)}::jsonb, ${request.expiresAt}, 'pending',
           ${now.toISOString()}, ${now.toISOString()})
        RETURNING *
      `;
      return { status: "enqueued", operation: rowToRecord(requiredRow(inserted, "enqueue")) };
    });
  }

  async pending(
    pylonRefInput: unknown,
    targetRefInput: unknown,
    limit = 32,
  ): Promise<ReadonlyArray<PortablePhaseOperationRecord>> {
    let pylonRef: string;
    let targetRef: string;
    try {
      pylonRef = decodeRef(pylonRefInput);
      targetRef = decodeRef(targetRefInput);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase binding is invalid");
    }
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase read limit is invalid");
    }
    const rows: Row[] = await this.sql`
      SELECT * FROM khala_sync_portable_phase_operations
      WHERE pylon_ref = ${pylonRef} AND target_ref = ${targetRef} AND state = 'pending'
        AND expires_at > ${this.now()}
      ORDER BY created_at, operation_ref
      LIMIT ${limit}
    `;
    return rows.map(rowToRecord);
  }

  async read(
    pylonRefInput: unknown,
    targetRefInput: unknown,
    operationRefInput: unknown,
  ): Promise<PortablePhaseOperationRecord> {
    let pylonRef: string;
    let targetRef: string;
    let operationRef: string;
    try {
      pylonRef = decodeRef(pylonRefInput);
      targetRef = decodeRef(targetRefInput);
      operationRef = decodeRef(operationRefInput);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase binding is invalid");
    }
    const rows: Row[] = await this.sql`
      SELECT * FROM khala_sync_portable_phase_operations
      WHERE operation_ref = ${operationRef}
        AND pylon_ref = ${pylonRef}
        AND target_ref = ${targetRef}
      LIMIT 1
    `;
    const row = rows[0];
    if (row === undefined) {
      throw new PortablePhaseOperationStoreError(
        "not_found",
        "portable phase operation does not exist in the exact target scope",
      );
    }
    return rowToRecord(row);
  }

  async claim(input: unknown): Promise<{
    readonly status: "claimed" | "replayed";
    readonly operation: PortablePhaseOperationRecord;
  }> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeClaimRequest>;
    try {
      request = decodeClaimRequest(input);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase claim is invalid");
    }
    const now = new Date(this.now());
    const leaseExpiry = new Date(request.leaseExpiresAt);
    if (leaseExpiry <= now) {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase lease is not future");
    }
    const claimFingerprint = fingerprint(request);
    return this.sql.begin(async (tx) => {
      const row = await selectByOperation(tx, request.operationRef);
      if (!row) {
        throw new PortablePhaseOperationStoreError(
          "not_found",
          "portable phase operation does not exist",
        );
      }
      this.assertClaimBinding(row, request);
      if (row.claim_ref !== null) {
        if (
          row.state === "claimed" &&
          row.claim_ref === request.claimRef &&
          row.claim_fingerprint === claimFingerprint &&
          row.worker_instance_ref === request.workerInstanceRef &&
          row.lease_expires_at !== null &&
          new Date(row.lease_expires_at) > now
        ) {
          return { status: "replayed", operation: rowToRecord(row) };
        }
        throw new PortablePhaseOperationStoreError(
          row.state === "expired" ||
            (row.lease_expires_at !== null && new Date(row.lease_expires_at) <= now)
            ? "expired"
            : "conflict",
          "portable phase operation already has an execution claim",
        );
      }
      if (row.state !== "pending") {
        throw new PortablePhaseOperationStoreError(
          "not_claimable",
          "portable phase operation is terminal",
        );
      }
      if (new Date(row.expires_at) <= now || leaseExpiry > new Date(row.expires_at)) {
        throw new PortablePhaseOperationStoreError(
          "expired",
          "portable phase claim is outside request expiry",
        );
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_phase_operations
        SET state = 'claimed', claim_ref = ${request.claimRef},
            claim_fingerprint = ${claimFingerprint},
            worker_instance_ref = ${request.workerInstanceRef},
            claim_generation = 1, lease_revision = 1,
            claimed_at = ${now.toISOString()}, lease_expires_at = ${request.leaseExpiresAt},
            updated_at = ${now.toISOString()}
        WHERE operation_ref = ${request.operationRef} AND state = 'pending'
        RETURNING *
      `;
      return { status: "claimed", operation: rowToRecord(requiredRow(updated, "claim")) };
    });
  }

  async renew(input: unknown): Promise<{
    readonly status: "renewed" | "replayed";
    readonly operation: PortablePhaseOperationRecord;
  }> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeRenewRequest>;
    try {
      request = decodeRenewRequest(input);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase renewal is invalid");
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const row = await selectByClaim(tx, request.claimRef);
      if (!row)
        throw new PortablePhaseOperationStoreError(
          "not_found",
          "portable phase claim does not exist",
        );
      this.assertMutationBinding(row, request);
      const revision = requiredPositive(row.lease_revision, "lease revision");
      if (
        row.state !== "claimed" ||
        row.lease_expires_at === null ||
        new Date(row.lease_expires_at) <= now
      ) {
        throw new PortablePhaseOperationStoreError("expired", "portable phase claim is not active");
      }
      if (
        revision === request.expectedLeaseRevision + 1 &&
        iso(row.lease_expires_at) === new Date(request.leaseExpiresAt).toISOString()
      ) {
        return { status: "replayed", operation: rowToRecord(row) };
      }
      if (revision !== request.expectedLeaseRevision) {
        throw new PortablePhaseOperationStoreError(
          "stale_revision",
          "portable phase lease revision is stale",
        );
      }
      const next = new Date(request.leaseExpiresAt);
      if (!(next > new Date(row.lease_expires_at)) || next > new Date(row.expires_at)) {
        throw new PortablePhaseOperationStoreError(
          "invalid",
          "portable phase renewal exceeds request bounds",
        );
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_phase_operations
        SET lease_revision = lease_revision + 1,
            lease_expires_at = ${request.leaseExpiresAt}, updated_at = ${now.toISOString()}
        WHERE claim_ref = ${request.claimRef} AND state = 'claimed'
          AND lease_revision = ${revision}
        RETURNING *
      `;
      return { status: "renewed", operation: rowToRecord(requiredRow(updated, "renew")) };
    });
  }

  async complete(input: unknown): Promise<{
    readonly status: "completed" | "failed" | "replayed";
    readonly operation: PortablePhaseOperationRecord;
  }> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeResultRequest>;
    try {
      request = decodeResultRequest(input);
    } catch {
      throw new PortablePhaseOperationStoreError("invalid", "portable phase result is invalid");
    }
    const now = new Date(this.now());
    const resultFingerprint = fingerprint(request);
    return this.sql.begin(async (tx) => {
      const row = await selectByClaim(tx, request.claimRef);
      if (!row)
        throw new PortablePhaseOperationStoreError(
          "not_found",
          "portable phase claim does not exist",
        );
      this.assertMutationBinding(row, request);
      const targetRows: Array<{ target_class: string }> = await tx`
        SELECT target_class
        FROM khala_sync_portable_targets
        WHERE target_ref = ${row.target_ref}
        FOR SHARE
      `;
      const targetClass = targetRows[0]?.target_class;
      if (targetClass === undefined) {
        throw new PortablePhaseOperationStoreError(
          "conflict",
          "portable phase target authority does not exist",
        );
      }
      this.assertResultShape(row, request, targetClass);
      const revision = requiredPositive(row.lease_revision, "lease revision");
      const sameResult =
        (row.state === "completed" || row.state === "failed") &&
        revision === request.expectedLeaseRevision + 1 &&
        row.result_fingerprint === resultFingerprint;
      if (sameResult) return { status: "replayed", operation: rowToRecord(row) };
      if (row.state === "completed" || row.state === "failed") {
        throw new PortablePhaseOperationStoreError(
          "stale_revision",
          "portable phase result differs",
        );
      }
      if (
        row.state !== "claimed" ||
        row.lease_expires_at === null ||
        new Date(row.lease_expires_at) <= now
      ) {
        throw new PortablePhaseOperationStoreError("expired", "portable phase claim is not active");
      }
      if (revision !== request.expectedLeaseRevision) {
        throw new PortablePhaseOperationStoreError(
          "stale_revision",
          "portable phase lease revision is stale",
        );
      }
      if (
        new Date(request.completedAt) > now ||
        row.claimed_at === null ||
        new Date(request.completedAt) < new Date(row.claimed_at) ||
        new Date(request.completedAt) < new Date(row.updated_at)
      ) {
        throw new PortablePhaseOperationStoreError(
          "invalid",
          "portable phase completion instant is outside the claim",
        );
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_phase_operations
        SET state = ${request.resultStatus}, lease_revision = lease_revision + 1,
            result_ref = ${request.resultRef}, result_fingerprint = ${resultFingerprint},
            result_status = ${request.resultStatus},
            result_checkpoint_ref = ${request.checkpointRef},
            result_checkpoint_object_ref = ${request.checkpointObjectRef},
            result_checkpoint_digest = ${request.checkpointDigest},
            result_checkpoint_manifest_digest = ${request.checkpointManifestDigest},
            result_destination_activation_receipt_json = ${
              request.destinationActivationReceipt === null
                ? null
                : JSON.stringify(request.destinationActivationReceipt)
            }::jsonb,
            result_evidence_refs_json = ${JSON.stringify(request.evidenceRefs)}::jsonb,
            error_ref = ${request.errorRef}, completed_at = ${request.completedAt},
            updated_at = ${request.completedAt}
        WHERE claim_ref = ${request.claimRef} AND state = 'claimed'
          AND lease_revision = ${revision}
        RETURNING *
      `;
      return {
        status: request.resultStatus,
        operation: rowToRecord(requiredRow(updated, "complete")),
      };
    });
  }

  async expire(at: string = this.now()): Promise<number> {
    const instant = new Date(at);
    if (Number.isNaN(instant.valueOf())) {
      throw new PortablePhaseOperationStoreError(
        "invalid",
        "portable phase expiry instant is invalid",
      );
    }
    return this.sql.begin(async (tx) => {
      const rows: Array<{ operation_ref: string }> = await tx`
        UPDATE khala_sync_portable_phase_operations
        SET state = 'expired', lease_revision = CASE
              WHEN lease_revision IS NULL THEN NULL ELSE lease_revision + 1 END,
            result_ref = 'result.portable-phase-expired.' || substring(request_fingerprint FROM 8),
            result_status = 'expired', completed_at = ${instant.toISOString()},
            updated_at = ${instant.toISOString()}
        WHERE (state = 'pending' AND expires_at <= ${instant.toISOString()})
           OR (state = 'claimed' AND lease_expires_at <= ${instant.toISOString()})
        RETURNING operation_ref
      `;
      return rows.length;
    });
  }

  private assertCheckpointRequestShape(request: PortablePhaseOperationRequest): void {
    const needsCheckpointArtifact =
      request.kind === "checkpoint-stage" || request.kind === "destination-activate";
    const hasCompleteCheckpoint =
      request.checkpointRef !== null &&
      request.checkpointObjectRef !== null &&
      request.checkpointDigest !== null;
    const hasAnyCheckpoint =
      request.checkpointRef !== null ||
      request.checkpointObjectRef !== null ||
      request.checkpointDigest !== null;
    const createsCheckpoint = request.kind === "checkpoint-create";
    const hasCheckpointIdentityOnly =
      request.checkpointRef !== null &&
      request.checkpointObjectRef === null &&
      request.checkpointDigest === null;
    if (
      (needsCheckpointArtifact && !hasCompleteCheckpoint) ||
      (createsCheckpoint && !hasCheckpointIdentityOnly) ||
      (!needsCheckpointArtifact && !createsCheckpoint && hasAnyCheckpoint)
    ) {
      throw new PortablePhaseOperationStoreError(
        "invalid",
        "portable phase checkpoint refs are invalid",
      );
    }
  }

  private assertExecutionBinding(
    request: PortablePhaseOperationRequest,
    binding: ExecutionBindingRow,
    now: Date,
  ): void {
    if (
      binding.command_ref !== request.commandRef ||
      binding.owner_user_id !== request.ownerRef ||
      binding.session_ref !== request.sessionRef
    ) {
      throw new PortablePhaseOperationStoreError(
        "conflict",
        "portable phase command scope is not exact",
      );
    }
    if (
      !["claimed", "pending_reconcile"].includes(binding.state) ||
      new Date(binding.lease_expires_at) <= now ||
      new Date(request.expiresAt) > new Date(binding.lease_expires_at)
    ) {
      throw new PortablePhaseOperationStoreError(
        "expired",
        "portable command execution claim is not active",
      );
    }
    const generation = Number(binding.source_generation);
    if (isSourceKind(request.kind)) {
      if (
        request.attachmentRef !== binding.source_attachment_ref ||
        request.attachmentGeneration !== generation ||
        request.targetRef !== binding.executor_environment_ref
      ) {
        throw new PortablePhaseOperationStoreError(
          "stale_generation",
          "portable source phase binding is stale",
        );
      }
      return;
    }
    if (
      request.attachmentGeneration !== generation + 1 ||
      request.targetRef !== binding.destination_target_ref
    ) {
      throw new PortablePhaseOperationStoreError(
        "stale_generation",
        "portable destination phase binding is stale",
      );
    }
  }

  private assertClaimBinding(
    row: Row,
    request: {
      readonly sessionRef: string;
      readonly attachmentRef: string;
      readonly attachmentGeneration: number;
      readonly pylonRef: string;
      readonly targetRef: string;
    },
  ): void {
    if (
      row.session_ref !== request.sessionRef ||
      row.attachment_ref !== request.attachmentRef ||
      Number(row.attachment_generation) !== request.attachmentGeneration ||
      row.pylon_ref !== request.pylonRef ||
      row.target_ref !== request.targetRef
    ) {
      throw new PortablePhaseOperationStoreError(
        "conflict",
        "portable phase scope binding differs",
      );
    }
  }

  private assertMutationBinding(
    row: Row,
    request: {
      readonly sessionRef: string;
      readonly attachmentRef: string;
      readonly attachmentGeneration: number;
      readonly pylonRef: string;
      readonly targetRef: string;
      readonly workerInstanceRef: string;
      readonly claimGeneration: number;
    },
  ): void {
    this.assertClaimBinding(row, request);
    if (row.worker_instance_ref !== request.workerInstanceRef) {
      throw new PortablePhaseOperationStoreError(
        "conflict",
        "portable phase claim belongs to another worker",
      );
    }
    if (positive(row.claim_generation, "claim generation") !== request.claimGeneration) {
      throw new PortablePhaseOperationStoreError(
        "stale_generation",
        "portable phase claim generation is stale",
      );
    }
  }

  private assertResultShape(
    row: Row,
    request: ReturnType<typeof decodeResultRequest>,
    targetClass: string,
  ): void {
    const hasCompleteCheckpoint =
      request.checkpointRef !== null &&
      request.checkpointObjectRef !== null &&
      request.checkpointDigest !== null;
    const hasAnyCheckpoint =
      request.checkpointRef !== null ||
      request.checkpointObjectRef !== null ||
      request.checkpointDigest !== null;
    const hasCheckpointManifest = request.checkpointManifestDigest !== null;
    if (
      request.resultStatus === "completed" &&
      ((row.kind === "checkpoint-create" && !hasCompleteCheckpoint) ||
        (row.kind !== "checkpoint-create" && hasAnyCheckpoint) ||
        (row.kind !== "checkpoint-create" && hasCheckpointManifest) ||
        request.errorRef !== null)
    ) {
      throw new PortablePhaseOperationStoreError(
        "invalid",
        "portable phase completed result shape is invalid",
      );
    }
    if (
      request.resultStatus === "failed" &&
      (request.errorRef === null || hasAnyCheckpoint || hasCheckpointManifest)
    ) {
      throw new PortablePhaseOperationStoreError(
        "invalid",
        "portable phase failed result shape is invalid",
      );
    }
    const receipt = request.destinationActivationReceipt;
    if (request.resultStatus === "completed" && row.kind === "destination-activate") {
      if (receipt === null || row.checkpoint_ref === null) {
        throw new PortablePhaseOperationStoreError(
          "invalid",
          "portable destination activation result is incomplete",
        );
      }
      try {
        if (canonical(receipt.evidenceRefs) !== canonical(request.evidenceRefs)) {
          throw new Error("destination activation evidence differs");
        }
        validateIdePortableDestinationActivationReceipt(receipt, {
          operationRef: row.operation_ref,
          sessionRef: row.session_ref,
          checkpointRef: row.checkpoint_ref,
          destinationTargetRef: row.target_ref,
          destinationAttachmentRef: row.attachment_ref,
          destinationGeneration: requiredPositive(
            row.attachment_generation,
            "attachment generation",
          ),
          authenticationPolicyRef: `policy.portable.destination.${targetClass}.v1`,
          now: new Date(request.completedAt),
        });
      } catch {
        throw new PortablePhaseOperationStoreError(
          "invalid",
          "portable destination activation receipt is invalid",
        );
      }
    } else if (receipt !== null) {
      throw new PortablePhaseOperationStoreError(
        "invalid",
        "portable phase result has an unexpected destination activation receipt",
      );
    }
  }
}
