/**
 * Reflection Storage
 *
 * JSONL-based storage for reflections in .openagents/memory/reflections.jsonl.
 * Uses Effect for all operations.
 */
import { Effect } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import * as S from "effect/Schema";
import { Reflection } from "./schema.js";
import { ReflectionError } from "./errors.js";

// ============================================================================
// Storage Interface
// ============================================================================

/**
 * Storage interface for reflections.
 */
export interface ReflectionStorage {
  /**
   * Save a reflection to storage.
   */
  save(reflection: Reflection): Effect.Effect<void, ReflectionError>;

  /**
   * Load all reflections from storage.
   */
  loadAll(): Effect.Effect<Reflection[], ReflectionError>;

  /**
   * Load reflections for a specific subtask.
   */
  loadBySubtask(subtaskId: string): Effect.Effect<Reflection[], ReflectionError>;

  /**
   * Load reflections for a specific task.
   */
  loadByTask(taskId: string): Effect.Effect<Reflection[], ReflectionError>;

  /**
   * Prune reflections older than maxAgeMs.
   * Returns number of reflections pruned.
   */
  prune(maxAgeMs: number): Effect.Effect<number, ReflectionError>;
}

// ============================================================================
// File-based Storage Implementation
// ============================================================================

/**
 * Create a file-based reflection storage.
 */
export const makeFileStorage = (openagentsDir: string): ReflectionStorage => {
  const memoryDir = path.join(openagentsDir, "memory");
  const reflectionsPath = path.join(memoryDir, "reflections.jsonl");

  // Ensure directory exists
  const ensureDir = (): void => {
    if (!fs.existsSync(memoryDir)) {
      fs.mkdirSync(memoryDir, { recursive: true });
    }
  };

  // Parse a single line to Reflection
  const parseLine = (line: string): Reflection | null => {
    try {
      const obj = JSON.parse(line);
      return S.decodeUnknownSync(Reflection)(obj);
    } catch {
      return null;
    }
  };

  return {
    save: (reflection: Reflection) =>
      Effect.gen(function* () {
        yield* Effect.try({
          try: () => {
            ensureDir();
            const line = JSON.stringify(reflection) + "\n";
            fs.appendFileSync(reflectionsPath, line, "utf-8");
          },
          catch: (e) => ReflectionError.storageError(`Failed to save reflection: ${e}`),
        });
      }),

    loadAll: () =>
      Effect.gen(function* () {
        const content = yield* Effect.try({
          try: () => {
            if (!fs.existsSync(reflectionsPath)) {
              return "";
            }
            return fs.readFileSync(reflectionsPath, "utf-8");
          },
          catch: (e) => ReflectionError.storageError(`Failed to read reflections: ${e}`),
        });

        const lines = content.split("\n").filter((line) => line.trim());
        const reflections: Reflection[] = [];

        for (const line of lines) {
          const reflection = parseLine(line);
          if (reflection) {
            reflections.push(reflection);
          }
        }

        return reflections;
      }),

    loadBySubtask: (subtaskId: string) =>
      Effect.gen(function* () {
        const all = yield* Effect.try({
          try: () => {
            if (!fs.existsSync(reflectionsPath)) {
              return [];
            }
            const content = fs.readFileSync(reflectionsPath, "utf-8");
            const lines = content.split("\n").filter((line) => line.trim());
            return lines
              .map(parseLine)
              .filter((r): r is Reflection => r !== null && r.subtaskId === subtaskId);
          },
          catch: (e) => ReflectionError.storageError(`Failed to load reflections: ${e}`),
        });
        return all;
      }),

    loadByTask: (taskId: string) =>
      Effect.gen(function* () {
        const all = yield* Effect.try({
          try: () => {
            if (!fs.existsSync(reflectionsPath)) {
              return [];
            }
            const content = fs.readFileSync(reflectionsPath, "utf-8");
            const lines = content.split("\n").filter((line) => line.trim());
            return lines.map(parseLine).filter((r): r is Reflection => r !== null && r.taskId === taskId);
          },
          catch: (e) => ReflectionError.storageError(`Failed to load reflections: ${e}`),
        });
        return all;
      }),

    prune: (maxAgeMs: number) =>
      Effect.gen(function* () {
        const all = yield* Effect.try({
          try: () => {
            if (!fs.existsSync(reflectionsPath)) {
              return [];
            }
            const content = fs.readFileSync(reflectionsPath, "utf-8");
            const lines = content.split("\n").filter((line) => line.trim());
            return lines.map(parseLine).filter((r): r is Reflection => r !== null);
          },
          catch: (e) => ReflectionError.storageError(`Failed to load reflections for pruning: ${e}`),
        });

        const now = Date.now();
        const cutoff = now - maxAgeMs;
        const toKeep = all.filter((r) => new Date(r.createdAt).getTime() > cutoff);
        const pruned = all.length - toKeep.length;

        if (pruned > 0) {
          yield* Effect.try({
            try: () => {
              ensureDir();
              const content = toKeep.map((r) => JSON.stringify(r)).join("\n") + (toKeep.length > 0 ? "\n" : "");
              fs.writeFileSync(reflectionsPath, content, "utf-8");
            },
            catch: (e) => ReflectionError.storageError(`Failed to write pruned reflections: ${e}`),
          });
        }

        return pruned;
      }),
  };
};

// ============================================================================
// In-Memory Storage (for testing)
// ============================================================================

/**
 * Create an in-memory reflection storage for testing.
 */
export const makeMemoryStorage = (): ReflectionStorage & { reflections: Reflection[] } => {
  const reflections: Reflection[] = [];

  return {
    reflections,

    save: (reflection: Reflection) =>
      Effect.sync(() => {
        reflections.push(reflection);
      }),

    loadAll: () => Effect.succeed([...reflections]),

    loadBySubtask: (subtaskId: string) =>
      Effect.succeed(reflections.filter((r) => r.subtaskId === subtaskId)),

    loadByTask: (taskId: string) => Effect.succeed(reflections.filter((r) => r.taskId === taskId)),

    prune: (maxAgeMs: number) =>
      Effect.sync(() => {
        const now = Date.now();
        const cutoff = now - maxAgeMs;
        const before = reflections.length;
        const toKeep = reflections.filter((r) => new Date(r.createdAt).getTime() > cutoff);
        reflections.length = 0;
        reflections.push(...toKeep);
        return before - toKeep.length;
      }),
  };
};
