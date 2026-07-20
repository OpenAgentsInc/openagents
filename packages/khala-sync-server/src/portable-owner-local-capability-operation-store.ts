import { createHash } from "node:crypto";

import {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRecordSchema,
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
  PortableOwnerLocalCapabilityOperationRequestSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
  type PortableOwnerLocalCapabilityOperationRecord,
  type PortableOwnerLocalCapabilityOperationRequest,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import type { SyncSql, SyncTransactionSql } from "./sql.js";

export {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
  type PortableOwnerLocalCapabilityOperationClaimRequest,
  type PortableOwnerLocalCapabilityOperationRecord,
  type PortableOwnerLocalCapabilityOperationRenewRequest,
  type PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/portable-session-contract";

type Row = Readonly<{
  operation_ref: string;
  request_fingerprint: string;
  action: "install" | "wipe";
  capability: "provider" | "scm_read" | "scm_write" | "tool" | "api" | null;
  command_execution_claim_ref: string;
  owner_user_id: string;
  pylon_ref: string;
  session_ref: string;
  attachment_ref: string;
  attachment_generation: string | number;
  target_ref: string;
  source_lease_ref: string;
  source_grant_ref: string;
  destination_lease_ref: string;
  destination_grant_ref: string;
  installation_ref: string | null;
  permission_refs_json: unknown;
  permission_fingerprint: string;
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
  result_installation_ref: string | null;
  receipt_ref: string | null;
  result_evidence_refs_json: unknown;
  error_ref: string | null;
  completed_at: Date | string | null;
  updated_at: Date | string;
}>;

type CommandBindingRow = Readonly<{
  owner_user_id: string;
  session_ref: string;
  source_attachment_ref: string;
  source_generation: string | number;
  destination_target_ref: string;
  executor_environment_ref: string;
  state: string;
  lease_expires_at: Date | string;
}>;

type TargetBindingRow = Readonly<{
  target_class: string;
  state: string;
  health: string;
  expires_at: Date | string;
}>;

export class PortableOwnerLocalCapabilityOperationStoreError extends Schema.TaggedErrorClass<PortableOwnerLocalCapabilityOperationStoreError>()(
  "PortableOwnerLocalCapabilityOperationStoreError",
  {
    code: Schema.Literals([
      "invalid",
      "not_found",
      "conflict",
      "not_claimable",
      "expired",
      "stale_generation",
      "stale_revision",
      "authority_unavailable",
      "unsafe_material",
    ]),
    detail: Schema.String,
  },
) {}

const decodeRequest = Schema.decodeUnknownSync(PortableOwnerLocalCapabilityOperationRequestSchema);
const decodeClaim = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
);
const decodeRenew = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
);
const decodeResult = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
);
const decodeRecord = Schema.decodeUnknownSync(PortableOwnerLocalCapabilityOperationRecordSchema);
const decodeRef = Schema.decodeUnknownSync(
  Schema.String.check(
    Schema.isMinLength(3),
    Schema.isMaxLength(256),
    Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
  ),
);

const forbiddenPrivateMaterial =
  /"(?:material|bytes|base64|endpoint|bearer|token|apiKey|authorization|credential|secret|localPath|hostname|processId|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/iu;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      // This is a new array. Sorting it does not mutate caller state.
      // eslint-disable-next-line unicorn/no-array-sort
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const fingerprint = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;

export const portableOwnerLocalCapabilityPermissionFingerprint = (
  permissionRefs: ReadonlyArray<string>,
): `sha256:${string}` => fingerprint(permissionRefs);

export const portableOwnerLocalCapabilityOperationRef = (
  request: Omit<
    PortableOwnerLocalCapabilityOperationRequest,
    "schema" | "operationRef" | "permissionFingerprint" | "expiresAt"
  >,
): string =>
  `operation.owner-local-capability.${createHash("sha256")
    .update(canonical(request))
    .digest("hex")}`;

const fail = (code: PortableOwnerLocalCapabilityOperationStoreError["code"], detail: string) =>
  new PortableOwnerLocalCapabilityOperationStoreError({ code, detail });

const assertPublicSafe = (value: unknown): void => {
  if (forbiddenPrivateMaterial.test(canonical(value))) {
    throw fail(
      "unsafe_material",
      "owner-local capability operation contains forbidden private material",
    );
  }
};

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;
const iso = (value: Date | string): string => new Date(value).toISOString();

