import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  AFS_SCHEMA_COMPATIBILITY_RULES,
  AnswerCandidate,
  InferenceProviderDescriptor,
  MAX_OWNER_BOUND_CANDIDATES,
  MAX_TURN_BLOCKER_REFS,
  MAX_TURN_INPUT_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  OwnerBoundCandidateSet,
  ReleasedArtifact,
  ROUTE_DECISION_SCHEMA_LITERAL,
  RouteDecision,
  RouteRecommendation,
  SafeTurnProjection,
  TurnBlockerRefs,
  TurnIntent,
  TurnReceipt,
  turnIntentTaskClass,
  turnRefusalReasons,
  turnStageKinds,
  turnTerminalStates,
  WorkContextEnvelope,
} from "./index.js";
import {
  explicitProviderRouteDecisionFixture,
  localAnswerProjectionFixture,
  localAnswerRouteDecisionFixture,
  routeRecommendationFixture,
  unavailableProviderRouteDecisionFixture,
} from "./afs-baseline-fixtures.js";

const decodeIntent = S.decodeUnknownSync(TurnIntent);
const decodeRecommendation = S.decodeUnknownSync(RouteRecommendation);
const decodeDecision = S.decodeUnknownSync(RouteDecision);
const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor);
const decodeContext = S.decodeUnknownSync(WorkContextEnvelope);
const decodeReceipt = S.decodeUnknownSync(TurnReceipt);
const decodeProjection = S.decodeUnknownSync(SafeTurnProjection);
const decodeCandidateSet = S.decodeUnknownSync(OwnerBoundCandidateSet);
const decodeArtifact = S.decodeUnknownSync(ReleasedArtifact);
const decodeAnswer = S.decodeUnknownSync(AnswerCandidate);
const decodeBlockerRefs = S.decodeUnknownSync(TurnBlockerRefs);

const providerDescriptor = {
  schema: "openagents.agent_turn_provider.v1" as const,
  providerRef: "provider.apple_fm.1",
  candidate: "apple_fm" as const,
  model: "apple-foundation-model",
  placement: "owner_local" as const,
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "on_device_local" as const,
  usageTruth: "estimated" as const,
  costClass: "local_resource_only" as const,
  maxContextChars: 4000,
  maxOutputChars: 8192,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
};

describe("AFS-00 turn contract round-trip", () => {
  test("decodes a turn intent and derives its task class", () => {
    const intent = decodeIntent({ _tag: "Ask", text: "hi" });
    expect(intent._tag).toBe("Ask");
    expect(turnIntentTaskClass.Ask).toBe("local_answer");
    expect(turnIntentTaskClass.RecommendRoute).toBe("route_recommendation");
  });

  test("round-trips the recommendation, decision, descriptor, receipt, and projection fixtures", () => {
    expect(decodeRecommendation(routeRecommendationFixture)).toEqual(routeRecommendationFixture);
    expect(decodeDecision(localAnswerRouteDecisionFixture)).toEqual(localAnswerRouteDecisionFixture);
    expect(decodeDecision(explicitProviderRouteDecisionFixture)).toEqual(explicitProviderRouteDecisionFixture);
    expect(decodeDecision(unavailableProviderRouteDecisionFixture)).toEqual(unavailableProviderRouteDecisionFixture);
    expect(decodeDescriptor(providerDescriptor)).toEqual(providerDescriptor);
    expect(decodeProjection(localAnswerProjectionFixture)).toEqual(localAnswerProjectionFixture);
  });

  test("round-trips a context envelope, receipt, candidate set, artifact, and answer", () => {
    expect(
      decodeContext({
        schema: "openagents.agent_turn_context_envelope.v1",
        manifestRef: "context.1",
        threadRef: "thread.1",
        generation: { state: "known", value: 1 },
        createdAt: "2026-07-20T08:00:00Z",
        items: [{ kind: "active_file", itemRef: "item.1", derived: false, byteLength: 10, truncated: false, redacted: false }],
        totalByteLength: 10,
        byteLimit: 64000,
        truncated: false,
        redacted: false,
      }),
    ).toMatchObject({ manifestRef: "context.1" });
    expect(
      decodeReceipt({
        schema: "openagents.agent_turn_receipt.v1",
        requestRef: "request.1",
        routeDecisionRef: "route.1",
        decision: "accepted",
        usageTruth: "estimated",
        evidenceRefs: ["evidence.1"],
      }),
    ).toMatchObject({ decision: "accepted" });
    expect(
      decodeCandidateSet({
        schema: "openagents.agent_owner_bound_candidate_set.v1",
        ordered: ["provider.apple_fm.1", "provider.codex.1"],
        policyArtifactRef: "artifact.1",
      }),
    ).toMatchObject({ ordered: ["provider.apple_fm.1", "provider.codex.1"] });
    expect(
      decodeArtifact({
        schema: "openagents.agent_turn_artifact.v1",
        artifactRef: "artifact.1",
        digest: "a".repeat(64),
        kind: "policy_bundle",
        releasedAt: "2026-07-20T08:00:00Z",
      }),
    ).toMatchObject({ kind: "policy_bundle" });
    expect(
      decodeAnswer({
        schema: "openagents.agent_turn_candidate.v1",
        kind: "answer",
        candidateRef: "candidate.1",
        provenance: {
          providerRef: "provider.apple_fm.1",
          candidate: "apple_fm",
          model: "apple-foundation-model",
          taskClass: "local_answer",
          usageTruth: "estimated",
          dataDestination: "on_device_local",
          stale: false,
        },
        text: "local answer",
      }),
    ).toMatchObject({ kind: "answer" });
  });
});

