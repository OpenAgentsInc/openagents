import { Effect } from "effect";

import type {
  CompiledApertureArtifact,
  CompiledApertureRule,
  ReconcileFailureCode,
  ReconcileRunSummary,
} from "../contracts.js";
import { ApertureCompileValidationError, GatewayRuntimeError } from "../errors.js";

import { ApertureConfigCompilerService } from "../compiler/service.js";
import { ControlPlaneService } from "../controlPlane/service.js";
import { GatewayService, type GatewayDeploymentSnapshot } from "../gateway/service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];
type GatewayApi = Parameters<typeof GatewayService.of>[0];

const mapGatewayStageToFailureCode = (
  stage: GatewayRuntimeError["stage"],
): ReconcileFailureCode => {
  switch (stage) {
    case "active_lookup":
      return "active_lookup_failed";
    case "apply":
      return "deploy_apply_failed";
    case "health":
      return "health_check_failed";
    case "challenge":
      return "challenge_check_failed";
    case "proxy":
      return "proxy_check_failed";
    case "rollback":
      return "rollback_failed";
  }
};

type ProgressFlags = {
  readonly healthOk: boolean;
  readonly challengeOk: boolean;
  readonly proxyOk: boolean;
};

const correlationMetadata = (input: {
  readonly requestId: string;
  readonly deploymentId: string;
  readonly configHash: string;
  readonly failureCode?: ReconcileFailureCode | undefined;
  readonly failureStage?: GatewayRuntimeError["stage"] | undefined;
  readonly failureReason?: string | undefined;
  readonly rolledBackFrom?: string | undefined;
  readonly progress: ProgressFlags;
}) => ({
  executionPath: "hosted-node" as const,
  correlation: {
    requestId: input.requestId,
    deploymentId: input.deploymentId,
    configHash: input.configHash,
    paritySchemaVersion: "l402_run_artifact_v1",
    parityKeys: ["executionPath", "requestId", "deploymentId", "configHash"],
  },
  progress: {
    healthOk: input.progress.healthOk,
    challengeOk: input.progress.challengeOk,
    proxyOk: input.progress.proxyOk,
  },
  failure: input.failureCode
    ? {
        code: input.failureCode,
        stage: input.failureStage,
        reason: input.failureReason,
      }
    : undefined,
  rollback: input.rolledBackFrom
    ? {
        rolledBackFrom: input.rolledBackFrom,
      }
    : undefined,
});

const probeFromCompiled = (compiled: CompiledApertureArtifact): CompiledApertureRule | null =>
  compiled.rules[0] ?? null;

const deploymentIdFromRequestId = (requestId: string, prefix: string): string => {
  const normalized = requestId.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 90);
  return `${prefix}_${normalized || "run"}`;
};

const recordEventForProbe = (input: {
  readonly controlPlane: ControlPlaneApi;
  readonly probeRoute: CompiledApertureRule | null;
  readonly eventType: string;
  readonly level: "info" | "warn" | "error";
  readonly requestId: string;
  readonly deploymentId: string;
  readonly configHash: string;
  readonly metadata?: unknown;
}) => {
  if (!input.probeRoute) return Effect.void;
  return input.controlPlane.recordGatewayEvent({
    paywallId: input.probeRoute.paywallId,
    ownerId: input.probeRoute.ownerId,
    eventType: input.eventType,
    level: input.level,
    requestId: input.requestId,
    deploymentId: input.deploymentId,
    configHash: input.configHash,
    metadata: {
      executionPath: "hosted-node",
      routeId: input.probeRoute.id,
      ...(input.metadata !== undefined ? { details: input.metadata } : {}),
    },
  }).pipe(Effect.asVoid);
};

