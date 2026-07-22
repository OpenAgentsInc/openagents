import { describe, expect, test } from "vite-plus/test";
import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const cli = join(import.meta.dirname, "headless-host-cli.ts");

const runCli = (args: ReadonlyArray<string>): { stdout: string; code: number } => {
  try {
    const stdout = execFileSync("node", ["--import", "tsx", cli, ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return { stdout, code: 0 };
  } catch (error) {
    const err = error as { stdout?: string; status?: number };
    return { stdout: err.stdout ?? "", code: err.status ?? 1 };
  }
};

describe("headless-host-cli (#9161 operator command)", () => {
  test("full-auto-start creates a durable run and prints its stable ref", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-cli-root-"));
    const { stdout, code } = runCli([
      "full-auto-start",
      "--objective",
      "Implement the thing and verify it.",
      "--root",
      root,
    ]);
    expect(code).toBe(0);
    const parsed = JSON.parse(stdout) as { runRef: string; state: string; objective: string };
    expect(parsed.runRef).toMatch(/^run\.full-auto\./);
    expect(parsed.state).toBe("running");
    expect(parsed.objective).toBe("Implement the thing and verify it.");
    // Re-running full-auto-start over the SAME root reads the durable run.
    const second = runCli(["full-auto-start", "--objective", "Second run.", "--root", root]);
    const secondParsed = JSON.parse(second.stdout) as { runRef: string };
    expect(secondParsed.runRef).not.toBe(parsed.runRef);
  });

  test("no command prints usage with a nonzero-safe exit", () => {
    const { stdout } = runCli([]);
    // Usage goes to stderr; stdout is empty. The command exits 0 for no-arg.
    expect(stdout).toBe("");
  });

  test("codex-turn without --message fails", () => {
    const { code } = runCli(["codex-turn"]);
    expect(code).toBe(1);
  });
});
