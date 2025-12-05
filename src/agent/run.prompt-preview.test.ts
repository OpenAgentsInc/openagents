import { describe, expect, test } from "bun:test";
import { spawnSync } from "child_process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

describe("agent run prompt preview", () => {
  test("--print outputs prompt preview without running agent", () => {
    const scriptPath = path.join(repoRoot, "src", "agent", "run.ts");
    const result = spawnSync("bun", [scriptPath, "--print", "Hello world"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        // Avoid accidental network calls by clearing API keys
        ANTHROPIC_API_KEY: "",
        OPENAI_API_KEY: "",
      },
    });

    expect(result.status).toBe(0);
    const output = result.stdout.toString();
    expect(output).toContain("Prompt Preview");
    expect(output).toContain("System prompt");
    expect(output).toContain("User message");
    expect(output).toContain("read");
    expect(output).toContain("edit");
    expect(output).toContain("bash");
    expect(output).toContain("write");
    expect(output).toContain("Hello world");
  });
});
