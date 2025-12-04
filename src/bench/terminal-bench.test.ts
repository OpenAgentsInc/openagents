import { describe, test, expect } from "bun:test";
import {
  terminalBenchToTask,
  toBenchmarkResults,
  type TerminalBenchTask,
  type TerminalBenchSuite,
} from "./terminal-bench.js";

const createMockTBTask = (overrides: Partial<TerminalBenchTask> = {}): TerminalBenchTask => ({
  id: "tb-task-1",
  name: "Fix Authentication Bug",
  description: "The login function fails when password contains special characters.",
  difficulty: "medium",
  category: "debugging",
  verification: {
    type: "test",
    command: "bun test auth.test.ts",
  },
  ...overrides,
});

describe("terminal-bench", () => {
  describe("terminalBenchToTask", () => {
    test("converts basic task correctly", () => {
      const tbTask = createMockTBTask();
      const task = terminalBenchToTask(tbTask);

      expect(task.title).toBe("Fix Authentication Bug");
      expect(task.description).toContain("The login function fails");
      expect(task.type).toBe("task");
      expect(task.priority).toBe(2); // medium = priority 2
      expect(task.labels).toContain("terminal-bench");
      expect(task.labels).toContain("debugging");
      expect(task.labels).toContain("medium");
    });

    test("maps difficulty to priority correctly", () => {
      const easy = terminalBenchToTask(createMockTBTask({ difficulty: "easy" }));
      const medium = terminalBenchToTask(createMockTBTask({ difficulty: "medium" }));
      const hard = terminalBenchToTask(createMockTBTask({ difficulty: "hard" }));
      const expert = terminalBenchToTask(createMockTBTask({ difficulty: "expert" }));

      expect(easy.priority).toBe(3);
      expect(medium.priority).toBe(2);
      expect(hard.priority).toBe(1);
      expect(expert.priority).toBe(0);
    });

    test("includes files_to_modify in description", () => {
      const tbTask = createMockTBTask({
        files_to_modify: ["src/auth.ts", "src/utils.ts"],
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.description).toContain("Files to modify: src/auth.ts, src/utils.ts");
    });

    test("includes hints in description", () => {
      const tbTask = createMockTBTask({
        hints: ["Check URL encoding", "Look at the sanitize function"],
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.description).toContain("Hints:");
      expect(task.description).toContain("- Check URL encoding");
      expect(task.description).toContain("- Look at the sanitize function");
    });

    test("includes tags as labels", () => {
      const tbTask = createMockTBTask({
        tags: ["security", "critical"],
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.labels).toContain("security");
      expect(task.labels).toContain("critical");
    });

    test("sets acceptance criteria for test verification", () => {
      const tbTask = createMockTBTask({
        verification: {
          type: "test",
          command: "npm test",
        },
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.acceptanceCriteria).toBe("Run tests: npm test");
    });

    test("sets acceptance criteria for output verification", () => {
      const tbTask = createMockTBTask({
        verification: {
          type: "output",
          command: "echo hello",
          expected: "hello",
        },
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.acceptanceCriteria).toBe("Expected output: hello");
    });

    test("sets acceptance criteria for diff verification", () => {
      const tbTask = createMockTBTask({
        verification: {
          type: "diff",
        },
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.acceptanceCriteria).toBe("Changes must match expected diff");
    });

    test("sets acceptance criteria for custom verification", () => {
      const tbTask = createMockTBTask({
        verification: {
          type: "custom",
          script: "./verify.sh",
        },
      });
      const task = terminalBenchToTask(tbTask);

      expect(task.acceptanceCriteria).toBe("Custom verification: ./verify.sh");
    });
  });

  describe("toBenchmarkResults", () => {
    test("converts results correctly", () => {
      const suite: TerminalBenchSuite = {
        name: "Test Suite",
        version: "1.0.0",
        tasks: [
          createMockTBTask({ id: "t1" }),
          createMockTBTask({ id: "t2" }),
        ],
      };

      const taskResults = [
        {
          taskId: "t1",
          outcome: "success" as const,
          durationMs: 1000,
          turns: 2,
          tokens: 100,
          verificationOutput: undefined,
          errorMessage: undefined,
        },
        {
          taskId: "t2",
          outcome: "failure" as const,
          durationMs: 2000,
          turns: 5,
          tokens: 250,
          verificationOutput: undefined,
          errorMessage: "Tests failed",
        },
      ];

      const results = toBenchmarkResults(suite, "grok-4.1", taskResults);

      expect(results.suite_name).toBe("Test Suite");
      expect(results.suite_version).toBe("1.0.0");
      expect(results.model).toBe("grok-4.1");
      expect(results.results.length).toBe(2);
      expect(results.results[0].status).toBe("pass");
      expect(results.results[1].status).toBe("fail");
      expect(results.results[1].error_message).toBe("Tests failed");

      expect(results.summary.total).toBe(2);
      expect(results.summary.passed).toBe(1);
      expect(results.summary.failed).toBe(1);
      expect(results.summary.pass_rate).toBe(0.5);
      expect(results.summary.avg_duration_ms).toBe(1500);
      expect(results.summary.avg_turns).toBe(3.5);
      expect(results.summary.total_tokens).toBe(350);
    });

    test("maps all outcome types correctly", () => {
      const suite: TerminalBenchSuite = {
        name: "Test Suite",
        version: "1.0.0",
        tasks: [
          createMockTBTask({ id: "t1" }),
          createMockTBTask({ id: "t2" }),
          createMockTBTask({ id: "t3" }),
          createMockTBTask({ id: "t4" }),
        ],
      };

      const taskResults = [
        { taskId: "t1", outcome: "success" as const, durationMs: 100, turns: 1, tokens: 10, verificationOutput: undefined, errorMessage: undefined },
        { taskId: "t2", outcome: "failure" as const, durationMs: 100, turns: 1, tokens: 10, verificationOutput: undefined, errorMessage: undefined },
        { taskId: "t3", outcome: "timeout" as const, durationMs: 100, turns: 1, tokens: 10, verificationOutput: undefined, errorMessage: undefined },
        { taskId: "t4", outcome: "error" as const, durationMs: 100, turns: 1, tokens: 10, verificationOutput: undefined, errorMessage: undefined },
      ];

      const results = toBenchmarkResults(suite, "test-model", taskResults);

      expect(results.results[0].status).toBe("pass");
      expect(results.results[1].status).toBe("fail");
      expect(results.results[2].status).toBe("timeout");
      expect(results.results[3].status).toBe("error");

      expect(results.summary.passed).toBe(1);
      expect(results.summary.failed).toBe(1);
      expect(results.summary.timeout).toBe(1);
      expect(results.summary.error).toBe(1);
    });

    test("includes skipped entries for tasks without results", () => {
      const suite: TerminalBenchSuite = {
        name: "Test Suite",
        version: "1.0.0",
        tasks: [
          createMockTBTask({ id: "t1" }),
          createMockTBTask({ id: "t2" }),
        ],
      };

      const taskResults = [
        {
          taskId: "t1",
          outcome: "success" as const,
          durationMs: 500,
          turns: 1,
          tokens: 50,
          verificationOutput: undefined,
          errorMessage: undefined,
        },
      ];

      const results = toBenchmarkResults(suite, "test-model", taskResults);

      expect(results.results.find((r) => r.task_id === "t2")?.status).toBe("skip");
      expect(results.summary.total).toBe(2);
      expect(results.summary.skipped).toBe(1);
      expect(results.summary.passed).toBe(1);
      expect(results.summary.total_tokens).toBe(50);
    });

    test("handles empty results", () => {
      const suite: TerminalBenchSuite = {
        name: "Empty Suite",
        version: "1.0.0",
        tasks: [],
      };

      const results = toBenchmarkResults(suite, "test-model", []);

      expect(results.summary.total).toBe(0);
      expect(results.summary.pass_rate).toBe(0);
      expect(results.summary.avg_duration_ms).toBe(0);
    });
  });
});
