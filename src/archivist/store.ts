/**
 * Archivist Store
 *
 * JSONL-based persistence for trajectories.
 * Stores completed task trajectories for pattern analysis.
 */

import { Effect, Context, Layer } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import type { Trajectory, ArchiveConfig, ArchivistLesson } from "./schema.js";
import { DEFAULT_ARCHIVE_CONFIG } from "./schema.js";

// --- Error Types ---

export class TrajectoryStoreError extends Error {
  readonly _tag = "TrajectoryStoreError";
  constructor(
    readonly reason: "io_error" | "parse_error" | "not_found",
    message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "TrajectoryStoreError";
  }
}

// --- Store Interface ---

export interface ITrajectoryStore {
  /** Save a trajectory */
  readonly save: (trajectory: Trajectory) => Effect.Effect<void, TrajectoryStoreError>;

  /** Get a trajectory by ID */
  readonly get: (id: string) => Effect.Effect<Trajectory | null, TrajectoryStoreError>;

  /** Get all trajectories */
  readonly getAll: () => Effect.Effect<Trajectory[], TrajectoryStoreError>;

  /** Get unarchived trajectories */
  readonly getUnarchived: () => Effect.Effect<Trajectory[], TrajectoryStoreError>;

  /** Get trajectories by outcome */
  readonly getByOutcome: (
    outcome: Trajectory["outcome"],
  ) => Effect.Effect<Trajectory[], TrajectoryStoreError>;

  /** Get trajectories within age limit */
  readonly getRecent: (maxAgeDays: number) => Effect.Effect<Trajectory[], TrajectoryStoreError>;

  /** Mark trajectories as archived */
  readonly markArchived: (ids: string[]) => Effect.Effect<void, TrajectoryStoreError>;

  /** Delete old trajectories */
  readonly prune: (maxAgeDays: number) => Effect.Effect<number, TrajectoryStoreError>;

  /** Get trajectory count */
  readonly count: () => Effect.Effect<number, TrajectoryStoreError>;
}

// --- Service Tag ---

export class TrajectoryStore extends Context.Tag("TrajectoryStore")<
  TrajectoryStore,
  ITrajectoryStore
>() {}

// --- Implementation ---

const TRAJECTORIES_FILE = ".openagents/trajectories.jsonl";

interface TrajectoryCache {
  trajectories: Map<string, Trajectory>;
  loaded: boolean;
}

