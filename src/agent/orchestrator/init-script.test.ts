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
    expect(result.output).toBeUndefined();
  });

  test("runs init.sh and captures output", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "hello init"\n');

    const events: string[] = [];
    const result = await Effect.runPromise(
      runInitScript(openagentsDir, tempDir, (event) => {
        events.push(event.type);
      })
    );

    expect(result.ran).toBe(true);
    expect(result.success).toBe(true);
    expect(result.output).toContain("hello init");
    expect(events).toContain("init_script_start");
    expect(events).toContain("init_script_complete");
  });

  test("reports failure when init.sh exits non-zero", async () => {
    const scriptPath = path.join(openagentsDir, "init.sh");
    fs.writeFileSync(scriptPath, '#!/bin/bash\necho "broken" >&2\nexit 1\n');

    const result = await Effect.runPromise(runInitScript(openagentsDir, tempDir));

    expect(result.ran).toBe(true);
    expect(result.success).toBe(false);
    expect(result.output).toContain("broken");
    expect(result.error).toBeTruthy();
  });
});
