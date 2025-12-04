import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { Task, type Dependency } from "./schema.js";

export class TaskMergeError extends Error {
  readonly _tag = "TaskMergeError";
  constructor(message: string) {
    super(message);
    this.name = "TaskMergeError";
  }
}

const parseTasksJsonl = (content: string): Task[] =>
  content
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => JSON.parse(line) as Task);

const toJsonl = (tasks: Task[]): string =>
  tasks
    .map((task) => JSON.stringify(task))
    .join("\n")
    .concat(tasks.length > 0 ? "\n" : "");

const isEqual = (a: unknown, b: unknown): boolean =>
  JSON.stringify(a) === JSON.stringify(b);

const latestTimestamp = (...timestamps: Array<string | null | undefined>): string | null => {
  const filtered = timestamps.filter((ts): ts is string => Boolean(ts));
  if (filtered.length === 0) return null;
  return filtered.reduce((acc, ts) => (ts > acc ? ts : acc));
};

const unionStrings = (...arrays: Array<ReadonlyArray<string> | undefined>): string[] => {
  const set = new Set<string>();
  for (const arr of arrays) {
    for (const value of arr ?? []) {
      set.add(value);
    }
  }
  return Array.from(set);
};

const unionDeps = (...arrays: Array<ReadonlyArray<Dependency> | undefined>): Dependency[] => {
  const dedup = new Map<string, Dependency>();
  for (const arr of arrays) {
    for (const dep of arr ?? []) {
      const key = `${dep.id}:${dep.type}`;
      if (!dedup.has(key)) dedup.set(key, dep);
    }
  }
  return Array.from(dedup.values());
};

const pickValue = <T>(
  base: T | undefined,
  current: T | undefined,
  incoming: T | undefined,
  updatedAtCurrent: string | undefined,
  updatedAtIncoming: string | undefined,
): T | undefined => {
  if (isEqual(current, incoming)) return current;
  if (isEqual(base, current)) return incoming;
  if (isEqual(base, incoming)) return current;
  // Both diverged: choose the side with the newer updatedAt (default to incoming)
  if (updatedAtCurrent && updatedAtIncoming) {
    return updatedAtCurrent >= updatedAtIncoming ? current : incoming;
  }
  if (updatedAtCurrent && !updatedAtIncoming) return current;
  if (!updatedAtCurrent && updatedAtIncoming) return incoming;
  return incoming ?? current;
};

export interface MergeResult {
  tasks: Task[];
  conflicts: string[];
}

export const mergeTasks = (
  baseContent: string,
  currentContent: string,
  incomingContent: string,
): MergeResult => {
  const baseTasks = parseTasksJsonl(baseContent);
  const currentTasks = parseTasksJsonl(currentContent);
  const incomingTasks = parseTasksJsonl(incomingContent);

  const allIds = new Set<string>();
  for (const t of [...baseTasks, ...currentTasks, ...incomingTasks]) {
    allIds.add(t.id);
  }

  const baseMap = new Map(baseTasks.map((t) => [t.id, t]));
  const currentMap = new Map(currentTasks.map((t) => [t.id, t]));
  const incomingMap = new Map(incomingTasks.map((t) => [t.id, t]));

  const merged: Task[] = [];
  const conflicts: string[] = [];

  for (const id of Array.from(allIds).sort()) {
    const base = baseMap.get(id);
    const current = currentMap.get(id);
    const incoming = incomingMap.get(id);

    const template = current ?? incoming ?? base;
    if (!template) {
      conflicts.push(`Task ${id} missing in all inputs`);
      continue;
    }

    const mergedUpdatedAt = latestTimestamp(
      base?.updatedAt,
      current?.updatedAt,
      incoming?.updatedAt,
    ) ?? template.updatedAt;

    const mergedClosedAt = latestTimestamp(
      base?.closedAt ?? null,
      current?.closedAt ?? null,
      incoming?.closedAt ?? null,
    );

    const mergedCreatedAt = latestTimestamp(
      base?.createdAt,
      current?.createdAt,
      incoming?.createdAt,
    ) ?? template.createdAt;

    const mergedTask: Task = {
      ...template,
      id,
      title: pickValue(base?.title, current?.title, incoming?.title, current?.updatedAt, incoming?.updatedAt)
        ?? template.title,
      description:
        pickValue(
          base?.description,
          current?.description,
          incoming?.description,
          current?.updatedAt,
          incoming?.updatedAt,
        ) ?? template.description,
      status:
        pickValue(base?.status, current?.status, incoming?.status, current?.updatedAt, incoming?.updatedAt)
        ?? template.status,
      priority:
        pickValue(base?.priority, current?.priority, incoming?.priority, current?.updatedAt, incoming?.updatedAt)
        ?? template.priority,
      type:
        pickValue(base?.type, current?.type, incoming?.type, current?.updatedAt, incoming?.updatedAt)
        ?? template.type,
      assignee:
        pickValue(base?.assignee, current?.assignee, incoming?.assignee, current?.updatedAt, incoming?.updatedAt),
      labels: unionStrings(base?.labels, current?.labels, incoming?.labels),
      deps: unionDeps(base?.deps, current?.deps, incoming?.deps),
      commits: unionStrings(base?.commits, current?.commits, incoming?.commits),
      createdAt: mergedCreatedAt,
      updatedAt: mergedUpdatedAt,
      closedAt: mergedClosedAt,
      closeReason:
        pickValue(base?.closeReason, current?.closeReason, incoming?.closeReason, current?.updatedAt, incoming?.updatedAt),
      source:
        pickValue(base?.source, current?.source, incoming?.source, current?.updatedAt, incoming?.updatedAt),
      design:
        pickValue(base?.design, current?.design, incoming?.design, current?.updatedAt, incoming?.updatedAt)
        ?? template.design,
      acceptanceCriteria:
        pickValue(
          base?.acceptanceCriteria,
          current?.acceptanceCriteria,
          incoming?.acceptanceCriteria,
          current?.updatedAt,
          incoming?.updatedAt,
        ) ?? template.acceptanceCriteria,
      notes:
        pickValue(base?.notes, current?.notes, incoming?.notes, current?.updatedAt, incoming?.updatedAt)
        ?? template.notes,
      estimatedMinutes:
        pickValue(
          base?.estimatedMinutes,
          current?.estimatedMinutes,
          incoming?.estimatedMinutes,
          current?.updatedAt,
          incoming?.updatedAt,
        ) ?? template.estimatedMinutes,
    };

    if (!isEqual(current, incoming) && !isEqual(base, current) && !isEqual(base, incoming)) {
      conflicts.push(id);
    }

    merged.push(mergedTask);
  }

  return { tasks: merged, conflicts };
};

