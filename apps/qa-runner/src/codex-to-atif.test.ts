// Unit tests for the Codex rollout -> ATIF converter (issue #6220).
//
// Uses a committed Codex-rollout fixture (src/fixtures/codex-rollout.sample.jsonl)
// so the tests never depend on a live ~/.codex. The fixture mirrors a real
// codex 0.142.0 rollout (session_meta + turn_context model + a reasoning ->
// assistant message -> two exec tool calls + outputs bundled into one API call,
// then a second assistant message; each API call closed by a token_count event).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { ATIF_SCHEMA_VERSION } from "./atif";
import { assertValidAtif, validateAtif } from "./atif-validate";
import {
  type CodexStepMetrics,
  convertCodexRolloutTextToAtif,
  convertCodexRolloutToAtif,
  parseCodexRollout,
} from "./codex-to-atif";

const FIXTURE = readFileSync(join(import.meta.dir, "fixtures", "codex-rollout.sample.jsonl"), "utf8");

describe("convertCodexRolloutToAtif", () => {
  test("converts the committed fixture into a VALID ATIF-v1.7 trajectory", () => {
    const trajectory = convertCodexRolloutTextToAtif(FIXTURE);
    expect(trajectory.schema_version).toBe(ATIF_SCHEMA_VERSION);

    const { valid, errors } = validateAtif(trajectory);
    expect(errors).toEqual([]);
    expect(valid).toBe(true);
    // assertValidAtif must not throw (the converter's own CLI tripwire).
    expect(() => assertValidAtif(trajectory)).not.toThrow();
  });

  test("header: session id, codex agent, cli version, model, extra", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    expect(t.session_id).toBe("019efa28-63ad-76a2-8e9b-8a653170604e");
    expect(t.trajectory_id).toBe("019efa28-63ad-76a2-8e9b-8a653170604e-trajectory");
    expect(t.agent.name).toBe("codex");
    expect(t.agent.version).toBe("0.142.0");
    expect(t.agent.model_name).toBe("gpt-5.5");
    expect(t.agent.extra?.cwd).toBe("/Users/example/work");
    expect(t.agent.extra?.originator).toBe("Codex CLI");
  });

  test("step count + sources: developer->system, user, two agent API calls", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    // 1 system (developer) + 1 user + 2 agent (two API calls) = 4 steps.
    expect(t.steps.length).toBe(4);
    expect(t.steps.map((s) => s.source)).toEqual(["system", "user", "agent", "agent"]);

    // Sequential step ids from 1 (also enforced by the validator).
    expect(t.steps.map((s) => s.step_id)).toEqual([1, 2, 3, 4]);

    const userStep = t.steps[1]!;
    expect(userStep.source).toBe("user");
    expect(userStep.message).toContain("List the files in the repo root");
    // Non-agent steps must not carry agent-only fields.
    expect(userStep.tool_calls).toBeUndefined();
    expect(userStep.reasoning_content).toBeUndefined();
    expect(userStep.metrics).toBeUndefined();
  });

  test("user prompts map to source:user; system/developer to source:system", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    const systemStep = t.steps[0]!;
    expect(systemStep.source).toBe("system");
    expect(systemStep.message).toContain("Filesystem sandboxing");
  });

  test("agent turn: message + reasoning_content + bundled tool_calls", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    const agentStep = t.steps[2]!;
    expect(agentStep.source).toBe("agent");
    expect(agentStep.message).toBe("Inspecting the directory.");
    expect(agentStep.model_name).toBe("gpt-5.5");
    // Reasoning summary surfaced; encrypted_content dropped.
    expect(agentStep.reasoning_content).toContain("inspect the working directory");
    expect(agentStep.reasoning_content).not.toContain("gAAAA");

    // Two exec tool calls bundled into the single API-call step (harbor shape).
    expect(agentStep.tool_calls).toBeDefined();
    expect(agentStep.tool_calls!.length).toBe(2);
    expect(agentStep.tool_calls!.map((c) => c.function_name)).toEqual(["exec_command", "exec_command"]);
    expect(agentStep.tool_calls!.map((c) => c.tool_call_id)).toEqual([
      "call_aAEAfn1zt6Z8LnW8B89hDtUF",
      "call_bBFBgo2yu7A9MoX9C90iEuVG",
    ]);
    // Tool-call arguments parsed from the JSON-string `arguments` field.
    expect(agentStep.tool_calls![0]!.arguments.cmd).toBe("pwd && ls");
  });

  test("tool_call <-> observation correlation matches the harbor golden shape", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    const agentStep = t.steps[2]!;

    // Each observation result's source_call_id references a tool_call_id in the
    // SAME step, correlated by id and in order (the harbor golden invariant).
    const callIds = agentStep.tool_calls!.map((c) => c.tool_call_id);
    const obs = agentStep.observation!;
    expect(obs.results.length).toBe(2);
    expect(obs.results.map((r) => r.source_call_id)).toEqual(callIds);

    // First call's output is the raw exec text; second call's `{output,metadata}`
    // blob is unwrapped to its textual `output` (harbor _parse_output_blob).
    expect(obs.results[0]!.content).toContain("README.md");
    expect(obs.results[0]!.content).toContain("package.json");
    expect(obs.results[1]!.content).toBe("# Example Project\nA small demo repo.");

    // Every source_call_id is a tool_call_id present in the step.
    for (const r of obs.results) {
      expect(r.source_call_id).toBeDefined();
      expect(callIds).toContain(r.source_call_id!);
    }
  });

  test("per-API-call + final token metrics are surfaced where present", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    const firstAgent = t.steps[2]!;
    // The first API call's last_token_usage attaches to its step.
    const metrics = firstAgent.metrics as CodexStepMetrics | undefined;
    expect(metrics?.prompt_tokens).toBe(40856);
    expect(metrics?.completion_tokens).toBe(292);
    expect(metrics?.cached_tokens).toBe(4992);

    // final_metrics from the LAST token_count total_token_usage.
    expect(t.final_metrics?.total_prompt_tokens).toBe(82056);
    expect(t.final_metrics?.total_completion_tokens).toBe(340);
    expect(t.final_metrics?.total_cached_tokens).toBe(44992);
    expect(t.final_metrics?.total_steps).toBe(4);
  });

  test("second agent step is the final standalone assistant message", () => {
    const t = convertCodexRolloutTextToAtif(FIXTURE);
    const second = t.steps[3]!;
    expect(second.source).toBe("agent");
    expect(second.message).toContain("small demo repo");
    expect(second.tool_calls).toBeUndefined();
  });
});

