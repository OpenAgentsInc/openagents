/**
 * Memory System Module
 *
 * Provides episodic, semantic, and procedural memory for MechaCoder's learning system.
 * Based on Generative Agents research showing 8 std dev improvement with memory + reflection.
 *
 * @module
 */

// Schema exports
export {
  type Memory,
  type MemoryType,
  type MemoryScope,
  type MemoryStatus,
  type ImportanceLevel,
  type MemoryContent,
  type EpisodicContent,
  type SemanticContent,
  type ProceduralContent,
  type MemoryMatch,
  type MemoryQuery,
  type MemoryFilter,
  type ScoringWeights,
  DEFAULT_SCORING_WEIGHTS,
  importanceToScore,
  calculateRecency,
  calculateMemoryScore,
  generateMemoryId,
  createMemory,
  createEpisodicMemory,
  createSemanticMemory,
  createProceduralMemory,
  formatMemoriesForPrompt,
  buildMemoryText,
} from "./schema.js";

// Store exports
export {
  MemoryStore,
  MemoryStoreError,
  MemoryStoreLive,
  makeMemoryStoreLayer,
  type IMemoryStore,
} from "./store.js";

// Retrieval exports
export {
  MemoryRetrievalService,
  MemoryRetrievalError,
  MemoryRetrievalServiceLive,
  type IMemoryRetrievalService,
} from "./retrieval.js";

// Service exports
export {
  MemoryService,
  MemoryServiceError,
  MemoryServiceLive,
  MemoryServiceLayer,
  makeMemoryServiceLive,
  type IMemoryService,
} from "./service.js";
