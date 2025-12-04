/**
 * ReflectionService Layers
 *
 * Effect Layer definitions for dependency injection.
 */
import { Effect, Layer } from "effect";
import type { Reflection } from "./schema.js";
import { ReflectionService, type IReflectionService } from "./service.js";
import { makeReflectionService, type ReflectionServiceOptions } from "./service-impl.js";
import { makeMemoryStorage } from "./storage.js";

// ============================================================================
// Live Layer
// ============================================================================

/**
 * Create a live ReflectionService layer.
 *
 * Usage:
 * ```typescript
 * const layer = ReflectionServiceLive({
 *   openagentsDir: ".openagents",
 *   cwd: process.cwd(),
 *   config: { enabled: true, maxReflectionsPerRetry: 3 }
 * });
 *
 * const program = Effect.gen(function* () {
 *   const service = yield* ReflectionService;
 *   // ...
 * });
 *
 * Effect.runPromise(program.pipe(Effect.provide(layer)));
 * ```
 */
export const ReflectionServiceLive = (options: ReflectionServiceOptions) =>
  Layer.succeed(ReflectionService, makeReflectionService(options));

// ============================================================================
// Test Layer
// ============================================================================

/**
 * Mock reflection for testing.
 */
const mockReflection: Reflection = {
  id: "ref-test-mock",
  sessionId: "sess-test",
  taskId: "oa-test",
  subtaskId: "sub-test",
  attemptNumber: 1,
  category: "root_cause",
  analysis: "Mock reflection analysis for testing",
  suggestion: "Mock suggestion for testing",
  actionItems: ["Action 1", "Action 2"],
  confidence: 0.8,
  createdAt: new Date().toISOString(),
};

/**
 * Create a test ReflectionService layer with in-memory storage.
 *
 * Usage in tests:
 * ```typescript
 * const layer = ReflectionServiceTest;
 *
 * const program = Effect.gen(function* () {
 *   const service = yield* ReflectionService;
 *   // ...
 * });
 *
 * Effect.runPromise(program.pipe(Effect.provide(layer)));
 * ```
 */
const mockService: IReflectionService = {
  generate: () => Effect.succeed(mockReflection),
  getRecent: () => Effect.succeed([]),
  getForTask: () => Effect.succeed([]),
  save: () => Effect.void,
  formatForPrompt: () => Effect.succeed(""),
  prune: () => Effect.succeed(0),
};

export const ReflectionServiceTest = Layer.succeed(ReflectionService, mockService);

/**
 * Create a test layer with in-memory storage that actually stores reflections.
 * Useful for integration tests.
 */
export const ReflectionServiceTestWithStorage = () => {
  const storage = makeMemoryStorage();

  const service = makeReflectionService({
    openagentsDir: "/tmp/test-openagents",
    cwd: "/tmp/test",
    storage,
    config: { enabled: true, maxReflectionsPerRetry: 3, generationTimeoutMs: 5000 },
  });

  return {
    layer: Layer.succeed(ReflectionService, service),
    storage, // Expose storage for assertions
  };
};
