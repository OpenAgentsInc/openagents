import { Console, Effect, Layer } from "effect";

import { ApertureConfigCompilerLive } from "./compiler/apertureCompiler.js";
import { ApiTransportLive } from "./controlPlane/apiTransport.js";
import { makeInMemoryControlPlaneHarness } from "./controlPlane/inMemory.js";
import { ControlPlaneLive } from "./controlPlane/live.js";
import { smokePaywalls } from "./fixtures/smokePaywalls.js";
import { compileAndPersistOnce } from "./programs/compileAndPersist.js";
import { runObservabilitySmoke, type ObservabilitySmokeMode } from "./programs/smokeObservability.js";
import { runSecuritySmoke, type SecuritySmokeMode } from "./programs/securityControls.js";
import { runSettlementSmoke, type SettlementSmokeMode } from "./programs/smokeSettlement.js";
import { runStagingSmoke, type StagingSmokeMode } from "./programs/smokeStaging.js";
import { runEp212RoutesSmoke, type Ep212RoutesSmokeMode, type Ep212RoutesSmokeSummary } from "./programs/smokeEp212Routes.js";
import {
  runEp212FullFlowSmoke,
  type Ep212FullFlowMode,
  type Ep212FullFlowSummary,
} from "./programs/smokeEp212FullFlow.js";
import {
  runFullFlowSmoke,
  type FullFlowSmokeMode,
  type FullFlowSmokeSummary,
} from "./programs/fullFlow.js";
import { OpsRuntimeConfigLive } from "./runtime/config.js";

const usage = `Usage:
  tsx src/main.ts smoke:compile [--json] [--mode mock|api]
  tsx src/main.ts compile:api [--json]
  tsx src/main.ts reconcile:api [--json]
  tsx src/main.ts smoke:security [--json] [--mode mock|api]
  tsx src/main.ts smoke:settlement [--json] [--mode mock|api]
  tsx src/main.ts smoke:staging [--json] [--mode mock|api]
  tsx src/main.ts smoke:ep212-routes [--json] [--mode mock|live]
  tsx src/main.ts smoke:ep212-full-flow [--json] [--mode mock|live] [--artifact-dir <path>]
  tsx src/main.ts smoke:observability [--json] [--mode mock|api]
  tsx src/main.ts smoke:full-flow [--json] [--mode mock|api] [--artifact-dir <path>] [--local-artifact <path>] [--allow-missing-local-artifact]
`;

type HostedControlPlaneMode = "api";
type ControlPlaneSmokeMode = "mock" | HostedControlPlaneMode;

const resolveDefaultControlPlaneSmokeMode = (): ControlPlaneSmokeMode => {
  const value = process.env.OA_LIGHTNING_OPS_CONTROL_PLANE_MODE?.trim().toLowerCase();
  if (value === "mock" || value === "api") {
    return value;
  }
  return "api";
};

const hostedControlPlaneLayer = () =>
  ControlPlaneLive.pipe(
    Layer.provideMerge(ApiTransportLive),
    Layer.provideMerge(OpsRuntimeConfigLive),
  );

const toCompileSummaryJson = (summary: {
  readonly configHash: string;
  readonly ruleCount: number;
  readonly valid: boolean;
  readonly deploymentStatus: string;
  readonly deploymentId: string;
  readonly diagnostics: ReadonlyArray<unknown>;
}) =>
  JSON.stringify({
    ok: true,
    configHash: summary.configHash,
    ruleCount: summary.ruleCount,
    valid: summary.valid,
    deploymentStatus: summary.deploymentStatus,
    deploymentId: summary.deploymentId,
    diagnosticsCount: summary.diagnostics.length,
    diagnostics: summary.diagnostics,
  });

