import assert from "node:assert/strict";
import test from "node:test";
import {
  buildLiveKitCostCapture,
  classifyLiveKitBillingRow,
  LIVEKIT_COST_CATEGORIES,
  projectLiveKitBudget,
} from "./livekit-cost-collector-lib.mjs";

const sourceBaseRevision = "a".repeat(40);
const deployedRevision = "b".repeat(40);

const row = (overrides = {}) => ({
  usageDate: "2026-07-31",
  projectId: "openagentsgemini",
  serviceId: "service-compute",
  serviceDescription: "Compute Engine",
  skuId: "sku-compute",
  skuDescription: "Instance core running in Americas",
  resourceName: "projects/openagentsgemini/zones/us-central1-a/instances/gke-oa-livekit-prod-sfu",
  labels: JSON.stringify([{ key: "service", value: "livekit" }]),
  systemLabels: "{}",
  grossCostUsd: "10.5",
  creditUsd: "-2.5",
  ...overrides,
});

const budget = (overrides = {}) => ({
  displayName: "LiveKit production",
  amount: { specifiedAmount: { currencyCode: "USD", units: "5000" } },
  budgetFilter: {
    projects: ["projects/157437760789"],
    labels: { service: { values: ["livekit"] } },
  },
  thresholdRules: [
    { thresholdPercent: 0.5, spendBasis: "CURRENT_SPEND" },
    { thresholdPercent: 0.8, spendBasis: "CURRENT_SPEND" },
    { thresholdPercent: 1, spendBasis: "FORECASTED_SPEND" },
  ],
  allUpdatesRule: { monitoringNotificationChannels: ["channel-ref"] },
  ...overrides,
});

test("classifies exact LiveKit resources and conservatively includes unlabelled networking", () => {
  assert.equal(classifyLiveKitBillingRow(row()), "sfu_compute");
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        resourceName:
          "projects/openagentsgemini/zones/us-central1-a/instances/gke-oa-livekit-prod-app",
      }),
    ),
    "worker_compute",
  );
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        serviceDescription: "Kubernetes Engine",
        skuDescription: "Kubernetes Engine cluster management fee",
        resourceName: "projects/openagentsgemini/locations/us-central1/clusters/oa-livekit-prod",
      }),
    ),
    "gke_control_plane",
  );
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        serviceDescription: "Memorystore for Redis",
        skuDescription: "Redis standard capacity",
        resourceName: "projects/openagentsgemini/locations/us-central1/instances/oa-livekit-redis",
      }),
    ),
    "redis",
  );
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        skuDescription: "Network Internet Data Transfer Out",
        resourceName: null,
        labels: "[]",
      }),
    ),
    "load_balancing_networking",
  );
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        serviceDescription: "Cloud Logging",
        skuDescription: "Log bytes ingested",
        resourceName: "projects/openagentsgemini/locations/us-central1/clusters/oa-livekit-prod",
      }),
    ),
    "observability",
  );
});

test("ignores unrelated rows and fails closed on an identified unknown LiveKit SKU", () => {
  assert.equal(
    classifyLiveKitBillingRow(
      row({
        resourceName: "projects/openagentsgemini/zones/us-central1-a/instances/unrelated",
        labels: "[]",
      }),
    ),
    null,
  );
  assert.throws(
    () =>
      classifyLiveKitBillingRow(
        row({
          serviceDescription: "Unknown Google Service",
          skuDescription: "Unknown SKU",
          resourceName: "projects/openagentsgemini/locations/us-central1/oa-livekit-prod",
        }),
      ),
    /cannot be categorized/u,
  );
});

test("projects only the exact active budget policy", () => {
  assert.deepEqual(projectLiveKitBudget([budget()]), {
    active: true,
    currency: "USD",
    thresholds: [
      { percent: 0.5, basis: "CURRENT_SPEND" },
      { percent: 0.8, basis: "CURRENT_SPEND" },
      { percent: 1, basis: "FORECASTED_SPEND" },
    ],
    notificationChannelCount: 1,
    filterIncludesProject: true,
    filterIncludesLivekitLabel: true,
  });
  assert.throws(() => projectLiveKitBudget([]), /exactly one/u);
  assert.throws(
    () => projectLiveKitBudget([budget({ thresholdRules: [] })]),
    /thresholds drifted/u,
  );
});

test("builds a closed gross-cost capture and materializes observed zero categories", () => {
  const categories = [
    row(),
    row({
      resourceName:
        "projects/openagentsgemini/zones/us-central1-a/instances/gke-oa-livekit-prod-app",
    }),
    row({
      serviceDescription: "Kubernetes Engine",
      skuDescription: "GKE cluster management fee",
      resourceName: "projects/openagentsgemini/locations/us-central1/clusters/oa-livekit-prod",
    }),
    row({
      serviceDescription: "Memorystore for Redis",
      skuDescription: "Redis standard capacity",
      resourceName: "projects/openagentsgemini/locations/us-central1/instances/oa-livekit-redis",
    }),
    row({ skuDescription: "Network Load Balancing forwarding rule", resourceName: null }),
    row({
      serviceDescription: "Cloud Monitoring",
      skuDescription: "Metric bytes ingested",
      resourceName: "projects/openagentsgemini/locations/us-central1/clusters/oa-livekit-prod",
    }),
  ];
  const capture = buildLiveKitCostCapture({
    sourceBaseRevision,
    deployedRevision,
    observedAt: "2026-08-01T00:00:00.000Z",
    fixedFloorMonthlyUsd: 1_500,
    exportRows: categories,
    budgets: [budget()],
    windowStart: "2026-07-31",
    windowEnd: "2026-08-02",
  });
  assert.equal(capture.billingRows.length, 2 * LIVEKIT_COST_CATEGORIES.length);
  assert.equal(
    capture.billingRows
      .filter((entry) => entry.usageDate === "2026-08-01")
      .every((entry) => entry.grossCostUsd === 0 && entry.creditUsd === 0),
    true,
  );
  assert.equal(
    capture.billingRows
      .filter((entry) => entry.usageDate === "2026-07-31")
      .reduce((total, entry) => total + entry.grossCostUsd, 0),
    63,
  );
  assert.equal(
    capture.billingRows
      .filter((entry) => entry.usageDate === "2026-07-31")
      .reduce((total, entry) => total + entry.creditUsd, 0),
    -15,
  );
  assert.equal(capture.forecastMonthlyGrossUsd, 1_500);
});

test("refuses empty export evidence and a floor below the admitted planning floor", () => {
  const common = {
    sourceBaseRevision,
    deployedRevision,
    observedAt: "2026-08-01T00:00:00.000Z",
    fixedFloorMonthlyUsd: 1_500,
    budgets: [budget()],
    windowStart: "2026-07-31",
    windowEnd: "2026-08-01",
  };
  assert.throws(() => buildLiveKitCostCapture({ ...common, exportRows: [] }), /no attributable/u);
  assert.throws(
    () =>
      buildLiveKitCostCapture({
        ...common,
        exportRows: [row()],
        fixedFloorMonthlyUsd: 1_499,
      }),
    /below the admitted planning floor/u,
  );
});
