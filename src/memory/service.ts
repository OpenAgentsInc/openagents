/**
 * Memory Service
 *
 * Unified service interface for the memory system.
 * Combines store, retrieval, and linking into a single API.
 */

import { Effect, Context, Layer } from "effect";
import { MemoryStore, makeMemoryStoreLayer, type MemoryStoreError } from "./store.js";
import { EmbeddingService, EmbeddingServiceLive, type EmbeddingError } from "../skills/embedding.js";
import {
  MemoryRetrievalService,
  MemoryRetrievalServiceLive,
  type MemoryRetrievalError,
} from "./retrieval.js";
import { FMService, makeFMServiceLayer } from "../fm/service.js";
import {
  type Memory,
  type MemoryQuery,
  type MemoryMatch,
  type MemoryFilter,
  type MemoryType,
  type ImportanceLevel,
  type EpisodicContent,
  type SemanticContent,
  type ProceduralContent,
  createMemory,
  createEpisodicMemory,
  createSemanticMemory,
  createProceduralMemory,
} from "./schema.js";

// --- Unified Error Type ---

export class MemoryServiceError extends Error {
  readonly _tag = "MemoryServiceError";
  constructor(
    readonly reason: string,
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "MemoryServiceError";
  }

  static from(e: MemoryStoreError | MemoryRetrievalError | EmbeddingError): MemoryServiceError {
    return new MemoryServiceError(e._tag, e.message, e);
  }
}

// --- Service Interface ---

export interface IMemoryService {
  // --- Memory Management ---

  /** Add a new memory */
  readonly addMemory: (memory: Memory) => Effect.Effect<void, MemoryServiceError>;

  /** Create and add an episodic memory from a task result */
  readonly recordTask: (
    taskDescription: string,
    outcome: "success" | "failure" | "partial" | "timeout",
    options?: {
      errorMessage?: string;
      skillsUsed?: string[];
      filesModified?: string[];
      durationMs?: number;
      importance?: ImportanceLevel;
      projectId?: string;
      sessionId?: string;
      tags?: string[];
    },
  ) => Effect.Effect<Memory, MemoryServiceError>;

  /** Create and add a semantic memory (knowledge) */
  readonly recordKnowledge: (
    category: SemanticContent["category"],
    knowledge: string,
    options?: {
      context?: string;
      examples?: string[];
      importance?: ImportanceLevel;
      projectId?: string;
      tags?: string[];
    },
  ) => Effect.Effect<Memory, MemoryServiceError>;

  /** Create and add a procedural memory linked to a skill */
  readonly linkSkill: (
    skillId: string,
    triggerPatterns: string[],
    options?: {
      successRate?: number;
      examples?: string[];
      importance?: ImportanceLevel;
      projectId?: string;
      tags?: string[];
    },
  ) => Effect.Effect<Memory, MemoryServiceError>;

  /** Get a memory by ID */
  readonly getMemory: (id: string) => Effect.Effect<Memory | null, MemoryServiceError>;

  /** List memories with optional filter */
  readonly listMemories: (filter?: MemoryFilter) => Effect.Effect<Memory[], MemoryServiceError>;

  /** Update an existing memory */
  readonly updateMemory: (memory: Memory) => Effect.Effect<void, MemoryServiceError>;

  /** Archive a memory */
  readonly archiveMemory: (id: string) => Effect.Effect<void, MemoryServiceError>;

  // --- Memory Retrieval ---

  /** Query memories using semantic search */
  readonly query: (query: MemoryQuery) => Effect.Effect<MemoryMatch[], MemoryServiceError>;

  /** Get memories relevant to a task description */
  readonly getRelevantMemories: (
    taskDescription: string,
    options?: {
      limit?: number;
      minRelevance?: number;
      filter?: MemoryFilter;
    },
  ) => Effect.Effect<Memory[], MemoryServiceError>;

  /** Format memories for prompt injection */
  readonly formatForPrompt: (
    taskDescription: string,
    options?: {
      limit?: number;
      minRelevance?: number;
      filter?: MemoryFilter;
    },
  ) => Effect.Effect<string, MemoryServiceError>;

  // --- Linking ---

  /** Link two memories as related */
  readonly linkMemories: (
    memoryId1: string,
    memoryId2: string,
  ) => Effect.Effect<void, MemoryServiceError>;

