/**
 * Terminal-Bench HUD Module
 *
 * Provides helpers for sending Terminal-Bench events to the HUD UI.
 *
 * @module tbench-hud
 */

export {
  createTBEmitter,
  createTBOutputCallback,
  type TBEmitter,
  type TBSuiteInfo,
  type TBTaskInfo,
  type TBTaskResult,
  type TBRunSummary,
} from "./emit.js";

export {
  saveTBRun,
  loadTBRun,
  loadTBRunMeta,
  listTBRuns,
  loadRecentRuns,
  deleteTBRun,
  getTBRunById,
  buildTBRunFile,
  buildTBRunMeta,
  convertResultsToTBRunFile,
  DEFAULT_TB_RUNS_DIR,
  type TBRunMeta,
  type TBTaskResult as TBPersistedTaskResult,
  type TBRunFile,
  type TBRunWithPath,
} from "./persistence.js";

export {
  createBuffer,
  getOrCreateBuffer,
  appendAndFlush,
  forceFlush,
  flushAllBuffers,
  clearAllBuffers,
  getBufferContent,
  type TBOutputBuffer,
  type BufferFlushOptions,
} from "./output-buffer.js";