const positive = (value: string | number | null, field: string): number | null => {
  if (value === null) return null;
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw fail("invalid", `${field} is invalid`);
  }
  return number;
};

const requiredPositive = (value: string | number | null, field: string): number => {
  const number = positive(value, field);
  if (number === null) throw fail("invalid", `${field} is missing`);
  return number;
};

const rowToRecord = (row: Row): PortableOwnerLocalCapabilityOperationRecord => {
  const request = decodeRequest(parseJson(row.request_json));
  assertPublicSafe(request);
  if (
    fingerprint(request) !== row.request_fingerprint ||
    request.operationRef !== row.operation_ref ||
    request.action !== row.action ||
    request.capability !== row.capability ||
    request.commandExecutionClaimRef !== row.command_execution_claim_ref ||
    request.ownerRef !== row.owner_user_id ||
    request.pylonRef !== row.pylon_ref ||
    request.sessionRef !== row.session_ref ||
    request.attachmentRef !== row.attachment_ref ||
    request.attachmentGeneration !==
      requiredPositive(row.attachment_generation, "attachment generation") ||
    request.targetRef !== row.target_ref ||
    request.sourceLeaseRef !== row.source_lease_ref ||
    request.sourceGrantRef !== row.source_grant_ref ||
    request.destinationLeaseRef !== row.destination_lease_ref ||
    request.destinationGrantRef !== row.destination_grant_ref ||
    request.installationRef !== row.installation_ref ||
    canonical(request.permissionRefs) !== canonical(parseJson(row.permission_refs_json)) ||
    request.permissionFingerprint !== row.permission_fingerprint ||
    iso(row.expires_at) !== new Date(request.expiresAt).toISOString()
  ) {
    throw fail("invalid", "persisted owner-local capability binding is invalid");
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
    resultInstallationRef: row.result_installation_ref,
    receiptRef: row.receipt_ref,
    resultEvidenceRefs: parseJson(row.result_evidence_refs_json),
    errorRef: row.error_ref,
    completedAt: row.completed_at === null ? null : iso(row.completed_at),
    updatedAt: iso(row.updated_at),
  });
};

const selectByClaim = async (
  sql: SyncTransactionSql,
  claimRef: string,
): Promise<Row | undefined> => {
  const rows: Row[] = await sql`
    SELECT * FROM khala_sync_portable_owner_local_capability_operations
    WHERE claim_ref = ${claimRef}
    FOR UPDATE
  `;
  return rows[0];
};

/** A durable refs-only queue between command dispatch and an outbound-poll Pylon. */
export class PostgresPortableOwnerLocalCapabilityOperationStore {
  constructor(
    private readonly sql: SyncSql,
    private readonly now: () => string = () => new Date().toISOString(),
  ) {}

