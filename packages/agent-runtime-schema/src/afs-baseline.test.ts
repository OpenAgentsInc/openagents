import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { RouteDecision, SafeTurnProjection } from "./index.js";
import {
  afsBaselineSafeProjectionFixtures,
  explicitProviderProjectionFixture,
  explicitProviderRouteDecisionFixture,
  localAnswerProjectionFixture,
  localAnswerRouteDecisionFixture,
  unavailableProviderRouteDecisionFixture,
} from "./afs-baseline-fixtures.js";

/**
 * The three surface decoders share one schema. Decoding on Desktop, web, and
 * mobile must reach equivalent facts. These decoders are independent decode
 * calls that stand in for the three surfaces.
 */
const desktopDecodeProjection = S.decodeUnknownSync(SafeTurnProjection);
const webDecodeProjection = S.decodeUnknownSync(SafeTurnProjection);
const mobileDecodeProjection = S.decodeUnknownSync(SafeTurnProjection);
const decodeDecision = S.decodeUnknownSync(RouteDecision);

describe("AFS-00 baseline: cross-surface decode equivalence", () => {
  test("Desktop, web, and mobile decode every safe projection to equivalent facts", () => {
    for (const fixture of afsBaselineSafeProjectionFixtures) {
      const desktop = desktopDecodeProjection(fixture);
      const web = webDecodeProjection(fixture);
      const mobile = mobileDecodeProjection(fixture);
      expect(desktop).toEqual(web);
      expect(web).toEqual(mobile);
      expect(desktop).toEqual(fixture);
    }
  });
});

describe("AFS-00 baseline: local chat does not dispatch a provider", () => {
  test("the local answer route stays on the local lane with an on-device destination", () => {
    const decision = decodeDecision(localAnswerRouteDecisionFixture);
    expect(decision.outcome).toBe("admitted");
    if (decision.outcome !== "admitted") throw new Error("expected an admitted decision");
    expect(decision.selected).toBe(decision.effective);
    expect(decision.disclosure.dataDestination).toBe("on_device_local");
    expect(decision.disclosure.localOnly).toBe(true);
    // The local answer projection carries no dispatched provider turn.
    const projection = desktopDecodeProjection(localAnswerProjectionFixture);
    expect(projection.providerTurnRef).toBeUndefined();
    expect(projection.candidate).toBe("apple_fm");
    expect(projection.localOnly).toBe(true);
  });
});

describe("AFS-00 baseline: the explicit provider path dispatches", () => {
  test("an explicit provider route moves off-device and carries a dispatched provider turn", () => {
    const decision = decodeDecision(explicitProviderRouteDecisionFixture);
    if (decision.outcome !== "admitted") throw new Error("expected an admitted decision");
    expect(decision.disclosure.dataDestination).toBe("remote_provider");
    expect(decision.disclosure.localOnly).toBe(false);
    const projection = webDecodeProjection(explicitProviderProjectionFixture);
    expect(projection.providerTurnRef).toBeDefined();
    expect(projection.candidate).toBe("codex");
    expect(projection.usageTruth).toBe("exact");
  });
});

describe("AFS-00 baseline: an unavailable provider fails closed", () => {
  test("the route closes without a selected lane and discloses the refused lane", () => {
    const decision = decodeDecision(unavailableProviderRouteDecisionFixture);
    expect(decision.outcome).toBe("closed");
    if (decision.outcome !== "closed") throw new Error("expected a closed decision");
    expect(decision.decisionReason).toBe("no_candidate_fail_closed");
    expect(decision.dispositions[0]?.disposition).toBe("refused");
  });
});
