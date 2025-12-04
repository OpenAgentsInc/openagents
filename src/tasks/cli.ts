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
 *   init     Initialize .openagents for a repo
 *   list     List tasks with optional filters
 *   ready    List ready tasks (no open blockers)
 *   next     Pick the next ready task and mark it in_progress
 *   create   Create a new task
 *   update   Update an existing task
 *   validate Validate tasks.jsonl (schema + conflict markers)
 *   doctor   Diagnose common issues in tasks.jsonl
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import { Effect } from "effect";
import * as nodePath from "node:path";
import {
  initOpenAgentsProject,
  readyTasks,
  pickNextTask,
  createTask,
  updateTask,
  closeTask,
  reopenTask,
  listTasks,
  archiveTasks,
  searchAllTasks,
  getTaskStats,
  getStaleTasks,
  getTaskWithDeps,
  readTasks,
  hasConflictMarkers,
  mergeTaskFiles,
  TaskMergeError,
  TaskServiceError,
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
  limit?: number;
  title?: string;
  description?: string;
  projectId?: string;
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
      case "--project-id":
        if (nextArg) {
          options.projectId = nextArg;
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
    }
  }

  return { command, options };
};

const getTasksPath = (rootDir: string): string =>
  nodePath.join(rootDir, OPENAGENTS_DIR, TASKS_FILE);

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
    const tasksPath = getTasksPath(options.rootDir);
    const filter: TaskFilter = {
      status: options.status as TaskFilter["status"],
      priority: options.priority,
      type: options.type as TaskFilter["type"],
      labelsAny: options.labels && options.labels.length > 0 ? options.labels : undefined,
      assignee: options.assignee,
      unassigned: options.unassigned,
      limit: options.limit,
    };

    const tasks = yield* listTasks(tasksPath, filter).pipe(
      Effect.catchAll(() => Effect.succeed([])),
    );

    output(tasks, options.json);
    return tasks;
  });

const cmdReady = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const filter: TaskFilter = {
      sortPolicy: "priority",
      priority: options.priority,
      labelsAny: options.labels && options.labels.length > 0 ? options.labels : undefined,
      assignee: options.assignee,
      unassigned: options.unassigned,
      limit: options.limit,
    };

    const tasks = yield* readyTasks(tasksPath, filter).pipe(
      Effect.catchAll(() => Effect.succeed([])),
    );

    output(tasks, options.json);
    return tasks;
  });

const cmdNext = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const filter: TaskFilter = {
      sortPolicy: "priority",
      assignee: options.assignee,
    };

    const task = yield* pickNextTask(tasksPath, filter).pipe(
      Effect.catchAll(() => Effect.succeed(null)),
    );

    if (!task) {
      output(null, options.json);
      return null;
    }

    const updated = yield* updateTask({
      tasksPath,
      id: task.id,
      update: { status: "in_progress" },
    }).pipe(Effect.catchAll(() => Effect.succeed(task)));

    output(updated, options.json);
    return updated;
  });

const cmdCreate = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
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
        assignee: options.assignee,
      };
    }

    const task = yield* createTask({
      tasksPath,
      task: taskData,
      idPrefix: "oa",
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
  });

