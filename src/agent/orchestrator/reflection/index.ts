/**
 * Reflection Module
 *
 * Exports for the Reflexion pattern implementation.
 *
 * Usage:
 * ```typescript
 * import {
 *   ReflectionService,
 *   ReflectionServiceLive,
 *   type Reflection,
 *   type FailureContext
 * } from "./reflection/index.js";
 *
 * // Create the service layer
 * const layer = ReflectionServiceLive({
 *   openagentsDir: ".openagents",
 *   cwd: process.cwd()
 * });
 *
 * // Use in Effect program
 * const program = Effect.gen(function* () {
 *   const service = yield* ReflectionService;
 *   const reflection = yield* service.generate(failureContext);
 *   yield* service.save(reflection);
 * });
 *
 * Effect.runPromise(program.pipe(Effect.provide(layer)));
 * ```
 */

// Schema types
export {
  Reflection,
  FailureContext,
  ReflectionCategory,
  FailureType,
  ReflexionConfig,
  type Reflection as ReflectionType,
  type FailureContext as FailureContextType,
  type ReflectionCategory as ReflectionCategoryType,
  type FailureType as FailureTypeType,
  type ReflexionConfig as ReflexionConfigType,
} from "./schema.js";

// Error types
export { ReflectionError, type ReflectionErrorReason } from "./errors.js";

// Service interface and tag
export { ReflectionService, type IReflectionService } from "./service.js";

// Service implementation
export { makeReflectionService, type ReflectionServiceOptions } from "./service-impl.js";

// Layers
export { ReflectionServiceLive, ReflectionServiceTest, ReflectionServiceTestWithStorage } from "./layer.js";

// Prompt utilities (for direct use if needed)
export { buildGenerationPrompt, formatReflectionsForPrompt, parseReflectionResponse } from "./prompt.js";

// Storage (for testing or custom implementations)
export { makeFileStorage, makeMemoryStorage, type ReflectionStorage } from "./storage.js";

// Generator (for direct use if needed)
export { generateReflection, generateHeuristicReflection, type GeneratorOptions } from "./generator.js";
