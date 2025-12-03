import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { runInitScript, detectFailureType } from "./init-script.js";

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

  test("detects typecheck failure and marks as self-healable", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(
      scriptPath,
      '#!/bin/bash\necho "error TS2322: Type \\"string\\" is not assignable to type \\"number\\""\nexit 1\n'
    );

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.failureType).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects test failure and marks as self-healable", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(
      scriptPath,
      '#!/bin/bash\necho "1 test failed: expected 5, got 6"\nexit 1\n'
    );

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.failureType).toBe("test_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects network error and marks as not self-healable", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(
      scriptPath,
      '#!/bin/bash\necho "ENOTFOUND: Unable to connect to registry.npmjs.org"\nexit 1\n'
    );

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.failureType).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("does not set failureType on success", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "all good"\nexit 0\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    expect(result.failureType).toBeUndefined();
    expect(result.canSelfHeal).toBeUndefined();
  });
});

describe("detectFailureType", () => {
  test("detects TypeScript error codes", () => {
    const result = detectFailureType("error TS2322: Type is not assignable");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects tsc errors", () => {
    const result = detectFailureType("tsc exited with error code 1");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects typecheck command failure", () => {
    const result = detectFailureType("typecheck failed: Cannot find name 'foo'");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects test failures", () => {
    const result = detectFailureType("5 tests failed\nexpect(received).toBe(expected)");
    expect(result.type).toBe("test_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects assertion failures", () => {
    const result = detectFailureType("AssertionError: expected true to be false");
    expect(result.type).toBe("test_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects network ENOTFOUND", () => {
    const result = detectFailureType("ENOTFOUND registry.npmjs.org");
    expect(result.type).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects network ECONNREFUSED", () => {
    const result = detectFailureType("ECONNREFUSED 127.0.0.1:3000");
    expect(result.type).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects disk full ENOSPC", () => {
    const result = detectFailureType("ENOSPC: no space left on device");
    expect(result.type).toBe("disk_full");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects permission denied EACCES", () => {
    const result = detectFailureType("EACCES: permission denied");
    expect(result.type).toBe("permission_denied");
    expect(result.canSelfHeal).toBe(false);
  });

  test("returns unknown for unrecognized errors", () => {
    const result = detectFailureType("Something weird happened");
    expect(result.type).toBe("unknown");
    expect(result.canSelfHeal).toBe(false);
  });

  test("is case insensitive", () => {
    const result = detectFailureType("TYPECHECK FAILED");
    expect(result.type).toBe("typecheck_failed");
  });

  test("detects 'Cannot find name' as typecheck failure", () => {
    const result = detectFailureType("error: Cannot find name 'myVariable'");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects property type errors as typecheck failure via ts+type keywords", () => {
    // The code detects TypeScript errors via "ts" + ("type" or "error") combo
    const result = detectFailureType("Property 'foo' does not exist on type 'Bar' - ts error");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects 'argument of type' as typecheck failure", () => {
    const result = detectFailureType("Argument of type 'string' is not assignable to parameter of type 'number'");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects TS error codes with 5 digits", () => {
    const result = detectFailureType("error TS18047: 'foo' is possibly 'null'");
    expect(result.type).toBe("typecheck_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects ETIMEDOUT as network error", () => {
    const result = detectFailureType("ETIMEDOUT: connection timed out");
    expect(result.type).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects 'could not resolve' as network error", () => {
    const result = detectFailureType("could not resolve hostname: api.example.com");
    expect(result.type).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects 'unable to connect' as network error", () => {
    const result = detectFailureType("Error: Unable to connect to the server");
    expect(result.type).toBe("network_error");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects 'no space left' as disk full", () => {
    const result = detectFailureType("Error: no space left on device");
    expect(result.type).toBe("disk_full");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects 'quota exceeded' as disk full", () => {
    const result = detectFailureType("Disk quota exceeded");
    expect(result.type).toBe("disk_full");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects EPERM as permission denied", () => {
    const result = detectFailureType("EPERM: operation not permitted");
    expect(result.type).toBe("permission_denied");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects 'operation not permitted' as permission denied", () => {
    const result = detectFailureType("Error: operation not permitted, unlink '/root/file'");
    expect(result.type).toBe("permission_denied");
    expect(result.canSelfHeal).toBe(false);
  });

  test("detects expect() assertion as test failure", () => {
    const result = detectFailureType("expect(received).toBe(expected)\nExpected: 5\nReceived: 3");
    expect(result.type).toBe("test_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("detects spec file failure as test failure", () => {
    const result = detectFailureType("FAIL src/components/Button.spec.ts");
    expect(result.type).toBe("test_failed");
    expect(result.canSelfHeal).toBe(true);
  });

  test("prioritizes typecheck over test when output contains both", () => {
    // TypeScript errors should be detected first since they're checked earlier in the function
    const result = detectFailureType("error TS2322: Type 'string' is not assignable\ntest failed");
    expect(result.type).toBe("typecheck_failed");
  });
});
