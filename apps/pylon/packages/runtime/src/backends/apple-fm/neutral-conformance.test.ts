import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";

import {
  appleFmActionClaimJsonFixture,
  appleFmAnswerCompletionFixture,
  appleFmHealthFixture,
  appleFmRecommendationJsonFixture,
  makeFakeAppleFmBridge,
} from "@openagentsinc/apple-fm-runtime/testing";

import { AppleFmNeutralRuntime } from "./neutral-runtime.js";

/**
 * AFS-02 (#9080): Pylon is a CONSUMER of the neutral
 * `@openagentsinc/apple-fm-runtime` package. It decodes the SAME shared wire,
 * client, and recommendation fixtures Desktop uses, through the compat surface,
 * so a shape drift between the two consumers is caught in one place.
 */
describe("Apple FM neutral runtime compat surface (Pylon consumer)", () => {
  test("Pylon can decode the shared wire fixtures", () => {
    const decodeHealth = S.decodeUnknownSync(AppleFmNeutralRuntime.AppleFmHealthResponse);
    const decodeCompletion = S.decodeUnknownSync(AppleFmNeutralRuntime.AppleFmChatCompletionResponse);
    expect(decodeHealth(appleFmHealthFixture).ready).toBe(true);
    expect(decodeCompletion(appleFmAnswerCompletionFixture).choices[0]?.message.content).toBe(
      "Hello there",
    );
  });

  test("Pylon drives the shared loopback client through the fake bridge", async () => {
    const fetchImpl = makeFakeAppleFmBridge();
    const probe = await AppleFmNeutralRuntime.appleFmProbe("http://127.0.0.1:11435", fetchImpl);
    expect(probe).toMatchObject({ status: "ready", ready: true });
    const turn = await AppleFmNeutralRuntime.appleFmComplete("http://127.0.0.1:11435", "hi", fetchImpl);
    expect(turn).toMatchObject({ outcome: "completed", text: "Hello there" });
  });

  test("Pylon shares the fail-closed recommendation decoder", () => {
    const admitted = ["apple_fm", "codex"] as const;
    const ok = AppleFmNeutralRuntime.decodeAppleFmRouteOutput({
      raw: appleFmRecommendationJsonFixture,
      admittedCandidates: admitted,
    });
    expect(ok._tag).toBe("Recommendation");
    const claim = AppleFmNeutralRuntime.decodeAppleFmRouteOutput({
      raw: appleFmActionClaimJsonFixture,
      admittedCandidates: admitted,
    });
    expect(claim).toEqual({ _tag: "Reject", reason: "action_claim_rejected" });
  });

  test("the single wire-version source is 0.1.3", () => {
    expect(AppleFmNeutralRuntime.APPLE_FM_CANONICAL_HELPER_VERSION).toBe("0.1.3");
  });
});
