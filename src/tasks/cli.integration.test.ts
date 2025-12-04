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

  test("validate command detects orphan dependencies", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-validate-orphans-"));

    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);
    const tasksPath = path.join(tmp, ".openagents", "tasks.jsonl");

    const now = "2024-01-01T00:00:00.000Z";
    const tasks = [
      {
        id: "oa-1",
        title: "Task with missing dep",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [{ id: "oa-missing", type: "blocks" }],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-2",
        title: "Independent task",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
    ];

    fs.writeFileSync(tasksPath, tasks.map((t) => JSON.stringify(t)).join("\n") + "\n", "utf8");

    const validate = runCli(["validate", "--dir", tmp, "--json"], tmp);
    expect(validate.code).toBe(1);
    const payload = JSON.parse(validate.stdout);
    expect(payload.ok).toBe(false);
    expect(payload.error).toBe("orphan_dependencies");
    expect(payload.orphanDeps).toEqual([
      { taskId: "oa-1", missingId: "oa-missing", type: "blocks" },
    ]);
  });

  test("doctor command reports multiple issues", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-doctor-"));
    const dir = path.join(tmp, ".openagents");
    fs.mkdirSync(dir, { recursive: true });
    const tasksPath = path.join(dir, "tasks.jsonl");

    const staleDate = "2023-01-01T00:00:00.000Z";
    const now = "2024-01-01T00:00:00.000Z";
    const tasks = [
      {
        id: "oa-1",
        title: "Cycle A",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [{ id: "oa-2", type: "blocks" }],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-2",
        title: "Cycle B",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [{ id: "oa-1", type: "blocks" }],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-dup",
        title: "Duplicate entry 1",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-dup",
        title: "Duplicate entry 2",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-orphan",
        title: "Has missing dep",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [{ id: "oa-missing", type: "blocks" }],
        commits: [],
        createdAt: now,
        updatedAt: now,
        closedAt: null,
      },
      {
        id: "oa-stale",
        title: "Old in_progress task",
        description: "",
        status: "in_progress",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: staleDate,
        updatedAt: staleDate,
        closedAt: null,
      },
    ];

    fs.writeFileSync(tasksPath, tasks.map((t) => JSON.stringify(t)).join("\n") + "\n", "utf8");

    const doctor = runCli(["doctor", "--dir", tmp, "--json", "--days", "14"], tmp);
    expect(doctor.code).toBe(1);
    const payload = JSON.parse(doctor.stdout);
    const issueTypes = payload.issues.map((issue: { type: string }) => issue.type);
    expect(payload.ok).toBe(false);
    expect(issueTypes).toContain("orphan_dependencies");
    expect(issueTypes).toContain("duplicate_ids");
    expect(issueTypes).toContain("dependency_cycles");
    expect(issueTypes).toContain("stale_tasks");
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

  test("config list/get/set commands", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-config-"));

    const init = runCli(["init", "--dir", tmp, "--json"], tmp);
    expect(init.code).toBe(0);

    const list = runCli(["config:list", "--dir", tmp, "--json"], tmp);
    expect(list.code).toBe(0);
    const listPayload = JSON.parse(list.stdout);
    expect(listPayload.projectId).toBeTruthy();

    const setModel = runCli(
      ["config:set", "--dir", tmp, "--key", "defaultModel", "--value", "my-model", "--json"],
      tmp,
    );
    expect(setModel.code).toBe(0);

    const getModel = runCli(
      ["config:get", "--dir", tmp, "--key", "defaultModel", "--json"],
      tmp,
    );
    const modelPayload = JSON.parse(getModel.stdout);
    expect(modelPayload.value).toBe("my-model");

    const setNested = runCli(
      ["config:set", "--dir", tmp, "--key", "claudeCode.enabled", "--value", "false", "--json"],
      tmp,
    );
    expect(setNested.code).toBe(0);

    const getNested = runCli(
      ["config:get", "--dir", tmp, "--key", "claudeCode.enabled", "--json"],
      tmp,
    );
    const nestedPayload = JSON.parse(getNested.stdout);
    expect(nestedPayload.value).toBe(false);
  });

  test("cleanup command deletes old closed tasks with optional cascade", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tasks-cli-cleanup-"));
    const dir = path.join(tmp, ".openagents");
    fs.mkdirSync(dir, { recursive: true });
    const tasksPath = path.join(dir, "tasks.jsonl");

    const oldDate = "2024-01-01T00:00:00.000Z";
    const recentDate = "2025-11-01T00:00:00.000Z";

    const tasks = [
      {
        id: "oa-old-1",
        title: "Old closed referenced",
        description: "",
        status: "closed",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: oldDate,
        updatedAt: oldDate,
        closedAt: oldDate,
      },
      {
        id: "oa-old-2",
        title: "Old closed orphan",
        description: "",
        status: "closed",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: oldDate,
        updatedAt: oldDate,
        closedAt: oldDate,
      },
      {
        id: "oa-open-1",
        title: "Open referencing old",
        description: "",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [{ id: "oa-old-1", type: "blocks" }],
        commits: [],
        createdAt: recentDate,
        updatedAt: recentDate,
        closedAt: null,
      },
      {
        id: "oa-closed-recent",
        title: "Recent closed keep",
        description: "",
        status: "closed",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        createdAt: recentDate,
        updatedAt: recentDate,
        closedAt: recentDate,
      },
    ];

    fs.writeFileSync(tasksPath, tasks.map((t) => JSON.stringify(t)).join("\n") + "\n", "utf8");

    const cleanupNoCascade = runCli(
      ["cleanup", "--dir", tmp, "--older-than", "300", "--json"],
      tmp,
    );
    expect(cleanupNoCascade.code).toBe(0);
    const afterNoCascade = fs
      .readFileSync(tasksPath, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    const idsAfterNoCascade = afterNoCascade.map((t: any) => t.id);
    expect(idsAfterNoCascade).toContain("oa-old-1");
    expect(idsAfterNoCascade).not.toContain("oa-old-2");

    const cleanupCascade = runCli(
      ["cleanup", "--dir", tmp, "--older-than", "300", "--cascade", "--json"],
      tmp,
    );
    expect(cleanupCascade.code).toBe(0);

    const afterCascade = fs
      .readFileSync(tasksPath, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    const idsAfterCascade = afterCascade.map((t: any) => t.id);
    expect(idsAfterCascade).not.toContain("oa-old-1");
    const openTask = afterCascade.find((t: any) => t.id === "oa-open-1");
    expect(openTask?.deps ?? []).toHaveLength(0);
  });
});
