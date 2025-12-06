/**
 * Skill Library Module
 *
 * Provides skill management for MechaCoder's no-gradient learning system.
 * Based on Voyager research showing 3.3x improvement with skill libraries.
 *
 * @module
 */

// Schema exports - values
export {
  Skill,
  SkillParameter,
  SkillParameterType,
  SkillVerification,
  VerificationType,
  SkillExample,
  SkillCategory,
  SkillStatus,
  SkillCall,
  SkillExecutionResult,
  generateSkillId,
  createSkill,
  formatSkillForPrompt,
  formatSkillsForPrompt,
} from "./schema.js";

// Schema exports - types (interfaces must be re-exported with type)
export type {
  SkillFilter,
  SkillQuery,
  SkillMatch,
} from "./schema.js";

// Store exports
export {
  SkillStore,
  SkillStoreError,
  SkillStoreLive,
  makeSkillStoreLayer,
  type ISkillStore,
} from "./store.js";

// Embedding exports
export {
  EmbeddingService,
  EmbeddingError,
  EmbeddingServiceLive,
  makeEmbeddingServiceLayer,
  cosineSimilarity,
  buildSkillText,
  type IEmbeddingService,
} from "./embedding.js";

// Retrieval exports
export {
  SkillRetrievalService,
  SkillRetrievalError,
  SkillRetrievalServiceLive,
  type ISkillRetrievalService,
} from "./retrieval.js";

// Service exports
export {
  SkillService,
  SkillServiceError,
  SkillServiceLive,
  SkillServiceLayer,
  makeSkillServiceLive,
  type ISkillService,
} from "./service.js";

// Evolution exports
export {
  SkillEvolutionService,
  SkillEvolutionError,
  SkillEvolutionServiceLive,
  makeSkillEvolutionLayer,
  DEFAULT_EVOLUTION_CONFIG,
  type ISkillEvolutionService,
  type SkillEvolutionConfig,
  type SkillEvolutionAction,
  type EvolutionResult,
  type EvolutionReport,
} from "./evolution.js";
