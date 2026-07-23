/**
 * Live `codex exec --json` spawner for the Codex harness adapter.
 *
 * Runs the real codex binary and parses its JSONL stdout into the neutral
 * {@link CodexEvent} vocabulary the adapter consumes. In owner-local mode
 * (`codexHome` undefined) the child environment's CODEX_HOME is left unset
 * so the codex runtime uses the developer's currently-authenticated default
 * home; this module never runs a login flow. Wire shapes follow the
 * observed codex-cli 0.145 exec output.
 */

import { spawn } from "node:child_process";
import { Effect } from "effect";
import type {
  CodexEvent,
  CodexExecSpawner,
  CodexThreadItem,
  CodexTokenUsage,
} from "./codex-adapter.ts";
import { CodexTransportError } from "./codex-adapter.ts";

export interface LiveCodexExecSpawnerOptions {
  /** Sandbox policy for model-generated commands. Default `read-only`. */
  readonly sandbox?: "read-only" | "workspace-write";
  /** Reasoning effort config override (`model_reasoning_effort`). */
  readonly reasoningEffort?: "low" | "medium" | "high";
  /** Turn wall-clock timeout in milliseconds. Default 600000. */
  readonly timeoutMs?: number;
  /** Extra `-c key=value` config overrides, appended verbatim. */
  readonly configOverrides?: readonly string[];
}

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string | null => (typeof value === "string" ? value : null);

const asNumber = (value: unknown): number => (typeof value === "number" ? value : 0);

const itemStatus = (value: unknown): "in_progress" | "completed" | "failed" =>
  value === "in_progress" || value === "failed" ? value : "completed";

/** Map one exec-wire thread item (snake_case `item.type`) to the neutral shape. */
const projectExecItem = (item: JsonRecord): CodexThreadItem | null => {
  const id = asString(item.id) ?? "item";
  switch (item.type) {
    case "agent_message":
      return { itemType: "agent_message", id, text: asString(item.text) ?? "" };
    case "reasoning":
      return { itemType: "reasoning", id, text: asString(item.text) ?? "" };
    case "command_execution": {
      const exitCode = item.exit_code;
      return {
        itemType: "command_execution",
        id,
        commandDisplay: asString(item.command) ?? "command",
        status: itemStatus(item.status),
        ...(typeof exitCode === "number" ? { exitCode } : {}),
      };
    }
    case "file_change": {
      const changes: Array<{ readonly path: string; readonly kind: "add" | "delete" | "update" }> =
        Array.isArray(item.changes)
          ? item.changes.flatMap((change) => {
              const record = asRecord(change);
              const path = asString(record?.path);
              const kind = record?.kind;
              if (path === null) return [];
              const boundedKind: "add" | "delete" | "update" =
                kind === "add" ? "add" : kind === "delete" ? "delete" : "update";
              return [{ path, kind: boundedKind }];
            })
          : [];
      return { itemType: "file_change", id, status: itemStatus(item.status), changes };
    }
    case "mcp_tool_call":
      return {
        itemType: "mcp_tool_call",
        id,
        serverName: asString(item.server) ?? "server",
        toolName: asString(item.tool) ?? "tool",
        status: itemStatus(item.status),
      };
    case "web_search":
      return { itemType: "web_search", id, status: itemStatus(item.status) };
    default:
      return null;
  }
};

const projectUsage = (usage: JsonRecord | null): CodexTokenUsage | undefined =>
  usage === null
    ? undefined
    : {
        inputTokens: asNumber(usage.input_tokens),
        cachedInputTokens: asNumber(usage.cached_input_tokens),
        outputTokens: asNumber(usage.output_tokens),
        reasoningOutputTokens: asNumber(usage.reasoning_output_tokens),
      };

