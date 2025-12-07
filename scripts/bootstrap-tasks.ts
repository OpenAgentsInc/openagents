#!/usr/bin/env bun
/**
 * Bootstrap script to initialize .openagents and create initial tasks
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { initOpenAgentsProject, createTask, type TaskCreate } from "../src/tasks/index.js";
import { DatabaseLive } from "../src/storage/database.js";

const OPENAGENTS_ROOT = "/Users/christopherdavid/code/openagents";

const initialTasks: TaskCreate[] = [
  {
    title: "Implement tasks CLI (tasks:init, tasks:list, tasks:ready)",
    description: `Create a CLI wrapper around TaskService/ProjectService for external agents.

Commands to implement:
- tasks:init - Initialize .openagents for a repo
- tasks:list - List tasks with filters, output JSON
- tasks:ready - List ready tasks (no open blockers)

The CLI should accept flags and output JSON for machine parsing.
See docs/mechacoder/TASK-SPEC.md for full spec.`,
    type: "task",
    priority: 1,
    status: "open",
    labels: ["cli", "task-system"],
    deps: [],
    comments: [],
  },
  {
    title: "Implement tasks CLI (tasks:next, tasks:create, tasks:update)",
    description: `Continue CLI implementation with remaining commands:

- tasks:next - Atomically pick next ready task, mark in_progress, return it
- tasks:create - Create new task from flags or JSON stdin
- tasks:update - Update/close task from JSON stdin

These commands enable external agents (Claude Code, Codex) to interact with the task system.
See docs/mechacoder/TASK-SPEC.md for full spec.`,
    type: "task",
    priority: 1,
    status: "open",
    labels: ["cli", "task-system"],
    deps: [],
    comments: [],
  },
  {
    title: "Add package.json scripts for tasks CLI",
    description: `Add npm scripts to package.json for the tasks CLI:

\`\`\`json
{
  "scripts": {
    "tasks:init": "bun src/tasks/cli.ts init",
    "tasks:list": "bun src/tasks/cli.ts list",
    "tasks:ready": "bun src/tasks/cli.ts ready",
    "tasks:next": "bun src/tasks/cli.ts next",
    "tasks:create": "bun src/tasks/cli.ts create",
    "tasks:update": "bun src/tasks/cli.ts update"
  }
}
\`\`\`

This enables external agents to use \`bun run tasks:*\` commands.`,
    type: "task",
    priority: 2,
    status: "open",
    labels: ["cli", "task-system"],
    deps: [],
    comments: [],
  },
  {
    title: "Document task CLI usage in AGENTS.md",
    description: `Add a section to AGENTS.md explaining how to use the tasks CLI:

- When to use CLI vs TaskService directly
- Example commands for external agents
- JSON input/output formats
- Link to TASK-SPEC.md for full details`,
    type: "task",
    priority: 2,
    status: "open",
    labels: ["docs", "task-system"],
    deps: [],
    comments: [],
  },
];

const bootstrap = Effect.gen(function* () {
  console.log("Initializing .openagents...");
  
  const result = yield* initOpenAgentsProject({
    rootDir: OPENAGENTS_ROOT,
    projectId: "openagents",
    allowExisting: true,
  });
  
  console.log(`Created project: ${result.projectId}`);
  console.log(`Project path: ${result.projectPath}`);
  console.log(`Database path: ${result.dbPath}`);

  console.log("\nCreating initial tasks...");

  for (const taskData of initialTasks) {
    const task = yield* createTask({
      task: taskData,
      idPrefix: "oa",
    });
    console.log(`Created: ${task.id} - ${task.title.slice(0, 50)}...`);
  }

  console.log("\nBootstrap complete!");
});

Effect.runPromise(
  bootstrap.pipe(
    Effect.provide(DatabaseLive),
    Effect.provide(BunContext.layer)
  )
)
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("Bootstrap failed:", err);
    process.exit(1);
  });
