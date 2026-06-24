// Skill-candidate emitter tests (spec §E.1 / §D.3). A trace lowers into a GOVERNED
// Blueprint optimizer candidate: typed, honest ladder tier, evidence-only,
// Release-Gate-gated, never self-promoted, fail-closed on a non-public-safe trace.

import { describe, expect, test } from "bun:test";
import { makeSessionTrace, type SessionBeat } from "./session-trace";
import {
  assertSkillCandidateGoverned,
  DISTILLER_SKILL_RELEASE_GATE_REF,
  emitSkillCandidate,
  evaluateReleaseGateWithoutApproval,
  SkillCandidateGovernanceError,
} from "./skill-candidate";
import type { BlueprintProgramSignature, VerificationClass } from "./distiller";

const signature: BlueprintProgramSignature = {
  name: "verify_login",
  description: "verify the login page works",
  inputs: [{ name: "target", type: "Target" }],
  outputs: [{ name: "verified", type: "boolean" }],
};

const trace = (overrides: { goal?: string } = {}) =>
  makeSessionTrace({
    goal: overrides.goal ?? "verify the login page works",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats: [{ kind: "verdict", verificationClass: "test_passed" }] as SessionBeat[],
    inputs: signature.inputs,
    outputs: signature.outputs,
    receipts: ["result:result.json"],
  });

const emit = (verificationClass: VerificationClass) =>
  emitSkillCandidate({ trace: trace(), signature, verificationClass, slug: "verify-login" });

describe("emitSkillCandidate", () => {
  test("emits a governed optimizer candidate that is not live and is gated", () => {
    const candidate = emit("exact_trace_replay");
    expect(candidate.kind).toBe("blueprint_optimizer_skill_candidate");
    expect(candidate.moduleKind).toBe("optimizer_candidate");
    expect(candidate.governance.authorityBoundary).toBe("evidence_only");
    expect(candidate.governance.live).toBe(false);
    expect(candidate.governance.requiresReleaseGate).toBe(true);
    expect(candidate.governance.releaseGateRef).toBe(DISTILLER_SKILL_RELEASE_GATE_REF);
    expect(candidate.governance.selfPromotionAllowed).toBe(false);
    expect(candidate.receiptRefs).toEqual(["result:result.json"]);
  });

  test("maps verification classes to honest NIP-SKL ladder tiers (no inflation)", () => {
    expect(emit("exact_trace_replay").ladderTier).toBe("E");
    expect(emit("test_passed").ladderTier).toBe("S");
    expect(emit("seeded").ladderTier).toBe("D");
    expect(emit("none").ladderTier).toBe("N");
  });

  test("fails closed on a non-public-safe trace", () => {
    const bad = makeSessionTrace({
      goal: "leak bearer aaaaaaaaaaaaaaaaaaaa here",
      target: { name: "t", baseUrl: "https://x" },
      model: "openagents/khala",
      beats: [{ kind: "verdict", verificationClass: "none" }],
      inputs: signature.inputs,
      outputs: signature.outputs,
    });
    expect(() =>
      emitSkillCandidate({ trace: bad, signature, verificationClass: "none", slug: "leak" }),
    ).toThrow();
  });
});

describe("governance guard (no self-promotion, ever)", () => {
  test("the Release Gate rejects an unapproved candidate", () => {
    const decision = evaluateReleaseGateWithoutApproval(emit("exact_trace_replay"));
    expect(decision.promoted).toBe(false);
    expect(decision.reason).toContain("operator approval");
  });

  test("assertSkillCandidateGoverned rejects a self-promotable candidate", () => {
    const tampered = {
      ...emit("test_passed"),
      governance: { ...emit("test_passed").governance, selfPromotionAllowed: true as unknown as false },
    };
    expect(() => assertSkillCandidateGoverned(tampered)).toThrow(SkillCandidateGovernanceError);
  });

  test("the emitter refuses an `any`-typed signature (fail closed)", () => {
    expect(() =>
      emitSkillCandidate({
        trace: trace(),
        signature: { ...signature, inputs: [{ name: "x", type: "any" }] },
        verificationClass: "test_passed",
        slug: "x",
      }),
    ).toThrow(SkillCandidateGovernanceError);
  });

  test("assertSkillCandidateGoverned rejects a tampered `any`-typed candidate", () => {
    const tampered = {
      ...emit("test_passed"),
      signature: { ...signature, inputs: [{ name: "x", type: "any" }] },
    };
    expect(() => assertSkillCandidateGoverned(tampered)).toThrow(SkillCandidateGovernanceError);
  });
});
