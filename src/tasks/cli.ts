#!/usr/bin/env bun
/**
 * OpenAgents Task CLI
 * 
 * A CLI interface for the OpenAgents task system, enabling external agents
 * (Claude Code, Codex, shell scripts) to interact with .openagents/tasks.jsonl.
 * 
 * Usage:
 *   bun src/tasks/cli.ts <command> [options]
 * 
 * Commands:
 *   init       Initialize .openagents for a repo
 *   list       List tasks with optional filters
 *   ready      List ready tasks (no open blockers)
 *   next       Pick the next ready task and mark it in_progress
 *   create     Create a new task
 *   update     Update an existing task
 *   validate   Validate tasks.jsonl (schema + conflict markers)
 *   config     Manage project config (list/get/set)
 *   doctor     Diagnose common issues in tasks.jsonl
 *   repair-deps Fix orphaned dependencies by removing references to missing tasks
 *   duplicates Find duplicate tasks by title+description hash, grouped by status
 *   compact    Compact old closed tasks to save space (--analyze, --apply, --stats)
 *   hooks:install Install git hooks for task auto-sync
 *   hooks:uninstall Uninstall git hooks
 *   comment:add Add a comment to a task
 *   comment:list List comments for a task
 *   rename-prefix Rename task IDs to a new prefix with optional dry-run
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import { Effect, Layer } from "effect";
import { makeDatabaseLive } from "../storage/database.js";
import * as nodePath from "node:path";
import { createHash } from "node:crypto";
import {
  initOpenAgentsProject,
  archiveTasks,
  compactTasks,
  writeTasks,
  searchAllTasks,
  getTaskStats,
  getStaleTasks,
  getTaskWithDeps,
  readTasks,
  hasConflictMarkers,
  mergeTaskFiles,
  TaskMergeError,
  TaskServiceError,
  listComments,
  renameTaskPrefix,
  mergeTasksById,
  loadProjectConfig,
  saveProjectConfig,
  defaultProjectConfig,
  decodeProjectConfig,
  createTaskRepository,
  installHooks,
  uninstallHooks,
  type TaskCreate,
  type TaskUpdate,
  type TaskFilter,
  type Task,
  type DependencyT,
} from "./index.js";

const OPENAGENTS_DIR = ".openagents";
const TASKS_FILE = "tasks.jsonl";
const PROJECT_FILE = "project.json";

interface CliOptions {
  json: boolean;
  jsonInput: boolean;
  rootDir: string;
  status?: string;
  priority?: number;
  priorityMax?: number;
  type?: string;
  label?: string[];
  labels?: string[];
  assignee?: string;
  unassigned?: boolean;
  fromPrefix?: string;
  toPrefix?: string;
  text?: string;
  author?: string;
  commentId?: string;
  limit?: number;
  title?: string;
  description?: string;
  projectId?: string;
  ids?: string[];
  into?: string;
  // close command options
  id?: string;
  reason?: string;
  commit?: string;
  stage?: boolean;
  // archive command options
  days?: number;
  dryRun?: boolean;
  // search command options
  query?: string;
  includeArchived?: boolean;
  // validate options
  checkConflicts?: boolean;
  // merge options
  base?: string;
  current?: string;
  incoming?: string;
  output?: string;
  // config options
  key?: string;
  value?: string;
  // cleanup options
  olderThan?: string;
  cascade?: boolean;
  deleteCascade?: boolean;
}

const parseArgs = (args: string[]): { command: string; options: CliOptions } => {
  const command = args[0] || "help";
  const options: CliOptions = {
    json: false,
    jsonInput: false,
    rootDir: process.cwd(),
  };

  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    const nextArg = args[i + 1];

    switch (arg) {
      case "--json":
        options.json = true;
        break;
      case "--json-input":
        options.jsonInput = true;
        break;
      case "--dir":
      case "--root":
        if (nextArg) {
          options.rootDir = nextArg.startsWith("~")
            ? nextArg.replace("~", process.env.HOME || "")
            : nextArg;
          i++;
        }
        break;
      case "--status":
        if (nextArg) {
          options.status = nextArg;
          i++;
        }
        break;
      case "--priority":
        if (nextArg) {
          options.priority = parseInt(nextArg, 10);
          i++;
        }
        break;
      case "--priority-max":
        if (nextArg) {
          options.priorityMax = parseInt(nextArg, 10);
          i++;
        }
        break;
      case "--type":
        if (nextArg) {
          options.type = nextArg;
          i++;
        }
        break;
      case "--label":
      case "--labels":
        if (nextArg) {
          const labelsArr = nextArg.split(",").map((l) => l.trim());
          options.labels = labelsArr;
          i++;
        }
        break;
      case "--assignee":
        if (nextArg) {
          options.assignee = nextArg;
          i++;
        }
        break;
      case "--unassigned":
        options.unassigned = true;
        break;
      case "--from":
      case "--from-prefix":
        if (nextArg) {
          options.fromPrefix = nextArg;
          i++;
        }
        break;
      case "--to":
      case "--to-prefix":
        if (nextArg) {
          options.toPrefix = nextArg;
          i++;
        }
        break;
      case "--limit":
        if (nextArg) {
          options.limit = parseInt(nextArg, 10);
          i++;
        }
        break;
      case "--title":
        if (nextArg) {
          options.title = nextArg;
          i++;
        }
        break;
      case "--description":
        if (nextArg) {
          options.description = nextArg;
          i++;
        }
        break;
      case "--text":
        if (nextArg) {
          options.text = nextArg;
          i++;
        }
        break;
      case "--author":
        if (nextArg) {
          options.author = nextArg;
          i++;
        }
        break;
      case "--comment-id":
        if (nextArg) {
          options.commentId = nextArg;
          i++;
        }
        break;
      case "--project-id":
        if (nextArg) {
          options.projectId = nextArg;
          i++;
        }
        break;
      case "--ids":
        if (nextArg) {
          options.ids = nextArg.split(",").map((id) => id.trim()).filter(Boolean);
          i++;
        }
        break;
      case "--into":
      case "--target":
        if (nextArg) {
          options.into = nextArg;
          i++;
        }
        break;
      case "--id":
        if (nextArg) {
          options.id = nextArg;
          i++;
        }
        break;
      case "--reason":
        if (nextArg) {
          options.reason = nextArg;
          i++;
        }
        break;
      case "--commit":
        if (nextArg) {
          options.commit = nextArg;
          i++;
        }
        break;
      case "--stage":
        options.stage = true;
        break;
      case "--days":
        if (nextArg) {
          options.days = parseInt(nextArg, 10);
          i++;
        }
        break;
      case "--older-than":
        if (nextArg) {
          options.olderThan = nextArg;
          i++;
        }
        break;
      case "--dry-run":
        options.dryRun = true;
        break;
      case "--query":
        if (nextArg) {
          options.query = nextArg;
          i++;
        }
        break;
      case "--include-archived":
        options.includeArchived = true;
        break;
      case "--check-conflicts":
        options.checkConflicts = true;
        break;
      case "--base":
        if (nextArg) {
          options.base = nextArg;
          i++;
        }
        break;
      case "--current":
        if (nextArg) {
          options.current = nextArg;
          i++;
        }
        break;
      case "--incoming":
        if (nextArg) {
          options.incoming = nextArg;
          i++;
        }
        break;
      case "--output":
        if (nextArg) {
          options.output = nextArg;
          i++;
        }
        break;
      case "--key":
        if (nextArg) {
          options.key = nextArg;
          i++;
        }
        break;
      case "--value":
        if (nextArg) {
          options.value = nextArg;
          i++;
        }
        break;
      case "--cascade":
        options.cascade = true;
        break;
      case "--delete-cascade":
        options.deleteCascade = true;
        break;
    }
  }

  return { command, options };
};

const getTasksPath = (rootDir: string): string =>
  nodePath.join(rootDir, OPENAGENTS_DIR, TASKS_FILE);

const getTaskRepository = (rootDir: string) =>
  createTaskRepository(getTasksPath(rootDir));

const output = (data: unknown, json: boolean): void => {
  if (json) {
    console.log(JSON.stringify(data, null, 2));
  } else {
    console.log(data);
  }
};

const readStdin = async (): Promise<string> => {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf-8");
};

const getLatestCommitSha = async (rootDir: string): Promise<string | null> => {
  try {
    const result = Bun.spawnSync(["git", "rev-parse", "HEAD"], {
      cwd: rootDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    if (result.exitCode === 0) {
      return result.stdout.toString().trim();
    }
    return null;
  } catch {
    return null;
  }
};

const stageTasksFile = async (rootDir: string): Promise<boolean> => {
  try {
    const tasksPath = nodePath.join(OPENAGENTS_DIR, TASKS_FILE);
    const result = Bun.spawnSync(["git", "add", tasksPath], {
      cwd: rootDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    return result.exitCode === 0;
  } catch {
    return false;
  }
};

const cmdInit = (options: CliOptions) =>
  Effect.gen(function* () {
    const initOptions: { rootDir: string; projectId?: string; allowExisting?: boolean } = {
      rootDir: options.rootDir,
      allowExisting: true,
    };
    if (options.projectId) {
      initOptions.projectId = options.projectId;
    }

    const result = yield* initOpenAgentsProject(initOptions).pipe(
      Effect.catchAll((e) =>
        Effect.succeed({
          projectId: options.projectId || nodePath.basename(options.rootDir),
          projectPath: nodePath.join(options.rootDir, OPENAGENTS_DIR, PROJECT_FILE),
          tasksPath: nodePath.join(options.rootDir, OPENAGENTS_DIR, TASKS_FILE),
          error: e.message,
        }),
      ),
    );

    output(result, options.json);
    return result;
  });

const cmdList = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);
    const filter: TaskFilter = {
      status: options.status as TaskFilter["status"],
      priority: options.priority,
      type: options.type as TaskFilter["type"],
      labelsAny: options.labels && options.labels.length > 0 ? options.labels : undefined,
      assignee: options.assignee,
      unassigned: options.unassigned,
      limit: options.limit,
    };

    const tasks = yield* repo.list(filter).pipe(
      Effect.catchAll(() => Effect.succeed([])),
    );

    output(tasks, options.json);
    return tasks;
  });

const cmdReady = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);
    const filter: TaskFilter = {
      sortPolicy: "priority",
      priority: options.priority,
      labelsAny: options.labels && options.labels.length > 0 ? options.labels : undefined,
      assignee: options.assignee,
      unassigned: options.unassigned,
      limit: options.limit,
    };

    const tasks = yield* repo.ready(filter).pipe(
      Effect.catchAll(() => Effect.succeed([])),
    );

    output(tasks, options.json);
    return tasks;
  });

const cmdNext = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);
    const filter: TaskFilter = {
      sortPolicy: "priority",
      assignee: options.assignee,
    };

    const task = yield* repo.pickNext(filter).pipe(
      Effect.catchAll(() => Effect.succeed(null)),
    );

    if (!task) {
      output(null, options.json);
      return null;
    }

    const updated = yield* repo.update(
      { id: task.id, status: "in_progress" },
    ).pipe(Effect.catchAll(() => Effect.succeed(task)));

    output(updated, options.json);
    return updated;
  });

const cmdCreate = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);
    let taskData: TaskCreate;

    if (options.jsonInput) {
      const stdin = yield* Effect.tryPromise({
        try: () => readStdin(),
        catch: (e) => new Error(`Failed to read stdin: ${e}`),
      });
      taskData = JSON.parse(stdin.trim()) as TaskCreate;
    } else {
      if (!options.title) {
        output({ error: "Missing required --title" }, options.json);
        return null;
      }
      taskData = {
        title: options.title,
        description: options.description || "",
        type: (options.type as TaskCreate["type"]) || "task",
        priority: options.priority ?? 2,
        status: "open",
        labels: options.labels || [],
        deps: [],
        comments: [],
        assignee: options.assignee,
      };
    }

    const task = yield* repo.create(taskData, { idPrefix: "oa" }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (task) {
      output(task, options.json);
    }
    return task;
  });

const cmdUpdate = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);

    if (!options.jsonInput) {
      output({ error: "update requires --json-input with JSON on stdin" }, options.json);
      return null;
    }

    const stdin = yield* Effect.tryPromise({
      try: () => readStdin(),
      catch: (e) => new Error(`Failed to read stdin: ${e}`),
    });

    const data = JSON.parse(stdin.trim()) as { id: string } & TaskUpdate & {
      commits?: string[];
      reason?: string;
    };

    if (!data.id) {
      output({ error: "Missing required 'id' field" }, options.json);
      return null;
    }

    const { id, commits, reason, ...updateFields } = data;

    if (updateFields.status === "closed" && reason) {
      const task = yield* repo.close({
        id,
        reason,
        commits: commits || [],
      }).pipe(
        Effect.catchAll((e) => {
          output({ error: e.message }, options.json);
          return Effect.succeed(null);
        }),
      );
      if (task) {
        output(task, options.json);
      }
      return task;
    }

    let mergedCommits: string[] | undefined;
    if (commits?.length) {
      const currentTask = yield* getTaskWithDeps(repo.paths.tasksPath, id).pipe(
        Effect.provide(BunContext.layer),
        Effect.catchAll(() => Effect.succeed(null)),
      );
      mergedCommits = [...(currentTask?.commits ?? []), ...commits];
    }

    const updatePayload: { id: string } & TaskUpdate = {
      id,
      ...updateFields,
      ...(mergedCommits ? { commits: mergedCommits } : {}),
    };

    const task = yield* repo.update(updatePayload).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (task) {
      output(task, options.json);
    }
    return task;
  });

const cmdClose = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }

    // Get commit SHA: use provided, or fetch from git
    let commitSha: string | undefined = options.commit;
    if (!commitSha) {
      const sha = yield* Effect.tryPromise({
        try: () => getLatestCommitSha(options.rootDir),
        catch: () => new Error("Failed to get latest commit SHA"),
      });
      commitSha = sha ?? undefined;
    }

    const commits = commitSha ? [commitSha] : [];
    const reason = options.reason || "Completed";

    const task = yield* repo.close({
      id: options.id,
      reason,
      commits,
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!task) {
      return null;
    }

    // Stage tasks file if requested
    if (options.stage) {
      const staged = yield* Effect.tryPromise({
        try: () => stageTasksFile(options.rootDir),
        catch: () => new Error("Failed to stage tasks file"),
      });
      if (!staged) {
        output({ warning: "Task closed but failed to stage tasks file" }, options.json);
      }
    }

    output(task, options.json);
    return task;
  });

const cmdReopen = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }

    const task = yield* repo.reopen(options.id).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (task) {
      output(task, options.json);
    }
    return task;
  });

const cmdArchive = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const staleTasks = yield* getStaleTasks({
      tasksPath,
      daysOld: options.days ?? 30,
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!staleTasks) {
      return null;
    }

    const taskIds = staleTasks.map((task) => task.id);

    if (taskIds.length === 0) {
      const summary = { archived: 0, dryRun: Boolean(options.dryRun), taskIds: [] };
      output(summary, options.json);
      return summary;
    }

    if (options.dryRun) {
      const summary = {
        archived: 0,
        dryRun: true,
        taskIds,
      };
      output(summary, options.json);
      return summary;
    }

    const archivedCount = yield* archiveTasks({ taskIds }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (archivedCount === null) {
      return null;
    }

    const summary = {
      archived: archivedCount,
      staleTasks: taskIds.length,
      taskIds,
      dryRun: false,
    };

    output(summary, options.json);
    return summary;
  });

const cmdCompact = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const result = yield* compactTasks({
      tasksPath,
      daysOld: options.days ?? 90,
      preview: options.dryRun ?? true, // Default to analyze mode
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    const summary = {
      compacted: result.compacted,
      dryRun: options.dryRun ?? true,
    };

    output(summary, options.json);
    return summary;
  });

const cmdHooksInstall = (options: CliOptions) =>
  Effect.gen(function* () {
    const result = yield* installHooks({
      rootDir: options.rootDir,
      openagentsDir: nodePath.join(options.rootDir, OPENAGENTS_DIR),
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    const summary = {
      installed: result.installed,
      skipped: result.skipped,
      errors: result.errors,
      success: result.errors.length === 0,
    };

    output(summary, options.json);
    return summary;
  });

const cmdHooksUninstall = (options: CliOptions) =>
  Effect.gen(function* () {
    const result = yield* uninstallHooks({
      rootDir: options.rootDir,
      openagentsDir: nodePath.join(options.rootDir, OPENAGENTS_DIR),
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    const summary = {
      uninstalled: result.installed,
      skipped: result.skipped,
      errors: result.errors,
      success: result.errors.length === 0,
    };

    output(summary, options.json);
    return summary;
  });

const cmdSearch = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    if (!options.query) {
      output({ error: "Missing required --query <text>" }, options.json);
      return null;
    }

    const result = yield* searchAllTasks({
      tasksPath,
      query: options.query,
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    const summary = {
      query: options.query,
      total: result.length,
      tasks: result,
    };

    output(summary, options.json);
    return summary;
  });

const findOrphanDependencies = (
  tasks: Task[],
): { taskId: string; missingId: string; type: DependencyT["type"] }[] => {
  const ids = new Set(tasks.map((t) => t.id));
  return tasks.flatMap((task) =>
    (task.deps ?? [])
      .filter((dep) => !ids.has(dep.id))
      .map((dep) => ({ taskId: task.id, missingId: dep.id, type: dep.type })),
  );
};

const findDuplicateIds = (tasks: Task[]): string[] => {
  const counts = new Map<string, number>();
  for (const task of tasks) {
    counts.set(task.id, (counts.get(task.id) ?? 0) + 1);
  }
  return [...counts.entries()].filter(([, count]) => count > 1).map(([id]) => id);
};

const hashTaskContent = (task: Task): string => {
  const title = task.title?.trim() ?? "";
  const description = task.description?.trim() ?? "";
  return createHash("sha256").update(title).update("\n").update(description).digest("hex");
};

const groupDuplicateTasks = (tasks: Task[]) => {
  const byStatus = new Map<Task["status"], Map<string, Task[]>>();

  for (const task of tasks) {
    const statusMap = byStatus.get(task.status) ?? new Map<string, Task[]>();
    byStatus.set(task.status, statusMap);

    const hash = hashTaskContent(task);
    const bucket = statusMap.get(hash) ?? [];
    bucket.push(task);
    statusMap.set(hash, bucket);
  }

  const groups: Array<{ status: Task["status"]; hash: string; tasks: Task[] }> = [];
  for (const [status, map] of byStatus.entries()) {
    for (const [hash, dupes] of map.entries()) {
      if (dupes.length > 1) {
        groups.push({ status, hash, tasks: dupes });
      }
    }
  }

  return groups;
};

const findDependencyCycles = (tasks: Task[]): string[][] => {
  const ids = new Set(tasks.map((t) => t.id));
  const graph = new Map<string, string[]>();
  for (const task of tasks) {
    graph.set(
      task.id,
      (task.deps ?? []).map((dep) => dep.id).filter((depId) => ids.has(depId)),
    );
  }

  const cycles: string[][] = [];
  const seen = new Set<string>();
  const stack = new Set<string>();
  const path: string[] = [];
  const recorded = new Set<string>();

  const dfs = (node: string) => {
    seen.add(node);
    stack.add(node);
    path.push(node);

    for (const neighbor of graph.get(node) ?? []) {
      if (!stack.has(neighbor)) {
        if (!seen.has(neighbor)) {
          dfs(neighbor);
        }
      } else {
        const idx = path.indexOf(neighbor);
        if (idx !== -1) {
          const cycle = [...path.slice(idx), neighbor];
          const key = cycle.join("->");
          if (!recorded.has(key)) {
            recorded.add(key);
            cycles.push(cycle);
          }
        }
      }
    }

    path.pop();
    stack.delete(node);
  };

  for (const id of ids) {
    if (!seen.has(id)) {
      dfs(id);
    }
  }

  return cycles;
};

const getValueAtPath = (obj: unknown, path: string): unknown => {
  if (typeof obj !== "object" || obj === null) return undefined;
  return path.split(".").reduce<unknown>((acc, key) => {
    if (typeof acc !== "object" || acc === null) return undefined;
    return (acc as Record<string, unknown>)[key];
  }, obj);
};

const setValueAtPath = (
  obj: Record<string, unknown>,
  path: string,
  value: unknown,
): Record<string, unknown> => {
  const parts = path.split(".");
  let current: Record<string, unknown> = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i]!;
    const next = current[key];
    if (typeof next !== "object" || next === null) {
      current[key] = {};
    }
    current = current[key] as Record<string, unknown>;
  }
  current[parts[parts.length - 1]!] = value;
  return obj;
};

const parseOlderThanDays = (options: CliOptions): number => {
  if (options.days && options.days > 0) return options.days;
  if (options.olderThan) {
    const text = options.olderThan.endsWith("d")
      ? options.olderThan.slice(0, -1)
      : options.olderThan;
    const num = parseInt(text, 10);
    if (!Number.isNaN(num) && num > 0) return num;
  }
  return 30;
};

const cmdValidate = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const fs = yield* FileSystem.FileSystem;
    const exists = yield* fs.exists(tasksPath);
    if (!exists) {
      const result = { ok: false, error: `${TASKS_FILE} is missing` };
      output(result, options.json);
      return result;
    }

    const content = yield* fs.readFileString(tasksPath);
    if (hasConflictMarkers(content)) {
      const result = {
        ok: false,
        error: "conflict_markers_detected",
        message:
          "Merge conflict markers detected in .openagents/tasks.jsonl. Resolve conflicts before running agents.",
      };
      output(result, options.json);
      return result;
    }

    const tasks = yield* readTasks(tasksPath);
    const orphanDeps = findOrphanDependencies(tasks);
    if (orphanDeps.length > 0) {
      const result = {
        ok: false,
        error: "orphan_dependencies",
        orphanDeps,
      };
      output(result, options.json);
      return result;
    }

    const result = { ok: true, message: "tasks.jsonl is valid" };
    output(result, options.json);
    return result;
  });

const cmdDoctor = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const fs = yield* FileSystem.FileSystem;
    const issues: Array<Record<string, unknown>> = [];

    const exists = yield* fs.exists(tasksPath);
    if (!exists) {
      const result = {
        ok: false,
        issues: [{ type: "missing_file", message: `${TASKS_FILE} is missing` }],
      };
      output(result, options.json);
      return result;
    }

    let tasks: Task[] | null = null;
    try {
      tasks = yield* readTasks(tasksPath);
    } catch (err) {
      if (err instanceof TaskServiceError) {
        const type =
          err.reason === "conflict"
            ? "conflict_markers"
            : err.reason === "parse_error"
              ? "parse_error"
              : "validation_error";
        issues.push({ type, message: err.message });
      } else {
        issues.push({ type: "unknown_error", message: String(err) });
      }
    }

    if (tasks) {
      const orphanDeps = findOrphanDependencies(tasks);
      if (orphanDeps.length > 0) {
        issues.push({ type: "orphan_dependencies", orphanDeps });
      }

      const duplicates = findDuplicateIds(tasks);
      if (duplicates.length > 0) {
        issues.push({ type: "duplicate_ids", ids: duplicates });
      }

      const cycles = findDependencyCycles(tasks);
      if (cycles.length > 0) {
        issues.push({ type: "dependency_cycles", cycles });
      }

      const staleTasks = yield* getStaleTasks({
        tasksPath,
        daysOld: options.days ?? 14,
      }).pipe(Effect.catchAll(() => Effect.succeed([])));
      if (staleTasks.length > 0) {
        const now = Date.now();
        const stale = staleTasks.map((task) => ({
          id: task.id,
          status: task.status,
          daysStale: Math.floor((now - new Date(task.updatedAt).getTime()) / (24 * 60 * 60 * 1000)),
        }));
        issues.push({ type: "stale_tasks", stale });
      }
    }

    const result = { ok: issues.length === 0, issues };
    output(result, options.json);
    return result;
  });

const cmdRepairDeps = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const fs = yield* FileSystem.FileSystem;

    const exists = yield* fs.exists(tasksPath);
    if (!exists) {
      const result = { ok: false, error: `${TASKS_FILE} is missing` };
      output(result, options.json);
      return result;
    }

    const tasks = yield* readTasks(tasksPath);
    const orphanDeps = findOrphanDependencies(tasks);
    const removedCount = orphanDeps.length;
    const tasksUpdated = [...new Set(orphanDeps.map((d) => d.taskId))];

    if (removedCount === 0) {
      const result = { ok: true, removedCount: 0, tasksUpdated };
      if (options.json) {
        output(result, true);
      } else {
        console.log("No orphan dependencies found.");
      }
      return result;
    }

    if (options.dryRun) {
      const result = {
        ok: false,
        dryRun: true,
        removedCount,
        tasksUpdated,
        orphanDeps,
      };
      output(result, options.json);
      return result;
    }

    const ids = new Set(tasks.map((t) => t.id));
    const repaired = tasks.map((task) => {
      if (!task.deps || task.deps.length === 0) return task;
      const filteredDeps = task.deps.filter((dep) => ids.has(dep.id));
      if (filteredDeps.length === task.deps.length) return task;
      return { ...task, deps: filteredDeps };
    });

    yield* writeTasks(tasksPath, repaired);

    const result = { ok: true, removedCount, tasksUpdated, fixedDeps: orphanDeps };
    if (options.json) {
      output(result, true);
    } else {
      console.log(
        `Removed ${removedCount} orphan dependencies from ${tasksUpdated.length} task(s).`,
      );
    }
    return result;
  });

const cmdDuplicates = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const fs = yield* FileSystem.FileSystem;

    const exists = yield* fs.exists(tasksPath);
    if (!exists) {
      const result = {
        ok: false,
        error: `${TASKS_FILE} is missing`,
      };
      output(result, options.json);
      return result;
    }

    const tasks = yield* readTasks(tasksPath);
    const groups = groupDuplicateTasks(tasks).map((group) => ({
      status: group.status,
      hash: group.hash,
      ids: group.tasks.map((t) => t.id),
      titles: group.tasks.map((t) => t.title),
    }));

    const result = { ok: groups.length === 0, groups };

    if (!options.json) {
      if (groups.length === 0) {
        console.log("No duplicate tasks found.");
      } else {
        for (const group of groups) {
          console.log(`status=${group.status} hash=${group.hash}`);
          for (const id of group.ids) {
            console.log(`  - ${id}`);
          }
        }
      }
    } else {
      output(result, true);
    }

    return result;
  });

const cmdConfigList = (options: CliOptions) =>
  Effect.gen(function* () {
    const config = yield* loadProjectConfig(options.rootDir).pipe(
      Effect.catchAll((e) => {
        output({ ok: false, error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );
    if (!config) {
      const result = { ok: false, error: "project_config_missing" };
      output(result, options.json);
      return result;
    }
    output(config, options.json);
    return config;
  });

const cmdConfigGet = (options: CliOptions) =>
  Effect.gen(function* () {
    if (!options.key) {
      const result = { ok: false, error: "Missing required --key" };
      output(result, options.json);
      return result;
    }

    const config = yield* loadProjectConfig(options.rootDir).pipe(
      Effect.catchAll((e) => {
        output({ ok: false, error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!config) {
      const result = { ok: false, error: "project_config_missing" };
      output(result, options.json);
      return result;
    }

    const value = getValueAtPath(config, options.key);
    const result = { ok: true, key: options.key, value: value ?? null };
    output(result, options.json);
    return result;
  });

const cmdConfigSet = (options: CliOptions) =>
  Effect.gen(function* () {
    if (!options.key) {
      const result = { ok: false, error: "Missing required --key" };
      output(result, options.json);
      return result;
    }
    if (options.value === undefined) {
      const result = { ok: false, error: "Missing required --value" };
      output(result, options.json);
      return result;
    }

    let parsedValue: unknown = options.value;
    try {
      parsedValue = JSON.parse(options.value);
    } catch {
      parsedValue = options.value;
    }

    let config =
      (yield* loadProjectConfig(options.rootDir).pipe(
        Effect.catchAll((e) => {
          output({ ok: false, error: e.message }, options.json);
          return Effect.succeed(null);
        }),
      )) ?? defaultProjectConfig(nodePath.basename(options.rootDir));

    const updated = setValueAtPath(
      JSON.parse(JSON.stringify(config)) as Record<string, unknown>,
      options.key,
      parsedValue,
    );

    const validated = decodeProjectConfig(updated as unknown);

    yield* saveProjectConfig(options.rootDir, validated).pipe(
      Effect.catchAll((e) => {
        output({ ok: false, error: e.message }, options.json);
        return Effect.succeed(undefined);
      }),
    );

    const result = { ok: true, config: validated };
    output(result, options.json);
    return result;
  });

const cmdCleanup = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const fs = yield* FileSystem.FileSystem;
    const exists = yield* fs.exists(tasksPath);
    if (!exists) {
      const result = { ok: false, error: `${TASKS_FILE} is missing` };
      output(result, options.json);
      return result;
    }

    const days = parseOlderThanDays(options);
    const cutoff = Date.now() - days * 24 * 60 * 60 * 1000;
    const tasks = yield* readTasks(tasksPath);

    const candidates = tasks.filter(
      (t) => t.status === "closed" && t.closedAt && new Date(t.closedAt).getTime() < cutoff,
    );
    const candidateIds = new Set(candidates.map((t) => t.id));

    const references = new Map<string, string[]>();
    for (const task of tasks) {
      for (const dep of task.deps ?? []) {
        if (candidateIds.has(dep.id)) {
          const arr = references.get(dep.id) ?? [];
          arr.push(task.id);
          references.set(dep.id, arr);
        }
      }
    }

    const referenced = new Set<string>();
    for (const [id, referrers] of references.entries()) {
      if (referrers.length > 0) referenced.add(id);
    }

    const deletable = options.cascade
      ? candidates
      : candidates.filter((c) => !referenced.has(c.id));
    const skipped = candidates.filter((c) => referenced.has(c.id));

    if (options.dryRun) {
      const result = {
        ok: true,
        dryRun: true,
        days,
        deletedCount: deletable.length,
        deletedIds: deletable.map((t) => t.id),
        skippedReferenced: skipped.map((t) => ({ id: t.id, referencedBy: references.get(t.id) })),
      };
      output(result, options.json);
      return result;
    }

    if (deletable.length === 0) {
      const result = {
        ok: true,
        deletedCount: 0,
        skippedReferenced: skipped.map((t) => ({ id: t.id, referencedBy: references.get(t.id) })),
      };
      output(result, options.json);
      return result;
    }

    const deletedIds = new Set(deletable.map((t) => t.id));
    const remaining = tasks.filter((t) => !deletedIds.has(t.id));

    const cleanedRemaining = options.cascade
      ? remaining.map((task) => ({
          ...task,
          deps: (task.deps ?? []).filter((dep) => !deletedIds.has(dep.id)),
        }))
      : remaining;

    yield* writeTasks(tasksPath, cleanedRemaining);

    const result = {
      ok: true,
      deletedCount: deletable.length,
      deletedIds: [...deletedIds],
      skippedReferenced: skipped.map((t) => ({ id: t.id, referencedBy: references.get(t.id) })),
      depsPruned: options.cascade,
    };
    output(result, options.json);
    return result;
  });

const cmdDelete = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    if (!options.id) {
      const result = { ok: false, error: "Missing required --id <task-id>" };
      output(result, options.json);
      return result;
    }

    const tasks = yield* readTasks(tasksPath);
    const target = tasks.find((t) => t.id === options.id);
    if (!target) {
      const result = { ok: false, error: "not_found", message: `Task ${options.id} not found` };
      output(result, options.json);
      return result;
    }

    const removedIds = new Set<string>();
    const deleteCascade = options.deleteCascade ?? options.cascade ?? false;
    const cascadeQueue = [target.id];

    if (deleteCascade) {
      while (cascadeQueue.length > 0) {
        const currentId = cascadeQueue.pop()!;
        if (removedIds.has(currentId)) continue;
        removedIds.add(currentId);

        for (const t of tasks) {
          if ((t.deps ?? []).some((d) => d.id === currentId)) {
            cascadeQueue.push(t.id);
          }
        }
      }
    } else {
      removedIds.add(target.id);
    }

    const remaining = tasks.filter((t) => !removedIds.has(t.id));
    const pruned = remaining.map((t) => ({
      ...t,
      deps: (t.deps ?? []).filter((d) => !removedIds.has(d.id)),
    }));

    if (options.dryRun) {
      const result = {
        ok: true,
        dryRun: true,
        deletedIds: [...removedIds],
        remaining: pruned.length,
      };
      output(result, options.json);
      return result;
    }

    yield* writeTasks(tasksPath, pruned);

    const result = {
      ok: true,
      deletedIds: [...removedIds],
      remaining: pruned.length,
    };
    output(result, options.json);
    return result;
  });

const cmdMerge = (options: CliOptions) =>
  Effect.gen(function* () {
    if (options.ids && options.into) {
      const tasksPath = getTasksPath(options.rootDir);
      const result = yield* mergeTasksById({
        tasksPath,
        targetId: options.into,
        sourceIds: options.ids,
      }).pipe(
        Effect.catchAll((e) => {
          output({ ok: false, error: e.message }, options.json);
          return Effect.succeed(null);
        }),
      );

      if (!result) {
        return null;
      }

      if (!options.json) {
        console.log(`Merged ${options.ids.length} task(s) into ${result.id}`);
      } else {
        output(result, true);
      }

      return result;
    }

    if (!options.base || !options.current || !options.incoming) {
      return yield* Effect.fail(
        new TaskMergeError("merge requires --base, --current, and --incoming paths"),
      );
    }

    const resolvePath = (p: string) =>
      nodePath.isAbsolute(p) ? p : nodePath.join(options.rootDir, p);

    const mergeOptions = {
      basePath: resolvePath(options.base),
      currentPath: resolvePath(options.current),
      incomingPath: resolvePath(options.incoming),
      ...(options.output ? { outputPath: resolvePath(options.output) } : {}),
    };

    const result = yield* mergeTaskFiles(mergeOptions);

    const payload = { ok: true, mergedPath: result.mergedPath, conflicts: result.conflicts };
    output(payload, options.json);
    return payload;
  });

const cmdStats = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const stats = yield* getTaskStats(tasksPath).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!stats) {
      return null;
    }

    if (options.json) {
      output(stats, true);
    } else {
      console.log(`Task Statistics`);
      console.log(`===============`);
      console.log(`Total tasks: ${stats.total}`);
      console.log();
      console.log(`By Status:`);
      console.log(`  Open:        ${stats.openCount}`);
      console.log(`  In Progress: ${stats.inProgressCount}`);
      console.log(`  Blocked:     ${stats.blockedCount}`);
      console.log(`  Closed:      ${stats.closedCount}`);
      console.log();
      console.log(`By Type:`);
      for (const [type, count] of Object.entries(stats.byType).sort(([, a], [, b]) => (b as number) - (a as number))) {
        console.log(`  ${type}: ${count}`);
      }
      console.log();
      console.log(`By Priority:`);
      for (const [priority, count] of Object.entries(stats.byPriority).sort(([a], [b]) => Number(a as string) - Number(b as string))) {
        const label = ["P0 (Critical)", "P1 (High)", "P2 (Medium)", "P3 (Low)", "P4 (Backlog)"][Number(priority)] || `P${priority}`;
        console.log(`  ${label}: ${count}`);
      }
    }

    return stats;
  });

const cmdStale = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const staleOptions: { tasksPath: string; days: number; status?: string } = {
      tasksPath,
      days: options.days ?? 30,
    };
    if (options.status) {
      staleOptions.status = options.status;
    }
    const staleTasks = yield* getStaleTasks(staleOptions).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!staleTasks) {
      return null;
    }

    if (options.json) {
      output(staleTasks, true);
    } else {
      if (staleTasks.length === 0) {
        console.log(`No stale tasks found (threshold: ${options.days ?? 30} days)`);
      } else {
        console.log(`Stale tasks (not updated in ${options.days ?? 30}+ days):`);
        console.log(`===============================================`);
        for (const task of staleTasks) {
          const daysSince = Math.floor(
            (Date.now() - new Date(task.updatedAt).getTime()) / (24 * 60 * 60 * 1000)
          );
          console.log(`[${task.id}] ${task.title}`);
          console.log(`  Status: ${task.status}, Last updated: ${daysSince} days ago`);
        }
        console.log();
        console.log(`Total: ${staleTasks.length} stale tasks`);
      }
    }

    return staleTasks;
  });

const cmdShow = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }

    const result = yield* getTaskWithDeps(tasksPath, options.id).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    const task = result;

    if (options.json) {
      output(task, true);
    } else {
      const priorityLabel = ["P0 (Critical)", "P1 (High)", "P2 (Medium)", "P3 (Low)", "P4 (Backlog)"][task.priority] || `P${task.priority}`;

      console.log(`Task: ${task.id}`);
      console.log(`======================================`);
      console.log(`Title: ${task.title}`);
      console.log(`Status: ${task.status}`);
      console.log(`Priority: ${priorityLabel}`);
      console.log(`Type: ${task.type}`);
      if (task.assignee) console.log(`Assignee: ${task.assignee}`);
      if (task.labels?.length) console.log(`Labels: ${task.labels.join(", ")}`);
      console.log(`Created: ${task.createdAt}`);
      console.log(`Updated: ${task.updatedAt}`);
      if (task.closedAt) console.log(`Closed: ${task.closedAt}`);
      if (task.closeReason) console.log(`Close Reason: ${task.closeReason}`);
      if (task.commits?.length) console.log(`Commits: ${task.commits.join(", ")}`);

      if (task.description) {
        console.log();
        console.log(`Description:`);
        console.log(task.description);
      }

      if (task.deps && task.deps.length > 0) {
        console.log();
        console.log(`Dependencies (${task.deps.length}):`);
        for (const dep of task.deps) {
          console.log(`  - ${dep.id} (${dep.type})`);
        }
      }
    }

    return result;
  });

const cmdCommentAdd = (options: CliOptions) =>
  Effect.gen(function* () {
    const repo = getTaskRepository(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }
    if (!options.text) {
      output({ error: "Missing required --text <comment>" }, options.json);
      return null;
    }

    const author = options.author ?? process.env.USER ?? "unknown";

    const addOptions = {
      taskId: options.id,
      comment: {
        text: options.text,
        author,
      },
    };

    const result = yield* repo.addComment(addOptions).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    if (options.json) {
      output(result, true);
    } else {
      const newComment = result.comments?.[result.comments.length - 1];
      console.log(
        `Added comment ${newComment?.id ?? "unknown"} by ${newComment?.author ?? author} to task ${options.id}`,
      );
    }

    return result;
  });

const cmdCommentList = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }

    const comments = yield* listComments({ tasksPath, taskId: options.id }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!comments) {
      return null;
    }

    if (options.json) {
      output({ id: options.id, comments }, true);
    } else {
      if (comments.length === 0) {
        console.log(`No comments for task ${options.id}`);
      } else {
        console.log(`Comments for task ${options.id}:`);
        for (const c of comments) {
          console.log(`- [${c.createdAt}] ${c.author}: ${c.text} (${c.id})`);
        }
      }
    }

    return comments;
  });

const cmdRenamePrefix = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);

    if (!options.fromPrefix || !options.toPrefix) {
      output({ error: "Missing required --from <prefix> and --to <prefix>" }, options.json);
      return null;
    }

    const result = yield* renameTaskPrefix({
      tasksPath,
      oldPrefix: options.fromPrefix,
      newPrefix: options.toPrefix,
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    if (options.json) {
      output(result, true);
    } else {
      console.log(
        `Renamed ${result.renamed} task(s) from ${options.fromPrefix} to ${options.toPrefix}`,
      );
    }

    return result;
  });

const showHelp = (): void => {
  console.log(`
OpenAgents Task CLI

Usage: bun src/tasks/cli.ts <command> [options]

Commands:
  init      Initialize .openagents for a repo
  list      List tasks with optional filters
  ready     List ready tasks (no open blockers)
  next      Pick the next ready task and mark it in_progress
  create    Create a new task
  update    Update an existing task
  close     Close a task with optional commit SHA capture
  reopen    Reopen a closed task
  show      Show task details with dependency tree
  rename-prefix Rename task IDs to a new prefix (with dry-run)
  comment:add  Add a comment to a task
  comment:list List comments for a task
  stats     Show task statistics (counts by status, type, priority)
  stale     Find tasks not updated in N days
  archive   Archive old closed tasks to tasks-archive.jsonl
  search    Search tasks across active and archived
  validate  Validate tasks.jsonl (schema + conflict markers)
  cleanup   Delete closed tasks older than N days (with optional cascade)
  delete    Delete a task (with optional cascade to dependents)
  config:list  Show project config (project.json)
  config:get   Get a project config value by key (dot notation supported)
  config:set   Set a project config value by key (value parsed as JSON when possible)
  doctor    Diagnose common tasks.jsonl issues (orphan deps, duplicates, cycles, stale tasks)
  repair-deps Fix orphan dependencies by removing references to missing tasks
  duplicates Find duplicate tasks by title+description hash (grouped by status)
  merge     Three-way merge for .openagents/tasks.jsonl (git merge driver helper)
  merge --ids <a,b> --into <target>   Merge duplicate tasks into target ID (updates deps; supports --dry-run)

Global Options:
  --json          Output as JSON
  --json-input    Read JSON from stdin (for create/update)
  --dir <path>    Set root directory (default: cwd)

list/ready Options:
  --status <status>       Filter by status (open, in_progress, blocked, closed)
  --priority <0-4>        Filter by priority
  --type <type>           Filter by type (bug, feature, task, epic, chore)
  --labels <a,b,c>        Filter by labels (comma-separated)
  --assignee <name>       Filter by assignee
  --unassigned            Filter to unassigned tasks
  --limit <n>             Limit results

create Options:
  --title <title>         Task title (required unless --json-input)
  --description <desc>    Task description
  --type <type>           Task type (default: task)
  --priority <0-4>        Task priority (default: 2)
  --labels <a,b,c>        Task labels (comma-separated)
  --assignee <name>       Task assignee

init Options:
  --project-id <id>       Custom project ID (default: directory name)

close Options:
  --id <task-id>          Task ID to close (required)
  --reason <reason>       Close reason (default: "Completed")
  --commit <sha>          Commit SHA to attach (default: auto-detect from HEAD)
  --stage                 Stage .openagents/tasks.jsonl after updating

reopen Options:
  --id <task-id>          Task ID to reopen (required)

show Options:
  --id <task-id>          Task ID to show (required)

comment:add Options:
  --id <task-id>          Task ID to comment on (required)
  --text <comment>        Comment text (required)
  --author <name>         Comment author (default: $USER or "unknown")
  --comment-id <id>       Optional explicit comment ID

comment:list Options:
  --id <task-id>          Task ID to list comments for (required)

rename-prefix Options:
  --from <prefix>         Existing prefix to replace (required)
  --to <prefix>           New prefix to apply (required)
  --dry-run               Preview changes without writing

stale Options:
  --days <n>              Find tasks not updated in N days (default: 30)
  --status <status>       Filter by status (open, in_progress, blocked, closed)

archive Options:
  --days <n>              Archive tasks closed more than N days ago (default: 30)
  --dry-run               Show what would be archived without making changes

search Options:
  --query <text>          Search text in title/description
  --include-archived      Include archived tasks (default: true)
  (Also supports list/ready filter options)

cleanup Options:
  --older-than <n>        Delete closed tasks older than N days (default: 30)
  --dry-run               Preview what would be deleted without making changes
  --cascade               Remove deleted task IDs from dependencies of remaining tasks

delete Options:
  --id <task-id>          Task ID to delete (required)
  --dry-run               Preview deletions without writing
  --delete-cascade        Also delete tasks that depend on this task (recursively) (alias: --cascade)

validate Options:
  --check-conflicts       Fail if git conflict markers are present (default: also checked during full parse)

repair-deps Options:
  --dry-run               Preview orphan dependency removals without writing changes

config:get Options:
  --key <path>            Config key (dot notation supported)

config:set Options:
  --key <path>            Config key (dot notation supported)
  --value <value>         Value to set (parsed as JSON when possible, otherwise treated as string)

doctor Options:
  --days <n>              Stale threshold (default: 14, in-progress tasks only)

merge Options:
  --base <path>           Base/ancestor tasks.jsonl
  --current <path>        Current branch version (usually %A)
  --incoming <path>       Incoming branch version (usually %B)
  --output <path>         Output path (default: overwrite current)
  --ids <a,b,c>           Task IDs to merge (with --into)
  --into <target>         Target task ID for merge (with --ids)

Examples:
  bun src/tasks/cli.ts init --json
  bun src/tasks/cli.ts list --status open --json
  bun src/tasks/cli.ts ready --limit 5 --json
  bun src/tasks/cli.ts next --json
  bun src/tasks/cli.ts create --title "Fix bug" --type bug --priority 1 --json
  bun src/tasks/cli.ts close --id oa-123 --reason "Implemented feature" --json
  bun src/tasks/cli.ts close --id oa-123 --stage --json  # auto-capture HEAD commit and stage
  bun src/tasks/cli.ts reopen --id oa-123 --json  # reopen a closed task
  bun src/tasks/cli.ts show --id oa-123 --json  # show task details and deps
  bun src/tasks/cli.ts stats --json  # show task statistics
  bun src/tasks/cli.ts stale --days 7 --status open --json  # find stale open tasks
  echo '{"id":"oa-123","status":"closed","reason":"Done"}' | bun src/tasks/cli.ts update --json-input --json
  bun src/tasks/cli.ts archive --dry-run --json  # preview what would be archived
  bun src/tasks/cli.ts archive --days 7 --json  # archive tasks closed >7 days ago
  bun src/tasks/cli.ts search --query "auth" --json  # search all tasks
  bun src/tasks/cli.ts search --query "login" --status closed --json  # search closed tasks only
  bun src/tasks/cli.ts validate --json  # validate tasks.jsonl and detect conflicts
  bun src/tasks/cli.ts config:list --json  # show project config
  bun src/tasks/cli.ts config:get --key defaultModel --json  # read a config value
  bun src/tasks/cli.ts config:set --key claudeCode.enabled --value false --json  # update a config value
  bun src/tasks/cli.ts cleanup --older-than 90 --json  # delete closed tasks older than 90 days
  bun src/tasks/cli.ts delete --id oa-123 --json  # delete a task by ID
  bun src/tasks/cli.ts doctor --json  # diagnose orphan deps, duplicates, cycles, stale tasks
  bun src/tasks/cli.ts comment:add --id oa-123 --text "Noted" --author alice --json  # add a comment
  bun src/tasks/cli.ts comment:list --id oa-123 --json  # list comments
  bun src/tasks/cli.ts rename-prefix --from oa --to zz --dry-run --json  # preview ID prefix rename
  bun src/tasks/cli.ts merge --ids oa-1,oa-2 --into oa-1 --dry-run --json  # preview merge of duplicate tasks
`);
};

// Helper to create CLI layer with correct database path
const makeCliLayer = (rootDir: string) => {
  const dbPath = nodePath.join(rootDir, OPENAGENTS_DIR, "openagents.db");
  return Layer.mergeAll(makeDatabaseLive(dbPath), BunContext.layer);
};

const main = async () => {
  const { command, options } = parseArgs(process.argv.slice(2));
  const cliLayer = makeCliLayer(options.rootDir);

  switch (command) {
    case "init":
      try {
        await Effect.runPromise(cmdInit(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "list":
      try {
        await Effect.runPromise(cmdList(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "ready":
      try {
        await Effect.runPromise(cmdReady(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "next":
      try {
        await Effect.runPromise(cmdNext(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "create":
      try {
        await Effect.runPromise(cmdCreate(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "update":
      try {
        await Effect.runPromise(cmdUpdate(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "close":
      try {
        await Effect.runPromise(cmdClose(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "reopen":
      try {
        await Effect.runPromise(cmdReopen(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "show":
      try {
        await Effect.runPromise(cmdShow(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "comment:add":
      try {
        await Effect.runPromise(cmdCommentAdd(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "comment:list":
      try {
        await Effect.runPromise(cmdCommentList(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "rename-prefix":
      try {
        await Effect.runPromise(cmdRenamePrefix(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "stats":
      try {
        await Effect.runPromise(cmdStats(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "stale":
      try {
        await Effect.runPromise(cmdStale(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "archive":
      try {
        await Effect.runPromise(cmdArchive(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "compact":
      try {
        await Effect.runPromise(cmdCompact(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "hooks:install":
      try {
        const result = await Effect.runPromise(
          cmdHooksInstall(options).pipe(Effect.provide(cliLayer)),
        );
        if (result && !result.success) {
          process.exitCode = 1;
        }
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "hooks:uninstall":
      try {
        const result = await Effect.runPromise(
          cmdHooksUninstall(options).pipe(Effect.provide(cliLayer)),
        );
        if (result && !result.success) {
          process.exitCode = 1;
        }
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "search":
      try {
        await Effect.runPromise(cmdSearch(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "validate":
      try {
        const result = await Effect.runPromise(
          cmdValidate(options).pipe(Effect.provide(cliLayer)),
        );
        if (result && result.ok === false) {
          process.exitCode = 1;
        }
      } catch (err) {
        const payload = { ok: false, error: String(err) };
        if (options.json) {
          console.log(JSON.stringify(payload));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "doctor":
      try {
        const result = await Effect.runPromise(
          cmdDoctor(options).pipe(Effect.provide(cliLayer)),
        );
        if (result && result.ok === false) {
          process.exitCode = 1;
        }
      } catch (err) {
        const payload = { ok: false, error: String(err) };
        if (options.json) {
          console.log(JSON.stringify(payload));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "repair-deps":
      try {
        const result = await Effect.runPromise(
          cmdRepairDeps(options).pipe(Effect.provide(cliLayer)),
        );
        if (result && result.ok === false) {
          process.exitCode = 1;
        }
      } catch (err) {
        const payload = { ok: false, error: String(err) };
        if (options.json) {
          console.log(JSON.stringify(payload));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "duplicates":
      try {
        const result = await Effect.runPromise(
          cmdDuplicates(options).pipe(Effect.provide(cliLayer)),
        );
        const hasGroups = (value: any): value is { groups: any[] } =>
          value && Array.isArray(value.groups);
        if (hasGroups(result) && result.groups.length > 0) {
          process.exitCode = 1;
        }
      } catch (err) {
        const payload = { ok: false, error: String(err) };
        if (options.json) {
          console.log(JSON.stringify(payload));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "config:list":
      try {
        await Effect.runPromise(cmdConfigList(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ ok: false, error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "config:get":
      try {
        await Effect.runPromise(cmdConfigGet(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ ok: false, error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "config:set":
      try {
        await Effect.runPromise(cmdConfigSet(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ ok: false, error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "cleanup":
      try {
        await Effect.runPromise(cmdCleanup(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ ok: false, error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "delete":
      try {
        await Effect.runPromise(cmdDelete(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        if (options.json) {
          console.log(JSON.stringify({ ok: false, error: String(err) }));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "merge":
      try {
        await Effect.runPromise(cmdMerge(options).pipe(Effect.provide(cliLayer)));
      } catch (err) {
        const payload =
          err instanceof TaskMergeError
            ? { ok: false, error: err.message }
            : { ok: false, error: String(err) };
        if (options.json) {
          console.log(JSON.stringify(payload));
        } else {
          console.error("Error:", err);
        }
        process.exit(1);
      }
      break;
    case "help":
    case "--help":
    case "-h":
      showHelp();
      process.exit(0);
      break;
    default:
      console.error(`Unknown command: ${command}`);
      showHelp();
      process.exit(1);
  }
};

main();
