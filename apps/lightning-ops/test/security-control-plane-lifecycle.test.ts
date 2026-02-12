import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { ControlPlaneService } from "../src/controlPlane/service.js";
import { makeInMemoryControlPlaneHarness } from "../src/controlPlane/inMemory.js";

describe("lightning-ops security control-plane lifecycle", () => {
  it.effect("runs rotation/revocation/activation lifecycle and global-owner controls", () => {
    const harness = makeInMemoryControlPlaneHarness();

    return Effect.gen(function* () {
      const controlPlane = yield* ControlPlaneService;

      yield* controlPlane.setGlobalPause({
        active: true,
        reason: "global incident",
      });
      yield* controlPlane.setOwnerKillSwitch({
        ownerId: "owner_1",
        active: true,
        reason: "owner incident",
      });

      const rotated = yield* controlPlane.rotateCredentialRole({
        role: "gateway_invoice",
        fingerprint: "fp_rotate_1",
        note: "rotate start",
      });
      const revoked = yield* controlPlane.revokeCredentialRole({
        role: "gateway_invoice",
        note: "incident revoke",
      });
      const activated = yield* controlPlane.activateCredentialRole({
        role: "gateway_invoice",
        fingerprint: "fp_active_1",
        note: "recovered",
      });

      yield* controlPlane.setGlobalPause({ active: false });
      yield* controlPlane.setOwnerKillSwitch({
        ownerId: "owner_1",
        active: false,
      });

      const state = yield* controlPlane.getSecurityState();
      expect(state.global.globalPause).toBe(false);
      expect(state.ownerControls.find((row) => row.ownerId === "owner_1")?.killSwitch).toBe(false);
      expect(rotated.status).toBe("rotating");
      expect(revoked.status).toBe("revoked");
      expect(activated.status).toBe("active");
      expect(activated.version).toBeGreaterThanOrEqual(2);
    }).pipe(Effect.provide(harness.layer));
  });
});
