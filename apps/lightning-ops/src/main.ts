import { Console, Effect, Layer } from "effect";

import { ApertureConfigCompilerLive } from "./compiler/apertureCompiler.js";
import { makeInMemoryControlPlaneHarness } from "./controlPlane/inMemory.js";
import { ConvexControlPlaneLive } from "./controlPlane/convex.js";
import { ConvexTransportLive } from "./controlPlane/convexTransport.js";
import { smokePaywalls } from "./fixtures/smokePaywalls.js";
import { compileAndPersistOnce } from "./programs/compileAndPersist.js";
import { runSecuritySmoke, type SecuritySmokeMode } from "./programs/securityControls.js";
import { runSettlementSmoke, type SettlementSmokeMode } from "./programs/smokeSettlement.js";
import { runStagingSmoke, type StagingSmokeMode } from "./programs/smokeStaging.js";
import { OpsRuntimeConfigLive } from "./runtime/config.js";

const usage = `Usage:
  tsx src/main.ts smoke:compile [--json]
  tsx src/main.ts compile:convex [--json]
  tsx src/main.ts reconcile:convex [--json]
  tsx src/main.ts smoke:security [--json] [--mode mock|convex]
  tsx src/main.ts smoke:settlement [--json] [--mode mock|convex]
  tsx src/main.ts smoke:staging [--json] [--mode mock|convex]
`;

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

const runSmokeCompile = (jsonOutput: boolean) => {
  const harness = makeInMemoryControlPlaneHarness({ paywalls: smokePaywalls });
  return compileAndPersistOnce({ requestId: "smoke:compile" }).pipe(
    Effect.provide(harness.layer),
    Effect.provide(ApertureConfigCompilerLive),
    Effect.flatMap((summary) => printCompileSummary(summary, jsonOutput)),
  );
};

const runConvexCompile = (jsonOutput: boolean) => {
  const controlPlaneLayer = ConvexControlPlaneLive.pipe(
    Layer.provideMerge(ConvexTransportLive),
    Layer.provideMerge(OpsRuntimeConfigLive),
  );
  const liveLayer = Layer.mergeAll(ApertureConfigCompilerLive, controlPlaneLayer);

  return compileAndPersistOnce({ requestId: "compile:convex" }).pipe(
    Effect.provide(liveLayer),
    Effect.flatMap((summary) => printCompileSummary(summary, jsonOutput)),
  );
};

const runConvexReconcile = (jsonOutput: boolean) => {
  return runStagingSmoke({ mode: "convex", requestId: "reconcile:convex" }).pipe(
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

const main = Effect.gen(function* () {
  const argv = process.argv.slice(2);
  const command = argv[0] ?? "";
  const jsonOutput = argv.includes("--json");

  if (command === "smoke:compile") {
    return yield* runSmokeCompile(jsonOutput);
  }

  if (command === "compile:convex") {
    return yield* runConvexCompile(jsonOutput);
  }

  if (command === "reconcile:convex") {
    return yield* runConvexReconcile(jsonOutput);
  }

  if (command === "smoke:settlement") {
    return yield* runSmokeSettlement(jsonOutput, parseMode(argv, ["mock", "convex"], "mock"));
  }

  if (command === "smoke:security") {
    return yield* runSmokeSecurity(jsonOutput, parseMode(argv, ["mock", "convex"], "mock"));
  }

  if (command === "smoke:staging") {
    return yield* runSmokeStaging(jsonOutput, parseMode(argv, ["mock", "convex"], "mock"));
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
