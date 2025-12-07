/**
 * Skill Library Service
 *
 * Main service interface for the skill library.
 * Combines store, embedding, and retrieval into a unified API.
 */

import { Effect, Context, Layer } from "effect";
import { SkillStore, makeSkillStoreLayer, type SkillStoreError } from "./store.js";
import {
  SkillRetrievalService,
  SkillRetrievalServiceLive,
  type SkillRetrievalError,
} from "./retrieval.js";
import {
  type Skill,
  type SkillQuery,
  type SkillMatch,
  type SkillFilter,
  createSkill,
} from "./schema.js";
import {
  EmbeddingError,
  EmbeddingServiceLive,
} from "./embedding.js";
import { makeFMServiceLayer } from "../fm/service.js";

// --- Deduplication Helpers ---

/**
 * Normalize code for comparison (remove whitespace, comments).
 */
const normalizeCode = (code: string): string => {
  return code
    .replace(/\/\/.*$/gm, "") // Remove single-line comments
    .replace(/\/\*[\s\S]*?\*\//g, "") // Remove multi-line comments
    .replace(/\s+/g, " ") // Normalize whitespace
    .trim()
    .toLowerCase();
};

/**
 * Check if two skills are content-duplicates.
 * Returns true if code is substantially similar.
 */
const areSkillsSimilar = (a: Skill, b: Skill): boolean => {
  const codeA = normalizeCode(a.code);
  const codeB = normalizeCode(b.code);

  // Exact match after normalization
  if (codeA === codeB) return true;

  // Check if one contains the other (substring match)
  if (codeA.length > 50 && codeB.length > 50) {
    if (codeA.includes(codeB) || codeB.includes(codeA)) return true;
  }

  // Same category and similar description (fuzzy match)
  if (a.category === b.category) {
    const descA = a.description.toLowerCase();
    const descB = b.description.toLowerCase();

    // Extract key words (remove common words)
    const commonWords = new Set(["the", "a", "an", "to", "for", "in", "on", "at", "and", "or", "is", "are"]);
    const wordsA = descA.split(/\s+/).filter(w => w.length > 3 && !commonWords.has(w));
    const wordsB = descB.split(/\s+/).filter(w => w.length > 3 && !commonWords.has(w));

    // Check word overlap
    const setB = new Set(wordsB);
    const overlap = wordsA.filter(w => setB.has(w)).length;
    const maxLen = Math.max(wordsA.length, wordsB.length);

    if (maxLen > 0 && overlap / maxLen > 0.6) {
      return true;
    }
  }

  return false;
};

// --- Unified Error Type ---

export class SkillServiceError extends Error {
  readonly _tag = "SkillServiceError";
  constructor(
    readonly reason: string,
    override readonly message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "SkillServiceError";
  }

  static from(e: SkillStoreError | SkillRetrievalError | EmbeddingError): SkillServiceError {
    return new SkillServiceError(e._tag, e.message, e);
  }
}

// --- Service Interface ---

export interface ISkillService {
  // --- Skill Management ---

  /** Register a new skill */
  readonly registerSkill: (skill: Skill) => Effect.Effect<void, SkillServiceError>;

  /** Create and register a skill from partial data */
  readonly createSkill: (
    partial: Partial<Skill> & Pick<Skill, "name" | "description" | "code" | "category">,
  ) => Effect.Effect<Skill, SkillServiceError>;

  /** Get a skill by ID */
  readonly getSkill: (id: string) => Effect.Effect<Skill | null, SkillServiceError>;

  /** List skills with optional filter */
  readonly listSkills: (filter?: SkillFilter) => Effect.Effect<Skill[], SkillServiceError>;

  /** Update an existing skill */
  readonly updateSkill: (skill: Skill) => Effect.Effect<void, SkillServiceError>;

  /** Archive a skill */
  readonly archiveSkill: (id: string) => Effect.Effect<void, SkillServiceError>;

  /** Delete a skill (alias for archive) */
  readonly deleteSkill: (id: string) => Effect.Effect<void, SkillServiceError>;

  /** Get all skills (alias for listSkills with no filter) */
  readonly getAllSkills: () => Effect.Effect<Skill[], SkillServiceError>;

  // --- Skill Retrieval ---

  /** Query skills using semantic search */
  readonly query: (query: SkillQuery) => Effect.Effect<SkillMatch[], SkillServiceError>;

  /** Get skills relevant to a task description */
  readonly selectSkills: (
    taskDescription: string,
    options?: {
      topK?: number;
      minSimilarity?: number;
      filter?: SkillFilter;
    },
  ) => Effect.Effect<Skill[], SkillServiceError>;

  /** Format skills for prompt injection */
  readonly formatForPrompt: (
    taskDescription: string,
    options?: {
      topK?: number;
      minSimilarity?: number;
      filter?: SkillFilter;
    },
  ) => Effect.Effect<string, SkillServiceError>;

  // --- Stats & Tracking ---

  /** Record skill usage */
  readonly recordUsage: (skillId: string, success: boolean) => Effect.Effect<void, SkillServiceError>;

  /** Update skill success rate */
  readonly updateStats: (
    skillId: string,
    success: boolean,
  ) => Effect.Effect<void, SkillServiceError>;

  /** Get library statistics */
  readonly getStats: () => Effect.Effect<
    {
      totalSkills: number;
      skillsWithEmbeddings: number;
      averageSuccessRate: number;
      byCategory: Record<string, number>;
    },
    SkillServiceError
  >;

  // --- Maintenance ---

  /** Populate embeddings for all skills */
  readonly populateEmbeddings: () => Effect.Effect<number, SkillServiceError>;

  /** Prune low-performing skills (archive skills with low success rate) */
  readonly pruneSkills: (
    options?: {
      minSuccessRate?: number;
      minUsageCount?: number;
    },
  ) => Effect.Effect<number, SkillServiceError>;

  /** Get skill count */
  readonly count: () => Effect.Effect<number, SkillServiceError>;
}

// --- Service Tag ---

export class SkillService extends Context.Tag("SkillService")<SkillService, ISkillService>() {}

// --- Implementation ---

const makeSkillService = (): Effect.Effect<
  ISkillService,
  never,
  SkillStore | SkillRetrievalService
> =>
  Effect.gen(function* () {
    const store = yield* SkillStore;
    const retrieval = yield* SkillRetrievalService;

    const mapStoreError = Effect.mapError(SkillServiceError.from);
    const mapRetrievalError = Effect.mapError(SkillServiceError.from);

    const registerSkill = (skill: Skill): Effect.Effect<void, SkillServiceError> =>
      Effect.gen(function* () {
        // Check for content duplicates before adding
        const existingSkills = yield* store.list({ status: ["active"] }).pipe(mapStoreError);

        // Find similar skill
        const similarSkill = existingSkills.find((existing) => areSkillsSimilar(existing, skill));

        if (similarSkill) {
          // Merge: update existing skill's stats instead of creating duplicate
          const merged: Skill = {
            ...similarSkill,
            // Combine usage stats
            usageCount: (similarSkill.usageCount ?? 0) + (skill.usageCount ?? 0),
            // Average success rates
            successRate:
              similarSkill.successRate !== undefined && skill.successRate !== undefined
                ? (similarSkill.successRate + skill.successRate) / 2
                : similarSkill.successRate ?? skill.successRate,
            // Merge tags
            tags: [...new Set([...(similarSkill.tags ?? []), ...(skill.tags ?? [])])],
            // Merge learnedFrom
            learnedFrom: [...new Set([...(similarSkill.learnedFrom ?? []), ...(skill.learnedFrom ?? [])])],
            // Update timestamp
            updatedAt: new Date().toISOString(),
          };

          yield* store.update(merged).pipe(mapStoreError);
          return;
        }

        // No duplicate found, add new skill
        yield* store.add(skill).pipe(mapStoreError);
      });

    const createSkillFn = (
      partial: Partial<Skill> & Pick<Skill, "name" | "description" | "code" | "category">,
    ): Effect.Effect<Skill, SkillServiceError> =>
      Effect.gen(function* () {
        const skill = createSkill(partial);
        yield* store.add(skill).pipe(mapStoreError);
        return skill;
      });

    const getSkill = (id: string): Effect.Effect<Skill | null, SkillServiceError> =>
      store.get(id).pipe(mapStoreError);

    const listSkills = (filter?: SkillFilter): Effect.Effect<Skill[], SkillServiceError> =>
      store.list(filter).pipe(mapStoreError);

    const updateSkill = (skill: Skill): Effect.Effect<void, SkillServiceError> =>
      store.update(skill).pipe(mapStoreError);

    const archiveSkill = (id: string): Effect.Effect<void, SkillServiceError> =>
      store.archive(id).pipe(mapStoreError);

    // Alias for archive (used by Archivist)
    const deleteSkill = archiveSkill;

    // Alias for listSkills with no filter (used by Archivist)
    const getAllSkills = (): Effect.Effect<Skill[], SkillServiceError> => listSkills();

    const query = (q: SkillQuery): Effect.Effect<SkillMatch[], SkillServiceError> =>
      retrieval.query(q).pipe(mapRetrievalError);

    const selectSkills = (
      taskDescription: string,
      options?: {
        topK?: number;
        minSimilarity?: number;
        filter?: SkillFilter;
      },
    ): Effect.Effect<Skill[], SkillServiceError> =>
      retrieval.getForTask(taskDescription, options).pipe(mapRetrievalError);

    const formatForPromptFn = (
      taskDescription: string,
      options?: {
        topK?: number;
        minSimilarity?: number;
        filter?: SkillFilter;
      },
    ): Effect.Effect<string, SkillServiceError> =>
      retrieval.formatForPrompt(taskDescription, options).pipe(mapRetrievalError);

    const recordUsage = (
      skillId: string,
      success: boolean,
    ): Effect.Effect<void, SkillServiceError> =>
      retrieval.recordUsage(skillId, success).pipe(mapRetrievalError);

    const updateStats = recordUsage; // Alias

    const getStats = (): Effect.Effect<
      {
        totalSkills: number;
        skillsWithEmbeddings: number;
        averageSuccessRate: number;
        byCategory: Record<string, number>;
      },
      SkillServiceError
    > =>
      Effect.gen(function* () {
        const baseStats = yield* retrieval.getStats().pipe(mapRetrievalError);
        const skills = yield* store.list().pipe(mapStoreError);

        // Count by category
        const byCategory: Record<string, number> = {};
        for (const skill of skills) {
          byCategory[skill.category] = (byCategory[skill.category] ?? 0) + 1;
        }

        return { ...baseStats, byCategory };
      });

    const populateEmbeddings = (): Effect.Effect<number, SkillServiceError> =>
      retrieval.populateEmbeddings().pipe(mapRetrievalError);

    const pruneSkills = (
      options?: {
        minSuccessRate?: number;
        minUsageCount?: number;
      },
    ): Effect.Effect<number, SkillServiceError> =>
      Effect.gen(function* () {
        const minRate = options?.minSuccessRate ?? 0.2;
        const minCount = options?.minUsageCount ?? 5;

        const skills = yield* store.list({ status: ["active"] }).pipe(mapStoreError);

        let pruned = 0;
        for (const skill of skills) {
          const usageCount = skill.usageCount ?? 0;
          const successRate = skill.successRate ?? 1; // Default to 1 if never used

          // Only prune skills that have been used enough to have reliable stats
          if (usageCount >= minCount && successRate < minRate) {
            yield* store.archive(skill.id).pipe(mapStoreError);
            pruned++;
          }
        }

        return pruned;
      });

    const countFn = (): Effect.Effect<number, SkillServiceError> =>
      store.count().pipe(Effect.map((n) => n));

    return {
      registerSkill,
      createSkill: createSkillFn,
      getSkill,
      listSkills,
      updateSkill,
      archiveSkill,
      deleteSkill,
      getAllSkills,
      query,
      selectSkills,
      formatForPrompt: formatForPromptFn,
      recordUsage,
      updateStats,
      getStats,
      populateEmbeddings,
      pruneSkills,
      count: countFn,
    };
  });

// --- Layer ---

/**
 * SkillService layer that requires SkillStore and SkillRetrievalService.
 */
export const SkillServiceLayer: Layer.Layer<
  SkillService,
  never,
  SkillStore | SkillRetrievalService
> = Layer.effect(SkillService, makeSkillService());

/**
 * Create a complete SkillService layer with all dependencies.
 */
export const makeSkillServiceLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<SkillService, SkillStoreError, never> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  const storeLayer = makeSkillStoreLayer(projectRoot);
  const embeddingLayer = Layer.provide(EmbeddingServiceLive, fmLayer);
  const retrievalLayer = Layer.provide(
    SkillRetrievalServiceLive,
    Layer.merge(storeLayer, embeddingLayer),
  );
  return Layer.provide(SkillServiceLayer, Layer.merge(storeLayer, retrievalLayer)) as Layer.Layer<SkillService, SkillStoreError, never>;
};

/**
 * Default SkillService layer using current working directory.
 */
export const SkillServiceLive: Layer.Layer<SkillService, SkillStoreError, never> =
  makeSkillServiceLive();
