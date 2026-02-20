import * as Fs from "node:fs/promises";
import * as Path from "node:path";

import {
  L402ObservabilityFieldKeys,
  type L402ObservabilityFieldKey,
  type L402ObservabilityRecord,
} from "@openagentsinc/lightning-effect";
import { Effect } from "effect";

import { smokePaywalls } from "../fixtures/smokePaywalls.js";
import { runObservabilitySmoke } from "./smokeObservability.js";
import { runSecuritySmoke } from "./securityControls.js";
import { runSettlementSmoke } from "./smokeSettlement.js";
import { runStagingSmoke, type StagingSmokeMode } from "./smokeStaging.js";

export type FullFlowSmokeMode = StagingSmokeMode;

type FullFlowEvent = Readonly<{
  ts: number;
  stage: string;
  requestId: string;
  status: "ok" | "failed";
  details?: unknown;
}>;

type LocalNodeFlowArtifact = Readonly<{
  generatedAtMs?: number;
  flows?: ReadonlyArray<{
    flow?: string;
    taskId?: string;
    createRequestId?: string;
    proofReference?: string;
    transitionRequestIds?: ReadonlyArray<string>;
    observabilityRecords?: ReadonlyArray<unknown>;
  }>;
}>;

export type FullFlowSmokeSummary = Readonly<{
  ok: boolean;
  mode: FullFlowSmokeMode;
  requestId: string;
  executionPath: "hosted-node";
  paywallCreation: {
    ok: boolean;
    paywallId: string;
    ownerId: string;
    routeId: string;
  };
  gatewayReconcile: {
    requestId: string;
    deploymentId: string;
    configHash: string;
    deploymentStatus: string;
    challengeOk: boolean;
    proxyOk: boolean;
    healthOk: boolean;
  };
  paidRequest: {
    status: "paid";
    requestId: string | null;
    taskId: string | null;
    routeId: string | null;
    settlementId: string;
    amountMsats: number;
    paymentProofRef: string;
  };
  policyDeniedRequest: {
    status: "denied";
    requestId: string;
    denyReasonCode: string;
    denyReason: string;
    source: "global_pause";
  };
  settlementCorrelation: {
    processed: number;
    settlementIds: ReadonlyArray<string>;
    paymentProofRefs: ReadonlyArray<string>;
  };
  observability: {
    recordCount: number;
    requiredFieldKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    missingFieldKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    correlation: {
      requestIds: ReadonlyArray<string>;
      paywallIds: ReadonlyArray<string>;
      taskIds: ReadonlyArray<string>;
      paymentProofRefs: ReadonlyArray<string>;
    };
  };
  parity: {
    requiredKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    hostedKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    hostedMissingKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    localArtifactPath: string;
    localArtifactPresent: boolean;
    localKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    localMissingKeys: ReadonlyArray<L402ObservabilityFieldKey>;
    sharedKeys: ReadonlyArray<L402ObservabilityFieldKey>;
  };
  coverage: {
    totalChecks: number;
    passedChecks: number;
    failedChecks: ReadonlyArray<string>;
  };
  artifacts: {
    artifactDir: string;
    summaryPath: string;
    eventsPath: string;
    generatedAtMs: number;
  };
}>;

const requiredParityKeys = [
  "executionPath",
  "requestId",
  "taskId",
  "paymentProofRef",
] as const satisfies ReadonlyArray<L402ObservabilityFieldKey>;

const sanitizePathSegment = (value: string): string =>
  value.replaceAll(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 120);

const defaultArtifactDir = (requestId: string): string =>
  Path.resolve(process.cwd(), "../../output/lightning-ops/full-flow", sanitizePathSegment(requestId));

const defaultLocalArtifactPath = (): string =>
  Path.resolve(process.cwd(), "../../output/l402-local-node-smoke-artifact.json");

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const parseLocalArtifact = (raw: unknown): LocalNodeFlowArtifact | null => {
  if (!isRecord(raw)) return null;
  return raw as LocalNodeFlowArtifact;
};

const collectPresentKeys = (records: ReadonlyArray<unknown>): ReadonlySet<string> => {
  const present = new Set<string>();
  for (const record of records) {
    if (!isRecord(record)) continue;
    for (const key of Object.keys(record)) {
      present.add(key);
    }
  }
  return present;
};

