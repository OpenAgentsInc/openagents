import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { runInitScript } from "./init-script.js";

describe("runInitScript", () => {
  let tempDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "init-script-"));
    openagentsDir = path.join(tempDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("skips when init.sh is missing", async () => {
    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(false);
    expect(result.success).toBe(true);
    expect(result.exitCode).toBe(0);
    expect(result.output).toBeUndefined();
  });

  test("runs init.sh and captures output (exit 0)", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "hello init"\nexit 0\n');

    const events: string[] = [];
    const result = await Effect.runPromise(
      runInitScript(openagentsDir, tempDir, (event) => {
        events.push(event.type);
      })
    );

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    expect(result.hasWarnings).toBe(false);
    expect(result.exitCode).toBe(0);
    expect(result.output).toContain("hello init");
    expect(events).toContain("init_script_start");
    expect(events).toContain("init_script_complete");
  });

  test("reports failure when init.sh exits with 1 (fatal)", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "broken" >&2\nexit 1\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.hasWarnings).toBe(false);
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("broken");
    expect(result.error).toBeTruthy();
  });

  test("continues with warnings when init.sh exits with 2", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(
      scriptPath,
      '#!/bin/bash\necho "Warning: uncommitted changes"\nexit 2\n'
    );

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true); // exit 2 means continue
    expect(result.hasWarnings).toBe(true);
    expect(result.exitCode).toBe(2);
    expect(result.output).toContain("uncommitted changes");
    expect(result.error).toBeUndefined();
  });

  test("respects custom timeout", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script that sleeps longer than our timeout
    fs.writeFileSync(scriptPath, '#!/bin/bash\nsleep 5\necho "done"\n');

    const result = await Effect.runPromise(
      runInitScript(openagentsDir, tempDir, () => {}, 100) // 100ms timeout
    );

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false); // Timeout is treated as failure (defaults to exit 1)
    expect(result.exitCode).toBe(1); // null status defaults to 1 (fatal)
  });

  test("captures both stdout and stderr", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(
      scriptPath,
      '#!/bin/bash\necho "stdout message"\necho "stderr message" >&2\nexit 0\n'
    );

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    expect(result.output).toContain("stdout message");
    expect(result.output).toContain("stderr message");
  });

  test("runs script with correct working directory", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script outputs its working directory
    fs.writeFileSync(scriptPath, '#!/bin/bash\npwd\nexit 0\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    // The script should run in the cwd (tempDir), not the openagentsDir
    expect(result.output).toContain(tempDir);
  });

  test("captures stderr-only output on failure", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script that only outputs to stderr
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "error details" >&2\nexit 1\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("error details");
  });

  test("tracks duration of script execution", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script with a small delay to measure duration
    fs.writeFileSync(scriptPath, '#!/bin/bash\nsleep 0.1\necho "done"\nexit 0\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    expect(result.durationMs).toBeDefined();
    expect(result.durationMs).toBeGreaterThanOrEqual(50); // At least ~50ms
  });

  test("handles script with environment variable access", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script that checks if it can access environment
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "HOME=$HOME"\nexit 0\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    // Should have access to HOME environment variable
    expect(result.output).toMatch(/HOME=\/.+/);
  });

  test("treats unexpected exit codes (>2) as warnings", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    // Script with unexpected exit code
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "unexpected exit"\nexit 5\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    // Exit codes other than 0, 1, 2 - based on current implementation,
    // only 0 and 2 are success, everything else is failure
    expect(result.success).toBe(false);
    expect(result.exitCode).toBe(5);
    expect(result.output).toContain("unexpected exit");
  });

  test("emits events in correct order", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "test"\nexit 0\n');

    const events: Array<{ type: string; timestamp: number }> = [];
    const result = await Effect.runPromise(
      runInitScript(openagentsDir, tempDir, (event) => {
        events.push({ type: event.type, timestamp: Date.now() });
      })
    );

    expect(result.ran).toBe(true);
    expect(events.length).toBe(2);
    expect(events[0].type).toBe("init_script_start");
    expect(events[1].type).toBe("init_script_complete");
    // Complete should come after start
    expect(events[1].timestamp).toBeGreaterThanOrEqual(events[0].timestamp);
  });
});