const cmdUpdate = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);

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
      const task = yield* closeTask({
        tasksPath,
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

    const task = yield* updateTask({
      tasksPath,
      id,
      update: updateFields,
      appendCommits: commits || [],
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
  });

const cmdClose = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);

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

    const task = yield* closeTask({
      tasksPath,
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
    const tasksPath = getTasksPath(options.rootDir);

    if (!options.id) {
      output({ error: "Missing required --id <task-id>" }, options.json);
      return null;
    }

    const task = yield* reopenTask({
      tasksPath,
      id: options.id,
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
  });

const cmdArchive = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const result = yield* archiveTasks({
      tasksPath,
      daysOld: options.days ?? 30,
      dryRun: options.dryRun ?? false,
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
      archivedCount: result.archived.length,
      remainingCount: result.remaining.length,
      archivePath: result.archivePath,
      dryRun: result.dryRun,
      archivedIds: result.archived.map((t) => t.id),
    };

    output(summary, options.json);
    return summary;
  });

const cmdSearch = (options: CliOptions) =>
  Effect.gen(function* () {
    const tasksPath = getTasksPath(options.rootDir);
    const filter: TaskFilter = {
      status: options.status as TaskFilter["status"],
      priority: options.priority,
      type: options.type as TaskFilter["type"],
      labelsAny: options.labels && options.labels.length > 0 ? options.labels : undefined,
      assignee: options.assignee,
      unassigned: options.unassigned,
      limit: options.limit,
    };

    const result = yield* searchAllTasks({
      tasksPath,
      filter,
      includeArchived: options.includeArchived ?? true,
    }).pipe(
      Effect.catchAll((e) => {
        output({ error: e.message }, options.json);
        return Effect.succeed(null);
      }),
    );

    if (!result) {
      return null;
    }

    // If a query string is provided, filter by title/description containing it
    let { active, archived } = result;
    if (options.query) {
      const q = options.query.toLowerCase();
      active = active.filter(
        (t) =>
          t.title.toLowerCase().includes(q) ||
          (t.description && t.description.toLowerCase().includes(q)),
      );
      archived = archived.filter(
        (t) =>
          t.title.toLowerCase().includes(q) ||
          (t.description && t.description.toLowerCase().includes(q)),
      );
    }

    const summary = {
      activeCount: active.length,
      archivedCount: archived.length,
      active,
      archived,
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
        status: "in_progress",
        days: options.days ?? 14,
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

const cmdMerge = (options: CliOptions) =>
  Effect.gen(function* () {
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
      for (const [type, count] of Object.entries(stats.byType).sort(([, a], [, b]) => b - a)) {
        console.log(`  ${type}: ${count}`);
      }
      console.log();
      console.log(`By Priority:`);
      for (const [priority, count] of Object.entries(stats.byPriority).sort(([a], [b]) => Number(a) - Number(b))) {
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

    const { task, blockedBy, blocking } = result;

    if (options.json) {
      output({ task, blockedBy, blocking }, true);
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

      if (blockedBy.length > 0) {
        console.log();
        console.log(`Blocked by (${blockedBy.length}):`);
        for (const t of blockedBy) {
          const statusIcon = t.status === "closed" ? "[x]" : "[ ]";
          console.log(`  ${statusIcon} ${t.id}: ${t.title}`);
        }
      }

      if (blocking.length > 0) {
        console.log();
        console.log(`Blocking (${blocking.length}):`);
        for (const t of blocking) {
          const statusIcon = t.status === "closed" ? "[x]" : "[ ]";
          console.log(`  ${statusIcon} ${t.id}: ${t.title}`);
        }
      }
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
  stats     Show task statistics (counts by status, type, priority)
  stale     Find tasks not updated in N days
  archive   Archive old closed tasks to tasks-archive.jsonl
  search    Search tasks across active and archived
  validate  Validate tasks.jsonl (schema + conflict markers)
  doctor    Diagnose common tasks.jsonl issues (orphan deps, duplicates, cycles, stale tasks)
  merge     Three-way merge for .openagents/tasks.jsonl (git merge driver helper)

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

validate Options:
  --check-conflicts       Fail if git conflict markers are present (default: also checked during full parse)

doctor Options:
  --days <n>              Stale threshold (default: 14, in-progress tasks only)

merge Options:
  --base <path>           Base/ancestor tasks.jsonl
  --current <path>        Current branch version (usually %A)
  --incoming <path>       Incoming branch version (usually %B)
  --output <path>         Output path (default: overwrite current)

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
  bun src/tasks/cli.ts doctor --json  # diagnose orphan deps, duplicates, cycles, stale tasks
`);
};

const main = async () => {
  const { command, options } = parseArgs(process.argv.slice(2));

  switch (command) {
    case "init":
      try {
        await Effect.runPromise(cmdInit(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdList(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdReady(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdNext(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdCreate(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdUpdate(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdClose(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdReopen(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdShow(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdStats(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdStale(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdArchive(options).pipe(Effect.provide(BunContext.layer)));
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
        await Effect.runPromise(cmdSearch(options).pipe(Effect.provide(BunContext.layer)));
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
          cmdValidate(options).pipe(Effect.provide(BunContext.layer)),
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
          cmdDoctor(options).pipe(Effect.provide(BunContext.layer)),
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
    case "merge":
      try {
        await Effect.runPromise(cmdMerge(options).pipe(Effect.provide(BunContext.layer)));
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
