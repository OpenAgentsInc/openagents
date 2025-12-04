/**
 * Reflection Generator
 *
 * Uses Claude Code to generate reflections from failure contexts.
 */
import { Effect } from "effect";
import type { Reflection, FailureContext, ReflectionCategory } from "./schema.js";
import { ReflectionError } from "./errors.js";
import { buildGenerationPrompt, parseReflectionResponse } from "./prompt.js";
import { runClaudeCodeSubagent } from "../claude-code-subagent.js";
import type { Subtask } from "../types.js";

// ============================================================================
// Generator Interface
// ============================================================================

/**
 * Options for reflection generation.
 */
export interface GeneratorOptions {
  /** Working directory */
  cwd: string;
  /** Path to .openagents directory */
  openagentsDir: string;
  /** Timeout for generation in ms (default: 30000) */
  timeoutMs?: number;
  /** Abort signal */
  signal?: AbortSignal;
}

/**
 * Generate a reflection using Claude Code.
 */
export const generateReflection = (
  ctx: FailureContext,
  options: GeneratorOptions
): Effect.Effect<Reflection, ReflectionError> =>
  Effect.gen(function* () {
    const prompt = buildGenerationPrompt(ctx);

    // Create a minimal subtask for the reflection generation
    const reflectionSubtask: Subtask = {
      id: `${ctx.subtaskId}-reflection-${ctx.attemptNumber}`,
      description: prompt,
      status: "in_progress",
      startedAt: new Date().toISOString(),
      failureCount: 0,
    };

    // Run Claude Code with minimal configuration
    const result = yield* Effect.tryPromise({
      try: () =>
        runClaudeCodeSubagent(reflectionSubtask, {
          cwd: options.cwd,
          openagentsDir: options.openagentsDir,
          maxTurns: 1, // Single turn for reflection
          timeoutMs: options.timeoutMs ?? 30000,
          permissionMode: "plan", // Read-only mode
          ...(options.signal ? { signal: options.signal } : {}),
        }),
      catch: (e) => ReflectionError.generationFailed(`Claude Code invocation failed: ${e}`),
    });

    // Extract response from session metadata
    const response = result.sessionMetadata?.summary;
    if (!response) {
      return yield* Effect.fail(ReflectionError.generationFailed("No response from Claude Code"));
    }

    // Parse the JSON response
    const parsed = parseReflectionResponse(response);
    if (!parsed) {
      return yield* Effect.fail(ReflectionError.parseError(`Failed to parse reflection response: ${response.slice(0, 200)}`));
    }

    // Validate category
    const validCategories: ReflectionCategory[] = [
      "root_cause",
      "misconception",
      "environment",
      "approach_error",
      "edge_case",
      "verification",
    ];
    const category = validCategories.includes(parsed.category as ReflectionCategory)
      ? (parsed.category as ReflectionCategory)
      : "root_cause";

    // Build the reflection
    const reflection: Reflection = {
      id: `ref-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
      sessionId: ctx.sessionId,
      taskId: ctx.taskId,
      subtaskId: ctx.subtaskId,
      attemptNumber: ctx.attemptNumber,
      category,
      analysis: parsed.analysis.slice(0, 500),
      suggestion: parsed.suggestion.slice(0, 500),
      actionItems: parsed.actionItems.slice(0, 5).map((a) => a.slice(0, 200)),
      confidence: Math.max(0, Math.min(1, parsed.confidence)),
      createdAt: new Date().toISOString(),
    };

    return reflection;
  });

// ============================================================================
// Fallback Heuristic Generator
// ============================================================================

/**
 * Generate a basic reflection from error patterns (fallback when LLM fails).
 */
export const generateHeuristicReflection = (ctx: FailureContext): Reflection => {
  // Detect common patterns in error output
  const errorLower = ctx.errorOutput.toLowerCase();

  let category: ReflectionCategory = "root_cause";
  let analysis = "The subtask failed with an error.";
  let suggestion = "Review the error output and try a different approach.";
  const actionItems: string[] = [];

  // Type errors
  if (errorLower.includes("type") && (errorLower.includes("error") || errorLower.includes("ts("))) {
    category = "root_cause";
    analysis = "TypeScript type errors were detected in the code.";
    suggestion = "Check type annotations and ensure they match the expected types.";
    actionItems.push("Run bun run typecheck to see all type errors");
    actionItems.push("Check function signatures and return types");
  }
  // Test failures
  else if (errorLower.includes("test") && (errorLower.includes("fail") || errorLower.includes("expect"))) {
    category = "verification";
    analysis = "Tests are failing, likely due to incorrect implementation or stale fixtures.";
    suggestion = "Review the test expectations and ensure the implementation matches.";
    actionItems.push("Run bun test to see detailed test output");
    actionItems.push("Check if fixtures/mocks need updating");
  }
  // Import/module errors
  else if (errorLower.includes("cannot find module") || errorLower.includes("import")) {
    category = "environment";
    analysis = "Module import errors detected - missing dependency or incorrect path.";
    suggestion = "Verify import paths and ensure dependencies are installed.";
    actionItems.push("Check import paths for typos");
    actionItems.push("Run bun install if dependencies are missing");
  }
  // Runtime errors
  else if (errorLower.includes("runtime") || errorLower.includes("undefined") || errorLower.includes("null")) {
    category = "edge_case";
    analysis = "Runtime error occurred, possibly due to null/undefined values.";
    suggestion = "Add null checks and handle edge cases.";
    actionItems.push("Add defensive null/undefined checks");
    actionItems.push("Review the data flow for potential null values");
  }
  // Timeout
  else if (ctx.failureType === "timeout") {
    category = "approach_error";
    analysis = "The operation timed out, likely due to long-running or infinite loop.";
    suggestion = "Optimize the approach or add early termination conditions.";
    actionItems.push("Check for infinite loops or unbounded iterations");
    actionItems.push("Consider breaking the task into smaller steps");
  }

  return {
    id: `ref-heuristic-${Date.now().toString(36)}`,
    sessionId: ctx.sessionId,
    taskId: ctx.taskId,
    subtaskId: ctx.subtaskId,
    attemptNumber: ctx.attemptNumber,
    category,
    analysis,
    suggestion,
    actionItems,
    confidence: 0.5, // Lower confidence for heuristic
    createdAt: new Date().toISOString(),
  };
};
