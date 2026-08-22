export const CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA =
  "openagents.cloud_computer.capacity_policy.v1" as const;
export const CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA =
  "openagents.cloud_computer.capacity_observation.v1" as const;
export const CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA =
  "openagents.cloud_computer.capacity_demand.v1" as const;
export const CLOUD_COMPUTER_CAPACITY_RECEIPT_SCHEMA =
  "openagents.cloud_computer.capacity_receipt.v1" as const;

export const CLOUD_COMPUTER_MAX_LOGICAL_PER_CONVERSATION = 30;
export const CLOUD_COMPUTER_STANDARD_ACTIVE_DEFAULT = 4;
export const CLOUD_COMPUTER_STANDARD_ACTIVE_HIGH_FAN_OUT = 8;
export const CLOUD_COMPUTER_STRONG_ACTIVE_MAX = 2;
export const CLOUD_COMPUTER_MIN_PROVIDER_HEADROOM_BPS = 2_500;

export type CloudComputerRuntimeClass = "standard" | "strong" | "batch";
export type CloudComputerCapacityPriority = "normal" | "cleanup" | "replacement" | "recovery";
export type CloudComputerCapacityProvider =
  | "gce"
  | "gke_agent_sandbox"
  | "firecracker"
  | "cloud_run";

export type CapacityResources = Readonly<{
  concurrency: number;
  cpuMillicores: number;
  memoryMiB: number;
  scratchMiB: number;
  durationMs: number;
  startsPerMinute: number;
  costMicros: number;
}>;

export type CapacityLimitVector = CapacityResources;

export type CloudComputerCapacityPolicy = Readonly<{
  schema: typeof CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA;
  quotaFreshnessMs: number;
  maxStartBatch: number;
  jitterWindowMs: number;
  providerHeadroomBps: number;
  batchActivePerConversation: number;
  privatePreviewCeiling: CapacityResources;
  protectedReserve: CapacityResources;
  limits: Readonly<{
    global: CapacityLimitVector;
    provider: CapacityLimitVector;
    region: CapacityLimitVector;
    owner: CapacityLimitVector;
    tenant: CapacityLimitVector;
    conversation: CapacityLimitVector;
  }>;
}>;

export type CloudComputerCapacityObservation = Readonly<{
  schema: typeof CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA;
  observationRef: string;
  observedAt: string;
  providerQuota: CapacityResources;
  allocatableCapacity: CapacityResources;
  drainAdjustedCapacity: CapacityResources;
  budgetCapacity: CapacityResources;
}>;

export type CloudComputerCapacityUsage = Readonly<{
  provider: CapacityResources;
  region: CapacityResources;
  global: CapacityResources;
  owners: Readonly<Record<string, CapacityResources>>;
  tenants: Readonly<Record<string, CapacityResources>>;
  conversations: Readonly<Record<string, CapacityResources>>;
}>;

export type CloudComputerCapacityDemand = Readonly<{
  schema: typeof CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA;
  demandRef: string;
  ownerRef: string;
  tenantRef: string;
  conversationRef: string;
  computerRef: string;
  runtimeClass: CloudComputerRuntimeClass;
  priority: CloudComputerCapacityPriority;
  resources: CapacityResources;
  requestedAt: string;
  tenantWeight: number;
  conversationWeight: number;
  existingLogicalComputers: number;
  existingActiveByRuntimeClass: Readonly<Record<CloudComputerRuntimeClass, number>>;
  createsLogicalComputer: boolean;
  highFanOutBudgeted: boolean;
}>;

export type CloudComputerCapacityReason =
  | "admitted"
  | "start_batch_limit"
  | "logical_computer_limit"
  | "runtime_class_limit"
  | "provider_quota_stale"
  | "provider_quota_exhausted"
  | "private_preview_ceiling"
  | "allocatable_capacity"
  | "drain_capacity"
  | "budget"
  | "global_limit"
  | "provider_limit"
  | "region_limit"
  | "owner_limit"
  | "tenant_limit"
  | "conversation_limit";

