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

// --- TRM Components (Tiny Recursive Model) ---
export {
  // State types
  TaskContext,
  CandidateSolution,
  ReasoningTrace,
  TRMState,
  type TaskContext as TaskContextType,
  type CandidateSolution as CandidateSolutionType,
  type ReasoningTrace as ReasoningTraceType,
  type TRMState as TRMStateType,
  // State helpers
  createTaskContext,
  createInitialSolution,
  createInitialReasoning,
  createTRMState,
  updateSolution,
  updateReasoning,
  addReasoningStep,
  markStuck,
  addHypothesis,
  ruleOutApproach,
  completeCycle,
  detachState,
  // Service
  type ITRMStateService,
  TRMStateService,
  TRMStateServiceLive,
} from "./trm-state.js";

export {
  // Halt types
  type HaltConfig,
  DEFAULT_HALT_CONFIG,
  type HaltReason,
  type HaltDecision,
  type ProgressStatus,
  // Halt functions
  checkMaxDepth,
  checkTestsPassed,
  checkHighConfidence,
  checkStuck,
  checkAccuracyAchieved,
  shouldHalt,
  detectProgress,
  // Service
  type ITRMHaltService,
  TRMHaltService,
  TRMHaltServiceLive,
  makeTRMHaltServiceLayer,
} from "./trm-halt.js";

export {
  // EMA types
  type EMAConfig,
  DEFAULT_EMA_CONFIG,
  EMAValue,
  type EMAValue as EMAValueType,
  TaskTypeStats,
  type TaskTypeStats as TaskTypeStatsType,
  type SkillEMAStats,
  // EMA functions
  createEMAValue,
  updateEMA,
  isReliable,
  getConfidenceInterval,
  createTaskTypeStats,
  updateTaskTypeStats,
  createSkillEMAStats,
  updateSkillEMAStats,
  // Service
  type ITRMEMAService,
  TRMEMAService,
  TRMEMAServiceLive,
  makeTRMEMAServiceLayer,
} from "./trm-ema.js";

// --- SOAR Components (Self-Improving via Hindsight) ---
export {
  // Hindsight types
  AttemptRecord,
  type AttemptRecord as AttemptRecordType,
  SyntheticTask,
  type SyntheticTask as SyntheticTaskType,
  SyntheticTaskSolution,
  type SyntheticTaskSolution as SyntheticTaskSolutionType,
  type HindsightConfig,
  DEFAULT_HINDSIGHT_CONFIG,
  type HindsightStats,
  // Hindsight functions
  generateSyntheticDescription,
  isSuitableForRelabeling,
  createSyntheticTask,
  relabelAttempt,
  relabelBatch,
  // Service
  type IHindsightService,
  HindsightService,
  HindsightServiceLive,
  makeHindsightServiceLayer,
} from "./soar-hindsight.js";

export {
  // Validation types
  ValidationResult,
  type ValidationResult as ValidationResultType,
  type ValidationConfig,
  DEFAULT_VALIDATION_CONFIG,
  type ValidationStats,
  // Validation functions
  checkNonTrivialOutput,
  checkNonIdentity,
  checkCodeComplexity,
  checkNotLookupTable,
  checkEntropy,
  validateSynthetic,
  validateBatch,
  // Service
  type IValidationService,
  ValidationService,
  ValidationServiceLive,
  makeValidationServiceLayer,
} from "./soar-validation.js";

export {
  // Selection types
  type SelectionConfig,
  DEFAULT_SELECTION_CONFIG,
  SelectionResult,
  type SelectionResult as SelectionResultType,
  type SelectionStats,
  // Selection functions
  selectTop,
  selectBottom,
  selectGreedyDiverse,
  groupByTask,
  selectWithTaskBalance,
  // Service
  type ISelectionService,
  SelectionService,
  SelectionServiceLive,
  makeSelectionServiceLayer,
} from "./soar-selection.js";

export {
  // Voting types
  type VotingConfig,
  DEFAULT_VOTING_CONFIG,
  Vote,
  type Vote as VoteType,
  VotingResult,
  type VotingResult as VotingResultType,
  type VotingStats,
  // Voting functions
  normalizeOutputKey,
  calculateVoteWeight,
  groupVotes,
  vote,
  createVotes,
  ensembleVote,
  // Service
  type IVotingService,
  VotingService,
  VotingServiceLive,
  makeVotingServiceLayer,
} from "./soar-voting.js";

export {
  // TTT types
  type TTTConfig,
  DEFAULT_TTT_CONFIG,
  TTTIterationResult,
  type TTTIterationResult as TTTIterationResultType,
  TTTSessionResult,
  type TTTSessionResult as TTTSessionResultType,
  type TTTState,
  type TrainingAccuracyEstimate,
  type TTTSkillContext,
  type TTTStats,
  // TTT functions
  createTTTState,
  shouldContinueTTT,
  getStopReason,
  processIteration,
  createSessionResult,
  outputsEqual,
  updateSkillContext,
  // Service
  type ITTTService,
  TTTService,
  TTTServiceLive,
  makeTTTServiceLayer,
} from "./soar-ttt.js";