const toSortedFieldKeys = (
  keys: ReadonlySet<string>,
  allowed: ReadonlyArray<L402ObservabilityFieldKey>,
): ReadonlyArray<L402ObservabilityFieldKey> =>
  [...keys]
    .filter((key): key is L402ObservabilityFieldKey =>
      allowed.includes(key as L402ObservabilityFieldKey),
    )
    .sort((a, b) => a.localeCompare(b));

const parityMissingKeys = (
  present: ReadonlySet<string>,
  required: ReadonlyArray<L402ObservabilityFieldKey>,
): ReadonlyArray<L402ObservabilityFieldKey> =>
  required.filter((key) => !present.has(key));

const asNonEmptyStringOrNull = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null;

const addEvent = (
  events: Array<FullFlowEvent>,
  input: Omit<FullFlowEvent, "ts">,
): void => {
  events.push({
    ts: Date.now(),
    ...input,
  });
};

const writeTextFile = (path: string, content: string) =>
  Effect.tryPromise({
    try: async () => {
      await Fs.mkdir(Path.dirname(path), { recursive: true });
      await Fs.writeFile(path, content, "utf8");
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

const readJsonFile = (path: string) =>
  Effect.tryPromise({
    try: async () => JSON.parse(await Fs.readFile(path, "utf8")) as unknown,
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

const fileExists = (path: string) =>
  Effect.tryPromise({
    try: async () => {
      await Fs.access(path);
      return true;
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  }).pipe(Effect.catchAll(() => Effect.succeed(false)));

const isHostedMode = (mode: FullFlowSmokeMode): mode is "convex" | "api" =>
  mode === "convex" || mode === "api";

const modeForStaging = (mode: FullFlowSmokeMode): StagingSmokeMode =>
  isHostedMode(mode) ? mode : "mock";

const toLocalObservabilityRecords = (
  artifact: LocalNodeFlowArtifact | null,
): ReadonlyArray<unknown> => {
  if (!artifact?.flows) return [];
  return artifact.flows.flatMap((flow) => flow.observabilityRecords ?? []);
};

const toLocalParityPresentKeys = (artifact: LocalNodeFlowArtifact | null): ReadonlySet<string> => {
  const present = new Set<string>();
  const records = toLocalObservabilityRecords(artifact);
  for (const key of collectPresentKeys(records)) {
    present.add(key);
  }
  for (const flow of artifact?.flows ?? []) {
    if (asNonEmptyStringOrNull(flow.taskId)) present.add("taskId");
    if (asNonEmptyStringOrNull(flow.createRequestId)) present.add("requestId");
    if (asNonEmptyStringOrNull(flow.proofReference)) present.add("paymentProofRef");
  }
  return present;
};

const toHostedParityPresentKeys = (
  records: ReadonlyArray<L402ObservabilityRecord>,
): ReadonlySet<string> => collectPresentKeys(records);

const ensure = (condition: boolean, message: string) =>
  condition ? Effect.void : Effect.fail(new Error(message));

const buildEventsJsonl = (events: ReadonlyArray<FullFlowEvent>): string =>
  `${events.map((event) => JSON.stringify(event)).join("\n")}\n`;

export const runFullFlowSmoke = (input?: {
  readonly mode?: FullFlowSmokeMode;
  readonly requestId?: string;
  readonly artifactDir?: string;
  readonly localArtifactPath?: string;
  readonly strictLocalParity?: boolean;
}) =>
  Effect.gen(function* () {
    const mode = input?.mode ?? "mock";
    const requestId = input?.requestId ?? `smoke:full-flow:${Date.now()}`;
    const artifactDir = input?.artifactDir ?? defaultArtifactDir(requestId);
    const summaryPath = Path.join(artifactDir, "summary.json");
    const eventsPath = Path.join(artifactDir, "events.jsonl");
    const localArtifactPath = input?.localArtifactPath ?? defaultLocalArtifactPath();
    const strictLocalParity = input?.strictLocalParity ?? true;
    const events: Array<FullFlowEvent> = [];
    const failedChecks: Array<string> = [];

    const paywall = smokePaywalls[0];
    if (!paywall) {
      return yield* Effect.fail(new Error("smoke_paywall_missing"));
    }
    const route = paywall.routes[0];
    if (!route) {
      return yield* Effect.fail(new Error("smoke_route_missing"));
    }

    addEvent(events, {
      stage: "paywall.create",
      requestId,
      status: "ok",
      details: {
        paywallId: paywall.paywallId,
        ownerId: paywall.ownerId,
        routeId: route.routeId,
      },
    });

    const reconcileRequestId = `${requestId}:reconcile`;
    const staging = yield* runStagingSmoke({
      mode: modeForStaging(mode),
      requestId: reconcileRequestId,
    });

    const stagingHealthy = staging.healthOk && staging.challengeOk && staging.proxyOk;
    if (!stagingHealthy) failedChecks.push("gateway_reconcile");
    yield* ensure(stagingHealthy, "full_flow_gateway_probe_failed");
    addEvent(events, {
      stage: "gateway.reconcile",
      requestId: reconcileRequestId,
      status: "ok",
      details: {
        deploymentId: staging.deploymentId,
        configHash: staging.configHash,
        status: staging.deploymentStatus,
        challengeOk: staging.challengeOk,
        proxyOk: staging.proxyOk,
        healthOk: staging.healthOk,
      },
    });

    const settlement = yield* runSettlementSmoke({ mode });
    const primarySettlement = settlement.settlements[0];
    const primaryCorrelation = settlement.correlationRefs[0];
    if (!primarySettlement || !primaryCorrelation) {
      return yield* Effect.fail(new Error("full_flow_settlement_missing"));
    }
    addEvent(events, {
      stage: "request.paid",
      requestId: asNonEmptyStringOrNull(primaryCorrelation.requestId) ?? requestId,
      status: "ok",
      details: {
        settlementId: primarySettlement.settlementId,
        paymentProofRef: primarySettlement.paymentProofRef,
        amountMsats: primarySettlement.amountMsats,
        taskId: asNonEmptyStringOrNull(primaryCorrelation.taskId),
        routeId: asNonEmptyStringOrNull(primaryCorrelation.routeId),
      },
    });

    const security = yield* runSecuritySmoke({ mode });
    const denyCode = security.globalPause.denyReasonCode ?? "global_pause_active";
    const denyReason = security.globalPause.allowed
      ? "global_pause_not_enforced"
      : "global pause policy is active";
    const denyPassed = security.globalPause.allowed === false;
    if (!denyPassed) failedChecks.push("policy_denied_request");
    yield* ensure(denyPassed, "full_flow_policy_denied_missing");
    addEvent(events, {
      stage: "request.denied",
      requestId: `${requestId}:policy-denied`,
      status: "ok",
      details: {
        denyReasonCode: denyCode,
        denyReason,
      },
    });

    const observability = yield* runObservabilitySmoke({
      mode,
      requestId: `${requestId}:observability`,
    });
    if (observability.missingFieldKeys.length > 0) failedChecks.push("observability_required_fields");
    yield* ensure(
      observability.missingFieldKeys.length === 0,
      `full_flow_missing_observability_fields:${observability.missingFieldKeys.join(",")}`,
    );
    addEvent(events, {
      stage: "observability.verify",
      requestId: observability.requestId,
      status: "ok",
      details: {
        recordCount: observability.records.length,
        missingFieldKeys: observability.missingFieldKeys,
      },
    });

    const hostedPresent = toHostedParityPresentKeys(observability.records);
    const hostedMissingKeys = parityMissingKeys(hostedPresent, requiredParityKeys);
    if (hostedMissingKeys.length > 0) failedChecks.push("hosted_parity_keys");
    yield* ensure(
      hostedMissingKeys.length === 0,
      `full_flow_missing_hosted_parity_keys:${hostedMissingKeys.join(",")}`,
    );

    const localArtifactPresent = yield* fileExists(localArtifactPath);
    const localArtifactRaw = localArtifactPresent ? yield* readJsonFile(localArtifactPath) : null;
    const localArtifact = parseLocalArtifact(localArtifactRaw);
    const localPresent = toLocalParityPresentKeys(localArtifact);
    const localMissingKeys = parityMissingKeys(localPresent, requiredParityKeys);
    if (strictLocalParity && !localArtifactPresent) {
      failedChecks.push("local_artifact_missing");
      return yield* Effect.fail(new Error(`full_flow_local_artifact_missing:${localArtifactPath}`));
    }
    if (strictLocalParity && localMissingKeys.length > 0) {
      failedChecks.push("local_parity_keys");
      return yield* Effect.fail(
        new Error(`full_flow_missing_local_parity_keys:${localMissingKeys.join(",")}`),
      );
    }
    addEvent(events, {
      stage: "parity.verify",
      requestId,
      status: "ok",
      details: {
        localArtifactPresent,
        localArtifactPath,
        hostedMissingKeys,
        localMissingKeys,
      },
    });

    const hostedKeys = toSortedFieldKeys(hostedPresent, L402ObservabilityFieldKeys);
    const localKeys = toSortedFieldKeys(localPresent, L402ObservabilityFieldKeys);
    const sharedKeys = toSortedFieldKeys(
      new Set(hostedKeys.filter((key) => localKeys.includes(key))),
      L402ObservabilityFieldKeys,
    );

    const checks = [
      { id: "paywall_creation", passed: true },
      { id: "gateway_reconcile", passed: stagingHealthy },
      { id: "paid_request_success", passed: true },
      { id: "policy_deny", passed: denyPassed },
      { id: "settlement_correlation", passed: true },
      { id: "observability", passed: observability.missingFieldKeys.length === 0 },
      { id: "hosted_parity", passed: hostedMissingKeys.length === 0 },
      {
        id: "local_parity",
        passed: strictLocalParity ? localArtifactPresent && localMissingKeys.length === 0 : true,
      },
    ] as const;
    const passedChecks = checks.filter((row) => row.passed).length;
    const totalChecks = checks.length;
    const generatedAtMs = Date.now();

    const summary: FullFlowSmokeSummary = {
      ok: failedChecks.length === 0,
      mode,
      requestId,
      executionPath: "hosted-node",
      paywallCreation: {
        ok: true,
        paywallId: paywall.paywallId,
        ownerId: paywall.ownerId,
        routeId: route.routeId,
      },
      gatewayReconcile: {
        requestId: staging.requestId,
        deploymentId: staging.deploymentId,
        configHash: staging.configHash,
        deploymentStatus: staging.deploymentStatus,
        challengeOk: staging.challengeOk,
        proxyOk: staging.proxyOk,
        healthOk: staging.healthOk,
      },
      paidRequest: {
        status: "paid",
        requestId: asNonEmptyStringOrNull(primaryCorrelation.requestId),
        taskId: asNonEmptyStringOrNull(primaryCorrelation.taskId),
        routeId: asNonEmptyStringOrNull(primaryCorrelation.routeId),
        settlementId: primarySettlement.settlementId,
        amountMsats: primarySettlement.amountMsats,
        paymentProofRef: primarySettlement.paymentProofRef,
      },
      policyDeniedRequest: {
        status: "denied",
        requestId: `${requestId}:policy-denied`,
        denyReasonCode: denyCode,
        denyReason,
        source: "global_pause",
      },
      settlementCorrelation: {
        processed: settlement.processed,
        settlementIds: settlement.settlements.map((row) => row.settlementId),
        paymentProofRefs: settlement.settlements.map((row) => row.paymentProofRef),
      },
      observability: {
        recordCount: observability.records.length,
        requiredFieldKeys: observability.requiredFieldKeys,
        missingFieldKeys: observability.missingFieldKeys,
        correlation: observability.correlation,
      },
      parity: {
        requiredKeys: requiredParityKeys,
        hostedKeys,
        hostedMissingKeys,
        localArtifactPath,
        localArtifactPresent,
        localKeys,
        localMissingKeys,
        sharedKeys,
      },
      coverage: {
        totalChecks,
        passedChecks,
        failedChecks: failedChecks.length > 0 ? [...failedChecks] : checks.filter((row) => !row.passed).map((row) => row.id),
      },
      artifacts: {
        artifactDir,
        summaryPath,
        eventsPath,
        generatedAtMs,
      },
    };

    yield* writeTextFile(eventsPath, buildEventsJsonl(events));
    yield* writeTextFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);

    return summary;
  });
