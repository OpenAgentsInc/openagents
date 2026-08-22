import { describe, expect, test } from "vite-plus/test";

import {
  CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA,
  CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA,
  CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA,
  CloudComputerCapacityValidationError,
  type CapacityResources,
  type CloudComputerCapacityDemand,
  type CloudComputerCapacityObservation,
  type CloudComputerCapacityPolicy,
  type CloudComputerCapacityUsage,
  deterministicStartJitterMs,
  evaluateCloudComputerAdmission,
  firecrackerCapacityAdapter,
  scheduleCloudComputerCapacity,
  startDeadlineAt,
  validateCloudComputerCapacityDemand,
  validateCloudComputerCapacityObservation,
  validateCloudComputerCapacityPolicy,
} from "./cloud-computer-capacity.js";

const now = Date.parse("2026-08-22T16:00:00.000Z");

const resources = (overrides: Partial<CapacityResources> = {}): CapacityResources => ({
  concurrency: 100,
  cpuMillicores: 100_000,
  memoryMiB: 200_000,
  scratchMiB: 1_000_000,
  durationMs: 100_000_000,
  startsPerMinute: 100,
  costMicros: 100_000_000,
  ...overrides,
});

const demandResources = (overrides: Partial<CapacityResources> = {}): CapacityResources => ({
  concurrency: 1,
  cpuMillicores: 1_000,
  memoryMiB: 2_048,
  scratchMiB: 10_240,
  durationMs: 3_600_000,
  startsPerMinute: 1,
  costMicros: 100_000,
  ...overrides,
});

const zero = (): CapacityResources =>
  resources({
    concurrency: 0,
    cpuMillicores: 0,
    memoryMiB: 0,
    scratchMiB: 0,
    durationMs: 0,
    startsPerMinute: 0,
    costMicros: 0,
  });

const policy = (
  overrides: Partial<CloudComputerCapacityPolicy> = {},
): CloudComputerCapacityPolicy => ({
  schema: CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA,
  quotaFreshnessMs: 60_000,
  maxStartBatch: 20,
  jitterWindowMs: 1_000,
  providerHeadroomBps: 2_500,
  batchActivePerConversation: 8,
  privatePreviewCeiling: resources(),
  protectedReserve: zero(),
  limits: {
    global: resources(),
    provider: resources(),
    region: resources(),
    owner: resources(),
    tenant: resources(),
    conversation: resources(),
  },
  ...overrides,
});

const observation = (
  overrides: Partial<CloudComputerCapacityObservation> = {},
): CloudComputerCapacityObservation => ({
  schema: CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA,
  observationRef: "observation.capacity.1",
  observedAt: new Date(now).toISOString(),
  providerQuota: resources(),
  allocatableCapacity: resources(),
  drainAdjustedCapacity: resources(),
  budgetCapacity: resources(),
  ...overrides,
});

const demand = (
  ordinal: number,
  overrides: Partial<CloudComputerCapacityDemand> = {},
): CloudComputerCapacityDemand => ({
  schema: CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA,
  demandRef: `demand.capacity.${ordinal}`,
  ownerRef: "owner.alpha",
  tenantRef: "tenant.alpha",
  conversationRef: "conversation.alpha",
  computerRef: `computer.alpha.${ordinal}`,
  runtimeClass: "standard",
  priority: "normal",
  resources: demandResources(),
  requestedAt: new Date(now + ordinal).toISOString(),
  tenantWeight: 1,
  conversationWeight: 1,
  existingLogicalComputers: 15,
  existingActiveByRuntimeClass: { standard: 0, strong: 0, batch: 0 },
  createsLogicalComputer: false,
  highFanOutBudgeted: false,
  ...overrides,
});

const usage = (global = zero()): CloudComputerCapacityUsage => ({
  provider: global,
  region: global,
  global,
  owners: {},
  tenants: {},
  conversations: {},
});

