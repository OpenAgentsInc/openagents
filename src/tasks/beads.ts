import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { decodeTask, type Task } from "./schema.js";
import { DatabaseService } from "../storage/database.js";

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
  // Deduplicate dependencies - keep only the first occurrence of each (issue_id, depends_on_id) pair
  const seenDeps = new Set<string>();
  const deps = (issue.dependencies ?? []).reduce((acc, d) => {
    const key = `${d.issue_id}-${d.depends_on_id}`;
    if (!seenDeps.has(key)) {
      seenDeps.add(key);
      acc.push({ id: d.depends_on_id, type: d.type as Task["deps"][number]["type"] });
    }
    return acc;
  }, [] as Array<{ id: string; type: Task["deps"][number]["type"] }>);

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
  FileSystem.FileSystem | Path.Path | DatabaseService
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

    const db = yield* DatabaseService;
    let count = 0;

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

      // Insert the task into the database
      yield* db.insertTask(task).pipe(
        Effect.mapError(
          (e) =>
            new BeadsImportError(
              "write_error",
              `Failed to insert task: ${e.message}`,
            ),
        ),
      );

      count++;
    }

    const resolvedTasks = path.resolve(tasksPath);
    return { count, tasksPath: resolvedTasks };
  });
