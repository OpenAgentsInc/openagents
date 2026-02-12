import { Effect } from "effect";

import type { CompiledRunSummary } from "../contracts.js";
import { ApertureCompileValidationError } from "../errors.js";

import { ApertureConfigCompilerService } from "../compiler/service.js";
import { ControlPlaneService } from "../controlPlane/service.js";

export const compileAndPersistOnce = (input?: { readonly requestId?: string }) =>
  Effect.gen(function* () {
    const controlPlane = yield* ControlPlaneService;
    const compiler = yield* ApertureConfigCompilerService;

    const paywalls = yield* controlPlane.listPaywallsForCompile();
    const snapshotHash = compiler.snapshotHash(paywalls);

    const compiledAttempt = yield* Effect.either(compiler.compile(paywalls));

    if (compiledAttempt._tag === "Right") {
      const compiled = compiledAttempt.right;
      const requestIdField = input?.requestId ? { requestId: input.requestId } : {};
      const deployment = yield* controlPlane.recordDeploymentIntent({
        configHash: compiled.configHash,
        status: "pending",
        diagnostics: compiled.diagnostics,
        ...requestIdField,
        metadata: {
          executionPath: "hosted-node",
          ruleCount: compiled.ruleCount,
          valid: compiled.valid,
        },
      });

      const summary: CompiledRunSummary = {
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        deploymentStatus: deployment.status,
        deploymentId: deployment.deploymentId,
      };
      return summary;
    }

    if (compiledAttempt.left._tag !== "ApertureCompileValidationError") {
      return yield* Effect.fail(compiledAttempt.left);
    }

    const failureDiagnostics = (compiledAttempt.left as ApertureCompileValidationError).diagnostics;
    const requestIdField = input?.requestId ? { requestId: input.requestId } : {};
    const deployment = yield* controlPlane.recordDeploymentIntent({
      configHash: snapshotHash,
      status: "failed",
      diagnostics: failureDiagnostics,
      ...requestIdField,
      metadata: {
        executionPath: "hosted-node",
        ruleCount: 0,
        valid: false,
      },
    });

    const failedSummary: CompiledRunSummary = {
      configHash: snapshotHash,
      ruleCount: 0,
      valid: false,
      diagnostics: failureDiagnostics,
      deploymentStatus: deployment.status,
      deploymentId: deployment.deploymentId,
    };

    return failedSummary;
  });