describe("cloud computer capacity policy", () => {
  test("admits four of 15 standard requests and queues 11 for one conversation", () => {
    const result = scheduleCloudComputerCapacity({
      policy: policy(),
      observation: observation(),
      usage: usage(),
      demands: Array.from({ length: 15 }, (_, index) => demand(index + 1)),
      nowMs: now,
    });

    expect(result.admitted).toHaveLength(4);
    expect(result.queued).toHaveLength(11);
    expect(result.refused).toHaveLength(0);
    expect(result.queued.every((receipt) => receipt.reason === "runtime_class_limit")).toBe(true);
  });

  test("admits eight standard computers only with an explicit high-fan-out budget", () => {
    const result = scheduleCloudComputerCapacity({
      policy: policy(),
      observation: observation(),
      usage: usage(),
      demands: Array.from({ length: 9 }, (_, index) =>
        demand(index + 1, {
          highFanOutBudgeted: true,
        }),
      ),
      nowMs: now,
    });

    expect(result.admitted).toHaveLength(8);
    expect(result.queued).toHaveLength(1);
  });

  test("limits strong computers to two active computers per conversation", () => {
    const result = scheduleCloudComputerCapacity({
      policy: policy(),
      observation: observation(),
      usage: usage(),
      demands: [1, 2, 3].map((index) => demand(index, { runtimeClass: "strong" })),
      nowMs: now,
    });

    expect(result.admitted).toHaveLength(2);
    expect(result.queued).toHaveLength(1);
    expect(result.queued[0]?.reason).toBe("runtime_class_limit");
  });

  test("refuses a 31st logical computer before capacity admission", () => {
    const receipt = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: observation(),
      usage: usage(),
      demand: demand(1, { createsLogicalComputer: true, existingLogicalComputers: 30 }),
      nowMs: now,
    });

    expect(receipt).toMatchObject({ status: "refused", reason: "logical_computer_limit" });
  });

  test("uses deterministic weighted fairness across tenants and conversations", () => {
    const demands = [
      demand(1, { tenantRef: "tenant.alpha", conversationRef: "conversation.alpha.one" }),
      demand(2, { tenantRef: "tenant.alpha", conversationRef: "conversation.alpha.one" }),
      demand(3, { tenantRef: "tenant.alpha", conversationRef: "conversation.alpha.two" }),
      demand(4, { tenantRef: "tenant.beta", conversationRef: "conversation.beta.one" }),
      demand(5, { tenantRef: "tenant.beta", conversationRef: "conversation.beta.one" }),
      demand(6, { tenantRef: "tenant.beta", conversationRef: "conversation.beta.two" }),
    ];
    const constrainedPolicy = policy({ maxStartBatch: 4 });
    const first = scheduleCloudComputerCapacity({
      policy: constrainedPolicy,
      observation: observation(),
      usage: usage(),
      demands,
      nowMs: now,
    });
    const second = scheduleCloudComputerCapacity({
      policy: constrainedPolicy,
      observation: observation(),
      usage: usage(),
      demands,
      nowMs: now,
    });

    expect(first.admitted.map((receipt) => receipt.tenantRef)).toEqual([
      "tenant.alpha",
      "tenant.beta",
      "tenant.alpha",
      "tenant.beta",
    ]);
    expect(first.admitted.map((receipt) => receipt.conversationRef)).toEqual([
      "conversation.alpha.one",
      "conversation.beta.one",
      "conversation.alpha.two",
      "conversation.beta.two",
    ]);
    expect(second.admitted).toEqual(first.admitted);
  });

  test("honors tenant weights without starving another tenant", () => {
    const demands = Array.from({ length: 12 }, (_, index) => {
      const beta = index % 3 === 2;
      return demand(index + 1, {
        tenantRef: beta ? "tenant.beta" : "tenant.alpha",
        conversationRef: beta ? "conversation.beta" : `conversation.alpha.${index}`,
        tenantWeight: beta ? 1 : 2,
      });
    });
    const result = scheduleCloudComputerCapacity({
      policy: policy({ maxStartBatch: 6 }),
      observation: observation(),
      usage: usage(),
      demands,
      nowMs: now,
    });

    expect(result.admitted.filter((receipt) => receipt.tenantRef === "tenant.alpha")).toHaveLength(
      4,
    );
    expect(result.admitted.filter((receipt) => receipt.tenantRef === "tenant.beta")).toHaveLength(
      2,
    );
  });

  test("queues normal work when the provider quota observation is stale", () => {
    const receipt = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: observation({ observedAt: new Date(now - 60_001).toISOString() }),
      usage: usage(),
      demand: demand(1),
      nowMs: now,
    });

    expect(receipt).toMatchObject({ status: "queued", reason: "provider_quota_stale" });
  });

  test("lets cleanup use provider headroom while normal work remains queued", () => {
    const small = resources({ concurrency: 4 });
    const used = zero();
    const currentUsage = usage({ ...used, concurrency: 3 });
    const observed = observation({ providerQuota: small });
    const normal = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: observed,
      usage: currentUsage,
      demand: demand(1),
      nowMs: now,
    });
    const cleanup = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: observed,
      usage: currentUsage,
      demand: demand(2, { priority: "cleanup" }),
      nowMs: now,
    });

    expect(normal).toMatchObject({ status: "queued", reason: "provider_quota_exhausted" });
    expect(cleanup).toMatchObject({ status: "admitted", reason: "admitted" });
  });

  test("reserves at least 25 percent of every private-preview resource for protected work", () => {
    const preview = resources({ concurrency: 8 });
    const configuredReserve = resources({ concurrency: 0 });
    const currentUsage = usage({ ...zero(), concurrency: 6 });
    const normal = evaluateCloudComputerAdmission({
      policy: policy({ privatePreviewCeiling: preview, protectedReserve: configuredReserve }),
      observation: observation(),
      usage: currentUsage,
      demand: demand(1),
      nowMs: now,
    });
    const cleanup = evaluateCloudComputerAdmission({
      policy: policy({ privatePreviewCeiling: preview, protectedReserve: configuredReserve }),
      observation: observation(),
      usage: currentUsage,
      demand: demand(2, { priority: "cleanup" }),
      nowMs: now,
    });

    expect(normal).toMatchObject({ status: "queued", reason: "private_preview_ceiling" });
    expect(cleanup).toMatchObject({ status: "admitted", reason: "admitted" });
  });

  test("lets recovery use reserved capacity with a stale observation", () => {
    const stale = observation({ observedAt: new Date(now - 120_000).toISOString() });
    const normal = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: stale,
      usage: usage(),
      demand: demand(1),
      nowMs: now,
    });
    const recovery = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: stale,
      usage: usage(),
      demand: demand(2, { priority: "recovery" }),
      nowMs: now,
    });

    expect(normal.reason).toBe("provider_quota_stale");
    expect(recovery.status).toBe("admitted");
  });

  test("applies the private-preview ceiling before a provider call", () => {
    const receipt = evaluateCloudComputerAdmission({
      policy: policy({ privatePreviewCeiling: resources({ concurrency: 2 }) }),
      observation: observation(),
      usage: usage({ ...zero(), concurrency: 2 }),
      demand: demand(1),
      nowMs: now,
    });

    expect(receipt).toMatchObject({ status: "queued", reason: "private_preview_ceiling" });
  });

  test("normalizes Firecracker capacity through the common observation adapter", async () => {
    let polls = 0;
    const adapter = firecrackerCapacityAdapter(async () => {
      polls += 1;
      return observation();
    });
    expect(adapter.provider).toBe("firecracker");
    await expect(adapter.observe()).resolves.toMatchObject({
      schema: CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA,
    });
    expect(polls).toBe(1);
  });

  test.each([
    ["global_limit", "global"],
    ["provider_limit", "provider"],
    ["region_limit", "region"],
    ["owner_limit", "owner"],
    ["tenant_limit", "tenant"],
    ["conversation_limit", "conversation"],
  ] as const)("reports %s for a cost dimension failure", (reason, scope) => {
    const base = policy();
    const limit = resources({ costMicros: 50_000 });
    const limits = { ...base.limits, [scope]: limit };
    const receipt = evaluateCloudComputerAdmission({
      policy: { ...base, limits },
      observation: observation(),
      usage: usage(),
      demand: demand(1),
      nowMs: now,
    });

    expect(receipt).toMatchObject({ status: "refused", reason });
  });

  test.each([
    ["cpuMillicores", 999],
    ["memoryMiB", 2_047],
    ["scratchMiB", 10_239],
    ["durationMs", 3_599_999],
    ["startsPerMinute", 0],
    ["costMicros", 99_999],
  ] as const)("reports a budget failure for the %s dimension", (key, value) => {
    const receipt = evaluateCloudComputerAdmission({
      policy: policy(),
      observation: observation({ budgetCapacity: resources({ [key]: value }) }),
      usage: usage(),
      demand: demand(1),
      nowMs: now,
    });

    expect(receipt).toMatchObject({ status: "queued", reason: "budget" });
  });

  test("bounds each start batch", () => {
    const result = scheduleCloudComputerCapacity({
      policy: policy({ maxStartBatch: 2 }),
      observation: observation(),
      usage: usage(),
      demands: [demand(1), demand(2), demand(3)],
      nowMs: now,
    });

    expect(result.admitted).toHaveLength(2);
    expect(result.queued).toHaveLength(1);
  });

  test("derives stable bounded jitter and a deterministic start deadline", () => {
    const first = deterministicStartJitterMs("demand.capacity.42", 1_000);
    const second = deterministicStartJitterMs("demand.capacity.42", 1_000);

    expect(first).toBe(second);
    expect(first).toBeGreaterThanOrEqual(0);
    expect(first).toBeLessThanOrEqual(1_000);
    expect(startDeadlineAt("2026-08-22T16:00:00.000Z", 60_000, first)).toBe(
      new Date(now + 60_000 + first).toISOString(),
    );
  });

  test("validates policy, observation, and demand without echoing rejected data", () => {
    expect(validateCloudComputerCapacityPolicy(policy())).toEqual(policy());
    expect(validateCloudComputerCapacityObservation(observation())).toEqual(observation());
    expect(validateCloudComputerCapacityDemand(demand(1))).toEqual(demand(1));

    const secret = "SECRET-CONTROL-PLANE-TOKEN";
    expect(() => validateCloudComputerCapacityDemand(demand(1, { tenantRef: secret }))).toThrow(
      CloudComputerCapacityValidationError,
    );
    try {
      validateCloudComputerCapacityDemand(demand(1, { tenantRef: secret }));
    } catch (error) {
      expect(String(error)).not.toContain(secret);
    }
    expect(() =>
      validateCloudComputerCapacityPolicy(policy({ providerHeadroomBps: 2_499 })),
    ).toThrow("invalid policy.providerHeadroomBps");
  });
});
