/**
 * Memory Store
 *
 * JSONL-based storage for memories with in-memory caching and indexing.
 * Optimized for fast retrieval and efficient persistence.
 */

import { Effect, Context, Layer } from "effect";
import {
  type Memory,
  type MemoryFilter,
  type MemoryType,
  type MemoryScope,
  type MemoryStatus,
} from "./schema.js";

// --- Store Configuration ---

const MEMORY_FILE = ".openagents/memories.jsonl";

// --- Error Types ---

export class MemoryStoreError extends Error {
  readonly _tag = "MemoryStoreError";
  constructor(
    readonly reason: "file_error" | "parse_error" | "not_found" | "duplicate",
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "MemoryStoreError";
  }
}

// --- Store Interface ---

export interface IMemoryStore {
  /** Get a memory by ID */
  readonly get: (id: string) => Effect.Effect<Memory | null, MemoryStoreError>;

  /** List memories with optional filter */
  readonly list: (filter?: MemoryFilter) => Effect.Effect<Memory[], MemoryStoreError>;

  /** Add a new memory */
  readonly add: (memory: Memory) => Effect.Effect<void, MemoryStoreError>;

  /** Update an existing memory */
  readonly update: (memory: Memory) => Effect.Effect<void, MemoryStoreError>;

  /** Delete a memory */
  readonly delete: (id: string) => Effect.Effect<void, MemoryStoreError>;

  /** Archive a memory (soft delete) */
  readonly archive: (id: string) => Effect.Effect<void, MemoryStoreError>;

  /** Count memories with optional filter */
  readonly count: (filter?: MemoryFilter) => Effect.Effect<number, MemoryStoreError>;

  /** Touch a memory (update access time and count) */
  readonly touch: (id: string) => Effect.Effect<void, MemoryStoreError>;

  /** Get memories by type */
  readonly getByType: (type: MemoryType) => Effect.Effect<Memory[], MemoryStoreError>;

  /** Get memories by scope */
  readonly getByScope: (scope: MemoryScope) => Effect.Effect<Memory[], MemoryStoreError>;

  /** Get memories by tag */
  readonly getByTag: (tag: string) => Effect.Effect<Memory[], MemoryStoreError>;

  /** Get related memories */
  readonly getRelated: (id: string) => Effect.Effect<Memory[], MemoryStoreError>;

  /** Prune decayed memories */
  readonly pruneDecayed: () => Effect.Effect<number, MemoryStoreError>;

  /** Force persist to disk */
  readonly persist: () => Effect.Effect<void, MemoryStoreError>;
}

// --- Store Tag ---

export class MemoryStore extends Context.Tag("MemoryStore")<MemoryStore, IMemoryStore>() {}

// --- In-Memory Cache with Indexes ---

interface MemoryCache {
  /** All memories by ID */
  byId: Map<string, Memory>;
  /** Index by type */
  byType: Map<MemoryType, Set<string>>;
  /** Index by scope */
  byScope: Map<MemoryScope, Set<string>>;
  /** Index by tag */
  byTag: Map<string, Set<string>>;
  /** Index by project */
  byProject: Map<string, Set<string>>;
  /** Index by session */
  bySession: Map<string, Set<string>>;
  /** Dirty flag for persistence */
  dirty: boolean;
}

const createEmptyCache = (): MemoryCache => ({
  byId: new Map(),
  byType: new Map([
    ["episodic", new Set()],
    ["semantic", new Set()],
    ["procedural", new Set()],
  ]),
  byScope: new Map([
    ["global", new Set()],
    ["project", new Set()],
    ["session", new Set()],
  ]),
  byTag: new Map(),
  byProject: new Map(),
  bySession: new Map(),
  dirty: false,
});

const indexMemory = (cache: MemoryCache, memory: Memory): void => {
  cache.byId.set(memory.id, memory);
  cache.byType.get(memory.memoryType)?.add(memory.id);
  cache.byScope.get(memory.scope)?.add(memory.id);

  for (const tag of memory.tags) {
    if (!cache.byTag.has(tag)) {
      cache.byTag.set(tag, new Set());
    }
    cache.byTag.get(tag)?.add(memory.id);
  }

  if (memory.projectId) {
    if (!cache.byProject.has(memory.projectId)) {
      cache.byProject.set(memory.projectId, new Set());
    }
    cache.byProject.get(memory.projectId)?.add(memory.id);
  }

  if (memory.sessionId) {
    if (!cache.bySession.has(memory.sessionId)) {
      cache.bySession.set(memory.sessionId, new Set());
    }
    cache.bySession.get(memory.sessionId)?.add(memory.id);
  }
};

