// Unit tests for the ATIF emitter (mapper) + validator (epic #6174).
//
// Uses an inline FIXTURE run (a realistic public-safe QaRunResult + a
// deterministic KhalaSessionTrace for a /login verify) so the tests need no
// network and no live model. The fixture mirrors the shape the runner emits.

import { describe, expect, test } from "bun:test";
import {
  ATIF_SCHEMA_VERSION,
  assertAtifPublicSafe,
  atifVerdict,
  mapKhalaRunToAtif,
  serializeTrajectory,
  type AtifStep,
} from "./atif";
import { PublicSafetyViolation } from "./result";
import { assertValidAtif, validateAtif } from "./atif-validate";
import { computeDigest, type KhalaSessionTrace, type SessionBeat } from "./session-trace";
import type { QaRunResult } from "./result";

// --- fixture: a real-shaped /login verify run (PASS) ------------------------

function fixtureResult(overrides: Partial<QaRunResult> = {}): QaRunResult {
  return {
    schemaVersion: "openagents.qa_runner.result.v1",
    status: "pass",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    brain: "khala",
    backend: "local",
    startedAt: "2026-06-24T10:00:00.000Z",
    endedAt: "2026-06-24T10:00:42.500Z",
    durationMs: 42500,
    steps: [
      { index: 0, kind: "navigate", label: "Open the login page to verify it renders", status: "ok" },
      { index: 1, kind: "assert", label: "The page shows 'Log in to OpenAgents'", status: "ok" },
      { index: 2, kind: "assert", label: "The URL still includes /login (no redirect)", status: "ok" },
      { index: 3, kind: "screenshot", label: "login-page", status: "ok" },
    ],
    artifacts: {
      video: "session.mp4",
      videoFormat: "mp4",
      screenshots: ["login-page.png"],
    },
    ...overrides,
  };
}