export type CloudComputerCapacityReceipt = Readonly<{
  schema: typeof CLOUD_COMPUTER_CAPACITY_RECEIPT_SCHEMA;
  demandRef: string;
  ownerRef: string;
  tenantRef: string;
  conversationRef: string;
  computerRef: string;
  runtimeClass: CloudComputerRuntimeClass;
  priority: CloudComputerCapacityPriority;
  status: "admitted" | "queued" | "refused";
  reason: CloudComputerCapacityReason;
  observationRef: string;
  decidedAt: string;
  startDeadlineAt: string | null;
  retryAfterMs: number | null;
}>;

export type CloudComputerCapacitySchedule = Readonly<{
  admitted: ReadonlyArray<CloudComputerCapacityReceipt>;
  queued: ReadonlyArray<CloudComputerCapacityReceipt>;
  refused: ReadonlyArray<CloudComputerCapacityReceipt>;
  nextUsage: CloudComputerCapacityUsage;
}>;

export class CloudComputerCapacityValidationError extends Error {
  readonly kind = "invalid_capacity_input" as const;

  constructor(readonly field: string) {
    super(`invalid ${field}`);
    this.name = "CloudComputerCapacityValidationError";
  }
}

const resourceKeys = [
  "concurrency",
  "cpuMillicores",
  "memoryMiB",
  "scratchMiB",
  "durationMs",
  "startsPerMinute",
  "costMicros",
] as const;

const zeroResources = (): CapacityResources => ({
  concurrency: 0,
  cpuMillicores: 0,
  memoryMiB: 0,
  scratchMiB: 0,
  durationMs: 0,
  startsPerMinute: 0,
  costMicros: 0,
});

const publicRefPattern = /^[a-z0-9][a-z0-9._-]{0,159}$/u;
const isoTimestampPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u;

const validatePublicRef = (value: string, field: string): void => {
  if (!publicRefPattern.test(value)) throw new CloudComputerCapacityValidationError(field);
};

const timestampMs = (value: string, field: string): number => {
  if (!isoTimestampPattern.test(value)) throw new CloudComputerCapacityValidationError(field);
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) throw new CloudComputerCapacityValidationError(field);
  return parsed;
};

const validateInteger = (
  value: number,
  field: string,
  minimum: number,
  maximum = Number.MAX_SAFE_INTEGER,
): void => {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new CloudComputerCapacityValidationError(field);
  }
};

const validateResources = (
  resources: CapacityResources,
  field: string,
  minimum: 0 | 1 = 0,
): void => {
  for (const key of resourceKeys) {
    validateInteger(resources[key], `${field}.${key}`, minimum);
  }
};

export const validateCloudComputerCapacityPolicy = (
  policy: CloudComputerCapacityPolicy,
): CloudComputerCapacityPolicy => {
  if (policy.schema !== CLOUD_COMPUTER_CAPACITY_POLICY_SCHEMA) {
    throw new CloudComputerCapacityValidationError("policy.schema");
  }
  validateInteger(policy.quotaFreshnessMs, "policy.quotaFreshnessMs", 1);
  validateInteger(policy.maxStartBatch, "policy.maxStartBatch", 1, 100);
  validateInteger(policy.jitterWindowMs, "policy.jitterWindowMs", 0, 60_000);
  if (policy.jitterWindowMs > policy.quotaFreshnessMs) {
    throw new CloudComputerCapacityValidationError("policy.jitterWindowMs");
  }
  validateInteger(
    policy.providerHeadroomBps,
    "policy.providerHeadroomBps",
    CLOUD_COMPUTER_MIN_PROVIDER_HEADROOM_BPS,
    9_999,
  );
  validateInteger(policy.batchActivePerConversation, "policy.batchActivePerConversation", 1, 30);
  validateResources(policy.privatePreviewCeiling, "policy.privatePreviewCeiling");
  validateResources(policy.protectedReserve, "policy.protectedReserve");
  validateResources(policy.limits.global, "policy.limits.global");
  validateResources(policy.limits.provider, "policy.limits.provider");
  validateResources(policy.limits.region, "policy.limits.region");
  validateResources(policy.limits.owner, "policy.limits.owner");
  validateResources(policy.limits.tenant, "policy.limits.tenant");
  validateResources(policy.limits.conversation, "policy.limits.conversation");
  for (const key of resourceKeys) {
    if (policy.protectedReserve[key] > policy.privatePreviewCeiling[key]) {
      throw new CloudComputerCapacityValidationError(`policy.protectedReserve.${key}`);
    }
  }
  return policy;
};

