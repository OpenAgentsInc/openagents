#!/usr/bin/env bun
/**
 * Orchestrator CLI
 *
 * Commands for managing parallel agent execution with git worktrees.
 *
 * Usage:
 *   bun src/agent/orchestrator/cli.ts worktrees list
 *   bun src/agent/orchestrator/cli.ts worktrees prune [--max-age <ms>]
 *   bun src/agent/orchestrator/cli.ts locks list
 *   bun src/agent/orchestrator/cli.ts locks prune
 *   bun src/agent/orchestrator/cli.ts status
 *
 * @see docs/claude/plans/containers-impl-v2.md
 */
import { Effect } from "effect";
import * as path from "node:path";
import {
  listWorktrees,
  pruneStaleWorktrees,
  getWorktreesDir,
} from "./worktree.js";
import {
  listWorktreeLocks,
  pruneWorktreeLocks,
  getLocksDir,
} from "./agent-lock.js";

const REPO_PATH = process.cwd();
const OPENAGENTS_DIR = path.join(REPO_PATH, ".openagents");

// ─────────────────────────────────────────────────────────────────────────────
// Command Handlers
// ─────────────────────────────────────────────────────────────────────────────

async function worktreesList() {
  console.log("\n📂 Worktrees\n");

  const worktrees = await Effect.runPromise(listWorktrees(REPO_PATH));

  if (worktrees.length === 0) {
    console.log("  No active worktrees found.");
    console.log(`  Directory: ${getWorktreesDir(REPO_PATH)}\n`);
    return;
  }

  for (const wt of worktrees) {
    console.log(`  ${wt.taskId}`);
    console.log(`    Branch: ${wt.branch}`);
    console.log(`    Path:   ${wt.path}`);
    console.log();
  }

  console.log(`  Total: ${worktrees.length} worktree(s)\n`);
}

async function worktreesPrune(maxAgeMs: number) {
  console.log("\n🧹 Pruning Stale Worktrees\n");
  console.log(`  Max age: ${maxAgeMs}ms (${(maxAgeMs / 1000 / 60).toFixed(1)} minutes)`);

  const pruned = await Effect.runPromise(pruneStaleWorktrees(REPO_PATH, maxAgeMs));

  console.log(`  Pruned:  ${pruned} worktree(s)\n`);
}

function locksList() {
  console.log("\n🔒 Active Worktree Locks\n");

  const locks = listWorktreeLocks(OPENAGENTS_DIR);

  if (locks.length === 0) {
    console.log("  No active locks found.");
    console.log(`  Directory: ${getLocksDir(OPENAGENTS_DIR)}\n`);
    return;
  }

  for (const lock of locks) {
    console.log(`  ${lock.worktreeId}`);
    console.log(`    PID:       ${lock.pid}`);
    console.log(`    Session:   ${lock.sessionId}`);
    console.log(`    Created:   ${lock.createdAt}`);
    console.log();
  }

  console.log(`  Total: ${locks.length} active lock(s)\n`);
}

function locksPrune() {
  console.log("\n🧹 Pruning Stale Locks\n");

  const pruned = pruneWorktreeLocks(OPENAGENTS_DIR);

  console.log(`  Pruned: ${pruned} stale lock(s)\n`);
}

async function showStatus() {
  console.log("\n📊 Parallel Execution Status\n");

  // Worktrees
  const worktrees = await Effect.runPromise(listWorktrees(REPO_PATH));
  console.log(`  Worktrees: ${worktrees.length}`);

  // Locks
  const locks = listWorktreeLocks(OPENAGENTS_DIR);
  console.log(`  Locks:     ${locks.length}`);

  // Paths
  console.log();
  console.log(`  Worktrees dir: ${getWorktreesDir(REPO_PATH)}`);
  console.log(`  Locks dir:     ${getLocksDir(OPENAGENTS_DIR)}`);
  console.log();
}

function showHelp() {
  console.log(`
Orchestrator CLI - Parallel Agent Execution

Usage:
  bun src/agent/orchestrator/cli.ts <command> [options]

Commands:
  worktrees list              List all active worktrees
  worktrees prune [--max-age] Prune stale worktrees (default: 1 hour)
  locks list                  List all active worktree locks
  locks prune                 Remove stale locks (dead PIDs)
  status                      Show overall status
  help                        Show this help message

Examples:
  bun src/agent/orchestrator/cli.ts worktrees list
  bun src/agent/orchestrator/cli.ts worktrees prune --max-age 3600000
  bun src/agent/orchestrator/cli.ts locks list
  bun src/agent/orchestrator/cli.ts status
`);
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const command = args[0];
  const subcommand = args[1];

  try {
    switch (command) {
      case "worktrees":
        switch (subcommand) {
          case "list":
            await worktreesList();
            break;
          case "prune": {
            const maxAgeIndex = args.indexOf("--max-age");
            const maxAgeMs =
              maxAgeIndex !== -1 && args[maxAgeIndex + 1]
                ? parseInt(args[maxAgeIndex + 1], 10)
                : 3600000; // 1 hour default
            await worktreesPrune(maxAgeMs);
            break;
          }
          default:
            console.error(`Unknown subcommand: worktrees ${subcommand}`);
            showHelp();
            process.exit(1);
        }
        break;

      case "locks":
        switch (subcommand) {
          case "list":
            locksList();
            break;
          case "prune":
            locksPrune();
            break;
          default:
            console.error(`Unknown subcommand: locks ${subcommand}`);
            showHelp();
            process.exit(1);
        }
        break;

      case "status":
        await showStatus();
        break;

      case "help":
      case "--help":
      case "-h":
        showHelp();
        break;

      default:
        if (command) {
          console.error(`Unknown command: ${command}`);
        }
        showHelp();
        process.exit(command ? 1 : 0);
    }
  } catch (error: any) {
    console.error(`\n❌ Error: ${error.message ?? error}\n`);
    process.exit(1);
  }
}

main();
