import { promises as fs } from "node:fs";
import * as path from "node:path";
import {
  type Agent,
  type Checkpoint,
  type RecoveryInfo,
  type Step,
  type StepSource,
  type Trajectory,
  ATIF_SCHEMA_VERSION,
  decodeTrajectory,
  encodeTrajectory,
  isStepCompleted,
  latestCheckpoint,
  timestamp,
} from "./schema.js";
import type { IndexData } from "./streaming-writer.js";

export interface StepInput {
  source: StepSource;
  message: unknown;
  timestamp?: string;
  reasoning_content?: string;
  model_name?: string;
  tool_calls?: Step["tool_calls"];
  observation?: Step["observation"];
  metrics?: Step["metrics"];
  extra?: Step["extra"];
}

export interface AppendStepOptions {
  status?: Step["status"];
  completedAt?: string;
  error?: string;
}

export interface RecoveryPlan {
  checkpoint?: Checkpoint;
  resumeFromStepId: number;
  completedSteps: Step[];
  stepsToReplay: Step[];
}

const ensureDir = async (filePath: string) => {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
};

const readTrajectory = async (filePath: string): Promise<Trajectory> => {
  const content = await fs.readFile(filePath, "utf8");
  return decodeTrajectory(JSON.parse(content));
};

const writeTrajectory = async (
  filePath: string,
  trajectory: Trajectory,
): Promise<void> => {
  await ensureDir(filePath);
  await fs.writeFile(filePath, JSON.stringify(encodeTrajectory(trajectory), null, 2), "utf8");
};

/**
 * Create a new trajectory file with no steps.
 */
export const initializeTrajectoryFile = async (options: {
  filePath: string;
  sessionId: string;
  agent: Agent;
  notes?: string;
}): Promise<Trajectory> => {
  const trajectory: Trajectory = {
    schema_version: "ATIF-v1.4",
    session_id: options.sessionId,
    agent: options.agent,
    steps: [],
    notes: options.notes,
  };
  await writeTrajectory(options.filePath, trajectory);
  return trajectory;
};

/**
 * Append a step to a trajectory file, auto-assigning the next step_id.
 */
export const appendStepToTrajectory = async (
  filePath: string,
  input: StepInput,
  options: AppendStepOptions = {},
): Promise<Step> => {
  const trajectory = await readTrajectory(filePath);
  const stepId = trajectory.steps.length + 1;
  const step: Step = {
    step_id: stepId,
    timestamp: input.timestamp ?? timestamp(),
    source: input.source,
    message: input.message,
    reasoning_content: input.reasoning_content,
    model_name: input.model_name,
    tool_calls: input.tool_calls,
    observation: input.observation,
    metrics: input.metrics,
    extra: input.extra,
    status: options.status ?? "completed",
    error: options.error,
    ...(options.completedAt || options.status === "completed"
      ? { completed_at: options.completedAt ?? timestamp() }
      : {}),
  };

  const updatedTrajectory: Trajectory = {
    ...trajectory,
    steps: [...trajectory.steps, step],
  };
  await writeTrajectory(filePath, updatedTrajectory);
  return step;
};

/**
 * Append a checkpoint marker to the trajectory file.
 */
export const appendCheckpoint = async (
  filePath: string,
  checkpoint: Omit<Checkpoint, "timestamp"> & { timestamp?: string },
): Promise<Checkpoint> => {
  const trajectory = await readTrajectory(filePath);
  const entry: Checkpoint = {
    ...checkpoint,
    timestamp: checkpoint.timestamp ?? timestamp(),
  };
  const checkpoints = trajectory.checkpoints ?? [];
  const updatedTrajectory: Trajectory = {
    ...trajectory,
    checkpoints: [...checkpoints, entry],
  };
  await writeTrajectory(filePath, updatedTrajectory);
  return entry;
};

/**
 * Compute which steps can be replayed from a trajectory.
 */
export const planRecoveryFromTrajectory = (
  trajectory: Trajectory,
): RecoveryPlan => {
  const checkpoint = latestCheckpoint(trajectory);
  const resumeFromStepId = checkpoint?.step_id
    ? checkpoint.step_id + 1
    : 1;

  const completedSteps = trajectory.steps.filter(isStepCompleted);
  const stepsToReplay = trajectory.steps.filter(
    (step) =>
      step.step_id >= resumeFromStepId && !isStepCompleted(step),
  );

  const plan: RecoveryPlan = {
    resumeFromStepId,
    completedSteps,
    stepsToReplay,
    ...(checkpoint ? { checkpoint } : {}),
  };

  return plan;
};

/**
 * Load a trajectory from disk and compute the recovery plan.
 */
export const planRecoveryFromFile = async (
  filePath: string,
): Promise<RecoveryPlan> => {
  const trajectory = await readTrajectory(filePath);
  return planRecoveryFromTrajectory(trajectory);
};

/**
 * Record recovery metadata on an existing trajectory file.
 */
