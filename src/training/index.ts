/**
 * Training Module
 *
 * Provides training loop runners and utilities for progressive
 * Terminal-Bench evaluation and overnight iteration support.
 *
 * @module
 */

export {
  // Loop Runner
  createLoopRunner,
  runTrainingLoop,
  runOvernightTraining,
  runProgressiveBenchmark,
  // Types
  type ILoopRunner,
  type LoopRunnerConfig,
  type LoopRunnerState,
  type IterationResult,
  // Error
  LoopRunnerError,
  // Config
  DEFAULT_CONFIG,
} from "./loop-runner.js";

export {
  // Episode Learner
  createEpisodeLearner,
  learnFromRecentEpisodes,
  learnFromEpisode,
  // Types
  type IEpisodeLearner,
  type EpisodeLearnerConfig,
  type LearningResult,
  type LearningSummary,
  type Reflection,
  // Error
  EpisodeLearnerError,
  // Config
  DEFAULT_LEARNER_CONFIG,
} from "./episode-learner.js";