  /** Get memories related to a given memory */
  readonly getRelatedMemories: (memoryId: string) => Effect.Effect<Memory[], MemoryServiceError>;

  // --- Stats & Maintenance ---

  /** Get memory statistics */
  readonly getStats: () => Effect.Effect<
    {
      totalMemories: number;
      memoriesWithEmbeddings: number;
      byType: Record<string, number>;
      byScope: Record<string, number>;
    },
    MemoryServiceError
  >;

  /** Populate embeddings for all memories */
  readonly populateEmbeddings: () => Effect.Effect<number, MemoryServiceError>;

  /** Prune old/decayed memories */
  readonly pruneMemories: (options?: {
    maxAge?: number; // Days
    minAccessCount?: number;
  }) => Effect.Effect<number, MemoryServiceError>;

  /** Get memory count */
  readonly count: () => Effect.Effect<number, MemoryServiceError>;
}

// --- Service Tag ---

export class MemoryService extends Context.Tag("MemoryService")<MemoryService, IMemoryService>() {}

// --- Implementation ---

const makeMemoryService = (): Effect.Effect<
  IMemoryService,
  never,
  MemoryStore | MemoryRetrievalService
> =>
  Effect.gen(function* () {
    const store = yield* MemoryStore;
    const retrieval = yield* MemoryRetrievalService;

    const mapStoreError = Effect.mapError(MemoryServiceError.from);
    const mapRetrievalError = Effect.mapError(MemoryServiceError.from);

    const addMemory = (memory: Memory): Effect.Effect<void, MemoryServiceError> =>
      store.add(memory).pipe(mapStoreError);

    const recordTask = (
      taskDescription: string,
      outcome: "success" | "failure" | "partial",
      options?: {
        errorMessage?: string;
        skillsUsed?: string[];
        filesModified?: string[];
        durationMs?: number;
        importance?: ImportanceLevel;
        projectId?: string;
        sessionId?: string;
        tags?: string[];
      },
  ): Effect.Effect<Memory, MemoryServiceError> =>
    Effect.gen(function* () {
      const memory = createEpisodicMemory(taskDescription, outcome, options);
      yield* store.add(memory).pipe(mapStoreError);
      return memory;
    });

    const recordKnowledge = (
      category: SemanticContent["category"],
      knowledge: string,
      options?: {
        context?: string;
        examples?: string[];
        importance?: ImportanceLevel;
        projectId?: string;
        tags?: string[];
      },
    ): Effect.Effect<Memory, MemoryServiceError> =>
      Effect.gen(function* () {
        const memory = createSemanticMemory(category, knowledge, options);
        yield* store.add(memory).pipe(mapStoreError);
        return memory;
      });

    const linkSkill = (
      skillId: string,
      triggerPatterns: string[],
      options?: {
        successRate?: number;
        examples?: string[];
        importance?: ImportanceLevel;
        projectId?: string;
        tags?: string[];
      },
    ): Effect.Effect<Memory, MemoryServiceError> =>
      Effect.gen(function* () {
        const memory = createProceduralMemory(skillId, triggerPatterns, options);
        yield* store.add(memory).pipe(mapStoreError);
        return memory;
      });

    const getMemory = (id: string): Effect.Effect<Memory | null, MemoryServiceError> =>
      store.get(id).pipe(mapStoreError);

    const listMemories = (filter?: MemoryFilter): Effect.Effect<Memory[], MemoryServiceError> =>
      store.list(filter).pipe(mapStoreError);

    const updateMemory = (memory: Memory): Effect.Effect<void, MemoryServiceError> =>
      store.update(memory).pipe(mapStoreError);

    const archiveMemory = (id: string): Effect.Effect<void, MemoryServiceError> =>
      store.archive(id).pipe(mapStoreError);

    const queryFn = (q: MemoryQuery): Effect.Effect<MemoryMatch[], MemoryServiceError> =>
      retrieval.query(q).pipe(mapRetrievalError);

    const getRelevantMemories = (
      taskDescription: string,
      options?: {
        limit?: number;
        minRelevance?: number;
        filter?: MemoryFilter;
      },
    ): Effect.Effect<Memory[], MemoryServiceError> =>
      retrieval.getForTask(taskDescription, options).pipe(mapRetrievalError);

    const formatForPromptFn = (
      taskDescription: string,
      options?: {
        limit?: number;
        minRelevance?: number;
        filter?: MemoryFilter;
      },
    ): Effect.Effect<string, MemoryServiceError> =>
      retrieval.formatForPrompt(taskDescription, options).pipe(mapRetrievalError);

    const linkMemories = (
      memoryId1: string,
      memoryId2: string,
    ): Effect.Effect<void, MemoryServiceError> =>
      Effect.gen(function* () {
        const memory1 = yield* store.get(memoryId1).pipe(mapStoreError);
        const memory2 = yield* store.get(memoryId2).pipe(mapStoreError);

        if (!memory1 || !memory2) {
          throw new MemoryServiceError(
            "not_found",
            `One or both memories not found: ${memoryId1}, ${memoryId2}`,
          );
        }

        // Add bidirectional links
        const related1 = new Set(memory1.relatedMemories ?? []);
        related1.add(memoryId2);

        const related2 = new Set(memory2.relatedMemories ?? []);
        related2.add(memoryId1);

        yield* store
          .update({ ...memory1, relatedMemories: Array.from(related1) })
          .pipe(mapStoreError);
        yield* store
          .update({ ...memory2, relatedMemories: Array.from(related2) })
          .pipe(mapStoreError);
      });

    const getRelatedMemories = (memoryId: string): Effect.Effect<Memory[], MemoryServiceError> =>
      store.getRelated(memoryId).pipe(mapStoreError);

    const getStats = (): Effect.Effect<
      {
        totalMemories: number;
        memoriesWithEmbeddings: number;
        byType: Record<string, number>;
        byScope: Record<string, number>;
      },
      MemoryServiceError
    > => retrieval.getStats().pipe(mapRetrievalError);

    const populateEmbeddings = (): Effect.Effect<number, MemoryServiceError> =>
      retrieval.populateEmbeddings().pipe(mapRetrievalError);

    const pruneMemories = (options?: {
      maxAge?: number;
      minAccessCount?: number;
    }): Effect.Effect<number, MemoryServiceError> =>
      Effect.gen(function* () {
        const maxAgeDays = options?.maxAge ?? 90;
        const minAccess = options?.minAccessCount ?? 0;

        const cutoffDate = new Date();
        cutoffDate.setDate(cutoffDate.getDate() - maxAgeDays);
        const cutoffStr = cutoffDate.toISOString();

        const memories = yield* store.list({ status: ["active"] }).pipe(mapStoreError);

        let pruned = 0;
        for (const memory of memories) {
          const isOld = new Date(memory.createdAt) < new Date(cutoffStr);
          const isUnused = memory.accessCount <= minAccess;

          if (isOld && isUnused) {
            yield* store.archive(memory.id).pipe(mapStoreError);
            pruned++;
          }
        }

        return pruned;
      });

    const countFn = (): Effect.Effect<number, MemoryServiceError> =>
      store.count().pipe(mapStoreError);

    return {
      addMemory,
      recordTask,
      recordKnowledge,
      linkSkill,
      getMemory,
      listMemories,
      updateMemory,
      archiveMemory,
      query: queryFn,
      getRelevantMemories,
      formatForPrompt: formatForPromptFn,
      linkMemories,
      getRelatedMemories,
      getStats,
      populateEmbeddings,
      pruneMemories,
      count: countFn,
    };
  });

// --- Layer ---

export const MemoryServiceLayer: Layer.Layer<
  MemoryService,
  never,
  MemoryStore | MemoryRetrievalService
> = Layer.effect(MemoryService, makeMemoryService());

/**
 * Create a complete MemoryService layer with all dependencies.
 */
export const makeMemoryServiceLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<MemoryService, MemoryStoreError, never> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  const storeLayer = makeMemoryStoreLayer(projectRoot);
  const embeddingLayer = Layer.provide(EmbeddingServiceLive, fmLayer);
  const retrievalLayer = Layer.provide(
    MemoryRetrievalServiceLive,
    Layer.merge(storeLayer, embeddingLayer),
  );
  return Layer.provide(MemoryServiceLayer, Layer.merge(storeLayer, retrievalLayer));
};

/**
 * Default MemoryService layer using current working directory.
 */
export const MemoryServiceLive: Layer.Layer<MemoryService, MemoryStoreError, never> =
  makeMemoryServiceLive();
