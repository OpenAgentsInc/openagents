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
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import * as nodePath from "node:path";
import {
  initOpenAgentsProject,
  readyTasks,
  pickNextTask,
  createTask,
  updateTask,
  closeTask,
  listTasks,
  type TaskCreate,
  type TaskUpdate,
  type TaskFilter,
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

Examples:
  bun src/tasks/cli.ts init --json
  bun src/tasks/cli.ts list --status open --json
  bun src/tasks/cli.ts ready --limit 5 --json
  bun src/tasks/cli.ts next --json
  bun src/tasks/cli.ts create --title "Fix bug" --type bug --priority 1 --json
  echo '{"id":"oa-123","status":"closed","reason":"Done"}' | bun src/tasks/cli.ts update --json-input --json
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
