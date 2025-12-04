/**
 * Healer Spell Registry
 *
 * Spells are controlled recovery operations that Healer can execute.
 * Each spell is designed to never make things worse.
 */
import { Effect } from "effect";
import type {
  HealerSpell,
  HealerSpellId,
  HealerSpellResult,
  HealerContext,
} from "../types.js";
import { rewindUncommittedChanges } from "./rewind.js";
import { markTaskBlockedWithFollowup } from "./blocked.js";
import { updateProgressWithGuidance } from "./progress.js";

// ============================================================================
// Spell Registry
// ============================================================================

/**
 * Registry of all available spells.
 */
export const spellRegistry: Map<HealerSpellId, HealerSpell> = new Map([
  ["rewind_uncommitted_changes", rewindUncommittedChanges],
  ["mark_task_blocked_with_followup", markTaskBlockedWithFollowup],
  ["update_progress_with_guidance", updateProgressWithGuidance],
  // Phase 2 spells (to be implemented):
  // ["fix_typecheck_errors", fixTypecheckErrors],
  // ["fix_test_errors", fixTestErrors],
  // ["rewind_to_last_green_commit", rewindToLastGreenCommit],
  // ["retry_with_minimal_subagent", retryWithMinimalSubagent],
  // ["retry_with_claude_code_resume", retryWithClaudeCodeResume],
  // ["run_tasks_doctor_like_checks", runTasksDoctorLikeChecks],
]);

/**
 * Get a spell by ID.
 */
export const getSpell = (id: HealerSpellId): HealerSpell | undefined => {
  return spellRegistry.get(id);
};

/**
 * Check if a spell exists in the registry.
 */
export const hasSpell = (id: HealerSpellId): boolean => {
  return spellRegistry.has(id);
};

/**
 * Get all registered spell IDs.
 */
export const getRegisteredSpellIds = (): HealerSpellId[] => {
  return Array.from(spellRegistry.keys());
};

// ============================================================================
// Spell Execution
// ============================================================================

/**
 * Execute a spell by ID.
 * Returns an error result if the spell is not found.
 */
export const executeSpell = (
  id: HealerSpellId,
  context: HealerContext
): Effect.Effect<HealerSpellResult, Error, never> => {
  const spell = getSpell(id);
  if (!spell) {
    return Effect.succeed({
      success: false,
      summary: `Spell '${id}' not found in registry`,
      error: `Unknown spell: ${id}`,
    });
  }

  return spell.apply(context).pipe(
    Effect.catchAll((error) =>
      Effect.succeed({
        success: false,
        summary: `Spell '${id}' failed with error`,
        error: error instanceof Error ? error.message : String(error),
      })
    )
  );
};

/**
 * Execute multiple spells in sequence.
 * Stops on first failure unless continueOnFailure is true.
 */
export const executeSpells = (
  ids: HealerSpellId[],
  context: HealerContext,
  options?: { continueOnFailure?: boolean }
): Effect.Effect<HealerSpellResult[], Error, never> => {
  const continueOnFailure = options?.continueOnFailure ?? false;

  return Effect.gen(function* () {
    const results: HealerSpellResult[] = [];

    for (const id of ids) {
      const result = yield* executeSpell(id, context);
      results.push(result);

      if (!result.success && !continueOnFailure) {
        break;
      }
    }

    return results;
  });
};

/**
 * Check if a spell is allowed by config.
 */
export const isSpellAllowed = (
  id: HealerSpellId,
  context: HealerContext
): boolean => {
  const spellsConfig = context.projectConfig.healer?.spells;
  if (!spellsConfig) return true;

  // Forbidden takes precedence
  if (spellsConfig.forbidden?.includes(id)) {
    return false;
  }

  // If allowed list is empty, all non-forbidden spells are allowed
  if (!spellsConfig.allowed || spellsConfig.allowed.length === 0) {
    return true;
  }

  // Otherwise, spell must be in allowed list
  return spellsConfig.allowed.includes(id);
};

/**
 * Filter spells to only those allowed by config.
 */
export const filterAllowedSpells = (
  ids: HealerSpellId[],
  context: HealerContext
): HealerSpellId[] => {
  return ids.filter((id) => isSpellAllowed(id, context));
};

// Re-export individual spells for direct access
export { rewindUncommittedChanges } from "./rewind.js";
export { markTaskBlockedWithFollowup } from "./blocked.js";
export { updateProgressWithGuidance } from "./progress.js";
