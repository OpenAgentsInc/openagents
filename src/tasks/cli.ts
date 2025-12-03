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
  // close command options
  id?: string;
  reason?: string;
  commit?: string;
  stage?: boolean;
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

Examples:
  bun src/tasks/cli.ts init --json
  bun src/tasks/cli.ts list --status open --json
  bun src/tasks/cli.ts ready --limit 5 --json
  bun src/tasks/cli.ts next --json
  bun src/tasks/cli.ts create --title "Fix bug" --type bug --priority 1 --json
  bun src/tasks/cli.ts close --id oa-123 --reason "Implemented feature" --json
  bun src/tasks/cli.ts close --id oa-123 --stage --json  # auto-capture HEAD commit and stage
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
