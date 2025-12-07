/**
 * Memory Retrieval Service
 *
 * Combines embedding-based semantic search with Generative Agents scoring.
 * Retrieves relevant memories based on query similarity, recency, and importance.
 */

import { Effect, Context, Layer } from "effect";
import { MemoryStore, type MemoryStoreError } from "./store.js";
import { EmbeddingService, type EmbeddingError } from "../skills/embedding.js";
import {
  type Memory,
  type MemoryQuery,
  type MemoryMatch,
  type MemoryFilter,
  calculateMemoryScore,
  buildMemoryText,
  formatMemoriesForPrompt,
  DEFAULT_SCORING_WEIGHTS,
} from "./schema.js";

// --- Configuration ---

const DEFAULT_LIMIT = 10;
const DEFAULT_MIN_RELEVANCE = 0.3;

// --- Error Types ---

export class MemoryRetrievalError extends Error {
  readonly _tag = "MemoryRetrievalError";
  constructor(
    readonly reason: "store_error" | "embedding_error" | "no_memories_found",
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "MemoryRetrievalError";
  }

  static fromStoreError(e: MemoryStoreError): MemoryRetrievalError {
    return new MemoryRetrievalError("store_error", e.message, e);
  }

  static fromEmbeddingError(e: EmbeddingError): MemoryRetrievalError {
    return new MemoryRetrievalError("embedding_error", e.message, e);
  }
}

// --- Retrieval Service Interface ---

export interface IMemoryRetrievalService {
  /**
   * Query memories using semantic search with scoring.
   */
  readonly query: (query: MemoryQuery) => Effect.Effect<MemoryMatch[], MemoryRetrievalError>;

  /**
   * Get memories relevant to a task description.
   * Convenience method that builds a query from the description.
   */
  readonly getForTask: (
    taskDescription: string,
    options?: {
      limit?: number;
      minRelevance?: number;
      filter?: MemoryFilter;
    },
  ) => Effect.Effect<Memory[], MemoryRetrievalError>;

  /**
   * Format retrieved memories for prompt injection.
   */
  readonly formatForPrompt: (
    taskDescription: string,
    options?: {
      limit?: number;
      minRelevance?: number;
      filter?: MemoryFilter;
    },
  ) => Effect.Effect<string, MemoryRetrievalError>;

  /**
   * Record memory access (touch).
   */
  readonly recordAccess: (memoryId: string) => Effect.Effect<void, MemoryRetrievalError>;

  /**
   * Populate embeddings for memories that don't have them.
   */
  readonly populateEmbeddings: () => Effect.Effect<number, MemoryRetrievalError>;

  /**
   * Get retrieval statistics.
   */
  readonly getStats: () => Effect.Effect<
    {
      totalMemories: number;
      memoriesWithEmbeddings: number;
      byType: Record<string, number>;
      byScope: Record<string, number>;
    },
    MemoryRetrievalError
  >;
}

// --- Retrieval Service Tag ---

export class MemoryRetrievalService extends Context.Tag("MemoryRetrievalService")<
  MemoryRetrievalService,
  IMemoryRetrievalService
>() {}

// --- Implementation ---

const makeRetrievalService = (): Effect.Effect<
  IMemoryRetrievalService,
  never,
  MemoryStore | EmbeddingService
