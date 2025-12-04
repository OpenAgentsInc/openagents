import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { decodeTask, type Task } from "./schema.js";
import { writeTasks } from "./service.js";

export class BeadsImportError extends Error {
  readonly _tag = "BeadsImportError";
  constructor(
    readonly reason: "not_found" | "parse_error" | "validation_error" | "write_error",
    message: string,
  ) {
    super(message);
    this.name = "BeadsImportError";
  }
}

type BeadsDependency = {
  issue_id: string;
  depends_on_id: string;
  type: string;
};

type BeadsIssue = {
  id: string;
  title: string;
  description?: string;
  status: Task["status"];
  priority: number;
  issue_type: Task["type"];
  assignee?: string;
  labels?: string[];
  dependencies?: BeadsDependency[];
  commits?: string[];
  created_at: string;
  updated_at?: string;
  closed_at?: string | null;
  close_reason?: string;
  design?: string;
  acceptance_criteria?: string;
  notes?: string;
  estimated_minutes?: number | null;
};

const toTask = (issue: BeadsIssue): Task => {
  const deps =
    issue.dependencies?.map((d) => ({ id: d.depends_on_id, type: d.type as Task["deps"][number]["type"] })) ?? [];

  const baseTask: Task = {
    id: issue.id,
    title: issue.title,
    description: issue.description ?? "",
    status: issue.status,
    priority: issue.priority,
    type: issue.issue_type,
    assignee: issue.assignee,
    labels: issue.labels ?? [],
    deps,
    commits: issue.commits ?? [],
    comments: [],
    createdAt: issue.created_at,
    updatedAt: issue.updated_at ?? issue.created_at,
    closedAt: issue.closed_at ?? null,
    closeReason: issue.close_reason,
    design: issue.design,
    acceptanceCriteria: issue.acceptance_criteria,
    notes: issue.notes,
    estimatedMinutes: issue.estimated_minutes ?? null,
  };

  return decodeTask(baseTask);
};

export const importBeadsIssues = (
  beadsPath: string,
  tasksPath: string,
): Effect.Effect<
  { count: number; tasksPath: string },
  BeadsImportError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const resolvedBeads = path.resolve(beadsPath);

    const exists = yield* fs.exists(resolvedBeads).pipe(
      Effect.mapError(
        (e) => new BeadsImportError("not_found", `Failed to check beads file: ${e.message}`),
      ),
    );
    if (!exists) {
      return yield* Effect.fail(
        new BeadsImportError("not_found", `Beads file not found: ${resolvedBeads}`),
      );
    }

    const content = yield* fs.readFileString(resolvedBeads).pipe(
      Effect.mapError(
        (e) => new BeadsImportError("parse_error", `Failed to read beads file: ${e.message}`),
      ),
    );

    const lines = content
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0);

    const tasks: Task[] = [];

    for (const line of lines) {
      const parsed = yield* Effect.try({
        try: () => JSON.parse(line) as BeadsIssue,
        catch: (error) =>
          new BeadsImportError("parse_error", `Invalid JSON line: ${String(error)}`),
      });

      const task = yield* Effect.try({
        try: () => toTask(parsed),
        catch: (error) =>
          new BeadsImportError(
            "validation_error",
            `Invalid issue ${parsed.id}: ${(error as Error).message}`,
          ),
      });

      tasks.push(task);
    }

    const resolvedTasks = path.resolve(tasksPath);

    yield* writeTasks(resolvedTasks, tasks).pipe(
      Effect.mapError(
        (e) =>
          new BeadsImportError(
            "write_error",
            `Failed to write tasks: ${e.message}`,
          ),
      ),
    );

    return { count: tasks.length, tasksPath: resolvedTasks };
  });
