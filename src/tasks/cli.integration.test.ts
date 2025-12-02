import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";

const runCli = (args: string[], targetDir: string) => {
  const result = Bun.spawnSync(["bun", "src/tasks/cli.ts", ...args], {
    cwd: process.cwd(),
    env: { ...process.env, PATH: process.env.PATH },
  });
  return {
    code: result.exitCode,
    stdout: new TextDecoder().decode(result.stdout),
    stderr: new TextDecoder().decode(result.stderr),
  };
};

describe("tasks CLI integration", () => {
  test("init/create/next/update lifecycle", () => {
    const tmp = fs.mkdtempSync(path.join(process.cwd(), "tasks-cli-"));

    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);
    expect(fs.existsSync(path.join(tmp, ".openagents", "project.json"))).toBe(true);

    const create = runCli(
      [
        "create",
        "--dir",
        tmp,
        "--title",
        "sample task",
        "--type",
        "task",
        "--priority",
        "1",
        "--json",
      ],
      tmp,
    );
    expect(create.code).toBe(0);
    const created = JSON.parse(create.stdout);
    expect(created.id).toBeTruthy();

    const next = runCli(["next", "--dir", tmp, "--json"], tmp);
    expect(next.code).toBe(0);
    const picked = JSON.parse(next.stdout);
    expect(picked.status).toBe("in_progress");

    const updatePayload = JSON.stringify({
      id: picked.id,
      status: "closed",
      closeReason: "done",
      commits: ["abc123"],
    });
    const update = Bun.spawnSync(["bun", "src/tasks/cli.ts", "update", "--dir", tmp, "--json-input"], {
      cwd: process.cwd(),
      stdin: new TextEncoder().encode(updatePayload),
    });
    expect(update.exitCode).toBe(0);

    const ready = runCli(["ready", "--dir", tmp, "--json"], tmp);
    const readyList = JSON.parse(ready.stdout);
    expect(readyList).toHaveLength(0);
  });
});
