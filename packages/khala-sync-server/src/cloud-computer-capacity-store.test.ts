import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA,
  CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA,
  CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA,
  type CapacityResources,
  type CloudComputerCapacityDemand,
  type CloudComputerCapacityObservation,
  type CloudComputerCapacityPolicy,
  deterministicStartJitterMs,
} from "./cloud-computer-capacity.js";
import {
  CloudComputerCapacityStoreError,
  PostgresCloudComputerCapacityStore,
  dispatchCloudComputerProviderStart,
  type CloudComputerInventoryInput,
} from "./cloud-computer-capacity-store.js";
import { runMigrations } from "./migrate.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const at = (seconds: number): string =>
  new Date(Date.UTC(2026, 7, 22, 17, 0, seconds)).toISOString();
const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const resources = (slots: number): CapacityResources => ({
  concurrency: slots,
  cpuMillicores: slots * 1_000,
  memoryMiB: slots * 2_048,
  scratchMiB: slots * 1_024,
  durationMs: slots * 60_000,
  startsPerMinute: slots,
  costMicros: slots * 1_000_000,
});

const policy: CloudComputerCapacityPolicy = {
  schema: CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA,
  quotaFreshnessMs: 60_000,
  maxStartBatch: 8,
  jitterWindowMs: 0,
  providerHeadroomBps: 2_500,
  batchActivePerConversation: 16,
  privatePreviewCeiling: resources(8),
  protectedReserve: resources(2),
  limits: {
    global: resources(8),
    provider: resources(8),
    region: resources(8),
    owner: resources(8),
    tenant: resources(8),
    conversation: resources(8),
  },
};

const observation = (ref: string, observedAt = at(0)): CloudComputerCapacityObservation => ({
  schema: CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA,
  observationRef: ref,
  observedAt,
  providerQuota: resources(20),
  allocatableCapacity: resources(20),
  drainAdjustedCapacity: resources(20),
  budgetCapacity: resources(20),
});

const inventory = (
  suffix: string,
  conversationRef = "conversation.capacity.main",
  tenantRef = "tenant.capacity.main",
): CloudComputerInventoryInput => ({
  computerRef: `computer.capacity.${suffix}`,
  ownerRef: "owner.capacity.main",
  tenantRef,
  conversationRef,
  workUnitRef: `work.capacity.${suffix}`,
  kind: "interactive_retained",
  runtimeClass: "standard",
  generation: 1,
  version: 1,
  runtimeProfileRef: "profile.capacity.standard",
  authoritySnapshotDigest: digest("a"),
  budgetSnapshotDigest: digest("b"),
  capabilityRefs: ["capability.capacity.execute"],
  observedAt: at(0),
});

const demand = (
  suffix: string,
  conversationRef = "conversation.capacity.main",
  tenantRef = "tenant.capacity.main",
  priority: CloudComputerCapacityDemand["priority"] = "normal",
): CloudComputerCapacityDemand => ({
  schema: CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA,
  demandRef: `demand.capacity.${suffix}`,
  ownerRef: "owner.capacity.main",
  tenantRef,
  conversationRef,
  computerRef: `computer.capacity.${suffix}`,
  runtimeClass: "standard",
  priority,
  resources: resources(1),
  requestedAt: at(0),
  tenantWeight: 1,
  conversationWeight: 1,
  existingLogicalComputers: 0,
  existingActiveByRuntimeClass: { standard: 0, strong: 0, batch: 0 },
  createsLogicalComputer: false,
  highFanOutBudgeted: false,
});

