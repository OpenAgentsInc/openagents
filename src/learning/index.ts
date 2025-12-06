/**
 * Learning Module
 *
 * MechaCoder's lifelong learning system.
 * Provides no-gradient continuous improvement through:
 *
 * 1. **Skills**: Voyager-style skill library (3.3x improvement)
 * 2. **Memory**: Generative Agents recency/importance/relevance
 * 3. **Reflexion**: Self-critique after failures (+11% improvement)
 * 4. **Archivist**: Pattern extraction from trajectories
 * 5. **Trainer**: Controlled execution environment
 * 6. **Loop**: Main training orchestration
 *
 * @module
 */

// Loop exports
export {
  TrainingLoop,
  TrainingLoopError,
  TrainingLoopLayer,
  TrainingLoopLive,
  makeTrainingLoopLayer,
  makeTrainingLoopLive,
  type ITrainingLoop,
  type LoopConfig,
  type LoopState,
  DEFAULT_LOOP_CONFIG,
} from "./loop.js";

// Orchestrator exports
export {
  LearningOrchestrator,
  OrchestratorError,
  LearningOrchestratorLayer,
  LearningOrchestratorLive,
  makeLearningOrchestratorLive,
  type ILearningOrchestrator,
  type LearningStats,
} from "./orchestrator.js";

// Re-export subsystems for convenience
export * from "../skills/index.js";
export * from "../memory/index.js";
export * from "../reflexion/index.js";
export * from "../archivist/index.js";
export * from "../trainer/index.js";
