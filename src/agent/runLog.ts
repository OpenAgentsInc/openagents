import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as fs from "node:fs";
import * as nodePath from "node:path";
import { RUN_LOG_TRIM_CONFIG, maybeTrimFileSync } from "./log-trim.js";

// Run event types for streaming JSONL
export type TaskRunEvent =
  | { type: "run_start"; ts: string; runId: string; taskId: string | null; workDir?: string; model?: string }
  | { type: "task_selected"; ts: string; taskId: string; title: string }
  | { type: "turn_start"; ts: string; turn: number }
  | { type: "llm_request"; ts: string; turn: number; messages: unknown; toolNames: string[] }
  | { type: "llm_response"; ts: string; turn: number; hasToolCalls: boolean; message: unknown; toolCalls: Array<{ id: string; name: string; arguments: string }> }
  | { type: "tool_call"; ts: string; tool: string; toolCallId: string; args: unknown }
  | { type: "tool_result"; ts: string; tool: string; toolCallId: string; ok: boolean; result: unknown }
  | { type: "edit_detected"; ts: string; tool: string }
  | { type: "verify_start"; ts: string; command: string }
  | { type: "verify_ok"; ts: string }
  | { type: "verify_fail"; ts: string; stderr: string }
  | { type: "retry_prompt"; ts: string; reason: string }
  | { type: "commit_pushed"; ts: string; commit: string }
  | { type: "task_closed"; ts: string; taskId: string }
  | { type: "run_end"; ts: string; status: string; finalMessage: string; error: string | null }
  | { type: "timeout"; ts: string; reason: string }
  | { type: "log_trimmed"; ts: string; dropped: number; kept: number; reason: string };

export interface TaskRunMetadata {
  id: string;
  taskId: string | null;
  taskTitle: string | null;
  status: "success" | "incomplete" | "failed" | "no_tasks";
  startedAt: string;
  finishedAt: string;
  workDir: string;
  logFilePath: string | null;
  sessionFilePath: string | null;
  commits: string[];
  totalTurns: number;
  finalMessage: string;
  error: string | null;
}

export class RunLogError extends Error {
  readonly _tag = "RunLogError";
  constructor(
    readonly reason: "write_error" | "dir_error",
    message: string,
  ) {
    super(message);
    this.name = "RunLogError";
  }
}