  async enqueue(input: unknown): Promise<{
    readonly status: "enqueued" | "replayed";
    readonly operation: PortableOwnerLocalCapabilityOperationRecord;
  }> {
    assertPublicSafe(input);
    let request: PortableOwnerLocalCapabilityOperationRequest;
    try {
      request = decodeRequest(input);
    } catch {
      throw fail("invalid", "owner-local capability operation request is invalid");
    }
    this.assertRequestIdentity(request);
    const now = new Date(this.now());
    if (!Number.isFinite(now.valueOf())) throw fail("invalid", "current instant is invalid");
    if (new Date(request.expiresAt) <= now) {
      throw fail("expired", "owner-local capability operation request expired");
    }
    const requestFingerprint = fingerprint(request);

    return this.sql.begin(async (tx) => {
      const existingRows: Row[] = await tx`
        SELECT * FROM khala_sync_portable_owner_local_capability_operations
        WHERE operation_ref = ${request.operationRef}
        FOR UPDATE
      `;
      const existing = existingRows[0];
      if (existing !== undefined) {
        if (existing.request_fingerprint === requestFingerprint) {
          await this.assertCurrentAuthority(tx, request, now);
          return { status: "replayed", operation: rowToRecord(existing) };
        }
        throw fail("conflict", "operation ref has different request bytes");
      }
      await this.assertCurrentAuthority(tx, request, now);
      const scopeRows: Row[] = await tx`
        SELECT * FROM khala_sync_portable_owner_local_capability_operations
        WHERE command_execution_claim_ref = ${request.commandExecutionClaimRef}
          AND action = ${request.action}
          AND source_lease_ref = ${request.sourceLeaseRef}
          AND destination_lease_ref = ${request.destinationLeaseRef}
          AND target_ref = ${request.targetRef}
        FOR UPDATE
      `;
      const scoped = scopeRows[0];
      if (
        scoped !== undefined &&
        scoped.operation_ref === request.operationRef &&
        scoped.request_fingerprint === requestFingerprint
      ) {
        return { status: "replayed", operation: rowToRecord(scoped) };
      }
      if (scoped !== undefined) {
        throw fail("conflict", "capability operation scope has a different operation ref");
      }
      const inserted: Row[] = await tx`
        INSERT INTO khala_sync_portable_owner_local_capability_operations
          (operation_ref, request_fingerprint, action, capability, command_execution_claim_ref,
           owner_user_id, pylon_ref, session_ref, attachment_ref, attachment_generation,
           target_ref, source_lease_ref, source_grant_ref, destination_lease_ref,
           destination_grant_ref, installation_ref, permission_refs_json, permission_fingerprint,
           request_json, expires_at, state, created_at, updated_at)
        VALUES
          (${request.operationRef}, ${requestFingerprint}, ${request.action}, ${request.capability},
           ${request.commandExecutionClaimRef}, ${request.ownerRef}, ${request.pylonRef},
           ${request.sessionRef}, ${request.attachmentRef}, ${request.attachmentGeneration},
           ${request.targetRef}, ${request.sourceLeaseRef}, ${request.sourceGrantRef},
           ${request.destinationLeaseRef}, ${request.destinationGrantRef}, ${request.installationRef},
           ${JSON.stringify(request.permissionRefs)}::text::jsonb, ${request.permissionFingerprint},
           ${JSON.stringify(request)}::text::jsonb, ${request.expiresAt}, 'pending',
           ${now.toISOString()}, ${now.toISOString()})
        RETURNING *
      `;
      const row = inserted[0];
      if (row === undefined) throw fail("conflict", "enqueue did not insert an operation");
      return { status: "enqueued", operation: rowToRecord(row) };
    });
  }

