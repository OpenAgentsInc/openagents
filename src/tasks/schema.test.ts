import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import {
  TaskUpdate,
  Status,
  IssueType,
  DependencyType,
  Dependency,
  isTaskReady,
  decodeTask,
  decodeTaskCreate,
  decodeProjectConfig,
} from "./schema.js";

describe("Status", () => {
  test("accepts valid statuses", () => {
    expect(S.decodeUnknownSync(Status)("open")).toBe("open");
    expect(S.decodeUnknownSync(Status)("in_progress")).toBe("in_progress");
    expect(S.decodeUnknownSync(Status)("blocked")).toBe("blocked");
    expect(S.decodeUnknownSync(Status)("closed")).toBe("closed");
  });

  test("rejects invalid status", () => {
    expect(() => S.decodeUnknownSync(Status)("invalid")).toThrow();
  });
});

describe("IssueType", () => {
  test("accepts valid types", () => {
    expect(S.decodeUnknownSync(IssueType)("bug")).toBe("bug");
    expect(S.decodeUnknownSync(IssueType)("feature")).toBe("feature");
    expect(S.decodeUnknownSync(IssueType)("task")).toBe("task");
    expect(S.decodeUnknownSync(IssueType)("epic")).toBe("epic");
    expect(S.decodeUnknownSync(IssueType)("chore")).toBe("chore");
  });

  test("rejects invalid type", () => {
    expect(() => S.decodeUnknownSync(IssueType)("invalid")).toThrow();
  });
});

describe("DependencyType", () => {
  test("accepts valid dependency types", () => {
    expect(S.decodeUnknownSync(DependencyType)("blocks")).toBe("blocks");
    expect(S.decodeUnknownSync(DependencyType)("related")).toBe("related");
    expect(S.decodeUnknownSync(DependencyType)("parent-child")).toBe("parent-child");
    expect(S.decodeUnknownSync(DependencyType)("discovered-from")).toBe("discovered-from");
  });
});

describe("Dependency", () => {
  test("decodes valid dependency", () => {
    const dep = S.decodeUnknownSync(Dependency)({
      id: "oa-abc123",
      type: "blocks",
    });
    expect(dep.id).toBe("oa-abc123");
    expect(dep.type).toBe("blocks");
  });
});

describe("Task", () => {
  const validTask = {
    id: "oa-abc123",
    title: "Test task",
    description: "A test description",
    status: "open",
    priority: 1,
    type: "task",
    createdAt: "2025-12-02T10:00:00Z",
    updatedAt: "2025-12-02T10:00:00Z",
  };

  test("decodes valid task", () => {
    const task = decodeTask(validTask);
    expect(task.id).toBe("oa-abc123");
    expect(task.title).toBe("Test task");
    expect(task.status).toBe("open");
    expect(task.priority).toBe(1);
    expect(task.type).toBe("task");
  });

  test("decodes task with optional fields", () => {
    const task = decodeTask({
      ...validTask,
      assignee: "mechacoder",
      labels: ["testing", "p1"],
      deps: [{ id: "oa-parent", type: "parent-child" }],
      commits: ["abc123"],
      closedAt: null,
    });
    expect(task.assignee).toBe("mechacoder");
    expect(task.labels).toEqual(["testing", "p1"]);
    expect(task.deps).toHaveLength(1);
    expect(task.commits).toEqual(["abc123"]);
  });

  test("applies defaults for missing optional fields", () => {
    const minimalTask = {
      id: "oa-abc123",
      title: "Test task",
      status: "open",
      priority: 1,
      type: "task",
      createdAt: "2025-12-02T10:00:00Z",
      updatedAt: "2025-12-02T10:00:00Z",
    };
    const task = decodeTask(minimalTask);
    expect(task.description).toBe("");
    expect(task.labels).toEqual([]);
    expect(task.deps).toEqual([]);
    expect(task.commits).toEqual([]);
  });

  test("rejects invalid priority", () => {
    expect(() => decodeTask({ ...validTask, priority: 5 })).toThrow();
    expect(() => decodeTask({ ...validTask, priority: -1 })).toThrow();
  });

  test("rejects empty title", () => {
    expect(() => decodeTask({ ...validTask, title: "" })).toThrow();
  });

  test("rejects title over 500 chars", () => {
    expect(() => decodeTask({ ...validTask, title: "a".repeat(501) })).toThrow();
  });
});

describe("TaskCreate", () => {
  test("decodes with minimal fields", () => {
    const task = decodeTaskCreate({ title: "New task" });
    expect(task.title).toBe("New task");
    expect(task.status).toBe("open");
    expect(task.priority).toBe(2);
    expect(task.type).toBe("task");
  });

  test("decodes with all fields", () => {
    const task = decodeTaskCreate({
      title: "New task",
      description: "Description",
      status: "in_progress",
      priority: 0,
      type: "bug",
      assignee: "user",
      labels: ["urgent"],
    });
    expect(task.status).toBe("in_progress");
    expect(task.priority).toBe(0);
    expect(task.type).toBe("bug");
  });
});

