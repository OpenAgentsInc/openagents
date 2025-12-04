/**
 * Healer Service
 *
 * Main entry point for the Healer subagent.
 * Orchestrates policy checks, spell planning, and execution.
 */
import { Effect } from "effect";
import type {
  HealerContext,
  HealerCounters,
  HealerOutcome,
  HealerScenario,
  HealerSpellId,
  HealerSpellResult,
  HealerTrigger,
} from "./types.js";
import type { OrchestratorEvent, OrchestratorState } from "../agent/orchestrator/types.js";
import type { ProjectConfig } from "../tasks/schema.js";
import type {
  HealerInvocationStartMessage,
  HealerSpellAppliedMessage,
  HealerInvocationCompleteMessage,
} from "../hud/protocol.js";
import {
  shouldRunHealer,
  incrementCounters,
} from "./policy.js";
import { buildHealerContext } from "./context.js";
import { planSpells } from "./planner.js";
import { executeSpell, getSpell } from "./spells/index.js";
import {
  executeTypecheckFix,
  executeTestFix,
  type ClaudeCodeInvoker,
  type VerificationRunner,
} from "./spells/typecheck.js";
import { generateSessionId } from "../atif/schema.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Options for running the Healer.
 */
export interface HealerServiceOptions {
  /** Claude Code invoker function for LLM-based spells */
  claudeCodeInvoker?: ClaudeCodeInvoker;
  /** Verification runner for checking if fixes worked */
  verificationRunner?: VerificationRunner;
  /** Callback for streaming output */
  onOutput?: (text: string) => void;
  /** Abort signal for cancellation */
  signal?: AbortSignal;
  /** Path to openagents directory */
  openagentsDir?: string;
  /** Skip LLM-based spells */
  skipLLMSpells?: boolean;
  /** Emit internal Healer events (for ATIF trajectory capture) */
  onEvent?: (event: HealerEvent) => void;
  /** Emit HUD messages (for UI integration) */
  onHudMessage?: (msg: HealerInvocationStartMessage | HealerSpellAppliedMessage | HealerInvocationCompleteMessage) => void;
}

/**
 * Events emitted during Healer execution.
 */
export type HealerEvent =
  | { type: "healer_start"; scenario: HealerScenario; spells: HealerSpellId[] }
  | { type: "healer_spell_start"; spellId: HealerSpellId }
  | { type: "healer_spell_complete"; spellId: HealerSpellId; result: HealerSpellResult }
  | { type: "healer_complete"; outcome: HealerOutcome };

// ============================================================================
// Healer Service
// ============================================================================

/**
 * Create a Healer service instance.
 */