const toReconcileSummaryJson = (summary: {
  readonly configHash: string;
  readonly ruleCount: number;
  readonly valid: boolean;
  readonly deploymentStatus: string;
  readonly deploymentId: string;
  readonly diagnostics: ReadonlyArray<unknown>;
  readonly challengeOk: boolean;
  readonly proxyOk: boolean;
  readonly healthOk: boolean;
  readonly requestId: string;
  readonly executionPath: string;
}) =>
  JSON.stringify({
    ok: true,
    requestId: summary.requestId,
    executionPath: summary.executionPath,
    configHash: summary.configHash,
    ruleCount: summary.ruleCount,
    valid: summary.valid,
    deploymentStatus: summary.deploymentStatus,
    deploymentId: summary.deploymentId,
    challengeOk: summary.challengeOk,
    proxyOk: summary.proxyOk,
    healthOk: summary.healthOk,
    diagnosticsCount: summary.diagnostics.length,
    diagnostics: summary.diagnostics,
  });

const toSettlementSmokeJson = (summary: {
  readonly processed: number;
  readonly invoiceTransitions: ReadonlyArray<{
    readonly invoiceId: string;
    readonly status: string;
    readonly updatedAtMs: number;
  }>;
  readonly settlements: ReadonlyArray<{
    readonly settlementId: string;
    readonly amountMsats: number;
    readonly paymentProofRef: string;
    readonly existed: boolean;
  }>;
  readonly correlationRefs: ReadonlyArray<{
    readonly settlementId: string;
    readonly paymentProofRef: string;
    readonly requestId?: string;
    readonly taskId?: string;
    readonly routeId?: string;
  }>;
}) =>
  JSON.stringify({
    ok: true,
    processed: summary.processed,
    invoiceTransitions: summary.invoiceTransitions,
    settlements: summary.settlements,
    settlementIds: summary.settlements.map((row) => row.settlementId),
    paymentProofRefs: summary.settlements.map((row) => row.paymentProofRef),
    correlationRefs: summary.correlationRefs,
  });

const toSecuritySmokeJson = (summary: {
  readonly executionPath: string;
  readonly failClosed: {
    readonly passed: boolean;
    readonly errorTag?: string;
    readonly errorCode?: string;
    readonly role?: string;
    readonly field?: string;
  };
  readonly globalPause: {
    readonly allowed: boolean;
    readonly denyReasonCode?: string;
  };
  readonly ownerKillSwitch: {
    readonly allowed: boolean;
    readonly denyReasonCode?: string;
  };
  readonly recovery: {
    readonly allowed: boolean;
  };
  readonly credentialLifecycle: {
    readonly rotatedVersion: number;
    readonly revokedStatus: string;
    readonly activatedStatus: string;
    readonly activatedVersion: number;
  };
  readonly statusSnapshot: {
    readonly globalPauseActive: boolean;
    readonly activeOwnerKillSwitches: number;
    readonly credentialRoles: ReadonlyArray<{
      readonly role: string;
      readonly status: string;
      readonly version: number;
    }>;
  };
}) =>
  JSON.stringify({
    ok: true,
    executionPath: summary.executionPath,
    failClosed: summary.failClosed,
    globalPause: summary.globalPause,
    ownerKillSwitch: summary.ownerKillSwitch,
    recovery: summary.recovery,
    credentialLifecycle: summary.credentialLifecycle,
    statusSnapshot: summary.statusSnapshot,
  });

const toObservabilitySmokeJson = (summary: {
  readonly requestId: string;
  readonly executionPath: string;
  readonly records: ReadonlyArray<{
    readonly requestId: string | null;
    readonly userId: string | null;
    readonly paywallId: string | null;
    readonly taskId: string | null;
    readonly endpoint: string | null;
    readonly quotedCostMsats: number | null;
    readonly capAppliedMsats: number | null;
    readonly paidAmountMsats: number | null;
    readonly paymentProofRef: string | null;
    readonly cacheHit: boolean | null;
    readonly denyReason: string | null;
    readonly executor: string;
    readonly plane: string;
    readonly executionPath: string;
    readonly desktopSessionId: string | null;
    readonly desktopRuntimeStatus: string | null;
    readonly walletState: string | null;
    readonly nodeSyncStatus: string | null;
    readonly observedAtMs: number;
  }>;
  readonly requiredFieldKeys: ReadonlyArray<string>;
  readonly missingFieldKeys: ReadonlyArray<string>;
  readonly correlation: {
    readonly requestIds: ReadonlyArray<string>;
    readonly paywallIds: ReadonlyArray<string>;
    readonly taskIds: ReadonlyArray<string>;
    readonly paymentProofRefs: ReadonlyArray<string>;
  };
}) =>
  JSON.stringify({
    ok: true,
    requestId: summary.requestId,
    executionPath: summary.executionPath,
    recordCount: summary.records.length,
    requiredFieldKeys: summary.requiredFieldKeys,
    missingFieldKeys: summary.missingFieldKeys,
    correlation: summary.correlation,
    records: summary.records,
  });