export interface MergeFilesOptions {
  basePath: string;
  currentPath: string;
  incomingPath: string;
  outputPath?: string;
}

export const mergeTaskFiles = ({
  basePath,
  currentPath,
  incomingPath,
  outputPath,
}: MergeFilesOptions): Effect.Effect<
  { mergedPath: string; conflicts: string[] },
  TaskMergeError,
  FileSystem.FileSystem
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const [baseContent, currentContent, incomingContent] = yield* Effect.all([
      fs.readFileString(basePath),
      fs.readFileString(currentPath),
      fs.readFileString(incomingPath),
    ]).pipe(
      Effect.mapError(
        (e) => new TaskMergeError(`Failed to read files: ${e.message}`),
      ),
    );

    const { tasks, conflicts } = mergeTasks(baseContent, currentContent, incomingContent);
    const mergedContent = toJsonl(tasks);
    const targetPath = outputPath ?? currentPath;

    yield* fs.writeFile(targetPath, new TextEncoder().encode(mergedContent)).pipe(
      Effect.mapError(
        (e) => new TaskMergeError(`Failed to write merged tasks: ${e.message}`),
      ),
    );

    return { mergedPath: targetPath, conflicts };
  });

const MERGE_DRIVER_CONFIG = `[merge \"oa-tasks\"]
\tname = OpenAgents tasks.jsonl merge
\tdriver = bun src/tasks/cli.ts merge --base %O --current %A --incoming %B --output %A
`;

const GITATTRIBUTES_ENTRY = ".openagents/tasks.jsonl merge=oa-tasks\n";

export const ensureMergeDriverConfig = (
  rootDir: string,
): Effect.Effect<
  { gitattributesPath: string | null; gitConfigPath: string | null },
  TaskMergeError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const resolvedRoot = path.resolve(rootDir);
    const gitDir = path.join(resolvedRoot, ".git");
    const gitConfigPath = path.join(gitDir, "config");
    const gitattributesPath = path.join(resolvedRoot, ".gitattributes");

    const gitExists = yield* fs.exists(gitDir).pipe(
      Effect.mapError(
        (e) => new TaskMergeError(`Failed to check .git: ${e.message}`),
      ),
    );

    if (!gitExists) {
      return { gitattributesPath: null, gitConfigPath: null };
    }

    // Ensure .gitattributes entry
    let attributesContent = "";
    const hasAttributes = yield* fs.exists(gitattributesPath).pipe(
      Effect.mapError(
        (e) => new TaskMergeError(`Failed to check .gitattributes: ${e.message}`),
      ),
    );
    if (hasAttributes) {
      attributesContent = yield* fs.readFileString(gitattributesPath).pipe(
        Effect.mapError(
          (e) => new TaskMergeError(`Failed to read .gitattributes: ${e.message}`),
        ),
      );
    }
    if (!attributesContent.includes("merge=oa-tasks")) {
      const needsNewline = attributesContent.length > 0 && !attributesContent.endsWith("\n");
      const updated = `${attributesContent}${needsNewline ? "\n" : ""}${GITATTRIBUTES_ENTRY}`;
      yield* fs.writeFile(gitattributesPath, new TextEncoder().encode(updated)).pipe(
        Effect.mapError(
          (e) => new TaskMergeError(`Failed to write .gitattributes: ${e.message}`),
        ),
      );
    }

    // Ensure merge driver config
    let configContent = "";
    const hasConfig = yield* fs.exists(gitConfigPath).pipe(
      Effect.mapError(
        (e) => new TaskMergeError(`Failed to check .git/config: ${e.message}`),
      ),
    );
    if (hasConfig) {
      configContent = yield* fs.readFileString(gitConfigPath).pipe(
        Effect.mapError(
          (e) => new TaskMergeError(`Failed to read .git/config: ${e.message}`),
        ),
      );
    }

    if (!configContent.includes('[merge "oa-tasks"]')) {
      const prefix = configContent.length > 0 && !configContent.endsWith("\n") ? "\n" : "";
      const updatedConfig = `${configContent}${prefix}${MERGE_DRIVER_CONFIG}`;
      yield* fs.writeFile(gitConfigPath, new TextEncoder().encode(updatedConfig)).pipe(
        Effect.mapError(
          (e) => new TaskMergeError(`Failed to write .git/config: ${e.message}`),
        ),
      );
    }

    return { gitattributesPath, gitConfigPath };
  });