function fixtureTrace(overrides: Partial<KhalaSessionTrace> = {}): KhalaSessionTrace {
  const beats: SessionBeat[] = [
    { kind: "chat_turn", role: "assistant", contentRef: "sha256:aaaa000011112222" },
    { kind: "tool_call", tool: "navigate", argsHash: "sha256:bbbb", effect: "read" },
    { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
    { kind: "chat_turn", role: "assistant", contentRef: "sha256:cccc" },
    { kind: "tool_call", tool: "assert", argsHash: "sha256:dddd", effect: "read" },
    { kind: "browser", action: "assert", targetHint: "The page shows 'Log in to OpenAgents'", status: "ok" },
    { kind: "chat_turn", role: "assistant", contentRef: "sha256:eeee" },
    { kind: "tool_call", tool: "assert", argsHash: "sha256:ffff", effect: "read" },
    { kind: "browser", action: "assert", targetHint: "url-includes:/login", status: "ok" },
    { kind: "chat_turn", role: "assistant", contentRef: "sha256:1111" },
    { kind: "tool_call", tool: "screenshot", argsHash: "sha256:2222", effect: "read" },
    { kind: "browser", action: "screenshot", targetHint: "login-page", status: "ok" },
    { kind: "verdict", verificationClass: "test_passed" },
  ];
  return {
    schemaVersion: "openagents.khala.session_trace.v1",
    goal: "Verify the login page works: open /login, confirm the sign-in form renders, and confirm it does not redirect to the homepage.",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats,
    inputs: [{ name: "target", type: "Target" }],
    outputs: [{ name: "verified", type: "boolean" }],
    receipts: ["result:result.json"],
    digest: computeDigest(beats),
    ...overrides,
  };
}

describe("mapKhalaRunToAtif", () => {
  test("emits a valid ATIF-v1.7 trajectory from a PASS /login run", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace(), sessionId: "login-trace" });
    assertValidAtif(traj); // throws if invalid
    expect(traj.schema_version).toBe(ATIF_SCHEMA_VERSION);
    expect(traj.agent.model_name).toBe("openagents/khala");
    expect(traj.session_id).toBe("login-trace");
    expect(traj.trajectory_id).toBe("login-trace-trajectory");
  });

  test("step 1 is the user goal", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    expect(traj.steps[0]!.source).toBe("user");
    expect(traj.steps[0]!.step_id).toBe(1);
    expect(traj.steps[0]!.message).toContain("Verify the login page");
    expect(traj.steps[0]!.tool_calls).toBeUndefined();
  });

  test("one agent step per action + a final verdict step", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    // 1 user + 4 action steps + 1 verdict step = 6
    expect(traj.steps.length).toBe(6);
    const agentSteps = traj.steps.filter((s) => s.source === "agent");
    expect(agentSteps.length).toBe(5);
  });

  test("step_id is sequential from 1", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    traj.steps.forEach((s, i) => expect(s.step_id).toBe(i + 1));
  });

  test("each agent step carries tool_calls, observation, reasoning, metrics", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    const nav = traj.steps[1]!;
    expect(nav.source).toBe("agent");
    expect(nav.tool_calls?.[0]!.function_name).toBe("navigate");
    expect(nav.tool_calls?.[0]!.arguments.target).toBe("/login");
    expect(nav.reasoning_content).toContain("navigate");
    expect(nav.metrics?.cost_usd).toBe(0);
  });

  test("observation source_call_id references the step's tool_call_id", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    for (const step of traj.steps) {
      if (!step.observation) continue;
      const ids = new Set((step.tool_calls ?? []).map((c) => c.tool_call_id));
      for (const r of step.observation.results) {
        if (r.source_call_id !== undefined) expect(ids.has(r.source_call_id)).toBe(true);
      }
    }
  });

  test("final step is a done tool_call with the verdict", () => {
    const traj = mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });
    const last = traj.steps[traj.steps.length - 1]!;
    expect(last.tool_calls?.[0]!.function_name).toBe("done");
    expect(last.tool_calls?.[0]!.arguments.verdict).toBe("PASS");
  });

  test("REFUTED run maps a failed assert + REFUTED verdict", () => {
    const result = fixtureResult({
      status: "fail",
      failure: "assertion FAILED: The page shows 'Log in to OpenAgents' (text lacks ...)",
      steps: [
        { index: 0, kind: "navigate", label: "Open the login page", status: "ok" },
        {
          index: 1,
          kind: "assert",
          label: "The page shows 'Log in to OpenAgents'",
          status: "failed",
          detail: { reason: "assertion FAILED" },
        },
      ],
    });
    const trace = fixtureTrace({
      beats: [
        { kind: "chat_turn", role: "assistant", contentRef: "sha256:a" },
        { kind: "tool_call", tool: "navigate", argsHash: "sha256:b", effect: "read" },
        { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
        { kind: "chat_turn", role: "assistant", contentRef: "sha256:c" },
        { kind: "tool_call", tool: "assert", argsHash: "sha256:d", effect: "read" },
        { kind: "browser", action: "assert", targetHint: "text", status: "failed" },
        { kind: "verdict", verificationClass: "failed" },
      ],
    });
    const traj = mapKhalaRunToAtif({ result, trace });
    assertValidAtif(traj);
    expect(atifVerdict(result)).toBe("REFUTED");
    const failedStep = traj.steps[2]!;
    expect(failedStep.observation?.results[0]!.content).toContain("FAILED");
    const last = traj.steps[traj.steps.length - 1]!;
    expect(last.tool_calls?.[0]!.arguments.verdict).toBe("REFUTED");
  });

  test("INCONCLUSIVE when the run did not reach a verdict", () => {
    const result = fixtureResult({
      status: "fail",
      failure: "khala did not reach a verdict within the step cap",
      steps: [{ index: 0, kind: "navigate", label: "Open the login page", status: "ok" }],
    });
    expect(atifVerdict(result)).toBe("INCONCLUSIVE");
  });

  test("never embeds forbidden fields (public-safe tripwire passes)", () => {
    // mapKhalaRunToAtif calls assertPublicSafeResult internally; a forbidden
    // KEY anywhere would throw. The PASS fixture must map cleanly.
    expect(() => mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() })).not.toThrow();
  });

  test("ATIF tripwire allows spec token-count keys but still catches secrets", () => {
    // The spec-mandated names must pass.
    expect(() =>
      assertAtifPublicSafe({ final_metrics: { total_prompt_tokens: 0, total_completion_tokens: 0 } }),
    ).not.toThrow();
    // A real secret-bearing key must still throw.
    expect(() => assertAtifPublicSafe({ extra: { api_key: "x" } })).toThrow(PublicSafetyViolation);
    expect(() => assertAtifPublicSafe({ steps: [{ cookie: "x" }] })).toThrow(PublicSafetyViolation);
    expect(() => assertAtifPublicSafe({ authorization: "Bearer x" })).toThrow(PublicSafetyViolation);
  });

  test("deterministic: same inputs yield identical serialization", () => {
    const a = serializeTrajectory(mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace(), sessionId: "x" }));
    const b = serializeTrajectory(mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace(), sessionId: "x" }));
    expect(a).toBe(b);
  });
});

