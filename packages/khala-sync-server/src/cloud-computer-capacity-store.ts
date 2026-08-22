import { createHash } from "node:crypto";

import {
  evaluateCloudComputerAdmission,
  deterministicStartJitterMs,
  scheduleCloudComputerCapacity,
  validateCloudComputerCapacityDemand,
  validateCloudComputerCapacityObservation,
  validateCloudComputerCapacityPolicy,
  type CapacityResources,
  type CloudComputerCapacityDemand,
  type CloudComputerCapacityObservation,
  type CloudComputerCapacityPolicy,
  type CloudComputerCapacityReceipt,
  type CloudComputerCapacityUsage,
  type CloudComputerRuntimeClass,
} from "./cloud-computer-capacity.js";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const GLOBAL_CAPACITY_LOCK = "openagents.cloud-computer.capacity.global";

type ReservationStatus =
  | "queued"
  | "reserved"
  | "provisioning"
  | "provisioning_uncertain"
  | "starting"
  | "active"
  | "releasing"
  | "release_uncertain"
  | "released"
  | "expired"
  | "quarantined"
  | "refused";

interface ReservationRow {
  readonly reservation_ref: string;
  readonly command_ref: string;
  readonly request_digest: string;
  readonly computer_ref: string;
  readonly generation: string | number;
  readonly tenant_ref: string;
  readonly conversation_ref: string;
  readonly runtime_class: CloudComputerRuntimeClass;
  readonly provider: string;
  readonly region: string;
  readonly status: ReservationStatus;
  readonly provider_lease_ref: string | null;
  readonly provider_operation_ref: string | null;
  readonly started_at: string | Date | null;
  readonly demand_json: unknown;
  readonly receipt_json: unknown;
}

interface ComputerRow {
  readonly computer_ref: string;
  readonly owner_ref: string;
  readonly tenant_ref: string;
  readonly conversation_ref: string;
  readonly generation: string | number;
  readonly runtime_class: CloudComputerRuntimeClass;
  readonly state: string;
}

export type CloudComputerCapacityStoreErrorCode =
  | "conflict"
  | "corrupt_store"
  | "invalid"
  | "logical_limit"
  | "not_found"
  | "permission_denied"
  | "stale_generation";

export class CloudComputerCapacityStoreError extends Error {
  constructor(
    readonly code: CloudComputerCapacityStoreErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerCapacityStoreError";
  }
}

export interface CloudComputerInventoryInput {
  readonly computerRef: string;
  readonly ownerRef: string;
  readonly tenantRef: string;
  readonly conversationRef: string;
  readonly workUnitRef: string;
  readonly kind: "interactive_retained" | "one_shot_batch";
  readonly runtimeClass: CloudComputerRuntimeClass;
  readonly generation: number;
  readonly version: number;
  readonly runtimeProfileRef: string;
  readonly authoritySnapshotDigest: string;
  readonly budgetSnapshotDigest: string;
  readonly capabilityRefs: ReadonlyArray<string>;
  readonly observedAt: string;
}

export interface StoredCapacityPolicy {
  readonly policyRef: string;
  readonly provider: string;
  readonly region: string;
  readonly runtimeClass: CloudComputerRuntimeClass;
  readonly policy: CloudComputerCapacityPolicy;
  readonly updatedAt: string;
}

export interface StoredCapacityObservation {
  readonly provider: string;
  readonly region: string;
  readonly observation: CloudComputerCapacityObservation;
  readonly expiresAt: string;
}

export interface AdmitCloudComputerInput {
  readonly reservationRef: string;
  readonly commandRef: string;
  readonly requestDigest: string;
  readonly ownerRef: string;
  readonly provider: string;
  readonly region: string;
  readonly generation: number;
  readonly protectedAuthorityRef?: string | undefined;
  readonly highFanOutAuthorityRef?: string | undefined;
  readonly demand: CloudComputerCapacityDemand;
  readonly nowMs: number;
}

export interface ProviderLeaseObservation {
  readonly providerLeaseRef: string;
  readonly providerOperationRef: string;
  readonly computerRef: string;
  readonly generation: number;
  readonly quarantined: boolean;
  readonly evidenceDigest: string;
}

const providerClaimToken = Symbol("cloud-computer-provider-start-claim");

export class CloudComputerProviderStartClaim {
  readonly #claim = providerClaimToken;

  constructor(
    token: typeof providerClaimToken,
    readonly reservationRef: string,
    readonly operationRef: string,
    readonly computerRef: string,
    readonly generation: number,
    readonly provider: string,
    readonly region: string,
  ) {
    if (token !== providerClaimToken) throw new Error("invalid provider start claim");
  }

  authentic(): boolean {
    return this.#claim === providerClaimToken;
  }
}

export type ProviderStartClaimResult =
  | Readonly<{ disposition: "proceed"; claim: CloudComputerProviderStartClaim }>
  | Readonly<{
      disposition:
        | "at_capacity"
        | "expired"
        | "jitter_pending"
        | "pending_uncertain"
        | "queued_stale";
    }>;

export interface CloudComputerProviderStarter {
  readonly start: (claim: CloudComputerProviderStartClaim) => Promise<
    Readonly<{
      providerLeaseRef: string;
      observedAt: string;
    }>
  >;
}

export const dispatchCloudComputerProviderStart = async (
  store: PostgresCloudComputerCapacityStore,
  result: ProviderStartClaimResult,
  provider: CloudComputerProviderStarter,
): Promise<Readonly<{ disposition: "not_dispatched" | "started"; providerLeaseRef?: string }>> => {
  if (result.disposition !== "proceed") return { disposition: "not_dispatched" };
  const started = await provider.start(result.claim);
  await store.bindProviderLease(result.claim, started.providerLeaseRef, started.observedAt);
  return { disposition: "started", providerLeaseRef: started.providerLeaseRef };
};

export type CapacityDriftKind =
  | "leaked"
  | "missing"
  | "double_claimed"
  | "generation_mismatch"
  | "operation_mismatch"
  | "quarantined";

export interface CapacityDrift {
  readonly driftRef: string;
  readonly kind: CapacityDriftKind;
  readonly providerLeaseRef: string;
  readonly reservationRef: string | null;
  readonly computerRef: string | null;
  readonly expectedGeneration: number | null;
  readonly observedGeneration: number | null;
  readonly evidenceDigest: string;
  readonly observedAt: string;
}

const assertRef = (value: string, field: string): void => {
  if (!/^[a-z][a-z0-9._-]{2,255}$/u.test(value)) {
    throw new CloudComputerCapacityStoreError("invalid", `${field} is invalid`);
  }
};