describe.skipIf(!hasLocalPostgres())("cloud computer capacity Postgres authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let store: PostgresCloudComputerCapacityStore;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const databaseName = `khala_sync_cloud_capacity_${process.pid}_${Date.now()}`;
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe(`CREATE DATABASE ${databaseName}`);
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor(databaseName) });
    sql = SQL({ url: pg.urlFor(databaseName), max: 24 });
    store = new PostgresCloudComputerCapacityStore(sql as unknown as SyncSql, {
      authorizeProtected: async ({ authorityRef }) => authorityRef === "authority.cleanup.main",
    });
    await store.putPolicy({
      policyRef: "policy.capacity.main",
      provider: "provider.capacity.main",
      region: "region.capacity.main",
      runtimeClass: "standard",
      policy,
      updatedAt: at(0),
    });
    await store.recordObservation({
      provider: "provider.capacity.main",
      region: "region.capacity.main",
      observation: observation("observation.capacity.main"),
      expiresAt: at(60),
    });
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("creates 15 logical computers and serializes admission at four active standard slots", async () => {
    const records = Array.from({ length: 15 }, (_, index) => inventory(`fanout.${index}`));
    await store.createLogicalComputers(records);
    const receipts = await Promise.all(
      records.map((record, index) =>
        store.admit({
          reservationRef: `reservation.capacity.fanout.${index}`,
          commandRef: `command.capacity.fanout.${index}`,
          requestDigest: digest(index.toString(16)),
          ownerRef: record.ownerRef,
          provider: "provider.capacity.main",
          region: "region.capacity.main",
          generation: 1,
          demand: demand(`fanout.${index}`),
          nowMs: Date.parse(at(0)),
        }),
      ),
    );
    expect(receipts.filter((receipt) => receipt.status === "admitted")).toHaveLength(4);
    expect(receipts.filter((receipt) => receipt.status === "queued")).toHaveLength(11);
    const rows: ReadonlyArray<{ readonly status: string; readonly count: string | number }> =
      await sql`
        SELECT status, COUNT(*) AS count
        FROM khala_sync_cloud_capacity_reservations
        WHERE conversation_ref = 'conversation.capacity.main'
        GROUP BY status
      `;
    expect(Object.fromEntries(rows.map((row) => [row.status, Number(row.count)]))).toEqual({
      queued: 11,
      reserved: 4,
    });
  });

  test("replays exact admission and rejects changed request bytes", async () => {
    const exact = await store.admit({
      reservationRef: "reservation.capacity.fanout.0",
      commandRef: "command.capacity.fanout.0",
      requestDigest: digest("0"),
      ownerRef: "owner.capacity.main",
      provider: "provider.capacity.main",
      region: "region.capacity.main",
      generation: 1,
      demand: demand("fanout.0"),
      nowMs: Date.parse(at(30)),
    });
    expect(exact.demandRef).toBe("demand.capacity.fanout.0");
    await expect(
      store.admit({
        reservationRef: "reservation.capacity.changed",
        commandRef: "command.capacity.fanout.0",
        requestDigest: digest("f"),
        ownerRef: "owner.capacity.main",
        provider: "provider.capacity.main",
        region: "region.capacity.main",
        generation: 1,
        demand: demand("fanout.0"),
        nowMs: Date.parse(at(30)),
      }),
    ).rejects.toMatchObject({ code: "conflict" });
  });

  test("rejects a 31-computer inventory atomically", async () => {
    const records = Array.from({ length: 31 }, (_, index) =>
      inventory(`limit.${index}`, "conversation.capacity.limit", "tenant.capacity.limit"),
    );
    await expect(store.createLogicalComputers(records)).rejects.toBeInstanceOf(
      CloudComputerCapacityStoreError,
    );
    const rows: ReadonlyArray<{ readonly count: string | number }> = await sql`
      SELECT COUNT(*) AS count FROM khala_sync_cloud_computers
      WHERE conversation_ref = 'conversation.capacity.limit'
    `;
    expect(Number(rows[0]?.count ?? -1)).toBe(0);
  });

  test("queues stale normal work but admits authenticated cleanup from protected capacity", async () => {
    const provider = "provider.capacity.stale";
    const region = "region.capacity.stale";
    await store.putPolicy({
      policyRef: "policy.capacity.stale",
      provider,
      region,
      runtimeClass: "standard",
      policy,
      updatedAt: at(0),
    });
    await store.recordObservation({
      provider,
      region,
      observation: observation("observation.capacity.stale", at(0)),
      expiresAt: at(60),
    });
    await store.createLogicalComputers([
      inventory("stale.normal", "conversation.capacity.stale", "tenant.capacity.stale"),
      inventory("stale.cleanup", "conversation.capacity.stale", "tenant.capacity.stale"),
    ]);
    const nowMs = Date.parse(at(120));
    const normal = await store.admit({
      reservationRef: "reservation.capacity.stale.normal",
      commandRef: "command.capacity.stale.normal",
      requestDigest: digest("c"),
      ownerRef: "owner.capacity.main",
      provider,
      region,
      generation: 1,
      demand: demand("stale.normal", "conversation.capacity.stale", "tenant.capacity.stale"),
      nowMs,
    });
    await expect(
      store.admit({
        reservationRef: "reservation.capacity.stale.unauthorized",
        commandRef: "command.capacity.stale.unauthorized",
        requestDigest: digest("1"),
        ownerRef: "owner.capacity.main",
        provider,
        region,
        generation: 1,
        protectedAuthorityRef: "authority.cleanup.forged",
        demand: demand(
          "stale.cleanup",
          "conversation.capacity.stale",
          "tenant.capacity.stale",
          "cleanup",
        ),
        nowMs,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
    const cleanup = await store.admit({
      reservationRef: "reservation.capacity.stale.cleanup",
      commandRef: "command.capacity.stale.cleanup",
      requestDigest: digest("d"),
      ownerRef: "owner.capacity.main",
      provider,
      region,
      generation: 1,
      protectedAuthorityRef: "authority.cleanup.main",
      demand: demand(
        "stale.cleanup",
        "conversation.capacity.stale",
        "tenant.capacity.stale",
        "cleanup",
      ),
      nowMs,
    });
    expect(normal).toMatchObject({ status: "queued", reason: "provider_quota_stale" });
    expect(cleanup).toMatchObject({ status: "admitted", reason: "admitted" });
  });

  test("records provider uncertainty once and never reissues the start claim", async () => {
    const candidates: ReadonlyArray<{ readonly reservation_ref: string }> = await sql`
      SELECT reservation_ref FROM khala_sync_cloud_capacity_reservations
      WHERE status = 'reserved' ORDER BY reservation_ref LIMIT 1
    `;
    const reservationRef = candidates[0]!.reservation_ref;
    const claim = await store.claimProviderStart(reservationRef, Date.parse(at(1)));
    expect(claim.disposition).toBe("proceed");
    const retry = await store.claimProviderStart(reservationRef, Date.parse(at(2)));
    expect(retry).toEqual({ disposition: "pending_uncertain" });
    expect(await store.markProvisioningUncertain(Date.parse(at(61)))).toBe(1);
  });

  test("does not call a provider for an expired start claim", async () => {
    const candidates: ReadonlyArray<{ readonly reservation_ref: string }> = await sql`
      SELECT reservation_ref FROM khala_sync_cloud_capacity_reservations
      WHERE status = 'reserved' ORDER BY reservation_ref LIMIT 1
    `;
    const result = await store.claimProviderStart(
      candidates[0]!.reservation_ref,
      Date.parse(at(61)),
    );
    let providerCalls = 0;
    const dispatched = await dispatchCloudComputerProviderStart(store, result, {
      start: async () => {
        providerCalls += 1;
        return { providerLeaseRef: "lease.should.not.exist", observedAt: at(61) };
      },
    });
    expect(dispatched).toEqual({ disposition: "not_dispatched" });
    expect(providerCalls).toBe(0);
  });

  test("requires authenticated budget authority for high-fan-out admission", async () => {
    await expect(
      store.admit({
        reservationRef: "reservation.capacity.high-fan-out.unauthorized",
        commandRef: "command.capacity.high-fan-out.unauthorized",
        requestDigest: digest("2"),
        ownerRef: "owner.capacity.main",
        provider: "provider.capacity.main",
        region: "region.capacity.main",
        generation: 1,
        demand: {
          ...demand("fanout.14"),
          demandRef: "demand.capacity.high-fan-out.unauthorized",
          highFanOutBudgeted: true,
        },
        nowMs: Date.parse(at(1)),
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });
  });

  test("rechecks fresh capacity after enforcing deterministic start jitter", async () => {
    const provider = "provider.capacity.dispatch-gate";
    const region = "region.capacity.dispatch-gate";
    const gatedPolicy = { ...policy, jitterWindowMs: 10_000 };
    const computer = inventory(
      "dispatch-gate",
      "conversation.capacity.dispatch-gate",
      "tenant.capacity.dispatch-gate",
    );
    await store.putPolicy({
      policyRef: "policy.capacity.dispatch-gate",
      provider,
      region,
      runtimeClass: "standard",
      policy: gatedPolicy,
      updatedAt: at(0),
    });
    await store.recordObservation({
      provider,
      region,
      observation: observation("observation.capacity.dispatch-gate"),
      expiresAt: at(60),
    });
    await store.createLogicalComputers([computer]);
    const gatedDemand = demand(
      "dispatch-gate",
      "conversation.capacity.dispatch-gate",
      "tenant.capacity.dispatch-gate",
    );
    await store.admit({
      reservationRef: "reservation.capacity.dispatch-gate",
      commandRef: "command.capacity.dispatch-gate",
      requestDigest: digest("8"),
      ownerRef: computer.ownerRef,
      provider,
      region,
      generation: 1,
      demand: gatedDemand,
      nowMs: Date.parse(at(0)),
    });
    const jitterMs = deterministicStartJitterMs(gatedDemand.demandRef, 10_000);
    expect(jitterMs).toBeGreaterThan(0);
    await expect(
      store.claimProviderStart("reservation.capacity.dispatch-gate", Date.parse(at(0))),
    ).resolves.toEqual({ disposition: "jitter_pending" });
    const gatedAt = new Date(Date.parse(at(0)) + jitterMs).toISOString();
    await store.recordObservation({
      provider,
      region,
      observation: {
        ...observation("observation.capacity.dispatch-gate.exhausted", gatedAt),
        providerQuota: resources(0),
        allocatableCapacity: resources(0),
        drainAdjustedCapacity: resources(0),
        budgetCapacity: resources(0),
      },
      expiresAt: at(60),
    });
    await expect(
      store.claimProviderStart("reservation.capacity.dispatch-gate", Date.parse(gatedAt)),
    ).resolves.toEqual({ disposition: "at_capacity" });
    await store.recordObservation({
      provider,
      region,
      observation: observation("observation.capacity.dispatch-gate.recovered", at(20)),
      expiresAt: at(60),
    });
    await store.claimQueued(provider, region, "standard", Date.parse(at(20)));
    const promoted: ReadonlyArray<{ readonly not_before_at: string | Date }> = await sql`
      SELECT not_before_at FROM khala_sync_cloud_capacity_reservations
      WHERE reservation_ref = 'reservation.capacity.dispatch-gate'
    `;
    expect(Date.parse(String(promoted[0]?.not_before_at))).toBeGreaterThan(Date.parse(at(20)));
    await expect(
      store.claimProviderStart("reservation.capacity.dispatch-gate", Date.parse(at(20))),
    ).resolves.toEqual({ disposition: "jitter_pending" });
  });

  test("enforces provider limits across regions", async () => {
    const provider = "provider.capacity.cross-region";
    const crossRegionPolicy: CloudComputerCapacityPolicy = {
      ...policy,
      limits: { ...policy.limits, global: resources(100), provider: resources(1) },
    };
    const entries = ["one", "two"].map((suffix) => ({
      suffix,
      region: `region.capacity.cross-region.${suffix}`,
      computer: inventory(
        `cross-region.${suffix}`,
        `conversation.capacity.cross-region.${suffix}`,
        "tenant.capacity.cross-region",
      ),
    }));
    await Promise.all(
      entries.map(async (entry) => {
        await store.putPolicy({
          policyRef: `policy.capacity.cross-region.${entry.suffix}`,
          provider,
          region: entry.region,
          runtimeClass: "standard",
          policy: crossRegionPolicy,
          updatedAt: at(0),
        });
        await store.recordObservation({
          provider,
          region: entry.region,
          observation: observation(`observation.capacity.cross-region.${entry.suffix}`),
          expiresAt: at(60),
        });
        await store.createLogicalComputers([entry.computer]);
      }),
    );
    const receipts = [];
    for (const [index, entry] of entries.entries()) {
      receipts.push(
        // eslint-disable-next-line no-await-in-loop -- the second admission must observe the first.
        await store.admit({
          reservationRef: `reservation.capacity.cross-region.${entry.suffix}`,
          commandRef: `command.capacity.cross-region.${entry.suffix}`,
          requestDigest: digest((index + 9).toString(16)),
          ownerRef: entry.computer.ownerRef,
          provider,
          region: entry.region,
          generation: 1,
          demand: demand(
            `cross-region.${entry.suffix}`,
            entry.computer.conversationRef,
            entry.computer.tenantRef,
          ),
          nowMs: Date.parse(at(0)),
        }),
      );
    }
    expect(receipts.map((receipt) => [receipt.status, receipt.reason])).toEqual([
      ["admitted", "admitted"],
      ["queued", "provider_limit"],
    ]);
  });

  test("uses a rolling one-minute start-rate window", async () => {
    const provider = "provider.capacity.start-rate";
    const region = "region.capacity.start-rate";
    const ratePolicy: CloudComputerCapacityPolicy = {
      ...policy,
      quotaFreshnessMs: 120_000,
      limits: {
        ...policy.limits,
        global: resources(100),
        provider: { ...resources(8), startsPerMinute: 1 },
        region: { ...resources(8), startsPerMinute: 1 },
      },
    };
    await store.putPolicy({
      policyRef: "policy.capacity.start-rate",
      provider,
      region,
      runtimeClass: "standard",
      policy: ratePolicy,
      updatedAt: at(0),
    });
    await store.recordObservation({
      provider,
      region,
      observation: observation("observation.capacity.start-rate"),
      expiresAt: at(120),
    });
    await Promise.all(
      ["one", "two"].map(async (suffix) => {
        const computer = inventory(
          `start-rate.${suffix}`,
          `conversation.capacity.start-rate.${suffix}`,
          "tenant.capacity.start-rate",
        );
        await store.createLogicalComputers([computer]);
        await store.admit({
          reservationRef: `reservation.capacity.start-rate.${suffix}`,
          commandRef: `command.capacity.start-rate.${suffix}`,
          requestDigest: digest(suffix === "one" ? "b" : "c"),
          ownerRef: computer.ownerRef,
          provider,
          region,
          generation: 1,
          demand: demand(`start-rate.${suffix}`, computer.conversationRef, computer.tenantRef),
          nowMs: Date.parse(at(0)),
        });
      }),
    );
    const first = await store.claimProviderStart(
      "reservation.capacity.start-rate.one",
      Date.parse(at(1)),
    );
    expect(first.disposition).toBe("proceed");
    await expect(
      store.claimProviderStart("reservation.capacity.start-rate.two", Date.parse(at(2))),
    ).resolves.toEqual({ disposition: "at_capacity" });
    await store.claimQueued(provider, region, "standard", Date.parse(at(61)));
    await expect(
      store.claimProviderStart("reservation.capacity.start-rate.two", Date.parse(at(61))),
    ).resolves.toMatchObject({ disposition: "proceed" });
  });

  test("settles exact-generation provider evidence and evidence-gated release", async () => {
    const provider = "provider.capacity.reconcile";
    const region = "region.capacity.reconcile";
    const computer = inventory(
      "reconcile",
      "conversation.capacity.reconcile",
      "tenant.capacity.reconcile",
    );
    const reconcilePolicy: CloudComputerCapacityPolicy = {
      ...policy,
      limits: {
        global: resources(100),
        provider: resources(100),
        region: resources(100),
        owner: resources(100),
        tenant: resources(100),
        conversation: resources(100),
      },
    };
    await store.putPolicy({
      policyRef: "policy.capacity.reconcile",
      provider,
      region,
      runtimeClass: "standard",
      policy: reconcilePolicy,
      updatedAt: at(0),
    });
    await store.recordObservation({
      provider,
      region,
      observation: observation("observation.capacity.reconcile"),
      expiresAt: at(60),
    });
    await store.createLogicalComputers([computer]);
    const receipt = await store.admit({
      reservationRef: "reservation.capacity.reconcile",
      commandRef: "command.capacity.reconcile",
      requestDigest: digest("3"),
      ownerRef: computer.ownerRef,
      provider,
      region,
      generation: 1,
      demand: demand("reconcile", "conversation.capacity.reconcile", "tenant.capacity.reconcile"),
      nowMs: Date.parse(at(0)),
    });
    expect(receipt.status).toBe("admitted");
    const claimed = await store.claimProviderStart(
      "reservation.capacity.reconcile",
      Date.parse(at(1)),
    );
    if (claimed.disposition !== "proceed") throw new Error("expected provider claim");
    await store.bindProviderLease(claimed.claim, "lease.capacity.reconcile", at(2));
    const mismatched = await store.reconcile(
      provider,
      region,
      [
        {
          providerLeaseRef: "lease.capacity.reconcile",
          providerOperationRef: "operation.capacity.unrelated",
          computerRef: computer.computerRef,
          generation: 1,
          quarantined: false,
          evidenceDigest: digest("4"),
        },
      ],
      Date.parse(at(3)),
      digest("5"),
    );
    expect(mismatched).toMatchObject([{ kind: "operation_mismatch" }]);
    await store.reconcile(
      provider,
      region,
      [
        {
          providerLeaseRef: "lease.capacity.reconcile",
          providerOperationRef: claimed.claim.operationRef,
          computerRef: computer.computerRef,
          generation: 1,
          quarantined: false,
          evidenceDigest: digest("4"),
        },
      ],
      Date.parse(at(4)),
      digest("5"),
    );
    const active: ReadonlyArray<{ readonly status: string; readonly state: string }> = await sql`
      SELECT reservation.status, computer.state
      FROM khala_sync_cloud_capacity_reservations AS reservation
      JOIN khala_sync_cloud_computers AS computer
        ON computer.computer_ref = reservation.computer_ref
       AND computer.generation = reservation.generation
      WHERE reservation.reservation_ref = 'reservation.capacity.reconcile'
    `;
    expect(active[0]).toEqual({ status: "active", state: "active" });

    await store.reconcile(provider, region, [], Date.parse(at(61)), digest("6"));
    await store.settleRelease({
      reservationRef: "reservation.capacity.reconcile",
      computerRef: computer.computerRef,
      generation: 1,
      providerLeaseRef: "lease.capacity.reconcile",
      providerOperationRef: claimed.claim.operationRef,
      cleanupEvidenceDigest: digest("7"),
      observedAt: at(62),
    });
    const released: ReadonlyArray<{ readonly status: string; readonly state: string }> = await sql`
      SELECT reservation.status, computer.state
      FROM khala_sync_cloud_capacity_reservations AS reservation
      JOIN khala_sync_cloud_computers AS computer
        ON computer.computer_ref = reservation.computer_ref
       AND computer.generation = reservation.generation
      WHERE reservation.reservation_ref = 'reservation.capacity.reconcile'
    `;
    expect(released[0]).toEqual({ status: "released", state: "failed" });
    const leaked = await store.reconcile(
      provider,
      region,
      [
        {
          providerLeaseRef: "lease.capacity.unowned",
          providerOperationRef: "operation.capacity.unowned",
          computerRef: "computer.capacity.unowned",
          generation: 1,
          quarantined: false,
          evidenceDigest: digest("8"),
        },
      ],
      Date.parse(at(63)),
      digest("9"),
    );
    expect(leaked).toMatchObject([{ kind: "leaked", reservationRef: null }]);
  });

  test("reports foreign-generation evidence without changing the live reservation", async () => {
    const candidates: ReadonlyArray<{
      readonly reservation_ref: string;
      readonly computer_ref: string;
    }> = await sql`
      SELECT reservation_ref, computer_ref FROM khala_sync_cloud_capacity_reservations
      WHERE status = 'reserved'
        AND provider = 'provider.capacity.main'
        AND region = 'region.capacity.main'
      ORDER BY reservation_ref LIMIT 1
    `;
    const candidate = candidates[0]!;
    const claimed = await store.claimProviderStart(candidate.reservation_ref, Date.parse(at(1)));
    if (claimed.disposition !== "proceed") throw new Error("expected provider claim");
    await store.bindProviderLease(claimed.claim, "lease.capacity.foreign", at(2));
    const drift = await store.reconcile(
      "provider.capacity.main",
      "region.capacity.main",
      [
        {
          providerLeaseRef: "lease.capacity.foreign",
          providerOperationRef: claimed.claim.operationRef,
          computerRef: candidate.computer_ref,
          generation: 2,
          quarantined: false,
          evidenceDigest: digest("e"),
        },
      ],
      Date.parse(at(3)),
      digest("f"),
    );
    expect(drift.some((item) => item.kind === "generation_mismatch")).toBe(true);
    const rows: ReadonlyArray<{
      readonly status: string;
      readonly generation: string | number;
      readonly provider_lease_ref: string | null;
    }> = await sql`
      SELECT status, generation, provider_lease_ref
      FROM khala_sync_cloud_capacity_reservations
      WHERE reservation_ref = ${candidate.reservation_ref}
    `;
    expect({ ...rows[0], generation: Number(rows[0]?.generation) }).toMatchObject({
      status: "starting",
      generation: 1,
      provider_lease_ref: "lease.capacity.foreign",
    });
  });
});
