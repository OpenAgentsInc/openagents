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
import type { StreamingWriter } from "./streaming-writer.js";

/**
 * Global HUD message sender.
 * Set this at application startup via setATIFHudSender().
 */
let hudSender: ((message: ATIFStepMessage) => void) | null = null;

/**
 * Registry of StreamingWriter instances for disk persistence.
 * Keyed by sessionId so each task run has its own writer.
 */
const diskWriters = new Map<string, StreamingWriter>();

/**
 * Register a StreamingWriter for a specific session.
 * Steps emitted for this sessionId will be persisted to disk.
 */
export const registerATIFDiskWriter = (sessionId: string, writer: StreamingWriter): void => {
  diskWriters.set(sessionId, writer);
};

/**
 * Unregister a StreamingWriter for a specific session.
 * Call this after closing the writer.
 */
export const unregisterATIFDiskWriter = (sessionId: string): void => {
  diskWriters.delete(sessionId);
};

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
  // Write to disk if a writer is registered for this session
  const diskWriter = diskWriters.get(sessionId);
  if (diskWriter) {
    // Fire and forget - don't block HUD emission on disk I/O
    diskWriter.writeStep(step).catch((err) => {
      console.error(`[ATIF] Failed to write step to disk for session ${sessionId}:`, err);
    });
  }

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
