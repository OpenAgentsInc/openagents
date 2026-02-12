import { Console, Effect, Layer } from "effect";

import { ApertureConfigCompilerLive } from "./compiler/apertureCompiler.js";
import { makeInMemoryControlPlaneHarness } from "./controlPlane/inMemory.js";
import { ConvexControlPlaneLive } from "./controlPlane/convex.js";
import { ConvexTransportLive } from "./controlPlane/convexTransport.js";
import { smokePaywalls } from "./fixtures/smokePaywalls.js";
import { compileAndPersistOnce } from "./programs/compileAndPersist.js";
import { OpsRuntimeConfigLive } from "./runtime/config.js";

const usage = `Usage:
  tsx src/main.ts smoke:compile [--json]
  tsx src/main.ts compile:convex [--json]
`;

const toSummaryJson = (summary: {
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

const runSmokeCompile = (jsonOutput: boolean) => {
  const harness = makeInMemoryControlPlaneHarness({ paywalls: smokePaywalls });
  return compileAndPersistOnce({ requestId: "smoke:compile" }).pipe(
    Effect.provide(harness.layer),
    Effect.provide(ApertureConfigCompilerLive),
    Effect.flatMap((summary) =>
      jsonOutput
        ? Console.log(toSummaryJson(summary))
        : Console.log(
            [
              `configHash=${summary.configHash}`,
              `ruleCount=${summary.ruleCount}`,
              `valid=${summary.valid}`,
              `deploymentStatus=${summary.deploymentStatus}`,
              `deploymentId=${summary.deploymentId}`,
            ].join("\n"),
          ),
    ),
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
    Effect.flatMap((summary) =>
      jsonOutput
        ? Console.log(toSummaryJson(summary))
        : Console.log(
            [
              `configHash=${summary.configHash}`,
              `ruleCount=${summary.ruleCount}`,
              `valid=${summary.valid}`,
              `deploymentStatus=${summary.deploymentStatus}`,
              `deploymentId=${summary.deploymentId}`,
            ].join("\n"),
          ),
    ),
  );
};

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
