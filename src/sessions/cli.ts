#!/usr/bin/env bun
/**
 * Session CLI - Query and view agent sessions
 *
 * Usage:
 *   bun run session:list                  # List recent sessions
 *   bun run session:show <id>             # Pretty-print session
 *   bun run session:search <term>         # Search in sessions
 *   bun run session:by-task <task-id>     # Find sessions for task
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { parseArgs } from "util";
import { DEFAULT_SESSIONS_DIR, makeSessionService } from "./service.js";
import type { SessionEntry, UsageMetrics } from "./schema.js";
import { extractText } from "./schema.js";

const formatTimestamp = (ts: string): string => {
  const date = new Date(ts);
  return date.toLocaleString();
};

const formatDuration = (startTs: string, endTs?: string): string => {
  const start = new Date(startTs).getTime();
  const end = endTs ? new Date(endTs).getTime() : Date.now();
  const durationMs = end - start;
  const minutes = Math.floor(durationMs / 60000);
  const seconds = Math.floor((durationMs % 60000) / 1000);
  return `${minutes}m ${seconds}s`;
};

const formatUsage = (usage?: UsageMetrics): string => {
  if (!usage) return "N/A";
  const parts: string[] = [];
  if (usage.inputTokens) parts.push(`in: ${usage.inputTokens.toLocaleString()}`);
  if (usage.outputTokens) parts.push(`out: ${usage.outputTokens.toLocaleString()}`);
  if (usage.totalCostUsd) parts.push(`$${usage.totalCostUsd.toFixed(4)}`);
  return parts.length > 0 ? parts.join(", ") : "N/A";
};

const truncate = (str: string, maxLen: number): string => {
  const oneLine = str.replace(/\n/g, " ").trim();
  return oneLine.length > maxLen ? oneLine.slice(0, maxLen - 3) + "..." : oneLine;
};

const colorize = {
  dim: (s: string) => `\x1b[2m${s}\x1b[0m`,
  bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
  green: (s: string) => `\x1b[32m${s}\x1b[0m`,
  red: (s: string) => `\x1b[31m${s}\x1b[0m`,
  yellow: (s: string) => `\x1b[33m${s}\x1b[0m`,
  cyan: (s: string) => `\x1b[36m${s}\x1b[0m`,
  blue: (s: string) => `\x1b[34m${s}\x1b[0m`,
};

const outcomeColor = (outcome?: string): string => {
  switch (outcome) {
    case "success":
      return colorize.green(outcome);
    case "failure":
      return colorize.red(outcome);
    case "blocked":
      return colorize.yellow(outcome);
    case "cancelled":
      return colorize.dim(outcome);
    default:
      return colorize.dim("in_progress");
  }
};

const printHelp = () => {
  console.log(`
Session CLI - Query and view agent sessions

Usage:
  bun src/sessions/cli.ts list [--json] [--limit N]
  bun src/sessions/cli.ts show <session-id> [--json]
  bun src/sessions/cli.ts search <term> [--json]
  bun src/sessions/cli.ts by-task <task-id> [--json]

Commands:
  list              List recent sessions
  show <id>         Pretty-print session content
  search <term>     Search sessions by text content
  by-task <task-id> Find all sessions for a specific task

Options:
  --json            Output in JSON format
  --limit N         Limit results (default: 20)
  --dir <path>      Sessions directory (default: .openagents/sessions)
  --help            Show this help

Examples:
  bun src/sessions/cli.ts list
  bun src/sessions/cli.ts show session-2025-12-03T10-00-00-000Z-abc123
  bun src/sessions/cli.ts search "error"
  bun src/sessions/cli.ts by-task oa-abc123
`);
};

const runCli = async () => {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    options: {
      json: { type: "boolean", default: false },
      limit: { type: "string", default: "20" },
      dir: { type: "string", default: DEFAULT_SESSIONS_DIR },
      help: { type: "boolean", default: false },
    },
    allowPositionals: true,
  });

  if (values.help || positionals.length === 0) {
    printHelp();
    process.exit(0);
  }

  const command = positionals[0];
  const sessionsDir = values.dir as string;
  const limit = parseInt(values.limit as string, 10);
  const jsonOutput = values.json as boolean;

  const program = Effect.gen(function* () {
    const service = yield* makeSessionService({ sessionsDir });

    switch (command) {
      case "list": {
        const sessionIds = yield* service.listSessions();
        const limitedIds = sessionIds.slice(0, limit);
        const metadatas = [];

        for (const sessionId of limitedIds) {
          const metadata = yield* service.getSessionMetadata(sessionId).pipe(
            Effect.catchAll(() => Effect.succeed(null)),
          );
          if (metadata) metadatas.push(metadata);
        }

        if (jsonOutput) {
          console.log(JSON.stringify(metadatas, null, 2));
        } else {
          console.log(colorize.bold(`\nSessions (${metadatas.length} of ${sessionIds.length}):\n`));
          for (const m of metadatas) {
            const duration = formatDuration(m.startedAt, m.endedAt);
            const taskLabel = m.taskId ? colorize.cyan(`[${m.taskId}]`) : "";
            const preview = m.firstUserMessage ? truncate(m.firstUserMessage, 60) : "(no messages)";
            console.log(
              `${colorize.dim(formatTimestamp(m.startedAt))} ${outcomeColor(m.outcome)} ${taskLabel}`,
            );
            console.log(`  ${colorize.bold(m.sessionId)}`);
            console.log(`  ${colorize.dim(preview)}`);
            console.log(`  ${colorize.dim(`${m.totalTurns} turns, ${duration}, ${formatUsage(m.totalUsage)}`)}`);
            console.log();
          }
        }
        break;
      }

      case "show": {
        const sessionId = positionals[1];
        if (!sessionId) {
          console.error("Error: session ID required");
          process.exit(1);
        }

        const entries = yield* service.loadSession(sessionId);

        if (jsonOutput) {
          console.log(JSON.stringify(entries, null, 2));
        } else {
          console.log(colorize.bold(`\nSession: ${sessionId}\n`));
          for (const entry of entries) {
            printEntry(entry);
          }
        }
        break;
      }

      case "search": {
        const term = positionals[1];
        if (!term) {
          console.error("Error: search term required");
          process.exit(1);
        }

        const results = yield* service.searchSessions(term);
        const limitedResults = results.slice(0, limit);

        if (jsonOutput) {
          console.log(JSON.stringify(limitedResults, null, 2));
        } else {
          console.log(colorize.bold(`\nSearch results for "${term}" (${limitedResults.length}):\n`));
          for (const m of limitedResults) {
            const taskLabel = m.taskId ? colorize.cyan(`[${m.taskId}]`) : "";
            console.log(`${colorize.dim(formatTimestamp(m.startedAt))} ${outcomeColor(m.outcome)} ${taskLabel}`);
            console.log(`  ${colorize.bold(m.sessionId)}`);
            console.log();
          }
        }
        break;
      }

      case "by-task": {
        const taskId = positionals[1];
        if (!taskId) {
          console.error("Error: task ID required");
          process.exit(1);
        }

        const results = yield* service.findSessionsByTask(taskId);

        if (jsonOutput) {
          console.log(JSON.stringify(results, null, 2));
        } else {
          console.log(colorize.bold(`\nSessions for task ${colorize.cyan(taskId)} (${results.length}):\n`));
          for (const m of results) {
            const duration = formatDuration(m.startedAt, m.endedAt);
            console.log(`${colorize.dim(formatTimestamp(m.startedAt))} ${outcomeColor(m.outcome)}`);
            console.log(`  ${colorize.bold(m.sessionId)}`);
            console.log(`  ${colorize.dim(`${m.totalTurns} turns, ${duration}, ${formatUsage(m.totalUsage)}`)}`);
            console.log();
          }
        }
        break;
      }

      default:
        console.error(`Unknown command: ${command}`);
        printHelp();
        process.exit(1);
    }
  });

  await Effect.runPromise(program.pipe(Effect.provide(BunContext.layer))).catch((e) => {
    console.error("Error:", e.message || e);
    process.exit(1);
  });
};

const printEntry = (entry: SessionEntry) => {
  const ts = colorize.dim(formatTimestamp(entry.timestamp));

  switch (entry.type) {
    case "session_start": {
      console.log(`${ts} ${colorize.bold("SESSION START")}`);
      if (entry.taskId) console.log(`  Task: ${colorize.cyan(entry.taskId)}`);
      if (entry.model) console.log(`  Model: ${entry.model}`);
      console.log(`  CWD: ${entry.cwd}`);
      console.log();
      break;
    }

    case "user": {
      const text = extractText(entry.message.content);
      console.log(`${ts} ${colorize.blue("USER")}${entry.userType ? ` (${entry.userType})` : ""}`);
      console.log(`  ${truncate(text, 200)}`);
      console.log();
      break;
    }

    case "assistant": {
      const content = entry.message.content;
      console.log(`${ts} ${colorize.green("ASSISTANT")}`);

      if (typeof content === "string") {
        console.log(`  ${truncate(content, 200)}`);
      } else if (Array.isArray(content)) {
        for (const block of content) {
          if (typeof block === "object" && block !== null && "type" in block) {
            if (block.type === "text") {
              console.log(`  ${truncate((block as { text: string }).text, 200)}`);
            } else if (block.type === "tool_use") {
              const toolUse = block as { name: string; id: string };
              console.log(`  ${colorize.yellow(`[tool_use: ${toolUse.name}]`)} ${colorize.dim(toolUse.id)}`);
            }
          }
        }
      }

      if (entry.usage) {
        console.log(`  ${colorize.dim(formatUsage(entry.usage))}`);
      }
      console.log();
      break;
    }

    case "tool_result": {
      const toolResult = entry.message.content[0];
      if (toolResult) {
        const resultPreview =
          typeof toolResult.content === "string"
            ? truncate(toolResult.content, 100)
            : truncate(JSON.stringify(toolResult.content), 100);
        const status = toolResult.is_error ? colorize.red("ERROR") : colorize.green("OK");
        console.log(`${ts} ${colorize.yellow("TOOL_RESULT")} ${status}`);
        console.log(`  ${colorize.dim(toolResult.tool_use_id)}`);
        console.log(`  ${resultPreview}`);
        console.log();
      }
      break;
    }

    case "session_end": {
      console.log(`${ts} ${colorize.bold("SESSION END")}`);
      console.log(`  Outcome: ${outcomeColor(entry.outcome)}`);
      if (entry.reason) console.log(`  Reason: ${entry.reason}`);
      console.log(`  Turns: ${entry.totalTurns}`);
      if (entry.totalUsage) console.log(`  Usage: ${formatUsage(entry.totalUsage)}`);
      if (entry.filesModified?.length)
        console.log(`  Files modified: ${entry.filesModified.length}`);
      if (entry.commits?.length) console.log(`  Commits: ${entry.commits.join(", ")}`);
      console.log();
      break;
    }
  }
};

runCli();
