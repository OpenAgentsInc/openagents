import { Effect, Layer } from "effect";

import { ApiTransportLive } from "../controlPlane/apiTransport.js";
import { ControlPlaneLive } from "../controlPlane/live.js";
import { makeInMemoryControlPlaneHarness } from "../controlPlane/inMemory.js";
import { smokePaywalls } from "../fixtures/smokePaywalls.js";
import { OpsRuntimeConfigLive } from "../runtime/config.js";

import { ingestSettlementEvents, type SettlementIngestEvent } from "./ingestSettlements.js";

export type SettlementSmokeMode = "mock" | "api";

const smokeSettlementEvents: ReadonlyArray<SettlementIngestEvent> = [
  {
    kind: "invoice_lifecycle",
    occurredAtMs: 1_735_000_000_000,
    invoiceId: "inv_smoke_1",
    paywallId: "pw_weather",
    ownerId: "owner_weather",
    amountMsats: 2_500,
    status: "open",
    paymentHash: "hash_smoke_1",
    paymentRequest: "lnbc1smokepaymentrequest1",
    requestId: "req_smoke_1",
  },
  {
    kind: "settlement",
    occurredAtMs: 1_735_000_000_100,
    settlementId: "set_smoke_1",
    paywallId: "pw_weather",
    ownerId: "owner_weather",
    invoiceId: "inv_smoke_1",
    amountMsats: 2_500,
    paymentHash: "hash_smoke_1",
    paymentProofType: "lightning_preimage",
    paymentProofValue: "a".repeat(64),
    requestId: "req_smoke_1",
    taskId: "task_smoke_1",
    routeId: "route_weather",
    metadata: { source: "smoke:settlement" },
  },
  {
    kind: "settlement",
    occurredAtMs: 1_735_000_000_101,
    settlementId: "set_smoke_1",
    paywallId: "pw_weather",
    ownerId: "owner_weather",
    invoiceId: "inv_smoke_1",
    amountMsats: 2_500,
    paymentHash: "hash_smoke_1",
    paymentProofType: "lightning_preimage",
    paymentProofValue: "a".repeat(64),
    requestId: "req_smoke_1",
    taskId: "task_smoke_1",
    routeId: "route_weather",
    metadata: { source: "smoke:settlement:duplicate" },
  },
  {
    kind: "invoice_lifecycle",
    occurredAtMs: 1_735_000_000_200,
    invoiceId: "inv_smoke_1",
    paywallId: "pw_weather",
    ownerId: "owner_weather",
    amountMsats: 2_500,
    status: "open",
    requestId: "req_smoke_reordered_open",
  },
  {
    kind: "invoice_lifecycle",
    occurredAtMs: 1_735_000_000_300,
    invoiceId: "inv_smoke_2",
    paywallId: "pw_news",
    ownerId: "owner_news",
    amountMsats: 1_800,
    status: "expired",
    requestId: "req_smoke_2",
  },
];

const runWithLayer = (layer: Layer.Layer<any, any, never>) =>
  ingestSettlementEvents(smokeSettlementEvents).pipe(Effect.provide(layer));

const runMockSmoke = () => {
  const harness = makeInMemoryControlPlaneHarness({ paywalls: smokePaywalls });
  return runWithLayer(harness.layer);
};

const runApiSmoke = () => {
  const controlPlaneLayer = ControlPlaneLive.pipe(
    Layer.provideMerge(ApiTransportLive),
    Layer.provideMerge(OpsRuntimeConfigLive),
  );
  return runWithLayer(controlPlaneLayer);
};

export const runSettlementSmoke = (input?: {
  readonly mode?: SettlementSmokeMode;
}) => {
  const mode = input?.mode ?? "mock";
  if (mode === "api") return runApiSmoke();
  return runMockSmoke();
};
