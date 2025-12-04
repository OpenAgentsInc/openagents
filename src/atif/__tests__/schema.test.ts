/**
 * ATIF Schema Tests
 */
import { describe, expect, test } from "bun:test";
import {
  ATIF_SCHEMA_VERSION,
  type Step,
  type Trajectory,
  decodeAgent,
  decodeStep,
  decodeTrajectory,
  extractSubagentSessionIds,
  extractToolCallIds,
  generateSessionId,
  generateToolCallId,
  getTotalTokens,
  hasObservation,
  hasSubagentRefs,
  hasToolCalls,
  isAgentStep,
  isSystemStep,
  isUserStep,
  timestamp,
} from "../schema.js";

describe("ATIF Schema", () => {
  describe("Agent", () => {
    test("decodes valid agent", () => {
      const agent = decodeAgent({
        name: "test-agent",
        version: "1.0.0",
        model_name: "gpt-4",
      });
      expect(agent.name).toBe("test-agent");
      expect(agent.version).toBe("1.0.0");
      expect(agent.model_name).toBe("gpt-4");
    });

    test("decodes agent with extra fields", () => {
      const agent = decodeAgent({
        name: "test-agent",
        version: "1.0.0",
        model_name: "gpt-4",
        extra: { custom: "value" },
      });
      expect(agent.extra).toEqual({ custom: "value" });
    });

    test("throws on missing required fields", () => {
      expect(() => decodeAgent({ name: "test" })).toThrow();
    });
  });

  describe("Step", () => {
    test("decodes valid user step", () => {
      const step = decodeStep({
        step_id: 1,
        timestamp: "2024-01-15T10:30:00.000Z",
        source: "user",
        message: "Hello, world!",
      });
      expect(step.step_id).toBe(1);
      expect(step.source).toBe("user");
      expect(step.message).toBe("Hello, world!");
    });

    test("decodes valid agent step with tool calls", () => {
      const step = decodeStep({
        step_id: 2,
        timestamp: "2024-01-15T10:30:01.000Z",
        source: "agent",
        message: "Let me check that file",
        model_name: "gpt-4",
        tool_calls: [
          {
            tool_call_id: "tc-123",
            function_name: "read_file",
            arguments: { path: "/test.txt" },
          },
        ],
      });
      expect(step.source).toBe("agent");
      expect(step.model_name).toBe("gpt-4");
      expect(step.tool_calls).toHaveLength(1);
      expect(step.tool_calls![0].function_name).toBe("read_file");
    });

    test("decodes system step with observation", () => {
      const step = decodeStep({
        step_id: 3,
        timestamp: "2024-01-15T10:30:02.000Z",
        source: "system",
        message: "Tool results",
        observation: {
          results: [
            {
              source_call_id: "tc-123",
              content: "File contents here",
            },
          ],
        },
      });
      expect(step.source).toBe("system");
      expect(step.observation?.results).toHaveLength(1);
    });

    test("rejects invalid source", () => {
      expect(() =>
        decodeStep({
          step_id: 1,
          timestamp: "2024-01-15T10:30:00.000Z",
          source: "invalid",
          message: "test",
        }),
      ).toThrow();
    });

    test("rejects non-positive step_id", () => {
      expect(() =>
        decodeStep({
          step_id: 0,
          timestamp: "2024-01-15T10:30:00.000Z",
          source: "user",
          message: "test",
        }),
      ).toThrow();
    });
  });

  describe("Trajectory", () => {
    const validTrajectory: Trajectory = {
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
        },
      ],
      final_metrics: {
        total_prompt_tokens: 100,
        total_completion_tokens: 50,
        total_steps: 2,
      },
    };

    test("decodes valid trajectory", () => {
      const trajectory = decodeTrajectory(validTrajectory);
      expect(trajectory.schema_version).toBe(ATIF_SCHEMA_VERSION);
      expect(trajectory.steps).toHaveLength(2);
    });

    test("rejects invalid schema version", () => {
      expect(() =>
        decodeTrajectory({
          ...validTrajectory,
          schema_version: "ATIF-v1.0",
        }),
      ).toThrow();
    });
  });

  describe("Type Guards", () => {
    const userStep: Step = {
      step_id: 1,
      timestamp: "2024-01-15T10:30:00.000Z",
      source: "user",
      message: "test",
    };

    const agentStep: Step = {
      step_id: 2,
      timestamp: "2024-01-15T10:30:01.000Z",
      source: "agent",
      message: "test",
      tool_calls: [
        {
          tool_call_id: "tc-1",
          function_name: "test",
          arguments: {},
        },
      ],
    };

    const systemStep: Step = {
      step_id: 3,
      timestamp: "2024-01-15T10:30:02.000Z",
      source: "system",
      message: "test",
      observation: {
        results: [
          {
            source_call_id: "tc-1",
            content: "result",
            subagent_trajectory_ref: [{ session_id: "child-session" }],
          },
        ],
      },
    };

    test("isAgentStep", () => {
      expect(isAgentStep(agentStep)).toBe(true);
      expect(isAgentStep(userStep)).toBe(false);
      expect(isAgentStep(systemStep)).toBe(false);
    });

    test("isUserStep", () => {
      expect(isUserStep(userStep)).toBe(true);
      expect(isUserStep(agentStep)).toBe(false);
    });

    test("isSystemStep", () => {
      expect(isSystemStep(systemStep)).toBe(true);
      expect(isSystemStep(userStep)).toBe(false);
    });

    test("hasToolCalls", () => {
      expect(hasToolCalls(agentStep)).toBe(true);
      expect(hasToolCalls(userStep)).toBe(false);
    });

    test("hasObservation", () => {
      expect(hasObservation(systemStep)).toBe(true);
      expect(hasObservation(userStep)).toBe(false);
    });

    test("hasSubagentRefs", () => {
      expect(hasSubagentRefs(systemStep)).toBe(true);
      expect(hasSubagentRefs(userStep)).toBe(false);
    });
  });

  describe("Extraction Helpers", () => {
    const trajectory: Trajectory = {
      schema_version: ATIF_SCHEMA_VERSION,
      session_id: "parent-session",
      agent: { name: "test", version: "1.0", model_name: "gpt-4" },
      steps: [
        {
          step_id: 1,
          timestamp: "2024-01-15T10:30:00.000Z",
          source: "agent",
          message: "test",
          tool_calls: [
            { tool_call_id: "tc-1", function_name: "fn1", arguments: {} },
            { tool_call_id: "tc-2", function_name: "fn2", arguments: {} },
          ],
        },
        {
          step_id: 2,
          timestamp: "2024-01-15T10:30:01.000Z",
          source: "system",
          message: "result",
          observation: {
            results: [
              {
                source_call_id: "tc-1",
                subagent_trajectory_ref: [
                  { session_id: "child-1" },
                  { session_id: "child-2" },
                ],
              },
            ],
          },
        },
      ],
    };

    test("extractToolCallIds", () => {
      const ids = extractToolCallIds(trajectory);
      expect(ids.size).toBe(2);
      expect(ids.has("tc-1")).toBe(true);
      expect(ids.has("tc-2")).toBe(true);
    });

    test("extractSubagentSessionIds", () => {
      const ids = extractSubagentSessionIds(trajectory);
      expect(ids).toHaveLength(2);
      expect(ids).toContain("child-1");
      expect(ids).toContain("child-2");
    });

    test("getTotalTokens from final_metrics", () => {
      const trajWithMetrics: Trajectory = {
        ...trajectory,
        final_metrics: {
          total_prompt_tokens: 100,
          total_completion_tokens: 50,
          total_cached_tokens: 20,
          total_steps: 2,
        },
      };
      const tokens = getTotalTokens(trajWithMetrics);
      expect(tokens.prompt).toBe(100);
      expect(tokens.completion).toBe(50);
      expect(tokens.cached).toBe(20);
    });

    test("getTotalTokens calculated from steps", () => {
      const trajWithStepMetrics: Trajectory = {
        ...trajectory,
        steps: [
          {
            step_id: 1,
            timestamp: "2024-01-15T10:30:00.000Z",
            source: "agent",
            message: "test",
            metrics: {
              prompt_tokens: 50,
              completion_tokens: 25,
              cached_tokens: 10,
            },
          },
          {
            step_id: 2,
            timestamp: "2024-01-15T10:30:01.000Z",
            source: "agent",
            message: "test",
            metrics: {
              prompt_tokens: 30,
              completion_tokens: 15,
            },
          },
        ],
      };
      const tokens = getTotalTokens(trajWithStepMetrics);
      expect(tokens.prompt).toBe(80);
      expect(tokens.completion).toBe(40);
      expect(tokens.cached).toBe(10);
    });
  });

  describe("ID Generation", () => {
    test("generateSessionId produces valid format", () => {
      const id = generateSessionId();
      expect(id).toMatch(/^session-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}/);
    });

    test("generateToolCallId produces unique IDs", () => {
      const id1 = generateToolCallId();
      const id2 = generateToolCallId();
      expect(id1).not.toBe(id2);
      expect(id1).toMatch(/^tc-/);
    });

    test("timestamp produces valid ISO format", () => {
      const ts = timestamp();
      expect(new Date(ts).toISOString()).toBe(ts);
    });
  });
});