const buildSummary = (input: {
  readonly requestId: string;
  readonly configHash: string;
  readonly ruleCount: number;
  readonly valid: boolean;
  readonly diagnostics: ReconcileRunSummary["diagnostics"];
  readonly deploymentStatus: ReconcileRunSummary["deploymentStatus"];
  readonly deploymentId: string;
  readonly progress: ProgressFlags;
  readonly failureCode?: ReconcileFailureCode | undefined;
  readonly imageDigest?: string | undefined;
  readonly rolledBackFrom?: string | undefined;
}): ReconcileRunSummary => ({
  requestId: input.requestId,
  executionPath: "hosted-node",
  configHash: input.configHash,
  ruleCount: input.ruleCount,
  valid: input.valid,
  diagnostics: input.diagnostics,
  deploymentStatus: input.deploymentStatus,
  deploymentId: input.deploymentId,
  ...(input.failureCode ? { failureCode: input.failureCode } : {}),
  ...(input.imageDigest ? { imageDigest: input.imageDigest } : {}),
  ...(input.rolledBackFrom ? { rolledBackFrom: input.rolledBackFrom } : {}),
  healthOk: input.progress.healthOk,
  challengeOk: input.progress.challengeOk,
  proxyOk: input.progress.proxyOk,
});

const resolveRuntimeFailure = (input: {
  readonly defaultCode: ReconcileFailureCode;
  readonly error: unknown;
}) => {
  if (input.error && typeof input.error === "object" && "_tag" in input.error) {
    const tagged = input.error as { readonly _tag: string };
    if (tagged._tag === "GatewayRuntimeError") {
      const runtimeError = input.error as GatewayRuntimeError;
      return {
        code: mapGatewayStageToFailureCode(runtimeError.stage),
        stage: runtimeError.stage,
        reason: runtimeError.reason,
      } as const;
    }
  }
  return {
    code: input.defaultCode,
    reason: String(input.error),
  } as const;
};

