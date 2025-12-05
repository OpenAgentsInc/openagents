import { beforeEach, describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import {
  createInitialParallelState,
  findAgentState,
  writeParallelState,
  type ParallelRunState,
} from "./state.js";
import { recoverParallelRun } from "./recovery.js";

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "parallel-recovery-"));

describe("recoverParallelRun", () => {
  let dir: string;
  let statePath: string;

  beforeEach(() => {
    dir = tmp();
    statePath = path.join(dir, "parallel-run.json");
  });

  test("processes pending merges and detects completed agents", async () => {
    const state: ParallelRunState = {
      ...createInitialParallelState({
        runId: "run-123",
        workDir: dir,
        mergeTargetBranch: "main",
        maxAgents: 2,
        maxTasks: 2,
      }),
      batches: [
        {
          batchIndex: 0,
          startedAt: "2025-12-04T00:00:00Z",
          agents: [
            {
              taskId: "oa-running",
              worktreePath: path.join(dir, "worktree-running"),
              branch: "agent/oa-running",
              status: "running",
              startedAt: "2025-12-04T00:00:00Z",
            },
            {
              taskId: "oa-pending",
              worktreePath: path.join(dir, "worktree-pending"),
              branch: "agent/oa-pending",
              status: "completed",
              startedAt: "2025-12-04T00:00:00Z",
              completedAt: "2025-12-04T00:05:00Z",
              commitSha: "worktree-sha",
            },
          ],
        },
      ],
      pendingMerges: [
        {
          taskId: "oa-pending",
          worktreePath: path.join(dir, "worktree-pending"),
          branch: "agent/oa-pending",
          commitSha: "worktree-sha",
          queuedAt: "2025-12-04T00:05:00Z",
        },
      ],
    };

    writeParallelState(statePath, state);

    const readyChecks: string[] = [];
    const merges: string[] = [];

    const recovered = await recoverParallelRun(statePath, {
      log: () => {},
      checkAgentReady: async (agent) => {
        readyChecks.push(agent.taskId);
        return { ready: agent.taskId === "oa-running", commitSha: "running-head-sha" };
      },
      processMerge: async (merge) => {
        merges.push(merge.taskId);
        return { success: true, mainCommitSha: `${merge.taskId}-merged` };
      },
    });

    expect(recovered).not.toBeNull();
    expect(readyChecks).toContain("oa-running");
    expect(merges).toEqual(expect.arrayContaining(["oa-pending", "oa-running"]));

    // Pending merges should be drained and completedMerges should include results
    expect(recovered?.pendingMerges.length).toBe(0);
    expect(recovered?.completedMerges.map((m) => m.taskId)).toEqual(
      expect.arrayContaining(["oa-pending", "oa-running"]),
    );

    // Agent states updated
    const runningAgent = findAgentState(recovered!, "oa-running");
    expect(runningAgent?.status).toBe("merged");
    expect(runningAgent?.commitSha).toBe("oa-running-merged");

    const pendingAgent = findAgentState(recovered!, "oa-pending");
    expect(pendingAgent?.status).toBe("merged");
    expect(pendingAgent?.commitSha).toBe("oa-pending-merged");

    // tasksCompleted should reflect processed merges
    expect(recovered?.tasksCompleted).toBe(2);

    // State written back to disk
    const diskState = JSON.parse(fs.readFileSync(statePath, "utf-8"));
    expect(diskState.completedMerges.length).toBe(2);
  });
});
