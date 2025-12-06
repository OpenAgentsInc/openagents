/**
 * Reflexion Module
 *
 * Self-critique and verbal reinforcement for MechaCoder.
 * Based on Reflexion research showing +11% improvement from self-critique.
 *
 * @module
 */

// Schema exports
export {
  type FailureContext,
  type Reflection,
  type ReflectionHistory,
  type ExtractedSkillPattern,
  type ErrorType,
  classifyError,
  buildReflectionPrompt,
  buildSkillExtractionPrompt,
  formatReflectionsForPrompt,
  generateFailureId,
  generateReflectionId,
  createFailureContext,
  createReflection,
} from "./schema.js";

// Generator exports
export {
  ReflectionGenerator,
  ReflectionGeneratorError,
  ReflectionGeneratorLive,
  makeReflectionGeneratorLive,
  type IReflectionGenerator,
} from "./generator.js";

// Service exports
export {
  ReflexionService,
  ReflexionServiceError,
  ReflexionServiceLive,
  ReflexionServiceLayer,
  makeReflexionServiceLive,
  type IReflexionService,
} from "./service.js";
