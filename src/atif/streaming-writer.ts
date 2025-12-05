/**
 * StreamingWriter - Incremental JSONL writer for ATIF trajectories
 *
 * Writes trajectories to disk incrementally as steps are captured:
 * - .atif.jsonl: Append-only step log (one step per line)
 * - .index.json: Metadata + checkpoint (atomic updates)
 *
 * Benefits:
 * - Zero data loss (each step flushed immediately)
 * - Crash-safe (incomplete lines skipped on recovery)
 * - Real-time progress tracking
 * - Partial trajectory recovery
 */

import { join } from "node:path";
import { mkdir, rename, writeFile, appendFile, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import type { Step, Agent, FinalMetrics } from "./schema.js";
import { timestamp } from "./schema.js";
import { emitTrajectoryStart, emitStepRecorded, emitTrajectoryComplete } from "./hud-streaming.js";

// ============================================================================
// Types
// ============================================================================

export interface StreamingWriterOptions {
  /** Session ID for this trajectory */
  sessionId: string;
  /** Agent metadata */
  agent: Agent;
  /** Base directory for trajectories (default: .openagents/trajectories) */
  baseDir?: string;
  /** Parent session ID for subagent linking */
  parentSessionId?: string;
  /** Agent type for HUD display */
  agentType?: "orchestrator" | "claude-code" | "minimal";
  /** Enable HUD event emission (default: true) */
  emitHudEvents?: boolean;
}

export interface IndexData {
  session_id: string;
  agent: Agent;
  checkpoint: {
    step_id: number;
    timestamp: string;
    completed_step_count: number;
  };
  status: "in_progress" | "complete" | "failed";
  final_metrics: FinalMetrics | null;
  parent_session_id?: string;
  created_at: string;
}

export interface StreamingWriterPaths {
  jsonl: string;
  index: string;
  dateDir: string;
}

// ============================================================================
// StreamingWriter Class
// ============================================================================

export class StreamingWriter {
  private readonly sessionId: string;
  private readonly agent: Agent;
  private readonly baseDir: string;
  private readonly paths: StreamingWriterPaths;
  private readonly parentSessionId?: string;
  private readonly agentType: "orchestrator" | "claude-code" | "minimal";
  private readonly emitHudEvents: boolean;
  private readonly createdAt: string;

  private stepCount = 0;
  private closed = false;

  constructor(options: StreamingWriterOptions) {
    this.sessionId = options.sessionId;
    this.agent = options.agent;
    this.baseDir = options.baseDir ?? ".openagents/trajectories";
    this.agentType = options.agentType ?? "orchestrator";
    this.emitHudEvents = options.emitHudEvents ?? true;
    if (options.parentSessionId !== undefined) {
      this.parentSessionId = options.parentSessionId;
    }
    this.createdAt = timestamp();

    // Compute paths
    const dateStr = this.extractDateFromSessionId(this.sessionId);
    const dateDir = join(this.baseDir, dateStr);
    const jsonlPath = join(dateDir, `${this.sessionId}.atif.jsonl`);
    const indexPath = join(dateDir, `${this.sessionId}.index.json`);

    this.paths = {
      jsonl: jsonlPath,
      index: indexPath,
      dateDir,
    };
  }

  /**
   * Initialize: Create directories and write JSONL header + initial index
   */
  async initialize(): Promise<void> {
    if (this.closed) {
      throw new Error("StreamingWriter already closed");
    }

    // Create date directory if needed
    if (!existsSync(this.paths.dateDir)) {
      await mkdir(this.paths.dateDir, { recursive: true });
    }

    // Write JSONL header (first line)
    const header: Record<string, unknown> = {
      __header__: true,
      schema_version: "ATIF-v1.4",
      session_id: this.sessionId,
      agent: this.agent,
      created_at: this.createdAt,
    };
    if (this.parentSessionId) {
      header.parent_session_id = this.parentSessionId;
    }
    await writeFile(this.paths.jsonl, JSON.stringify(header) + "\n", "utf-8");

    // Write initial index
    await this.updateIndex({
      status: "in_progress",
      final_metrics: null,
    });

    // Emit HUD event for trajectory start
    if (this.emitHudEvents) {
      const startOpts: Parameters<typeof emitTrajectoryStart>[0] = {
        sessionId: this.sessionId,
        agent: this.agent,
        agentType: this.agentType,
      };
      if (this.parentSessionId !== undefined) {
        startOpts.parentSessionId = this.parentSessionId;
      }
      emitTrajectoryStart(startOpts);
    }
  }

  /**
   * Append a step to the JSONL file and update index checkpoint
   */
  async writeStep(step: Step): Promise<void> {
    if (this.closed) {
      throw new Error("StreamingWriter already closed");
    }

    // Append step as single line
    await appendFile(this.paths.jsonl, JSON.stringify(step) + "\n", "utf-8");

    this.stepCount++;

    // Update index checkpoint
    await this.updateIndex({
      status: "in_progress",
      final_metrics: null,
    });

    // Emit HUD event for step recorded
    if (this.emitHudEvents) {
      emitStepRecorded(this.sessionId, step);
    }
  }

  /**
   * Finalize trajectory with final_metrics and status
   */
  async close(finalMetrics: FinalMetrics, status: "complete" | "failed" = "complete"): Promise<StreamingWriterPaths> {
    if (this.closed) {
      throw new Error("StreamingWriter already closed");
    }

    this.closed = true;

    // Final index update
    await this.updateIndex({
      status,
      final_metrics: finalMetrics,
    });

    // Emit HUD event for trajectory complete
    if (this.emitHudEvents && status === "complete") {
      emitTrajectoryComplete({
        sessionId: this.sessionId,
        trajectoryPath: this.paths.jsonl,
        totalSteps: this.stepCount,
        finalMetrics,
      });
    }

    return this.paths;
  }

  /**
   * Get file paths
   */
  getPaths(): StreamingWriterPaths {
    return this.paths;
  }

  /**
   * Get current step count
   */
  getStepCount(): number {
    return this.stepCount;
  }

  // ===========================================================================
  // Private Helpers
  // ===========================================================================

  /**
   * Update index file atomically via .tmp rename
   */
  private async updateIndex(updates: {
    status: "in_progress" | "complete" | "failed";
    final_metrics: FinalMetrics | null;
  }): Promise<void> {
    await mkdir(this.paths.dateDir, { recursive: true });

    const indexData: IndexData = {
      session_id: this.sessionId,
      agent: this.agent,
      checkpoint: {
        step_id: this.stepCount,
        timestamp: timestamp(),
        completed_step_count: this.stepCount,
      },
      status: updates.status,
      final_metrics: updates.final_metrics,
      created_at: this.createdAt,
    };
    if (this.parentSessionId) {
      indexData.parent_session_id = this.parentSessionId;
    }

    const indexJson = JSON.stringify(indexData, null, 2);

    const writeAndRename = async () => {
      const tmpPath = this.buildTempIndexPath();
      try {
        await writeFile(tmpPath, indexJson, "utf-8");
        await rename(tmpPath, this.paths.index);
      } finally {
        await rm(tmpPath, { force: true });
      }
    };

    try {
      await writeAndRename();
    } catch (error: any) {
      if (error?.code === "ENOENT") {
        await mkdir(this.paths.dateDir, { recursive: true });
        await writeAndRename();
      } else {
        throw error;
      }
    }
  }

  /**
   * Extract YYYYMMDD from session ID
   */
  private extractDateFromSessionId(sessionId: string): string {
    // Session ID format: session-2025-12-05T05-42-15-607Z-3xqeb8
    const match = sessionId.match(/session-(\d{4})-(\d{2})-(\d{2})/);
    if (match) {
      return `${match[1]}${match[2]}${match[3]}`; // YYYYMMDD
    }
    // Fallback to current date
    const now = new Date();
    const year = now.getFullYear();
    const month = String(now.getMonth() + 1).padStart(2, "0");
    const day = String(now.getDate()).padStart(2, "0");
    return `${year}${month}${day}`;
  }

  private buildTempIndexPath(): string {
    const suffix = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    return `${this.paths.index}.${suffix}.tmp`;
  }
}
