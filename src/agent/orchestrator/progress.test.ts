import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import type { SessionProgress } from "./types.js";
import {
  writeProgress,
  readProgress,
  formatProgressMarkdown,
  parseProgressMarkdown,
  progressExists,
  getPreviousSessionSummary,
  createEmptyProgress,
} from "./progress.js";

const createMockProgress = (): SessionProgress => ({
  sessionId: "session-2025-01-01T12-00-00-abc123",
  startedAt: "2025-01-01T12:00:00.000Z",
  taskId: "oa-test01",
  taskTitle: "Test task",
  orientation: {
    repoState: "clean",
    testsPassingAtStart: true,
    previousSessionSummary: "Completed setup tasks",
  },
  work: {
    subtasksCompleted: ["oa-test01-sub-001"],
    subtasksInProgress: ["oa-test01-sub-002"],
    filesModified: ["src/test.ts", "src/utils.ts"],
    testsRun: true,
    testsPassingAfterWork: true,
  },
  nextSession: {
    suggestedNextSteps: ["Continue with sub-002", "Run e2e tests"],
    blockers: ["Waiting for API response"],
    notes: "Check the error logs",
  },
  completedAt: "2025-01-01T13:00:00.000Z",
});

describe("formatProgressMarkdown", () => {
  test("formats complete progress to markdown", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);

    expect(markdown).toContain("# Session Progress");
    expect(markdown).toContain("session-2025-01-01T12-00-00-abc123");
    expect(markdown).toContain("oa-test01 - Test task");
    expect(markdown).toContain("Repo State**: clean");
    expect(markdown).toContain("Tests Passing at Start**: Yes");
    expect(markdown).toContain("oa-test01-sub-001");
    expect(markdown).toContain("src/test.ts, src/utils.ts");
    expect(markdown).toContain("Continue with sub-002");
    expect(markdown).toContain("Waiting for API response");
    expect(markdown).toContain("Check the error logs");
  });

  test("handles empty lists", () => {
    const progress = createEmptyProgress("session-123", "oa-test", "Test");
    const markdown = formatProgressMarkdown(progress);

    expect(markdown).toContain("Subtasks Completed**: None");
    expect(markdown).toContain("Files Modified**: None");
  });

  test("handles missing optional fields", () => {
    const progress = createEmptyProgress("session-123", "oa-test", "Test");
    const markdown = formatProgressMarkdown(progress);

    expect(markdown).not.toContain("Previous Session");
    expect(markdown).not.toContain("### Blockers");
    expect(markdown).not.toContain("### Notes");
  });
});

describe("parseProgressMarkdown", () => {
  test("parses session info", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.sessionId).toBe("session-2025-01-01T12-00-00-abc123");
    expect(parsed.startedAt).toBe("2025-01-01T12:00:00.000Z");
    expect(parsed.taskId).toBe("oa-test01");
    expect(parsed.taskTitle).toBe("Test task");
  });

  test("parses orientation", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.orientation?.repoState).toBe("clean");
    expect(parsed.orientation?.testsPassingAtStart).toBe(true);
    expect(parsed.orientation?.previousSessionSummary).toBe("Completed setup tasks");
  });

  test("parses work done", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.work?.subtasksCompleted).toEqual(["oa-test01-sub-001"]);
    expect(parsed.work?.subtasksInProgress).toEqual(["oa-test01-sub-002"]);
    expect(parsed.work?.filesModified).toEqual(["src/test.ts", "src/utils.ts"]);
    expect(parsed.work?.testsRun).toBe(true);
    expect(parsed.work?.testsPassingAfterWork).toBe(true);
  });

  test("parses next session instructions", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.nextSession?.suggestedNextSteps).toContain("Continue with sub-002");
    expect(parsed.nextSession?.suggestedNextSteps).toContain("Run e2e tests");
    expect(parsed.nextSession?.blockers).toContain("Waiting for API response");
  });

  test("parses completed timestamp", () => {
    const progress = createMockProgress();
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.completedAt).toBe("2025-01-01T13:00:00.000Z");
  });

  test("handles in progress state", () => {
    const progress = createEmptyProgress("session-123", "oa-test", "Test");
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.completedAt).toBeUndefined();
  });

  test("handles empty lists", () => {
    const progress = createEmptyProgress("session-123", "oa-test", "Test");
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.work?.subtasksCompleted).toEqual([]);
    expect(parsed.work?.filesModified).toEqual([]);
  });
});

describe("file operations", () => {
  let tempDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "progress-test-"));
    openagentsDir = path.join(tempDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("writeProgress creates file", () => {
    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    expect(fs.existsSync(path.join(openagentsDir, "progress.md"))).toBe(true);
  });

  test("readProgress returns null for missing file", () => {
    const result = readProgress(openagentsDir);
    expect(result).toBeNull();
  });

  test("readProgress parses written file", () => {
    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    const parsed = readProgress(openagentsDir);
    expect(parsed).not.toBeNull();
    expect(parsed?.taskId).toBe("oa-test01");
    expect(parsed?.work?.testsPassingAfterWork).toBe(true);
  });

  test("progressExists returns correct value", () => {
    expect(progressExists(openagentsDir)).toBe(false);

    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    expect(progressExists(openagentsDir)).toBe(true);
  });
});

describe("getPreviousSessionSummary", () => {
  let tempDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "progress-test-"));
    openagentsDir = path.join(tempDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("returns null when no progress file", () => {
    const summary = getPreviousSessionSummary(openagentsDir);
    expect(summary).toBeNull();
  });

  test("returns summary with task info", () => {
    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    const summary = getPreviousSessionSummary(openagentsDir);
    expect(summary).not.toBeNull();
    expect(summary).toContain("oa-test01");
    expect(summary).toContain("Test task");
  });

  test("includes completed subtasks", () => {
    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    const summary = getPreviousSessionSummary(openagentsDir);
    expect(summary).toContain("oa-test01-sub-001");
  });

  test("includes blockers", () => {
    const progress = createMockProgress();
    writeProgress(openagentsDir, progress);

    const summary = getPreviousSessionSummary(openagentsDir);
    expect(summary).toContain("Waiting for API response");
  });
});

describe("createEmptyProgress", () => {
  test("creates progress with defaults", () => {
    const progress = createEmptyProgress("session-123", "oa-test", "Test Task");

    expect(progress.sessionId).toBe("session-123");
    expect(progress.taskId).toBe("oa-test");
    expect(progress.taskTitle).toBe("Test Task");
    expect(progress.orientation.testsPassingAtStart).toBe(false);
    expect(progress.work.subtasksCompleted).toEqual([]);
    expect(progress.nextSession.suggestedNextSteps).toEqual([]);
  });

  test("sets startedAt to current time", () => {
    const before = new Date().toISOString();
    const progress = createEmptyProgress("session-123", "oa-test", "Test");
    const after = new Date().toISOString();

    expect(progress.startedAt >= before).toBe(true);
    expect(progress.startedAt <= after).toBe(true);
  });
});