const makeTrajectoryStore = (config: ArchiveConfig): ITrajectoryStore => {
  const filePath = path.join(config.projectRoot, TRAJECTORIES_FILE);
  const cache: TrajectoryCache = {
    trajectories: new Map(),
    loaded: false,
  };

  const ensureDir = (): Effect.Effect<void, TrajectoryStoreError> =>
    Effect.try({
      try: () => {
        const dir = path.dirname(filePath);
        if (!fs.existsSync(dir)) {
          fs.mkdirSync(dir, { recursive: true });
        }
      },
      catch: (e) =>
        new TrajectoryStoreError("io_error", `Failed to create directory: ${e}`, e as Error),
    });

  const loadFromDisk = (): Effect.Effect<void, TrajectoryStoreError> =>
    Effect.gen(function* () {
      if (cache.loaded) {
        return;
      }

      yield* ensureDir();

      if (!fs.existsSync(filePath)) {
        cache.loaded = true;
        return;
      }

      const content = yield* Effect.try({
        try: () => fs.readFileSync(filePath, "utf-8"),
        catch: (e) =>
          new TrajectoryStoreError("io_error", `Failed to read trajectories: ${e}`, e as Error),
      });

      const lines = content.trim().split("\n").filter(Boolean);

      for (const line of lines) {
        const trajectory = yield* Effect.try({
          try: () => JSON.parse(line) as Trajectory,
          catch: (e) =>
            new TrajectoryStoreError("parse_error", `Failed to parse trajectory: ${e}`, e as Error),
        });

        cache.trajectories.set(trajectory.id, trajectory);
      }

      cache.loaded = true;
    });

  const saveToDisk = (): Effect.Effect<void, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* ensureDir();

      const lines = Array.from(cache.trajectories.values())
        .map((t) => JSON.stringify(t))
        .join("\n");

      yield* Effect.try({
        try: () => fs.writeFileSync(filePath, lines + (lines ? "\n" : ""), "utf-8"),
        catch: (e) =>
          new TrajectoryStoreError("io_error", `Failed to write trajectories: ${e}`, e as Error),
      });
    });

  const save = (trajectory: Trajectory): Effect.Effect<void, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      cache.trajectories.set(trajectory.id, trajectory);
      yield* saveToDisk();
    });

  const get = (id: string): Effect.Effect<Trajectory | null, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return cache.trajectories.get(id) ?? null;
    });

  const getAll = (): Effect.Effect<Trajectory[], TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.trajectories.values());
    });

  const getUnarchived = (): Effect.Effect<Trajectory[], TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.trajectories.values()).filter((t) => !t.archived);
    });

  const getByOutcome = (
    outcome: Trajectory["outcome"],
  ): Effect.Effect<Trajectory[], TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.trajectories.values()).filter((t) => t.outcome === outcome);
    });

  const getRecent = (maxAgeDays: number): Effect.Effect<Trajectory[], TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      const cutoff = Date.now() - maxAgeDays * 24 * 60 * 60 * 1000;
      return Array.from(cache.trajectories.values()).filter(
        (t) => new Date(t.timestamp).getTime() > cutoff,
      );
    });

  const markArchived = (ids: string[]): Effect.Effect<void, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      for (const id of ids) {
        const trajectory = cache.trajectories.get(id);
        if (trajectory) {
          trajectory.archived = true;
          cache.trajectories.set(id, trajectory);
        }
      }
      yield* saveToDisk();
    });

  const prune = (maxAgeDays: number): Effect.Effect<number, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      const cutoff = Date.now() - maxAgeDays * 24 * 60 * 60 * 1000;
      let pruned = 0;

      for (const [id, trajectory] of cache.trajectories) {
        if (new Date(trajectory.timestamp).getTime() < cutoff) {
          cache.trajectories.delete(id);
          pruned++;
        }
      }

      if (pruned > 0) {
        yield* saveToDisk();
      }

      return pruned;
    });

  const count = (): Effect.Effect<number, TrajectoryStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return cache.trajectories.size;
    });

  return {
    save,
    get,
    getAll,
    getUnarchived,
    getByOutcome,
    getRecent,
    markArchived,
    prune,
    count,
  };
};

// --- Layer ---

export const makeTrajectoryStoreLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<TrajectoryStore, never, never> =>
  Layer.succeed(
    TrajectoryStore,
    makeTrajectoryStore({ ...DEFAULT_ARCHIVE_CONFIG, projectRoot }),
  );

export const TrajectoryStoreLive: Layer.Layer<TrajectoryStore, never, never> =
  makeTrajectoryStoreLive();

// ============================================================================
// Lesson Store
// ============================================================================

export class LessonStoreError extends Error {
  readonly _tag = "LessonStoreError";
  constructor(
    readonly reason: "io_error" | "parse_error" | "not_found",
    message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "LessonStoreError";
  }
}

export interface ILessonStore {
  /** Save a lesson */
  readonly save: (lesson: ArchivistLesson) => Effect.Effect<void, LessonStoreError>;

  /** Get a lesson by ID */
  readonly get: (id: string) => Effect.Effect<ArchivistLesson | null, LessonStoreError>;

  /** Get all lessons */
  readonly getAll: () => Effect.Effect<ArchivistLesson[], LessonStoreError>;

  /** Get lessons by source */
  readonly getBySource: (
    source: ArchivistLesson["source"],
  ) => Effect.Effect<ArchivistLesson[], LessonStoreError>;

  /** Get lessons by model */
  readonly getByModel: (model: string) => Effect.Effect<ArchivistLesson[], LessonStoreError>;

  /** Get recent lessons */
  readonly getRecent: (limit: number) => Effect.Effect<ArchivistLesson[], LessonStoreError>;

  /** Delete a lesson */
  readonly delete: (id: string) => Effect.Effect<void, LessonStoreError>;