export const validateCloudComputerCapacityObservation = (
  observation: CloudComputerCapacityObservation,
): CloudComputerCapacityObservation => {
  if (observation.schema !== CLOUD_COMPUTER_CAPACITY_OBSERVATION_SCHEMA) {
    throw new CloudComputerCapacityValidationError("observation.schema");
  }
  validatePublicRef(observation.observationRef, "observation.observationRef");
  timestampMs(observation.observedAt, "observation.observedAt");
  validateResources(observation.providerQuota, "observation.providerQuota");
  validateResources(observation.allocatableCapacity, "observation.allocatableCapacity");
  validateResources(observation.drainAdjustedCapacity, "observation.drainAdjustedCapacity");
  validateResources(observation.budgetCapacity, "observation.budgetCapacity");
  return observation;
};

export const validateCloudComputerCapacityDemand = (
  demand: CloudComputerCapacityDemand,
): CloudComputerCapacityDemand => {
  if (demand.schema !== CLOUD_COMPUTER_CAPACITY_DEMAND_SCHEMA) {
    throw new CloudComputerCapacityValidationError("demand.schema");
  }
  validatePublicRef(demand.demandRef, "demand.demandRef");
  validatePublicRef(demand.ownerRef, "demand.ownerRef");
  validatePublicRef(demand.tenantRef, "demand.tenantRef");
  validatePublicRef(demand.conversationRef, "demand.conversationRef");
  validatePublicRef(demand.computerRef, "demand.computerRef");
  timestampMs(demand.requestedAt, "demand.requestedAt");
  if (!["standard", "strong", "batch"].includes(demand.runtimeClass)) {
    throw new CloudComputerCapacityValidationError("demand.runtimeClass");
  }
  if (!["normal", "cleanup", "replacement", "recovery"].includes(demand.priority)) {
    throw new CloudComputerCapacityValidationError("demand.priority");
  }
  if (typeof demand.createsLogicalComputer !== "boolean") {
    throw new CloudComputerCapacityValidationError("demand.createsLogicalComputer");
  }
  if (typeof demand.highFanOutBudgeted !== "boolean") {
    throw new CloudComputerCapacityValidationError("demand.highFanOutBudgeted");
  }
  validateResources(demand.resources, "demand.resources");
  if (demand.resources.concurrency !== 1 || demand.resources.startsPerMinute !== 1) {
    throw new CloudComputerCapacityValidationError("demand.resources");
  }
  validateInteger(demand.tenantWeight, "demand.tenantWeight", 1, 16);
  validateInteger(demand.conversationWeight, "demand.conversationWeight", 1, 16);
  validateInteger(demand.existingLogicalComputers, "demand.existingLogicalComputers", 0, 30);
  for (const runtimeClass of ["standard", "strong", "batch"] as const) {
    validateInteger(
      demand.existingActiveByRuntimeClass[runtimeClass],
      `demand.existingActiveByRuntimeClass.${runtimeClass}`,
      0,
      30,
    );
  }
  return demand;
};

const addResources = (left: CapacityResources, right: CapacityResources): CapacityResources => ({
  concurrency: left.concurrency + right.concurrency,
  cpuMillicores: left.cpuMillicores + right.cpuMillicores,
  memoryMiB: left.memoryMiB + right.memoryMiB,
  scratchMiB: left.scratchMiB + right.scratchMiB,
  durationMs: left.durationMs + right.durationMs,
  startsPerMinute: left.startsPerMinute + right.startsPerMinute,
  costMicros: left.costMicros + right.costMicros,
});

const fits = (
  used: CapacityResources,
  demand: CapacityResources,
  limit: CapacityResources,
): boolean => resourceKeys.every((key) => used[key] + demand[key] <= limit[key]);

