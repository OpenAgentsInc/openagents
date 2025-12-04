/**
 * TrajectoryCollector Tests
 */
import { describe, expect, test } from "bun:test";
import {
  ATIF_SCHEMA_VERSION,
  type Agent,
} from "../schema.js";
import {
  StandaloneTrajectoryCollector,
  TrajectoryCollectorError,
} from "../collector.js";
import { isValidTrajectory } from "../validation.js";

describe("StandaloneTrajectoryCollector", () => {
  const testAgent: Agent = {
    name: "test-agent",
    version: "1.0.0",
    model_name: "gpt-4",
  };

  describe("Lifecycle", () => {
    test("starts and finishes trajectory", () => {
      const collector = new StandaloneTrajectoryCollector();

      const sessionId = collector.startTrajectory({ agent: testAgent });
      expect(sessionId).toMatch(/^session-/);
      expect(collector.isActive()).toBe(true);

      const trajectory = collector.finishTrajectory();
      expect(trajectory.session_id).toBe(sessionId);
      expect(trajectory.schema_version).toBe(ATIF_SCHEMA_VERSION);
      expect(collector.isActive()).toBe(false);
    });

    test("uses provided session ID", () => {
      const collector = new StandaloneTrajectoryCollector();
      const sessionId = collector.startTrajectory({
        agent: testAgent,
        sessionId: "custom-session-id",
      });
      expect(sessionId).toBe("custom-session-id");
    });

    test("throws when starting twice", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      expect(() => collector.startTrajectory({ agent: testAgent })).toThrow(
        TrajectoryCollectorError,
      );
    });

    test("throws when recording without starting", () => {
      const collector = new StandaloneTrajectoryCollector();

      expect(() => collector.recordUserStep("test")).toThrow(
        TrajectoryCollectorError,
      );
    });

    test("includes parent session ID in extra", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({
        agent: testAgent,
        parentSessionId: "parent-session",
      });

      const trajectory = collector.finishTrajectory();
      expect(trajectory.extra?.parent_session_id).toBe("parent-session");
    });
  });

  describe("Recording Steps", () => {
    test("records user step", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      const step = collector.recordUserStep("Hello, agent!");

      expect(step.step_id).toBe(1);
      expect(step.source).toBe("user");
      expect(step.message).toBe("Hello, agent!");
    });

    test("records agent step with tool calls", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });
      collector.recordUserStep("Hello");

      const step = collector.recordAgentStep({
        message: "Let me check that file",
        modelName: "gpt-4",
        toolCalls: [
          {
            functionName: "read_file",
            arguments: { path: "/test.txt" },
          },
        ],
        metrics: {
          promptTokens: 100,
          completionTokens: 50,
          costUsd: 0.01,
        },
      });

      expect(step.step_id).toBe(2);
      expect(step.source).toBe("agent");
      expect(step.model_name).toBe("gpt-4");
      expect(step.tool_calls).toHaveLength(1);
      expect(step.tool_calls![0].function_name).toBe("read_file");
      expect(step.metrics?.prompt_tokens).toBe(100);
    });

    test("records system step", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      const step = collector.recordSystemStep("System message");

      expect(step.step_id).toBe(1);
      expect(step.source).toBe("system");
    });

    test("records observation with tool results", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      // Record a tool call first
      collector.recordAgentStep({
        message: "Calling tool",
        toolCalls: [
          {
            toolCallId: "tc-1",
            functionName: "read_file",
            arguments: {},
          },
        ],
      });

      const step = collector.recordObservation([
        {
          sourceCallId: "tc-1",
          content: "File contents here",
        },
      ]);

      expect(step.source).toBe("system");
      expect(step.observation?.results).toHaveLength(1);
      expect(step.observation?.results[0].source_call_id).toBe("tc-1");
    });

    test("records observation with subagent refs", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      const step = collector.recordObservation([
        {
          sourceCallId: "tc-1",
          content: "Subagent completed",
          subagentRefs: [
            {
              sessionId: "child-session-1",
              trajectoryPath: "/path/to/trajectory.json",
            },
          ],
        },
      ]);

      expect(step.observation?.results[0].subagent_trajectory_ref).toHaveLength(1);
      expect(step.observation?.results[0].subagent_trajectory_ref![0].session_id).toBe(
        "child-session-1",
      );
    });
  });

  describe("Metrics Accumulation", () => {
    test("accumulates metrics in final_metrics", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      collector.recordAgentStep({
        message: "Step 1",
        metrics: {
          promptTokens: 100,
          completionTokens: 50,
          cachedTokens: 20,
          costUsd: 0.01,
        },
      });

      collector.recordAgentStep({
        message: "Step 2",
        metrics: {
          promptTokens: 80,
          completionTokens: 40,
          costUsd: 0.008,
        },
      });

      const trajectory = collector.finishTrajectory();

      expect(trajectory.final_metrics?.total_prompt_tokens).toBe(180);
      expect(trajectory.final_metrics?.total_completion_tokens).toBe(90);
      expect(trajectory.final_metrics?.total_cached_tokens).toBe(20);
      expect(trajectory.final_metrics?.total_cost_usd).toBeCloseTo(0.018);
      expect(trajectory.final_metrics?.total_steps).toBe(2);
    });

    test("excludes zero totals from final_metrics", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });
      collector.recordUserStep("Hello");

      const trajectory = collector.finishTrajectory();

      expect(trajectory.final_metrics?.total_cached_tokens).toBeUndefined();
      expect(trajectory.final_metrics?.total_cost_usd).toBeUndefined();
    });
  });

  describe("Step Sequencing", () => {
    test("auto-increments step IDs", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      const step1 = collector.recordUserStep("Message 1");
      const step2 = collector.recordAgentStep({ message: "Response 1" });
      const step3 = collector.recordSystemStep("System note");
      const step4 = collector.recordUserStep("Message 2");

      expect(step1.step_id).toBe(1);
      expect(step2.step_id).toBe(2);
      expect(step3.step_id).toBe(3);
      expect(step4.step_id).toBe(4);
    });
  });

  describe("Subagent Registration", () => {
    test("registers subagent", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      collector.registerSubagent("child-session-1", "/path/to/trajectory.json");

      const state = collector.getCurrentState();
      expect(state?.subagentRefs.has("child-session-1")).toBe(true);
    });
  });

  describe("Complete Trajectory", () => {
    test("produces valid ATIF trajectory", () => {
      const collector = new StandaloneTrajectoryCollector();
      collector.startTrajectory({ agent: testAgent });

      collector.recordUserStep("Write a hello world function");
      collector.recordAgentStep({
        message: "I'll write that for you",
        modelName: "gpt-4",
        toolCalls: [
          {
            functionName: "write_file",
            arguments: { path: "hello.ts", content: 'console.log("Hello")' },
          },
        ],
        metrics: {
          promptTokens: 50,
          completionTokens: 25,
        },
      });
      collector.recordObservation([
        {
          content: "File written successfully",
        },
      ]);

      const trajectory = collector.finishTrajectory("Test session notes");

      expect(trajectory.notes).toBe("Test session notes");
      expect(trajectory.steps).toHaveLength(3);
      expect(isValidTrajectory(trajectory)).toBe(true);
    });
  });
});
