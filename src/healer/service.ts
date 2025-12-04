/**
 * Healer Service
 *
 * Main entry point for the Healer subagent.
 * Orchestrates policy checks, spell planning, and execution.
 */
import * as path from "node:path";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { Effect } from "effect";
import type {
  HealerContext,
  HealerCounters,
  HealerOutcome,
  HealerOutcomeStatus,
  HealerScenario,
  HealerSpellId,
  HealerSpellResult,
  HealerTrigger,
  HealingAttempt,
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
  buildHealingKey,
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

type SpellSequenceRunner = (
  ctx: HealerContext,
  scenario: HealerScenario,
  spells: HealerSpellId[],
  options: HealerServiceOptions,
  sessionId: string
) => Effect.Effect<HealerOutcome, Error, never>;

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
  /** Override spell execution (useful for testing) */
  spellRunner?: SpellSequenceRunner;
}

/**
 * Events emitted during Healer execution.
 */
export type HealerEvent =
  | { type: "healer_start"; scenario: HealerScenario; spells: HealerSpellId[] }
  | { type: "healer_spell_start"; spellId: HealerSpellId }
  | { type: "healer_spell_complete"; spellId: HealerSpellId; result: HealerSpellResult }
  | { type: "healer_complete"; outcome: HealerOutcome };

const HEALER_STATE_FILENAME = "healer-state.json";

const shouldSkipBasedOnAttempt = (attempt?: HealingAttempt): boolean =>
  attempt?.outcome === "resolved" || attempt?.outcome === "contained";

const getHealerStatePaths = (config: ProjectConfig, options: HealerServiceOptions) => {
  const baseDir = options.openagentsDir ?? path.join(config.rootDir ?? ".", ".openagents");
  return {
    baseDir,
    statePath: path.join(baseDir, HEALER_STATE_FILENAME),
  };
};

const mergePersistedAttempts = (
  counters: HealerCounters,
  statePath: string
): Effect.Effect<void, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      let content: string;
      try {
        content = await readFile(statePath, "utf8");
      } catch (error) {
        if ((error as NodeJS.ErrnoException)?.code === "ENOENT") {
          return;
        }
        throw error;
      }

      const parsed = JSON.parse(content);
      const attempts: HealingAttempt[] = Array.isArray(parsed?.attempts) ? parsed.attempts : [];
      for (const attempt of attempts) {
        if (!counters.healingAttempts.has(attempt.key)) {
          counters.healingAttempts.set(attempt.key, attempt);
        }
      }
    },
    catch: (error) =>
      new Error(
        `Failed to load healer state: ${
          error instanceof Error ? error.message : String(error)
        }`
      ),
  }).pipe(
    Effect.catchAll((error) =>
      Effect.sync(() =>
        console.warn("[Healer] Unable to load healer state:", error.message)
      )
    )
  );

const persistHealingAttempts = (
  counters: HealerCounters,
  baseDir: string,
  statePath: string
): Effect.Effect<void, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      await mkdir(baseDir, { recursive: true });
      const attempts = Array.from(counters.healingAttempts.values());
      const payload = JSON.stringify({ attempts }, null, 2);
      await writeFile(statePath, payload, "utf8");
    },
    catch: (error) =>
      new Error(
        `Failed to persist healer state: ${
          error instanceof Error ? error.message : String(error)
        }`
      ),
  }).pipe(
    Effect.catchAll((error) =>
      Effect.sync(() =>
        console.warn("[Healer] Unable to persist healer state:", error.message)
      )
    )
  );

// ============================================================================
// Healer Service
// ============================================================================

/**
 * Create a Healer service instance.
 */