const withProviderHeadroom = (quota: CapacityResources, headroomBps: number): CapacityResources => {
  const usableBps = 10_000 - headroomBps;
  return Object.fromEntries(
    resourceKeys.map((key) => [key, Math.floor((quota[key] * usableBps) / 10_000)]),
  ) as unknown as CapacityResources;
};

const minusReserve = (capacity: CapacityResources, reserve: CapacityResources): CapacityResources =>
  Object.fromEntries(
    resourceKeys.map((key) => [key, Math.max(0, capacity[key] - reserve[key])]),
  ) as unknown as CapacityResources;

const providerNormalCapacity = (
  quota: CapacityResources,
  reserve: CapacityResources,
  headroomBps: number,
): CapacityResources => {
  const headroomCapacity = withProviderHeadroom(quota, headroomBps);
  const reserveCapacity = minusReserve(quota, reserve);
  return Object.fromEntries(
    resourceKeys.map((key) => [key, Math.min(headroomCapacity[key], reserveCapacity[key])]),
  ) as unknown as CapacityResources;
};

const capacityBoundaries = (
  policy: CloudComputerCapacityPolicy,
  observation: CloudComputerCapacityObservation,
  priority: CloudComputerCapacityPriority,
): ReadonlyArray<readonly [CloudComputerCapacityReason, CapacityResources]> => {
  const protectedWork = priority !== "normal";
  const requiredReserve = Object.fromEntries(
    resourceKeys.map((key) => [
      key,
      Math.max(
        policy.protectedReserve[key],
        Math.ceil(
          (policy.privatePreviewCeiling[key] * CLOUD_COMPUTER_MIN_PROVIDER_HEADROOM_BPS) / 10_000,
        ),
      ),
    ]),
  ) as unknown as CapacityResources;
  return [
    [
      "private_preview_ceiling",
      protectedWork
        ? policy.privatePreviewCeiling
        : minusReserve(policy.privatePreviewCeiling, requiredReserve),
    ],
    [
      "provider_quota_exhausted",
      protectedWork
        ? observation.providerQuota
        : providerNormalCapacity(
            observation.providerQuota,
            requiredReserve,
            policy.providerHeadroomBps,
          ),
    ],
    [
      "allocatable_capacity",
      protectedWork
        ? observation.allocatableCapacity
        : minusReserve(observation.allocatableCapacity, requiredReserve),
    ],
    [
      "drain_capacity",
      protectedWork
        ? observation.drainAdjustedCapacity
        : minusReserve(observation.drainAdjustedCapacity, requiredReserve),
    ],
    [
      "budget",
      protectedWork
        ? observation.budgetCapacity
        : minusReserve(observation.budgetCapacity, requiredReserve),
    ],
  ];
};

const makeReceipt = (
  demand: CloudComputerCapacityDemand,
  observation: CloudComputerCapacityObservation,
  nowMs: number,
  status: CloudComputerCapacityReceipt["status"],
  reason: CloudComputerCapacityReason,
  policy: CloudComputerCapacityPolicy,
): CloudComputerCapacityReceipt => ({
  schema: CLOUD_COMPUTER_CAPACITY_RECEIPT_SCHEMA,
  demandRef: demand.demandRef,
  ownerRef: demand.ownerRef,
  tenantRef: demand.tenantRef,
  conversationRef: demand.conversationRef,
  computerRef: demand.computerRef,
  runtimeClass: demand.runtimeClass,
  priority: demand.priority,
  status,
  reason,
  observationRef: observation.observationRef,
  decidedAt: new Date(nowMs).toISOString(),
  startDeadlineAt:
    status === "admitted"
      ? startDeadlineAt(
          demand.requestedAt,
          policy.quotaFreshnessMs,
          deterministicStartJitterMs(demand.demandRef, policy.jitterWindowMs),
        )
      : null,
  retryAfterMs: status === "queued" ? Math.max(1_000, policy.quotaFreshnessMs) : null,
});