describe("TaskUpdate", () => {
  test("allows partial updates", () => {
    const update = S.decodeUnknownSync(TaskUpdate)({ status: "closed" });
    expect(update.status).toBe("closed");
    expect(update.title).toBeUndefined();
  });

  test("allows null assignee (to clear)", () => {
    const update = S.decodeUnknownSync(TaskUpdate)({ assignee: null });
    expect(update.assignee).toBeNull();
  });
});

describe("ProjectConfig", () => {
  test("decodes minimal config", () => {
    const config = decodeProjectConfig({ projectId: "my-project" });
    expect(config.projectId).toBe("my-project");
    expect(config.version).toBe(1);
    expect(config.defaultBranch).toBe("main");
    expect(config.defaultModel).toBe("x-ai/grok-4.1-fast:free");
    expect(config.typecheckCommands).toEqual([]);
    expect(config.allowPush).toBe(true);
    expect(config.allowForcePush).toBe(false);
    expect(config.idPrefix).toBe("oa");
    expect(config.claudeCode?.enabled).toBe(true);
    expect(config.claudeCode?.preferForComplexTasks).toBe(true);
    expect(config.claudeCode?.fallbackToMinimal).toBe(true);
    expect(config.claudeCode?.maxTurnsPerSubtask).toBe(300);
    expect(config.claudeCode?.permissionMode).toBe("bypassPermissions");
  });

  test("decodes full config", () => {
    const config = decodeProjectConfig({
      projectId: "openagents",
      version: 1,
      defaultBranch: "develop",
      typecheckCommands: ["bun run typecheck"],
      testCommands: ["bun test"],
      e2eCommands: ["bun test:e2e"],
      maxTasksPerRun: 5,
      idPrefix: "openagents",
      claudeCode: {
        enabled: false,
        preferForComplexTasks: false,
        maxTurnsPerSubtask: 20,
        permissionMode: "dontAsk",
        fallbackToMinimal: false,
      },
      cloud: {
        useGateway: true,
        sendTelemetry: false,
      },
    });
    expect(config.defaultBranch).toBe("develop");
    expect(config.typecheckCommands).toEqual(["bun run typecheck"]);
    expect(config.testCommands).toEqual(["bun test"]);
    expect(config.maxTasksPerRun).toBe(5);
    expect(config.idPrefix).toBe("openagents");
    expect(config.claudeCode?.enabled).toBe(false);
    expect(config.claudeCode?.preferForComplexTasks).toBe(false);
    expect(config.claudeCode?.maxTurnsPerSubtask).toBe(20);
    expect(config.claudeCode?.permissionMode).toBe("dontAsk");
    expect(config.claudeCode?.fallbackToMinimal).toBe(false);
    expect(config.cloud?.useGateway).toBe(true);
  });
});

describe("isTaskReady", () => {
  const makeTask = (id: string, status: string, deps: Array<{ id: string; type: string }> = []) =>
    ({
      id,
      title: `Task ${id}`,
      status,
      priority: 1,
      type: "task",
      deps,
      createdAt: "2025-12-02T10:00:00Z",
      updatedAt: "2025-12-02T10:00:00Z",
    }) as any;

  test("open task with no deps is ready", () => {
    const task = makeTask("oa-1", "open");
    expect(isTaskReady(task, [])).toBe(true);
  });

  test("in_progress task is ready", () => {
    const task = makeTask("oa-1", "in_progress");
    expect(isTaskReady(task, [])).toBe(true);
  });

  test("blocked task is not ready", () => {
    const task = makeTask("oa-1", "blocked");
    expect(isTaskReady(task, [])).toBe(false);
  });

  test("closed task is not ready", () => {
    const task = makeTask("oa-1", "closed");
    expect(isTaskReady(task, [])).toBe(false);
  });

  test("task with closed blocking dep is ready", () => {
    const parent = makeTask("oa-parent", "closed");
    const task = makeTask("oa-1", "open", [{ id: "oa-parent", type: "blocks" }]);
    expect(isTaskReady(task, [parent, task])).toBe(true);
  });

  test("task with open blocking dep is not ready", () => {
    const parent = makeTask("oa-parent", "open");
    const task = makeTask("oa-1", "open", [{ id: "oa-parent", type: "blocks" }]);
    expect(isTaskReady(task, [parent, task])).toBe(false);
  });

  test("task with related dep ignores dep status", () => {
    const related = makeTask("oa-related", "open");
    const task = makeTask("oa-1", "open", [{ id: "oa-related", type: "related" }]);
    expect(isTaskReady(task, [related, task])).toBe(true);
  });

  test("task with discovered-from dep ignores dep status", () => {
    const source = makeTask("oa-source", "open");
    const task = makeTask("oa-1", "open", [{ id: "oa-source", type: "discovered-from" }]);
    expect(isTaskReady(task, [source, task])).toBe(true);
  });
});