const toFullFlowSmokeJson = (summary: FullFlowSmokeSummary) => JSON.stringify(summary);
const toEp212RoutesSmokeJson = (summary: Ep212RoutesSmokeSummary) => JSON.stringify(summary);
const toEp212FullFlowSmokeJson = (summary: Ep212FullFlowSummary) => JSON.stringify(summary);

const printCompileSummary = (
  summary: {
    readonly configHash: string;
    readonly ruleCount: number;
    readonly valid: boolean;
    readonly deploymentStatus: string;
    readonly deploymentId: string;
    readonly diagnostics: ReadonlyArray<unknown>;
  },
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toCompileSummaryJson(summary))
    : Console.log(
        [
          `configHash=${summary.configHash}`,
          `ruleCount=${summary.ruleCount}`,
          `valid=${summary.valid}`,
          `deploymentStatus=${summary.deploymentStatus}`,
          `deploymentId=${summary.deploymentId}`,
        ].join("\n"),
      );

const printReconcileSummary = (
  summary: {
    readonly configHash: string;
    readonly ruleCount: number;
    readonly valid: boolean;
    readonly deploymentStatus: string;
    readonly deploymentId: string;
    readonly diagnostics: ReadonlyArray<unknown>;
    readonly challengeOk: boolean;
    readonly proxyOk: boolean;
    readonly healthOk: boolean;
    readonly requestId: string;
    readonly executionPath: string;
  },
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toReconcileSummaryJson(summary))
    : Console.log(
        [
          `requestId=${summary.requestId}`,
          `executionPath=${summary.executionPath}`,
          `configHash=${summary.configHash}`,
          `ruleCount=${summary.ruleCount}`,
          `valid=${summary.valid}`,
          `deploymentStatus=${summary.deploymentStatus}`,
          `deploymentId=${summary.deploymentId}`,
          `healthOk=${summary.healthOk}`,
          `challengeOk=${summary.challengeOk}`,
          `proxyOk=${summary.proxyOk}`,
        ].join("\n"),
      );

const printSettlementSummary = (
  summary: {
    readonly processed: number;
    readonly invoiceTransitions: ReadonlyArray<{
      readonly invoiceId: string;
      readonly status: string;
      readonly updatedAtMs: number;
    }>;
    readonly settlements: ReadonlyArray<{
      readonly settlementId: string;
      readonly amountMsats: number;
      readonly paymentProofRef: string;
      readonly existed: boolean;
    }>;
    readonly correlationRefs: ReadonlyArray<{
      readonly settlementId: string;
      readonly paymentProofRef: string;
      readonly requestId?: string;
      readonly taskId?: string;
      readonly routeId?: string;
    }>;
  },
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toSettlementSmokeJson(summary))
    : Console.log(
        [
          `processed=${summary.processed}`,
          `invoices=${summary.invoiceTransitions.length}`,
          `settlements=${summary.settlements.length}`,
          `settlementIds=${summary.settlements.map((row) => row.settlementId).join(",")}`,
          `settlementAmountsMsats=${summary.settlements.map((row) => String(row.amountMsats)).join(",")}`,
          `paymentProofRefs=${summary.settlements.map((row) => row.paymentProofRef).join(",")}`,
        ].join("\n"),
      );

