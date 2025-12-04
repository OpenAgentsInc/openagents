/**
 * ATIF Validation Tests
 */
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import type { Trajectory } from "../schema.js";
import { ATIF_SCHEMA_VERSION } from "../schema.js";
import {
  collectValidationErrors,
  isValidTrajectory,
  validateTrajectory,
  validateTrajectorySync,
} from "../validation.js";

// Mutable version for testing (Effect/Schema types are readonly)
interface MutableTrajectory {
  schema_version: string;
  session_id: string;
  agent: { name: string; version: string; model_name: string };
  steps: MutableStep[];
  final_metrics?: { total_prompt_tokens: number; total_completion_tokens: number; total_steps: number };
}

interface MutableStep {
  step_id: number;
  timestamp: string;
  source: "user" | "agent" | "system";
  message: unknown;
  model_name?: string;
  reasoning_content?: string;
  tool_calls?: Array<{ tool_call_id: string; function_name: string; arguments: unknown }>;
  observation?: { results: Array<{ source_call_id?: string; content?: unknown }> };
  metrics?: { prompt_tokens?: number; completion_tokens?: number; cached_tokens?: number };
}

describe("ATIF Validation", () => {
  const createValidTrajectory = (): MutableTrajectory => ({
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: "session-2024-01-15T10-30-00-abc123",
    agent: {
      name: "test-agent",
      version: "1.0.0",
      model_name: "gpt-4",
    },
    steps: [
      {
        step_id: 1,
        timestamp: "2024-01-15T10:30:00.000Z",
        source: "user",
        message: "Hello",
      },
      {
        step_id: 2,
        timestamp: "2024-01-15T10:30:01.000Z",
        source: "agent",
        message: "Hi there!",
        model_name: "gpt-4",
        tool_calls: [
          {
            tool_call_id: "tc-1",
            function_name: "read_file",
            arguments: { path: "/test.txt" },
          },
        ],
      },
      {
        step_id: 3,
        timestamp: "2024-01-15T10:30:02.000Z",
        source: "system",
        message: "Tool results",
        observation: {
          results: [
            {
              source_call_id: "tc-1",
              content: "File contents",
            },
          ],
        },
      },
    ],
    final_metrics: {
      total_prompt_tokens: 100,
      total_completion_tokens: 50,
      total_steps: 3,
    },
  });

  describe("validateTrajectory", () => {
    test("accepts valid trajectory", async () => {
      const trajectory = createValidTrajectory();
      const result = await Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory));
      expect(result.session_id).toBe(trajectory.session_id);
    });

    test("rejects invalid schema version", async () => {
      const trajectory = createValidTrajectory();
      trajectory.schema_version = "ATIF-v1.0";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("Unsupported schema version");
    });
  });

  describe("Rule 1: Sequential step_id", () => {
    test("rejects non-sequential step IDs", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[1].step_id = 5; // Should be 2
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("Expected step_id");
    });

    test("rejects step_id starting from 0", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].step_id = 0;
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("Expected step_id");
    });
  });

  describe("Rule 2: ISO 8601 timestamp", () => {
    test("rejects invalid timestamp format", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].timestamp = "not-a-timestamp";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("Invalid ISO 8601 timestamp");
    });

    test("rejects date without time (missing T)", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].timestamp = "2024-01-15";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("Invalid ISO 8601 timestamp");
    });

    test("accepts various ISO 8601 formats", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].timestamp = "2024-01-15T10:30:00Z";
      const result = await Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory));
      expect(result).toBeDefined();
    });
  });

  describe("Rule 4: Tool call references", () => {
    test("rejects orphan tool reference in observation", async () => {
      const trajectory = createValidTrajectory();
      // Reference a tool_call_id that doesn't exist
      trajectory.steps[2].observation = {
        results: [
          {
            source_call_id: "tc-nonexistent",
            content: "Some result",
          },
        ],
      };
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("unknown tool_call_id");
    });

    test("accepts valid tool call references", async () => {
      const trajectory = createValidTrajectory();
      const result = await Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory));
      expect(result).toBeDefined();
    });
  });

  describe("Rule 5: Agent-only fields", () => {
    test("rejects model_name on user step", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].model_name = "gpt-4";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("only allowed on agent steps");
    });

    test("rejects reasoning_content on user step", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].reasoning_content = "Thinking...";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("only allowed on agent steps");
    });

    test("rejects model_name on system step", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[2].model_name = "gpt-4";
      await expect(
        Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory)),
      ).rejects.toThrow("only allowed on agent steps");
    });

    test("accepts model_name on agent step", async () => {
      const trajectory = createValidTrajectory();
      const result = await Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory));
      expect(result.steps[1].model_name).toBe("gpt-4");
    });

    test("accepts reasoning_content on agent step", async () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[1].reasoning_content = "Let me think about this...";
      const result = await Effect.runPromise(validateTrajectory(trajectory as unknown as Trajectory));
      expect(result.steps[1].reasoning_content).toBeDefined();
    });
  });

  describe("validateTrajectorySync", () => {
    test("returns trajectory on valid input", () => {
      const trajectory = createValidTrajectory();
      const result = validateTrajectorySync(trajectory as unknown as Trajectory);
      expect(result.session_id).toBe(trajectory.session_id);
    });

    test("throws on invalid input", () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].step_id = 5;
      expect(() => validateTrajectorySync(trajectory as unknown as Trajectory)).toThrow();
    });
  });

  describe("isValidTrajectory", () => {
    test("returns true for valid trajectory", () => {
      const trajectory = createValidTrajectory();
      expect(isValidTrajectory(trajectory as unknown as Trajectory)).toBe(true);
    });

    test("returns false for invalid trajectory", () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].step_id = 5;
      expect(isValidTrajectory(trajectory as unknown as Trajectory)).toBe(false);
    });
  });

  describe("collectValidationErrors", () => {
    test("returns empty array for valid trajectory", () => {
      const trajectory = createValidTrajectory();
      const errors = collectValidationErrors(trajectory as unknown as Trajectory);
      expect(errors).toHaveLength(0);
    });

    test("collects multiple errors", () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[0].step_id = 5; // Error 1: wrong sequence
      trajectory.steps[0].model_name = "gpt-4"; // Error 2: agent-only field
      trajectory.steps[1].timestamp = "invalid"; // Error 3: invalid timestamp

      const errors = collectValidationErrors(trajectory as unknown as Trajectory);
      expect(errors.length).toBeGreaterThanOrEqual(2);
    });

    test("includes step_id in error details", () => {
      const trajectory = createValidTrajectory();
      trajectory.steps[1].step_id = 10;
      const errors = collectValidationErrors(trajectory as unknown as Trajectory);
      const seqError = errors.find((e) => e.reason === "invalid_step_sequence");
      expect(seqError).toBeDefined();
      expect(seqError?.stepId).toBe(10);
    });
  });
});
