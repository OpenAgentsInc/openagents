/**
 * Archivist Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  generateTrajectoryId,
  generatePatternId,
  generateArchiveId,
  createTrajectory,
  buildPatternExtractionPrompt,
  parsePatternsFromResponse,
  calculateSuccessRate,
  groupSimilarTrajectories,
  type Trajectory,
  type TrajectoryAction,
} from "./schema.js";

describe("ID Generation", () => {
  test("generateTrajectoryId creates unique IDs", () => {
    const id1 = generateTrajectoryId();
    const id2 = generateTrajectoryId();

    expect(id1).toMatch(/^traj-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^traj-[a-z0-9]+-[a-z0-9]+$/);
    expect(id1).not.toBe(id2);
  });

  test("generatePatternId creates typed IDs", () => {
    const skillId = generatePatternId("skill");
    const antiId = generatePatternId("antipattern");

    expect(skillId).toMatch(/^pat-ski-[a-z0-9]+-[a-z0-9]+$/);
    expect(antiId).toMatch(/^pat-ant-[a-z0-9]+-[a-z0-9]+$/);
  });

  test("generateArchiveId creates unique IDs", () => {
    const id1 = generateArchiveId();
    const id2 = generateArchiveId();

    expect(id1).toMatch(/^arch-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^arch-[a-z0-9]+-[a-z0-9]+$/);
    expect(id1).not.toBe(id2);
  });
});

describe("Trajectory Creation", () => {
  test("createTrajectory creates with required fields", () => {
    const actions: TrajectoryAction[] = [
      {
        type: "tool_call",
        tool: "read",
        content: "read src/index.ts",
        success: true,
        timestamp: new Date().toISOString(),
      },
    ];

    const trajectory = createTrajectory("task-1", "Fix the import error", {
      actions,
      outcome: "success",
      totalDurationMs: 5000,
      model: "fm",
      tokens: { input: 100, output: 50, total: 150 },
    });

    expect(trajectory.id).toMatch(/^traj-/);
    expect(trajectory.taskId).toBe("task-1");
    expect(trajectory.taskDescription).toBe("Fix the import error");
    expect(trajectory.actions).toEqual(actions);
    expect(trajectory.outcome).toBe("success");
    expect(trajectory.totalDurationMs).toBe(5000);
    expect(trajectory.model).toBe("fm");
    expect(trajectory.tokens.total).toBe(150);
    expect(trajectory.archived).toBe(false);
    expect(trajectory.timestamp).toBeDefined();
  });

  test("createTrajectory includes optional fields", () => {
    const trajectory = createTrajectory("task-2", "Add feature", {
      actions: [],
      outcome: "failure",
      errorMessage: "Type error",
      skillsUsed: ["skill-1", "skill-2"],
      filesModified: ["src/foo.ts"],
      totalDurationMs: 10000,
      model: "fm",
      tokens: { input: 200, output: 100, total: 300 },
      projectId: "proj-123",
    });

    expect(trajectory.errorMessage).toBe("Type error");
    expect(trajectory.skillsUsed).toEqual(["skill-1", "skill-2"]);
    expect(trajectory.filesModified).toEqual(["src/foo.ts"]);
    expect(trajectory.projectId).toBe("proj-123");
  });
});

describe("Success Rate Calculation", () => {
  const makeTrajectory = (outcome: Trajectory["outcome"]): Trajectory => ({
    id: generateTrajectoryId(),
    taskId: "task-1",
    taskDescription: "Test task",
    actions: [],
    outcome,
    skillsUsed: [],
    filesModified: [],
    totalDurationMs: 1000,
    model: "fm",
    tokens: { input: 10, output: 10, total: 20 },
    timestamp: new Date().toISOString(),
    archived: false,
  });

  test("calculateSuccessRate returns 0 for empty array", () => {
    expect(calculateSuccessRate([])).toBe(0);
  });

  test("calculateSuccessRate returns 1 for all successful", () => {
    const trajectories = [
      makeTrajectory("success"),
      makeTrajectory("success"),
      makeTrajectory("success"),
    ];
    expect(calculateSuccessRate(trajectories)).toBe(1);
  });

  test("calculateSuccessRate returns 0 for all failures", () => {
    const trajectories = [
      makeTrajectory("failure"),
      makeTrajectory("failure"),
    ];
    expect(calculateSuccessRate(trajectories)).toBe(0);
  });

  test("calculateSuccessRate calculates correctly for mixed", () => {
    const trajectories = [
      makeTrajectory("success"),
      makeTrajectory("failure"),
      makeTrajectory("success"),
      makeTrajectory("partial"),
    ];
    expect(calculateSuccessRate(trajectories)).toBe(0.5);
  });
});

describe("Trajectory Grouping", () => {
  const makeTrajectoryWithAction = (
    tool: string,
    outcome: Trajectory["outcome"],
  ): Trajectory => ({
    id: generateTrajectoryId(),
    taskId: "task-1",
    taskDescription: "Test task",
    actions: [
      {
        type: "tool_call",
        tool,
        content: "action content",
        timestamp: new Date().toISOString(),
      },
    ],
    outcome,
    skillsUsed: [],
    filesModified: [],
    totalDurationMs: 1000,
    model: "fm",
    tokens: { input: 10, output: 10, total: 20 },
    timestamp: new Date().toISOString(),
    archived: false,
  });

  test("groupSimilarTrajectories groups by tool and outcome", () => {
    const trajectories = [
      makeTrajectoryWithAction("read", "success"),
      makeTrajectoryWithAction("read", "success"),
      makeTrajectoryWithAction("read", "failure"),
      makeTrajectoryWithAction("write", "success"),
    ];

    const groups = groupSimilarTrajectories(trajectories);

    expect(groups.size).toBe(3);
    expect(groups.get("read-success")?.length).toBe(2);
    expect(groups.get("read-failure")?.length).toBe(1);
    expect(groups.get("write-success")?.length).toBe(1);
  });

  test("groupSimilarTrajectories handles empty actions", () => {
    const trajectory: Trajectory = {
      id: generateTrajectoryId(),
      taskId: "task-1",
      taskDescription: "Test task",
      actions: [],
      outcome: "success",
      skillsUsed: [],
      filesModified: [],
      totalDurationMs: 1000,
      model: "fm",
      tokens: { input: 10, output: 10, total: 20 },
      timestamp: new Date().toISOString(),
      archived: false,
    };

    const groups = groupSimilarTrajectories([trajectory]);

    expect(groups.size).toBe(1);
    expect(groups.get("unknown-success")?.length).toBe(1);
  });
});

describe("Pattern Extraction Prompt", () => {
  const makeTrajectory = (outcome: Trajectory["outcome"], desc: string): Trajectory => ({
    id: generateTrajectoryId(),
    taskId: "task-1",
    taskDescription: desc,
    actions: [
      {
        type: "tool_call",
        tool: "read",
        content: "read src/index.ts for context",
        timestamp: new Date().toISOString(),
      },
    ],
    outcome,
    skillsUsed: ["skill-1"],
    filesModified: [],
    totalDurationMs: 1000,
    model: "fm",
    tokens: { input: 10, output: 10, total: 20 },
    timestamp: new Date().toISOString(),
    archived: false,
  });

  test("buildPatternExtractionPrompt includes successful trajectories", () => {
    const trajectories = [
      makeTrajectory("success", "Fix import error"),
      makeTrajectory("success", "Add new feature"),
    ];

    const prompt = buildPatternExtractionPrompt(trajectories);

    expect(prompt).toContain("Successful Trajectories");
    expect(prompt).toContain("Fix import error");
    expect(prompt).toContain("Add new feature");
    expect(prompt).toContain("skill-1");
  });

  test("buildPatternExtractionPrompt includes failed trajectories", () => {
    const trajectories = [
      makeTrajectory("success", "Good task"),
      { ...makeTrajectory("failure", "Bad task"), errorMessage: "Something broke" },
    ];

    const prompt = buildPatternExtractionPrompt(trajectories);

    expect(prompt).toContain("Failed Trajectories");
    expect(prompt).toContain("Bad task");
    expect(prompt).toContain("Something broke");
  });

  test("buildPatternExtractionPrompt includes extraction instructions", () => {
    const trajectories = [makeTrajectory("success", "Test task")];

    const prompt = buildPatternExtractionPrompt(trajectories);

    expect(prompt).toContain("Extract Patterns");
    expect(prompt).toContain("name");
    expect(prompt).toContain("type");
    expect(prompt).toContain("description");
    expect(prompt).toContain("JSON array");
  });
});

describe("Pattern Parsing", () => {
  test("parsePatternsFromResponse parses valid JSON array", () => {
    const response = `
Here are the patterns I found:

[
  {
    "name": "Import Fix Pattern",
    "type": "skill",
    "description": "Fixes missing imports",
    "content": "import { foo } from 'bar'",
    "triggerContext": ["import error"],
    "category": "debugging"
  }
]

These patterns were extracted from the trajectories.
`;

    const patterns = parsePatternsFromResponse(response, ["traj-1", "traj-2"]);

    expect(patterns.length).toBe(1);
    expect(patterns[0].name).toBe("Import Fix Pattern");
    expect(patterns[0].type).toBe("skill");
    expect(patterns[0].description).toBe("Fixes missing imports");
    expect(patterns[0].sourceTrajectoryIds).toEqual(["traj-1", "traj-2"]);
    expect(patterns[0].occurrences).toBe(2);
    expect(patterns[0].extractedAt).toBeDefined();
  });

  test("parsePatternsFromResponse handles empty response", () => {
    const patterns = parsePatternsFromResponse("No patterns found", ["traj-1"]);
    expect(patterns).toEqual([]);
  });

  test("parsePatternsFromResponse handles empty array", () => {
    const patterns = parsePatternsFromResponse("[]", ["traj-1"]);
    expect(patterns).toEqual([]);
  });

  test("parsePatternsFromResponse handles invalid JSON", () => {
    const patterns = parsePatternsFromResponse("{ invalid json }", ["traj-1"]);
    expect(patterns).toEqual([]);
  });

  test("parsePatternsFromResponse fills missing fields", () => {
    const response = '[{ "name": "Test" }]';
    const patterns = parsePatternsFromResponse(response, ["traj-1"]);

    expect(patterns.length).toBe(1);
    expect(patterns[0].name).toBe("Test");
    expect(patterns[0].type).toBe("skill");
    expect(patterns[0].description).toBe("");
    expect(patterns[0].content).toBe("");
    expect(patterns[0].triggerContext).toEqual([]);
    expect(patterns[0].tags).toEqual([]);
  });
});