const printSecuritySummary = (
  summary: {
    readonly executionPath: string;
    readonly failClosed: {
      readonly passed: boolean;
      readonly errorTag?: string;
      readonly errorCode?: string;
      readonly role?: string;
      readonly field?: string;
    };
    readonly globalPause: {
      readonly allowed: boolean;
      readonly denyReasonCode?: string;
    };
    readonly ownerKillSwitch: {
      readonly allowed: boolean;
      readonly denyReasonCode?: string;
    };
    readonly recovery: {
      readonly allowed: boolean;
    };
    readonly credentialLifecycle: {
      readonly rotatedVersion: number;
      readonly revokedStatus: string;
      readonly activatedStatus: string;
      readonly activatedVersion: number;
    };
    readonly statusSnapshot: {
      readonly globalPauseActive: boolean;
      readonly activeOwnerKillSwitches: number;
      readonly credentialRoles: ReadonlyArray<{
        readonly role: string;
        readonly status: string;
        readonly version: number;
      }>;
    };
  },
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toSecuritySmokeJson(summary))
    : Console.log(
        [
          `executionPath=${summary.executionPath}`,
          `failClosed=${summary.failClosed.passed}`,
          `globalPauseAllowed=${summary.globalPause.allowed}`,
          `ownerKillAllowed=${summary.ownerKillSwitch.allowed}`,
          `recoveryAllowed=${summary.recovery.allowed}`,
          `credentialLifecycle=${summary.credentialLifecycle.rotatedVersion}:${summary.credentialLifecycle.revokedStatus}->${summary.credentialLifecycle.activatedStatus}@${summary.credentialLifecycle.activatedVersion}`,
          `globalPauseActive=${summary.statusSnapshot.globalPauseActive}`,
          `activeOwnerKillSwitches=${summary.statusSnapshot.activeOwnerKillSwitches}`,
        ].join("\n"),
      );

const printObservabilitySummary = (
  summary: {
    readonly requestId: string;
    readonly executionPath: string;
    readonly records: ReadonlyArray<{
      readonly requestId: string | null;
      readonly userId: string | null;
      readonly paywallId: string | null;
      readonly taskId: string | null;
      readonly endpoint: string | null;
      readonly quotedCostMsats: number | null;
      readonly capAppliedMsats: number | null;
      readonly paidAmountMsats: number | null;
      readonly paymentProofRef: string | null;
      readonly cacheHit: boolean | null;
      readonly denyReason: string | null;
      readonly executor: string;
      readonly plane: string;
      readonly executionPath: string;
      readonly desktopSessionId: string | null;
      readonly desktopRuntimeStatus: string | null;
      readonly walletState: string | null;
      readonly nodeSyncStatus: string | null;
      readonly observedAtMs: number;
    }>;
    readonly requiredFieldKeys: ReadonlyArray<string>;
    readonly missingFieldKeys: ReadonlyArray<string>;
    readonly correlation: {
      readonly requestIds: ReadonlyArray<string>;
      readonly paywallIds: ReadonlyArray<string>;
      readonly taskIds: ReadonlyArray<string>;
      readonly paymentProofRefs: ReadonlyArray<string>;
    };
  },
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toObservabilitySmokeJson(summary))
    : Console.log(
        [
          `requestId=${summary.requestId}`,
          `executionPath=${summary.executionPath}`,
          `records=${summary.records.length}`,
          `missingFieldKeys=${summary.missingFieldKeys.join(",") || "none"}`,
          `correlation.requestIds=${summary.correlation.requestIds.join(",")}`,
          `correlation.paywallIds=${summary.correlation.paywallIds.join(",")}`,
          `correlation.taskIds=${summary.correlation.taskIds.join(",")}`,
          `correlation.paymentProofRefs=${summary.correlation.paymentProofRefs.join(",")}`,
        ].join("\n"),
      );

const printFullFlowSummary = (summary: FullFlowSmokeSummary, jsonOutput: boolean) =>
  jsonOutput
    ? Console.log(toFullFlowSmokeJson(summary))
    : Console.log(
        [
          `ok=${summary.ok}`,
          `mode=${summary.mode}`,
          `requestId=${summary.requestId}`,
          `executionPath=${summary.executionPath}`,
          `paywallId=${summary.paywallCreation.paywallId}`,
          `deploymentId=${summary.gatewayReconcile.deploymentId}`,
          `configHash=${summary.gatewayReconcile.configHash}`,
          `challengeOk=${summary.gatewayReconcile.challengeOk}`,
          `proxyOk=${summary.gatewayReconcile.proxyOk}`,
          `healthOk=${summary.gatewayReconcile.healthOk}`,
          `settlementId=${summary.paidRequest.settlementId}`,
          `paymentProofRef=${summary.paidRequest.paymentProofRef}`,
          `denyReasonCode=${summary.policyDeniedRequest.denyReasonCode}`,
          `coverage=${summary.coverage.passedChecks}/${summary.coverage.totalChecks}`,
          `artifacts.events=${summary.artifacts.eventsPath}`,
          `artifacts.summary=${summary.artifacts.summaryPath}`,
        ].join("\n"),
      );

