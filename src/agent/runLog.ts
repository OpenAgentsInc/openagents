import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";

// Run event types for streaming JSONL
export type TaskRunEvent =
  | { type: "run_start"; ts: string; runId: string; taskId: string | null }
  | { type: "task_selected"; ts: string; taskId: string; title: string }
  | { type: "tool_call"; ts: string; tool: string; argsPreview: string }
  | { type: "tool_result"; ts: string; tool: string; ok: boolean }
  | { type: "edit_detected"; ts: string; tool: string }
  | { type: "verify_start"; ts: string; command: string }
  | { type: "verify_ok"; ts: string }
  | { type: "verify_fail"; ts: string; stderr: string }
  | { type: "retry_prompt"; ts: string; reason: string }
  | { type: "commit_pushed"; ts: string; commit: string }
  | { type: "task_closed"; ts: string; taskId: string }
  | { type: "run_end"; ts: string; status: string; finalMessage: string; error: string | null };

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

// Append a run event to JSONL file (for streaming/tailing)
export const appendRunEvent = (
  runLogDir: string,
  runId: string,
  event: TaskRunEvent,
): Effect.Effect<void, RunLogError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    const datePath = getDatePath();
    const dayDir = path.join(runLogDir, datePath);
    const filePath = path.join(dayDir, `${runId}.jsonl`);

    // Ensure directory exists
    yield* fs.makeDirectory(dayDir, { recursive: true }).pipe(
      Effect.mapError(
        (e) => new RunLogError("dir_error", `Failed to create run-logs directory: ${e.message}`),
      ),
    );

    // Append event as JSON line
    const line = JSON.stringify(event) + "\n";
    yield* fs.writeFile(filePath, new TextEncoder().encode(line), { flag: "a" }).pipe(
      Effect.mapError(
        (e) => new RunLogError("write_error", `Failed to append run event: ${e.message}`),
      ),
    );
  });

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
