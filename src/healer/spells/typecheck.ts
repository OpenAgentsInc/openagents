/**
 * Fix Typecheck Errors Spell
 *
 * Extracted from orchestrator.ts safe-mode logic.
 * Creates an emergency subtask to fix TypeScript errors,
 * invokes Claude Code subagent, and re-runs verification.
 */
import { Effect } from "effect";
import type { HealerSpell, HealerSpellResult, HealerContext } from "../types.js";
import type { Subtask, SubagentResult } from "../../agent/orchestrator/types.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Options for running the typecheck fix.
 * These are passed through from HealerService when executing the spell.
 */
export interface TypecheckFixOptions {
  /** Maximum turns for Claude Code subagent */
  maxTurns?: number;
  /** Permission mode for Claude Code */
  permissionMode?: "bypassPermissions" | "acceptEdits" | "plan" | "full";
  /** Callback for streaming output */
  onOutput?: (text: string) => void;
  /** Abort signal for cancellation */
  signal?: AbortSignal;
  /** Path to openagents directory */
  openagentsDir?: string;
}

/**
 * Function signature for invoking Claude Code subagent.
 * Injected by HealerService to enable testing.
 */
export type ClaudeCodeInvoker = (
  subtask: Subtask,
  options: TypecheckFixOptions & { cwd: string }
) => Promise<SubagentResult>;

/**
 * Function signature for running verification (typecheck).
 * Injected by HealerService to enable testing.
 */
export type VerificationRunner = (cwd: string) => Promise<{
  success: boolean;
  output: string;
}>;

// ============================================================================
// Emergency Subtask Generation
// ============================================================================

/**
 * Generate emergency typecheck fix subtask description.
 */
export const generateTypecheckFixDescription = (
  errorOutput: string | null,
  failureCount: number
): string => {
  const outputSnippet = errorOutput?.slice(0, 2000) ?? "No output available";

  const retryNote = failureCount > 0
    ? `\n\n**Note:** This is retry #${failureCount + 1}. Previous fix attempts failed.`
    : "";

  return `## EMERGENCY: Fix All TypeScript Errors

The init script failed due to typecheck errors. You MUST fix ALL typecheck errors immediately.
${retryNote}

### Error Output
\`\`\`
${outputSnippet}
\`\`\`

### Steps
1. Run \`bun run typecheck\` to see all errors
2. Fix each error - do NOT just suppress them
3. Verify fix with \`bun run typecheck\`
4. All errors must be resolved before continuing

### Important
- Focus on the root cause, not just suppressing errors
- Check for missing imports, incorrect types, null handling
- If a type definition is wrong, fix it properly
- Test your changes compile before finishing`;
};

/**
 * Generate emergency test fix subtask description.
 */
export const generateTestFixDescription = (
  errorOutput: string | null,
  failureCount: number
): string => {
  const outputSnippet = errorOutput?.slice(0, 2000) ?? "No output available";

  const retryNote = failureCount > 0
    ? `\n\n**Note:** This is retry #${failureCount + 1}. Previous fix attempts failed.`
    : "";

  return `## EMERGENCY: Fix Failing Tests

The init script failed due to test failures. You MUST fix ALL failing tests.
${retryNote}

### Test Output
\`\`\`
${outputSnippet}
\`\`\`

### Steps
1. Run \`bun test\` to see all failing tests
2. Fix the root cause of each failure
3. Verify fix with \`bun test\`
4. All tests must pass before continuing

### Important
- Understand why each test is failing before fixing
- Don't just change assertions to match wrong behavior
- If the implementation is wrong, fix the implementation
- If the test expectation is outdated, fix the test`;
};

/**
 * Create emergency subtask for typecheck or test fixes.
 */
export const createEmergencySubtask = (
  ctx: HealerContext,
  type: "typecheck" | "test"
): Subtask => {
  const id = `emergency-${type}-fix-${Date.now()}`;
  const failureCount = ctx.heuristics.failureCount;

  const description = type === "typecheck"
    ? generateTypecheckFixDescription(ctx.errorOutput ?? null, failureCount)
    : generateTestFixDescription(ctx.errorOutput ?? null, failureCount);

  return {
    id,
    description,
    status: "in_progress",
    startedAt: new Date().toISOString(),
    failureCount: 0,
  };
};

