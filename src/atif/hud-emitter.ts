/**
 * ATIF HUD Emitter
 *
 * Emits ATIF step events to the HUD (Heads-Up Display) via WebSocket.
 * This enables real-time trajectory visualization in the frontend.
 *
 * Phase 1: Tool calls + observations (MVP debugging info)
 * Future phases will add reasoning_content, metrics, model_name, etc.
 *
 * Usage:
 * ```typescript
 * import { emitATIFStep } from "./hud-emitter.js";
 * import type { Step } from "./schema.js";
 *
 * // After recording a step
 * emitATIFStep(runId, sessionId, step);
 * ```
 */

import type { Step } from "./schema.js";
import type { ATIFStepMessage } from "../hud/protocol.js";

/**
 * Global HUD message sender.
 * Set this at application startup via setATIFHudSender().
 */
let hudSender: ((message: ATIFStepMessage) => void) | null = null;

/**
 * Set the HUD sender function for ATIF step emission.
 *
 * Call this once at application startup to wire up the HUD client:
 *
 * ```typescript
 * import { setATIFHudSender } from "./atif/hud-emitter.js";
 * import { createHudCallbacks } from "./hud/emit.js";
 *
 * const { emitHud } = createHudCallbacks();
 * setATIFHudSender((msg) => emitHud(msg));
 * ```
 */
export const setATIFHudSender = (sender: (message: ATIFStepMessage) => void): void => {
  hudSender = sender;
};

/**
 * Clear the HUD sender (useful for testing or cleanup).
 */
export const clearATIFHudSender = (): void => {
  hudSender = null;
};

/**
 * Emit an ATIF step to the HUD for real-time display.
 *
 * Phase 1 MVP: Emits tool_calls and observation (core debugging info).
 * Future phases will add reasoning_content, metrics, etc.
 *
 * @param runId - TB run ID (for grouping steps by run)
 * @param sessionId - ATIF session ID
 * @param step - Full ATIF step object
 *
 * @example
 * ```typescript
 * const step: Step = {
 *   step_id: 1,
 *   timestamp: new Date().toISOString(),
 *   source: "agent",
 *   message: "Calling Read tool",
 *   tool_calls: [{
 *     tool_call_id: "call_123",
 *     function_name: "Read",
 *     arguments: { file_path: "/path/to/file" }
 *   }],
 * };
 *
 * emitATIFStep("run_abc", "session_xyz", step);
 * ```
 */
export const emitATIFStep = (
  runId: string,
  sessionId: string,
  step: Step
): void => {
  if (!hudSender) {
    // HUD not initialized - silently skip (not an error during tests or headless runs)
    return;
  }

  // Phase 1: Extract only tool_calls and observation
  // Future phases will add more fields (reasoning_content, metrics, model_name)
  // Note: We create a new object to avoid readonly type issues
  const message: ATIFStepMessage = {
    type: "atif_step",
    runId,
    sessionId,
    step: {
      step_id: step.step_id,
      timestamp: step.timestamp,
      source: step.source,
      message: step.message,
      ...(step.tool_calls
        ? {
            tool_calls: step.tool_calls.map((tc) => ({
              tool_call_id: tc.tool_call_id,
              function_name: tc.function_name,
              arguments: tc.arguments,
            })),
          }
        : {}),
      ...(step.observation
        ? {
            observation: {
              results: step.observation.results.map((r) => ({
                ...(r.source_call_id ? { source_call_id: r.source_call_id } : {}),
                ...(r.content !== undefined ? { content: r.content } : {}),
              })),
            },
          }
        : {}),
    },
  };

  hudSender(message);
};
