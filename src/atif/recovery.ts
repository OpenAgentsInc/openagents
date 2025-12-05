import { promises as fs } from "node:fs";
import * as path from "node:path";
import {
  type Agent,
  type Checkpoint,
  type RecoveryInfo,
  type Step,
  type StepSource,
  type Trajectory,
  decodeTrajectory,
  encodeTrajectory,
  isStepCompleted,
  latestCheckpoint,
  timestamp,
} from "./schema.js";

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
