#!/usr/bin/env bun
/**
 * Terminal-Bench Task Importer
 *
 * Imports tasks from the Terminal-Bench 2.0 repository (or Harbor format)
 * into our local suite JSON format for use with tbench-local.ts
 *
 * Usage:
 *   bun src/cli/import-tasks.ts --source ~/code/harbor --output tasks/terminal-bench-2.json
 *   bun src/cli/import-tasks.ts --clone --output tasks/terminal-bench-2.json
 */

import { parseArgs } from "util";
import { join, basename } from "path";
import { existsSync, mkdirSync, writeFileSync, readFileSync, readdirSync, statSync } from "fs";
import { parse as parseToml } from "smol-toml";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface ImportArgs {
  source: string | undefined;
  output: string;
  clone: boolean | undefined;
  registry: string | undefined;
  dataset: string | undefined;
  help: boolean | undefined;
}

const parseCliArgs = (): ImportArgs => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      source: { type: "string", short: "s" },
      output: { type: "string", short: "o" },
      clone: { type: "boolean", short: "c" },
      registry: { type: "string", short: "r" },
      dataset: { type: "string", short: "d" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(`
Terminal-Bench Task Importer

Import tasks from Terminal-Bench repos into local suite format.

Usage:
  bun src/cli/import-tasks.ts --source <dir> --output <file> [options]
  bun src/cli/import-tasks.ts --clone --output <file> [options]

Options:
  -s, --source    Path to cloned terminal-bench repo or harbor examples
  -o, --output    Output JSON file for the suite
  -c, --clone     Clone terminal-bench-2 repo to /tmp and import
  -r, --registry  Path to registry.json (default: <source>/registry.json)
  -d, --dataset   Dataset name to import (default: terminal-bench)
  -h, --help      Show this help message

Examples:
  # Import from local harbor repo
  bun src/cli/import-tasks.ts -s ~/code/harbor -o tasks/hello-world.json -d hello-world

  # Clone and import terminal-bench-2
  bun src/cli/import-tasks.ts --clone -o tasks/terminal-bench-2.json
`);
    process.exit(0);
  }

  if (!values.output) {
    console.error("Error: --output is required");
    process.exit(1);
  }

  if (!values.source && !values.clone) {
    console.error("Error: either --source or --clone is required");
    process.exit(1);
  }

  return {
    source: values.source,
    output: values.output,
    clone: values.clone,
    registry: values.registry,
    dataset: values.dataset,
    help: values.help,
  };
};

// ============================================================================
// Task Parsing
// ============================================================================

interface HarborTaskToml {
  version?: string;
  metadata?: {
    author_name?: string;
    author_email?: string;
    difficulty?: string;
    category?: string;
    tags?: string[];
  };
  verifier?: {
    timeout_sec?: number;
  };
  agent?: {
    timeout_sec?: number;
  };
}

interface ImportedTask {
  id: string;
  name: string;
  description: string;
  difficulty: "easy" | "medium" | "hard" | "expert";
  category: string;
  verification: {
    type: "test" | "output" | "custom";
    command?: string;
    expected?: string;
    script?: string;
  };
  timeout_seconds?: number;
  max_turns?: number;
  tags?: string[];
  setup?: string[];
  source_path?: string;
}

const mapDifficulty = (d: string | undefined): ImportedTask["difficulty"] => {
  switch (d?.toLowerCase()) {
    case "easy":
    case "trivial":
      return "easy";
    case "medium":
    case "moderate":
      return "medium";
    case "hard":
    case "difficult":
      return "hard";
    case "expert":
    case "extreme":
      return "expert";
    default:
      return "medium";
  }
};

const mapCategory = (c: string | undefined): string => {
  // Normalize category names
  const category = c?.toLowerCase() ?? "implementation";
  const categoryMap: Record<string, string> = {
    programming: "implementation",
    coding: "implementation",
    debug: "debugging",
    test: "testing",
    tests: "testing",
    docs: "documentation",
    config: "configuration",
    perf: "optimization",
    performance: "optimization",
  };
  return categoryMap[category] ?? category;
};

/**
 * Parse a single Harbor-format task directory
 */
const parseHarborTask = (taskDir: string): ImportedTask | null => {
  const taskTomlPath = join(taskDir, "task.toml");
  const instructionPath = join(taskDir, "instruction.md");
  const testsDir = join(taskDir, "tests");

  if (!existsSync(taskTomlPath)) {
    console.warn(`Skipping ${taskDir}: no task.toml`);
    return null;
  }

  if (!existsSync(instructionPath)) {
    console.warn(`Skipping ${taskDir}: no instruction.md`);
    return null;
  }

  // Parse task.toml
  let toml: HarborTaskToml;
  try {
    const tomlContent = readFileSync(taskTomlPath, "utf-8");
    toml = parseToml(tomlContent) as HarborTaskToml;
  } catch (e) {
    console.warn(`Skipping ${taskDir}: failed to parse task.toml: ${e}`);
    return null;
  }

  // Read instruction
  const instruction = readFileSync(instructionPath, "utf-8").trim();

  // Determine task ID from directory name
  const taskId = basename(taskDir);

  // Build verification config
  let verification: ImportedTask["verification"] = {
    type: "test",
    command: "pytest tests/",
  };

  // Check for test files
  if (existsSync(testsDir)) {
    const testFiles = readdirSync(testsDir);
    if (testFiles.some((f) => f.endsWith(".py"))) {
      verification = {
        type: "test",
        command: "cd /app && pytest tests/ -v",
      };
    } else if (testFiles.some((f) => f === "test.sh")) {
      verification = {
        type: "custom",
        script: "cd /app && bash tests/test.sh",
      };
    }
  }

  return {
    id: taskId,
    name: taskId.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
    description: instruction,
    difficulty: mapDifficulty(toml.metadata?.difficulty),
    category: mapCategory(toml.metadata?.category),
    verification,
    timeout_seconds: toml.agent?.timeout_sec ?? 300,
    max_turns: 100,
    tags: toml.metadata?.tags ?? [],
    source_path: taskDir,
  };
};