const printEp212RoutesSummary = (summary: Ep212RoutesSmokeSummary, jsonOutput: boolean) =>
  jsonOutput
    ? Console.log(toEp212RoutesSmokeJson(summary))
    : Console.log(
        [
          `ok=${summary.ok}`,
          `mode=${summary.mode}`,
          `requestId=${summary.requestId}`,
          `walletBackend=${summary.walletBackend}`,
          `routeA.url=${summary.routeA.url}`,
          `routeA.challengeStatusCode=${summary.routeA.challengeStatusCode}`,
          `routeA.quotedAmountMsats=${summary.routeA.quotedAmountMsats ?? "n/a"}`,
          `routeA.paidStatusCode=${summary.routeA.paidStatusCode}`,
          `routeA.paidAmountMsats=${summary.routeA.paidAmountMsats}`,
          `routeA.paymentId=${summary.routeA.paymentId ?? "n/a"}`,
          `routeA.proofReference=${summary.routeA.proofReference}`,
          `routeA.responseBytes=${summary.routeA.responseBytes}`,
          `routeA.responseSha256=${summary.routeA.responseSha256}`,
          `routeB.url=${summary.routeB.url}`,
          `routeB.challengeStatusCode=${summary.routeB.challengeStatusCode}`,
          `routeB.quotedAmountMsats=${summary.routeB.quotedAmountMsats ?? "n/a"}`,
          `routeB.maxSpendMsats=${summary.routeB.maxSpendMsats}`,
          `routeB.blocked=${summary.routeB.blocked}`,
          `routeB.denyReasonCode=${summary.routeB.denyReasonCode}`,
          `routeB.payerCallsBefore=${summary.routeB.payerCallsBefore}`,
          `routeB.payerCallsAfter=${summary.routeB.payerCallsAfter}`,
        ].join("\n"),
      );

const printEp212FullFlowSummary = (
  summary: Ep212FullFlowSummary,
  jsonOutput: boolean,
) =>
  jsonOutput
    ? Console.log(toEp212FullFlowSmokeJson(summary))
    : Console.log(
        [
          `ok=${summary.ok}`,
          `mode=${summary.mode}`,
          `requestId=${summary.requestId}`,
          `walletBackend=${summary.walletBackend}`,
          `sats4ai.url=${summary.sats4ai.endpointUrl}`,
          `sats4ai.challengeStatusCode=${summary.sats4ai.challengeStatusCode}`,
          `sats4ai.firstStatusCode=${summary.sats4ai.firstStatusCode}`,
          `sats4ai.firstPaid=${summary.sats4ai.firstPaid}`,
          `sats4ai.secondStatusCode=${summary.sats4ai.secondStatusCode}`,
          `sats4ai.secondPaid=${summary.sats4ai.secondPaid}`,
          `sats4ai.cacheHit=${summary.sats4ai.cacheHit}`,
          `sats4ai.payerCallsAfterFirst=${summary.sats4ai.payerCallsAfterFirst}`,
          `sats4ai.payerCallsAfterSecond=${summary.sats4ai.payerCallsAfterSecond}`,
          `openAgents.url=${summary.openAgentsRoute.endpointUrl}`,
          `openAgents.challengeStatusCode=${summary.openAgentsRoute.challengeStatusCode}`,
          `openAgents.paidStatusCode=${summary.openAgentsRoute.paidStatusCode}`,
          `openAgents.paidAmountMsats=${summary.openAgentsRoute.paidAmountMsats}`,
          `overCap.url=${summary.overCap.endpointUrl}`,
          `overCap.challengeStatusCode=${summary.overCap.challengeStatusCode}`,
          `overCap.blocked=${summary.overCap.blocked}`,
          `overCap.denyReasonCode=${summary.overCap.denyReasonCode}`,
          `overCap.payerCallsBefore=${summary.overCap.payerCallsBefore}`,
          `overCap.payerCallsAfter=${summary.overCap.payerCallsAfter}`,
          `artifacts.events=${summary.artifacts.eventsPath}`,
          `artifacts.summary=${summary.artifacts.summaryPath}`,
        ].join("\n"),
      );

