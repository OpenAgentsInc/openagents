import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runSecuritySmoke } from "../src/programs/securityControls.js";

describe("lightning-ops smoke:security", () => {
  it.effect("verifies fail-closed, pause, owner kill switch, and recovery paths", () =>
    Effect.gen(function* () {
      const summary = yield* runSecuritySmoke({ mode: "mock" });

      expect(summary.executionPath).toBe("hosted-node");
      expect(summary.failClosed.passed).toBe(true);
      expect(summary.failClosed.errorCode).toBe("missing_credential_role");

      expect(summary.globalPause.allowed).toBe(false);
      expect(summary.globalPause.denyReasonCode).toBe("global_pause_active");

      expect(summary.ownerKillSwitch.allowed).toBe(false);
      expect(summary.ownerKillSwitch.denyReasonCode).toBe("owner_kill_switch_active");

      expect(summary.recovery.allowed).toBe(true);
      expect(summary.credentialLifecycle.revokedStatus).toBe("revoked");
      expect(summary.credentialLifecycle.activatedStatus).toBe("active");
      expect(summary.statusSnapshot.globalPauseActive).toBe(false);
    }),
  );
});
