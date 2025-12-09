/**
 * HillClimber HUD Event Emitter
 *
 * Emits structured events to the desktop server for real-time UI updates.
 * Used by MAP orchestrator and TestGen to stream progress to the UI.
 *
 * Usage:
 *   const emitter = createHillClimberHudEmitter(sessionId);
 *   emitter.onTurnStart(1, 10, "write-regex");
 *   // ... later
 *   emitter.close();
 */

import { HudClient } from "../hud/client.js";
import type { HudMessage } from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

export interface HillClimberHudEmitter {
  // TestGen phase events
  onTestGenStart(taskId: string, description: string): void;
  onTestGenCategory(category: string, testCount: number): void;
  onTestGenComplete(totalTests: number, score: number): void;

  // MAP Orchestrator phase events
  onTurnStart(turn: number, maxTurns: number, subtask: string): void;
  onFMAction(action: "thinking" | "tool_call" | "complete", toolName?: string): void;
  onVerifyStart(): void;
  onVerifyComplete(passed: number, total: number, progress: number): void;
  onSubtaskChange(subtask: string, status: "active" | "completed" | "failed"): void;
  onHeartbeat(turn: number, maxTurns: number, progress: number, bestProgress: number, elapsedMs: number): void;
  onRunComplete(success: boolean, finalProgress: number): void;

  // Lifecycle
  close(): void;
}

// ============================================================================
// Implementation
// ============================================================================

export function createHillClimberHudEmitter(sessionId: string): HillClimberHudEmitter {
  const client = new HudClient({ verbose: false });

  const send = (message: HudMessage) => {
    client.send(message);
  };

  return {
    // TestGen phase events
    onTestGenStart(taskId: string, description: string): void {
      send({
        type: "testgen_start",
        sessionId,
        taskId,
        taskDescription: description,
        environment: {
          platform: process.platform,
          prohibitedTools: [],
          languages: [],
          fileCount: 0,
          filePreviews: 0,
        },
      });
    },

    onTestGenCategory(category: string, testCount: number): void {
      send({
        type: "testgen_progress",
        sessionId,
        phase: "category_generation",
        currentCategory: category,
        roundNumber: testCount,
        status: `Generated ${testCount} tests for ${category}`,
      });
    },

    onTestGenComplete(totalTests: number, score: number): void {
      send({
        type: "testgen_complete",
        sessionId,
        totalTests,
        totalRounds: 1,
        categoryRounds: {},
        comprehensivenessScore: score,
        totalTokensUsed: 0,
        durationMs: 0,
        uncertainties: [],
      });
    },

    // MAP Orchestrator phase events
    onTurnStart(turn: number, maxTurns: number, subtask: string): void {
      send({
        type: "map_turn_start",
        sessionId,
        turn,
        maxTurns,
        subtask,
      } as HudMessage);
    },

    onFMAction(action: "thinking" | "tool_call" | "complete", toolName?: string): void {
      send({
        type: "map_fm_action",
        sessionId,
        action,
        toolName,
      } as HudMessage);
    },

    onVerifyStart(): void {
      send({
        type: "map_verify",
        sessionId,
        status: "running",
      } as HudMessage);
    },

    onVerifyComplete(passed: number, total: number, progress: number): void {
      send({
        type: "map_verify",
        sessionId,
        status: "complete",
        passed,
        total,
        progress,
      } as HudMessage);
    },

    onSubtaskChange(subtask: string, status: "active" | "completed" | "failed"): void {
      send({
        type: "map_subtask_change",
        sessionId,
        subtask,
        status,
      } as HudMessage);
    },

    onHeartbeat(turn: number, maxTurns: number, progress: number, bestProgress: number, elapsedMs: number): void {
      send({
        type: "map_heartbeat",
        sessionId,
        turn,
        maxTurns,
        progress,
        bestProgress,
        elapsedMs,
      } as HudMessage);
    },

    onRunComplete(success: boolean, finalProgress: number): void {
      send({
        type: "map_run_complete",
        sessionId,
        success,
        finalProgress,
      } as HudMessage);
    },

    close(): void {
      client.close();
    },
  };
}

// ============================================================================
// No-op Emitter (for when HUD isn't needed)
// ============================================================================

export function createNoopHillClimberHudEmitter(): HillClimberHudEmitter {
  return {
    onTestGenStart: () => {},
    onTestGenCategory: () => {},
    onTestGenComplete: () => {},
    onTurnStart: () => {},
    onFMAction: () => {},
    onVerifyStart: () => {},
    onVerifyComplete: () => {},
    onSubtaskChange: () => {},
    onHeartbeat: () => {},
    onRunComplete: () => {},
    close: () => {},
  };
}