export const recordRecoveryInfo = async (
  filePath: string,
  info: RecoveryInfo,
): Promise<RecoveryInfo> => {
  const trajectory = await readTrajectory(filePath);
  const recovery: RecoveryInfo = {
    ...trajectory.recovery_info,
    ...info,
    recovery_timestamp: info.recovery_timestamp ?? timestamp(),
  };
  const updatedTrajectory: Trajectory = {
    ...trajectory,
    recovery_info: recovery,
  };
  await writeTrajectory(filePath, updatedTrajectory);
  return recovery;
};

// ============================================================================
// JSONL Streaming Format Support
// ============================================================================

interface JSONLHeader {
  __header__: true;
  schema_version: string;
  session_id: string;
  agent: Agent;
  created_at: string;
  parent_session_id?: string;
}

/**
 * Load index file for streaming trajectory
 */
export const loadIndex = async (indexPath: string): Promise<IndexData> => {
  const content = await fs.readFile(indexPath, "utf-8");
  return JSON.parse(content) as IndexData;
};

/**
 * Load trajectory from JSONL format
 *
 * JSONL format:
 * - Line 1: Header with session metadata
 * - Lines 2+: Individual steps (one per line)
 *
 * Skips incomplete final line if present (crash recovery).
 */
export const loadFromJSONL = async (jsonlPath: string): Promise<Trajectory> => {
  const content = await fs.readFile(jsonlPath, "utf-8");
  const lines = content.split("\n").filter((line) => line.trim().length > 0);

  if (lines.length === 0) {
    throw new Error(`Empty JSONL file: ${jsonlPath}`);
  }

  // Parse header (first line)
  let header: JSONLHeader;
  try {
    header = JSON.parse(lines[0]!) as JSONLHeader;
    if (!header.__header__) {
      throw new Error("First line must be header");
    }
  } catch (e) {
    throw new Error(`Invalid JSONL header in ${jsonlPath}: ${e}`);
  }

  // Parse steps (skip header, attempt to parse each line)
  const steps: Step[] = [];
  for (let i = 1; i < lines.length; i++) {
    try {
      const step = JSON.parse(lines[i]!) as Step;
      steps.push(step);
    } catch (e) {
      // Skip incomplete/malformed lines (crash recovery)
      console.warn(`[Recovery] Skipping malformed line ${i + 1} in ${jsonlPath}`);
    }
  }

  // Reconstruct trajectory
  const trajectory: Trajectory = {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: header.session_id,
    agent: header.agent,
    steps,
    ...(header.parent_session_id && {
      extra: { parent_session_id: header.parent_session_id },
    }),
  };

  return trajectory;
};

/**
 * Load trajectory from JSONL and compute recovery plan
 */
export const loadTrajectoryFromJSONL = async (
  jsonlPath: string,
  indexPath: string,
): Promise<{ trajectory: Trajectory; index: IndexData; recoveryPlan: RecoveryPlan }> => {
  const trajectory = await loadFromJSONL(jsonlPath);
  const index = await loadIndex(indexPath);

  const recoveryPlan = planRecoveryFromTrajectory(trajectory);

  return { trajectory, index, recoveryPlan };
};

/**
 * Detect if a trajectory is incomplete (crashed mid-execution)
 */
export const detectIncompleteTrajectory = async (
  indexPath: string,
): Promise<boolean> => {
  try {
    const index = await loadIndex(indexPath);
    return index.status === "in_progress";
  } catch {
    return false; // No index = no incomplete trajectory
  }
};

/**
 * Get recovery plan for a session ID
 * Supports both JSON (.atif.json) and JSONL (.atif.jsonl) formats
 */
export const getRecoveryPlan = async (
  sessionId: string,
  baseDir: string = ".openagents/trajectories",
): Promise<RecoveryPlan | null> => {
  // Extract date from session ID
  const match = sessionId.match(/session-(\d{4})-(\d{2})-(\d{2})/);
  if (!match) {
    console.warn(`[Recovery] Cannot extract date from session ID: ${sessionId}`);
    return null;
  }

  const dateStr = `${match[1]}${match[2]}${match[3]}`; // YYYYMMDD
  const dateDir = path.join(baseDir, dateStr);

  // Try JSONL format first (new)
  const jsonlPath = path.join(dateDir, `${sessionId}.atif.jsonl`);
  const indexPath = path.join(dateDir, `${sessionId}.index.json`);

  try {
    const isIncomplete = await detectIncompleteTrajectory(indexPath);
    if (isIncomplete) {
      const { recoveryPlan } = await loadTrajectoryFromJSONL(jsonlPath, indexPath);
      return recoveryPlan;
    }
  } catch {
    // JSONL format not found or error, try legacy JSON format
  }

  // Fallback to JSON format (legacy)
  const jsonPath = path.join(dateDir, `${sessionId}.atif.json`);
  try {
    return await planRecoveryFromFile(jsonPath);
  } catch {
    // No recovery available
    return null;
  }
};
