/**
 * Skill Library Module
 *
 * Provides skill management for MechaCoder's no-gradient learning system.
 * Based on Voyager research showing 3.3x improvement with skill libraries.
 *
 * @module
 */

// Schema exports
export {
  Skill,
  SkillParameter,
  SkillParameterType,
  SkillVerification,
  VerificationType,
  SkillExample,
  SkillCategory,
  SkillStatus,
  SkillFilter,
  SkillQuery,
  SkillMatch,
  SkillCall,
  SkillExecutionResult,
  generateSkillId,
  createSkill,
  formatSkillForPrompt,
  formatSkillsForPrompt,
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
