import * as fs from "node:fs";
import * as path from "node:path";

export type AgentStatus = "pending" | "running" | "completed" | "failed" | "merged";

export interface AgentState {
  taskId: string;
  worktreePath: string;
  branch: string;
  status: AgentStatus;
  startedAt?: string | undefined;
  completedAt?: string | undefined;
  commitSha?: string | undefined;
  error?: string | undefined;
}

export interface BatchState {
  batchIndex: number;
  startedAt: string;
  completedAt?: string;
  agents: AgentState[];
}

export interface MergeRequest {
  taskId: string;
  worktreePath: string;
  branch: string;
  commitSha?: string | undefined;
  queuedAt: string;
}

export interface MergeResult {
  taskId: string;
  mergedAt: string;
  mainCommitSha?: string | undefined;
  success: boolean;
  error?: string | undefined;
}

export interface ParallelRunState {
  version: 1;
  runId: string;
  startedAt: string;
  config: {
    workDir: string;
    mergeTargetBranch: string;
    maxAgents: number;
    maxTasks: number;
  };
  batches: BatchState[];
  currentBatchIndex: number;
  tasksCompleted: number;
  tasksFailed: number;
  pendingMerges: MergeRequest[];
  completedMerges: MergeResult[];
}

export const createInitialParallelState = (params: {
  runId: string;
  workDir: string;
  mergeTargetBranch: string;
  maxAgents: number;
  maxTasks: number;
}): ParallelRunState => ({
  version: 1,
  runId: params.runId,
  startedAt: new Date().toISOString(),
  config: {
    workDir: params.workDir,
    mergeTargetBranch: params.mergeTargetBranch,
    maxAgents: params.maxAgents,
    maxTasks: params.maxTasks,
  },
  batches: [],
  currentBatchIndex: 0,
  tasksCompleted: 0,
  tasksFailed: 0,
  pendingMerges: [],
  completedMerges: [],
});

export const readParallelState = (statePath: string): ParallelRunState | null => {
  if (!fs.existsSync(statePath)) return null;

  try {
    const raw = fs.readFileSync(statePath, "utf-8");
    const parsed = JSON.parse(raw) as ParallelRunState;
    return parsed;
  } catch (error) {
    // If parsing fails, treat as no state to avoid blocking recovery
    return null;
  }
};

export const writeParallelState = (statePath: string, state: ParallelRunState): void => {
  fs.mkdirSync(path.dirname(statePath), { recursive: true });
  fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
};

export const clearParallelState = (statePath: string): void => {
  if (fs.existsSync(statePath)) {
    fs.rmSync(statePath);
  }
};

export const findAgentState = (state: ParallelRunState, taskId: string): AgentState | undefined => {
  for (const batch of state.batches) {
    const agent = batch.agents.find((a) => a.taskId === taskId);
    if (agent) return agent;
  }
  return undefined;
};