/** Parse one exec `--json` stdout line into a {@link CodexEvent}. */
export const parseLiveCodexExecLine = (line: string): CodexEvent | null => {
  const trimmed = line.trim();
  if (trimmed === "") return null;
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    return null;
  }
  const record = asRecord(value);
  if (record === null) return null;
  switch (record.type) {
    case "thread.started": {
      const threadId = asString(record.thread_id);
      return threadId === null ? null : { type: "thread.started", threadId };
    }
    case "turn.started":
      return { type: "turn.started" };
    case "item.completed": {
      const item = asRecord(record.item);
      if (item === null) return null;
      const projected = projectExecItem(item);
      return projected === null ? null : { type: "item.completed", item: projected };
    }
    case "turn.completed": {
      const usage = projectUsage(asRecord(record.usage));
      return {
        type: "turn.completed",
        status: "completed",
        ...(usage === undefined ? {} : { usage }),
      };
    }
    case "turn.failed": {
      const error = asRecord(record.error);
      return { type: "turn.failed", messageSafe: asString(error?.message) ?? "turn failed" };
    }
    case "error":
      return { type: "error", messageSafe: asString(record.message) ?? "error" };
    default:
      return null;
  }
};

const classifySpawnFailure = (message: string): string => {
  const lowered = message.toLowerCase();
  if (
    lowered.includes("token could not be refreshed") ||
    lowered.includes("refresh token was revoked") ||
    lowered.includes("sign in again")
  ) {
    return "account_auth_failed";
  }
  if (lowered.includes("usage limit") || lowered.includes("quota")) return "account_exhausted";
  if (lowered.includes("rate limit") || lowered.includes("429")) return "account_rate_limited";
  return "spawn_failed";
};

/**
 * Build a live {@link CodexExecSpawner} over the real codex binary.
 *
 * The spawner is fail-typed: a process that cannot start or exits without a
 * parseable event stream fails with {@link CodexTransportError}; a turn that
 * fails inside Codex still resolves with its events (the adapter projects
 * the typed failure). Resume re-drives through `codex exec resume`.
 */
export const makeLiveCodexExecSpawner = (
  options: LiveCodexExecSpawnerOptions = {},
): CodexExecSpawner => ({
  spawn: (params) =>
    Effect.callback<ReadonlyArray<CodexEvent>, CodexTransportError>((resume) => {
      // exec-level flags come BEFORE the `resume` subcommand; the prompt is
      // always the final positional.
      const args: string[] = ["exec", "--json", "--skip-git-repo-check"];
      if (params.model !== undefined) args.push("-m", params.model);
      if (options.reasoningEffort !== undefined) {
        args.push("-c", `model_reasoning_effort="${options.reasoningEffort}"`);
      }
      for (const override of options.configOverrides ?? []) args.push("-c", override);
      args.push("-s", options.sandbox ?? "read-only");
      if (params.workingDirectory !== undefined) args.push("--cd", params.workingDirectory);
      if (params.resumeThreadId !== undefined) args.push("resume", params.resumeThreadId);
      args.push(params.prompt);

      const env = { ...process.env };
      if (params.codexHome !== undefined) env.CODEX_HOME = params.codexHome;
      else delete env.CODEX_HOME;

      const child = spawn(params.codexBinaryPath, args, {
        stdio: ["ignore", "pipe", "pipe"],
        env,
      });
      let stdout = "";
      let stderr = "";
      let settled = false;
      const timeoutMs = options.timeoutMs ?? 600_000;
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        child.kill("SIGKILL");
        resume(
          Effect.fail(
            new CodexTransportError({
              failureClass: "turn_timeout",
              detail: `codex exec exceeded ${timeoutMs}ms`,
            }),
          ),
        );
      }, timeoutMs);

      child.stdout.on("data", (chunk: Buffer) => {
        stdout += chunk.toString("utf8");
      });
      child.stderr.on("data", (chunk: Buffer) => {
        stderr += chunk.toString("utf8");
      });
      child.on("error", (error) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resume(
          Effect.fail(
            new CodexTransportError({
              failureClass: "spawn_failed",
              detail: String(error),
            }),
          ),
        );
      });
      child.on("close", () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        const events: CodexEvent[] = [];
        for (const line of stdout.split("\n")) {
          const event = parseLiveCodexExecLine(line);
          if (event !== null) events.push(event);
        }
        if (events.length === 0) {
          const detail = stderr.trim().slice(0, 500) || "no parseable exec events";
          resume(
            Effect.fail(
              new CodexTransportError({
                failureClass: classifySpawnFailure(detail),
                detail,
              }),
            ),
          );
          return;
        }
        resume(Effect.succeed(events));
      });
    }),
});