const getDatePath = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}${month}${day}`;
};

const getTimestamp = () => {
  const now = new Date();
  const hours = String(now.getHours()).padStart(2, "0");
  const mins = String(now.getMinutes()).padStart(2, "0");
  const secs = String(now.getSeconds()).padStart(2, "0");
  return `${hours}${mins}${secs}`;
};

const SECRET_KEY_REGEX = /(api[_-]?key|token|secret|password|authorization)/i;
const isLikelySecretString = (value: string): boolean => {
  // Long, no whitespace tokens (e.g., keys) or contains common secret markers
  if (SECRET_KEY_REGEX.test(value)) return true;
  if (value.length >= 40 && !/\s/.test(value)) return true;
  return false;
};

const sanitizeValue = (value: unknown): unknown => {
  if (typeof value === "string") {
    if (isLikelySecretString(value)) return "[redacted]";
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((v) => sanitizeValue(v));
  }
  if (value && typeof value === "object") {
    const result: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) {
      if (SECRET_KEY_REGEX.test(k)) {
        result[k] = "[redacted]";
      } else {
        result[k] = sanitizeValue(v);
      }
    }
    return result;
  }
  return value;
};

export const sanitizeEvent = (event: TaskRunEvent): TaskRunEvent => {
  if ("messages" in event) {
    return { ...event, messages: sanitizeValue(event.messages) };
  }
  if ("message" in event) {
    return { ...event, message: sanitizeValue(event.message), toolCalls: sanitizeValue(event.toolCalls) as any };
  }
  if (event.type === "tool_call") {
    return { ...event, args: sanitizeValue(event.args) };
  }
  if (event.type === "tool_result") {
    return { ...event, result: sanitizeValue(event.result) };
  }
  return event;
};

export const writeRunLog = (
  runLogDir: string,
  metadata: TaskRunMetadata,
): Effect.Effect<string, RunLogError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    const datePath = getDatePath();
    const timestamp = getTimestamp();
    const taskSuffix = metadata.taskId ?? "no-task";
    const fileName = `${timestamp}-${taskSuffix}.json`;
    const dayDir = path.join(runLogDir, datePath);
    const filePath = path.join(dayDir, fileName);

    // Ensure directory exists
    yield* fs.makeDirectory(dayDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) => new RunLogError("dir_error", `Failed to create run-logs directory: ${e.message}`),
      ),
    );

    // Write metadata
    const content = JSON.stringify(metadata, null, 2) + "\n";
    yield* fs.writeFile(filePath, new TextEncoder().encode(content)).pipe(
      Effect.mapError(
        (e) => new RunLogError("write_error", `Failed to write run log: ${e.message}`),
      ),
    );

    return filePath;
  });

export const determineRunStatus = (
  finalMessage: string,
  taskId: string | null,
): TaskRunMetadata["status"] => {
  if (!taskId) return "no_tasks";
  if (finalMessage.includes("TASK_COMPLETED")) return "success";
  if (finalMessage.toLowerCase().includes("error")) return "failed";
  return "incomplete";
};

// Append a run event to JSONL file SYNCHRONOUSLY for immediate flush
// This is intentionally synchronous so `tail -f` sees events immediately
export function appendRunEventSync(
  runLogDir: string,
  runId: string,
  event: TaskRunEvent,
): void {
  const datePath = getDatePath();
  const dayDir = nodePath.join(runLogDir, datePath);
  const filePath = nodePath.join(dayDir, `${runId}.jsonl`);

  // Ensure directory exists
  if (!fs.existsSync(dayDir)) {
    fs.mkdirSync(dayDir, { recursive: true });
  }

  // Append event as JSON line - synchronous write for immediate flush
  const line = JSON.stringify(sanitizeEvent(event)) + "\n";
  fs.appendFileSync(filePath, line, "utf8");

  const stat = fs.statSync(filePath);
  const shouldTrim =
    stat.size >= RUN_LOG_TRIM_CONFIG.maxBytes || stat.size >= RUN_LOG_TRIM_CONFIG.maxLines * 1024;
  if (shouldTrim) {
    maybeTrimFileSync(filePath, RUN_LOG_TRIM_CONFIG);
  }
}

// Helper to generate run ID
export const generateRunId = (): string => {
  const now = new Date();
  const date = now.toISOString().slice(0, 10).replace(/-/g, "");
  const time = now.toISOString().slice(11, 19).replace(/:/g, "");
  const rand = Math.random().toString(36).slice(2, 6);
  return `run-${date}-${time}-${rand}`;
};

// Helper to get current timestamp
export const nowTs = (): string => new Date().toISOString();

export const createRunMetadata = (opts: {
  taskId: string | null;
  taskTitle: string | null;
  startedAt: string;
  workDir: string;
  logFilePath: string | null;
  sessionFilePath: string | null;
  totalTurns: number;
  finalMessage: string;
  error: string | null;
}): TaskRunMetadata => {
  const now = new Date().toISOString();
  const status = determineRunStatus(opts.finalMessage, opts.taskId);
  
  // Extract commits from final message if present
  const commits: string[] = [];
  const commitMatch = opts.finalMessage.match(/commits?:\s*\[([^\]]+)\]/i);
  if (commitMatch) {
    commits.push(...commitMatch[1].split(",").map((s) => s.trim().replace(/"/g, "")));
  }

  return {
    id: `run-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    taskId: opts.taskId,
    taskTitle: opts.taskTitle,
    status,
    startedAt: opts.startedAt,
    finishedAt: now,
    workDir: opts.workDir,
    logFilePath: opts.logFilePath,
    sessionFilePath: opts.sessionFilePath,
    commits,
    totalTurns: opts.totalTurns,
    finalMessage: opts.finalMessage,
    error: opts.error,
  };
};
