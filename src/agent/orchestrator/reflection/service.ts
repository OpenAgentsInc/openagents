/**
 * ReflectionService Interface
 *
 * Effect service interface for the Reflexion pattern.
 * Uses Context.Tag for dependency injection.
 */
import { Context, Effect } from "effect";
import type { Reflection, FailureContext } from "./schema.js";
import type { ReflectionError } from "./errors.js";

// ============================================================================
// Service Interface
// ============================================================================

/**
 * Service interface for generating, storing, and retrieving reflections.
 */
export interface IReflectionService {
  /**
   * Generate a reflection from a failure context.
   * Uses LLM to analyze the failure and suggest improvements.
   */
  generate(failure: FailureContext): Effect.Effect<Reflection, ReflectionError>;

  /**
   * Get recent reflections for a subtask, most recent first.
   * @param subtaskId - The subtask to get reflections for
   * @param limit - Maximum number to return (default: 3)
   */
  getRecent(subtaskId: string, limit?: number): Effect.Effect<Reflection[], ReflectionError>;

  /**
   * Get all reflections for a task.
   * @param taskId - The task to get reflections for
   */
  getForTask(taskId: string): Effect.Effect<Reflection[], ReflectionError>;

  /**
   * Save a reflection to storage.
   */
  save(reflection: Reflection): Effect.Effect<void, ReflectionError>;

  /**
   * Format reflections for injection into a subagent prompt.
   * Returns markdown-formatted context.
   */
  formatForPrompt(reflections: Reflection[]): Effect.Effect<string, ReflectionError>;

  /**
   * Prune old reflections from storage.
   * @param maxAgeMs - Maximum age in milliseconds
   * @returns Number of reflections pruned
   */
  prune(maxAgeMs: number): Effect.Effect<number, ReflectionError>;
}

// ============================================================================
// Context Tag
// ============================================================================

/**
 * Context.Tag for ReflectionService dependency injection.
 *
 * Usage:
 * ```typescript
 * const program = Effect.gen(function* () {
 *   const reflectionService = yield* ReflectionService;
 *   const reflection = yield* reflectionService.generate(failureContext);
 * });
 * ```
 */
export class ReflectionService extends Context.Tag("ReflectionService")<
  ReflectionService,
  IReflectionService
>() {}