const runtimeClassLimit = (
  policy: CloudComputerCapacityPolicy,
  demand: CloudComputerCapacityDemand,
): number => {
  switch (demand.runtimeClass) {
    case "standard":
      return demand.highFanOutBudgeted
        ? CLOUD_COMPUTER_STANDARD_ACTIVE_HIGH_FAN_OUT
        : CLOUD_COMPUTER_STANDARD_ACTIVE_DEFAULT;
    case "strong":
      return CLOUD_COMPUTER_STRONG_ACTIVE_MAX;
    case "batch":
      return policy.batchActivePerConversation;
  }
};

export const evaluateCloudComputerAdmission = (
  input: Readonly<{
    policy: CloudComputerCapacityPolicy;
    observation: CloudComputerCapacityObservation;
    usage: CloudComputerCapacityUsage;
    demand: CloudComputerCapacityDemand;
    nowMs: number;
  }>,
): CloudComputerCapacityReceipt => {
  const policy = validateCloudComputerCapacityPolicy(input.policy);
  const observation = validateCloudComputerCapacityObservation(input.observation);
  const demand = validateCloudComputerCapacityDemand(input.demand);
  validateInteger(input.nowMs, "nowMs", 0);
  validateResources(input.usage.global, "usage.global");
  validateResources(input.usage.provider, "usage.provider");
  validateResources(input.usage.region, "usage.region");
  const tenantUsage = input.usage.tenants[demand.tenantRef] ?? zeroResources();
  const ownerUsage = input.usage.owners[demand.ownerRef] ?? zeroResources();
  const conversationUsage = input.usage.conversations[demand.conversationRef] ?? zeroResources();
  validateResources(tenantUsage, "usage.tenant");
  validateResources(ownerUsage, "usage.owner");
  validateResources(conversationUsage, "usage.conversation");

  if (
    demand.createsLogicalComputer &&
    demand.existingLogicalComputers >= CLOUD_COMPUTER_MAX_LOGICAL_PER_CONVERSATION
  ) {
    return makeReceipt(
      demand,
      observation,
      input.nowMs,
      "refused",
      "logical_computer_limit",
      policy,
    );
  }
  if (
    demand.existingActiveByRuntimeClass[demand.runtimeClass] >= runtimeClassLimit(policy, demand)
  ) {
    return makeReceipt(demand, observation, input.nowMs, "queued", "runtime_class_limit", policy);
  }

  const observedAtMs = timestampMs(observation.observedAt, "observation.observedAt");
  const quotaStale =
    input.nowMs - observedAtMs > policy.quotaFreshnessMs ||
    observedAtMs - input.nowMs > policy.quotaFreshnessMs;
  if (demand.priority === "normal" && quotaStale) {
    return makeReceipt(demand, observation, input.nowMs, "queued", "provider_quota_stale", policy);
  }

  for (const [reason, capacity] of capacityBoundaries(policy, observation, demand.priority)) {
    if (!fits(input.usage.region, demand.resources, capacity)) {
      return makeReceipt(demand, observation, input.nowMs, "queued", reason, policy);
    }
  }

  const hierarchicalLimits: ReadonlyArray<
    readonly [CloudComputerCapacityReason, CapacityResources, CapacityResources]
  > = [
    ["global_limit", input.usage.global, policy.limits.global],
    ["provider_limit", input.usage.provider, policy.limits.provider],
    ["region_limit", input.usage.region, policy.limits.region],
    ["owner_limit", ownerUsage, policy.limits.owner],
    ["tenant_limit", tenantUsage, policy.limits.tenant],
    ["conversation_limit", conversationUsage, policy.limits.conversation],
  ];
  for (const [reason, usage, limit] of hierarchicalLimits) {
    if (!fits(usage, demand.resources, limit)) {
      const intrinsicFailure = !fits(zeroResources(), demand.resources, limit);
      return makeReceipt(
        demand,
        observation,
        input.nowMs,
        intrinsicFailure ? "refused" : "queued",
        reason,
        policy,
      );
    }
  }

  return makeReceipt(demand, observation, input.nowMs, "admitted", "admitted", policy);
};

type MutableDemandState = {
  demand: CloudComputerCapacityDemand;
  ordinal: number;
};

