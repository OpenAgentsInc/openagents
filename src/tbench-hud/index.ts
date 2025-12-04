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