export const createHealerService = (options: HealerServiceOptions = {}) => {
  const spellRunner: SpellSequenceRunner =
    options.spellRunner ?? executeSpellSequence;

  const invokeHealer = (
    trigger: HealerTrigger,
    state: OrchestratorState,
    config: ProjectConfig,
    counters: HealerCounters
  ): Effect.Effect<HealerOutcome, Error, never> =>
    Effect.gen(function* () {
      const { baseDir, statePath } = getHealerStatePaths(config, options);
      // Merge persisted attempts for crash recovery/deduplication.
      yield* mergePersistedAttempts(counters, statePath);

      const keyInfo = buildHealingKey({
        scenario: trigger.scenario,
        event: trigger.event,
        state,
      });
      const previousAttempt = counters.healingAttempts.get(keyInfo.key);

      if (shouldSkipBasedOnAttempt(previousAttempt)) {
        return {
          scenario: trigger.scenario,
          status: "skipped",
          spellsTried: previousAttempt?.spellsTried ?? [],
          spellsSucceeded: previousAttempt?.spellsSucceeded ?? [],
          summary: "Skipping Healer: previous attempt already resolved this failure",
        };
      }

      incrementCounters(counters, keyInfo.subtaskId);

      console.log("[DEBUG] Building Healer context...");
      const ctx = yield* buildHealerContext(trigger, state, config, counters).pipe(
        Effect.tapError((error) =>
          Effect.sync(() => {
            console.log("[DEBUG] buildHealerContext ERROR:", error);
            console.log("[DEBUG] Error stack:", error?.stack);
          })
        )
      );
      console.log("[DEBUG] Healer context built:", Object.keys(ctx));

      // Plan spells
      const spells = planSpells(ctx, {
        skipLLMSpells: options.skipLLMSpells ?? false,
      });

      if (spells.length === 0) {
        const skippedOutcome: HealerOutcome = {
          scenario: trigger.scenario,
          status: "skipped",
          spellsTried: [],
          spellsSucceeded: [],
          summary: "No applicable spells for scenario",
        };
        return skippedOutcome;
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
      const outcome = yield* spellRunner(
        ctx,
        trigger.scenario,
        spells,
        options,
        healerSessionId
      );

      // Emit complete event
      options.onEvent?.({ type: "healer_complete", outcome });

      // Emit HUD complete message
      options.onHudMessage?.({
        type: "healer_invocation_complete",
        sessionId: healerSessionId,
        status: outcome.status,
        reason: outcome.summary,
        spellsExecuted: outcome.spellsTried.length,
        successfulSpells: outcome.spellsSucceeded.length,
        failedSpells: outcome.spellsTried.length - outcome.spellsSucceeded.length,
      });

      const attempt: HealingAttempt = {
        key: keyInfo.key,
        scenario: trigger.scenario,
        taskId: keyInfo.taskId,
        ...(keyInfo.subtaskId ? { subtaskId: keyInfo.subtaskId } : {}),
        errorHash: keyInfo.errorHash,
        timestamp: new Date().toISOString(),
        outcome: outcome.status,
        spellsTried: outcome.spellsTried,
        spellsSucceeded: outcome.spellsSucceeded,
        summary: outcome.summary,
      };

      counters.healingAttempts.set(keyInfo.key, attempt);
      yield* persistHealingAttempts(counters, baseDir, statePath);

      return outcome;
    });

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

        // DEBUG: Log the decision
        console.log("[DEBUG] shouldRunHealer decision:", decision);

        if (!decision.run) {
          return null;
        }

        const scenario = decision.scenario!;
        const trigger: HealerTrigger = { scenario, event, state };

        console.log("[DEBUG] Healer will run for scenario:", scenario);

        return yield* invokeHealer(trigger, state, config, counters);
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
      invokeHealer(trigger, state, config, counters),
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
  scenario: HealerScenario,
  spells: HealerSpellId[],
  options: HealerServiceOptions,
  sessionId: string
): Effect.Effect<HealerOutcome, Error, never> =>
  Effect.gen(function* () {
    const spellsTried: HealerSpellId[] = [];
    const spellsSucceeded: HealerSpellId[] = [];
    let resolved = false;
    let lastSummary = "";

    for (const spellId of spells) {
      // Emit spell start event
      options.onEvent?.({ type: "healer_spell_start", spellId });

      // Execute the spell
      const result = yield* executeSpellWithLLM(ctx, spellId, options);

      spellsTried.push(spellId);
      lastSummary = result.summary;

      // Emit spell complete event
      options.onEvent?.({ type: "healer_spell_complete", spellId, result });

      // Emit HUD spell applied message
      const hudMsg: HealerSpellAppliedMessage = {
        type: "healer_spell_applied",
        sessionId,
        spellId,
        success: result.success,
        changesApplied: result.changesApplied ?? false,
        summary: result.summary,
      };
      if (result.filesModified) {
        hudMsg.filesModified = result.filesModified;
      }
      if (result.error) {
        hudMsg.error = result.error;
      }
      options.onHudMessage?.(hudMsg);

      if (result.success) {
        spellsSucceeded.push(spellId);
      }

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
    let status: HealerOutcomeStatus;
    if (resolved) {
      status = "resolved";
    } else if (spellsSucceeded.includes("mark_task_blocked_with_followup")) {
      status = "contained";
    } else if (spellsSucceeded.length === 0) {
      status = "unresolved";
    } else {
      // Some spells succeeded but didn't fully resolve
      status = "resolved"; // Treat partial success as resolved
    }

    const outcome: HealerOutcome = {
      scenario,
      status,
      spellsTried,
      spellsSucceeded,
      summary: summarizeOutcome(spellsTried, spellsSucceeded, lastSummary),
    };

    return outcome;
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
  spellsTried: HealerSpellId[],
  spellsSucceeded: HealerSpellId[],
  lastSummary: string
): string => {
  if (spellsTried.length === 0) {
    return "No spells executed";
  }

  const parts: string[] = [];

  if (spellsSucceeded.length > 0) {
    parts.push(`${spellsSucceeded.length} spell(s) succeeded`);
  }

  const failedCount = spellsTried.length - spellsSucceeded.length;
  if (failedCount > 0) {
    parts.push(`${failedCount} spell(s) failed`);
  }

  // Add last spell's summary
  if (lastSummary) {
    parts.push(`Last: ${lastSummary}`);
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