  /** Get lesson count */
  readonly count: () => Effect.Effect<number, LessonStoreError>;
}

export class LessonStore extends Context.Tag("LessonStore")<LessonStore, ILessonStore>() {}

const LESSONS_FILE = ".openagents/archivist/lessons.jsonl";

interface LessonCache {
  lessons: Map<string, ArchivistLesson>;
  loaded: boolean;
}

const makeLessonStore = (projectRoot: string): ILessonStore => {
  const filePath = path.join(projectRoot, LESSONS_FILE);
  const cache: LessonCache = {
    lessons: new Map(),
    loaded: false,
  };

  const ensureDir = (): Effect.Effect<void, LessonStoreError> =>
    Effect.try({
      try: () => {
        const dir = path.dirname(filePath);
        if (!fs.existsSync(dir)) {
          fs.mkdirSync(dir, { recursive: true });
        }
      },
      catch: (e) =>
        new LessonStoreError("io_error", `Failed to create directory: ${e}`, e as Error),
    });

  const loadFromDisk = (): Effect.Effect<void, LessonStoreError> =>
    Effect.gen(function* () {
      if (cache.loaded) {
        return;
      }

      yield* ensureDir();

      if (!fs.existsSync(filePath)) {
        cache.loaded = true;
        return;
      }

      const content = yield* Effect.try({
        try: () => fs.readFileSync(filePath, "utf-8"),
        catch: (e) =>
          new LessonStoreError("io_error", `Failed to read lessons: ${e}`, e as Error),
      });

      const lines = content.trim().split("\n").filter(Boolean);

      for (const line of lines) {
        const lesson = yield* Effect.try({
          try: () => JSON.parse(line) as ArchivistLesson,
          catch: (e) =>
            new LessonStoreError("parse_error", `Failed to parse lesson: ${e}`, e as Error),
        });

        cache.lessons.set(lesson.id, lesson);
      }

      cache.loaded = true;
    });

  const saveToDisk = (): Effect.Effect<void, LessonStoreError> =>
    Effect.gen(function* () {
      yield* ensureDir();

      const lines = Array.from(cache.lessons.values())
        .map((l) => JSON.stringify(l))
        .join("\n");

      yield* Effect.try({
        try: () => fs.writeFileSync(filePath, lines + (lines ? "\n" : ""), "utf-8"),
        catch: (e) =>
          new LessonStoreError("io_error", `Failed to write lessons: ${e}`, e as Error),
      });
    });

  const save = (lesson: ArchivistLesson): Effect.Effect<void, LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      cache.lessons.set(lesson.id, lesson);
      yield* saveToDisk();
    });

  const get = (id: string): Effect.Effect<ArchivistLesson | null, LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return cache.lessons.get(id) ?? null;
    });

  const getAll = (): Effect.Effect<ArchivistLesson[], LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.lessons.values());
    });

  const getBySource = (
    source: ArchivistLesson["source"],
  ): Effect.Effect<ArchivistLesson[], LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.lessons.values()).filter((l) => l.source === source);
    });

  const getByModel = (model: string): Effect.Effect<ArchivistLesson[], LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.lessons.values()).filter((l) => l.model === model);
    });

  const getRecent = (limit: number): Effect.Effect<ArchivistLesson[], LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return Array.from(cache.lessons.values())
        .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
        .slice(0, limit);
    });

  const deleteLesson = (id: string): Effect.Effect<void, LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      cache.lessons.delete(id);
      yield* saveToDisk();
    });

  const count = (): Effect.Effect<number, LessonStoreError> =>
    Effect.gen(function* () {
      yield* loadFromDisk();
      return cache.lessons.size;
    });

  return {
    save,
    get,
    getAll,
    getBySource,
    getByModel,
    getRecent,
    delete: deleteLesson,
    count,
  };
};

export const makeLessonStoreLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<LessonStore, never, never> =>
  Layer.succeed(LessonStore, makeLessonStore(projectRoot));

export const LessonStoreLive: Layer.Layer<LessonStore, never, never> = makeLessonStoreLive();
