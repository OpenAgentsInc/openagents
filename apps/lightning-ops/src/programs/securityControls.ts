import { Effect, Layer } from "effect";

import type {
  ControlPlaneCredentialRoleState,
  ControlPlaneOwnerSecurityControl,
  ControlPlaneSecurityGlobal,
  SecurityDenyReasonCode,
} from "../contracts.js";
import { ApiTransportLive } from "../controlPlane/apiTransport.js";
import { ConvexControlPlaneLive } from "../controlPlane/convex.js";
import { ConvexTransportLive } from "../controlPlane/convexTransport.js";
import { makeInMemoryControlPlaneHarness } from "../controlPlane/inMemory.js";
import { ControlPlaneService, type ControlPlaneSecurityState } from "../controlPlane/service.js";
import { OpsRuntimeConfigLive } from "../runtime/config.js";
import { validateCredentialRoleMap } from "../runtime/credentials.js";

export type SecuritySmokeMode = "mock" | "convex" | "api";

export type SecuritySmokeSummary = Readonly<{
  executionPath: "hosted-node";
  failClosed: {
    passed: boolean;
    errorTag?: string;
    errorCode?: string;
    role?: string;
    field?: string;
  };
  globalPause: {
    allowed: boolean;
    denyReasonCode?: SecurityDenyReasonCode;
  };
  ownerKillSwitch: {
    allowed: boolean;
    denyReasonCode?: SecurityDenyReasonCode;
  };
  recovery: {
    allowed: boolean;
  };
  credentialLifecycle: {
    rotatedVersion: number;
    revokedStatus: string;
    activatedStatus: string;
    activatedVersion: number;
  };
  statusSnapshot: {
    globalPauseActive: boolean;
    activeOwnerKillSwitches: number;
    credentialRoles: ReadonlyArray<{
      role: string;
      status: string;
      version: number;
    }>;
  };
}>;

const smokeOwnerId = "owner_security_smoke";

const evaluateSecurityGate = (
  ownerId: string,
  state: ControlPlaneSecurityState,
): Readonly<{ allowed: true } | { allowed: false; denyReasonCode: SecurityDenyReasonCode }> => {
  const global: ControlPlaneSecurityGlobal = state.global;
  if (global.globalPause) {
    return {
      allowed: false,
      denyReasonCode: "global_pause_active",
    };
  }

  const ownerControl: ControlPlaneOwnerSecurityControl | undefined = state.ownerControls.find(
    (row) => row.ownerId === ownerId,
  );
  if (ownerControl?.killSwitch) {
    return {
      allowed: false,
      denyReasonCode: "owner_kill_switch_active",
    };
  }

  return { allowed: true };
};

const mapRoleSummary = (roles: ReadonlyArray<ControlPlaneCredentialRoleState>) =>
  roles
    .slice()
    .sort((a, b) => a.role.localeCompare(b.role))
    .map((role) => ({
      role: role.role,
      status: role.status,
      version: role.version,
    }));

