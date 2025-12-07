/**
 * Trainer Module
 *
 * Training system for MechaCoder.
 * Provides controlled execution environment with skill/memory integration.
 *
 * The Trainer:
 * 1. Manages training runs on Terminal-Bench or custom tasks
 * 2. Integrates with Gym for task execution
 * 3. Uses skills and memories to improve performance
 * 4. Records trajectories for the Archivist
 * 5. Applies Reflexion on failures
 *
 * @module
 */

// Schema exports
export {
  type TrainingTask,
  type TaskResult,
  type TrainingConfig,
  type TrainingRun,
  type TrainingStats,
  type BenchmarkSuite,
  type BenchmarkResult,
  type TBSubset,
  type TrainerHudMessage,
  DEFAULT_TRAINING_CONFIG,
  TB_SUBSETS,
  generateRunId,
  generateTaskId,
  calculateStats,
  createTask,
  createTaskResult,
  createTrainingRun,
} from "./schema.js";

// Gym exports
export {
  Gym,
  GymError,
  GymLayer,
  GymLive,
  makeGymLayer,
  makeGymLive,
  type IGym,
} from "./gym.js";

// Service exports
export {
  TrainerService,
  TrainerError,
  TrainerServiceLayer,
  TrainerServiceLive,
  makeTrainerServiceLive,
  type ITrainerService,
} from "./service.js";

// Evolution exports
export {
  type EvolutionProfile,
  type PromptModifiers,
  type MutationConfig,
  type ABComparisonResult,
  type EvolutionPopulation,
  DEFAULT_MUTATION_CONFIG,
  generateProfileId,
  createSeedProfile,
  mutateConfig,
  mutatePromptModifiers,
  mutateProfile,
  crossoverProfiles,
  calculateFitness,
  updateProfileFitness,
  compareProfiles,
  createPopulation,
  selectTopProfiles,
  evolvePopulation,
  applyPromptModifiers,
  runEvolution,
} from "./evolution.js";