const rollbackIfPossible = (input: {
  readonly gateway: GatewayApi;
  readonly controlPlane: ControlPlaneApi;
  readonly previousDeployment: GatewayDeploymentSnapshot | null;
  readonly requestId: string;
  readonly deploymentId: string;
  readonly configHash: string;
  readonly ruleCount: number;
  readonly valid: boolean;
  readonly diagnostics: ReconcileRunSummary["diagnostics"];
  readonly probeRoute: CompiledApertureRule | null;
  readonly failureCode: ReconcileFailureCode;
  readonly failureStage?: GatewayRuntimeError["stage"] | undefined;
  readonly failureReason: string;
  readonly progress: ProgressFlags;
}) =>
  Effect.gen(function* () {
    if (!input.previousDeployment) {
      const failed = yield* input.controlPlane.recordDeploymentIntent({
        deploymentId: input.deploymentId,
        configHash: input.configHash,
        status: "failed",
        diagnostics: input.diagnostics,
        requestId: input.requestId,
        metadata: correlationMetadata({
          requestId: input.requestId,
          deploymentId: input.deploymentId,
          configHash: input.configHash,
          failureCode: input.failureCode,
          failureStage: input.failureStage,
          failureReason: input.failureReason,
          progress: input.progress,
        }),
      });

      yield* recordEventForProbe({
        controlPlane: input.controlPlane,
        probeRoute: input.probeRoute,
        eventType: `gateway_reconcile_failed_${input.failureCode}`,
        level: "error",
        requestId: input.requestId,
        deploymentId: input.deploymentId,
        configHash: input.configHash,
        metadata: {
          failureReason: input.failureReason,
          failureStage: input.failureStage,
        },
      });

      return buildSummary({
        requestId: input.requestId,
        configHash: input.configHash,
        ruleCount: input.ruleCount,
        valid: input.valid,
        diagnostics: input.diagnostics,
        deploymentStatus: "failed",
        deploymentId: failed.deploymentId,
        failureCode: input.failureCode,
        progress: input.progress,
      });
    }

    const rollbackAttempt = yield* Effect.either(
      input.gateway.rollbackTo({
        requestId: input.requestId,
        deploymentId: input.deploymentId,
        target: input.previousDeployment,
      }),
    );

    if (rollbackAttempt._tag === "Right") {
      const rollbackDeployment = rollbackAttempt.right;
      const rolledBack = yield* input.controlPlane.recordDeploymentIntent({
        deploymentId: input.deploymentId,
        configHash: input.configHash,
        ...(rollbackDeployment.imageDigest ? { imageDigest: rollbackDeployment.imageDigest } : {}),
        status: "rolled_back",
        diagnostics: input.diagnostics,
        requestId: input.requestId,
        rolledBackFrom: input.previousDeployment.configHash,
        metadata: correlationMetadata({
          requestId: input.requestId,
          deploymentId: input.deploymentId,
          configHash: input.configHash,
          failureCode: input.failureCode,
          failureStage: input.failureStage,
          failureReason: input.failureReason,
          rolledBackFrom: input.previousDeployment.configHash,
          progress: input.progress,
        }),
      });

      yield* recordEventForProbe({
        controlPlane: input.controlPlane,
        probeRoute: input.probeRoute,
        eventType: "gateway_reconcile_rolled_back",
        level: "warn",
        requestId: input.requestId,
        deploymentId: input.deploymentId,
        configHash: input.configHash,
        metadata: {
          failureCode: input.failureCode,
          failureReason: input.failureReason,
          rolledBackFrom: input.previousDeployment.configHash,
        },
      });

      return buildSummary({
        requestId: input.requestId,
        configHash: input.configHash,
        ruleCount: input.ruleCount,
        valid: input.valid,
        diagnostics: input.diagnostics,
        deploymentStatus: "rolled_back",
        deploymentId: rolledBack.deploymentId,
        failureCode: input.failureCode,
        imageDigest: rollbackDeployment.imageDigest,
        rolledBackFrom: input.previousDeployment.configHash,
        progress: input.progress,
      });
    }

    const rollbackFailure = resolveRuntimeFailure({
      defaultCode: "rollback_failed",
      error: rollbackAttempt.left,
    });

    const failed = yield* input.controlPlane.recordDeploymentIntent({
      deploymentId: input.deploymentId,
      configHash: input.configHash,
      status: "failed",
      diagnostics: input.diagnostics,
      requestId: input.requestId,
      metadata: correlationMetadata({
        requestId: input.requestId,
        deploymentId: input.deploymentId,
        configHash: input.configHash,
        failureCode: rollbackFailure.code,
        failureStage: rollbackFailure.stage,
        failureReason: rollbackFailure.reason,
        progress: input.progress,
      }),
    });

    yield* recordEventForProbe({
      controlPlane: input.controlPlane,
      probeRoute: input.probeRoute,
      eventType: "gateway_reconcile_failed_rollback_failed",
      level: "error",
      requestId: input.requestId,
      deploymentId: input.deploymentId,
      configHash: input.configHash,
      metadata: {
        failureCode: input.failureCode,
        failureReason: input.failureReason,
        rollbackFailure: rollbackFailure.reason,
      },
    });

    return buildSummary({
      requestId: input.requestId,
      configHash: input.configHash,
      ruleCount: input.ruleCount,
      valid: input.valid,
      diagnostics: input.diagnostics,
      deploymentStatus: "failed",
      deploymentId: failed.deploymentId,
      failureCode: rollbackFailure.code,
      progress: input.progress,
    });
  });