const runHostedCompile = (
  jsonOutput: boolean,
  requestId: string,
) => {
  const liveLayer = Layer.mergeAll(ApertureConfigCompilerLive, hostedControlPlaneLayer());

  return compileAndPersistOnce({ requestId }).pipe(
    Effect.provide(liveLayer),
    Effect.flatMap((summary) => printCompileSummary(summary, jsonOutput)),
  );
};

const runSmokeCompile = (jsonOutput: boolean, mode: ControlPlaneSmokeMode) => {
  if (mode === "mock") {
    const harness = makeInMemoryControlPlaneHarness({ paywalls: smokePaywalls });
    return compileAndPersistOnce({ requestId: "smoke:compile" }).pipe(
      Effect.provide(harness.layer),
      Effect.provide(ApertureConfigCompilerLive),
      Effect.flatMap((summary) => printCompileSummary(summary, jsonOutput)),
    );
  }

  return runHostedCompile(jsonOutput, "smoke:compile");
};

const runApiCompile = (jsonOutput: boolean) => {
  return runHostedCompile(jsonOutput, "compile:api");
};

const runApiReconcile = (jsonOutput: boolean) => {
  return runStagingSmoke({ mode: "api", requestId: "reconcile:api" }).pipe(
    Effect.flatMap((summary) => printReconcileSummary(summary, jsonOutput)),
  );
};

const parseMode = <A extends string>(argv: ReadonlyArray<string>, values: ReadonlyArray<A>, fallback: A): A => {
  const modeFlag = argv.find((value) => value.startsWith("--mode="));
  if (modeFlag) {
    const value = modeFlag.slice("--mode=".length);
    return values.includes(value as A) ? (value as A) : fallback;
  }

  const modeIndex = argv.findIndex((value) => value === "--mode");
  if (modeIndex >= 0) {
    const next = argv[modeIndex + 1];
    return values.includes(next as A) ? (next as A) : fallback;
  }

  return fallback;
};

const runSmokeStaging = (jsonOutput: boolean, mode: StagingSmokeMode) =>
  runStagingSmoke({ mode }).pipe(Effect.flatMap((summary) => printReconcileSummary(summary, jsonOutput)));

const runSmokeSettlement = (jsonOutput: boolean, mode: SettlementSmokeMode) =>
  runSettlementSmoke({ mode }).pipe(Effect.flatMap((summary) => printSettlementSummary(summary, jsonOutput)));

const runSmokeSecurity = (jsonOutput: boolean, mode: SecuritySmokeMode) =>
  runSecuritySmoke({ mode }).pipe(Effect.flatMap((summary) => printSecuritySummary(summary, jsonOutput)));

const runSmokeObservability = (jsonOutput: boolean, mode: ObservabilitySmokeMode) =>
  runObservabilitySmoke({ mode }).pipe(
    Effect.flatMap((summary) => printObservabilitySummary(summary, jsonOutput)),
  );

const runSmokeEp212Routes = (jsonOutput: boolean, mode: Ep212RoutesSmokeMode) =>
  runEp212RoutesSmoke({ mode }).pipe(
    Effect.flatMap((summary) => printEp212RoutesSummary(summary, jsonOutput)),
  );

const runSmokeEp212FullFlow = (input: {
  readonly jsonOutput: boolean;
  readonly mode: Ep212FullFlowMode;
  readonly artifactDir?: string;
}) =>
  runEp212FullFlowSmoke({
    mode: input.mode,
    ...(input.artifactDir ? { artifactDir: input.artifactDir } : {}),
  }).pipe(Effect.flatMap((summary) => printEp212FullFlowSummary(summary, input.jsonOutput)));

