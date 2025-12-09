/**
 * TestGen Programmatic API
 *
 * Clean API for programmatic test generation.
 * Used by HillClimber, CI/CD, and other systems.
 */

import { Effect } from "effect";
import { runTestGenWithStreaming, type TestGenEmitter, type TestGenOptions } from "./testgen-service.js";
import { TestGenStore, TestGenStoreLive } from "./testgen-store.js";
import type { TestGenConfig } from "./testgen-types.js";

// ============================================================================
// Simple API
// ============================================================================

/**
 * Simple test generation API.
 * Returns all tests when complete.
 */
export interface GenerateTestsOptions {
  /** Task ID from TB suite */
  taskId: string;
  /** Task description */
  taskDescription?: string;
  /** Environment info (optional, will be built if not provided) */
  environment?: unknown;
  /** Config to use (optional, uses current if not provided) */
  config?: TestGenConfig;
  /** Model override */
  model?: "local" | "claude";
  /** Path to TB2 task directory */
  tb2Path?: string;
}

export interface GenerateTestsResult {
  /** Generated tests */
  tests: Array<{
    id: string;
    category: string;
    input: string;
    expectedOutput: string | null;
    reasoning: string;
    confidence: number;
  }>;
  /** Session ID */
  sessionId: string;
  /** Total tests generated */
  totalTests: number;
  /** Comprehensiveness score (1-10) */
  comprehensivenessScore: number | null;
  /** Total tokens used */
  totalTokensUsed: number;
  /** Duration in milliseconds */
  durationMs: number;
}

/**
 * Generate tests for a task.
 */
export const generateTests = async (
  options: GenerateTestsOptions,
): Promise<GenerateTestsResult> => {
  const suitePath = "tasks/terminal-bench-2.json";
  const sessionId = `tg-api-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  const tests: GenerateTestsResult["tests"] = [];
  let completeMessage: {
    totalTests: number;
    comprehensivenessScore: number | null;
    totalTokensUsed: number;
    durationMs: number;
  } | null = null;

  const emitter: TestGenEmitter = {
    onStart: () => {},
    onTest: (msg) => {
      tests.push(msg.test);
    },
    onProgress: () => {},
    onReflection: () => {},
    onComplete: (msg) => {
      completeMessage = {
        totalTests: msg.totalTests,
        comprehensivenessScore: msg.comprehensivenessScore,
        totalTokensUsed: msg.totalTokensUsed,
        durationMs: msg.durationMs,
      };
    },
    onError: (msg) => {
      throw new Error(`Test generation failed: ${msg.error}`);
    },
  };

  const genOptions: TestGenOptions = {
    model: options.model || "local",
  };
  if (options.tb2Path) {
    genOptions.tb2Path = options.tb2Path;
  }

  await runTestGenWithStreaming(
    suitePath,
    options.taskId,
    sessionId,
    emitter,
    genOptions,
  );

  if (!completeMessage) {
    throw new Error("Test generation did not complete");
  }

  // TypeScript needs explicit type assertion after null check
  const msg: {
    totalTests: number;
    comprehensivenessScore: number | null;
    totalTokensUsed: number;
    durationMs: number;
  } = completeMessage;
  
  return {
    tests,
    sessionId,
    totalTests: msg.totalTests,
    comprehensivenessScore: msg.comprehensivenessScore,
    totalTokensUsed: msg.totalTokensUsed,
    durationMs: msg.durationMs,
  };
};

// ============================================================================
// Advanced API with Callbacks
// ============================================================================

/**
 * Advanced test generation API with callbacks.
 */
export interface GenerateTestsWithCallbacksOptions extends GenerateTestsOptions {
  /** Called when each test is generated */
  onTest?: (test: GenerateTestsResult["tests"][0]) => void;
  /** Called for progress updates */
  onProgress?: (progress: {
    phase: "category_generation" | "global_refinement";
    currentCategory?: string;
    roundNumber: number;
    status: string;
  }) => void;
  /** Called for reflection/gap analysis */
  onReflection?: (reflection: {
    category?: string;
    reflectionText: string;
    action: "refining" | "assessing" | "complete";
  }) => void;
  /** Called when generation completes */
  onComplete?: (result: GenerateTestsResult) => void;
}

/**
 * Generate tests with callbacks for streaming updates.
 */
export const generateTestsWithCallbacks = async (
  options: GenerateTestsWithCallbacksOptions,
): Promise<GenerateTestsResult> => {
  const suitePath = "tasks/terminal-bench-2.json";
  const sessionId = `tg-api-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  const tests: GenerateTestsResult["tests"] = [];
  let completeMessage: GenerateTestsResult | null = null;

  const emitter: TestGenEmitter = {
    onStart: () => {},
    onTest: (msg) => {
      tests.push(msg.test);
      options.onTest?.(msg.test);
    },
    onProgress: (msg) => {
      const progress: {
        phase: "category_generation" | "global_refinement";
        currentCategory?: string;
        roundNumber: number;
        status: string;
      } = {
        phase: msg.phase,
        roundNumber: msg.roundNumber,
        status: msg.status,
      };
      if (msg.currentCategory !== undefined) {
        progress.currentCategory = msg.currentCategory;
      }
      options.onProgress?.(progress);
    },
    onReflection: (msg) => {
      const reflection: {
        category?: string;
        reflectionText: string;
        action: "refining" | "assessing" | "complete";
      } = {
        reflectionText: msg.reflectionText,
        action: msg.action,
      };
      if (msg.category !== undefined) {
        reflection.category = msg.category;
      }
      options.onReflection?.(reflection);
    },
    onComplete: (msg) => {
      completeMessage = {
        tests,
        sessionId,
        totalTests: msg.totalTests,
        comprehensivenessScore: msg.comprehensivenessScore,
        totalTokensUsed: msg.totalTokensUsed,
        durationMs: msg.durationMs,
      };
      options.onComplete?.(completeMessage);
    },
    onError: (msg) => {
      throw new Error(`Test generation failed: ${msg.error}`);
    },
  };

  const genOptions: TestGenOptions = {
    model: options.model || "local",
  };
  if (options.tb2Path) {
    genOptions.tb2Path = options.tb2Path;
  }

  await runTestGenWithStreaming(
    suitePath,
    options.taskId,
    sessionId,
    emitter,
    genOptions,
  );

  if (!completeMessage) {
    throw new Error("Test generation did not complete");
  }

  return completeMessage;
};

// ============================================================================
// Config API
// ============================================================================

/**
 * Get current test generation config.
 */
export const getCurrentConfig = async (
  taskType?: string,
): Promise<TestGenConfig | null> => {
  return await Effect.runPromise(
    TestGenStore.pipe(
      Effect.flatMap((store) => store.getCurrentConfig(taskType)),
      Effect.provide(TestGenStoreLive),
    )
  );
};

/**
 * Ensure default config exists.
 */
export const ensureDefaultConfig = async (): Promise<TestGenConfig> => {
  return await Effect.runPromise(
    TestGenStore.pipe(
      Effect.flatMap((store) => store.ensureDefaultConfig()),
      Effect.provide(TestGenStoreLive),
    )
  );
};
