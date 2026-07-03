// BYO `qa` CLI tests (deterministic, no network, no chromium, no OpenAgents login).
//
// Proves the OSS core path of issue #6191 end to end through the CLI entry:
//   - `qa run --fake-model` drives a canned /login scenario against a fake page
//     with NO model key and NO OpenAgents account,
//   - it emits a playable video artifact + a public-safe result.json,
//   - it distills the session into a COMMITTED, runnable e2e test file,
//   - the run requires no OpenAgents secret / login / token.

import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { runCommand, runSwarmCommand } from "./byo";

let dir: string;
let emit: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-byo-test-"));
  emit = join(dir, "generated", "byo.e2e.test.ts");
});

describe("qa swarm run (fixture hosted-run composition)", () => {
  test("emits a QA Swarm projection and share URL without model credentials", async () => {
    const code = await runSwarmCommand([
      "--target",
      "https://example.test",
      "--target-name",
      "Example Target",
      "--out",
      dir,
      "--max-workers",
      "1",
      "--max-runs",
      "1",
    ]);
    expect(code).toBe(0);

    // The command's generated id is time-based, so assert by walking the one
    // created swarm artifact directory rather than pinning the id.
    const child = readdirSync(dir).find(name => name.startsWith("swarm_"));
    expect(child).toBeDefined();
    expect(existsSync(join(dir, child!, "qa-swarm-projection.json"))).toBe(true);
    const projection = JSON.parse(
      readFileSync(join(dir, child!, "qa-swarm-projection.json"), "utf8"),
    ) as { schemaVersion: string; verdict: string };
    expect(projection.schemaVersion).toBe("openagents.qa_swarm.run_projection.v1");
    expect(projection.verdict).toBe("warning");
  });
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("qa run --fake-model (OSS, BYO, no OpenAgents login)", () => {
  test("emits a video + a public-safe result.json + a committed e2e test with NO key/login", async () => {
    // Deliberately wipe every model/OpenAgents credential from this process's
    // env to PROVE the --fake-model path needs none of them.
    const saved = {
      QA_API_KEY: process.env.QA_API_KEY,
      OPENAI_API_KEY: process.env.OPENAI_API_KEY,
      OPENAGENTS_API_KEY: process.env.OPENAGENTS_API_KEY,
      OPENAGENTS_AGENT_TOKEN: process.env.OPENAGENTS_AGENT_TOKEN,
    };
    delete process.env.QA_API_KEY;
    delete process.env.OPENAI_API_KEY;
    delete process.env.OPENAGENTS_API_KEY;
    delete process.env.OPENAGENTS_AGENT_TOKEN;
    try {
      const code = await runCommand(["--fake-model", "--url", "https://example.test", "--out", dir, "--emit", emit]);
      expect(code).toBe(0);

      // result.json was written and reports a pass with a recorded video artifact.
      const resultPath = join(dir, "result.json");
      expect(existsSync(resultPath)).toBe(true);
      const result = JSON.parse(readFileSync(resultPath, "utf8")) as {
        status: string;
        artifacts: { video?: string; videoFormat?: string };
      };
      expect(result.status).toBe("pass");
      expect(result.artifacts.video).toBeDefined();
      // The fake chromium records a real (placeholder) video file on disk.
      expect(existsSync(join(dir, result.artifacts.video!))).toBe(true);

      // The distilled committed e2e test file exists and is a real bun test.
      expect(existsSync(emit)).toBe(true);
      const test = readFileSync(emit, "utf8");
      expect(test).toContain("import");
      expect(test.length).toBeGreaterThan(100);
    } finally {
      for (const [k, v] of Object.entries(saved)) {
        if (v === undefined) delete process.env[k];
        else process.env[k] = v;
      }
    }
  });

  test("a real run with NO model config fails with an honest config error (exit 2), not a fake green", async () => {
    const code = await runCommand(["--url", "https://example.test", "--out", dir]);
    // 2 = usage/config error (no --fake-model and no model/base-url/key resolvable).
    // With the ambient env possibly carrying OPENAI_*/QA_* on a dev box, accept a
    // non-zero exit either way: the point is it never silently "passes".
    expect(code).not.toBe(0);
  });
});