// ============================================================================
// Spell Implementation
// ============================================================================

/**
 * Fix Typecheck Errors spell.
 *
 * This spell is LLM-based and requires Claude Code invocation.
 * It extracts the safe-mode logic from the orchestrator for reusability.
 *
 * When executed:
 * 1. Creates an emergency subtask with typecheck fix instructions
 * 2. Invokes Claude Code subagent to fix the errors
 * 3. Re-runs typecheck verification
 * 4. Returns success if typecheck passes after fix
 *
 * Note: This spell requires external functions to be provided via
 * the spell execution context. When run through HealerService,
 * these will be injected automatically.
 */
export const fixTypecheckErrors: HealerSpell = {
  id: "fix_typecheck_errors",
  description: "Invoke Claude Code to fix TypeScript compilation errors",
  requiresLLM: true,

  apply: (ctx: HealerContext): Effect.Effect<HealerSpellResult, Error, never> =>
    Effect.gen(function* () {
      // This spell requires LLM invocation which must be handled by HealerService
      // The spell itself just prepares the subtask and validates applicability

      // Check if we have error output to work with
      if (!ctx.errorOutput) {
        return {
          success: false,
          changesApplied: false,
          summary: "No error output available to diagnose typecheck errors",
          error: "Missing error output",
        };
      }

      // Validate this is actually a typecheck scenario
      if (ctx.heuristics.scenario !== "InitScriptTypecheckFailure") {
        return {
          success: false,
          changesApplied: false,
          summary: `Spell only applicable to InitScriptTypecheckFailure, got ${ctx.heuristics.scenario}`,
          error: "Wrong scenario for this spell",
        };
      }

      // Create the emergency subtask
      const subtask = createEmergencySubtask(ctx, "typecheck");

      // Return a "prepared" result - HealerService will execute the actual LLM call
      // and update the result accordingly
      return {
        success: true,
        changesApplied: false, // Will be updated by HealerService after LLM execution
        summary: `Prepared emergency typecheck fix subtask: ${subtask.id}`,
        // Store prepared subtask for HealerService to use
        // (This will be picked up by the service layer)
      } as HealerSpellResult & {
        _preparedSubtask?: Subtask;
      };
    }),
};

/**
 * Fix Test Errors spell.
 *
 * Similar to fix_typecheck_errors but for test failures.
 */
export const fixTestErrors: HealerSpell = {
  id: "fix_test_errors",
  description: "Invoke Claude Code to fix failing tests",
  requiresLLM: true,

  apply: (ctx: HealerContext): Effect.Effect<HealerSpellResult, Error, never> =>
    Effect.gen(function* () {
      // Check if we have error output to work with
      if (!ctx.errorOutput) {
        return {
          success: false,
          changesApplied: false,
          summary: "No error output available to diagnose test failures",
          error: "Missing error output",
        };
      }

      // Validate this is actually a test failure scenario
      if (ctx.heuristics.scenario !== "InitScriptTestFailure") {
        return {
          success: false,
          changesApplied: false,
          summary: `Spell only applicable to InitScriptTestFailure, got ${ctx.heuristics.scenario}`,
          error: "Wrong scenario for this spell",
        };
      }

      // Create the emergency subtask
      const subtask = createEmergencySubtask(ctx, "test");

      // Return a "prepared" result - HealerService will execute the actual LLM call
      return {
        success: true,
        changesApplied: false,
        summary: `Prepared emergency test fix subtask: ${subtask.id}`,
      } as HealerSpellResult & {
        _preparedSubtask?: Subtask;
      };
    }),
};

// ============================================================================
// Execution Helpers (for HealerService)
// ============================================================================

/**
 * Execute typecheck fix with Claude Code.
 *
 * This is called by HealerService when executing an LLM-based spell.
 * It handles the actual Claude Code invocation and verification.
 */