> =>
  Effect.gen(function* () {
    const store = yield* MemoryStore;
    const embedding = yield* EmbeddingService;

    const query = (q: MemoryQuery): Effect.Effect<MemoryMatch[], MemoryRetrievalError> =>
      Effect.gen(function* () {
        // Get query embedding
        const queryEmbedding = yield* embedding.embed(q.query).pipe(
          Effect.mapError(MemoryRetrievalError.fromEmbeddingError),
        );

        // Build filter from query
        const filter: MemoryFilter = {
          types: q.types,
          scopes: q.scopes,
          status: q.status ?? ["active"],
          tags: q.tags,
          projectId: q.projectId,
          sessionId: q.sessionId,
        };

        // Get all matching memories
        const memories = yield* store.list(filter).pipe(
          Effect.mapError(MemoryRetrievalError.fromStoreError),
        );

        if (memories.length === 0) {
          return [];
        }

        // Build embeddings for memories that don't have them
        const memoriesWithEmbeddings: Array<{ memory: Memory; embedding: number[] }> = [];

        for (const memory of memories) {
          let memoryEmbedding: number[];

          if (memory.embedding && memory.embedding.length > 0) {
            memoryEmbedding = memory.embedding;
          } else {
            // Generate embedding on-the-fly
            const text = buildMemoryText(memory);
            memoryEmbedding = yield* embedding.embed(text).pipe(
              Effect.mapError(MemoryRetrievalError.fromEmbeddingError),
            );
          }

          memoriesWithEmbeddings.push({ memory, embedding: memoryEmbedding });
        }

        // Find similar memories using embedding service
        const limit = q.limit ?? DEFAULT_LIMIT;
        const minRelevance = q.minRelevance ?? DEFAULT_MIN_RELEVANCE;

        const similarMatches = embedding.findSimilar(
          queryEmbedding,
          memoriesWithEmbeddings.map((m) => ({
            skill: m.memory as any, // Reuse skill similarity logic
            embedding: m.embedding,
          })),
          limit * 2, // Get more than needed for scoring
          minRelevance,
        );

        // Apply Generative Agents scoring
        const weights = q.weights ?? DEFAULT_SCORING_WEIGHTS;
        const scoredMatches: MemoryMatch[] = similarMatches.map((match) => {
          const memory = match.skill as unknown as Memory;
          const relevance = match.similarity;
          const score = calculateMemoryScore(memory, relevance, weights);

          return {
            memory,
            score,
            relevance,
            matchReason: `Score: ${(score * 100).toFixed(1)}% (relevance: ${(relevance * 100).toFixed(1)}%, recency: recent, importance: ${memory.importance})`,
          };
        });

        // Sort by score and take top limit
        return scoredMatches
          .sort((a, b) => b.score - a.score)
          .slice(0, limit);
      });

    const getForTask = (
      taskDescription: string,
      options?: {
        limit?: number;
        minRelevance?: number;
        filter?: MemoryFilter;
      },
    ): Effect.Effect<Memory[], MemoryRetrievalError> =>
      query({
        query: taskDescription,
        limit: options?.limit ?? DEFAULT_LIMIT,
        minRelevance: options?.minRelevance ?? DEFAULT_MIN_RELEVANCE,
        ...options?.filter,
      }).pipe(Effect.map((matches) => matches.map((m) => m.memory)));

    const formatForPromptFn = (
      taskDescription: string,
      options?: {
        limit?: number;
        minRelevance?: number;
        filter?: MemoryFilter;
      },
    ): Effect.Effect<string, MemoryRetrievalError> =>
      getForTask(taskDescription, options).pipe(
        Effect.map((memories) => {
          if (memories.length === 0) {
            return "## Relevant Memories\n\nNo relevant memories found for this task.";
          }
          return `## Relevant Memories (${memories.length} found)\n\n${formatMemoriesForPrompt(memories)}`;
        }),
      );

    const recordAccess = (memoryId: string): Effect.Effect<void, MemoryRetrievalError> =>
      store.touch(memoryId).pipe(
        Effect.mapError(MemoryRetrievalError.fromStoreError),
      );

    const populateEmbeddings = (): Effect.Effect<number, MemoryRetrievalError> =>
      Effect.gen(function* () {
        const memories = yield* store.list({ status: ["active"] }).pipe(
          Effect.mapError(MemoryRetrievalError.fromStoreError),
        );

        let populated = 0;

        for (const memory of memories) {
          if (!memory.embedding || memory.embedding.length === 0) {
            const text = buildMemoryText(memory);
            const memoryEmbedding = yield* embedding.embed(text).pipe(
              Effect.mapError(MemoryRetrievalError.fromEmbeddingError),
            );

            yield* store
              .update({
                ...memory,
                embedding: memoryEmbedding,
                updatedAt: new Date().toISOString(),
              })
              .pipe(Effect.mapError(MemoryRetrievalError.fromStoreError));

            populated++;
          }
        }

        return populated;
      });

    const getStats = (): Effect.Effect<
      {
        totalMemories: number;
        memoriesWithEmbeddings: number;
        byType: Record<string, number>;
        byScope: Record<string, number>;
      },
      MemoryRetrievalError
    > =>
      Effect.gen(function* () {
        const memories = yield* store.list().pipe(
          Effect.mapError(MemoryRetrievalError.fromStoreError),
        );

        const withEmbeddings = memories.filter(
          (m) => m.embedding && m.embedding.length > 0,
        ).length;

        const byType: Record<string, number> = {};
        const byScope: Record<string, number> = {};

        for (const memory of memories) {
          byType[memory.memoryType] = (byType[memory.memoryType] ?? 0) + 1;
          byScope[memory.scope] = (byScope[memory.scope] ?? 0) + 1;
        }

        return {
          totalMemories: memories.length,
          memoriesWithEmbeddings: withEmbeddings,
          byType,
          byScope,
        };
      });

    return {
      query,
      getForTask,
      formatForPrompt: formatForPromptFn,
      recordAccess,
      populateEmbeddings,
      getStats,
    };
  });

// --- Layer ---

export const MemoryRetrievalServiceLive: Layer.Layer<
  MemoryRetrievalService,
  never,
  MemoryStore | EmbeddingService
> = Layer.effect(MemoryRetrievalService, makeRetrievalService());
