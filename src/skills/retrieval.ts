/**
 * Skill Retrieval Service
 *
 * Combines embedding-based semantic search with filtered retrieval.
 * Implements the skill selection algorithm for FM prompts.
 */

import { Effect, Context, Layer } from "effect";
import { SkillStore, type SkillStoreError } from "./store.js";
import { EmbeddingService, type EmbeddingError, buildSkillText } from "./embedding.js";
import {
  type Skill,
  type SkillQuery,
  type SkillMatch,
  type SkillFilter,
  formatSkillsForPrompt,
} from "./schema.js";

// --- Configuration ---

const DEFAULT_TOP_K = 5;
const DEFAULT_MIN_SIMILARITY = 0.3;

// --- Error Types ---

export class SkillRetrievalError extends Error {
  readonly _tag = "SkillRetrievalError";
  constructor(
    readonly reason: "store_error" | "embedding_error" | "no_skills_found",
    override readonly message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "SkillRetrievalError";
  }

  static fromStoreError(e: SkillStoreError): SkillRetrievalError {
    return new SkillRetrievalError("store_error", e.message, e);
  }

  static fromEmbeddingError(e: EmbeddingError): SkillRetrievalError {
    return new SkillRetrievalError("embedding_error", e.message, e);
  }
}

// --- Retrieval Service Interface ---

export interface ISkillRetrievalService {
  /**
   * Query skills using semantic search.
   */
  readonly query: (query: SkillQuery) => Effect.Effect<SkillMatch[], SkillRetrievalError>;

  /**
   * Get skills relevant to a task description.
   * Convenience method that builds a query from the description.
   */
  readonly getForTask: (
    taskDescription: string,
    options?: {
      topK?: number;
      minSimilarity?: number;
      filter?: SkillFilter;
    },
  ) => Effect.Effect<Skill[], SkillRetrievalError>;

  /**
   * Format retrieved skills for prompt injection.
   */
  readonly formatForPrompt: (
    taskDescription: string,
    options?: {
      topK?: number;
      minSimilarity?: number;
      filter?: SkillFilter;
    },
  ) => Effect.Effect<string, SkillRetrievalError>;

  /**
   * Record skill usage for stats tracking.
   */
  readonly recordUsage: (
    skillId: string,
    success: boolean,
  ) => Effect.Effect<void, SkillRetrievalError>;

  /**
   * Populate embeddings for all skills that don't have them.
   */
  readonly populateEmbeddings: () => Effect.Effect<number, SkillRetrievalError>;

  /**
   * Get retrieval statistics.
   */
  readonly getStats: () => Effect.Effect<
    {
      totalSkills: number;
      skillsWithEmbeddings: number;
      averageSuccessRate: number;
    },
    SkillRetrievalError
  >;
}

// --- Retrieval Service Tag ---

export class SkillRetrievalService extends Context.Tag("SkillRetrievalService")<
  SkillRetrievalService,
  ISkillRetrievalService
>() {}

// --- Implementation ---

const makeRetrievalService = (): Effect.Effect<
  ISkillRetrievalService,
  never,
  SkillStore | EmbeddingService
