import { describe, expect, it } from "vitest";
import {
  formatGoalNotice,
  goalBudgetExhaustedPrompt,
  goalContinuationPrompt,
  goalTool,
  InMemoryGoalStore,
  isGoalSlashCommand,
  parseGoalSlashCommand,
} from "../src/coder-goals.js";

describe("isGoalSlashCommand", () => {
  it("matches standard /goal and variations", () => {
    expect(isGoalSlashCommand("/goal")).toBe(true);
    expect(isGoalSlashCommand("/goal foo bar")).toBe(true);
    expect(isGoalSlashCommand("/goooal build the thing")).toBe(true);
    expect(isGoalSlashCommand("/goooooooooal")).toBe(true);
    expect(isGoalSlashCommand("  /goal clear  ")).toBe(true);
  });

  it("does not match non-goal slash commands or regular text", () => {
    expect(isGoalSlashCommand("/help")).toBe(false);
    expect(isGoalSlashCommand("/reload")).toBe(false);
    expect(isGoalSlashCommand("what is our goal?")).toBe(false);
    expect(isGoalSlashCommand("goal")).toBe(false);
  });
});

describe("parseGoalSlashCommand", () => {
  it("parses status command", () => {
    expect(parseGoalSlashCommand("/goal")).toEqual({ kind: "status" });
    expect(parseGoalSlashCommand("/goal status")).toEqual({ kind: "status" });
    expect(parseGoalSlashCommand("/goooal")).toEqual({ kind: "status" });
  });

  it("parses control commands", () => {
    expect(parseGoalSlashCommand("/goal clear")).toEqual({ kind: "clear" });
    expect(parseGoalSlashCommand("/goal pause")).toEqual({ kind: "pause" });
    expect(parseGoalSlashCommand("/goal resume")).toEqual({ kind: "resume" });
  });

  it("parses set command with objective", () => {
    expect(parseGoalSlashCommand("/goal build persistent goals")).toEqual({
      kind: "set",
      objective: "build persistent goals",
    });
  });

  it("parses set command with --budget flag", () => {
    expect(parseGoalSlashCommand("/goal --budget 25000 refactor database schema")).toEqual({
      kind: "set",
      objective: "refactor database schema",
      tokenBudget: 25000,
    });
  });
});

describe("InMemoryGoalStore and goalTool", () => {
  it("manages goal lifecycle and usage tracking", async () => {
    const store = new InMemoryGoalStore();
    expect(store.getGoal()).toBeUndefined();

    const goal = store.setGoal("Write test suite", 10000);
    expect(goal.objective).toBe("Write test suite");
    expect(goal.status).toBe("active");
    expect(goal.tokenBudget).toBe(10000);
    expect(goal.tokensUsed).toBe(0);

    store.addUsage(2500, 10);
    expect(store.getGoal()?.tokensUsed).toBe(2500);
    expect(store.getGoal()?.timeUsedSeconds).toBe(10);
    expect(store.getGoal()?.status).toBe("active");

    // Exceeding budget marks goal budget_limited
    store.addUsage(8000, 15);
    expect(store.getGoal()?.tokensUsed).toBe(10500);
    expect(store.getGoal()?.status).toBe("budget_limited");

    // Tool interactions
    const tool = goalTool(store);
    const completeRes = await tool.run({ action: "complete" }, new AbortController().signal);
    expect(completeRes).toContain("marked as completed");
    expect(store.getGoal()?.status).toBe("completed");

    // Clear
    expect(store.clearGoal()).toBe(true);
    expect(store.getGoal()).toBeUndefined();
  });

  it("no longer offers or accepts a get action", async () => {
    // Reading the goal stopped being an action (issue #60): the objective
    // rides every outgoing turn, so the tool keeps only the state changes the
    // model genuinely decides.
    const store = new InMemoryGoalStore();
    store.setGoal("Write test suite");
    const tool = goalTool(store);

    const parameters = tool.parameters as {
      properties: { action: { enum: string[] } };
    };
    expect(parameters.properties.action.enum).toEqual(["complete", "block", "pause", "resume"]);
    expect(tool.description).not.toContain("'get'");

    const rejected = await tool.run({ action: "get" }, new AbortController().signal);
    expect(rejected).toContain("Unknown action");
    expect(store.getGoal()?.status).toBe("active");
  });
});

describe("formatGoalNotice and Prompts", () => {
  it("formats notice when no goal set", () => {
    const notice = formatGoalNotice(undefined);
    expect(notice).toContain("No active task goal.");
    expect(notice).toContain("Usage:");
  });

  it("formats notice with active goal and budget", () => {
    const store = new InMemoryGoalStore();
    const goal = store.setGoal("Implement feature", 50000);
    store.addUsage(1200, 5);

    const notice = formatGoalNotice(goal);
    expect(notice).toContain('Active Goal (active):');
    expect(notice).toContain('"Implement feature"');
    expect(notice).toContain("Budget: 1,200 / 50,000 tokens");
    expect(notice).toContain("Time Spent: 5s");
  });

  it("generates continuation prompt", () => {
    const store = new InMemoryGoalStore();
    const goal = store.setGoal("Implement feature", 50000);
    store.addUsage(10000, 20);

    const prompt = goalContinuationPrompt(goal);
    expect(prompt).toContain("Continue working toward the active task goal.");
    expect(prompt).toContain("<objective>\nImplement feature\n</objective>");
    expect(prompt).toContain("Token budget remaining: 40,000 tokens");
  });

  it("generates budget exhausted prompt", () => {
    const store = new InMemoryGoalStore();
    const goal = store.setGoal("Implement feature", 5000);
    store.addUsage(6000, 30);

    const prompt = goalBudgetExhaustedPrompt(goal);
    expect(prompt).toContain("reached its configured token budget");
    expect(prompt).toContain("budget_limited");
  });
});