const runSecurityFlow = Effect.gen(function* () {
  const controlPlane = yield* ControlPlaneService;

  const failClosedAttempt = yield* Effect.either(
    validateCredentialRoleMap({
      gateway_invoice: "",
      settlement_read: "valid_settlement_credential_12345",
      operator_admin: "valid_operator_credential_12345",
    }),
  );

  const failClosed =
    failClosedAttempt._tag === "Left"
      ? {
          passed: failClosedAttempt.left._tag === "CredentialValidationError",
          errorTag: failClosedAttempt.left._tag,
          ...(failClosedAttempt.left._tag === "CredentialValidationError"
            ? {
                errorCode: failClosedAttempt.left.code,
                role: failClosedAttempt.left.role,
                field: failClosedAttempt.left.field,
              }
            : {}),
        }
      : {
          passed: false,
        };

  // Ensure role-validation can recover with valid credentials.
  yield* validateCredentialRoleMap({
    gateway_invoice: "valid_gateway_credential_12345",
    settlement_read: "valid_settlement_credential_12345",
    operator_admin: "valid_operator_credential_12345",
  });

  yield* controlPlane.setOwnerKillSwitch({
    ownerId: smokeOwnerId,
    active: true,
    reason: "owner kill switch smoke",
    updatedBy: "smoke:security",
  });
  yield* controlPlane.setGlobalPause({
    active: true,
    reason: "global pause smoke",
    updatedBy: "smoke:security",
  });

  const globalPausedState = yield* controlPlane.getSecurityState();
  const globalPauseGate = evaluateSecurityGate(smokeOwnerId, globalPausedState);

  yield* controlPlane.setGlobalPause({
    active: false,
    updatedBy: "smoke:security",
  });

  const ownerKillState = yield* controlPlane.getSecurityState();
  const ownerKillGate = evaluateSecurityGate(smokeOwnerId, ownerKillState);

  yield* controlPlane.setOwnerKillSwitch({
    ownerId: smokeOwnerId,
    active: false,
    updatedBy: "smoke:security",
  });

  const rotated = yield* controlPlane.rotateCredentialRole({
    role: "gateway_invoice",
    fingerprint: "fp_gateway_rotate_smoke",
    note: "rotation started",
  });
  const revoked = yield* controlPlane.revokeCredentialRole({
    role: "gateway_invoice",
    note: "rotation revoked",
  });
  const activated = yield* controlPlane.activateCredentialRole({
    role: "gateway_invoice",
    fingerprint: "fp_gateway_active_smoke",
    note: "rotation recovered",
  });

  const recoveredState = yield* controlPlane.getSecurityState();
  const recoveredGate = evaluateSecurityGate(smokeOwnerId, recoveredState);

  const summary: SecuritySmokeSummary = {
    executionPath: "hosted-node",
    failClosed,
    globalPause: {
      allowed: globalPauseGate.allowed,
      ...(globalPauseGate.allowed ? {} : { denyReasonCode: globalPauseGate.denyReasonCode }),
    },
    ownerKillSwitch: {
      allowed: ownerKillGate.allowed,
      ...(ownerKillGate.allowed ? {} : { denyReasonCode: ownerKillGate.denyReasonCode }),
    },
    recovery: {
      allowed: recoveredGate.allowed,
    },
    credentialLifecycle: {
      rotatedVersion: rotated.version,
      revokedStatus: revoked.status,
      activatedStatus: activated.status,
      activatedVersion: activated.version,
    },
    statusSnapshot: {
      globalPauseActive: recoveredState.global.globalPause,
      activeOwnerKillSwitches: recoveredState.ownerControls.filter((row) => row.killSwitch).length,
      credentialRoles: mapRoleSummary(recoveredState.credentialRoles),
    },
  };

  return summary;
});

const runMockSecuritySmoke = () => {
  const harness = makeInMemoryControlPlaneHarness();
  return runSecurityFlow.pipe(Effect.provide(harness.layer));
};

const runConvexSecuritySmoke = () => {
  const controlPlaneLayer = ConvexControlPlaneLive.pipe(
    Layer.provideMerge(ConvexTransportLive),
    Layer.provideMerge(OpsRuntimeConfigLive),
  );
  return runSecurityFlow.pipe(Effect.provide(controlPlaneLayer));
};

const runApiSecuritySmoke = () => {
  const controlPlaneLayer = ConvexControlPlaneLive.pipe(
    Layer.provideMerge(ApiTransportLive),
    Layer.provideMerge(OpsRuntimeConfigLive),
  );
  return runSecurityFlow.pipe(Effect.provide(controlPlaneLayer));
};

export const runSecuritySmoke = (input?: {
  readonly mode?: SecuritySmokeMode;
}) => {
  const mode = input?.mode ?? "mock";
  if (mode === "convex") return runConvexSecuritySmoke();
  if (mode === "api") return runApiSecuritySmoke();
  return runMockSecuritySmoke();
};
