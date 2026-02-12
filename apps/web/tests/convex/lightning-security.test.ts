import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  activateCredentialRoleImpl,
  evaluateOwnerSecurityGate,
  getControlPlaneSecurityStateImpl,
  getOwnerSecurityStateImpl,
  revokeCredentialRoleImpl,
  rotateCredentialRoleImpl,
  setGlobalPauseImpl,
  setOwnerKillSwitchImpl,
} from "../../convex/lightning/security";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const mutationCtx = (db: any) => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.none()),
  },
});

const authedCtx = (db: any, subject: string) => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

describe("convex/lightning security controls", () => {
  it("supports global pause, owner kill switch precedence, and owner security state view", async () => {
    const db = makeInMemoryDb();
    const mutCtx = mutationCtx(db);
    const ownerCtx = authedCtx(db, "owner_1");
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const baseline = await run(getControlPlaneSecurityStateImpl(ownerCtx as any, { secret: "ops-secret" }));
      expect(baseline.global.globalPause).toBe(false);
      expect(baseline.ownerControls).toHaveLength(0);

      await run(
        setOwnerKillSwitchImpl(mutCtx, {
          secret: "ops-secret",
          ownerId: "owner_1",
          active: true,
          reason: "owner emergency stop",
        }),
      );

      await run(
        setGlobalPauseImpl(mutCtx, {
          secret: "ops-secret",
          active: true,
          reason: "global emergency stop",
        }),
      );

      const withGlobalPause = await run(evaluateOwnerSecurityGate(ownerCtx as any, "owner_1"));
      expect(withGlobalPause).toMatchObject({
        allowed: false,
        denyReasonCode: "global_pause_active",
      });

      await run(
        setGlobalPauseImpl(mutCtx, {
          secret: "ops-secret",
          active: false,
        }),
      );

      const withOwnerKill = await run(evaluateOwnerSecurityGate(ownerCtx as any, "owner_1"));
      expect(withOwnerKill).toMatchObject({
        allowed: false,
        denyReasonCode: "owner_kill_switch_active",
      });

      await run(
        setOwnerKillSwitchImpl(mutCtx, {
          secret: "ops-secret",
          ownerId: "owner_1",
          active: false,
        }),
      );

      const recovered = await run(evaluateOwnerSecurityGate(ownerCtx as any, "owner_1"));
      expect(recovered).toEqual({ allowed: true });

      const ownerView = await run(getOwnerSecurityStateImpl(ownerCtx as any));
      expect(ownerView.ownerId).toBe("owner_1");
      expect(ownerView.gate).toEqual({ allowed: true });
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });

  it("supports credential role rotation, revocation, and activation lifecycle", async () => {
    const db = makeInMemoryDb();
    const ctx = mutationCtx(db);
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const rotated = await run(
        rotateCredentialRoleImpl(ctx, {
          secret: "ops-secret",
          role: "gateway_invoice",
          fingerprint: "fp_rotate_1",
          note: "rotation started",
        }),
      );
      expect(rotated.role.status).toBe("rotating");
      expect(rotated.role.version).toBe(1);

      const revoked = await run(
        revokeCredentialRoleImpl(ctx, {
          secret: "ops-secret",
          role: "gateway_invoice",
          note: "revoked for incident",
        }),
      );
      expect(revoked.role.status).toBe("revoked");
      expect(revoked.role.version).toBe(1);
      expect(revoked.role.revokedAtMs).toEqual(expect.any(Number));

      const activated = await run(
        activateCredentialRoleImpl(ctx, {
          secret: "ops-secret",
          role: "gateway_invoice",
          fingerprint: "fp_active_1",
          note: "rotation recovered",
        }),
      );
      expect(activated.role.status).toBe("active");
      expect(activated.role.version).toBe(2);
      expect(activated.role.fingerprint).toBe("fp_active_1");

      const state = await run(getControlPlaneSecurityStateImpl(ctx as any, { secret: "ops-secret" }));
      expect(state.credentialRoles).toHaveLength(1);
      expect(state.credentialRoles[0]).toMatchObject({
        role: "gateway_invoice",
        status: "active",
        version: 2,
      });
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });
});
