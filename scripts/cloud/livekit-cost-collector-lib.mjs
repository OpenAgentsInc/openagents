export const LIVEKIT_COST_CAPTURE_SCHEMA = "openagents.livekit_cost_capture.v1";
export const LIVEKIT_COST_CATEGORIES = Object.freeze([
  "gke_control_plane",
  "sfu_compute",
  "worker_compute",
  "redis",
  "load_balancing_networking",
  "observability",
]);

const PROJECT_ID = "openagentsgemini";
const PROJECT_NUMBER = "157437760789";
const LIVEKIT_RESOURCE =
  /(?:^|[/_.-])(?:oa-livekit-prod|oa-livekit-redis|livekit-server|sarah-livekit-agent)(?:$|[/_.-])/iu;
const NETWORK_SKU =
  /(?:load balanc|forwarding rule|ip address|network egress|network internet|data transfer|cloud nat|inter-region|inter zone|turn)/iu;
const CONTROL_PLANE_SKU = /(?:cluster management|gke cluster|kubernetes engine cluster)/iu;

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const exactKeys = (value, required, optional, label) => {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) assert(allowed.has(key), `${label} has unsupported ${key}`);
  for (const key of required) assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
};

const money = (value, label) => {
  const number = typeof value === "string" && value.trim() !== "" ? Number(value) : value;
  assert(
    typeof number === "number" && Number.isFinite(number) && Math.abs(number) <= 1_000_000,
    `${label} is invalid`,
  );
  return number;
};

const labelMap = (value, label) => {
  let decoded = value;
  if (typeof decoded === "string") {
    try {
      decoded = JSON.parse(decoded);
    } catch (error) {
      throw new Error(`${label} is not JSON`, { cause: error });
    }
  }
  if (decoded === null || decoded === undefined) return new Map();
  if (Array.isArray(decoded)) {
    const entries = decoded.map((entry, index) => {
      exactKeys(entry, ["key", "value"], [], `${label}[${index}]`);
      assert(
        typeof entry.key === "string" && typeof entry.value === "string",
        `${label} is invalid`,
      );
      return [entry.key, entry.value];
    });
    assert(new Set(entries.map(([key]) => key)).size === entries.length, `${label} repeats a key`);
    return new Map(entries);
  }
  assert(typeof decoded === "object", `${label} must be an object or key/value array`);
  return new Map(
    Object.entries(decoded).map(([key, raw]) => [
      key,
      typeof raw === "string" ? raw : JSON.stringify(raw),
    ]),
  );
};

const labelValue = (labels, ...keys) => {
  for (const key of keys) {
    const value = labels.get(key);
    if (value !== undefined) return value;
  }
  return "";
};

const hasLiveKitIdentity = (row, labels, systemLabels) =>
  labelValue(labels, "service") === "livekit" ||
  labelValue(labels, "goog-k8s-cluster-name", "k8s-cluster-name") === "oa-livekit-prod" ||
  labelValue(systemLabels, "compute.googleapis.com/resource_name").includes("oa-livekit-prod") ||
  LIVEKIT_RESOURCE.test(row.resourceName ?? "");

export const classifyLiveKitBillingRow = (row) => {
  exactKeys(
    row,
    [
      "usageDate",
      "projectId",
      "serviceId",
      "serviceDescription",
      "skuId",
      "skuDescription",
      "resourceName",
      "labels",
      "systemLabels",
      "grossCostUsd",
      "creditUsd",
    ],
    [],
    "billing row",
  );
  assert(/^\d{4}-\d{2}-\d{2}$/u.test(row.usageDate), "billing row usageDate is invalid");
  assert(row.projectId === PROJECT_ID, "billing row is outside the admitted project");
  for (const key of ["serviceId", "serviceDescription", "skuId", "skuDescription"]) {
    assert(typeof row[key] === "string" && row[key].length > 0, `billing row ${key} is invalid`);
  }
  assert(
    row.resourceName === null || typeof row.resourceName === "string",
    "billing row resourceName is invalid",
  );
  const labels = labelMap(row.labels, "billing row labels");
  const systemLabels = labelMap(row.systemLabels, "billing row systemLabels");
  const identified = hasLiveKitIdentity(row, labels, systemLabels);
  const service = row.serviceDescription;
  const sku = row.skuDescription;

  if (/Cloud Logging|Cloud Monitoring|Stackdriver/iu.test(service)) {
    // Logging and Monitoring export at project scope without retaining the
    // monitored-resource labels. Include all matching project rows so the
    // collector can overstate but never silently understate this category.
    return "observability";
  }
  if (/Memorystore|Cloud Memorystore|Redis/iu.test(service) || /Redis/iu.test(sku)) {
    if (!identified) return null;
    return "redis";
  }
  if (/Kubernetes Engine/iu.test(service) && CONTROL_PLANE_SKU.test(sku)) {
    if (!identified) return null;
    return "gke_control_plane";
  }

  const nodePool = labelValue(
    labels,
    "goog-k8s-node-pool-name",
    "k8s-node-pool-name",
    "goog-k8s-node-pool",
  );
  const resourceIdentity = `${row.resourceName ?? ""} ${labelValue(
    systemLabels,
    "compute.googleapis.com/resource_name",
  )}`;
  if (nodePool === "oa-livekit-prod-sfu" || /oa-livekit-prod-sfu/iu.test(resourceIdentity)) {
    return "sfu_compute";
  }
  if (nodePool === "oa-livekit-prod-app" || /oa-livekit-prod-app/iu.test(resourceIdentity)) {
    return "worker_compute";
  }

  if (NETWORK_SKU.test(sku)) {
    // Kubernetes-created frontends and network egress are often unlabelled in
    // the billing export. Including every matching project row is a bounded,
    // conservative over-attribution. It cannot make observed cost look lower.
    return "load_balancing_networking";
  }
  if (!identified) return null;
  throw new Error(
    `identified LiveKit billing row cannot be categorized (${row.serviceId}/${row.skuId})`,
  );
};

