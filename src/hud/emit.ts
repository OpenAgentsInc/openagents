/**
 * HUD Emit Helpers
 *
 * Converts OrchestratorEvents to HudMessages and sends them via HudClient.
 * Use createHudEmitter() to get an emit function compatible with runOrchestrator().
 */

import type { OrchestratorEvent } from "../agent/orchestrator/types.js";
import type { HudMessage, HudTaskInfo, HudSubtaskInfo, HudSubagentResult } from "./protocol.js";
import { HudClient, getHudClient, type HudClientOptions } from "./client.js";

/**
 * Convert an OrchestratorEvent to a HudMessage.
 * Returns null for events that don't need to be sent to the HUD.
 */
export const orchestratorEventToHudMessage = (event: OrchestratorEvent): HudMessage | null => {
  switch (event.type) {
    case "session_start":
      return {
        type: "session_start",
        sessionId: event.sessionId,
        timestamp: event.timestamp,
      };

    case "session_complete":
      return {
        type: "session_complete",
        success: event.success,
        summary: event.summary,
      };

    case "task_selected": {
      const task: HudTaskInfo = {
        id: event.task.id,
        title: event.task.title,
        status: event.task.status,
        priority: event.task.priority,
      };
      return { type: "task_selected", task };
    }

    case "task_decomposed": {
      const subtasks: HudSubtaskInfo[] = event.subtasks.map((s) => ({
        id: s.id,
        description: s.description,
        status: s.status,
      }));
      return { type: "task_decomposed", subtasks };
    }

    case "subtask_start": {
      const subtask: HudSubtaskInfo = {
        id: event.subtask.id,
        description: event.subtask.description,
        status: event.subtask.status,
      };
      return { type: "subtask_start", subtask };
    }

    case "subtask_complete": {
      const subtask: HudSubtaskInfo = {
        id: event.subtask.id,
        description: event.subtask.description,
        status: event.subtask.status,
      };
      const result: HudSubagentResult = {
        success: event.result.success,
        filesModified: event.result.filesModified,
        turns: event.result.turns,
        ...(event.result.agent ? { agent: event.result.agent } : {}),
        ...(event.result.error ? { error: event.result.error } : {}),
      };
      return { type: "subtask_complete", subtask, result };
    }

    case "subtask_failed": {
      const subtask: HudSubtaskInfo = {
        id: event.subtask.id,
        description: event.subtask.description,
        status: event.subtask.status,
      };
      return { type: "subtask_failed", subtask, error: event.error };
    }

    case "verification_start":
      return { type: "verification_start", command: event.command };

    case "verification_complete":
      return {
        type: "verification_complete",
        command: event.command,
        passed: event.passed,
        output: event.output,
      };

    case "commit_created":
      return {
        type: "commit_created",
        sha: event.sha,
        message: event.message,
      };

    case "push_complete":
      return { type: "push_complete", branch: event.branch };

    case "error":
      return {
        type: "error",
        phase: event.phase,
        error: event.error,
      };

    // Events we don't forward to HUD (internal or not useful for display)
    case "orientation_complete":
    case "init_script_start":
    case "init_script_complete":
    case "task_updated":
    case "progress_written":
    case "lock_acquired":
    case "lock_stale_removed":
    case "lock_failed":
    case "lock_released":
      return null;
  }
};

/**
 * Create an emit function that sends OrchestratorEvents to the HUD.
 *
 * Usage:
 *   const emit = createHudEmitter();
 *   runOrchestrator(config, emit);
 */
export const createHudEmitter = (
  clientOptions?: HudClientOptions
): ((event: OrchestratorEvent) => void) => {
  const client = getHudClient(clientOptions);

  return (event: OrchestratorEvent) => {
    const hudMessage = orchestratorEventToHudMessage(event);
    if (hudMessage) {
      client.send(hudMessage);
    }
  };
};

/**
 * Create an output callback that sends streaming text to the HUD.
 *
 * Usage:
 *   const onOutput = createHudOutputCallback();
 *   runOrchestrator({ ...config, onOutput });
 */
export const createHudOutputCallback = (
  clientOptions?: HudClientOptions,
  source: "claude-code" | "minimal" | "orchestrator" = "claude-code"
): ((text: string) => void) => {
  const client = getHudClient(clientOptions);

  return (text: string) => {
    client.send({
      type: "text_output",
      text,
      source,
    });
  };
};

/**
 * Create both an emit function and an onOutput callback that share the same client.
 *
 * Usage:
 *   const { emit, onOutput, client } = createHudCallbacks();
 *   runOrchestrator({ ...config, onOutput }, emit);
 *   // Later:
 *   client.close();
 */
export const createHudCallbacks = (clientOptions?: HudClientOptions) => {
  const client = new HudClient(clientOptions);

  const emit = (event: OrchestratorEvent) => {
    const hudMessage = orchestratorEventToHudMessage(event);
    if (hudMessage) {
      client.send(hudMessage);
    }
  };

  const onOutput = (text: string) => {
    client.send({
      type: "text_output",
      text,
      source: "claude-code",
    });
  };

  return { emit, onOutput, client };
};
