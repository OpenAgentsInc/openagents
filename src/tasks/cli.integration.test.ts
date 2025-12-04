import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
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
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-"));

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

  test("close command with explicit commit SHA", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-close-"));

    // Initialize
    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);

    // Create a task
    const create = runCli(
      [
        "create",
        "--dir",
        tmp,
        "--title",
        "task to close",
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

    // Close the task with explicit commit SHA
    const close = runCli(
      [
        "close",
        "--dir",
        tmp,
        "--id",
        created.id,
        "--reason",
        "Done via close command",
        "--commit",
        "abc123def456",
        "--json",
      ],
      tmp,
    );
    expect(close.code).toBe(0);
    const closed = JSON.parse(close.stdout);
    expect(closed.status).toBe("closed");
    expect(closed.closeReason).toBe("Done via close command");
    expect(closed.commits).toContain("abc123def456");

    // Verify no ready tasks remain
    const ready = runCli(["ready", "--dir", tmp, "--json"], tmp);
    const readyList = JSON.parse(ready.stdout);
    expect(readyList).toHaveLength(0);
  });

  test("close command without commit SHA (defaults to empty when not in git repo)", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-close-nogit-"));

    // Initialize
    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);

    // Create a task
    const create = runCli(
      [
        "create",
        "--dir",
        tmp,
        "--title",
        "task without commit",
        "--type",
        "task",
        "--json",
      ],
      tmp,
    );
    expect(create.code).toBe(0);
    const created = JSON.parse(create.stdout);

    // Close the task without commit SHA (not a git repo, so auto-detect fails)
    const close = runCli(
      [
        "close",
        "--dir",
        tmp,
        "--id",
        created.id,
        "--json",
      ],
      tmp,
    );
    expect(close.code).toBe(0);
    const closed = JSON.parse(close.stdout);
    expect(closed.status).toBe("closed");
    expect(closed.closeReason).toBe("Completed"); // default reason
  });

  test("validate command fails on conflict markers", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-validate-"));

    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);
    const tasksPath = path.join(tmp, ".openagents", "tasks.jsonl");
    const conflict = `<<<<<<< ours
{"id":"oa-1","title":"A","description":"","status":"open","priority":2,"type":"task","labels":[],"deps":[],"commits":[],"createdAt":"2024-01-01T00:00:00.000Z","updatedAt":"2024-01-01T00:00:00.000Z","closedAt":null}
=======
{"id":"oa-1","title":"B","description":"","status":"open","priority":2,"type":"task","labels":[],"deps":[],"commits":[],"createdAt":"2024-01-01T00:00:00.000Z","updatedAt":"2024-01-01T00:00:00.000Z","closedAt":null}
>>>>>>> theirs
`;
    fs.writeFileSync(tasksPath, conflict, "utf8");

    const validate = runCli(["validate", "--dir", tmp, "--check-conflicts", "--json"], tmp);
    expect(validate.code).toBe(1);
    const payload = JSON.parse(validate.stdout);
    expect(payload.ok).toBe(false);
    expect(String(payload.error)).toContain("conflict");
  });

  test("close command requires --id", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-close-noid-"));

    // Initialize
    runCli(["init", "--dir", tmp, "--json"], tmp);

    // Close without --id should fail
    const close = runCli(
      [
        "close",
        "--dir",
        tmp,
        "--json",
      ],
      tmp,
    );
    expect(close.code).toBe(0); // exits 0 but outputs error
    const output = JSON.parse(close.stdout);
    expect(output.error).toContain("Missing required --id");
  });
});