const unindexMemory = (cache: MemoryCache, memory: Memory): void => {
  cache.byId.delete(memory.id);
  cache.byType.get(memory.memoryType)?.delete(memory.id);
  cache.byScope.get(memory.scope)?.delete(memory.id);

  for (const tag of memory.tags) {
    cache.byTag.get(tag)?.delete(memory.id);
  }

  if (memory.projectId) {
    cache.byProject.get(memory.projectId)?.delete(memory.id);
  }

  if (memory.sessionId) {
    cache.bySession.get(memory.sessionId)?.delete(memory.id);
  }
};

// --- Store Implementation ---

const makeMemoryStore = (projectRoot: string): IMemoryStore => {
  const filePath = `${projectRoot}/${MEMORY_FILE}`;
  let cache = createEmptyCache();
  let loaded = false;

  // Load memories from file
  const loadFromFile = (): Effect.Effect<void, MemoryStoreError> =>
    Effect.gen(function* () {
      if (loaded) return;

      try {
        const file = Bun.file(filePath);
        const exists = yield* Effect.tryPromise({
          try: () => file.exists(),
          catch: (e) =>
            new MemoryStoreError(
              "file_error",
              `Failed to check file existence: ${e}`,
              e instanceof Error ? e : undefined,
            ),
        });

        if (!exists) {
          loaded = true;
          return;
        }

        const content = yield* Effect.tryPromise({
          try: () => file.text(),
          catch: (e) =>
            new MemoryStoreError(
              "file_error",
              `Failed to read memories file: ${e}`,
              e instanceof Error ? e : undefined,
            ),
        });

        const lines = content.split("\n").filter((line) => line.trim());
        for (const line of lines) {
          try {
            const memory = JSON.parse(line) as Memory;
            indexMemory(cache, memory);
          } catch (e) {
            // Skip malformed lines
            console.warn(`Skipping malformed memory line: ${e}`);
          }
        }

        loaded = true;
      } catch (e) {
        throw new MemoryStoreError(
          "file_error",
          `Failed to load memories: ${e}`,
          e instanceof Error ? e : undefined,
        );
      }
    });

  // Persist memories to file
  const persistToFile = (): Effect.Effect<void, MemoryStoreError> =>
    Effect.gen(function* () {
      if (!cache.dirty) return;

      const lines = Array.from(cache.byId.values())
        .map((memory) => JSON.stringify(memory))
        .join("\n");

      // Ensure directory exists
      const dir = filePath.substring(0, filePath.lastIndexOf("/"));
      yield* Effect.tryPromise({
        try: async () => {
          const { mkdir } = await import("fs/promises");
          await mkdir(dir, { recursive: true });
        },
        catch: (e) =>
          new MemoryStoreError(
            "file_error",
            `Failed to create directory: ${e}`,
            e instanceof Error ? e : undefined,
          ),
      });

      yield* Effect.tryPromise({
        try: () => Bun.write(filePath, lines + "\n"),
        catch: (e) =>
          new MemoryStoreError(
            "file_error",
            `Failed to write memories file: ${e}`,
            e instanceof Error ? e : undefined,
          ),
      });

      cache.dirty = false;
    });

  // Ensure loaded before operations
  const ensureLoaded = loadFromFile;

  // Apply filter to memories
  const applyFilter = (memories: Memory[], filter?: MemoryFilter): Memory[] => {
    if (!filter) return memories;

    return memories.filter((m) => {
      if (filter.types && !filter.types.includes(m.memoryType)) return false;
      if (filter.scopes && !filter.scopes.includes(m.scope)) return false;
      if (filter.status && !filter.status.includes(m.status)) return false;
      if (filter.tags && !filter.tags.some((t) => m.tags.includes(t))) return false;
      if (filter.projectId && m.projectId !== filter.projectId) return false;
      if (filter.sessionId && m.sessionId !== filter.sessionId) return false;
      if (filter.source && !filter.source.includes(m.source)) return false;
      if (filter.since && new Date(m.createdAt) < new Date(filter.since)) return false;
      if (filter.until && new Date(m.createdAt) > new Date(filter.until)) return false;
      return true;
    });
  };

  return {
    get: (id) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        return cache.byId.get(id) ?? null;
      }),

    list: (filter) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const all = Array.from(cache.byId.values());
        return applyFilter(all, filter);
      }),

    add: (memory) =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        if (cache.byId.has(memory.id)) {
          throw new MemoryStoreError(
            "duplicate",
            `Memory with ID ${memory.id} already exists`,
          );
        }

        indexMemory(cache, memory);
        cache.dirty = true;
        yield* persistToFile();
      }),

    update: (memory) =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        const existing = cache.byId.get(memory.id);
        if (!existing) {
          throw new MemoryStoreError(
            "not_found",
            `Memory with ID ${memory.id} not found`,
          );
        }

        // Reindex if needed
        unindexMemory(cache, existing);
        indexMemory(cache, { ...memory, updatedAt: new Date().toISOString() });
        cache.dirty = true;
        yield* persistToFile();
      }),

    delete: (id) =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        const memory = cache.byId.get(id);
        if (!memory) {
          return; // Idempotent delete
        }

        unindexMemory(cache, memory);
        cache.dirty = true;
        yield* persistToFile();
      }),

    archive: (id) =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        const memory = cache.byId.get(id);
        if (!memory) {
          throw new MemoryStoreError(
            "not_found",
            `Memory with ID ${id} not found`,
          );
        }

        const archived: Memory = {
          ...memory,
          status: "archived",
          updatedAt: new Date().toISOString(),
        };

        unindexMemory(cache, memory);
        indexMemory(cache, archived);
        cache.dirty = true;
        yield* persistToFile();
      }),

    count: (filter) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const all = Array.from(cache.byId.values());
        return applyFilter(all, filter).length;
      }),

    touch: (id) =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        const memory = cache.byId.get(id);
        if (!memory) {
          return; // Ignore missing
        }

        const touched: Memory = {
          ...memory,
          accessCount: memory.accessCount + 1,
          lastAccessedAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };

        cache.byId.set(id, touched);
        cache.dirty = true;
        // Don't persist immediately for performance
      }),

    getByType: (type) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const ids = cache.byType.get(type) ?? new Set();
        return Array.from(ids)
          .map((id) => cache.byId.get(id))
          .filter((m): m is Memory => m !== undefined);
      }),

    getByScope: (scope) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const ids = cache.byScope.get(scope) ?? new Set();
        return Array.from(ids)
          .map((id) => cache.byId.get(id))
          .filter((m): m is Memory => m !== undefined);
      }),

    getByTag: (tag) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const ids = cache.byTag.get(tag) ?? new Set();
        return Array.from(ids)
          .map((id) => cache.byId.get(id))
          .filter((m): m is Memory => m !== undefined);
      }),

    getRelated: (id) =>
      Effect.gen(function* () {
        yield* ensureLoaded();
        const memory = cache.byId.get(id);
        if (!memory?.relatedMemories) return [];

        return memory.relatedMemories
          .map((relId) => cache.byId.get(relId))
          .filter((m): m is Memory => m !== undefined);
      }),

    pruneDecayed: () =>
      Effect.gen(function* () {
        yield* ensureLoaded();

        const decayed = Array.from(cache.byId.values()).filter(
          (m) => m.status === "decayed",
        );

        for (const memory of decayed) {
          unindexMemory(cache, memory);
        }

        if (decayed.length > 0) {
          cache.dirty = true;
          yield* persistToFile();
        }

        return decayed.length;
      }),

    persist: () => persistToFile(),
  };
};

// --- Layer ---

export const makeMemoryStoreLayer = (
  projectRoot: string,
): Layer.Layer<MemoryStore, never, never> =>
  Layer.succeed(MemoryStore, makeMemoryStore(projectRoot));

export const MemoryStoreLive: Layer.Layer<MemoryStore, never, never> =
  makeMemoryStoreLayer(process.cwd());
