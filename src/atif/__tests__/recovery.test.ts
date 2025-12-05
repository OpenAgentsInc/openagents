import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { describe, expect, test } from "bun:test";
import {
  appendCheckpoint,
  appendStepToTrajectory,
  initializeTrajectoryFile,
  planRecoveryFromFile,
  recordRecoveryInfo,
} from "../recovery.js";
import { decodeTrajectory, type Agent } from "../schema.js";

const createTempFile = async (name: string) => {
  const dir = await mkdtemp(path.join(tmpdir(), "atif-recovery-"));
  return path.join(dir, name);
};

const baseAgent: Agent = {
  name: "test-agent",
  version: "1.0.0",
  model_name: "gpt-test",
};

describe("ATIF recovery and incremental persistence", () => {
  test("appendStepToTrajectory assigns step ids and tracks statuses", async () => {
    const filePath = await createTempFile("traj.json");
    await initializeTrajectoryFile({
      filePath,
      sessionId: "session-test",
      agent: baseAgent,
    });

    await appendStepToTrajectory(
      filePath,
      { source: "system", message: "start", timestamp: "2024-01-01T00:00:00.000Z" },
      { status: "completed", completedAt: "2024-01-01T00:00:01.000Z" },
    );

    await appendCheckpoint(filePath, {
      checkpoint_id: "chk-1",
      phase: "execute_subtasks",
      resumable: true,
      step_id: 1,
      timestamp: "2024-01-01T00:00:02.000Z",
    });

    await appendStepToTrajectory(
      filePath,
      { source: "agent", message: "work-in-progress" },
      { status: "failed", error: "boom" },
    );

    const plan = await planRecoveryFromFile(filePath);
    expect(plan.resumeFromStepId).toBe(2);
    expect(plan.completedSteps.map((s) => s.step_id)).toEqual([1]);
    expect(plan.stepsToReplay.map((s) => s.step_id)).toEqual([2]);
    expect(plan.checkpoint?.checkpoint_id).toBe("chk-1");
  });

  test("recordRecoveryInfo persists recovery metadata", async () => {
    const filePath = await createTempFile("traj.json");
    await initializeTrajectoryFile({
      filePath,
      sessionId: "session-test-2",
      agent: baseAgent,
    });

    await recordRecoveryInfo(filePath, {
      recovered_from_session: "session-original",
      recovered_at_step: 3,
      notes: "Resumed after crash",
    });

    const raw = await readFile(filePath, "utf8");
    const trajectory = decodeTrajectory(JSON.parse(raw));
    expect(trajectory.recovery_info?.recovered_from_session).toBe("session-original");
    expect(trajectory.recovery_info?.recovered_at_step).toBe(3);
    expect(trajectory.recovery_info?.notes).toBe("Resumed after crash");
  });
});