> =>
  Effect.gen(function* () {
    const store = yield* SkillStore;
    const embedding = yield* EmbeddingService;

    const query = (q: SkillQuery): Effect.Effect<SkillMatch[], SkillRetrievalError> =>
      Effect.gen(function* () {
        // Get all active skills FIRST - avoid embedding call if no skills exist
        const skills = yield* store
          .list({
            ...q.filter,
            status: q.filter?.status ?? ["active"],
          })
          .pipe(Effect.mapError(SkillRetrievalError.fromStoreError));

        // Early exit if no skills - avoids unnecessary embedding call
        if (skills.length === 0) {
          return [];
        }

        // Only generate query embedding if we have skills to compare against
        const queryEmbedding = yield* embedding.embed(q.query).pipe(
          Effect.mapError(SkillRetrievalError.fromEmbeddingError),
        );

        // Build embeddings for skills that don't have them
        const skillsWithEmbeddings: Array<{ skill: Skill; embedding: number[] }> = [];

        for (const skill of skills) {
          let skillEmbedding: number[];

          if (skill.embedding && skill.embedding.length > 0) {
            skillEmbedding = skill.embedding;
          } else {
            // Generate embedding on-the-fly
            skillEmbedding = yield* embedding.embedSkill(skill).pipe(
              Effect.mapError(SkillRetrievalError.fromEmbeddingError),
            );
          }

          skillsWithEmbeddings.push({ skill, embedding: skillEmbedding });
        }

        // Find similar skills
        const topK = q.topK ?? DEFAULT_TOP_K;
        const minSimilarity = q.minSimilarity ?? DEFAULT_MIN_SIMILARITY;

        const matches = embedding.findSimilar(
          queryEmbedding,
          skillsWithEmbeddings,
          topK,
          minSimilarity,
        );

        // Build match results with reasons
        return matches.map((m) => ({
          skill: m.skill,
          similarity: m.similarity,
          matchReason: `Similarity: ${(m.similarity * 100).toFixed(1)}% to "${q.query.slice(0, 50)}..."`,
        }));
      });

    const getForTask = (
      taskDescription: string,
      options?: {
        topK?: number;
        minSimilarity?: number;
        filter?: SkillFilter;
      },
    ): Effect.Effect<Skill[], SkillRetrievalError> =>
      query({
        query: taskDescription,
        topK: options?.topK ?? DEFAULT_TOP_K,
        minSimilarity: options?.minSimilarity ?? DEFAULT_MIN_SIMILARITY,
        filter: options?.filter,
      }).pipe(Effect.map((matches) => matches.map((m) => m.skill)));

    const formatForPromptFn = (
      taskDescription: string,
      options?: {
        topK?: number;
        minSimilarity?: number;
        filter?: SkillFilter;
      },
    ): Effect.Effect<string, SkillRetrievalError> =>
      getForTask(taskDescription, options).pipe(
        Effect.map((skills) => {
          if (skills.length === 0) {
            return "## Available Skills\n\nNo relevant skills found for this task. Use basic tools directly.";
          }
          return `## Available Skills (${skills.length} relevant)\n\n${formatSkillsForPrompt(skills)}`;
        }),
      );

    const recordUsage = (
      skillId: string,
      success: boolean,
    ): Effect.Effect<void, SkillRetrievalError> =>
      Effect.gen(function* () {
        const skill = yield* store.get(skillId).pipe(
          Effect.mapError(SkillRetrievalError.fromStoreError),
        );

        if (!skill) {
          return; // Skill not found, skip
        }

        // Update usage stats
        const currentRate = skill.successRate ?? 0;
        const currentCount = skill.usageCount ?? 0;
        const newCount = currentCount + 1;

        // Exponential moving average for success rate
        const alpha = 0.2;
        const newRate = alpha * (success ? 1 : 0) + (1 - alpha) * currentRate;

        const updated: Skill = {
          ...skill,
          successRate: newRate,
          usageCount: newCount,
          lastUsed: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };

        yield* store.update(updated).pipe(
          Effect.mapError(SkillRetrievalError.fromStoreError),
        );
      });

    const populateEmbeddings = (): Effect.Effect<number, SkillRetrievalError> =>
      Effect.gen(function* () {
        const skills = yield* store.list({ status: ["active"] }).pipe(
          Effect.mapError(SkillRetrievalError.fromStoreError),
        );

        let populated = 0;

        for (const skill of skills) {
          if (!skill.embedding || skill.embedding.length === 0) {
            const skillEmbedding = yield* embedding.embedSkill(skill).pipe(
              Effect.mapError(SkillRetrievalError.fromEmbeddingError),
            );

            yield* store
              .update({
                ...skill,
                embedding: skillEmbedding,
                updatedAt: new Date().toISOString(),
              })
              .pipe(Effect.mapError(SkillRetrievalError.fromStoreError));

            populated++;
          }
        }

        return populated;
      });

    const getStats = (): Effect.Effect<
      {
        totalSkills: number;
        skillsWithEmbeddings: number;
        averageSuccessRate: number;
      },
      SkillRetrievalError
    > =>
      Effect.gen(function* () {
        const skills = yield* store.list().pipe(
          Effect.mapError(SkillRetrievalError.fromStoreError),
        );

        const withEmbeddings = skills.filter(
          (s) => s.embedding && s.embedding.length > 0,
        ).length;

        const rates = skills
          .filter((s) => s.successRate !== undefined)
          .map((s) => s.successRate!);

        const avgRate =
          rates.length > 0 ? rates.reduce((a, b) => a + b, 0) / rates.length : 0;

        return {
          totalSkills: skills.length,
          skillsWithEmbeddings: withEmbeddings,
          averageSuccessRate: avgRate,
        };
      });

    return {
      query,
      getForTask,
      formatForPrompt: formatForPromptFn,
      recordUsage,
      populateEmbeddings,
      getStats,
    };
  });

// --- Layer ---

/**
 * SkillRetrievalService layer that requires SkillStore and EmbeddingService.
 */
export const SkillRetrievalServiceLive: Layer.Layer<
  SkillRetrievalService,
  never,
  SkillStore | EmbeddingService
> = Layer.effect(SkillRetrievalService, makeRetrievalService());
