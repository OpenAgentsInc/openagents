/**
 * Healer Subagent Module
 *
 * A self-healing subagent that wakes up automatically when agent trajectories
 * go off the rails, diagnoses what went wrong, and tries to repair or safely
 * contain the damage.
 *
 * @module healer
 */

// Types
export type {
  HealerScenario,
  HealerSpellId,
  HealerSpellResult,
  HealerSpell,
  HealerOutcomeStatus,
  HealerOutcome,
  GitStatus,
  HealerHeuristics,
  HealerContext,
  HealerCounters,
  HealerTrigger,
  HealerPolicyDecision,
} from "./types.js";

// Type guards and helpers
export {
  createHealerCounters,
  isResolved,
  isContained,
  spellRequiresLLM,
  mapInitFailureToScenario,
} from "./types.js";

// Policy
export {
  shouldRunHealer,
  mapEventToScenario,
  getInitScriptResult,
  getErrorOutput,
  isScenarioEnabled,
  hasExceededSessionLimit,
  hasExceededSubtaskLimit,
  incrementCounters,
  explainDecision,
  isHealableInitFailure,
} from "./policy.js";

// Context builder
export {
  buildHealerContext,
  getGitStatus,
  readProgressFile,
  buildHeuristics,
  detectErrorPatterns,
} from "./context.js";

// Spells
export {
  spellRegistry,
  getSpell,
  hasSpell,
  getRegisteredSpellIds,
  executeSpell,
  executeSpells,
  isSpellAllowed,
  filterAllowedSpells,
  rewindUncommittedChanges,
  markTaskBlockedWithFollowup,
  updateProgressWithGuidance,
} from "./spells/index.js";

// Planner (to be implemented in oa-healer-06)
// export { planSpells } from "./planner.js";

// Service (to be implemented in oa-healer-06)
// export { HealerService } from "./service.js";

// ATIF integration (to be implemented in oa-healer-07)
// export { createHealerAgent, createHealerTrajectory } from "./atif.js";