const runSmokeFullFlow = (input: {
  readonly jsonOutput: boolean;
  readonly mode: FullFlowSmokeMode;
  readonly artifactDir?: string;
  readonly localArtifactPath?: string;
  readonly strictLocalParity: boolean;
}) =>
  runFullFlowSmoke({
    mode: input.mode,
    ...(input.artifactDir ? { artifactDir: input.artifactDir } : {}),
    ...(input.localArtifactPath ? { localArtifactPath: input.localArtifactPath } : {}),
    strictLocalParity: input.strictLocalParity,
  }).pipe(Effect.flatMap((summary) => printFullFlowSummary(summary, input.jsonOutput)));

const parseTextOption = (
  argv: ReadonlyArray<string>,
  name: string,
): string | undefined => {
  const inlineFlag = argv.find((value) => value.startsWith(`${name}=`));
  if (inlineFlag) {
    const value = inlineFlag.slice(`${name}=`.length).trim();
    return value.length > 0 ? value : undefined;
  }

  const index = argv.findIndex((value) => value === name);
  if (index < 0) return undefined;
  const next = argv[index + 1];
  if (typeof next !== "string") return undefined;
  const trimmed = next.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const main = Effect.gen(function* () {
  const argv = process.argv.slice(2);
  const command = argv[0] ?? "";
  const jsonOutput = argv.includes("--json");
  const controlPlaneModeFallback = resolveDefaultControlPlaneSmokeMode();

  if (command === "smoke:compile") {
    return yield* runSmokeCompile(
      jsonOutput,
      parseMode(argv, ["mock", "api"], controlPlaneModeFallback),
    );
  }

  if (command === "compile:api") {
    return yield* runApiCompile(jsonOutput);
  }

  if (command === "reconcile:api") {
    return yield* runApiReconcile(jsonOutput);
  }

  if (command === "smoke:settlement") {
    return yield* runSmokeSettlement(
      jsonOutput,
      parseMode(argv, ["mock", "api"], controlPlaneModeFallback),
    );
  }

  if (command === "smoke:security") {
    return yield* runSmokeSecurity(
      jsonOutput,
      parseMode(argv, ["mock", "api"], controlPlaneModeFallback),
    );
  }

  if (command === "smoke:staging") {
    return yield* runSmokeStaging(jsonOutput, parseMode(argv, ["mock", "api"], "mock"));
  }

  if (command === "smoke:observability") {
    return yield* runSmokeObservability(jsonOutput, parseMode(argv, ["mock", "api"], "mock"));
  }

  if (command === "smoke:ep212-routes") {
    return yield* runSmokeEp212Routes(jsonOutput, parseMode(argv, ["mock", "live"], "mock"));
  }

  if (command === "smoke:ep212-full-flow") {
    const artifactDir = parseTextOption(argv, "--artifact-dir");
    return yield* runSmokeEp212FullFlow({
      jsonOutput,
      mode: parseMode(argv, ["mock", "live"], "mock"),
      ...(artifactDir ? { artifactDir } : {}),
    });
  }

  if (command === "smoke:full-flow") {
    const artifactDir = parseTextOption(argv, "--artifact-dir");
    const localArtifactPath = parseTextOption(argv, "--local-artifact");
    return yield* runSmokeFullFlow({
      jsonOutput,
      mode: parseMode(argv, ["mock", "api"], "mock"),
      ...(artifactDir ? { artifactDir } : {}),
      ...(localArtifactPath ? { localArtifactPath } : {}),
      strictLocalParity: !argv.includes("--allow-missing-local-artifact"),
    });
  }

  return yield* Console.error(usage).pipe(Effect.zipRight(Effect.fail(new Error("invalid_command"))));
}).pipe(
  Effect.catchAll((error) =>
    Effect.sync(() => {
      if (process.argv.includes("--json")) {
        console.log(
          JSON.stringify({
            ok: false,
            error: String(error),
          }),
        );
      } else {
        console.error(String(error));
      }
    }).pipe(Effect.zipRight(Effect.fail(error))),
  ),
);

Effect.runPromise(main).catch(() => {
  process.exitCode = 1;
});