/**
 * Import tasks from a directory containing Harbor-format tasks
 */
const importTasksFromDir = (tasksDir: string): ImportedTask[] => {
  const tasks: ImportedTask[] = [];

  if (!existsSync(tasksDir)) {
    console.error(`Tasks directory not found: ${tasksDir}`);
    return tasks;
  }

  const entries = readdirSync(tasksDir);
  for (const entry of entries) {
    const entryPath = join(tasksDir, entry);
    if (statSync(entryPath).isDirectory()) {
      const task = parseHarborTask(entryPath);
      if (task) {
        tasks.push(task);
      }
    }
  }

  return tasks;
};

/**
 * Clone terminal-bench-2 repo and import tasks
 */
const cloneAndImport = async (): Promise<ImportedTask[]> => {
  const cloneDir = "/tmp/terminal-bench-2";

  if (existsSync(cloneDir)) {
    console.log(`Using existing clone at ${cloneDir}`);
  } else {
    console.log("Cloning terminal-bench-2 repo...");
    const proc = Bun.spawn([
      "git",
      "clone",
      "--depth",
      "1",
      "https://github.com/laude-institute/terminal-bench-2.git",
      cloneDir,
    ]);
    const exitCode = await proc.exited;
    if (exitCode !== 0) {
      console.error("Failed to clone terminal-bench-2 repo");
      return [];
    }
    console.log("Clone complete");
  }

  return importTasksFromDir(cloneDir);
};

/**
 * Import tasks using registry.json
 */
const importFromRegistry = async (
  registryPath: string,
  datasetName: string,
  sourceDir: string
): Promise<ImportedTask[]> => {
  if (!existsSync(registryPath)) {
    console.error(`Registry not found: ${registryPath}`);
    return [];
  }

  const registry = JSON.parse(readFileSync(registryPath, "utf-8"));
  const dataset = registry.find((d: any) => d.name === datasetName);

  if (!dataset) {
    console.error(`Dataset '${datasetName}' not found in registry`);
    console.log("Available datasets:", registry.map((d: any) => d.name).join(", "));
    return [];
  }

  console.log(`Importing ${dataset.tasks.length} tasks from ${datasetName}`);

  const tasks: ImportedTask[] = [];
  for (const taskRef of dataset.tasks) {
    // Check if task is local (in examples) or remote (needs clone)
    const localPath = join(sourceDir, taskRef.path);
    if (existsSync(localPath)) {
      const task = parseHarborTask(localPath);
      if (task) {
        tasks.push(task);
      }
    } else if (taskRef.git_url) {
      console.log(`Task ${taskRef.name} requires clone from ${taskRef.git_url}`);
      // For remote tasks, we'd need to clone the repo
      // For now, skip and suggest using --clone
    }
  }

  return tasks;
};

// ============================================================================
// Main
// ============================================================================

const main = async (): Promise<void> => {
  const args = parseCliArgs();

  let tasks: ImportedTask[] = [];

  if (args.clone) {
    tasks = await cloneAndImport();
  } else if (args.source) {
    const registryPath = args.registry ?? join(args.source, "registry.json");
    const datasetName = args.dataset ?? "terminal-bench";

    if (existsSync(registryPath)) {
      tasks = await importFromRegistry(registryPath, datasetName, args.source);
    } else {
      // Direct import from examples/tasks
      const examplesDir = join(args.source, "examples", "tasks");
      if (existsSync(examplesDir)) {
        tasks = importTasksFromDir(examplesDir);
      } else {
        tasks = importTasksFromDir(args.source);
      }
    }
  }

  if (tasks.length === 0) {
    console.error("No tasks imported");
    process.exit(1);
  }

  // Build suite
  const suite = {
    name: args.dataset ?? "Terminal-Bench 2.0",
    version: "2.0.0",
    description: `Imported ${tasks.length} tasks from Terminal-Bench`,
    source_repo: args.source ?? "/tmp/terminal-bench-2",
    tasks: tasks.map((t) => ({
      id: t.id,
      name: t.name,
      description: t.description,
      difficulty: t.difficulty,
      category: t.category,
      verification: t.verification,
      timeout_seconds: t.timeout_seconds,
      max_turns: t.max_turns,
      tags: t.tags,
      source_path: t.source_path,
    })),
  };

  // Ensure output directory exists
  const outputDir = join(args.output, "..");
  if (!existsSync(outputDir)) {
    mkdirSync(outputDir, { recursive: true });
  }

  writeFileSync(args.output, JSON.stringify(suite, null, 2));
  console.log(`\nImported ${tasks.length} tasks to ${args.output}`);

  // Print summary by difficulty
  const byDifficulty = tasks.reduce(
    (acc, t) => {
      acc[t.difficulty] = (acc[t.difficulty] ?? 0) + 1;
      return acc;
    },
    {} as Record<string, number>
  );
  console.log("\nBy difficulty:");
  for (const [diff, count] of Object.entries(byDifficulty)) {
    console.log(`  ${diff}: ${count}`);
  }

  // Print summary by category
  const byCategory = tasks.reduce(
    (acc, t) => {
      acc[t.category] = (acc[t.category] ?? 0) + 1;
      return acc;
    },
    {} as Record<string, number>
  );
  console.log("\nBy category:");
  for (const [cat, count] of Object.entries(byCategory)) {
    console.log(`  ${cat}: ${count}`);
  }
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
