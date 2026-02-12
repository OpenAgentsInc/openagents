import { Console, Effect, Layer } from "effect";

import { ApertureConfigCompilerLive } from "./compiler/apertureCompiler.js";
import { makeInMemoryControlPlaneHarness } from "./controlPlane/inMemory.js";
import { ConvexControlPlaneLive } from "./controlPlane/convex.js";
import { ConvexTransportLive } from "./controlPlane/convexTransport.js";
import { smokePaywalls } from "./fixtures/smokePaywalls.js";
import { compileAndPersistOnce } from "./programs/compileAndPersist.js";
import { runStagingSmoke, type StagingSmokeMode } from "./programs/smokeStaging.js";
import { OpsRuntimeConfigLive } from "./runtime/config.js";

const usage = `Usage:
  tsx src/main.ts smoke:compile [--json]
  tsx src/main.ts compile:convex [--json]
  tsx src/main.ts reconcile:convex [--json]
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

const parseMode = (argv: ReadonlyArray<string>): StagingSmokeMode => {
  const modeFlag = argv.find((value) => value.startsWith("--mode="));
  if (modeFlag) {
    const value = modeFlag.slice("--mode=".length);
    return value === "convex" ? "convex" : "mock";
  }

  const modeIndex = argv.findIndex((value) => value === "--mode");
  if (modeIndex >= 0) {
    const next = argv[modeIndex + 1];
    return next === "convex" ? "convex" : "mock";
  }

  return "mock";
};

const runSmokeStaging = (jsonOutput: boolean, mode: StagingSmokeMode) =>
  runStagingSmoke({ mode }).pipe(Effect.flatMap((summary) => printReconcileSummary(summary, jsonOutput)));

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

  if (command === "smoke:staging") {
    return yield* runSmokeStaging(jsonOutput, parseMode(argv));
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
