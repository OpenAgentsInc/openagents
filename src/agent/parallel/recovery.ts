import {
  type AgentState,
  type MergeRequest,
  type ParallelRunState,
  type MergeResult,
  findAgentState,
  readParallelState,
  writeParallelState,
} from "./state.js";

interface RecoveryHooks {
  log: (msg: string) => void;
  checkAgentReady: (agent: AgentState) => Promise<{ ready: boolean; commitSha?: string }>;
  processMerge: (
    merge: MergeRequest,
  ) => Promise<{ success: boolean; mainCommitSha?: string | undefined; error?: string | undefined }>;
}

export const recoverParallelRun = async (
  statePath: string,
  hooks: RecoveryHooks,
): Promise<ParallelRunState | null> => {
  const state = readParallelState(statePath);
  if (!state) return null;

  let changed = false;

  hooks.log(`[Recovery] Found existing parallel run state (${state.runId}), attempting resume`);

  // Check agents that were marked running/completed to see if their worktrees finished while parent died
  for (const batch of state.batches) {
    for (const agent of batch.agents) {
      if (agent.status === "running" || agent.status === "completed") {
        const result = await hooks.checkAgentReady(agent);
        if (result.ready) {
          const queuedAt = new Date().toISOString();
          state.pendingMerges.push({
            taskId: agent.taskId,
            worktreePath: agent.worktreePath,
            branch: agent.branch,
            commitSha: result.commitSha,
            queuedAt,
          });
          agent.status = "completed";
          agent.completedAt = queuedAt;
          agent.commitSha = result.commitSha ?? agent.commitSha;
          changed = true;
          hooks.log(`[Recovery] Queued merge for completed worktree ${agent.taskId}`);
        }
      }
    }
  }

  if (changed) {
    writeParallelState(statePath, state);
  }

  // Process pending merges
  for (const merge of [...state.pendingMerges]) {
    const mergeResult = await hooks.processMerge(merge);
    const completed: MergeResult = {
      ...mergeResult,
      taskId: merge.taskId,
      mergedAt: new Date().toISOString(),
    };
    state.completedMerges.push(completed);

    if (mergeResult.success) {
      state.pendingMerges = state.pendingMerges.filter((m) => m !== merge);
      state.tasksCompleted += 1;
      const agent = findAgentState(state, merge.taskId);
      if (agent) {
        agent.status = "merged";
        agent.commitSha = mergeResult.mainCommitSha ?? agent.commitSha;
        agent.completedAt = completed.mergedAt;
      }
      hooks.log(`[Recovery] Merged pending worktree for ${merge.taskId}`);
    } else {
      const agent = findAgentState(state, merge.taskId);
      if (agent) {
        agent.status = "failed";
        agent.error = mergeResult.error ?? "merge_failed";
      }
      hooks.log(`[Recovery] Merge failed for ${merge.taskId}: ${mergeResult.error ?? "unknown error"}`);
    }

    writeParallelState(statePath, state);
  }

  return state;
};
