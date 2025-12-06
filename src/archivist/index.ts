/**
 * Archivist Module
 *
 * Subagent for reviewing trajectories and extracting patterns.
 * Part of MechaCoder's no-gradient lifelong learning system.
 *
 * The Archivist:
 * 1. Records task trajectories (actions, outcomes, context)
 * 2. Periodically analyzes trajectories for patterns
 * 3. Extracts reusable skills from successful patterns
 * 4. Updates semantic memories with lessons learned
 * 5. Prunes low-quality or outdated entries
 *
 * @module
 */

// Schema exports
export {
  type Trajectory,
  type TrajectoryAction,
  type ExtractedPattern,
  type ArchivistLesson,
  type ArchiveResult,
  type ArchiveConfig,
  DEFAULT_ARCHIVE_CONFIG,
  generateTrajectoryId,
  generatePatternId,
  generateArchiveId,
  generateLessonId,
  createTrajectory,
  createLesson,
  buildPatternExtractionPrompt,
  parsePatternsFromResponse,
  calculateSuccessRate,
  groupSimilarTrajectories,
} from "./schema.js";

// Store exports
export {
  TrajectoryStore,
  TrajectoryStoreError,
  makeTrajectoryStoreLive,
  TrajectoryStoreLive,
  LessonStore,
  LessonStoreError,
  makeLessonStoreLive,
  LessonStoreLive,
  type ITrajectoryStore,
  type ILessonStore,
} from "./store.js";

// Extractor exports
export {
  PatternExtractor,
  PatternExtractorError,
  PatternExtractorLive,
  makePatternExtractorLive,
  makePatternExtractorWithFM,
  type IPatternExtractor,
} from "./extractor.js";

// Service exports
export {
  ArchivistService,
  ArchivistError,
  ArchivistServiceLayer,
  ArchivistServiceLive,
  makeArchivistServiceLive,
  type IArchivistService,
} from "./service.js";

// Retrieval exports
export {
  getRelevantLessons,
  getLessonsForTask,
  formatLessonsForPrompt,
  formatLessonsCompact,
  getFormattedLessons,
  type LessonQuery,
} from "./retrieval.js";
