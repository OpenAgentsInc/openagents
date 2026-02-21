import { Effect, Ref } from "effect";
import { describe, expect, it } from "@effect/vitest";

import type { ControlPlaneCredentialRoleState } from "../src/contracts.js";
import {
  CONTROL_PLANE_ACTIVATE_CREDENTIAL_ROLE_FN,
  CONTROL_PLANE_GET_SECURITY_STATE_FN,
  CONTROL_PLANE_REVOKE_CREDENTIAL_ROLE_FN,
  CONTROL_PLANE_ROTATE_CREDENTIAL_ROLE_FN,
  CONTROL_PLANE_SET_GLOBAL_PAUSE_FN,
  CONTROL_PLANE_SET_OWNER_KILL_SWITCH_FN,
  ControlPlaneLive,
} from "../src/controlPlane/live.js";
import { makeControlPlaneTransportTestLayer } from "../src/controlPlane/transport.js";
import { ControlPlaneService } from "../src/controlPlane/service.js";
import { ControlPlaneTransportError } from "../src/errors.js";
import { makeOpsRuntimeConfigTestLayer } from "../src/runtime/config.js";

describe("lightning-ops security control-plane transport pipeline", () => {
  it.effect("maps security lifecycle operations to control-plane transport functions", () =>
    Effect.gen(function* () {
      const callsRef = yield* Ref.make<Array<{ fn: string; args: Record<string, unknown> }>>([]);
      const roleStateRef = yield* Ref.make<ControlPlaneCredentialRoleState>({
        role: "gateway_invoice" as const,
        status: "active" as const,
        version: 1,
        fingerprint: "fp_bootstrap_1",
        updatedAtMs: 1_733_000_000_000,
      });
      const globalPauseRef = yield* Ref.make(false);
      const ownerKillRef = yield* Ref.make(false);

      const transportLayer = makeControlPlaneTransportTestLayer({
        query: (functionName, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { fn: functionName, args }]);
            if (functionName !== CONTROL_PLANE_GET_SECURITY_STATE_FN) {
              return yield* Effect.fail(
                ControlPlaneTransportError.make({
                  operation: functionName,
                  reason: "query_not_expected",
                }),
              );
            }

            const role = yield* Ref.get(roleStateRef);
            const globalPause = yield* Ref.get(globalPauseRef);
            const ownerKill = yield* Ref.get(ownerKillRef);
            return {
              ok: true,
              global: {
                stateId: "global",
                globalPause,
                ...(globalPause
                  ? {
                      denyReasonCode: "global_pause_active",
                      denyReason: "global pause",
                    }
                  : {}),
                updatedAtMs: 1_733_000_000_001,
              },
              ownerControls: ownerKill
                ? [
                    {
                      ownerId: "owner_1",
                      killSwitch: true,
                      denyReasonCode: "owner_kill_switch_active",
                      denyReason: "owner kill",
                      updatedAtMs: 1_733_000_000_002,
                    },
                  ]
                : [],
              credentialRoles: [role],
            };
          }),
        mutation: (functionName, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { fn: functionName, args }]);

            if (functionName === CONTROL_PLANE_SET_GLOBAL_PAUSE_FN) {
              const active = Boolean(args.active);
              yield* Ref.set(globalPauseRef, active);
              return {
                ok: true,
                global: {
                  stateId: "global",
                  globalPause: active,
                  ...(active
                    ? {
                        denyReasonCode: "global_pause_active",
                        denyReason: "global pause",
                      }
                    : {}),
                  updatedAtMs: 1_733_000_000_010,
                },
              };
            }

            if (functionName === CONTROL_PLANE_SET_OWNER_KILL_SWITCH_FN) {
              const active = Boolean(args.active);
              yield* Ref.set(ownerKillRef, active);
              return {
                ok: true,
                ownerControl: {
                  ownerId: String(args.ownerId),
                  killSwitch: active,
                  ...(active
                    ? {
                        denyReasonCode: "owner_kill_switch_active",
                        denyReason: "owner kill",
                      }
                    : {}),
                  updatedAtMs: 1_733_000_000_011,
                },
              };
            }

            if (functionName === CONTROL_PLANE_ROTATE_CREDENTIAL_ROLE_FN) {
              const current = yield* Ref.get(roleStateRef);
              const next = {
                ...current,
                status: "rotating" as const,
                version: current.version + 1,
                fingerprint: String(args.fingerprint ?? "fp_rotate"),
                updatedAtMs: 1_733_000_000_020,
                lastRotatedAtMs: 1_733_000_000_020,
              };
              yield* Ref.set(roleStateRef, next);
              return { ok: true, role: next };
            }

            if (functionName === CONTROL_PLANE_REVOKE_CREDENTIAL_ROLE_FN) {
              const current = yield* Ref.get(roleStateRef);
              const next = {
                ...current,
                status: "revoked" as const,
                updatedAtMs: 1_733_000_000_021,
                revokedAtMs: 1_733_000_000_021,
              };
              yield* Ref.set(roleStateRef, next);
              return { ok: true, role: next };
            }

            if (functionName === CONTROL_PLANE_ACTIVATE_CREDENTIAL_ROLE_FN) {
              const current = yield* Ref.get(roleStateRef);
              const next = {
                ...current,
                status: "active" as const,
                version: current.version + 1,
                fingerprint: String(args.fingerprint ?? "fp_active"),
                updatedAtMs: 1_733_000_000_022,
                lastRotatedAtMs: 1_733_000_000_022,
              };
              yield* Ref.set(roleStateRef, next);
              return { ok: true, role: next };
            }

            return yield* Effect.fail(
              ControlPlaneTransportError.make({
                operation: functionName,
                reason: "mutation_not_expected",
              }),
            );
          }),
      });

      const summary = yield* Effect.gen(function* () {
        const controlPlane = yield* ControlPlaneService;
        const initial = yield* controlPlane.getSecurityState();
        yield* controlPlane.setGlobalPause({ active: true });
        yield* controlPlane.setOwnerKillSwitch({ ownerId: "owner_1", active: true });
        const rotated = yield* controlPlane.rotateCredentialRole({
          role: "gateway_invoice",
          fingerprint: "fp_rotate_1",
        });
        const revoked = yield* controlPlane.revokeCredentialRole({
          role: "gateway_invoice",
        });
        const activated = yield* controlPlane.activateCredentialRole({
          role: "gateway_invoice",
          fingerprint: "fp_active_1",
        });
        yield* controlPlane.setGlobalPause({ active: false });
        yield* controlPlane.setOwnerKillSwitch({ ownerId: "owner_1", active: false });
        const recovered = yield* controlPlane.getSecurityState();

        return {
          initial,
          rotated,
          revoked,
          activated,
          recovered,
        };
      }).pipe(
        Effect.provide(ControlPlaneLive),
        Effect.provide(transportLayer),
        Effect.provide(
          makeOpsRuntimeConfigTestLayer({
            opsSecret: "ops-secret",
          }),
        ),
      );

      const calls = yield* Ref.get(callsRef);
      expect(summary.initial.global.globalPause).toBe(false);
      expect(summary.rotated.status).toBe("rotating");
      expect(summary.revoked.status).toBe("revoked");
      expect(summary.activated.status).toBe("active");
      expect(summary.activated.version).toBeGreaterThan(summary.rotated.version);
      expect(summary.recovered.global.globalPause).toBe(false);
      expect(summary.recovered.ownerControls).toHaveLength(0);
      expect(calls.map((call) => call.fn)).toContain(CONTROL_PLANE_SET_GLOBAL_PAUSE_FN);
      expect(calls.map((call) => call.fn)).toContain(CONTROL_PLANE_SET_OWNER_KILL_SWITCH_FN);
      expect(calls.map((call) => call.fn)).toContain(CONTROL_PLANE_ROTATE_CREDENTIAL_ROLE_FN);
      expect(calls.map((call) => call.fn)).toContain(CONTROL_PLANE_REVOKE_CREDENTIAL_ROLE_FN);
      expect(calls.map((call) => call.fn)).toContain(CONTROL_PLANE_ACTIVATE_CREDENTIAL_ROLE_FN);
    }),
  );
});
