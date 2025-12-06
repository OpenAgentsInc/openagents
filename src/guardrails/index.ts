/**
 * Guardrails Module
 *
 * Safety constraints for MechaCoder's learning system.
 *
 * Provides:
 * - Resource limits (tokens, duration, retries)
 * - Safety constraints (blocked files, network access)
 * - Quality thresholds (success rate, consecutive failures)
 * - Behavioral limits (skills per run, memory entries)
 *
 * @module
 */

// Schema exports
export {
  type GuardrailRule,
  type GuardrailResult,
  type GuardrailStatus,
  type GuardrailsConfig,
  type RuleCategory,
  DEFAULT_GUARDRAILS_CONFIG,
  BUILTIN_RULES,
  createResult,
  aggregateResults,
  matchesBlockedPattern,
  getRulesByCategory,
  getEnabledRules,
} from "./schema.js";

// Service exports
export {
  GuardrailsService,
  GuardrailsError,
  GuardrailsServiceLive,
  makeGuardrailsServiceLive,
  type IGuardrailsService,
  type ValidationContext,
} from "./service.js";