export const reconcileAndDeployOnce = (input?: { readonly requestId?: string }) =>
  Effect.gen(function* () {
    const controlPlane = yield* ControlPlaneService;
    const compiler = yield* ApertureConfigCompilerService;
    const gateway = yield* GatewayService;

    const requestId = input?.requestId ?? `reconcile:${Date.now()}`;
    const paywalls = yield* controlPlane.listPaywallsForCompile();
    const snapshotHash = compiler.snapshotHash(paywalls);
    const failureDeploymentId = deploymentIdFromRequestId(requestId, "dep_compile");

    const compiledAttempt = yield* Effect.either(compiler.compile(paywalls));
    if (compiledAttempt._tag === "Left") {
      if (compiledAttempt.left._tag !== "ApertureCompileValidationError") {
        return yield* Effect.fail(compiledAttempt.left);
      }

      const compileFailure = compiledAttempt.left as ApertureCompileValidationError;
      const failed = yield* controlPlane.recordDeploymentIntent({
        deploymentId: failureDeploymentId,
        configHash: snapshotHash,
        status: "failed",
        diagnostics: compileFailure.diagnostics,
        requestId,
        metadata: correlationMetadata({
          requestId,
          deploymentId: failureDeploymentId,
          configHash: snapshotHash,
          failureCode: "compile_validation_failed",
          failureReason: "compile_validation_failed",
          progress: {
            healthOk: false,
            challengeOk: false,
            proxyOk: false,
          },
        }),
      });

      return buildSummary({
        requestId,
        configHash: snapshotHash,
        ruleCount: 0,
        valid: false,
        diagnostics: compileFailure.diagnostics,
        deploymentStatus: "failed",
        deploymentId: failed.deploymentId,
        failureCode: "compile_validation_failed",
        progress: {
          healthOk: false,
          challengeOk: false,
          proxyOk: false,
        },
      });
    }

    const compiled = compiledAttempt.right;
    const probeRoute = probeFromCompiled(compiled);
    const deploymentId = deploymentIdFromRequestId(requestId, "dep_reconcile");

    const pending = yield* controlPlane.recordDeploymentIntent({
      deploymentId,
      configHash: compiled.configHash,
      status: "pending",
      diagnostics: compiled.diagnostics,
      requestId,
      metadata: correlationMetadata({
        requestId,
        deploymentId,
        configHash: compiled.configHash,
        progress: {
          healthOk: false,
          challengeOk: false,
          proxyOk: false,
        },
      }),
    });

    const previousDeploymentAttempt = yield* Effect.either(gateway.getActiveDeployment());
    if (previousDeploymentAttempt._tag === "Left") {
      const failure = resolveRuntimeFailure({
        defaultCode: "active_lookup_failed",
        error: previousDeploymentAttempt.left,
      });

      return yield* rollbackIfPossible({
        gateway,
        controlPlane,
        previousDeployment: null,
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        probeRoute,
        failureCode: failure.code,
        failureStage: failure.stage,
        failureReason: failure.reason,
        progress: {
          healthOk: false,
          challengeOk: false,
          proxyOk: false,
        },
      });
    }

    const previousDeployment = previousDeploymentAttempt.right;

    const applyAttempt = yield* Effect.either(
      gateway.applyConfig({
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        apertureYaml: compiled.apertureYaml,
      }),
    );

    if (applyAttempt._tag === "Left") {
      const failure = resolveRuntimeFailure({
        defaultCode: "deploy_apply_failed",
        error: applyAttempt.left,
      });

      return yield* rollbackIfPossible({
        gateway,
        controlPlane,
        previousDeployment,
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        probeRoute,
        failureCode: failure.code,
        failureStage: failure.stage,
        failureReason: failure.reason,
        progress: {
          healthOk: false,
          challengeOk: false,
          proxyOk: false,
        },
      });
    }

    const appliedDeployment = applyAttempt.right;

    const healthAttempt = yield* Effect.either(
      gateway.checkHealth({ requestId, deploymentId: pending.deploymentId }),
    );
    if (healthAttempt._tag === "Left") {
      const failure = resolveRuntimeFailure({
        defaultCode: "health_check_failed",
        error: healthAttempt.left,
      });

      return yield* rollbackIfPossible({
        gateway,
        controlPlane,
        previousDeployment,
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        probeRoute,
        failureCode: failure.code,
        failureStage: failure.stage,
        failureReason: failure.reason,
        progress: {
          healthOk: false,
          challengeOk: false,
          proxyOk: false,
        },
      });
    }

    yield* recordEventForProbe({
      controlPlane,
      probeRoute,
      eventType: "gateway_reconcile_health_ok",
      level: "info",
      requestId,
      deploymentId: pending.deploymentId,
      configHash: compiled.configHash,
      metadata: {
        statusCode: healthAttempt.right.statusCode,
      },
    });

    const challengeAttempt = yield* Effect.either(
      probeRoute
        ? gateway.checkChallenge({
            requestId,
            deploymentId: pending.deploymentId,
            probeRoute,
          })
        : Effect.fail(
            GatewayRuntimeError.make({
              stage: "challenge",
              reason: "missing_probe_route",
            }),
          ),
    );

    if (challengeAttempt._tag === "Left") {
      const failure = resolveRuntimeFailure({
        defaultCode: "challenge_check_failed",
        error: challengeAttempt.left,
      });

      return yield* rollbackIfPossible({
        gateway,
        controlPlane,
        previousDeployment,
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        probeRoute,
        failureCode: failure.code,
        failureStage: failure.stage,
        failureReason: failure.reason,
        progress: {
          healthOk: true,
          challengeOk: false,
          proxyOk: false,
        },
      });
    }

    yield* recordEventForProbe({
      controlPlane,
      probeRoute,
      eventType: "gateway_reconcile_challenge_ok",
      level: "info",
      requestId,
      deploymentId: pending.deploymentId,
      configHash: compiled.configHash,
      metadata: {
        statusCode: challengeAttempt.right.statusCode,
      },
    });

    const proxyAttempt = yield* Effect.either(
      probeRoute
        ? gateway.checkProxy({
            requestId,
            deploymentId: pending.deploymentId,
            probeRoute,
            authorizationHeader: challengeAttempt.right.authorizationHeader,
          })
        : Effect.fail(
            GatewayRuntimeError.make({
              stage: "proxy",
              reason: "missing_probe_route",
            }),
          ),
    );

    if (proxyAttempt._tag === "Left") {
      const failure = resolveRuntimeFailure({
        defaultCode: "proxy_check_failed",
        error: proxyAttempt.left,
      });

      return yield* rollbackIfPossible({
        gateway,
        controlPlane,
        previousDeployment,
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        ruleCount: compiled.ruleCount,
        valid: compiled.valid,
        diagnostics: compiled.diagnostics,
        probeRoute,
        failureCode: failure.code,
        failureStage: failure.stage,
        failureReason: failure.reason,
        progress: {
          healthOk: true,
          challengeOk: true,
          proxyOk: false,
        },
      });
    }

    yield* recordEventForProbe({
      controlPlane,
      probeRoute,
      eventType: "gateway_reconcile_proxy_ok",
      level: "info",
      requestId,
      deploymentId: pending.deploymentId,
      configHash: compiled.configHash,
      metadata: {
        statusCode: proxyAttempt.right.statusCode,
      },
    });

    const applied = yield* controlPlane.recordDeploymentIntent({
      deploymentId: pending.deploymentId,
      configHash: compiled.configHash,
      ...(appliedDeployment.imageDigest ? { imageDigest: appliedDeployment.imageDigest } : {}),
      status: "applied",
      diagnostics: compiled.diagnostics,
      requestId,
      appliedAtMs: Date.now(),
      metadata: correlationMetadata({
        requestId,
        deploymentId: pending.deploymentId,
        configHash: compiled.configHash,
        progress: {
          healthOk: true,
          challengeOk: true,
          proxyOk: true,
        },
      }),
    });

    yield* recordEventForProbe({
      controlPlane,
      probeRoute,
      eventType: "gateway_reconcile_applied",
      level: "info",
      requestId,
      deploymentId: pending.deploymentId,
      configHash: compiled.configHash,
      metadata: {
        imageDigest: appliedDeployment.imageDigest,
      },
    });

    return buildSummary({
      requestId,
      configHash: compiled.configHash,
      ruleCount: compiled.ruleCount,
      valid: compiled.valid,
      diagnostics: compiled.diagnostics,
      deploymentStatus: "applied",
      deploymentId: applied.deploymentId,
      imageDigest: appliedDeployment.imageDigest,
      progress: {
        healthOk: true,
        challengeOk: true,
        proxyOk: true,
      },
    });
  });
