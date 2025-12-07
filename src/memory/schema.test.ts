/**
 * Memory Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  generateMemoryId,
  createMemory,
  createEpisodicMemory,
  createSemanticMemory,
  createProceduralMemory,
  importanceToScore,
  calculateRecency,
  calculateMemoryScore,
  formatMemoriesForPrompt,
  buildMemoryText,
  DEFAULT_SCORING_WEIGHTS,
} from "./schema.js";

describe("Memory Schema", () => {
  test("generateMemoryId creates valid ID", () => {
    const id1 = generateMemoryId("episodic");
    const id2 = generateMemoryId("semantic");
    const id3 = generateMemoryId("procedural");

    expect(id1).toMatch(/^mem-epi-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^mem-sem-[a-z0-9]+-[a-z0-9]+$/);
    expect(id3).toMatch(/^mem-pro-[a-z0-9]+-[a-z0-9]+$/);

    // IDs should be unique
    expect(id1).not.toBe(id2);
  });

  test("createMemory creates memory with defaults", () => {
    const memory = createMemory({
      memoryType: "semantic",
      description: "Test memory",
      content: {
        type: "semantic",
        category: "pattern",
        knowledge: "Test knowledge",
      },
    });

    expect(memory.id).toMatch(/^mem-sem-/);
    expect(memory.memoryType).toBe("semantic");
    expect(memory.scope).toBe("project");
    expect(memory.status).toBe("active");
    expect(memory.importance).toBe("medium");
    expect(memory.accessCount).toBe(0);
    expect(memory.source).toBe("system");
    expect(memory.createdAt).toBeDefined();
    expect(memory.updatedAt).toBeDefined();
  });

  test("createMemory respects provided values", () => {
    const memory = createMemory({
      id: "custom-id",
      memoryType: "episodic",
      description: "Custom memory",
      content: {
        type: "episodic",
        taskDescription: "Test task",
        outcome: "success",
      },
      scope: "session",
      status: "archived",
      importance: "critical",
      tags: ["test", "custom"],
      projectId: "proj-123",
      sessionId: "sess-456",
      source: "user",
    });

    expect(memory.id).toBe("custom-id");
    expect(memory.scope).toBe("session");
    expect(memory.status).toBe("archived");
    expect(memory.importance).toBe("critical");
    expect(memory.tags).toEqual(["test", "custom"]);
    expect(memory.projectId).toBe("proj-123");
    expect(memory.sessionId).toBe("sess-456");
    expect(memory.source).toBe("user");
  });
});

describe("Episodic Memory", () => {
  test("createEpisodicMemory creates success memory", () => {
    const memory = createEpisodicMemory("Fix the bug", "success", {
      skillsUsed: ["skill-1"],
      filesModified: ["file.ts"],
      durationMs: 5000,
    });

    expect(memory.memoryType).toBe("episodic");
    expect(memory.content.type).toBe("episodic");
    expect((memory.content as any).outcome).toBe("success");
    expect((memory.content as any).skillsUsed).toEqual(["skill-1"]);
    expect(memory.importance).toBe("low"); // Success is low importance
    expect(memory.source).toBe("task");
  });

  test("createEpisodicMemory creates failure memory with higher importance", () => {
    const memory = createEpisodicMemory("Fix the bug", "failure", {
      errorMessage: "Type error",
    });

    expect((memory.content as any).outcome).toBe("failure");
    expect((memory.content as any).errorMessage).toBe("Type error");
    expect(memory.importance).toBe("high"); // Failures are high importance
  });
});

describe("Semantic Memory", () => {
  test("createSemanticMemory creates knowledge memory", () => {
    const memory = createSemanticMemory("pattern", "Use Effect for async", {
      context: "TypeScript development",
      examples: ["Effect.gen(function* () { ... })"],
    });

    expect(memory.memoryType).toBe("semantic");
    expect(memory.content.type).toBe("semantic");
    expect((memory.content as any).category).toBe("pattern");
    expect((memory.content as any).knowledge).toBe("Use Effect for async");
    expect((memory.content as any).context).toBe("TypeScript development");
    expect(memory.source).toBe("reflection");
  });
});

describe("Procedural Memory", () => {
  test("createProceduralMemory links to skill", () => {
    const memory = createProceduralMemory(
      "skill-fix-import-error-v1",
      ["import error", "cannot find module"],
      {
        successRate: 0.95,
        examples: ["Fixed import for Effect"],
      },
    );

    expect(memory.memoryType).toBe("procedural");
    expect(memory.content.type).toBe("procedural");
    expect((memory.content as any).skillId).toBe("skill-fix-import-error-v1");
    expect((memory.content as any).triggerPatterns).toContain("import error");
    expect((memory.content as any).successRate).toBe(0.95);
    expect(memory.source).toBe("system");
  });
});

describe("Importance Scoring", () => {
  test("importanceToScore returns correct values", () => {
    expect(importanceToScore("trivial")).toBe(0.1);
    expect(importanceToScore("low")).toBe(0.3);
    expect(importanceToScore("medium")).toBe(0.5);
    expect(importanceToScore("high")).toBe(0.7);
    expect(importanceToScore("critical")).toBe(1.0);
  });
});

describe("Recency Scoring", () => {
  test("calculateRecency returns 1 for recent access", () => {
    const now = new Date().toISOString();
    const recency = calculateRecency(now);
    expect(recency).toBeCloseTo(1.0, 1);
  });

  test("calculateRecency decays over time", () => {
    const oneHourAgo = new Date(Date.now() - 60 * 60 * 1000).toISOString();
    const oneDayAgo = new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString();

    const recentRecency = calculateRecency(oneHourAgo);
    const oldRecency = calculateRecency(oneDayAgo);

    expect(recentRecency).toBeGreaterThan(oldRecency);
    expect(recentRecency).toBeLessThan(1.0);
    expect(oldRecency).toBeLessThan(recentRecency);
  });
});

describe("Memory Scoring", () => {
  test("calculateMemoryScore combines factors", () => {
    const memory = createMemory({
      memoryType: "episodic",
      description: "Test",
      content: {
        type: "episodic",
        taskDescription: "Test",
        outcome: "success",
      },
      importance: "high",
      lastAccessedAt: new Date().toISOString(),
    });

    const score = calculateMemoryScore(memory, 0.8, DEFAULT_SCORING_WEIGHTS);

    // Score should be combination of recency (~1), importance (0.7), relevance (0.8)
    expect(score).toBeGreaterThan(2.0);
    expect(score).toBeLessThan(3.0);
  });

  test("calculateMemoryScore respects weights", () => {
    const memory = createMemory({
      memoryType: "semantic",
      description: "Test",
      content: {
        type: "semantic",
        category: "pattern",
        knowledge: "Test",
      },
      importance: "medium",
      lastAccessedAt: new Date().toISOString(),
    });

    const defaultScore = calculateMemoryScore(memory, 0.5, DEFAULT_SCORING_WEIGHTS);
    const importanceWeighted = calculateMemoryScore(memory, 0.5, {
      recency: 0.5,
      importance: 2.0,
      relevance: 0.5,
    });

    // Higher importance weight should boost score
    expect(importanceWeighted).toBeGreaterThan(defaultScore * 0.5);
  });
});

describe("Memory Formatting", () => {
  test("formatMemoriesForPrompt handles empty array", () => {
    const formatted = formatMemoriesForPrompt([]);
    expect(formatted).toBe("No relevant memories found.");
  });

  test("formatMemoriesForPrompt formats episodic memory", () => {
    const memory = createEpisodicMemory("Fix import error", "failure", {
      errorMessage: "Module not found",
    });

    const formatted = formatMemoriesForPrompt([memory]);

    expect(formatted).toContain("[episodic]");
    expect(formatted).toContain("Fix import error");
    expect(formatted).toContain("failure");
    expect(formatted).toContain("Module not found");
  });

  test("formatMemoriesForPrompt formats multiple memories", () => {
    const memories = [
      createEpisodicMemory("Task 1", "success"),
      createSemanticMemory("pattern", "Use Effect"),
      createProceduralMemory("skill-1", ["trigger"]),
    ];

    const formatted = formatMemoriesForPrompt(memories);

    expect(formatted).toContain("[episodic]");
    expect(formatted).toContain("[semantic]");
    expect(formatted).toContain("[procedural]");
  });
});

describe("Memory Text Building", () => {
  test("buildMemoryText includes all relevant parts", () => {
    const memory = createEpisodicMemory("Fix the import error", "failure", {
      errorMessage: "Cannot find module effect",
      tags: ["import", "typescript"],
    });

    const text = buildMemoryText(memory);

    expect(text).toContain("Fix the import error");
    expect(text).toContain("Cannot find module effect");
    expect(text).toContain("import");
    expect(text).toContain("typescript");
  });

  test("buildMemoryText handles semantic memory", () => {
    const memory = createSemanticMemory("convention", "Use camelCase", {
      context: "TypeScript naming",
    });

    const text = buildMemoryText(memory);

    expect(text).toContain("Use camelCase");
    expect(text).toContain("TypeScript naming");
    expect(text).toContain("convention");
  });

  test("buildMemoryText handles procedural memory", () => {
    const memory = createProceduralMemory("skill-test-v1", [
      "write test",
      "add test",
    ]);

    const text = buildMemoryText(memory);

    expect(text).toContain("write test");
    expect(text).toContain("add test");
    expect(text).toContain("skill");
  });
});
