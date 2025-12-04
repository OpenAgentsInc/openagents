import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const createMockClaude = (dir: string, body: string, exitCode = 0): string => {
  const script = join(dir, "claude");
  writeFileSync(
    script,
    `#!/bin/bash\ncat <<'JSON'\n${body}\nJSON\nexit ${exitCode}\n`,
  );
  chmodSync(script, 0o755);
  return script;
};

const runTbench = (envPath: string, outputDir: string, extraArgs: string[] = []) => {
  return spawnSync(
    "bun",
    ["src/cli/tbench.ts", "--instruction", "print hello", "--output-dir", outputDir, "--timeout", "30", ...extraArgs],
    {
      env: { ...process.env, PATH: `${envPath}:${process.env.PATH}` },
      encoding: "utf-8",
    },
  );
};

describe("tbench CLI / Harbor adapter integration", () => {
  test("writes ATIF trajectory and metrics from mock Claude success", () => {
    const tmp = mkdtempSync(join(tmpdir(), "tbench-harbor-success-"));
    const binDir = join(tmp, "bin");
    const outDir = join(tmp, "out");
    mkdirSync(binDir, { recursive: true });
    mkdirSync(outDir, { recursive: true });
    createMockClaude(
      binDir,
      JSON.stringify({
        type: "result",
        subtype: "success",
        session_id: "sess-123",
        num_turns: 2,
        usage: {
          input_tokens: 11,
          output_tokens: 7,
          cache_read_input_tokens: 3,
          cache_creation_input_tokens: 1,
        },
        total_cost_usd: 0.0123,
      }),
    );

    const result = runTbench(binDir, outDir);
    expect(result.status).toBe(0);

    const metrics = JSON.parse(readFileSync(join(outDir, "metrics.json"), "utf-8"));
    expect(metrics.success).toBe(true);
    expect(metrics.tokens).toEqual({
      input: 11,
      output: 7,
      cacheRead: 3,
      cacheCreation: 1,
      total: 18,
    });
    expect(metrics.cost).toBeCloseTo(0.0123);
    expect(metrics.turns).toBe(2);

    const trajectory = JSON.parse(readFileSync(join(outDir, "trajectory.json"), "utf-8"));
    expect(Array.isArray(trajectory.steps)).toBe(true);
    expect(trajectory.steps[0]?.message).toContain("print hello");
    expect(trajectory.final_metrics.total_prompt_tokens).toBe(metrics.tokens.input);
    expect(trajectory.final_metrics.total_completion_tokens).toBe(metrics.tokens.output);
    expect(trajectory.extra.success).toBe(true);
  });

  test("records failure and error message when Claude exits non-zero", () => {
    const tmp = mkdtempSync(join(tmpdir(), "tbench-harbor-fail-"));
    const binDir = join(tmp, "bin");
    const outDir = join(tmp, "out");
    mkdirSync(binDir, { recursive: true });
    mkdirSync(outDir, { recursive: true });
    createMockClaude(
      binDir,
      JSON.stringify({
        type: "result",
        subtype: "error",
        session_id: "sess-fail",
        num_turns: 1,
        usage: { input_tokens: 0, output_tokens: 0 },
        total_cost_usd: 0,
      }),
      1,
    );

    const result = runTbench(binDir, outDir);
    expect(result.status).toBe(1);

    const metrics = JSON.parse(readFileSync(join(outDir, "metrics.json"), "utf-8"));
    expect(metrics.success).toBe(false);
    expect(metrics.error).toContain("error");
    expect(metrics.tokens.total).toBe(0);
    expect(metrics.turns).toBe(1);
  });
});
