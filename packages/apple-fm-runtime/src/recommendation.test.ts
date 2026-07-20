import { describe, expect, test } from "vite-plus/test";

import { decodeAppleFmRouteOutput } from "./recommendation.js";
import {
  appleFmActionClaimJsonFixture,
  appleFmMalformedRecommendationJsonFixture,
  appleFmPlainAnswerFixture,
  appleFmRecommendationJsonFixture,
  appleFmUnavailableAgentJsonFixture,
} from "./testing.js";

const admitted = ["apple_fm", "codex", "claude"] as const;

describe("Apple FM Phase-1 JSON recommendation decoder (fail-closed)", () => {
  test("a valid structured recommendation decodes and may dispatch", () => {
    const result = decodeAppleFmRouteOutput({ raw: appleFmRecommendationJsonFixture, admittedCandidates: admitted });
    expect(result._tag).toBe("Recommendation");
    if (result._tag === "Recommendation") {
      expect(result.recommendation.candidate).toBe("codex");
      expect(result.recommendation.confidence).toBeCloseTo(0.82);
    }
  });

  test("a valid recommendation wrapped in prose + json fences still decodes", () => {
    const raw = `Here is my route:\n\`\`\`json\n${appleFmRecommendationJsonFixture}\n\`\`\`\nThanks!`;
    const result = decodeAppleFmRouteOutput({ raw, admittedCandidates: admitted });
    expect(result._tag).toBe("Recommendation");
  });

  test("plain advisory prose falls back to a safe Answer (never a dispatch)", () => {
    const result = decodeAppleFmRouteOutput({ raw: appleFmPlainAnswerFixture, admittedCandidates: admitted });
    expect(result._tag).toBe("Answer");
    if (result._tag === "Answer") expect(result.text).toContain("README");
  });

  test("empty output refuses and does NOT dispatch", () => {
    const result = decodeAppleFmRouteOutput({ raw: "   ", admittedCandidates: admitted });
    expect(result).toEqual({ _tag: "Reject", reason: "empty_output" });
  });

  test("oversized output refuses and does NOT dispatch", () => {
    const result = decodeAppleFmRouteOutput({ raw: "a".repeat(20), admittedCandidates: admitted, maxOutputChars: 10 });
    expect(result).toEqual({ _tag: "Reject", reason: "oversized_output" });
  });

  test("a malformed structured route refuses (malformed_output), never an answer", () => {
    const result = decodeAppleFmRouteOutput({ raw: appleFmMalformedRecommendationJsonFixture, admittedCandidates: admitted });
    expect(result).toEqual({ _tag: "Reject", reason: "malformed_output" });
  });

  test("an unavailable-agent recommendation refuses (provider_unadmitted)", () => {
    const result = decodeAppleFmRouteOutput({ raw: appleFmUnavailableAgentJsonFixture, admittedCandidates: admitted });
    expect(result).toEqual({ _tag: "Reject", reason: "provider_unadmitted" });
  });

  test("an action-claim refuses (action_claim_rejected) and does NOT dispatch", () => {
    const result = decodeAppleFmRouteOutput({ raw: appleFmActionClaimJsonFixture, admittedCandidates: admitted });
    expect(result).toEqual({ _tag: "Reject", reason: "action_claim_rejected" });
  });

  test("a `route` alias is accepted for `candidate`", () => {
    const raw = JSON.stringify({ route: "claude", taskClass: "delegate", reasonCode: "needs_delegation", confidence: 0.5 });
    const result = decodeAppleFmRouteOutput({ raw, admittedCandidates: admitted });
    expect(result._tag).toBe("Recommendation");
    if (result._tag === "Recommendation") expect(result.recommendation.candidate).toBe("claude");
  });
});