export const createHealerService = (options: HealerServiceOptions = {}) => {
  return {
    /**
     * Maybe run Healer based on policy and context.
     *
     * This is the main entry point called by the orchestrator.
     * Returns null if Healer should not run, otherwise returns the outcome.
     */
    maybeRun: (
      event: OrchestratorEvent,
      state: OrchestratorState,
      config: ProjectConfig,
      counters: HealerCounters
    ): Effect.Effect<HealerOutcome | null, Error, never> =>
      Effect.gen(function* () {
        // Extract Healer config
        const healerConfig = config.healer ?? {};

        // Get subtask ID if available
        const subtaskId = state.subtasks?.subtasks.find(
          (s) => s.status === "in_progress" || s.status === "failed"
        )?.id;

        // Check if we should run Healer
        const decision = shouldRunHealer(event, healerConfig, counters, subtaskId);

        if (!decision.run) {
          return null;
        }

        const scenario = decision.scenario!;
        const trigger: HealerTrigger = { scenario, event };

        // Update counters
        incrementCounters(counters, subtaskId);

        // Build context
        const ctx = yield* buildHealerContext(trigger, state, config, counters);

        // Plan spells
        const spells = planSpells(ctx, {
          skipLLMSpells: options.skipLLMSpells,
        });

        if (spells.length === 0) {
          return {
            status: "skipped" as const,
            reason: "No applicable spells for scenario",
            spellsExecuted: [],
            context: ctx,
          };
        }

        // Generate session ID for this Healer invocation
        const healerSessionId = generateSessionId();

        // Emit start event
        options.onEvent?.({ type: "healer_start", scenario, spells });

        // Emit HUD start message
        options.onHudMessage?.({
          type: "healer_invocation_start",
          sessionId: healerSessionId,
          scenario,
          plannedSpells: spells,
          parentSessionId: state.sessionId,
        });

        // Execute spells
        const outcome = yield* executeSpellSequence(ctx, spells, options, healerSessionId);

        // Emit complete event
        options.onEvent?.({ type: "healer_complete", outcome });

        // Emit HUD complete message
        const successfulSpells = outcome.spellsExecuted.filter((s) => s.result.success).length;
        const failedSpells = outcome.spellsExecuted.filter((s) => !s.result.success).length;
        options.onHudMessage?.({
          type: "healer_invocation_complete",
          sessionId: healerSessionId,
          status: outcome.status,
          reason: outcome.reason,
          spellsExecuted: outcome.spellsExecuted.length,
          successfulSpells,
          failedSpells,
        });

        return outcome;
      }),

    /**
     * Run Healer directly with a specific trigger.
     * Used for testing or manual invocation.
     */
    run: (
      trigger: HealerTrigger,
      state: OrchestratorState,
      config: ProjectConfig,
      counters: HealerCounters
    ): Effect.Effect<HealerOutcome, Error, never> =>
      Effect.gen(function* () {
        // Build context
        const ctx = yield* buildHealerContext(trigger, state, config, counters);

        // Plan spells
        const spells = planSpells(ctx, {
          skipLLMSpells: options.skipLLMSpells,
        });

        if (spells.length === 0) {
          return {
            status: "skipped" as const,
            reason: "No applicable spells for scenario",
            spellsExecuted: [],
            context: ctx,
          };
        }

        // Generate session ID for this Healer invocation
        const healerSessionId = generateSessionId();

        // Emit start event
        options.onEvent?.({ type: "healer_start", scenario: trigger.scenario, spells });

        // Emit HUD start message
        options.onHudMessage?.({
          type: "healer_invocation_start",
          sessionId: healerSessionId,
          scenario: trigger.scenario,
          plannedSpells: spells,
          parentSessionId: state.sessionId,
        });

        // Execute spells
        const outcome = yield* executeSpellSequence(ctx, spells, options, healerSessionId);

        // Emit complete event
        options.onEvent?.({ type: "healer_complete", outcome });

        // Emit HUD complete message
        const successfulSpells = outcome.spellsExecuted.filter((s) => s.result.success).length;
        const failedSpells = outcome.spellsExecuted.filter((s) => !s.result.success).length;
        options.onHudMessage?.({
          type: "healer_invocation_complete",
          sessionId: healerSessionId,
          status: outcome.status,
          reason: outcome.reason,
          spellsExecuted: outcome.spellsExecuted.length,
          successfulSpells,
          failedSpells,
        });

        return outcome;
      }),
  };
};

// ============================================================================
// Spell Execution
// ============================================================================

/**
 * Execute a sequence of spells until one succeeds or all fail.
 */
const executeSpellSequence = (
  ctx: HealerContext,
  spells: HealerSpellId[],
  options: HealerServiceOptions,
  sessionId: string
): Effect.Effect<HealerOutcome, Error, never> =>
  Effect.gen(function* () {
    const spellsExecuted: Array<{ spellId: HealerSpellId; result: HealerSpellResult }> = [];
    let resolved = false;

    for (const spellId of spells) {
      // Emit spell start event
      options.onEvent?.({ type: "healer_spell_start", spellId });

      // Execute the spell
      const result = yield* executeSpellWithLLM(ctx, spellId, options);

      spellsExecuted.push({ spellId, result });

      // Emit spell complete event
      options.onEvent?.({ type: "healer_spell_complete", spellId, result });

      // Emit HUD spell applied message
      options.onHudMessage?.({
        type: "healer_spell_applied",
        sessionId,
        spellId,
        success: result.success,
        changesApplied: result.changesApplied,
        summary: result.summary,
        filesModified: result.filesModified,
        error: result.error,
      });

      // Check if the spell resolved the issue
      if (result.success && result.changesApplied) {
        resolved = true;
        break;
      }

      // For containment spells, stop after success even without changes
      if (spellId === "mark_task_blocked_with_followup" && result.success) {
        break;
      }
    }

    // Determine outcome status
    const status: HealerOutcome["status"] = resolved
      ? "resolved"
      : spellsExecuted.some((s) => s.result.success && s.spellId === "mark_task_blocked_with_followup")
      ? "contained"
      : spellsExecuted.some((s) => s.result.success)
      ? "partial"
      : "failed";

    return {
      status,
      reason: summarizeOutcome(spellsExecuted),
      spellsExecuted,
      context: ctx,
    };
  });