describe("parseCodexRollout", () => {
  test("skips blank and malformed JSONL lines without throwing", () => {
    const events = parseCodexRollout(
      [
        '{"type":"session_meta","payload":{"id":"s1"}}',
        "",
        "   ",
        "{ this is not json",
        '{"type":"event_msg","payload":{"type":"token_count"}}',
      ].join("\n"),
    );
    expect(events.length).toBe(2);
    expect(events[0]!.type).toBe("session_meta");
  });

  test("convert applies sessionId/model overrides", () => {
    const events = parseCodexRollout(FIXTURE);
    const t = convertCodexRolloutToAtif(events, { sessionId: "custom", modelName: "fallback" });
    expect(t.session_id).toBe("custom");
    expect(t.trajectory_id).toBe("custom-trajectory");
    // turn_context model still wins over the fallback when present.
    expect(t.agent.model_name).toBe("gpt-5.5");
  });
});

describe("edge cases", () => {
  test("output with no matching call still produces a valid trajectory", () => {
    const jsonl = [
      '{"type":"session_meta","payload":{"id":"s2"}}',
      '{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}',
      '{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"working"}]}}',
      '{"type":"response_item","payload":{"type":"function_call_output","call_id":"orphan","output":"done"}}',
      '{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12}}}}',
    ].join("\n");
    const t = convertCodexRolloutTextToAtif(jsonl);
    const { valid, errors } = validateAtif(t);
    expect(errors).toEqual([]);
    expect(valid).toBe(true);
  });

  test("empty rollout yields a header-only, structurally valid-but-empty result", () => {
    const t = convertCodexRolloutTextToAtif("");
    expect(t.steps.length).toBe(0);
    expect(t.agent.name).toBe("codex");
    expect(t.session_id).toBe("codex-session");
    // No steps -> the validator (correctly) reports steps must be non-empty.
    expect(validateAtif(t).valid).toBe(false);
  });
});
