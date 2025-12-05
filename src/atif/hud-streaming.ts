/**
 * ATIF HUD Streaming - Emit trajectory events to HUD in real-time
 *
 * Sends trajectory progress updates to the desktop HUD as steps are captured.
 * Enables real-time visualization of agent execution.
 */

import type {
  ATIFTrajectoryStartMessage,
  ATIFStepRecordedMessage,
  ATIFTrajectoryCompleteMessage,
} from "../hud/protocol.js";
import type { Step, FinalMetrics, Agent } from "./schema.js";
import { sendToHud } from "../hud/client.js";

/**
 * Emit trajectory start event to HUD
 */
export const emitTrajectoryStart = (options: {
  sessionId: string;
  agent: Agent;
  agentType: "orchestrator" | "claude-code" | "minimal";
  parentSessionId?: string;
}): void => {
  const message: ATIFTrajectoryStartMessage = {
    type: "atif_trajectory_start",
    sessionId: options.sessionId,
    agentName: options.agent.name,
    agentType: options.agentType,
  };
  if (options.parentSessionId !== undefined) {
    message.parentSessionId = options.parentSessionId;
  }

  sendToHud(message);
};

/**
 * Emit step recorded event to HUD
 */
export const emitStepRecorded = (sessionId: string, step: Step): void => {
  const message: ATIFStepRecordedMessage = {
    type: "atif_step_recorded",
    sessionId,
    stepId: step.step_id,
    source: step.source,
    hasToolCalls: !!step.tool_calls && step.tool_calls.length > 0,
    hasObservation: !!step.observation,
  };

  sendToHud(message);
};

/**
 * Emit trajectory complete event to HUD
 */
export const emitTrajectoryComplete = (options: {
  sessionId: string;
  trajectoryPath: string;
  totalSteps: number;
  finalMetrics: FinalMetrics;
}): void => {
  const message: ATIFTrajectoryCompleteMessage = {
    type: "atif_trajectory_complete",
    sessionId: options.sessionId,
    totalSteps: options.totalSteps,
    totalTokens: {
      prompt: options.finalMetrics.total_prompt_tokens,
      completion: options.finalMetrics.total_completion_tokens,
    },
    trajectoryPath: options.trajectoryPath,
  };
  if (options.finalMetrics.total_cached_tokens !== undefined) {
    message.totalTokens!.cached = options.finalMetrics.total_cached_tokens;
  }
  if (options.finalMetrics.total_cost_usd !== undefined) {
    message.totalCostUsd = options.finalMetrics.total_cost_usd;
  }

  sendToHud(message);
};