const priorityRank: Readonly<Record<CloudComputerCapacityPriority, number>> = {
  cleanup: 0,
  recovery: 1,
  replacement: 2,
  normal: 3,
};

const weightedFairOrder = (
  demands: ReadonlyArray<CloudComputerCapacityDemand>,
): ReadonlyArray<CloudComputerCapacityDemand> => {
  const pending = demands.map((demand, ordinal): MutableDemandState => ({ demand, ordinal }));
  const tenantService = new Map<string, number>();
  const conversationService = new Map<string, number>();
  const ordered: Array<CloudComputerCapacityDemand> = [];

  while (pending.length > 0) {
    pending.sort((left, right) => {
      const leftTenantScore =
        (tenantService.get(left.demand.tenantRef) ?? 0) / left.demand.tenantWeight;
      const rightTenantScore =
        (tenantService.get(right.demand.tenantRef) ?? 0) / right.demand.tenantWeight;
      if (leftTenantScore !== rightTenantScore) return leftTenantScore - rightTenantScore;
      const tenantOrder = left.demand.tenantRef.localeCompare(right.demand.tenantRef);
      if (tenantOrder !== 0) return tenantOrder;

      const leftConversationScore =
        (conversationService.get(left.demand.conversationRef) ?? 0) /
        left.demand.conversationWeight;
      const rightConversationScore =
        (conversationService.get(right.demand.conversationRef) ?? 0) /
        right.demand.conversationWeight;
      if (leftConversationScore !== rightConversationScore) {
        return leftConversationScore - rightConversationScore;
      }
      const conversationOrder = left.demand.conversationRef.localeCompare(
        right.demand.conversationRef,
      );
      if (conversationOrder !== 0) return conversationOrder;
      const priorityOrder =
        priorityRank[left.demand.priority] - priorityRank[right.demand.priority];
      if (priorityOrder !== 0) return priorityOrder;
      const requestedOrder = left.demand.requestedAt.localeCompare(right.demand.requestedAt);
      if (requestedOrder !== 0) return requestedOrder;
      const refOrder = left.demand.demandRef.localeCompare(right.demand.demandRef);
      return refOrder !== 0 ? refOrder : left.ordinal - right.ordinal;
    });
    const next = pending.shift();
    if (next === undefined) break;
    ordered.push(next.demand);
    tenantService.set(next.demand.tenantRef, (tenantService.get(next.demand.tenantRef) ?? 0) + 1);
    conversationService.set(
      next.demand.conversationRef,
      (conversationService.get(next.demand.conversationRef) ?? 0) + 1,
    );
  }
  return ordered;
};

