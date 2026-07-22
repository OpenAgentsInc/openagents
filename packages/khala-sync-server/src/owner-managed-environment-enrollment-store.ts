import { createHash } from "node:crypto";

import {
  OwnerManagedEnvironmentEnrollmentSchema,
  type OwnerManagedEnvironmentEnrollment,
  type OwnerManagedEnvironmentEnrollmentRequest,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import type { SqlTag, SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const DEFAULT_TTL_MS = 5 * 60 * 1_000;

export type OwnerManagedEnvironmentEnrollmentWrite = OwnerManagedEnvironmentEnrollmentRequest &
  Readonly<{
    idempotencyKeyHash: string;
    ownerUserId: string;
    ownerAgentUserId: string;
    targetRef: string;
    pylonRef: string;
  }>;

export class OwnerManagedEnvironmentEnrollmentError extends Schema.TaggedErrorClass<OwnerManagedEnvironmentEnrollmentError>()(
  "OwnerManagedEnvironmentEnrollmentError",
  {
    reason: Schema.Literals([
      "invalid",
      "not_found",
      "conflict",
      "stale_revision",
      "stale_generation",
      "authority_mismatch",
      "expired",
      "unavailable",
    ]),
  },
) {}

type EnrollmentRow = {
  enrollment_ref: string;
  owner_user_id: string;
  owner_agent_user_id: string;
  target_ref: string;
  pylon_ref: string;
  worker_instance_ref: string;
  adapter_ref: string;
  compatibility_ref: string;
  isolation: "owner_host_process" | "owner_host_container";
  checkpoint_key_ref: string;
  region_ref: string;
  network_destination_refs_json: unknown;
  data_destination_refs_json: unknown;
  retention_seconds: string | number;
  cost_policy_ref: string;
  generation: string | number;
  revision: string | number;
  state: "active" | "revoked";
  health: "ready" | "draining" | "offline" | "revoked";
  evidence_refs_json: unknown;
  observed_at: Date | string;
  expires_at: Date | string;
  revoked_at: Date | string | null;
};

const iso = (value: Date | string): string =>
  value instanceof Date ? value.toISOString() : new Date(value).toISOString();

const refs = (value: unknown): ReadonlyArray<string> => {
  let decoded = value;
  if (typeof value === "string") decoded = JSON.parse(value);
  if (
    !Array.isArray(decoded) ||
    decoded.length > 64 ||
    decoded.some((item) => typeof item !== "string" || !SAFE_REF.test(item))
  ) {
    throw new OwnerManagedEnvironmentEnrollmentError({ reason: "unavailable" });
  }
  return decoded as ReadonlyArray<string>;
};

const record = (row: EnrollmentRow): OwnerManagedEnvironmentEnrollment =>
  OwnerManagedEnvironmentEnrollmentSchema.make({
    schema: "openagents.owner_managed_environment_enrollment.v1",
    enrollmentRef: row.enrollment_ref,
    ownerRef: row.owner_user_id,
    targetRef: row.target_ref,
    pylonRef: row.pylon_ref,
    workerInstanceRef: row.worker_instance_ref,
    targetClass: "owner_managed",
    adapterRef: row.adapter_ref,
    compatibilityRef: row.compatibility_ref,
    isolation: row.isolation,
    dataPosture: "owner_managed_region",
    custodyPolicy: "owner_held_key",
    checkpointKeyRef: row.checkpoint_key_ref,
    regionRef: row.region_ref,
    networkDestinationRefs: refs(row.network_destination_refs_json),
    dataDestinationRefs: refs(row.data_destination_refs_json),
    retentionSeconds: Number(row.retention_seconds),
    costPolicyRef: row.cost_policy_ref,
    generation: Number(row.generation),
    revision: Number(row.revision),
    state: row.state,
    health: row.health,
    evidenceRefs: refs(row.evidence_refs_json),
    observedAt: iso(row.observed_at),
    expiresAt: iso(row.expires_at),
    revokedAt: row.revoked_at === null ? null : iso(row.revoked_at),
  });

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`;

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

const requestDigest = (input: OwnerManagedEnvironmentEnrollmentWrite): string =>
  `sha256:${createHash("sha256")
    .update(canonical({ ...input, expectedRevision: input.expectedRevision ?? null }))
    .digest("hex")}`;

const assertWrite = (input: OwnerManagedEnvironmentEnrollmentWrite): void => {
  const allRefs = [
    input.ownerUserId,
    input.ownerAgentUserId,
    input.targetRef,
    input.pylonRef,
    input.workerInstanceRef,
    input.adapterRef,
    input.compatibilityRef,
    input.checkpointKeyRef,
    input.regionRef,
    input.costPolicyRef,
    ...input.networkDestinationRefs,
    ...input.dataDestinationRefs,
    ...input.evidenceRefs,
  ];
  if (
    input.schema !== "openagents.owner_managed_environment_enrollment.request.v1" ||
    !SHA256.test(input.idempotencyKeyHash) ||
    !allRefs.every((value) => SAFE_REF.test(value)) ||
    new Set(input.networkDestinationRefs).size !== input.networkDestinationRefs.length ||
    new Set(input.dataDestinationRefs).size !== input.dataDestinationRefs.length ||
    new Set(input.evidenceRefs).size !== input.evidenceRefs.length ||
    input.networkDestinationRefs.length > 32 ||
    input.dataDestinationRefs.length > 32 ||
    input.evidenceRefs.length > 64
  ) {
    throw new OwnerManagedEnvironmentEnrollmentError({ reason: "invalid" });
  }
};

export class PostgresOwnerManagedEnvironmentEnrollmentStore {
  constructor(
    private readonly sql: SyncSql,
    private readonly now: () => string = () => new Date().toISOString(),
    private readonly ttlMs: number = DEFAULT_TTL_MS,
  ) {
    if (!Number.isSafeInteger(ttlMs) || ttlMs < 30_000 || ttlMs > DEFAULT_TTL_MS) {
      throw new OwnerManagedEnvironmentEnrollmentError({ reason: "invalid" });
    }
  }

  async admit(
    input: OwnerManagedEnvironmentEnrollmentWrite,
  ): Promise<OwnerManagedEnvironmentEnrollment> {
    assertWrite(input);
    const now = new Date(this.now());
    if (!Number.isFinite(now.getTime())) {
      throw new OwnerManagedEnvironmentEnrollmentError({ reason: "invalid" });
    }
    const digest = requestDigest(input);
    return this.sql.begin(async (sql) => {
      const replay: Array<{ request_digest: string; enrollment_ref: string }> = await sql`
        SELECT request_digest,enrollment_ref
        FROM khala_sync_owner_managed_environment_enrollment_events
        WHERE idempotency_key_hash=${input.idempotencyKeyHash}
        LIMIT 1`;
      if (replay[0] !== undefined) {
        if (replay[0].request_digest !== digest) {
          throw new OwnerManagedEnvironmentEnrollmentError({ reason: "conflict" });
        }
        const current = await this.readWith(sql, input.ownerUserId, input.targetRef);
        if (current === undefined || current.enrollmentRef !== replay[0].enrollment_ref) {
          throw new OwnerManagedEnvironmentEnrollmentError({ reason: "unavailable" });
        }
        return current;
      }

      await this.assertAuthority(sql, input, now);
      const rows: EnrollmentRow[] = await sql`
        SELECT * FROM khala_sync_owner_managed_environment_enrollments
        WHERE owner_user_id=${input.ownerUserId} AND target_ref=${input.targetRef}
        FOR UPDATE`;
      const current = rows[0] === undefined ? undefined : record(rows[0]);
      if (current === undefined && input.expectedRevision !== undefined) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "stale_revision" });
      }
      if (current !== undefined && current.revision !== input.expectedRevision) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "stale_revision" });
      }
      if (current !== undefined && input.generation < current.generation) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "stale_generation" });
      }
      if (
        current !== undefined &&
        current.state === "active" &&
        (current.pylonRef !== input.pylonRef ||
          current.workerInstanceRef !== input.workerInstanceRef ||
          current.checkpointKeyRef !== input.checkpointKeyRef ||
          current.adapterRef !== input.adapterRef ||
          current.compatibilityRef !== input.compatibilityRef ||
          current.isolation !== input.isolation)
      ) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "conflict" });
      }
      const enrollmentRef =
        current?.enrollmentRef ??
        stableRef("enrollment.owner-managed", `${input.ownerUserId}\0${input.targetRef}`);
      const revision = (current?.revision ?? 0) + 1;
      const expiresAt = new Date(now.getTime() + this.ttlMs);
      const networkJson = JSON.stringify(input.networkDestinationRefs);
      const dataJson = JSON.stringify(input.dataDestinationRefs);
      const evidenceJson = JSON.stringify(input.evidenceRefs);
      await sql`
        INSERT INTO khala_sync_owner_managed_environment_enrollments
          (enrollment_ref,owner_user_id,owner_agent_user_id,target_ref,pylon_ref,
           worker_instance_ref,adapter_ref,compatibility_ref,isolation,checkpoint_key_ref,
           region_ref,network_destination_refs_json,data_destination_refs_json,
           retention_seconds,cost_policy_ref,generation,revision,state,health,
           evidence_refs_json,observed_at,expires_at,revoked_at,created_at,updated_at)
        VALUES (${enrollmentRef},${input.ownerUserId},${input.ownerAgentUserId},${input.targetRef},
          ${input.pylonRef},${input.workerInstanceRef},${input.adapterRef},${input.compatibilityRef},
          ${input.isolation},${input.checkpointKeyRef},${input.regionRef},${networkJson}::jsonb,
          ${dataJson}::jsonb,${input.retentionSeconds},${input.costPolicyRef},${input.generation},
          ${revision},'active',${input.health},${evidenceJson}::jsonb,${now},${expiresAt},NULL,
          ${current === undefined ? now : new Date(current.observedAt)},${now})
        ON CONFLICT (owner_user_id,target_ref) DO UPDATE SET
          owner_agent_user_id=EXCLUDED.owner_agent_user_id,pylon_ref=EXCLUDED.pylon_ref,
          worker_instance_ref=EXCLUDED.worker_instance_ref,adapter_ref=EXCLUDED.adapter_ref,
          compatibility_ref=EXCLUDED.compatibility_ref,isolation=EXCLUDED.isolation,
          checkpoint_key_ref=EXCLUDED.checkpoint_key_ref,region_ref=EXCLUDED.region_ref,
          network_destination_refs_json=EXCLUDED.network_destination_refs_json,
          data_destination_refs_json=EXCLUDED.data_destination_refs_json,
          retention_seconds=EXCLUDED.retention_seconds,cost_policy_ref=EXCLUDED.cost_policy_ref,
          generation=EXCLUDED.generation,revision=EXCLUDED.revision,state='active',
          health=EXCLUDED.health,evidence_refs_json=EXCLUDED.evidence_refs_json,
          observed_at=EXCLUDED.observed_at,expires_at=EXCLUDED.expires_at,revoked_at=NULL,
          updated_at=EXCLUDED.updated_at`;
      await sql`
        INSERT INTO khala_sync_owner_managed_environment_enrollment_events
          (event_ref,idempotency_key_hash,request_digest,enrollment_ref,owner_user_id,
           owner_agent_user_id,target_ref,pylon_ref,generation,revision,event_kind,health,
           evidence_refs_json,created_at)
        VALUES (${stableRef("event.owner-managed-enrollment", input.idempotencyKeyHash)},
          ${input.idempotencyKeyHash},${digest},${enrollmentRef},${input.ownerUserId},
          ${input.ownerAgentUserId},${input.targetRef},${input.pylonRef},${input.generation},
          ${revision},${current === undefined ? "admitted" : "renewed"},${input.health},
          ${evidenceJson}::jsonb,${now})`;
      const stored = await this.readWith(sql, input.ownerUserId, input.targetRef);
      if (stored === undefined) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "unavailable" });
      }
      return stored;
    });
  }

  async revoke(
    input: OwnerManagedEnvironmentEnrollmentWrite,
  ): Promise<OwnerManagedEnvironmentEnrollment> {
    assertWrite(input);
    if (input.expectedRevision === undefined) {
      throw new OwnerManagedEnvironmentEnrollmentError({ reason: "stale_revision" });
    }
    const now = new Date(this.now());
    const digest = requestDigest(input);
    return this.sql.begin(async (sql) => {
      const replay: Array<{ request_digest: string; enrollment_ref: string }> = await sql`
        SELECT request_digest,enrollment_ref
        FROM khala_sync_owner_managed_environment_enrollment_events
        WHERE idempotency_key_hash=${input.idempotencyKeyHash}
        LIMIT 1`;
      if (replay[0] !== undefined) {
        if (replay[0].request_digest !== digest) {
          throw new OwnerManagedEnvironmentEnrollmentError({ reason: "conflict" });
        }
        const value = await this.readWith(sql, input.ownerUserId, input.targetRef);
        if (value === undefined || value.state !== "revoked") {
          throw new OwnerManagedEnvironmentEnrollmentError({ reason: "unavailable" });
        }
        return value;
      }
      const current = await this.readWith(sql, input.ownerUserId, input.targetRef, true);
      if (current === undefined) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "not_found" });
      }
      if (current.revision !== input.expectedRevision || current.generation !== input.generation) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "stale_revision" });
      }
      const revision = current.revision + 1;
      const evidenceJson = JSON.stringify(input.evidenceRefs);
      await sql`
        UPDATE khala_sync_owner_managed_environment_enrollments SET
          revision=${revision},state='revoked',health='revoked',revoked_at=${now},
          expires_at=${new Date(now.getTime() + 1_000)},evidence_refs_json=${evidenceJson}::jsonb,
          updated_at=${now}
        WHERE owner_user_id=${input.ownerUserId} AND target_ref=${input.targetRef}`;
      await sql`
        INSERT INTO khala_sync_owner_managed_environment_enrollment_events
          (event_ref,idempotency_key_hash,request_digest,enrollment_ref,owner_user_id,
           owner_agent_user_id,target_ref,pylon_ref,generation,revision,event_kind,health,
           evidence_refs_json,created_at)
        VALUES (${stableRef("event.owner-managed-enrollment", input.idempotencyKeyHash)},
          ${input.idempotencyKeyHash},${digest},${current.enrollmentRef},${input.ownerUserId},
          ${input.ownerAgentUserId},${input.targetRef},${input.pylonRef},${input.generation},
          ${revision},'revoked','revoked',${evidenceJson}::jsonb,${now})`;
      const stored = await this.readWith(sql, input.ownerUserId, input.targetRef);
      if (stored === undefined) {
        throw new OwnerManagedEnvironmentEnrollmentError({ reason: "unavailable" });
      }
      return stored;
    });
  }

  read(ownerUserId: string, targetRef: string) {
    return this.readWith(this.sql, ownerUserId, targetRef);
  }

  async resolveActive(ownerUserId: string, targetRef: string) {
    const now = new Date(this.now());
    const staleBefore = new Date(now.getTime() - DEFAULT_TTL_MS);
    const rows: EnrollmentRow[] = await this.sql`
      SELECT enrollment.*
      FROM khala_sync_owner_managed_environment_enrollments enrollment
      JOIN khala_sync_portable_targets target
        ON target.target_ref=enrollment.target_ref AND target.owner_user_id=enrollment.owner_user_id
      JOIN pylon_registrations pylon ON pylon.pylon_ref=enrollment.pylon_ref
      WHERE enrollment.owner_user_id=${ownerUserId} AND enrollment.target_ref=${targetRef}
        AND enrollment.state='active' AND enrollment.health='ready' AND enrollment.expires_at>${now}
        AND target.target_class='owner_managed' AND target.health='ready'
        AND pylon.owner_agent_user_id=enrollment.owner_agent_user_id
        AND pylon.status='active' AND pylon.archived_at IS NULL
        AND lower(coalesce(pylon.latest_heartbeat_status,''))='online'
        AND pylon.latest_heartbeat_at IS NOT NULL
        AND pylon.latest_heartbeat_at::timestamptz>=${staleBefore}
        AND NOT EXISTS (SELECT 1 FROM pylon_quarantines quarantine
          WHERE quarantine.pylon_ref=pylon.pylon_ref AND quarantine.state='active'
            AND quarantine.released_at IS NULL
            AND (quarantine.expires_at IS NULL OR quarantine.expires_at::timestamptz>${now}))
      LIMIT 1`;
    return rows[0] === undefined ? undefined : record(rows[0]);
  }

  private async readWith(sql: SqlTag, ownerUserId: string, targetRef: string, forUpdate = false) {
    const rows: EnrollmentRow[] = forUpdate
      ? await sql`SELECT * FROM khala_sync_owner_managed_environment_enrollments
          WHERE owner_user_id=${ownerUserId} AND target_ref=${targetRef} FOR UPDATE`
      : await sql`SELECT * FROM khala_sync_owner_managed_environment_enrollments
          WHERE owner_user_id=${ownerUserId} AND target_ref=${targetRef}`;
    return rows[0] === undefined ? undefined : record(rows[0]);
  }

  private async assertAuthority(
    sql: SqlTag,
    input: OwnerManagedEnvironmentEnrollmentWrite,
    now: Date,
  ) {
    const staleBefore = new Date(now.getTime() - DEFAULT_TTL_MS);
    const rows: Array<{ ok: boolean }> = await sql`
      SELECT true AS ok
      FROM khala_sync_portable_targets target
      JOIN pylon_registrations pylon ON pylon.pylon_ref=${input.pylonRef}
      WHERE target.target_ref=${input.targetRef}
        AND target.owner_user_id=${input.ownerUserId}
        AND target.target_class='owner_managed'
        AND target.adapter_ref=${input.adapterRef}
        AND target.compatibility_ref=${input.compatibilityRef}
        AND target.isolation=${input.isolation}
        AND target.data_posture='owner_managed_region'
        AND target.health='ready'
        AND pylon.owner_agent_user_id=${input.ownerAgentUserId}
        AND pylon.status='active' AND pylon.archived_at IS NULL
        AND lower(coalesce(pylon.latest_heartbeat_status,''))='online'
        AND pylon.latest_heartbeat_at IS NOT NULL
        AND pylon.latest_heartbeat_at::timestamptz>=${staleBefore}
        AND NOT EXISTS (SELECT 1 FROM pylon_quarantines quarantine
          WHERE quarantine.pylon_ref=pylon.pylon_ref AND quarantine.state='active'
            AND quarantine.released_at IS NULL
            AND (quarantine.expires_at IS NULL OR quarantine.expires_at::timestamptz>${now}))
      LIMIT 1`;
    if (rows[0] === undefined) {
      throw new OwnerManagedEnvironmentEnrollmentError({ reason: "authority_mismatch" });
    }
  }
}
