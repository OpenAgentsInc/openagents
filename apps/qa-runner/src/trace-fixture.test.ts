import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, test } from "bun:test";

import type { AtifTrajectory } from "./atif";
import {
  generateTraceFixture,
  loadTraceFixtureFromFile,
  TRACE_FIXTURE_SCHEMA_VERSION,
} from "./trace-fixture";

const trajectory = (overrides: Partial<AtifTrajectory> = {}): AtifTrajectory => ({
  schema_version: "ATIF-v1.7",
  trajectory_id: "trace-123",
  session_id: "session-123",
  agent: {
    name: "openagents-qa-runner",
    version: "0.1.0",
    model_name: "openagents/khala",
  },
  steps: [
    {
      step_id: 1,
      source: "user",
      message: "Reproduce public issue #6398",
    },
    {
      step_id: 2,
      source: "agent",
      model_name: "openagents/khala",
      message: "open the failing page",
      reasoning_content: "Chose action \"navigate\" against /login; outcome ok.",
      tool_calls: [
        {
          tool_call_id: "call_2",
          function_name: "navigate",
          arguments: { target: "/login", action: "navigate" },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: "call_2",
            content: "ok: open the failing page",
          },
        ],
      },
    },
  ],
  final_metrics: { total_steps: 2 },
  ...overrides,
});

describe("generateTraceFixture", () => {
  test("extracts step input/output payloads from ATIF evidence", () => {
    const fixture = generateTraceFixture({
      evidenceRef: "https://openagents.com/trace/trace-123",
      trajectory: trajectory(),
    });

    expect(fixture.schemaVersion).toBe(TRACE_FIXTURE_SCHEMA_VERSION);
    expect(fixture.evidenceRef).toBe("https://openagents.com/trace/trace-123");
    expect(fixture.trajectoryId).toBe("trace-123");
    expect(fixture.cases).toHaveLength(2);

    const agentCase = fixture.cases[1]!;
    expect(agentCase.id).toBe("trace-123:step-2");
    expect(agentCase.input.message).toBe("open the failing page");
    expect(agentCase.input.toolCalls?.[0]).toEqual({
      name: "navigate",
      arguments: { target: "/login", action: "navigate" },
    });
    expect(agentCase.output).toEqual({
      observations: ["ok: open the failing page"],
      status: "ok",
    });
  });

  test("marks failed observations as failed fixture cases", () => {
    const fixture = generateTraceFixture({
      evidenceRef: "trace://failed",
      trajectory: trajectory({
        steps: [
          {
            step_id: 1,
            source: "agent",
            message: "assert checkout works",
            tool_calls: [
              {
                tool_call_id: "call_1",
                function_name: "assert",
                arguments: { target: "checkout completed" },
              },
            ],
            observation: {
              results: [
                {
                  source_call_id: "call_1",
                  content: "FAILED: checkout button did not enable",
                },
              ],
            },
          },
        ],
      }),
    });

    expect(fixture.cases[0]!.output.status).toBe("failed");
  });

  test("redacts unsafe values before returning a fixture", () => {
    const fixture = generateTraceFixture({
      evidenceRef: "trace://unsafe",
      trajectory: trajectory({
        steps: [
          {
            step_id: 1,
            source: "user",
            message:
              "Failure happened at /Users/alice/work/app with token sk-test_secretvalue",
          },
        ],
      }),
    });

    const serialized = JSON.stringify(fixture);
    expect(serialized).not.toContain("/Users/alice");
    expect(serialized).not.toContain("sk-test_secretvalue");
    expect(serialized).toContain("[REDACTED:home_path]");
    expect(serialized).toContain("[REDACTED:provider_key]");
    expect(fixture.redaction.total).toBeGreaterThanOrEqual(2);
  });

  test("loads a fixture from an ATIF JSON file", () => {
    const dir = mkdtempSync(join(tmpdir(), "trace-fixture-"));
    try {
      const inputPath = join(dir, "trajectory.json");
      writeFileSync(inputPath, JSON.stringify(trajectory()));

      const fixture = loadTraceFixtureFromFile(inputPath, {
        evidenceRef: "local-test-ref",
      });

      expect(fixture.evidenceRef).toBe("local-test-ref");
      expect(fixture.cases[0]!.input.message).toBe("Reproduce public issue #6398");
      expect(readFileSync(inputPath, "utf8")).toContain("trace-123");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