const assertTimestamp = (value: string, field: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new CloudComputerCapacityStoreError("invalid", `${field} is invalid`);
  }
};

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  const record = value as Readonly<Record<string, unknown>>;
  const keys = Object.keys(record);
  keys.sort();
  return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(",")}}`;
};

const digestJson = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;

const json = <A>(value: unknown, field: string): A => {
  try {
    return (typeof value === "string" ? JSON.parse(value) : value) as A;
  } catch {
    throw new CloudComputerCapacityStoreError("corrupt_store", `${field} is invalid JSON`);
  }
};

const zeroResources = (): CapacityResources => ({
  concurrency: 0,
  cpuMillicores: 0,
  memoryMiB: 0,
  scratchMiB: 0,
  durationMs: 0,
  startsPerMinute: 0,
  costMicros: 0,
});

const addResources = (left: CapacityResources, right: CapacityResources): CapacityResources => ({
  concurrency: left.concurrency + right.concurrency,
  cpuMillicores: left.cpuMillicores + right.cpuMillicores,
  memoryMiB: left.memoryMiB + right.memoryMiB,
  scratchMiB: left.scratchMiB + right.scratchMiB,
  durationMs: left.durationMs + right.durationMs,
  startsPerMinute: left.startsPerMinute + right.startsPerMinute,
  costMicros: left.costMicros + right.costMicros,
});

const reservationRows = async (tx: SyncTransactionSql): Promise<ReadonlyArray<ReservationRow>> =>
  tx`
    SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
           tenant_ref, conversation_ref, runtime_class, provider, region, status,
           provider_lease_ref, provider_operation_ref, started_at, demand_json, receipt_json
    FROM khala_sync_cloud_capacity_reservations
    WHERE status IN ('reserved', 'provisioning', 'provisioning_uncertain',
                     'starting', 'active', 'releasing', 'release_uncertain',
                     'quarantined')
    FOR UPDATE
  `;

const recentStartRows = async (
  tx: SyncTransactionSql,
  nowMs: number,
): Promise<ReadonlyArray<ReservationRow>> =>
  tx`
    SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
           tenant_ref, conversation_ref, runtime_class, provider, region, status,
           provider_lease_ref, provider_operation_ref, started_at, demand_json, receipt_json
    FROM khala_sync_cloud_capacity_reservations
    WHERE started_at > ${new Date(nowMs - 60_000).toISOString()}
      AND started_at <= ${new Date(nowMs).toISOString()}
    FOR UPDATE
  `;

const usageFrom = (
  rows: ReadonlyArray<ReservationRow>,
  providerRows: ReadonlyArray<ReservationRow>,
  regionRows: ReadonlyArray<ReservationRow>,
  rateRows: ReadonlyArray<ReservationRow>,
  providerRef: string,
  regionRef: string,
): CloudComputerCapacityUsage => {
  let global = zeroResources();
  let provider = zeroResources();
  let region = zeroResources();
  const owners: Record<string, CapacityResources> = {};
  const tenants: Record<string, CapacityResources> = {};
  const conversations: Record<string, CapacityResources> = {};
  for (const row of rows) {
    const demand = json<CloudComputerCapacityDemand>(row.demand_json, "stored demand");
    validateCloudComputerCapacityDemand(demand);
    const counted = { ...demand.resources, startsPerMinute: 0 };
    global = addResources(global, counted);
    owners[demand.ownerRef] = addResources(owners[demand.ownerRef] ?? zeroResources(), counted);
    tenants[row.tenant_ref] = addResources(tenants[row.tenant_ref] ?? zeroResources(), counted);
    conversations[row.conversation_ref] = addResources(
      conversations[row.conversation_ref] ?? zeroResources(),
      counted,
    );
  }
  for (const row of providerRows) {
    const demand = json<CloudComputerCapacityDemand>(row.demand_json, "stored demand");
    provider = addResources(provider, { ...demand.resources, startsPerMinute: 0 });
  }
  for (const row of regionRows) {
    const demand = json<CloudComputerCapacityDemand>(row.demand_json, "stored demand");
    region = addResources(region, { ...demand.resources, startsPerMinute: 0 });
  }
  for (const row of rateRows) {
    const demand = json<CloudComputerCapacityDemand>(row.demand_json, "stored demand");
    const started = { ...zeroResources(), startsPerMinute: demand.resources.startsPerMinute };
    global = addResources(global, started);
    owners[demand.ownerRef] = addResources(owners[demand.ownerRef] ?? zeroResources(), started);
    tenants[row.tenant_ref] = addResources(tenants[row.tenant_ref] ?? zeroResources(), started);
    conversations[row.conversation_ref] = addResources(
      conversations[row.conversation_ref] ?? zeroResources(),
      started,
    );
    if (row.provider === providerRef) provider = addResources(provider, started);
    if (row.provider === providerRef && row.region === regionRef) {
      region = addResources(region, started);
    }
  }
  return { provider, region, global, owners, tenants, conversations };
};

const statusForReceipt = (receipt: CloudComputerCapacityReceipt): ReservationStatus => {
  if (receipt.status === "admitted") return "reserved";
  if (receipt.status === "queued") return "queued";
  return "refused";
};

const rowReceipt = (row: ReservationRow): CloudComputerCapacityReceipt =>
  json<CloudComputerCapacityReceipt>(row.receipt_json, "stored receipt");

const selectPolicy = async (
  tx: SyncTransactionSql,
  provider: string,
  region: string,
  runtimeClass: CloudComputerRuntimeClass,
): Promise<
  Readonly<{
    policy: CloudComputerCapacityPolicy;
    policyRef: string;
    policyDigest: string;
  }>
> => {
  const rows: ReadonlyArray<{
    readonly policy_ref: string;
    readonly policy_digest: string;
    readonly policy_json: unknown;
  }> = await tx`
    SELECT policy_ref, policy_digest, policy_json
    FROM khala_sync_cloud_capacity_policies
    WHERE provider = ${provider} AND region = ${region} AND runtime_class = ${runtimeClass}
    ORDER BY updated_at DESC, policy_ref DESC
    LIMIT 1
  `;
  const row = rows[0];
  if (row === undefined) {
    throw new CloudComputerCapacityStoreError("not_found", "capacity policy is missing");
  }
  const policy = json<CloudComputerCapacityPolicy>(row.policy_json, "capacity policy");
  validateCloudComputerCapacityPolicy(policy);
  if (digestJson(policy) !== row.policy_digest) {
    throw new CloudComputerCapacityStoreError("corrupt_store", "capacity policy digest differs");
  }
  return { policy, policyRef: row.policy_ref, policyDigest: row.policy_digest };
};

const selectObservation = async (
  tx: SyncTransactionSql,
  provider: string,
  region: string,
  nowMs: number,
  quotaFreshnessMs: number,
): Promise<CloudComputerCapacityObservation> => {
  const rows: ReadonlyArray<{
    readonly observation_json: unknown;
    readonly expires_at: string | Date;
  }> = await tx`
    SELECT observation_json, expires_at
    FROM khala_sync_cloud_capacity_observations
    WHERE provider = ${provider} AND region = ${region}
    ORDER BY observed_at DESC
    LIMIT 1
  `;
  const row = rows[0];
  if (row === undefined) {
    throw new CloudComputerCapacityStoreError("not_found", "capacity observation is missing");
  }
  const observation = json<CloudComputerCapacityObservation>(
    row.observation_json,
    "capacity observation",
  );
  validateCloudComputerCapacityObservation(observation);
  if (Date.parse(String(row.expires_at)) <= nowMs) {
    return {
      ...observation,
      observedAt: new Date(nowMs - quotaFreshnessMs - 1).toISOString(),
    };
  }
  return observation;
};

export class PostgresCloudComputerCapacityStore {
  constructor(
    private readonly sql: SyncSql,
    private readonly options: Readonly<{
      authorizeProtected?:
        | ((
            input: Readonly<{
              authorityRef: string;
              ownerRef: string;
              tenantRef: string;
              conversationRef: string;
              priority: Exclude<CloudComputerCapacityDemand["priority"], "normal">;
            }>,
          ) => Promise<boolean>)
        | undefined;
      authorizeHighFanOut?:
        | ((
            input: Readonly<{
              authorityRef: string;
              ownerRef: string;
              tenantRef: string;
              conversationRef: string;
              computerRef: string;
            }>,
          ) => Promise<boolean>)
        | undefined;
    }> = {},
  ) {}

  private async serializable<A>(fn: (tx: SyncTransactionSql) => Promise<A>): Promise<A> {
    for (let attempt = 0; attempt < 32; attempt += 1) {
      try {
        // eslint-disable-next-line no-await-in-loop -- each retry needs the prior SQLSTATE.
        return await this.sql.begin("isolation level serializable", fn);
      } catch (error) {
        const code =
          typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
        if (code !== "40001" || attempt === 31) throw error;
      }
    }
    throw new CloudComputerCapacityStoreError("corrupt_store", "serialization retry exhausted");
  }

  async putPolicy(input: StoredCapacityPolicy): Promise<void> {
    assertRef(input.policyRef, "policy ref");
    assertRef(input.provider, "provider");
    assertRef(input.region, "region");
    assertTimestamp(input.updatedAt, "updated at");
    validateCloudComputerCapacityPolicy(input.policy);
    const policyDigest = digestJson(input.policy);
    const rows: ReadonlyArray<{ readonly policy_ref: string }> = await this.sql`
      INSERT INTO khala_sync_cloud_capacity_policies
        (policy_ref, policy_digest, provider, region, runtime_class, policy_json, updated_at)
      VALUES
        (${input.policyRef}, ${policyDigest}, ${input.provider}, ${input.region}, ${input.runtimeClass},
         ${input.policy}::jsonb, ${input.updatedAt})
      ON CONFLICT (policy_ref) DO UPDATE SET policy_ref = EXCLUDED.policy_ref
      WHERE khala_sync_cloud_capacity_policies.policy_digest = EXCLUDED.policy_digest
      RETURNING policy_ref
    `;
    if (rows.length !== 1) {
      throw new CloudComputerCapacityStoreError("conflict", "policy ref bytes conflict");
    }
  }

  async recordObservation(input: StoredCapacityObservation): Promise<void> {
    assertRef(input.provider, "provider");
    assertRef(input.region, "region");
    assertTimestamp(input.expiresAt, "expires at");
    validateCloudComputerCapacityObservation(input.observation);
    const observedAt = input.observation.observedAt;
    if (Date.parse(input.expiresAt) <= Date.parse(observedAt)) {
      throw new CloudComputerCapacityStoreError("invalid", "observation expiry is invalid");
    }
    const observationDigest = digestJson(input.observation);
    const rows: ReadonlyArray<{ readonly observation_ref: string }> = await this.sql`
      INSERT INTO khala_sync_cloud_capacity_observations
        (observation_ref, observation_digest, provider, region, quota_units, allocatable_units,
         drained_units, quota_resources, allocatable_resources,
         drain_adjusted_resources, budget_resources, observed_at, expires_at,
         observation_json)
      VALUES
        (${input.observation.observationRef}, ${observationDigest}, ${input.provider}, ${input.region},
         ${input.observation.providerQuota.concurrency},
         ${input.observation.allocatableCapacity.concurrency},
         ${Math.max(0, input.observation.allocatableCapacity.concurrency - input.observation.drainAdjustedCapacity.concurrency)},
         ${input.observation.providerQuota}::jsonb,
         ${input.observation.allocatableCapacity}::jsonb,
         ${input.observation.drainAdjustedCapacity}::jsonb,
         ${input.observation.budgetCapacity}::jsonb,
         ${observedAt}, ${input.expiresAt}, ${input.observation}::jsonb)
      ON CONFLICT (observation_ref) DO UPDATE SET observation_ref = EXCLUDED.observation_ref
      WHERE khala_sync_cloud_capacity_observations.observation_digest = EXCLUDED.observation_digest
        AND khala_sync_cloud_capacity_observations.provider = EXCLUDED.provider
        AND khala_sync_cloud_capacity_observations.region = EXCLUDED.region
      RETURNING observation_ref
    `;
    if (rows.length !== 1) {
      throw new CloudComputerCapacityStoreError("conflict", "observation ref bytes conflict");
    }
  }

  async createLogicalComputers(records: ReadonlyArray<CloudComputerInventoryInput>): Promise<void> {
    if (records.length === 0) return;
    const first = records[0]!;
    for (const record of records) {
      for (const [field, value] of Object.entries({
        computerRef: record.computerRef,
        ownerRef: record.ownerRef,
        tenantRef: record.tenantRef,
        conversationRef: record.conversationRef,
        workUnitRef: record.workUnitRef,
        runtimeProfileRef: record.runtimeProfileRef,
      })) {
        assertRef(value, field);
      }
      assertTimestamp(record.observedAt, "observed at");
      if (
        !SHA256.test(record.authoritySnapshotDigest) ||
        !SHA256.test(record.budgetSnapshotDigest) ||
        record.capabilityRefs.length === 0 ||
        record.capabilityRefs.some((ref) => {
          try {
            assertRef(ref, "capability ref");
            return false;
          } catch {
            return true;
          }
        })
      ) {
        throw new CloudComputerCapacityStoreError("invalid", "computer policy scope is invalid");
      }
      if (
        record.ownerRef !== first.ownerRef ||
        record.tenantRef !== first.tenantRef ||
        record.conversationRef !== first.conversationRef ||
        !Number.isSafeInteger(record.generation) ||
        record.generation <= 0 ||
        !Number.isSafeInteger(record.version) ||
        record.version <= 0
      ) {
        throw new CloudComputerCapacityStoreError("invalid", "inventory batch scope is invalid");
      }
    }
    const uniqueRefs = new Set(records.map((record) => record.computerRef));
    if (uniqueRefs.size !== records.length) {
      throw new CloudComputerCapacityStoreError(
        "invalid",
        "inventory batch repeats a computer ref",
      );
    }

    await this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${first.tenantRef}|${first.conversationRef}`}, 3104))`;
      const existingRows: ReadonlyArray<ComputerRow> = await tx`
        SELECT computer_ref, owner_ref, tenant_ref, conversation_ref, generation,
               runtime_class, state
        FROM khala_sync_cloud_computers
        WHERE computer_ref = ANY(${[...uniqueRefs]})
        FOR UPDATE
      `;
      const existingRefs = new Set(existingRows.map((row) => row.computer_ref));
      for (const row of existingRows) {
        const expected = records.find((record) => record.computerRef === row.computer_ref)!;
        if (
          row.owner_ref !== expected.ownerRef ||
          row.tenant_ref !== expected.tenantRef ||
          row.conversation_ref !== expected.conversationRef ||
          Number(row.generation) !== expected.generation ||
          row.runtime_class !== expected.runtimeClass
        ) {
          throw new CloudComputerCapacityStoreError("conflict", "computer ref scope conflicts");
        }
      }
      const counts: ReadonlyArray<{ readonly count: string | number }> = await tx`
        SELECT COUNT(*) AS count
        FROM khala_sync_cloud_computers
        WHERE tenant_ref = ${first.tenantRef}
          AND conversation_ref = ${first.conversationRef}
          AND state <> 'destroyed'
      `;
      const additions = records.length - existingRefs.size;
      if (Number(counts[0]?.count ?? 0) + additions > 30) {
        throw new CloudComputerCapacityStoreError(
          "logical_limit",
          "a conversation cannot own more than 30 logical computers",
        );
      }
      await Promise.all(
        records
          .filter((record) => !existingRefs.has(record.computerRef))
          .map(
            (record) => tx`
          INSERT INTO khala_sync_cloud_computers
            (computer_ref, owner_ref, tenant_ref, conversation_ref, work_unit_ref,
             kind, runtime_class, generation, version, runtime_profile_ref,
             authority_snapshot_digest, budget_snapshot_digest, capability_refs,
             state, created_at, updated_at)
          VALUES
            (${record.computerRef}, ${record.ownerRef}, ${record.tenantRef},
             ${record.conversationRef}, ${record.workUnitRef}, ${record.kind},
             ${record.runtimeClass}, ${record.generation}, ${record.version},
             ${record.runtimeProfileRef}, ${record.authoritySnapshotDigest},
             ${record.budgetSnapshotDigest}, to_jsonb(${record.capabilityRefs}::text[]),
             'cold', ${record.observedAt}, ${record.observedAt})
        `,
          ),
      );
    });
  }

  async admit(input: AdmitCloudComputerInput): Promise<CloudComputerCapacityReceipt> {
    for (const [field, value] of Object.entries({
      reservationRef: input.reservationRef,
      commandRef: input.commandRef,
      ownerRef: input.ownerRef,
      provider: input.provider,
      region: input.region,
    })) {
      assertRef(value, field);
    }
    if (!SHA256.test(input.requestDigest) || !Number.isFinite(input.nowMs)) {
      throw new CloudComputerCapacityStoreError("invalid", "admission identity is invalid");
    }
    validateCloudComputerCapacityDemand(input.demand);
    if (input.demand.ownerRef !== input.ownerRef) {
      throw new CloudComputerCapacityStoreError("permission_denied", "demand owner differs");
    }
    let highFanOutBudgeted = false;
    if (input.demand.highFanOutBudgeted) {
      const authorityRef = input.highFanOutAuthorityRef;
      if (authorityRef === undefined) {
        throw new CloudComputerCapacityStoreError(
          "permission_denied",
          "high-fan-out work lacks budget authority",
        );
      }
      assertRef(authorityRef, "high-fan-out authority ref");
      highFanOutBudgeted =
        (await this.options.authorizeHighFanOut?.({
          authorityRef,
          ownerRef: input.ownerRef,
          tenantRef: input.demand.tenantRef,
          conversationRef: input.demand.conversationRef,
          computerRef: input.demand.computerRef,
        })) === true;
      if (!highFanOutBudgeted) {
        throw new CloudComputerCapacityStoreError(
          "permission_denied",
          "high-fan-out budget authority is invalid",
        );
      }
    } else if (input.highFanOutAuthorityRef !== undefined) {
      throw new CloudComputerCapacityStoreError(
        "invalid",
        "high-fan-out authority requires a high-fan-out demand",
      );
    }
    if (input.demand.priority === "normal") {
      if (input.protectedAuthorityRef !== undefined) {
        throw new CloudComputerCapacityStoreError(
          "invalid",
          "normal work cannot claim protected authority",
        );
      }
    } else {
      const authorityRef = input.protectedAuthorityRef;
      if (authorityRef === undefined) {
        throw new CloudComputerCapacityStoreError(
          "permission_denied",
          "protected work lacks authority",
        );
      }
      assertRef(authorityRef, "protected authority ref");
      const authorized = await this.options.authorizeProtected?.({
        authorityRef,
        ownerRef: input.ownerRef,
        tenantRef: input.demand.tenantRef,
        conversationRef: input.demand.conversationRef,
        priority: input.demand.priority,
      });
      if (authorized !== true) {
        throw new CloudComputerCapacityStoreError(
          "permission_denied",
          "protected work is not authorized",
        );
      }
    }

    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${GLOBAL_CAPACITY_LOCK}, 3104))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${input.provider}|${input.region}`}, 3104))`;
      const replayRows: ReadonlyArray<ReservationRow> = await tx`
        SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
               tenant_ref, conversation_ref, runtime_class, status,
               provider_lease_ref, provider_operation_ref, demand_json, receipt_json
        FROM khala_sync_cloud_capacity_reservations
        WHERE command_ref = ${input.commandRef}
        FOR UPDATE
      `;
      const replay = replayRows[0];
      if (replay !== undefined) {
        if (
          replay.request_digest !== input.requestDigest ||
          replay.computer_ref !== input.demand.computerRef ||
          replay.tenant_ref !== input.demand.tenantRef ||
          replay.conversation_ref !== input.demand.conversationRef
        ) {
          throw new CloudComputerCapacityStoreError("conflict", "command retry bytes conflict");
        }
        return rowReceipt(replay);
      }

      const computers: ReadonlyArray<ComputerRow> = await tx`
        SELECT computer_ref, owner_ref, tenant_ref, conversation_ref, generation,
               runtime_class, state
        FROM khala_sync_cloud_computers
        WHERE computer_ref = ${input.demand.computerRef}
        FOR UPDATE
      `;
      const computer = computers[0];
      if (computer === undefined) {
        throw new CloudComputerCapacityStoreError("not_found", "logical computer is missing");
      }
      if (
        computer.owner_ref !== input.ownerRef ||
        computer.tenant_ref !== input.demand.tenantRef ||
        computer.conversation_ref !== input.demand.conversationRef ||
        computer.runtime_class !== input.demand.runtimeClass
      ) {
        throw new CloudComputerCapacityStoreError("permission_denied", "computer scope differs");
      }
      if (Number(computer.generation) !== input.generation) {
        throw new CloudComputerCapacityStoreError(
          "stale_generation",
          "computer generation differs",
        );
      }

      const selectedPolicy = await selectPolicy(
        tx,
        input.provider,
        input.region,
        input.demand.runtimeClass,
      );
      const observation = await selectObservation(
        tx,
        input.provider,
        input.region,
        input.nowMs,
        selectedPolicy.policy.quotaFreshnessMs,
      );
      const live = await reservationRows(tx);
      const recentStarts = await recentStartRows(tx, input.nowMs);
      const providerLive = live.filter((row) => row.provider === input.provider);
      const regionLive = providerLive.filter((row) => row.region === input.region);
      const usage = usageFrom(
        live,
        providerLive,
        regionLive,
        recentStarts,
        input.provider,
        input.region,
      );
      const inventoryCountRows: ReadonlyArray<{ readonly count: string | number }> = await tx`
        SELECT COUNT(*) AS count FROM khala_sync_cloud_computers
        WHERE tenant_ref = ${input.demand.tenantRef}
          AND conversation_ref = ${input.demand.conversationRef}
          AND state <> 'destroyed'
      `;
      const classCountRows: ReadonlyArray<{
        readonly runtime_class: CloudComputerRuntimeClass;
        readonly count: string | number;
      }> = await tx`
        SELECT runtime_class, COUNT(*) AS count
        FROM khala_sync_cloud_capacity_reservations
        WHERE tenant_ref = ${input.demand.tenantRef}
          AND conversation_ref = ${input.demand.conversationRef}
          AND status IN ('reserved', 'provisioning', 'provisioning_uncertain',
                         'starting', 'active', 'releasing', 'release_uncertain')
        GROUP BY runtime_class
      `;
      const existingActiveByRuntimeClass = Object.fromEntries([
        ["standard", 0],
        ["strong", 0],
        ["batch", 0],
        ...classCountRows.map((row) => [row.runtime_class, Number(row.count)] as const),
      ]) as Record<CloudComputerRuntimeClass, number>;
      const trustedDemand: CloudComputerCapacityDemand = {
        ...input.demand,
        ownerRef: input.ownerRef,
        createsLogicalComputer: false,
        highFanOutBudgeted,
        existingLogicalComputers: Number(inventoryCountRows[0]?.count ?? 0),
        existingActiveByRuntimeClass,
      };
      const receipt = evaluateCloudComputerAdmission({
        policy: selectedPolicy.policy,
        observation,
        usage,
        demand: trustedDemand,
        nowMs: input.nowMs,
      });
      const status = statusForReceipt(receipt);
      const releasedAt = status === "refused" ? new Date(input.nowMs).toISOString() : null;
      const fallbackDeadline = new Date(
        input.nowMs + selectedPolicy.policy.quotaFreshnessMs,
      ).toISOString();
      const deadlineAt =
        receipt.startDeadlineAt !== null && Date.parse(receipt.startDeadlineAt) > input.nowMs
          ? receipt.startDeadlineAt
          : fallbackDeadline;
      const notBeforeAt = new Date(
        Math.min(
          Date.parse(deadlineAt) - 1,
          input.nowMs +
            deterministicStartJitterMs(
              input.demand.demandRef,
              selectedPolicy.policy.jitterWindowMs,
            ),
        ),
      ).toISOString();
      await tx`
        INSERT INTO khala_sync_cloud_capacity_fair_flows
          (provider, region, tenant_ref, conversation_ref)
        VALUES
          (${input.provider}, ${input.region}, ${input.demand.tenantRef},
           ${input.demand.conversationRef})
        ON CONFLICT DO NOTHING
      `;
      const flowRows: ReadonlyArray<{
        readonly conversation_ref: string;
        readonly tenant_virtual_finish: string | number;
        readonly conversation_virtual_finish: string | number;
      }> = await tx`
        SELECT conversation_ref, tenant_virtual_finish, conversation_virtual_finish
        FROM khala_sync_cloud_capacity_fair_flows
        WHERE provider = ${input.provider} AND region = ${input.region}
          AND tenant_ref = ${input.demand.tenantRef}
        FOR UPDATE
      `;
      const conversationFlow = flowRows.find(
        (flow) => flow.conversation_ref === input.demand.conversationRef,
      )!;
      const tenantVirtualFinish = Math.max(
        ...flowRows.map((flow) => Number(flow.tenant_virtual_finish)),
      );
      const virtualFinish = Math.max(
        tenantVirtualFinish + 1 / input.demand.tenantWeight,
        Number(conversationFlow.conversation_virtual_finish) + 1 / input.demand.conversationWeight,
      );
      await tx`
        UPDATE khala_sync_cloud_capacity_fair_flows
        SET tenant_virtual_finish = ${virtualFinish}, revision = revision + 1
        WHERE provider = ${input.provider} AND region = ${input.region}
          AND tenant_ref = ${input.demand.tenantRef}
      `;
      await tx`
        UPDATE khala_sync_cloud_capacity_fair_flows
        SET conversation_virtual_finish = ${virtualFinish}, revision = revision + 1
        WHERE provider = ${input.provider} AND region = ${input.region}
          AND tenant_ref = ${input.demand.tenantRef}
          AND conversation_ref = ${input.demand.conversationRef}
      `;
      const priorityRank = { cleanup: 0, recovery: 1, replacement: 2, normal: 3 }[
        input.demand.priority
      ];
      await tx`
        INSERT INTO khala_sync_cloud_capacity_reservations
          (reservation_ref, command_ref, request_digest, computer_ref, generation,
           owner_ref, tenant_ref, conversation_ref, runtime_class, priority,
           provider, region, status, reason, cpu_millis, memory_mib, scratch_mib,
           duration_seconds, cost_micros, queue_weight, priority_rank, virtual_finish,
           observation_ref, policy_ref, policy_digest, protected_authority_ref,
           not_before_at, deadline_at, created_at, updated_at, released_at,
           demand_json, receipt_json)
        VALUES
          (${input.reservationRef}, ${input.commandRef}, ${input.requestDigest},
           ${input.demand.computerRef}, ${input.generation}, ${input.ownerRef},
           ${input.demand.tenantRef}, ${input.demand.conversationRef},
           ${input.demand.runtimeClass}, ${input.demand.priority}, ${input.provider},
           ${input.region}, ${status}, ${receipt.reason},
           ${input.demand.resources.cpuMillicores}, ${input.demand.resources.memoryMiB},
           ${input.demand.resources.scratchMiB},
           ${Math.max(1, Math.ceil(input.demand.resources.durationMs / 1_000))},
           ${input.demand.resources.costMicros},
           ${Math.max(1, Math.round(input.demand.tenantWeight * input.demand.conversationWeight))},
           ${priorityRank}, ${virtualFinish}, ${receipt.observationRef},
           ${selectedPolicy.policyRef}, ${selectedPolicy.policyDigest},
           ${input.protectedAuthorityRef ?? null}, ${notBeforeAt}, ${deadlineAt},
           ${new Date(input.nowMs).toISOString()}, ${new Date(input.nowMs).toISOString()},
           ${releasedAt}, ${trustedDemand}::jsonb, ${receipt}::jsonb)
      `;
      if (status === "queued") {
        await tx`
          UPDATE khala_sync_cloud_computers SET state = 'queued', updated_at = ${new Date(input.nowMs).toISOString()}
          WHERE computer_ref = ${input.demand.computerRef} AND generation = ${input.generation}
        `;
      }
      return receipt;
    });
  }

  async claimQueued(
    provider: string,
    region: string,
    runtimeClass: CloudComputerRuntimeClass,
    nowMs: number,
  ): Promise<ReadonlyArray<CloudComputerCapacityReceipt>> {
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${GLOBAL_CAPACITY_LOCK}, 3104))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${provider}|${region}`}, 3104))`;
      const selectedPolicy = await selectPolicy(tx, provider, region, runtimeClass);
      const observation = await selectObservation(
        tx,
        provider,
        region,
        nowMs,
        selectedPolicy.policy.quotaFreshnessMs,
      );
      const live = await reservationRows(tx);
      const recentStarts = await recentStartRows(tx, nowMs);
      const providerLive = live.filter((row) => row.provider === provider);
      const regionLive = providerLive.filter((row) => row.region === region);
      const queued: ReadonlyArray<ReservationRow> = await tx`
        SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
               tenant_ref, conversation_ref, runtime_class, status,
               provider_lease_ref, provider_operation_ref, demand_json, receipt_json
        FROM khala_sync_cloud_capacity_reservations
        WHERE provider = ${provider} AND region = ${region}
          AND runtime_class = ${runtimeClass} AND status = 'queued'
          AND deadline_at > ${new Date(nowMs).toISOString()}
        ORDER BY priority_rank, virtual_finish, queue_sequence
        FOR UPDATE
      `;
      const inventoryRows: ReadonlyArray<{
        readonly conversation_ref: string;
        readonly count: string | number;
      }> = await tx`
        SELECT conversation_ref, COUNT(*) AS count
        FROM khala_sync_cloud_computers
        WHERE state <> 'destroyed'
        GROUP BY conversation_ref
      `;
      const inventoryByConversation = new Map(
        inventoryRows.map((row) => [row.conversation_ref, Number(row.count)]),
      );
      const activeByConversation = new Map<string, Record<CloudComputerRuntimeClass, number>>();
      for (const row of live) {
        const counts = activeByConversation.get(row.conversation_ref) ?? {
          standard: 0,
          strong: 0,
          batch: 0,
        };
        counts[row.runtime_class] += 1;
        activeByConversation.set(row.conversation_ref, counts);
      }
      // eslint-disable-next-line oxc/no-map-spread -- stored demand snapshots stay immutable.
      const queuedDemands = queued.map((row) => {
        const stored = json<CloudComputerCapacityDemand>(row.demand_json, "queued demand");
        return {
          ...stored,
          existingLogicalComputers: inventoryByConversation.get(stored.conversationRef) ?? 0,
          existingActiveByRuntimeClass: activeByConversation.get(stored.conversationRef) ?? {
            standard: 0,
            strong: 0,
            batch: 0,
          },
        };
      });
      const schedule = scheduleCloudComputerCapacity({
        policy: selectedPolicy.policy,
        observation,
        usage: usageFrom(live, providerLive, regionLive, recentStarts, provider, region),
        demands: queuedDemands,
        nowMs,
        preserveOrder: true,
      });
      await Promise.all(
        schedule.admitted.map((receipt) => {
          const deadlineAt =
            receipt.startDeadlineAt !== null && Date.parse(receipt.startDeadlineAt) > nowMs
              ? receipt.startDeadlineAt
              : new Date(nowMs + selectedPolicy.policy.quotaFreshnessMs).toISOString();
          const notBeforeAt = new Date(
            Math.min(
              Date.parse(deadlineAt) - 1,
              nowMs +
                deterministicStartJitterMs(receipt.demandRef, selectedPolicy.policy.jitterWindowMs),
            ),
          ).toISOString();
          return tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'reserved', reason = ${receipt.reason},
              receipt_json = ${receipt}::jsonb,
              not_before_at = ${notBeforeAt}, deadline_at = ${deadlineAt},
              updated_at = ${new Date(nowMs).toISOString()}
          WHERE provider = ${provider} AND region = ${region}
            AND status = 'queued' AND demand_json->>'demandRef' = ${receipt.demandRef}
        `;
        }),
      );
      return schedule.admitted;
    });
  }

  async bindProviderLease(
    claim: CloudComputerProviderStartClaim,
    providerLeaseRef: string,
    observedAt: string,
  ): Promise<void> {
    if (!claim.authentic()) {
      throw new CloudComputerCapacityStoreError("permission_denied", "provider claim is invalid");
    }
    assertRef(providerLeaseRef, "provider lease ref");
    await this.serializable(async (tx) => {
      const rows: ReadonlyArray<ReservationRow> = await tx`
        SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
               tenant_ref, conversation_ref, runtime_class, status,
               provider_lease_ref, provider_operation_ref, demand_json, receipt_json
        FROM khala_sync_cloud_capacity_reservations
        WHERE reservation_ref = ${claim.reservationRef}
        FOR UPDATE
      `;
      const row = rows[0];
      if (row === undefined)
        throw new CloudComputerCapacityStoreError("not_found", "reservation missing");
      if (row.computer_ref !== claim.computerRef || Number(row.generation) !== claim.generation) {
        throw new CloudComputerCapacityStoreError("stale_generation", "lease generation differs");
      }
      if (row.status !== "provisioning" && row.status !== "starting") {
        throw new CloudComputerCapacityStoreError("conflict", "reservation cannot start");
      }
      if (row.provider_lease_ref !== null && row.provider_lease_ref !== providerLeaseRef) {
        throw new CloudComputerCapacityStoreError("conflict", "reservation owns another lease");
      }
      if (row.provider_operation_ref !== claim.operationRef) {
        throw new CloudComputerCapacityStoreError("conflict", "provider operation differs");
      }
      await tx`
        UPDATE khala_sync_cloud_capacity_reservations
        SET status = 'starting', provider_lease_ref = ${providerLeaseRef},
            updated_at = ${observedAt}
        WHERE reservation_ref = ${claim.reservationRef}
          AND provider_operation_ref = ${claim.operationRef}
      `;
    });
  }

  async claimProviderStart(
    reservationRef: string,
    nowMs: number,
  ): Promise<ProviderStartClaimResult> {
    assertRef(reservationRef, "reservation ref");
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${GLOBAL_CAPACITY_LOCK}, 3104))`;
      const rows: ReadonlyArray<
        ReservationRow & {
          readonly provider: string;
          readonly region: string;
          readonly priority: CloudComputerCapacityDemand["priority"];
          readonly not_before_at: string | Date;
          readonly deadline_at: string | Date;
          readonly provider_operation_ref: string | null;
          readonly operation_revision: string | number;
        }
      > = await tx`
        SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
               tenant_ref, conversation_ref, runtime_class, status,
               provider_lease_ref, demand_json, receipt_json, provider, region,
               priority, not_before_at, deadline_at, provider_operation_ref,
               operation_revision
        FROM khala_sync_cloud_capacity_reservations
        WHERE reservation_ref = ${reservationRef}
        FOR UPDATE
      `;
      const row = rows[0];
      if (row === undefined)
        throw new CloudComputerCapacityStoreError("not_found", "reservation missing");
      if (row.status === "provisioning" || row.status === "provisioning_uncertain") {
        return { disposition: "pending_uncertain" };
      }
      if (row.status !== "reserved") {
        throw new CloudComputerCapacityStoreError("conflict", "reservation cannot be claimed");
      }
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${row.provider}|${row.region}`}, 3104))`;
      if (Date.parse(String(row.deadline_at)) <= nowMs) {
        await tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'expired', reason = 'start_deadline_expired',
              released_at = ${new Date(nowMs).toISOString()},
              revision = revision + 1, updated_at = ${new Date(nowMs).toISOString()}
          WHERE reservation_ref = ${row.reservation_ref} AND status = 'reserved'
        `;
        return { disposition: "expired" };
      }
      if (Date.parse(String(row.not_before_at)) > nowMs) {
        return { disposition: "jitter_pending" };
      }
      const selectedPolicy = await selectPolicy(tx, row.provider, row.region, row.runtime_class);
      const currentObservation = await selectObservation(
        tx,
        row.provider,
        row.region,
        nowMs,
        selectedPolicy.policy.quotaFreshnessMs,
      );
      const observationStale =
        nowMs - Date.parse(currentObservation.observedAt) > selectedPolicy.policy.quotaFreshnessMs;
      if (row.priority === "normal" && observationStale) {
        const receipt = rowReceipt(row);
        const queuedReceipt: CloudComputerCapacityReceipt = {
          ...receipt,
          status: "queued",
          reason: "provider_quota_stale",
          decidedAt: new Date(nowMs).toISOString(),
          startDeadlineAt: null,
          retryAfterMs: selectedPolicy.policy.quotaFreshnessMs,
        };
        await tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'queued', reason = 'provider_quota_stale',
              receipt_json = ${queuedReceipt}::jsonb,
              deadline_at = ${new Date(nowMs + selectedPolicy.policy.quotaFreshnessMs).toISOString()},
              revision = revision + 1, updated_at = ${new Date(nowMs).toISOString()}
          WHERE reservation_ref = ${row.reservation_ref} AND status = 'reserved'
        `;
        await tx`
          UPDATE khala_sync_cloud_computers
          SET state = 'queued', version = version + 1,
              updated_at = ${new Date(nowMs).toISOString()}
          WHERE computer_ref = ${row.computer_ref} AND generation = ${Number(row.generation)}
        `;
        return { disposition: "queued_stale" };
      }
      const currentDemand = json<CloudComputerCapacityDemand>(row.demand_json, "stored demand");
      const otherLive = (await reservationRows(tx)).filter(
        (candidate) => candidate.reservation_ref !== row.reservation_ref,
      );
      const recentStarts = await recentStartRows(tx, nowMs);
      const providerLive = otherLive.filter((candidate) => candidate.provider === row.provider);
      const regionLive = providerLive.filter((candidate) => candidate.region === row.region);
      const currentReceipt = evaluateCloudComputerAdmission({
        policy: selectedPolicy.policy,
        observation: currentObservation,
        usage: usageFrom(
          otherLive,
          providerLive,
          regionLive,
          recentStarts,
          row.provider,
          row.region,
        ),
        demand: currentDemand,
        nowMs,
      });
      if (currentReceipt.status !== "admitted") {
        await tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'queued', reason = ${currentReceipt.reason},
              receipt_json = ${currentReceipt}::jsonb,
              revision = revision + 1, updated_at = ${new Date(nowMs).toISOString()}
          WHERE reservation_ref = ${row.reservation_ref} AND status = 'reserved'
        `;
        await tx`
          UPDATE khala_sync_cloud_computers
          SET state = 'queued', version = version + 1,
              updated_at = ${new Date(nowMs).toISOString()}
          WHERE computer_ref = ${row.computer_ref} AND generation = ${Number(row.generation)}
        `;
        return { disposition: "at_capacity" };
      }
      const startCounts: ReadonlyArray<{ readonly count: string | number }> = await tx`
        SELECT COUNT(*) AS count
        FROM khala_sync_cloud_capacity_reservations
        WHERE provider = ${row.provider} AND region = ${row.region}
          AND status IN ('provisioning', 'provisioning_uncertain', 'starting')
      `;
      if (Number(startCounts[0]?.count ?? 0) >= selectedPolicy.policy.maxStartBatch) {
        return { disposition: "at_capacity" };
      }
      const revision = Number(row.operation_revision) + 1;
      const operationRef = `operation.${row.reservation_ref}.${revision}`;
      await tx`
        UPDATE khala_sync_cloud_capacity_reservations
        SET status = 'provisioning', provider_operation_ref = ${operationRef},
            operation_revision = ${revision}, started_at = ${new Date(nowMs).toISOString()},
            revision = revision + 1,
            updated_at = ${new Date(nowMs).toISOString()}
        WHERE reservation_ref = ${row.reservation_ref} AND status = 'reserved'
      `;
      return {
        disposition: "proceed",
        claim: new CloudComputerProviderStartClaim(
          providerClaimToken,
          row.reservation_ref,
          operationRef,
          row.computer_ref,
          Number(row.generation),
          row.provider,
          row.region,
        ),
      };
    });
  }

  async markProvisioningUncertain(nowMs: number): Promise<number> {
    const rows: ReadonlyArray<{ readonly reservation_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_capacity_reservations
      SET status = 'provisioning_uncertain', reason = 'start_deadline_expired',
          revision = revision + 1, updated_at = ${new Date(nowMs).toISOString()}
      WHERE status = 'provisioning' AND deadline_at <= ${new Date(nowMs).toISOString()}
      RETURNING reservation_ref
    `;
    return rows.length;
  }

  async expireUnstarted(nowMs: number): Promise<number> {
    const rows: ReadonlyArray<{ readonly reservation_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_capacity_reservations
      SET status = 'expired', reason = 'start_deadline_expired',
          released_at = ${new Date(nowMs).toISOString()}, updated_at = ${new Date(nowMs).toISOString()}
      WHERE status IN ('queued', 'reserved') AND provider_lease_ref IS NULL
        AND deadline_at <= ${new Date(nowMs).toISOString()}
      RETURNING reservation_ref
    `;
    return rows.length;
  }

  async reconcile(
    provider: string,
    region: string,
    observed: ReadonlyArray<ProviderLeaseObservation>,
    nowMs: number,
    inventoryEvidenceDigest: string,
  ): Promise<ReadonlyArray<CapacityDrift>> {
    assertRef(provider, "provider");
    assertRef(region, "region");
    if (!SHA256.test(inventoryEvidenceDigest)) {
      throw new CloudComputerCapacityStoreError("invalid", "inventory evidence digest is invalid");
    }
    const observedAt = new Date(nowMs).toISOString();
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${GLOBAL_CAPACITY_LOCK}, 3104))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${provider}|${region}`}, 3104))`;
      const live = (await reservationRows(tx)).filter(
        (row) => row.provider === provider && row.region === region,
      );
      const byLease = new Map(
        live
          .filter((row) => row.provider_lease_ref !== null)
          .map((row) => [row.provider_lease_ref!, row]),
      );
      const observedLeaseCounts = new Map<string, number>();
      for (const item of observed) {
        assertRef(item.providerLeaseRef, "provider lease ref");
        assertRef(item.providerOperationRef, "provider operation ref");
        assertRef(item.computerRef, "computer ref");
        if (!SHA256.test(item.evidenceDigest)) {
          throw new CloudComputerCapacityStoreError("invalid", "drift evidence digest is invalid");
        }
        observedLeaseCounts.set(
          item.providerLeaseRef,
          (observedLeaseCounts.get(item.providerLeaseRef) ?? 0) + 1,
        );
      }
      const drift: CapacityDrift[] = [];
      const repairs: Array<Promise<unknown>> = [];
      const append = (
        kind: CapacityDriftKind,
        item: ProviderLeaseObservation,
        reservation: ReservationRow | undefined,
      ): void => {
        drift.push({
          driftRef: `drift.${kind}.${item.providerLeaseRef}.${nowMs}`,
          kind,
          providerLeaseRef: item.providerLeaseRef,
          reservationRef: reservation?.reservation_ref ?? null,
          computerRef: item.computerRef,
          expectedGeneration: reservation === undefined ? null : Number(reservation.generation),
          observedGeneration: item.generation,
          evidenceDigest: item.evidenceDigest,
          observedAt,
        });
      };
      for (const item of observed) {
        const row = byLease.get(item.providerLeaseRef);
        const exactComputer = live.find(
          (candidate) =>
            candidate.computer_ref === item.computerRef &&
            Number(candidate.generation) === item.generation,
        );
        const matchedReservation = row ?? exactComputer;
        if ((observedLeaseCounts.get(item.providerLeaseRef) ?? 0) > 1)
          append("double_claimed", item, matchedReservation);
        else if (
          matchedReservation !== undefined &&
          matchedReservation.provider_operation_ref !== item.providerOperationRef
        )
          append("operation_mismatch", item, matchedReservation);
        else if (
          item.quarantined &&
          row !== undefined &&
          (row.computer_ref !== item.computerRef || Number(row.generation) !== item.generation)
        )
          append("generation_mismatch", item, row);
        else if (item.quarantined) {
          append("quarantined", item, row ?? exactComputer);
          if (exactComputer !== undefined) {
            repairs.push(tx`
              UPDATE khala_sync_cloud_capacity_reservations
              SET status = 'quarantined', provider_lease_ref = ${item.providerLeaseRef},
                  reason = 'provider_quarantined', revision = revision + 1,
                  updated_at = ${observedAt}
              WHERE reservation_ref = ${exactComputer.reservation_ref}
                AND generation = ${item.generation}
                AND status IN ('provisioning', 'provisioning_uncertain', 'starting', 'active')
            `);
            repairs.push(tx`
              UPDATE khala_sync_cloud_computers
              SET state = 'failed', active_lease_ref = NULL, version = version + 1,
                  updated_at = ${observedAt}
              WHERE computer_ref = ${item.computerRef} AND generation = ${item.generation}
            `);
          }
        } else if (row === undefined) {
          const sameComputer = live.find(
            (candidate) => candidate.computer_ref === item.computerRef,
          );
          if (
            exactComputer !== undefined &&
            ["provisioning", "provisioning_uncertain", "starting"].includes(exactComputer.status)
          ) {
            repairs.push(tx`
              UPDATE khala_sync_cloud_capacity_reservations
              SET status = 'active', provider_lease_ref = ${item.providerLeaseRef},
                  reason = 'provider_observation_settled', revision = revision + 1,
                  updated_at = ${observedAt}
              WHERE reservation_ref = ${exactComputer.reservation_ref}
                AND generation = ${item.generation}
                AND status IN ('provisioning', 'provisioning_uncertain', 'starting')
            `);
            repairs.push(tx`
              UPDATE khala_sync_cloud_computers
              SET state = 'active', active_lease_ref = ${item.providerLeaseRef},
                  version = version + 1, updated_at = ${observedAt}
              WHERE computer_ref = ${item.computerRef} AND generation = ${item.generation}
            `);
          } else {
            append(
              sameComputer === undefined ? "leaked" : "generation_mismatch",
              item,
              sameComputer,
            );
          }
        } else if (
          row.computer_ref !== item.computerRef ||
          Number(row.generation) !== item.generation
        ) {
          append("generation_mismatch", item, row);
        } else if (["provisioning_uncertain", "starting"].includes(row.status)) {
          repairs.push(tx`
            UPDATE khala_sync_cloud_capacity_reservations
            SET status = 'active', reason = 'provider_observation_settled',
                revision = revision + 1, updated_at = ${observedAt}
            WHERE reservation_ref = ${row.reservation_ref}
              AND generation = ${item.generation}
              AND provider_lease_ref = ${item.providerLeaseRef}
              AND status IN ('provisioning_uncertain', 'starting')
          `);
          repairs.push(tx`
            UPDATE khala_sync_cloud_computers
            SET state = 'active', active_lease_ref = ${item.providerLeaseRef},
                version = version + 1, updated_at = ${observedAt}
            WHERE computer_ref = ${item.computerRef} AND generation = ${item.generation}
          `);
        }
      }
      const observedRefs = new Set(observed.map((item) => item.providerLeaseRef));
      for (const row of live) {
        if (row.provider_lease_ref === null || observedRefs.has(row.provider_lease_ref)) continue;
        drift.push({
          driftRef: `drift.missing.${row.provider_lease_ref}.${nowMs}`,
          kind: "missing",
          providerLeaseRef: row.provider_lease_ref,
          reservationRef: row.reservation_ref,
          computerRef: row.computer_ref,
          expectedGeneration: Number(row.generation),
          observedGeneration: Number(row.generation),
          evidenceDigest: inventoryEvidenceDigest,
          observedAt,
        });
        repairs.push(tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'release_uncertain', reason = 'provider_lease_missing',
              revision = revision + 1, updated_at = ${observedAt}
          WHERE reservation_ref = ${row.reservation_ref}
            AND generation = ${Number(row.generation)}
            AND provider_lease_ref = ${row.provider_lease_ref}
            AND deadline_at <= ${observedAt}
            AND status IN ('starting', 'active', 'releasing')
        `);
      }
      for (const row of live) {
        if (
          row.provider_lease_ref !== null ||
          row.provider_operation_ref === null ||
          !["provisioning", "provisioning_uncertain"].includes(row.status)
        ) {
          continue;
        }
        const missingRef = row.provider_operation_ref;
        drift.push({
          driftRef: `drift.missing.${missingRef}.${nowMs}`,
          kind: "missing",
          providerLeaseRef: missingRef,
          reservationRef: row.reservation_ref,
          computerRef: row.computer_ref,
          expectedGeneration: Number(row.generation),
          observedGeneration: null,
          evidenceDigest: inventoryEvidenceDigest,
          observedAt,
        });
        repairs.push(tx`
          UPDATE khala_sync_cloud_capacity_reservations
          SET status = 'release_uncertain', reason = 'provider_create_unresolved',
              revision = revision + 1, updated_at = ${observedAt}
          WHERE reservation_ref = ${row.reservation_ref}
            AND generation = ${Number(row.generation)}
            AND provider_operation_ref = ${missingRef}
            AND deadline_at <= ${observedAt}
            AND status IN ('provisioning', 'provisioning_uncertain')
        `);
      }
      await Promise.all(repairs);
      await Promise.all(
        drift.map(
          (item) => tx`
          INSERT INTO khala_sync_cloud_capacity_drift
            (drift_ref, provider, region, provider_lease_ref, computer_ref,
             reservation_ref, expected_generation, observed_generation, kind,
             evidence_digest, observed_at)
          VALUES
            (${item.driftRef}, ${provider}, ${region}, ${item.providerLeaseRef},
             ${item.computerRef}, ${item.reservationRef}, ${item.expectedGeneration},
             ${item.observedGeneration}, ${item.kind},
             ${item.evidenceDigest}, ${item.observedAt})
          ON CONFLICT (drift_ref) DO NOTHING
        `,
        ),
      );
      return drift;
    });
  }

  async settleRelease(
    input: Readonly<{
      reservationRef: string;
      computerRef: string;
      generation: number;
      providerLeaseRef: string | null;
      providerOperationRef: string;
      cleanupEvidenceDigest: string;
      observedAt: string;
    }>,
  ): Promise<void> {
    if (!SHA256.test(input.cleanupEvidenceDigest)) {
      throw new CloudComputerCapacityStoreError("invalid", "cleanup evidence digest is invalid");
    }
    assertTimestamp(input.observedAt, "observed at");
    await this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${GLOBAL_CAPACITY_LOCK}, 3104))`;
      const rows: ReadonlyArray<ReservationRow> = await tx`
        SELECT reservation_ref, command_ref, request_digest, computer_ref, generation,
               tenant_ref, conversation_ref, runtime_class, provider, region, status,
               provider_lease_ref, provider_operation_ref, demand_json, receipt_json
        FROM khala_sync_cloud_capacity_reservations
        WHERE reservation_ref = ${input.reservationRef}
        FOR UPDATE
      `;
      const row = rows[0];
      if (row === undefined)
        throw new CloudComputerCapacityStoreError("not_found", "reservation missing");
      if (
        row.computer_ref !== input.computerRef ||
        Number(row.generation) !== input.generation ||
        row.provider_lease_ref !== input.providerLeaseRef ||
        row.provider_operation_ref !== input.providerOperationRef
      ) {
        throw new CloudComputerCapacityStoreError("stale_generation", "cleanup ownership differs");
      }
      if (row.status === "released") return;
      if (!["release_uncertain", "releasing", "quarantined"].includes(row.status)) {
        throw new CloudComputerCapacityStoreError("conflict", "reservation is not releasable");
      }
      await tx`
        UPDATE khala_sync_cloud_capacity_reservations
        SET status = 'released', reason = ${`cleanup_confirmed:${input.cleanupEvidenceDigest}`},
            released_at = ${input.observedAt}, revision = revision + 1,
            updated_at = ${input.observedAt}
        WHERE reservation_ref = ${input.reservationRef}
          AND generation = ${input.generation}
      `;
      await tx`
        UPDATE khala_sync_cloud_computers
        SET state = 'failed', active_lease_ref = NULL, version = version + 1,
            updated_at = ${input.observedAt}
        WHERE computer_ref = ${input.computerRef} AND generation = ${input.generation}
      `;
    });
  }
}