export const scheduleCloudComputerCapacity = (
  input: Readonly<{
    policy: CloudComputerCapacityPolicy;
    observation: CloudComputerCapacityObservation;
    usage: CloudComputerCapacityUsage;
    demands: ReadonlyArray<CloudComputerCapacityDemand>;
    nowMs: number;
    preserveOrder?: boolean | undefined;
  }>,
): CloudComputerCapacitySchedule => {
  const policy = validateCloudComputerCapacityPolicy(input.policy);
  validateCloudComputerCapacityObservation(input.observation);
  const validatedDemands = input.demands.map(validateCloudComputerCapacityDemand);
  const ordered = input.preserveOrder ? validatedDemands : weightedFairOrder(validatedDemands);
  let global = { ...input.usage.global };
  let provider = { ...input.usage.provider };
  let region = { ...input.usage.region };
  const tenants: Record<string, CapacityResources> = { ...input.usage.tenants };
  const owners: Record<string, CapacityResources> = { ...input.usage.owners };
  const conversations: Record<string, CapacityResources> = { ...input.usage.conversations };
  const activeByConversation = new Map<string, Record<CloudComputerRuntimeClass, number>>();
  const logicalByConversation = new Map<string, number>();
  const admitted: Array<CloudComputerCapacityReceipt> = [];
  const queued: Array<CloudComputerCapacityReceipt> = [];
  const refused: Array<CloudComputerCapacityReceipt> = [];

  for (const originalDemand of ordered) {
    const active = activeByConversation.get(originalDemand.conversationRef) ?? {
      ...originalDemand.existingActiveByRuntimeClass,
    };
    const logical =
      logicalByConversation.get(originalDemand.conversationRef) ??
      originalDemand.existingLogicalComputers;
    const demand: CloudComputerCapacityDemand = {
      ...originalDemand,
      existingActiveByRuntimeClass: active,
      existingLogicalComputers: logical,
    };
    if (admitted.length >= policy.maxStartBatch) {
      queued.push(
        makeReceipt(demand, input.observation, input.nowMs, "queued", "start_batch_limit", policy),
      );
      continue;
    }
    const receipt = evaluateCloudComputerAdmission({
      policy,
      observation: input.observation,
      usage: { provider, region, global, owners, tenants, conversations },
      demand,
      nowMs: input.nowMs,
    });
    if (receipt.status === "admitted") {
      admitted.push(receipt);
      global = addResources(global, demand.resources);
      provider = addResources(provider, demand.resources);
      region = addResources(region, demand.resources);
      owners[demand.ownerRef] = addResources(
        owners[demand.ownerRef] ?? zeroResources(),
        demand.resources,
      );
      tenants[demand.tenantRef] = addResources(
        tenants[demand.tenantRef] ?? zeroResources(),
        demand.resources,
      );
      conversations[demand.conversationRef] = addResources(
        conversations[demand.conversationRef] ?? zeroResources(),
        demand.resources,
      );
      active[demand.runtimeClass] += 1;
      activeByConversation.set(demand.conversationRef, active);
      if (demand.createsLogicalComputer) {
        logicalByConversation.set(demand.conversationRef, logical + 1);
      }
    } else if (receipt.status === "queued") {
      queued.push(receipt);
    } else {
      refused.push(receipt);
    }
  }

  return {
    admitted,
    queued,
    refused,
    nextUsage: { provider, region, global, owners, tenants, conversations },
  };
};

export const deterministicStartJitterMs = (publicRef: string, windowMs: number): number => {
  validatePublicRef(publicRef, "publicRef");
  validateInteger(windowMs, "windowMs", 0, 60_000);
  if (windowMs === 0) return 0;
  let hash = 2_166_136_261;
  for (let index = 0; index < publicRef.length; index += 1) {
    hash ^= publicRef.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619) >>> 0;
  }
  return hash % (windowMs + 1);
};

export const startDeadlineAt = (
  requestedAt: string,
  startWindowMs: number,
  jitterMs: number,
): string => {
  const requestedAtMs = timestampMs(requestedAt, "requestedAt");
  validateInteger(startWindowMs, "startWindowMs", 1);
  validateInteger(jitterMs, "jitterMs", 0, startWindowMs);
  const deadline = requestedAtMs + startWindowMs + jitterMs;
  if (!Number.isSafeInteger(deadline))
    throw new CloudComputerCapacityValidationError("startDeadlineAt");
  return new Date(deadline).toISOString();
};

export interface CloudComputerCapacityAdapter {
  readonly provider: CloudComputerCapacityProvider;
  readonly observe: () => Promise<CloudComputerCapacityObservation>;
}

const capacityAdapter = (
  provider: CloudComputerCapacityProvider,
  poll: () => Promise<CloudComputerCapacityObservation>,
): CloudComputerCapacityAdapter => ({
  provider,
  observe: async () => validateCloudComputerCapacityObservation(await poll()),
});

export const gceCapacityAdapter = (
  poll: () => Promise<CloudComputerCapacityObservation>,
): CloudComputerCapacityAdapter => capacityAdapter("gce", poll);

export const gkeAgentSandboxCapacityAdapter = (
  poll: () => Promise<CloudComputerCapacityObservation>,
): CloudComputerCapacityAdapter => capacityAdapter("gke_agent_sandbox", poll);

export const firecrackerCapacityAdapter = (
  poll: () => Promise<CloudComputerCapacityObservation>,
): CloudComputerCapacityAdapter => capacityAdapter("firecracker", poll);

export const cloudRunCapacityAdapter = (
  poll: () => Promise<CloudComputerCapacityObservation>,
): CloudComputerCapacityAdapter => capacityAdapter("cloud_run", poll);