/**
 * Execute a spell, handling LLM-based spells specially.
 */
const executeSpellWithLLM = (
  ctx: HealerContext,
  spellId: HealerSpellId,
  options: HealerServiceOptions
): Effect.Effect<HealerSpellResult, Error, never> =>
  Effect.gen(function* () {
    const spell = getSpell(spellId);
    if (!spell) {
      return {
        success: false,
        changesApplied: false,
        summary: `Spell '${spellId}' not found`,
        error: `Unknown spell: ${spellId}`,
      };
    }

    // Handle LLM-based spells
    if (spell.requiresLLM) {
      if (!options.claudeCodeInvoker || !options.verificationRunner) {
        return {
          success: false,
          changesApplied: false,
          summary: `Spell '${spellId}' requires LLM but no invoker provided`,
          error: "LLM invoker not available",
        };
      }

      // Execute the appropriate LLM spell
      if (spellId === "fix_typecheck_errors") {
        // Build options, omitting undefined values
        const fixOptions: Record<string, unknown> = {};
        if (options.onOutput) fixOptions.onOutput = options.onOutput;
        if (options.signal) fixOptions.signal = options.signal;
        if (options.openagentsDir) fixOptions.openagentsDir = options.openagentsDir;

        return yield* Effect.tryPromise({
          try: () =>
            executeTypecheckFix(
              ctx,
              options.claudeCodeInvoker!,
              options.verificationRunner!,
              fixOptions
            ),
          catch: (e) => new Error(`Typecheck fix failed: ${e}`),
        });
      }

      if (spellId === "fix_test_errors") {
        // Build options, omitting undefined values
        const fixOptions: Record<string, unknown> = {};
        if (options.onOutput) fixOptions.onOutput = options.onOutput;
        if (options.signal) fixOptions.signal = options.signal;
        if (options.openagentsDir) fixOptions.openagentsDir = options.openagentsDir;

        return yield* Effect.tryPromise({
          try: () =>
            executeTestFix(
              ctx,
              options.claudeCodeInvoker!,
              options.verificationRunner!,
              fixOptions
            ),
          catch: (e) => new Error(`Test fix failed: ${e}`),
        });
      }

      // Other LLM spells not yet implemented
      return {
        success: false,
        changesApplied: false,
        summary: `LLM spell '${spellId}' not implemented`,
        error: "Not implemented",
      };
    }

    // Execute non-LLM spell normally
    return yield* executeSpell(spellId, ctx);
  });

/**
 * Summarize the outcome for logging.
 */
const summarizeOutcome = (
  spellsExecuted: Array<{ spellId: HealerSpellId; result: HealerSpellResult }>
): string => {
  if (spellsExecuted.length === 0) {
    return "No spells executed";
  }

  const successful = spellsExecuted.filter((s) => s.result.success);
  const failed = spellsExecuted.filter((s) => !s.result.success);

  const parts: string[] = [];

  if (successful.length > 0) {
    parts.push(`${successful.length} spell(s) succeeded`);
  }

  if (failed.length > 0) {
    parts.push(`${failed.length} spell(s) failed`);
  }

  // Add last spell's summary
  const last = spellsExecuted[spellsExecuted.length - 1];
  if (last) {
    parts.push(`Last: ${last.result.summary}`);
  }

  return parts.join("; ");
};

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a default Healer service with no LLM capabilities.
 * Useful for testing or when LLM is not available.
 */
export const createBasicHealerService = (
  onEvent?: (event: HealerEvent) => void
) => {
  const opts: HealerServiceOptions = { skipLLMSpells: true };
  if (onEvent) opts.onEvent = onEvent;
  return createHealerService(opts);
};

/**
 * Create a full Healer service with LLM capabilities.
 */
export const createFullHealerService = (
  claudeCodeInvoker: ClaudeCodeInvoker,
  verificationRunner: VerificationRunner,
  options: Omit<HealerServiceOptions, "claudeCodeInvoker" | "verificationRunner"> = {}
) =>
  createHealerService({
    ...options,
    claudeCodeInvoker,
    verificationRunner,
  });

// Re-export types
export type { ClaudeCodeInvoker, VerificationRunner };
