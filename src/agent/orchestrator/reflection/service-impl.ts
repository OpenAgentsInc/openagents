/**
 * ReflectionService Implementation
 *
 * Implements the ReflectionService interface using storage and generator.
 */
import { Effect } from "effect";
import type { Reflection, FailureContext, ReflexionConfig } from "./schema.js";
import type { IReflectionService } from "./service.js";
import { makeFileStorage, type ReflectionStorage } from "./storage.js";
import { generateReflection, generateHeuristicReflection } from "./generator.js";
import { formatReflectionsForPrompt } from "./prompt.js";

// ============================================================================
// Service Factory
// ============================================================================

/**
 * Options for creating a ReflectionService.
 */
export interface ReflectionServiceOptions {
  /** Path to .openagents directory */
  openagentsDir: string;
  /** Working directory */
  cwd: string;
  /** Reflexion configuration */
  config?: ReflexionConfig;
  /** Optional custom storage (for testing) */
  storage?: ReflectionStorage;
  /** Abort signal */
  signal?: AbortSignal;
}

/**
 * Create a ReflectionService implementation.
 */
export const makeReflectionService = (options: ReflectionServiceOptions): IReflectionService => {
  const storage = options.storage ?? makeFileStorage(options.openagentsDir);
  const config = options.config ?? { enabled: true, maxReflectionsPerRetry: 3, generationTimeoutMs: 30000 };

  return {
    generate: (failure: FailureContext) =>
      Effect.gen(function* () {
        // Try LLM generation first
        const reflection = yield* generateReflection(failure, {
          cwd: options.cwd,
          openagentsDir: options.openagentsDir,
          timeoutMs: config.generationTimeoutMs,
          ...(options.signal ? { signal: options.signal } : {}),
        }).pipe(
          Effect.catchAll((error) => {
            // Log the error and fall back to heuristic
            console.log(`[Reflexion] LLM generation failed, using heuristic: ${error.message}`);
            return Effect.succeed(generateHeuristicReflection(failure));
          })
        );

        return reflection;
      }),

    getRecent: (subtaskId: string, limit?: number) =>
      Effect.gen(function* () {
        const all = yield* storage.loadBySubtask(subtaskId);
        // Sort by attemptNumber descending (most recent first)
        const sorted = all.sort((a, b) => b.attemptNumber - a.attemptNumber);
        // Limit to max reflections
        const maxLimit = config.maxReflectionsPerRetry ?? 3;
        return sorted.slice(0, limit ?? maxLimit);
      }),

    getForTask: (taskId: string) =>
      Effect.gen(function* () {
        const all = yield* storage.loadByTask(taskId);
        // Sort by createdAt descending
        return all.sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());
      }),

    save: (reflection: Reflection) =>
      Effect.gen(function* () {
        yield* storage.save(reflection);
      }),

    formatForPrompt: (reflections: Reflection[]) =>
      Effect.succeed(formatReflectionsForPrompt(reflections)),

    prune: (maxAgeMs: number) =>
      Effect.gen(function* () {
        return yield* storage.prune(maxAgeMs);
      }),
  };
};
