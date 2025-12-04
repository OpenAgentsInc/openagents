/**
 * Healer Spell Planner
 *
 * Determines which spells to execute based on the scenario and context.
 * Each scenario maps to a sequence of spells that should be tried.
 */
import type {
  HealerContext,
  HealerScenario,
  HealerSpellId,
} from "./types.js";
import { filterAllowedSpells } from "./spells/index.js";

// ============================================================================
// Scenario to Spell Mappings
// ============================================================================

/**
 * Map scenarios to default spell sequences.
 * Spells are executed in order; execution stops on success or after all fail.
 */
const SCENARIO_SPELLS: Record<HealerScenario, HealerSpellId[]> = {
  // Init script failures - try to fix, then document
  InitScriptTypecheckFailure: [
    "fix_typecheck_errors",           // Attempt Claude Code fix
    "update_progress_with_guidance",   // Document what happened
    "mark_task_blocked_with_followup", // If fix fails, block and create follow-up
  ],

  InitScriptTestFailure: [
    "fix_test_errors",                // Attempt Claude Code fix
    "update_progress_with_guidance",   // Document what happened
    "mark_task_blocked_with_followup", // If fix fails, block and create follow-up
  ],

  InitScriptEnvironmentFailure: [
    "update_progress_with_guidance",   // Document the environment issue
    "mark_task_blocked_with_followup", // Block with clear reason
  ],

  // Subtask/verification failures - rewind and document
  SubtaskFailed: [
    "rewind_uncommitted_changes",     // Clean up any partial changes
    "update_progress_with_guidance",   // Document failure
    "mark_task_blocked_with_followup", // Block after max retries
  ],

  VerificationFailed: [
    "rewind_uncommitted_changes",     // Revert changes that broke verification
    "update_progress_with_guidance",   // Document what went wrong
  ],

  // Runtime errors - document and contain
  RuntimeError: [
    "rewind_uncommitted_changes",     // Clean slate
    "update_progress_with_guidance",   // Document the error
    "mark_task_blocked_with_followup", // Block and investigate
  ],

  // Stuck detection - escalate
  SubtaskStuck: [
    "update_progress_with_guidance",   // Document stuck state
    "mark_task_blocked_with_followup", // Create investigation task
  ],
};

// ============================================================================
// Spell Planning
// ============================================================================

/**
 * Options for spell planning.
 */
export interface PlanSpellsOptions {
  /** Skip spells that require LLM invocation */
  skipLLMSpells?: boolean;
  /** Maximum number of spells to return */
  maxSpells?: number;
}

/**
 * Plan which spells to execute for a given context.
 *
 * This function:
 * 1. Gets the default spell sequence for the scenario
 * 2. Filters out forbidden spells (per project config)
 * 3. Optionally limits to non-LLM spells if LLM is not available
 *
 * @param ctx - The Healer context
 * @param options - Planning options
 * @returns Array of spell IDs to execute in order
 */
export const planSpells = (
  ctx: HealerContext,
  options: PlanSpellsOptions = {}
): HealerSpellId[] => {
  const { skipLLMSpells = false, maxSpells } = options;

  // Get default spells for this scenario
  let spells = SCENARIO_SPELLS[ctx.heuristics.scenario] ?? [];

  // Filter by project config (allowed/forbidden lists)
  spells = filterAllowedSpells(spells, ctx);

  // Optionally skip LLM-based spells
  if (skipLLMSpells) {
    spells = spells.filter((id) => !isLLMSpell(id));
  }

  // Apply max spells limit
  if (maxSpells !== undefined && spells.length > maxSpells) {
    spells = spells.slice(0, maxSpells);
  }

  return spells;
};

// ============================================================================
// Helpers
// ============================================================================

/**
 * Check if a spell requires LLM invocation.
 */
const isLLMSpell = (id: HealerSpellId): boolean => {
  const llmSpells: HealerSpellId[] = [
    "fix_typecheck_errors",
    "fix_test_errors",
    "retry_with_claude_code_resume",
    "retry_with_minimal_subagent",
  ];
  return llmSpells.includes(id);
};

/**
 * Get the default spell sequence for a scenario.
 */
export const getScenarioSpells = (scenario: HealerScenario): HealerSpellId[] => {
  return SCENARIO_SPELLS[scenario] ?? [];
};

/**
 * Check if a scenario has any associated spells.
 */
export const hasScenarioSpells = (scenario: HealerScenario): boolean => {
  const spells = SCENARIO_SPELLS[scenario];
  return spells !== undefined && spells.length > 0;
};

/**
 * Get all scenarios that use a specific spell.
 */
export const getScenariosUsingSpell = (spellId: HealerSpellId): HealerScenario[] => {
  return (Object.entries(SCENARIO_SPELLS) as [HealerScenario, HealerSpellId[]][])
    .filter(([_, spells]) => spells.includes(spellId))
    .map(([scenario]) => scenario);
};