const normalizeBudgetLabels = (labels) => {
  if (!labels || typeof labels !== "object" || Array.isArray(labels)) return [];
  const service = labels.service;
  if (Array.isArray(service)) return service;
  if (service && typeof service === "object" && Array.isArray(service.values)) {
    return service.values;
  }
  return typeof service === "string" ? [service] : [];
};

export const projectLiveKitBudget = (budgets) => {
  assert(Array.isArray(budgets), "budget inventory must be an array");
  const matching = budgets.filter((budget) => budget?.displayName === "LiveKit production");
  assert(matching.length === 1, "exactly one LiveKit production budget is required");
  const budget = matching[0];
  const filter = budget.budgetFilter;
  assert(filter && typeof filter === "object", "LiveKit budget filter is missing");
  const thresholds = budget.thresholdRules?.map((rule) => ({
    percent: Number(rule.thresholdPercent),
    basis: rule.spendBasis,
  }));
  const expectedThresholds = [
    { percent: 0.5, basis: "CURRENT_SPEND" },
    { percent: 0.8, basis: "CURRENT_SPEND" },
    { percent: 1, basis: "FORECASTED_SPEND" },
  ];
  assert(
    JSON.stringify(thresholds) === JSON.stringify(expectedThresholds),
    "LiveKit budget thresholds drifted",
  );
  const currency = budget.amount?.specifiedAmount?.currencyCode;
  assert(currency === "USD", "LiveKit budget currency drifted");
  const projects = Array.isArray(filter.projects) ? filter.projects : [];
  const channels = budget.allUpdatesRule?.monitoringNotificationChannels;
  return {
    active: true,
    currency,
    thresholds,
    notificationChannelCount: Array.isArray(channels) ? channels.length : 0,
    filterIncludesProject: projects.includes(`projects/${PROJECT_NUMBER}`),
    filterIncludesLivekitLabel: normalizeBudgetLabels(filter.labels).includes("livekit"),
  };
};

export const buildLiveKitCostCapture = ({
  sourceBaseRevision,
  deployedRevision,
  observedAt,
  fixedFloorMonthlyUsd,
  exportRows,
  budgets,
  windowStart,
  windowEnd,
}) => {
  assert(/^[0-9a-f]{40}$/u.test(sourceBaseRevision), "source revision must be full Git SHA");
  assert(/^[0-9a-f]{40}$/u.test(deployedRevision), "deployed revision must be full Git SHA");
  assert(Number.isFinite(Date.parse(observedAt)), "cost observation timestamp is invalid");
  assert(/^\d{4}-\d{2}-\d{2}$/u.test(windowStart), "cost window start is invalid");
  assert(/^\d{4}-\d{2}-\d{2}$/u.test(windowEnd), "cost window end is invalid");
  const start = Date.parse(`${windowStart}T00:00:00Z`);
  const end = Date.parse(`${windowEnd}T00:00:00Z`);
  assert(
    end > start && end - start <= 31 * 24 * 60 * 60_000,
    "cost window must be 1 through 31 days",
  );
  assert(Array.isArray(exportRows) && exportRows.length <= 100_000, "billing export is too large");
  const aggregates = new Map();
  for (const row of exportRows) {
    const usageAt = Date.parse(`${row.usageDate}T00:00:00Z`);
    assert(usageAt >= start && usageAt < end, "billing row is outside the requested window");
    const category = classifyLiveKitBillingRow(row);
    if (category === null) continue;
    const key = `${row.usageDate}\0${category}`;
    const current = aggregates.get(key) ?? { grossCostUsd: 0, creditUsd: 0 };
    current.grossCostUsd += money(row.grossCostUsd, "gross cost");
    current.creditUsd += money(row.creditUsd, "credit");
    assert(current.grossCostUsd >= 0, "gross cost cannot be negative");
    assert(current.creditUsd <= 0, "credit cannot be positive");
    aggregates.set(key, current);
  }
  assert(aggregates.size > 0, "billing export contains no attributable LiveKit rows");

  const dates = [];
  for (let at = start; at < end; at += 24 * 60 * 60_000) {
    dates.push(new Date(at).toISOString().slice(0, 10));
  }
  const billingRows = dates.flatMap((usageDate) =>
    LIVEKIT_COST_CATEGORIES.map((serviceCategory) => {
      const values = aggregates.get(`${usageDate}\0${serviceCategory}`) ?? {
        grossCostUsd: 0,
        creditUsd: 0,
      };
      return { usageDate, serviceCategory, ...values };
    }),
  );
  const fixedFloor = money(fixedFloorMonthlyUsd, "fixed monthly floor");
  const observedGrossCost = billingRows.reduce((total, row) => total + row.grossCostUsd, 0);
  const forecast = Math.max(fixedFloor, (observedGrossCost / dates.length) * (365.25 / 12));
  assert(fixedFloor >= 1_500, "fixed monthly floor is below the admitted planning floor");
  const budget = projectLiveKitBudget(budgets);
  assert(
    budget.active === true &&
      budget.notificationChannelCount > 0 &&
      budget.filterIncludesProject === true &&
      budget.filterIncludesLivekitLabel === true,
    "LiveKit budget is not active with the admitted scope and notification policy",
  );
  return {
    schemaVersion: LIVEKIT_COST_CAPTURE_SCHEMA,
    sourceBaseRevision,
    deployedRevision,
    observedAt,
    fixedFloorMonthlyUsd: fixedFloor,
    forecastMonthlyGrossUsd: forecast,
    billingRows,
    budget,
  };
};
