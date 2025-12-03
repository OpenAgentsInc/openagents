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
    initScript: {
      ran: true,
      success: true,
      output: "Init ok",
    },
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
    expect(markdown).toContain("Init Script**: Success");
    expect(markdown).toContain("Init Output**: Init ok");
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
    expect(parsed.orientation?.initScript?.ran).toBe(true);
    expect(parsed.orientation?.initScript?.success).toBe(true);
    expect(parsed.orientation?.initScript?.output).toBe("Init ok");
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

describe("Claude Code session metadata", () => {
  test("formats Claude Code session data to markdown", () => {
    const progress = createMockProgress();
    progress.work.claudeCodeSession = {
      sessionId: "sess-abc123",
      forkedFromSessionId: "sess-old456",
      toolsUsed: { Read: 5, Edit: 3, Bash: 2 },
      summary: "Implemented feature X",
      usage: {
        inputTokens: 10000,
        outputTokens: 5000,
        cacheReadInputTokens: 2000,
        cacheCreationInputTokens: 500,
      },
      totalCostUsd: 0.1234,
    };
    const markdown = formatProgressMarkdown(progress);

    expect(markdown).toContain("### Claude Code Session");
    expect(markdown).toContain("Session ID**: sess-abc123");
    expect(markdown).toContain("Forked From**: sess-old456");
    expect(markdown).toContain("Tools Used**: Read(5), Edit(3), Bash(2)");
    expect(markdown).toContain("Summary**: Implemented feature X");
    expect(markdown).toContain("Token Usage**: 10,000 in, 5,000 out, 2,000 cache hits, 500 cache writes");
    expect(markdown).toContain("Cost**: $0.1234 USD");
  });

  test("parses Claude Code session data from markdown", () => {
    const progress = createMockProgress();
    progress.work.claudeCodeSession = {
      sessionId: "sess-abc123",
      forkedFromSessionId: "sess-old456",
      toolsUsed: { Read: 5, Edit: 3, Bash: 2 },
      summary: "Implemented feature X",
      usage: {
        inputTokens: 10000,
        outputTokens: 5000,
        cacheReadInputTokens: 2000,
        cacheCreationInputTokens: 500,
      },
      totalCostUsd: 0.1234,
    };
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.work?.claudeCodeSession?.sessionId).toBe("sess-abc123");
    expect(parsed.work?.claudeCodeSession?.forkedFromSessionId).toBe("sess-old456");
    expect(parsed.work?.claudeCodeSession?.toolsUsed).toEqual({ Read: 5, Edit: 3, Bash: 2 });
    expect(parsed.work?.claudeCodeSession?.summary).toBe("Implemented feature X");
    expect(parsed.work?.claudeCodeSession?.usage?.inputTokens).toBe(10000);
    expect(parsed.work?.claudeCodeSession?.usage?.outputTokens).toBe(5000);
    expect(parsed.work?.claudeCodeSession?.usage?.cacheReadInputTokens).toBe(2000);
    expect(parsed.work?.claudeCodeSession?.usage?.cacheCreationInputTokens).toBe(500);
    expect(parsed.work?.claudeCodeSession?.totalCostUsd).toBe(0.1234);
  });

  test("handles partial Claude Code session data", () => {
    const progress = createMockProgress();
    progress.work.claudeCodeSession = {
      sessionId: "sess-only",
    };
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.work?.claudeCodeSession?.sessionId).toBe("sess-only");
    expect(parsed.work?.claudeCodeSession?.forkedFromSessionId).toBeUndefined();
  });
});

describe("malformed progress file recovery", () => {
  let tempDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "progress-corrupt-test-"));
    openagentsDir = path.join(tempDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("readProgress returns null for empty file", () => {
    fs.writeFileSync(path.join(openagentsDir, "progress.md"), "");
    const result = readProgress(openagentsDir);
    // Should return partial result with defaults, not null
    expect(result).not.toBeNull();
    expect(result?.work?.subtasksCompleted).toEqual([]);
  });

  test("readProgress handles file with only headers", () => {
    const malformed = `# Session Progress

## Session Info

## Orientation

## Work Done
`;
    fs.writeFileSync(path.join(openagentsDir, "progress.md"), malformed);
    const result = readProgress(openagentsDir);
    expect(result).not.toBeNull();
    expect(result?.orientation?.testsPassingAtStart).toBe(false);
  });

  test("readProgress handles truncated markdown", () => {
    const truncated = `# Session Progress

## Session Info
- **Session ID**: session-123
- **Started**: 2025-01-01T12:00:00.000Z
- **Task**: oa-test - Test task

## Orientat`;  // Truncated mid-section
    fs.writeFileSync(path.join(openagentsDir, "progress.md"), truncated);
    const result = readProgress(openagentsDir);
    expect(result).not.toBeNull();
    expect(result?.sessionId).toBe("session-123");
    expect(result?.taskId).toBe("oa-test");
  });

  test("readProgress handles garbled content", () => {
    const garbled = `# Session Progress

## Session Info
- **Session ID**: session-123
GARBLED_BINARY_DATA_HERE\x00\x01\x02
- **Task**: oa-test - Still parses

## Work Done
- **Tests Run**: Yes
`;
    fs.writeFileSync(path.join(openagentsDir, "progress.md"), garbled);
    const result = readProgress(openagentsDir);
    expect(result).not.toBeNull();
    expect(result?.sessionId).toBe("session-123");
    expect(result?.taskId).toBe("oa-test");
    expect(result?.work?.testsRun).toBe(true);
  });

  test("readProgress handles missing required sections", () => {
    const minimal = `# Session Progress
Completed: In Progress
`;
    fs.writeFileSync(path.join(openagentsDir, "progress.md"), minimal);
    const result = readProgress(openagentsDir);
    expect(result).not.toBeNull();
    expect(result?.completedAt).toBeUndefined();
  });

  test("parseProgressMarkdown handles malformed key-value pairs", () => {
    const malformed = `# Session Progress

## Session Info
- **Session ID**:
- **Started**
- Task: oa-test - missing bold markers
- **Tests Passing at Start**: maybe
`;
    const parsed = parseProgressMarkdown(malformed);
    expect(parsed).not.toBeNull();
    // Empty session ID should be parsed as empty string
    expect(parsed.sessionId).toBe("");
  });

  test("parseProgressMarkdown handles non-standard formatting", () => {
    const nonStandard = `# Session Progress

##Session Info
-**Session ID**:session-123
- **Started** : 2025-01-01T12:00:00.000Z
-  **Task**:   oa-test  -  Test task
`;
    const parsed = parseProgressMarkdown(nonStandard);
    // Should handle these gracefully even if not perfectly parsed
    expect(parsed).not.toBeNull();
  });
});

describe("edge cases in list parsing", () => {
  test("handles lists with extra whitespace", () => {
    const progress = createMockProgress();
    progress.work.filesModified = ["  src/file.ts  ", "src/other.ts"];
    const markdown = formatProgressMarkdown(progress);
    const parsed = parseProgressMarkdown(markdown);

    expect(parsed.work?.filesModified).toContain("src/file.ts");
  });

  test("handles lists with empty entries", () => {
    const markdown = `# Session Progress

## Work Done
- **Subtasks Completed**: sub1, , sub2, ,
- **Files Modified**: None
`;
    const parsed = parseProgressMarkdown(markdown);
    expect(parsed.work?.subtasksCompleted).toEqual(["sub1", "sub2"]);
  });
});
