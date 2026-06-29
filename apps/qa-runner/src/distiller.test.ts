// Distiller tests (fakes, no network): a hand-checked fixture trace lowers into
// the expected executor-style e2e scenario shape, the candidate acceptance bar
// (spec §D.2) is enforced, dishonest grades are caught, and a non-public-safe
// trace fails closed.

import { describe, expect, test } from "bun:test";
import { assessCandidate, distill, DistillError } from "./distiller";
import { makeSessionTrace, type SessionBeat } from "./session-trace";
import {
  assertSkillCandidateGoverned,
  evaluateReleaseGateWithoutApproval,
  SkillCandidateGovernanceError,
} from "./skill-candidate";

/** A fixture login-verification session, as a Khala session would record it. */
const loginBeats: SessionBeat[] = [
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:t1" },
  { kind: "tool_call", tool: "navigate", argsHash: "sha256:a1", effect: "read" },
  { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:t2" },
  { kind: "tool_call", tool: "waitFor", argsHash: "sha256:a2", effect: "read" },
  { kind: "browser", action: "wait", targetHint: "text-visible:Log in to OpenAgents", status: "ok" },
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:t3" },
  { kind: "tool_call", tool: "screenshot", argsHash: "sha256:a3", effect: "read" },
  { kind: "browser", action: "screenshot", targetHint: "login-page", status: "ok" },
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:t4" },
  { kind: "tool_call", tool: "assert", argsHash: "sha256:a4", effect: "read" },
  { kind: "browser", action: "assert", targetHint: 'stays at /login (no redirect to home)', status: "ok" },
  { kind: "chat_turn", role: "assistant", contentRef: "sha256:t5" },
  { kind: "tool_call", tool: "assert", argsHash: "sha256:a5", effect: "read" },
  { kind: "browser", action: "assert", targetHint: 'body contains "Log in to OpenAgents"', status: "ok" },
  { kind: "verdict", verificationClass: "test_passed" },
];

const loginTrace = () =>
  makeSessionTrace({
    goal: "verify the login page works",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats: loginBeats,
    inputs: [
      { name: "target", type: "Target", description: "the deployment under test" },
      { name: "path", type: "string", description: "the route to verify" },
    ],
    outputs: [{ name: "verified", type: "boolean" }],
    receipts: ["result:result.json"],
  });

describe("distill (deterministic v1 reducer)", () => {
  test("infers a typed signature (no `any`)", () => {
    const result = distill(loginTrace());
    expect(result.signatureCandidate.name).toBe("verify_the_login_page_works");
    expect(result.signatureCandidate.inputs.every((f) => f.type.toLowerCase() !== "any")).toBe(true);
    expect(result.moduleCandidate.moduleKind).toBe("deterministic_reducer");
  });

  test("carries an honest verification class (exact_trace_replay for a deterministic pass)", () => {
    const result = distill(loginTrace());
    expect(result.verificationClass).toBe("exact_trace_replay");
  });

  test("emits an executor-style e2e scenario with the expected shape", () => {
    const { emitters } = distill(loginTrace());
    const src = emitters.e2e.source;
    expect(emitters.e2e.slug).toBe("verify-the-login-page-works");
    // a navigate step, a deterministic wait (NO sleep), a screenshot, assertions
    expect(src).toContain('{ kind: "navigate", url: "/login"');
    expect(src).toContain('{ kind: "wait-for", condition: { kind: "text-visible", value: "Log in to OpenAgents" }');
    expect(src).toContain('{ kind: "screenshot", label: "login-page" }');
    // outcome assertions (url stays at /login; body contains the text)
    expect(src).toContain('{ kind: "url-includes", value: "/login" }');
    expect(src).toContain('{ kind: "text-contains", value: "Log in to OpenAgents" }');
    // it is a real, runnable bun test against the runner
    expect(src).toContain('import { describe, expect, test } from "bun:test";');
    expect(src).toContain("runQaSession");
    expect(src).toContain("TARGET_URL");
    // NO sleeps: no timer/sleep CALLS in the generated code (the comment may
    // use the word "sleeps" to explain the discipline; assert on call syntax).
    expect(src).not.toMatch(/setTimeout\s*\(|waitForTimeout\s*\(|sleep\s*\(/);
    // at least two outcome assertions
    expect(emitters.e2e.assertionCount).toBeGreaterThanOrEqual(2);
  });

  test("the e2e emitter source pins the source digest for traceability", () => {
    const trace = loginTrace();
    const { emitters } = distill(trace);
    expect(emitters.e2e.source).toContain(trace.digest);
  });

  test("the skill emitter (E.1) emits a GOVERNED optimizer candidate (not live, gated, no self-promotion)", () => {
    const result = distill(loginTrace());
    const skill = result.emitters.skill;
    expect(skill.kind).toBe("blueprint_optimizer_skill_candidate");
    expect(skill.moduleKind).toBe("optimizer_candidate");
    // honest ladder tier: a deterministic exact replay is tier E
    expect(skill.verificationClass).toBe("exact_trace_replay");
    expect(skill.ladderTier).toBe("E");
    // typed signature, no `any`
    expect(skill.signature.inputs.every((f) => f.type.toLowerCase() !== "any")).toBe(true);
    // GOVERNANCE: evidence-only, Release-Gate-gated, never self-promoted, not live
    expect(skill.governance.authorityBoundary).toBe("evidence_only");
    expect(skill.governance.requiresReleaseGate).toBe(true);
    expect(skill.governance.selfPromotionAllowed).toBe(false);
    expect(skill.governance.live).toBe(false);
    expect(skill.governance.releaseGateRef.length).toBeGreaterThan(0);
    // traceability: the candidate pins the source trace digest
    expect(skill.sourceDigest).toBe(result.emitters.e2e.source.match(/source digest:\s+(\w+)/)?.[1] ?? "");
  });

  test("ONE capture distills into BOTH a committed e2e test AND a governed skill candidate (spec §E)", () => {
    const result = distill(loginTrace());
    // (a) a real, runnable committed e2e test
    expect(result.emitters.e2e.source).toContain("runQaSession");
    expect(result.emitters.e2e.assertionCount).toBeGreaterThanOrEqual(1);
    // (b) a governed Blueprint optimizer skill candidate from the same trace
    expect(result.emitters.skill.moduleKind).toBe("optimizer_candidate");
    expect(result.emitters.skill.slug).toBe(result.emitters.e2e.slug);
  });
});

describe("skill candidate governance (spec §D.3 / §D.4 — Release Gate, no self-promotion)", () => {
  test("the Release Gate REJECTS an unapproved candidate (no self-promotion, ever)", () => {
    const result = distill(loginTrace());
    const decision = evaluateReleaseGateWithoutApproval(result.emitters.skill);
    expect(decision.promoted).toBe(false);
    expect(decision.reason).toContain("release_gate_rejected");
  });

  test("assertSkillCandidateGoverned throws when a candidate is tampered self-promotable", () => {
    const result = distill(loginTrace());
    const tampered = {
      ...result.emitters.skill,
      governance: { ...result.emitters.skill.governance, selfPromotionAllowed: true as unknown as false },
    };
    expect(() => assertSkillCandidateGoverned(tampered)).toThrow(SkillCandidateGovernanceError);
  });
});

describe("assessCandidate (acceptance bar, spec §D.2)", () => {
  test("a clean distilled login candidate is admissible", () => {
    const trace = loginTrace();
    const result = distill(trace);
    const assessment = assessCandidate(result, trace);
    expect(assessment.admissible).toBe(true);
    expect(assessment.reasons).toEqual([]);
  });

  test("a typed-`any` signature is rejected as not typed", () => {
    const trace = loginTrace();
    const result = distill(trace);
    const dirty = { ...result, signatureCandidate: { ...result.signatureCandidate, inputs: [{ name: "x", type: "any" }] } };
    const assessment = assessCandidate(dirty, trace);
    expect(assessment.admissible).toBe(false);
    expect(assessment.reasons.some((r) => r.includes("not typed"))).toBe(true);
  });

  test("a candidate with zero assertions is rejected (asserts outcomes)", () => {
    // a trace with no assert beats -> no outcome assertions
    const noAssert = makeSessionTrace({
      goal: "open the homepage",
      target: { name: "t", baseUrl: "https://openagents.com" },
      model: "openagents/khala",
      beats: [
        { kind: "browser", action: "navigate", targetHint: "/", status: "ok" },
        { kind: "verdict", verificationClass: "test_passed" },
      ],
      inputs: [{ name: "target", type: "Target" }],
      outputs: [{ name: "verified", type: "boolean" }],
    });
    const result = distill(noAssert);
    const assessment = assessCandidate(result, noAssert);
    expect(assessment.admissible).toBe(false);
    expect(assessment.reasons.some((r) => r.includes("no outcome assertions"))).toBe(true);
  });
});

describe("distill fails closed on a non-public-safe trace", () => {
  test("throws DistillError when the trace carries a secret value", () => {
    const bad = makeSessionTrace({
      goal: "leak bearer aaaaaaaaaaaaaaaaaaaa here",
      target: { name: "t", baseUrl: "https://x" },
      model: "openagents/khala",
      beats: [{ kind: "verdict", verificationClass: "none" }],
      inputs: [],
      outputs: [{ name: "v", type: "boolean" }],
    });
    expect(() => distill(bad)).toThrow(DistillError);
  });
});

describe("honest grading", () => {
  test("a failed verdict distills to verification class 'none' (no exactness inflation)", () => {
    const failed = makeSessionTrace({
      goal: "verify something that failed",
      target: { name: "t", baseUrl: "https://openagents.com" },
      model: "openagents/khala",
      beats: [
        { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
        { kind: "browser", action: "assert", targetHint: "stays at /login", status: "failed" },
        { kind: "verdict", verificationClass: "failed" },
      ],
      inputs: [{ name: "target", type: "Target" }],
      outputs: [{ name: "verified", type: "boolean" }],
    });
    const result = distill(failed);
    expect(result.verificationClass).toBe("none");
  });
});