describe("AFS-00 turn contract invalid input", () => {
  test("rejects an unknown provider candidate", () => {
    expect(() => decodeRecommendation({ ...routeRecommendationFixture, candidate: "gemini" })).toThrow();
  });

  test("rejects a confidence outside 0..1", () => {
    expect(() => decodeRecommendation({ ...routeRecommendationFixture, confidence: 1.5 })).toThrow();
  });

  test("rejects a route decision with an unknown decision reason", () => {
    expect(() =>
      decodeDecision({ ...localAnswerRouteDecisionFixture, decisionReason: "vibes" }),
    ).toThrow();
  });

  test("rejects a projection with an unknown card state", () => {
    expect(() => decodeProjection({ ...localAnswerProjectionFixture, cardState: "sparkling" })).toThrow();
  });

  test("rejects a descriptor that claims external action capability with a non-boolean", () => {
    expect(() => decodeDescriptor({ ...providerDescriptor, supportsExternalActions: "yes" })).toThrow();
  });
});

describe("AFS-00 turn contract size bounds", () => {
  test("rejects input over the frozen maximum", () => {
    expect(() => decodeIntent({ _tag: "Ask", text: "x".repeat(MAX_TURN_INPUT_CHARS + 1) })).toThrow();
    expect(decodeIntent({ _tag: "Ask", text: "x".repeat(MAX_TURN_INPUT_CHARS) })._tag).toBe("Ask");
  });

  test("rejects answer output over the frozen maximum", () => {
    const base = {
      schema: "openagents.agent_turn_candidate.v1" as const,
      kind: "answer" as const,
      candidateRef: "candidate.1",
      provenance: {
        providerRef: "provider.apple_fm.1",
        candidate: "apple_fm" as const,
        model: "apple-foundation-model",
        taskClass: "local_answer" as const,
        usageTruth: "estimated" as const,
        dataDestination: "on_device_local" as const,
        stale: false,
      },
    };
    expect(() => decodeAnswer({ ...base, text: "x".repeat(MAX_TURN_OUTPUT_CHARS + 1) })).toThrow();
  });

  test("rejects a blocker-ref list over the frozen maximum", () => {
    const refs = Array.from({ length: MAX_TURN_BLOCKER_REFS + 1 }, (_, index) => `blocker.apple_fm.r${index}`);
    expect(() => decodeBlockerRefs(refs)).toThrow();
    expect(decodeBlockerRefs(refs.slice(0, MAX_TURN_BLOCKER_REFS))).toHaveLength(MAX_TURN_BLOCKER_REFS);
  });

  test("rejects an owner-bound candidate set over the frozen maximum", () => {
    const ordered = Array.from({ length: MAX_OWNER_BOUND_CANDIDATES + 1 }, (_, index) => `provider.p${index}`);
    expect(() =>
      decodeCandidateSet({
        schema: "openagents.agent_owner_bound_candidate_set.v1",
        ordered,
        policyArtifactRef: "artifact.1",
      }),
    ).toThrow();
  });
});

describe("AFS-00 contract invariants", () => {
  test("records the compatibility rules and the decision schema literal", () => {
    expect(AFS_SCHEMA_COMPATIBILITY_RULES.length).toBeGreaterThanOrEqual(3);
    expect(ROUTE_DECISION_SCHEMA_LITERAL).toBe("openagents.agent_route_decision.v1");
  });

  test("freezes exactly eight distinct turn-stage concepts", () => {
    expect(turnStageKinds).toEqual([
      "recommendation",
      "decision",
      "action",
      "card",
      "evidence",
      "acceptance",
      "delivery",
      "release",
    ]);
    expect(new Set(turnStageKinds).size).toBe(8);
  });

  test("terminal states are a subset of the lifecycle and include every refusal", () => {
    expect(turnTerminalStates).toContain("refused");
    expect(turnTerminalStates).toContain("completed");
    expect(turnRefusalReasons).toContain("decode_failed");
    expect(turnRefusalReasons).toContain("action_claim_rejected");
    expect(turnRefusalReasons).toContain("route_closed_no_candidate");
  });

  test("every turn intent tag maps to a task class", () => {
    const tags = ["Ask", "Complete", "NextEdit", "ProposeEdit", "ExplainFailure", "ExplainDebug", "DraftCommitMessage", "RecommendRoute"] as const;
    for (const tag of tags) expect(turnIntentTaskClass[tag]).toBeDefined();
  });
});