describe("validateAtif", () => {
  const good = () => mapKhalaRunToAtif({ result: fixtureResult(), trace: fixtureTrace() });

  test("accepts a well-formed trajectory", () => {
    expect(validateAtif(good())).toEqual({ valid: true, errors: [] });
  });

  test("rejects a wrong schema_version", () => {
    const bad = { ...good(), schema_version: "ATIF-v1.6" };
    const r = validateAtif(bad);
    expect(r.valid).toBe(false);
  });

  test("rejects a non-sequential step_id", () => {
    const traj = good();
    const steps = traj.steps.map((s, i) => (i === 2 ? { ...s, step_id: 99 } : s));
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("sequential"))).toBe(true);
  });

  test("rejects an invalid source enum", () => {
    const traj = good();
    const steps = [...traj.steps];
    steps[1] = { ...steps[1]!, source: "robot" } as unknown as AtifStep;
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
  });

  test("rejects an observation source_call_id with no matching tool_call", () => {
    const traj = good();
    const steps = [...traj.steps];
    steps[1] = {
      ...steps[1]!,
      observation: { results: [{ source_call_id: "call_does_not_exist", content: "x" }] },
    };
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("does not reference"))).toBe(true);
  });

  test("rejects a non-ISO timestamp", () => {
    const traj = good();
    const steps = [...traj.steps];
    steps[0] = { ...steps[0]!, timestamp: "yesterday" };
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("ISO 8601"))).toBe(true);
  });

  test("rejects tool_calls on a user step", () => {
    const traj = good();
    const steps = [...traj.steps];
    steps[0] = {
      ...steps[0]!,
      tool_calls: [{ tool_call_id: "x", function_name: "navigate", arguments: {} }],
    };
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("must not carry tool_calls"))).toBe(true);
  });

  test("rejects a missing agent.name", () => {
    const traj = good();
    const r = validateAtif({ ...traj, agent: { ...traj.agent, name: "" } });
    expect(r.valid).toBe(false);
  });

  test("rejects an empty steps array", () => {
    const traj = good();
    const r = validateAtif({ ...traj, steps: [] });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("non-empty"))).toBe(true);
  });

  test("rejects a duplicate tool_call_id within a step", () => {
    const traj = good();
    const steps = [...traj.steps];
    steps[1] = {
      ...steps[1]!,
      tool_calls: [
        { tool_call_id: "dup", function_name: "navigate", arguments: {} },
        { tool_call_id: "dup", function_name: "assert", arguments: {} },
      ],
      observation: { results: [{ source_call_id: "dup", content: "ok" }] },
    };
    const r = validateAtif({ ...traj, steps });
    expect(r.valid).toBe(false);
    expect(r.errors.some((e) => e.includes("duplicated"))).toBe(true);
  });
});