export const executeTypecheckFix = async (
  ctx: HealerContext,
  invoker: ClaudeCodeInvoker,
  verifier: VerificationRunner,
  options: TypecheckFixOptions = {}
): Promise<HealerSpellResult> => {
  const subtask = createEmergencySubtask(ctx, "typecheck");

  try {
    // Build invoker options, omitting undefined values
    const invokerOptions: TypecheckFixOptions & { cwd: string } = {
      cwd: ctx.projectRoot,
      maxTurns: options.maxTurns ?? 50,
      permissionMode: options.permissionMode ?? "bypassPermissions",
    };
    if (options.onOutput) invokerOptions.onOutput = options.onOutput;
    if (options.signal) invokerOptions.signal = options.signal;
    if (options.openagentsDir) invokerOptions.openagentsDir = options.openagentsDir;

    // Invoke Claude Code to fix the errors
    const healResult = await invoker(subtask, invokerOptions);

    if (!healResult.success) {
      return {
        success: false,
        changesApplied: healResult.filesModified.length > 0,
        summary: `Claude Code failed to fix typecheck errors: ${healResult.error || "Unknown error"}`,
        error: healResult.error ?? "Unknown error",
        filesModified: healResult.filesModified,
      };
    }

    // Re-run verification to confirm fix
    const verifyResult = await verifier(ctx.projectRoot);

    if (!verifyResult.success) {
      return {
        success: false,
        changesApplied: healResult.filesModified.length > 0,
        summary: "Claude Code made changes but typecheck still fails",
        error: `Verification failed: ${verifyResult.output.slice(0, 500)}`,
        filesModified: healResult.filesModified,
      };
    }

    return {
      success: true,
      changesApplied: true,
      summary: `Fixed typecheck errors in ${healResult.filesModified.length} files`,
      filesModified: healResult.filesModified,
    };
  } catch (error) {
    return {
      success: false,
      changesApplied: false,
      summary: `Typecheck fix failed with error: ${error instanceof Error ? error.message : String(error)}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};

/**
 * Execute test fix with Claude Code.
 */
export const executeTestFix = async (
  ctx: HealerContext,
  invoker: ClaudeCodeInvoker,
  verifier: VerificationRunner,
  options: TypecheckFixOptions = {}
): Promise<HealerSpellResult> => {
  const subtask = createEmergencySubtask(ctx, "test");

  try {
    // Build invoker options, omitting undefined values
    const invokerOptions: TypecheckFixOptions & { cwd: string } = {
      cwd: ctx.projectRoot,
      maxTurns: options.maxTurns ?? 50,
      permissionMode: options.permissionMode ?? "bypassPermissions",
    };
    if (options.onOutput) invokerOptions.onOutput = options.onOutput;
    if (options.signal) invokerOptions.signal = options.signal;
    if (options.openagentsDir) invokerOptions.openagentsDir = options.openagentsDir;

    // Invoke Claude Code to fix the tests
    const healResult = await invoker(subtask, invokerOptions);

    if (!healResult.success) {
      return {
        success: false,
        changesApplied: healResult.filesModified.length > 0,
        summary: `Claude Code failed to fix test errors: ${healResult.error || "Unknown error"}`,
        error: healResult.error ?? "Unknown error",
        filesModified: healResult.filesModified,
      };
    }

    // Re-run verification to confirm fix
    const verifyResult = await verifier(ctx.projectRoot);

    if (!verifyResult.success) {
      return {
        success: false,
        changesApplied: healResult.filesModified.length > 0,
        summary: "Claude Code made changes but tests still fail",
        error: `Verification failed: ${verifyResult.output.slice(0, 500)}`,
        filesModified: healResult.filesModified,
      };
    }

    return {
      success: true,
      changesApplied: true,
      summary: `Fixed test errors, ${healResult.filesModified.length} files modified`,
      filesModified: healResult.filesModified,
    };
  } catch (error) {
    return {
      success: false,
      changesApplied: false,
      summary: `Test fix failed with error: ${error instanceof Error ? error.message : String(error)}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};
