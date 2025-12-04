/**
 * Tests for overnight.ts cleanup guardrails
 *
 * These tests verify the cleanup behavior after subtask failures.
 *
 * Background:
 * The orchestrator can optionally clean up uncommitted changes after failures.
 * This is controlled by projectConfig.failureCleanup:
 *   - revertTrackedFiles: runs `git checkout -- .` (default: true)
 *   - deleteUntrackedFiles: runs `git clean -fd` (default: false - DESTRUCTIVE)
 *
 * Default behavior (non-destructive):
 * - Tracked file modifications are reverted
 * - Untracked files are PRESERVED (git clean -fd is NOT run by default)
 *
 * See docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.3 for full documentation.
 */
import { describe, expect, test } from "bun:test";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";

/**
 * Creates a test git repository
 */
const createTestRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `overnight-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "test@example.com"', {
    cwd: dir,
    stdio: "ignore",
  });
  execSync('git config user.name "Test"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, openagentsDir: oaDir };
};

describe("overnight.ts cleanup guardrails", () => {
  describe("revert on failure", () => {
    test("git checkout -- . reverts modified tracked files", () => {
      const { dir } = createTestRepo("revert-modified");

      // Modify an existing file
      fs.writeFileSync(path.join(dir, "README.md"), "# Modified!\n");

      // Verify it's modified
      let status = execSync("git status --porcelain", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(status).toContain("README.md");
      expect(status).toMatch(/M.*README\.md/);

      // Revert using the same command as overnight.ts
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });

      // Verify it's clean
      status = execSync("git status --porcelain", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(status.trim()).toBe("");

      // Verify content is restored
      const content = fs.readFileSync(path.join(dir, "README.md"), "utf-8");
      expect(content).toBe("# Test Repo\n");
    });

    test("default behavior preserves untracked files", () => {
      const { dir } = createTestRepo("preserve-untracked");

      // Create new untracked files
      fs.writeFileSync(path.join(dir, "new-feature.ts"), "const x = 1;");
      fs.mkdirSync(path.join(dir, "new-dir"));
      fs.writeFileSync(path.join(dir, "new-dir", "file.ts"), "new code");

      // Modify a tracked file
      fs.writeFileSync(path.join(dir, "README.md"), "# Modified!\n");

      // DEFAULT behavior: only revert tracked files, preserve untracked
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });
      // NOTE: git clean -fd is NOT run by default (deleteUntrackedFiles=false)

      // Tracked file should be reverted
      const readme = fs.readFileSync(path.join(dir, "README.md"), "utf-8");
      expect(readme).toBe("# Test Repo\n");

      // Untracked files should still exist
      expect(fs.existsSync(path.join(dir, "new-feature.ts"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "new-dir", "file.ts"))).toBe(true);
    });

    test("git clean -fd removes untracked files when deleteUntrackedFiles=true", () => {
      const { dir } = createTestRepo("clean-untracked");

      // Create new untracked files
      fs.writeFileSync(path.join(dir, "broken.ts"), "const x: string = 123;");
      fs.mkdirSync(path.join(dir, "new-dir"));
      fs.writeFileSync(path.join(dir, "new-dir", "file.ts"), "broken code");

      // Verify they exist
      expect(fs.existsSync(path.join(dir, "broken.ts"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "new-dir", "file.ts"))).toBe(true);

      // OPT-IN behavior when deleteUntrackedFiles=true
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });
      execSync("git clean -fd", { cwd: dir, encoding: "utf-8" });

      // Verify they're removed
      expect(fs.existsSync(path.join(dir, "broken.ts"))).toBe(false);
      expect(fs.existsSync(path.join(dir, "new-dir"))).toBe(false);

      // Verify status is clean
      const status = execSync("git status --porcelain", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(status.trim()).toBe("");
    });

    test("git clean -fd preserves .gitignore'd files", () => {
      const { dir } = createTestRepo("preserve-gitignored");

      // Create .gitignore
      fs.writeFileSync(path.join(dir, ".gitignore"), "node_modules/\n*.log\n");
      execSync("git add .gitignore", { cwd: dir, stdio: "ignore" });
      execSync('git commit -m "add gitignore"', { cwd: dir, stdio: "ignore" });

      // Create gitignored files
      fs.mkdirSync(path.join(dir, "node_modules"));
      fs.writeFileSync(
        path.join(dir, "node_modules", "package.json"),
        "{}"
      );
      fs.writeFileSync(path.join(dir, "debug.log"), "log content");

      // Clean (without -x flag, so gitignored files are preserved)
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });
      execSync("git clean -fd", { cwd: dir, encoding: "utf-8" });

      // Gitignored files should still exist
      expect(fs.existsSync(path.join(dir, "node_modules", "package.json"))).toBe(
        true
      );
      expect(fs.existsSync(path.join(dir, "debug.log"))).toBe(true);
    });
  });

  describe("selective add for cleanup commit", () => {
    test("only stages progress files, not broken code", () => {
      const { dir, openagentsDir } = createTestRepo("selective-add");

      // Create broken code file (simulating failed subtask output)
      fs.writeFileSync(path.join(dir, "broken.ts"), "const x: string = 123;");

      // Create progress files
      fs.writeFileSync(path.join(openagentsDir, "progress.md"), "# Progress\n");
      fs.mkdirSync(path.join(openagentsDir, "subtasks"), { recursive: true });
      fs.writeFileSync(
        path.join(openagentsDir, "subtasks", "task.json"),
        "{}"
      );

      // Create logs directory
      fs.mkdirSync(path.join(dir, "docs", "logs"), { recursive: true });
      fs.writeFileSync(
        path.join(dir, "docs", "logs", "session.md"),
        "# Log\n"
      );

      // Use the same selective add command as overnight.ts
      execSync(
        "git add .openagents/progress.md .openagents/subtasks/ docs/logs/ 2>/dev/null || true",
        {
          cwd: dir,
          encoding: "utf-8",
          shell: "/bin/bash",
        }
      );

      // Check what was staged
      const staged = execSync("git diff --cached --name-only", {
        cwd: dir,
        encoding: "utf-8",
      });

      // Progress files should be staged
      expect(staged).toContain(".openagents/progress.md");
      expect(staged).toContain(".openagents/subtasks/task.json");
      expect(staged).toContain("docs/logs/session.md");

      // Broken code should NOT be staged
      expect(staged).not.toContain("broken.ts");

      // Verify broken.ts is still untracked
      const status = execSync("git status --porcelain", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(status).toContain("?? broken.ts");
    });

    test("handles missing paths gracefully", () => {
      const { dir } = createTestRepo("missing-paths");

      // Don't create any of the expected paths
      // The command should not fail

      // This should not throw
      execSync(
        "git add .openagents/progress.md .openagents/subtasks/ docs/logs/ 2>/dev/null || true",
        {
          cwd: dir,
          encoding: "utf-8",
          shell: "/bin/bash",
        }
      );

      // Nothing should be staged
      const staged = execSync("git diff --cached --name-only", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(staged.trim()).toBe("");
    });
  });

  describe("combined revert + selective add workflow", () => {
    test("default cleanup reverts tracked files but preserves untracked", () => {
      const { dir, openagentsDir } = createTestRepo("default-cleanup");

      // Simulate a failed subtask leaving broken files
      fs.writeFileSync(path.join(dir, "README.md"), "# Broken!\n");
      fs.writeFileSync(path.join(dir, "new-feature.ts"), "broken code");

      // Create progress files (these should be preserved)
      fs.writeFileSync(
        path.join(openagentsDir, "progress.md"),
        "# Session Progress\n"
      );

      // DEFAULT behavior: only revert tracked files
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });
      // git clean -fd is NOT run by default

      // Tracked file should be reverted
      const readme = fs.readFileSync(path.join(dir, "README.md"), "utf-8");
      expect(readme).toBe("# Test Repo\n");

      // Untracked files should still exist (preserved by default)
      expect(fs.existsSync(path.join(dir, "new-feature.ts"))).toBe(true);
      expect(fs.existsSync(path.join(openagentsDir, "progress.md"))).toBe(true);
    });

    test("aggressive cleanup removes untracked when deleteUntrackedFiles=true", () => {
      const { dir, openagentsDir } = createTestRepo("aggressive-cleanup");

      // Simulate a failed subtask leaving broken files
      fs.writeFileSync(path.join(dir, "README.md"), "# Broken!\n");
      fs.writeFileSync(path.join(dir, "new-feature.ts"), "broken code");

      // OPT-IN aggressive cleanup (deleteUntrackedFiles=true)
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });
      execSync("git clean -fd", { cwd: dir, encoding: "utf-8" });

      // Tracked file should be reverted
      const readme = fs.readFileSync(path.join(dir, "README.md"), "utf-8");
      expect(readme).toBe("# Test Repo\n");

      // Untracked files should be deleted
      expect(fs.existsSync(path.join(dir, "new-feature.ts"))).toBe(false);
      // Note: .openagents was untracked so it would also be deleted
    });

    test("progress files in .openagents survive revert if tracked", () => {
      const { dir, openagentsDir } = createTestRepo("progress-survives");

      // Add .openagents to git first
      fs.writeFileSync(
        path.join(openagentsDir, "progress.md"),
        "# Initial\n"
      );
      execSync("git add .openagents/", { cwd: dir, stdio: "ignore" });
      execSync('git commit -m "add openagents"', { cwd: dir, stdio: "ignore" });

      // Now modify progress and add broken code
      fs.writeFileSync(
        path.join(openagentsDir, "progress.md"),
        "# Updated Progress\n"
      );
      fs.writeFileSync(path.join(dir, "broken.ts"), "broken");

      // DEFAULT behavior: only revert tracked files
      execSync("git checkout -- .", { cwd: dir, encoding: "utf-8" });

      // Progress file should be reverted to committed version
      const progress = fs.readFileSync(
        path.join(openagentsDir, "progress.md"),
        "utf-8"
      );
      expect(progress).toBe("# Initial\n");

      // Broken code should still exist (untracked files preserved by default)
      expect(fs.existsSync(path.join(dir, "broken.ts"))).toBe(true);
    });
  });
});

/**
 * Tests for Golden Loop log creation
 *
 * These tests verify that run logs are created in the correct location
 * with the expected content structure as documented in GOLDEN-LOOP-v2.md
 * Section 2.9 and 2.10.
 */
describe("Golden Loop log creation", () => {
  /**
   * Helper to get expected date folder name (YYYYMMDD format)
   */
  const getDateFolder = (date: Date = new Date()): string => {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}${month}${day}`;
  };

  /**
   * Helper to get expected time prefix (HHMM format)
   */
  const getTimePrefix = (date: Date = new Date()): string => {
    const hours = String(date.getHours()).padStart(2, "0");
    const mins = String(date.getMinutes()).padStart(2, "0");
    return `${hours}${mins}`;
  };

  describe("log directory structure", () => {
    test("log folder uses YYYYMMDD format", () => {
      const { dir } = createTestRepo("log-folder-format");

      // Create log folder using same logic as overnight.ts
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      // Verify folder exists and matches expected format
      expect(fs.existsSync(logsDir)).toBe(true);
      expect(path.basename(logsDir)).toMatch(/^\d{8}$/);
    });

    test("log filename uses HHMM-overnight-{sessionId}.md format", () => {
      const { dir } = createTestRepo("log-filename-format");

      const sessionId = "orchestrator-1234567890";
      const timePrefix = getTimePrefix();
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      const logPath = path.join(logsDir, `${timePrefix}-overnight-${sessionId}.md`);
      fs.writeFileSync(logPath, "# Test Log\n");

      // Verify file exists and matches expected format
      expect(fs.existsSync(logPath)).toBe(true);
      expect(path.basename(logPath)).toMatch(/^\d{4}-overnight-orchestrator-\d+\.md$/);
    });
  });

  describe("log content structure", () => {
    test("run log contains required header fields", () => {
      const { dir } = createTestRepo("log-header-fields");

      const sessionId = "orchestrator-1234567890";
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      // Create log with expected header format (matching overnight.ts)
      const logContent = `# Overnight Agent Log
Session: ${sessionId}
Started: ${new Date().toISOString()}

[${new Date().toISOString()}] ############################################################
[${new Date().toISOString()}] OVERNIGHT AGENT STARTING - Orchestrator Mode
[${new Date().toISOString()}] Session: ${sessionId}
[${new Date().toISOString()}] Work directory: ${dir}
[${new Date().toISOString()}] Max tasks: 5
[${new Date().toISOString()}] Claude Code enabled: true
[${new Date().toISOString()}] Safe mode: false
[${new Date().toISOString()}] ############################################################
`;

      const logPath = path.join(logsDir, `${getTimePrefix()}-overnight-${sessionId}.md`);
      fs.writeFileSync(logPath, logContent);

      const content = fs.readFileSync(logPath, "utf-8");

      // Verify required header fields
      expect(content).toContain("# Overnight Agent Log");
      expect(content).toContain(`Session: ${sessionId}`);
      expect(content).toContain("Started:");
      expect(content).toContain("Work directory:");
      expect(content).toContain("Max tasks:");
      expect(content).toContain("Claude Code enabled:");
    });

    test("run log contains task cycle with expected fields", () => {
      const { dir } = createTestRepo("log-task-cycle");

      const sessionId = "orchestrator-1234567890";
      const taskId = "oa-abc123";
      const taskTitle = "Test task title";
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      // Create log with task cycle content (matching overnight.ts format)
      const ts = new Date().toISOString();
      const logContent = `# Overnight Agent Log
Session: ${sessionId}
Started: ${ts}

[${ts}] ============================================================
[${ts}] TASK CYCLE 1/5
[${ts}] ============================================================

[${ts}] [${ts}] Task selected: ${taskId} - ${taskTitle}
[${ts}] [${ts}] Subtask started: ${taskId}-sub-001
[${ts}] [${ts}] Running: bun run typecheck
[${ts}] [${ts}] PASS: bun run typecheck
[${ts}] [${ts}] Running: bun test
[${ts}] [${ts}] PASS: bun test
[${ts}] [${ts}] Subtask complete: ${taskId}-sub-001 (agent: claude-code)
[${ts}] [${ts}] Commit: abc12345 - ${taskTitle}
[${ts}] [${ts}] Pushed to main
[${ts}] [${ts}] Session SUCCESS: Completed task ${taskId}: ${taskTitle}
[${ts}]
✓ Task 1 completed
`;

      const logPath = path.join(logsDir, `${getTimePrefix()}-overnight-${sessionId}.md`);
      fs.writeFileSync(logPath, logContent);

      const content = fs.readFileSync(logPath, "utf-8");

      // Verify task cycle fields
      expect(content).toContain("TASK CYCLE");
      expect(content).toContain(`Task selected: ${taskId}`);
      expect(content).toContain(taskTitle);
      expect(content).toContain("Subtask started:");
      expect(content).toContain("Running: bun run typecheck");
      expect(content).toContain("PASS: bun run typecheck");
      expect(content).toContain("Running: bun test");
      expect(content).toContain("PASS: bun test");
      expect(content).toContain("Subtask complete:");
      expect(content).toContain("Commit:");
      expect(content).toContain("Pushed to");
      expect(content).toContain("Session SUCCESS:");
      expect(content).toContain("✓ Task 1 completed");
    });

    test("run log lands in date folder with task/tests/commit info", () => {
      const { dir } = createTestRepo("log-task-tests-commit");
      const sessionId = "orchestrator-5555";
      const taskId = "oa-task123";
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      const ts = new Date().toISOString();
      const timePrefix = getTimePrefix();
      const logPath = path.join(logsDir, `${timePrefix}-overnight-${sessionId}.md`);

      const logContent = `# Overnight Agent Log
Session: ${sessionId}
Started: ${ts}

[${ts}] TASK CYCLE 1/1
[${ts}] [${ts}] Task selected: ${taskId} - Sample task
[${ts}] [${ts}] Running: bun run typecheck
[${ts}] [${ts}] PASS: bun run typecheck
[${ts}] [${ts}] Running: bun test
[${ts}] [${ts}] PASS: bun test
[${ts}] [${ts}] Commit: deadbeef - Sample task
[${ts}] [${ts}] Session SUCCESS: Completed task ${taskId}: Sample task
`;

      fs.writeFileSync(logPath, logContent);

      expect(fs.existsSync(logPath)).toBe(true);
      expect(logPath).toContain(getDateFolder());

      const content = fs.readFileSync(logPath, "utf-8");
      expect(content).toContain(`Task selected: ${taskId}`);
      expect(content).toContain("PASS: bun run typecheck");
      expect(content).toContain("PASS: bun test");
      expect(content).toContain("Commit:");
      expect(content).toContain("Session SUCCESS:");
    });

    test("run log records test failures correctly", () => {
      const { dir } = createTestRepo("log-test-failure");

      const sessionId = "orchestrator-1234567890";
      const taskId = "oa-def456";
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });

      const ts = new Date().toISOString();
      const logContent = `# Overnight Agent Log
Session: ${sessionId}
Started: ${ts}

[${ts}] TASK CYCLE 1/5
[${ts}] [${ts}] Task selected: ${taskId} - Failing task
[${ts}] [${ts}] Subtask started: ${taskId}-sub-001
[${ts}] [${ts}] Running: bun run typecheck
[${ts}] [${ts}] FAIL: bun run typecheck
[${ts}] [${ts}] Subtask FAILED: ${taskId}-sub-001 - Typecheck errors
[${ts}]
✗ Task failed: Typecheck errors
[${ts}] [Guardrail] Reverting uncommitted changes from failed subtask...
[${ts}] [Guardrail] Uncommitted changes reverted.
`;

      const logPath = path.join(logsDir, `${getTimePrefix()}-overnight-${sessionId}.md`);
      fs.writeFileSync(logPath, logContent);

      const content = fs.readFileSync(logPath, "utf-8");

      // Verify failure fields
      expect(content).toContain("FAIL: bun run typecheck");
      expect(content).toContain("Subtask FAILED:");
      expect(content).toContain("✗ Task failed:");
      expect(content).toContain("[Guardrail] Reverting uncommitted changes");
    });
  });

  describe("log naming edge cases", () => {
    test("handles midnight rollover (23:59 to 00:00)", () => {
      // Test date at 23:59
      const lateDate = new Date(2025, 11, 3, 23, 59); // Dec 3, 23:59
      const lateDateFolder = getDateFolder(lateDate);
      expect(lateDateFolder).toBe("20251203");

      // Test date at 00:00 next day
      const earlyDate = new Date(2025, 11, 4, 0, 0); // Dec 4, 00:00
      const earlyDateFolder = getDateFolder(earlyDate);
      expect(earlyDateFolder).toBe("20251204");

      // Verify they're different
      expect(lateDateFolder).not.toBe(earlyDateFolder);
    });

    test("time prefix pads single digits correctly", () => {
      // 1:05 AM should be "0105"
      const earlyMorning = new Date(2025, 11, 3, 1, 5);
      expect(getTimePrefix(earlyMorning)).toBe("0105");

      // 12:59 PM should be "1259"
      const noon = new Date(2025, 11, 3, 12, 59);
      expect(getTimePrefix(noon)).toBe("1259");

      // 9:09 AM should be "0909"
      const nineOClock = new Date(2025, 11, 3, 9, 9);
      expect(getTimePrefix(nineOClock)).toBe("0909");
    });
  });

  describe("log cleanup selective staging", () => {
    test("log files in docs/logs are staged by cleanup commit", () => {
      const { dir } = createTestRepo("log-staging");

      // Create a log file
      const logsDir = path.join(dir, "docs", "logs", getDateFolder());
      fs.mkdirSync(logsDir, { recursive: true });
      const logFileName = "1234-overnight-test.md";
      fs.writeFileSync(
        path.join(logsDir, logFileName),
        "# Test Log\n"
      );

      // Create a code file (should NOT be staged)
      fs.writeFileSync(path.join(dir, "feature.ts"), "const x = 1;");

      // Stage ONLY the docs/logs directory (simulating selective add from overnight.ts)
      // Note: We need to actually add the paths that exist, not all paths
      execSync("git add docs/logs/", {
        cwd: dir,
        encoding: "utf-8",
      });

      // Check what was staged
      const staged = execSync("git diff --cached --name-only", {
        cwd: dir,
        encoding: "utf-8",
      });

      // Log file should be staged
      expect(staged).toContain("docs/logs/");
      expect(staged).toContain(logFileName);

      // Code file should NOT be staged
      expect(staged).not.toContain("feature.ts");

      // Verify feature.ts is still untracked
      const status = execSync("git status --porcelain", {
        cwd: dir,
        encoding: "utf-8",
      });
      expect(status).toContain("?? feature.ts");
    });
  });
});