  async pending(
    ownerRefInput: unknown,
    pylonRefInput: unknown,
    targetRefInput: unknown,
    limit = 32,
  ): Promise<ReadonlyArray<PortableOwnerLocalCapabilityOperationRecord>> {
    const ownerRef = this.decodeOwner(ownerRefInput);
    const { pylonRef, targetRef } = this.decodeTargetBinding(pylonRefInput, targetRefInput);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
      throw fail("invalid", "owner-local capability queue limit is invalid");
    }
    const rows: Row[] = await this.sql`
      SELECT operation.*
      FROM khala_sync_portable_owner_local_capability_operations AS operation
      WHERE operation.owner_user_id = ${ownerRef}
        AND operation.pylon_ref = ${pylonRef}
        AND operation.target_ref = ${targetRef}
        AND operation.state = 'pending'
        AND operation.expires_at > ${this.now()}
        AND EXISTS (
          SELECT 1 FROM khala_sync_portable_command_executions AS execution
          WHERE execution.claim_ref = operation.command_execution_claim_ref
            AND execution.owner_user_id = operation.owner_user_id
            AND execution.session_ref = operation.session_ref
            AND execution.state IN ('claimed', 'pending_reconcile')
            AND execution.lease_expires_at > ${this.now()}
        )
        AND EXISTS (
          SELECT 1 FROM khala_sync_portable_target_pylon_bindings AS binding
          WHERE binding.owner_user_id = operation.owner_user_id
            AND binding.session_ref = operation.session_ref
            AND binding.target_ref = operation.target_ref
            AND binding.pylon_ref = operation.pylon_ref
            AND binding.state = 'active'
            AND binding.health IN ('ready', 'draining')
            AND binding.expires_at > ${this.now()}
        )
      ORDER BY operation.created_at, operation.operation_ref
      LIMIT ${limit}
    `;
    return rows.map(rowToRecord);
  }

  async read(
    ownerRefInput: unknown,
    pylonRefInput: unknown,
    targetRefInput: unknown,
    operationRefInput: unknown,
  ): Promise<PortableOwnerLocalCapabilityOperationRecord> {
    const ownerRef = this.decodeOwner(ownerRefInput);
    const { pylonRef, targetRef } = this.decodeTargetBinding(pylonRefInput, targetRefInput);
    let operationRef: string;
    try {
      operationRef = decodeRef(operationRefInput);
    } catch {
      throw fail("invalid", "owner-local capability operation ref is invalid");
    }
    const rows: Row[] = await this.sql`
      SELECT operation.*
      FROM khala_sync_portable_owner_local_capability_operations AS operation
      WHERE operation.operation_ref = ${operationRef}
        AND operation.owner_user_id = ${ownerRef}
        AND operation.pylon_ref = ${pylonRef}
        AND operation.target_ref = ${targetRef}
        AND EXISTS (
          SELECT 1 FROM khala_sync_portable_command_executions AS execution
          WHERE execution.claim_ref = operation.command_execution_claim_ref
            AND execution.owner_user_id = operation.owner_user_id
            AND execution.session_ref = operation.session_ref
            AND execution.state IN ('claimed', 'pending_reconcile')
            AND execution.lease_expires_at > ${this.now()}
        )
        AND EXISTS (
          SELECT 1 FROM khala_sync_portable_target_pylon_bindings AS binding
          WHERE binding.owner_user_id = operation.owner_user_id
            AND binding.session_ref = operation.session_ref
            AND binding.target_ref = operation.target_ref
            AND binding.pylon_ref = operation.pylon_ref
            AND binding.state = 'active'
            AND binding.health IN ('ready', 'draining')
            AND binding.expires_at > ${this.now()}
        )
      LIMIT 1
    `;
    const row = rows[0];
    if (row === undefined) throw fail("not_found", "operation does not exist in target scope");
    return rowToRecord(row);
  }

  async claim(
    ownerRefInput: unknown,
    input: unknown,
  ): Promise<{
    readonly status: "claimed" | "replayed";
    readonly operation: PortableOwnerLocalCapabilityOperationRecord;
  }> {
    const ownerRef = this.decodeOwner(ownerRefInput);
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeClaim>;
    try {
      request = decodeClaim(input);
    } catch {
      throw fail("invalid", "owner-local capability claim is invalid");
    }
    const now = new Date(this.now());
    const leaseExpiry = new Date(request.leaseExpiresAt);
    if (leaseExpiry <= now) throw fail("invalid", "claim lease is not future");
    const claimFingerprint = fingerprint(request);
    return this.sql.begin(async (tx) => {
      const rows: Row[] = await tx`
        SELECT * FROM khala_sync_portable_owner_local_capability_operations
        WHERE operation_ref = ${request.operationRef}
        FOR UPDATE SKIP LOCKED
      `;
      const row = rows[0];
      if (row === undefined) {
        throw fail("not_claimable", "operation is absent or claimed by another transaction");
      }
      this.assertActorOwner(row, ownerRef);
      this.assertClaimBinding(row, request);
      await this.assertCurrentAuthority(tx, rowToRecord(row).request, now);
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
        throw fail(
          row.state === "expired" ||
            (row.lease_expires_at !== null && new Date(row.lease_expires_at) <= now)
            ? "expired"
            : "conflict",
          "operation already has a claim",
        );
      }
      if (row.state !== "pending") throw fail("not_claimable", "operation is terminal");
      if (new Date(row.expires_at) <= now || leaseExpiry > new Date(row.expires_at)) {
        throw fail("expired", "claim lease is outside operation expiry");
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_owner_local_capability_operations
        SET state = 'claimed', claim_ref = ${request.claimRef},
            claim_fingerprint = ${claimFingerprint},
            worker_instance_ref = ${request.workerInstanceRef},
            claim_generation = 1, lease_revision = 1,
            claimed_at = ${now.toISOString()}, lease_expires_at = ${request.leaseExpiresAt},
            updated_at = ${now.toISOString()}
        WHERE operation_ref = ${request.operationRef} AND state = 'pending'
        RETURNING *
      `;
      const updatedRow = updated[0];
      if (updatedRow === undefined) throw fail("not_claimable", "claim CAS failed");
      return { status: "claimed", operation: rowToRecord(updatedRow) };
    });
  }

  async renew(
    ownerRefInput: unknown,
    input: unknown,
  ): Promise<{
    readonly status: "renewed" | "replayed";
    readonly operation: PortableOwnerLocalCapabilityOperationRecord;
  }> {
    const ownerRef = this.decodeOwner(ownerRefInput);
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeRenew>;
    try {
      request = decodeRenew(input);
    } catch {
      throw fail("invalid", "owner-local capability renewal is invalid");
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const row = await selectByClaim(tx, request.claimRef);
      if (row === undefined) throw fail("not_found", "claim does not exist");
      this.assertActorOwner(row, ownerRef);
      this.assertMutationBinding(row, request);
      await this.assertCurrentAuthority(tx, rowToRecord(row).request, now);
      const revision = requiredPositive(row.lease_revision, "lease revision");
      if (
        row.state !== "claimed" ||
        row.lease_expires_at === null ||
        new Date(row.lease_expires_at) <= now
      ) {
        throw fail("expired", "claim is not active");
      }
      if (
        revision === request.expectedLeaseRevision + 1 &&
        iso(row.lease_expires_at) === new Date(request.leaseExpiresAt).toISOString()
      ) {
        return { status: "replayed", operation: rowToRecord(row) };
      }
      if (revision !== request.expectedLeaseRevision) {
        throw fail("stale_revision", "lease revision is stale");
      }
      const next = new Date(request.leaseExpiresAt);
      if (!(next > new Date(row.lease_expires_at)) || next > new Date(row.expires_at)) {
        throw fail("invalid", "renewal exceeds operation bounds");
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_owner_local_capability_operations
        SET lease_revision = lease_revision + 1,
            lease_expires_at = ${request.leaseExpiresAt}, updated_at = ${now.toISOString()}
        WHERE claim_ref = ${request.claimRef} AND state = 'claimed'
          AND lease_revision = ${revision}
        RETURNING *
      `;
      const updatedRow = updated[0];
      if (updatedRow === undefined) throw fail("stale_revision", "renew CAS failed");
      return { status: "renewed", operation: rowToRecord(updatedRow) };
    });
  }

  async complete(
    ownerRefInput: unknown,
    input: unknown,
  ): Promise<{
    readonly status: "completed" | "failed" | "replayed";
    readonly operation: PortableOwnerLocalCapabilityOperationRecord;
  }> {
    const ownerRef = this.decodeOwner(ownerRefInput);
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeResult>;
    try {
      request = decodeResult(input);
    } catch {
      throw fail("invalid", "owner-local capability result is invalid");
    }
    this.assertResultShape(request);
    const now = new Date(this.now());
    const resultFingerprint = fingerprint(request);
    return this.sql.begin(async (tx) => {
      const row = await selectByClaim(tx, request.claimRef);
      if (row === undefined) throw fail("not_found", "claim does not exist");
      this.assertActorOwner(row, ownerRef);
      this.assertMutationBinding(row, request);
      this.assertActionResultShape(row.action, request);
      await this.assertCurrentAuthority(tx, rowToRecord(row).request, now);
      const revision = requiredPositive(row.lease_revision, "lease revision");
      if (
        (row.state === "completed" || row.state === "failed") &&
        revision === request.expectedLeaseRevision + 1 &&
        row.result_fingerprint === resultFingerprint
      ) {
        return { status: "replayed", operation: rowToRecord(row) };
      }
      if (row.state === "completed" || row.state === "failed") {
        throw fail("stale_revision", "terminal result differs");
      }
      if (
        row.state !== "claimed" ||
        row.lease_expires_at === null ||
        new Date(row.lease_expires_at) <= now
      ) {
        throw fail("expired", "claim is not active");
      }
      if (revision !== request.expectedLeaseRevision) {
        throw fail("stale_revision", "lease revision is stale");
      }
      if (
        row.claimed_at === null ||
        new Date(request.completedAt) > now ||
        new Date(request.completedAt) < new Date(row.claimed_at) ||
        new Date(request.completedAt) < new Date(row.updated_at)
      ) {
        throw fail("invalid", "completion instant is outside the active claim");
      }
      const updated: Row[] = await tx`
        UPDATE khala_sync_portable_owner_local_capability_operations
        SET state = ${request.resultStatus}, lease_revision = lease_revision + 1,
            result_ref = ${request.resultRef}, result_fingerprint = ${resultFingerprint},
            result_status = ${request.resultStatus},
            result_installation_ref = ${request.resultInstallationRef},
            receipt_ref = ${request.receiptRef},
            result_evidence_refs_json = ${JSON.stringify(request.evidenceRefs)}::text::jsonb,
            error_ref = ${request.errorRef}, completed_at = ${request.completedAt},
            updated_at = ${request.completedAt}
        WHERE claim_ref = ${request.claimRef} AND state = 'claimed'
          AND lease_revision = ${revision}
        RETURNING *
      `;
      const updatedRow = updated[0];
      if (updatedRow === undefined) throw fail("stale_revision", "completion CAS failed");
      return { status: request.resultStatus, operation: rowToRecord(updatedRow) };
    });
  }

  async expire(at: string = this.now()): Promise<number> {
    const instant = new Date(at);
    if (!Number.isFinite(instant.valueOf())) throw fail("invalid", "expiry instant is invalid");
    const rows: Array<{ operation_ref: string }> = await this.sql`
      UPDATE khala_sync_portable_owner_local_capability_operations
      SET state = 'expired',
          lease_revision = CASE WHEN lease_revision IS NULL THEN NULL ELSE lease_revision + 1 END,
          result_ref = 'result.owner-local-capability-expired.' || substring(request_fingerprint FROM 8),
          result_status = 'expired', completed_at = ${instant.toISOString()},
          updated_at = ${instant.toISOString()}
      WHERE (state = 'pending' AND expires_at <= ${instant.toISOString()})
         OR (state = 'claimed' AND lease_expires_at <= ${instant.toISOString()})
      RETURNING operation_ref
    `;
    return rows.length;
  }

  private assertRequestIdentity(request: PortableOwnerLocalCapabilityOperationRequest): void {
    const permissionRefs = [...request.permissionRefs];
    // This is a new array. Sorting it does not mutate caller state.
    // eslint-disable-next-line unicorn/no-array-sort
    permissionRefs.sort((left, right) => left.localeCompare(right));
    if (
      new Set(permissionRefs).size !== permissionRefs.length ||
      canonical(permissionRefs) !== canonical(request.permissionRefs) ||
      portableOwnerLocalCapabilityPermissionFingerprint(request.permissionRefs) !==
        request.permissionFingerprint ||
      request.sourceLeaseRef === request.destinationLeaseRef ||
      request.sourceGrantRef === request.destinationGrantRef ||
      (request.action === "install" &&
        (request.capability === null ||
          request.installationRef !== null ||
          permissionRefs.length === 0)) ||
      (request.action === "wipe" &&
        (request.capability !== null ||
          request.installationRef === null ||
          permissionRefs.length !== 0))
    ) {
      throw fail("invalid", "capability refs or permission fingerprint are invalid");
    }
    const {
      schema: _schema,
      operationRef: _operationRef,
      permissionFingerprint: _permissionFingerprint,
      expiresAt: _expiresAt,
      ...operationIdentity
    } = request;
    if (portableOwnerLocalCapabilityOperationRef(operationIdentity) !== request.operationRef) {
      throw fail("invalid", "owner-local capability operation ref is not deterministic");
    }
  }

  private async assertCurrentAuthority(
    sql: SyncTransactionSql,
    request: PortableOwnerLocalCapabilityOperationRequest,
    now: Date,
  ): Promise<void> {
    const commandRows: CommandBindingRow[] = await sql`
      SELECT owner_user_id, session_ref, source_attachment_ref, source_generation,
             destination_target_ref, executor_environment_ref, state, lease_expires_at
      FROM khala_sync_portable_command_executions
      WHERE claim_ref = ${request.commandExecutionClaimRef}
      FOR UPDATE
    `;
    const command = commandRows[0];
    if (command === undefined) throw fail("not_found", "command execution claim does not exist");
    if (
      command.owner_user_id !== request.ownerRef ||
      command.session_ref !== request.sessionRef ||
      !["claimed", "pending_reconcile"].includes(command.state)
    ) {
      throw fail("conflict", "command execution claim scope is not exact");
    }
    if (
      new Date(command.lease_expires_at) <= now ||
      new Date(request.expiresAt) > new Date(command.lease_expires_at)
    ) {
      throw fail("expired", "command execution claim is not active");
    }
    const sourceGeneration = Number(command.source_generation);
    const sourceBound =
      request.attachmentRef === command.source_attachment_ref &&
      request.attachmentGeneration === sourceGeneration &&
      request.targetRef === command.executor_environment_ref;
    const destinationBound =
      request.attachmentGeneration === sourceGeneration + 1 &&
      request.targetRef === command.destination_target_ref;
    if (
      (request.action === "wipe" && !sourceBound) ||
      (request.action === "install" && !destinationBound)
    ) {
      throw fail("stale_generation", "capability operation attachment binding is stale");
    }

    const targetRows: TargetBindingRow[] = await sql`
      SELECT target.target_class, binding.state, binding.health, binding.expires_at
      FROM khala_sync_portable_targets AS target
      JOIN khala_sync_portable_target_pylon_bindings AS binding
        ON binding.owner_user_id = target.owner_user_id
       AND binding.target_ref = target.target_ref
      WHERE target.owner_user_id = ${request.ownerRef}
        AND target.target_ref = ${request.targetRef}
        AND binding.session_ref = ${request.sessionRef}
        AND binding.pylon_ref = ${request.pylonRef}
      FOR SHARE OF binding
    `;
    const target = targetRows[0];
    if (
      target === undefined ||
      target.target_class !== "owner_local" ||
      target.state !== "active" ||
      !["ready", "draining"].includes(target.health)
    ) {
      throw fail("authority_unavailable", "exact owner-local target Pylon binding is unavailable");
    }
    if (new Date(target.expires_at) <= now) {
      throw fail("expired", "owner-local target Pylon binding expired");
    }
  }

  private decodeTargetBinding(
    pylonRefInput: unknown,
    targetRefInput: unknown,
  ): Readonly<{ pylonRef: string; targetRef: string }> {
    try {
      return { pylonRef: decodeRef(pylonRefInput), targetRef: decodeRef(targetRefInput) };
    } catch {
      throw fail("invalid", "owner-local capability target binding is invalid");
    }
  }

  private decodeOwner(ownerRefInput: unknown): string {
    try {
      return decodeRef(ownerRefInput);
    } catch {
      throw fail("invalid", "owner-local capability owner binding is invalid");
    }
  }

  private assertActorOwner(row: Row, ownerRef: string): void {
    if (row.owner_user_id !== ownerRef) {
      throw fail("conflict", "owner-local capability operation belongs to another owner");
    }
  }

  private assertClaimBinding(
    row: Row,
    request: Readonly<{
      pylonRef: string;
      targetRef: string;
      sessionRef: string;
      attachmentRef: string;
      attachmentGeneration: number;
    }>,
  ): void {
    if (
      row.pylon_ref !== request.pylonRef ||
      row.target_ref !== request.targetRef ||
      row.session_ref !== request.sessionRef ||
      row.attachment_ref !== request.attachmentRef ||
      Number(row.attachment_generation) !== request.attachmentGeneration
    ) {
      throw fail("conflict", "owner-local capability claim scope differs");
    }
  }

  private assertMutationBinding(
    row: Row,
    request: Readonly<{
      pylonRef: string;
      targetRef: string;
      sessionRef: string;
      attachmentRef: string;
      attachmentGeneration: number;
      workerInstanceRef: string;
      claimGeneration: number;
    }>,
  ): void {
    this.assertClaimBinding(row, request);
    if (row.worker_instance_ref !== request.workerInstanceRef) {
      throw fail("conflict", "claim belongs to another worker");
    }
    if (positive(row.claim_generation, "claim generation") !== request.claimGeneration) {
      throw fail("stale_generation", "claim generation is stale");
    }
  }

  private assertResultShape(request: ReturnType<typeof decodeResult>): void {
    if (
      (request.resultStatus === "failed" &&
        (request.resultInstallationRef !== null ||
          request.receiptRef !== null ||
          request.errorRef === null)) ||
      (request.resultStatus === "completed" && request.errorRef !== null)
    ) {
      throw fail("invalid", "terminal receipt shape is invalid");
    }
  }

  private assertActionResultShape(
    action: "install" | "wipe",
    request: ReturnType<typeof decodeResult>,
  ): void {
    if (request.resultStatus === "failed") return;
    const installResult =
      action === "install" &&
      request.resultInstallationRef !== null &&
      request.receiptRef !== null &&
      request.evidenceRefs.length === 1;
    const wipeResult =
      action === "wipe" &&
      request.resultInstallationRef === null &&
      request.receiptRef !== null &&
      request.evidenceRefs.length === 0;
    if (!installResult && !wipeResult) {
      throw fail("invalid", "terminal result does not match the queued action");
    }
  }
}
