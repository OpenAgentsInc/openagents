import { describe, expect, test } from "vite-plus/test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

type Exit = { code: number | null; signal: NodeJS.Signals | null };
type SupervisorModule = {
  classifyDevExit: (exit: Exit) => string;
  superviseRestart: (
    config: Record<string, unknown>,
    dependencies: Record<string, unknown>,
  ) => Promise<unknown>;
};

const appRoot = path.resolve(import.meta.dirname, "..");
const supervisor = (await import(
  pathToFileURL(path.join(appRoot, "scripts", "oa-dev-supervisor.mjs")).href
)) as SupervisorModule;

const fixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-dev-supervisor-"));
  const lockDir = path.join(root, "active.lock");
  mkdirSync(lockDir);
  return {
    root,
    lockDir,
    receiptPath: path.join(root, "receipt.json"),
    config: {
      requestRef: "restart-fixture",
      requestedAt: "2026-07-17T00:00:00.000Z",
      oldCommit: "1".repeat(40),
      targetSha: "2".repeat(40),
      oldAppPid: 101,
      oldProcessGroupId: 100,
      sourceRepo: path.join(root, "source"),
      launchRepo: path.join(root, "launch"),
      receiptPath: path.join(root, "receipt.json"),
      lockDir,
      logPath: path.join(root, "restart.log"),
      ownerLaunchCwd: root,
      rendererPort: 5734,
      graceMs: 0,
      quitTimeoutMs: 1,
      groupTerminateTimeoutMs: 1,
      forceQuitTimeoutMs: 1,
      portReleaseTimeoutMs: 1,
      readinessTimeoutMs: 1,
    },
  };
};

describe("oa-dev launchd restart supervisor", () => {
  test("does not touch the launch worktree until the old process group is gone", async () => {
    const value = fixture();
    const events: string[] = [];
    let oldGroupAlive = true;
    let rendererAccepting = false;
    const childExit: Exit = { code: 143, signal: null };
    try {
      await supervisor.superviseRestart(value.config, {
        now: () => `time-${events.length}`,
        sleep: async () => {
          events.push("grace");
        },
        coordinatorProcessGroupId: () => 900,
        oldProcessGroupAlive: () => oldGroupAlive,
        requestQuit: () => {
          events.push("quit-old-app");
          oldGroupAlive = false;
        },
        terminateOldProcessGroup: (signal: string) => {
          events.push(`terminate-${signal}`);
          oldGroupAlive = false;
        },
        rendererPortAccepting: async () => rendererAccepting,
        syncLaunchWorktree: () => {
          expect(oldGroupAlive).toBe(false);
          events.push("sync-launch-worktree");
        },
        ensureDependencies: () => {
          events.push("ensure-dependencies");
        },
        launchDev: () => {
          events.push("launch-new-dev-group");
          rendererAccepting = true;
          return { pid: 201, alive: () => true, exited: Promise.resolve(childExit) };
        },
        findNewAppPid: () => 202,
        rendererReady: () => true,
        notifyFailure: () => {
          throw new Error("unexpected notification");
        },
        releaseLock: () => {
          events.push("release-lock");
          rmSync(value.lockDir, { recursive: true, force: true });
        },
      });

      expect(events).toEqual([
        "grace",
        "quit-old-app",
        "sync-launch-worktree",
        "ensure-dependencies",
        "launch-new-dev-group",
        "release-lock",
      ]);
      expect(JSON.parse(readFileSync(value.receiptPath, "utf8"))).toMatchObject({
        schemaVersion: "openagents.desktop.dev_restart_receipt.v1",
        state: "stopped",
        ready: true,
        oldProcessGroupId: 100,
        newDevPid: 201,
        newAppPid: 202,
        exitClassification: "expected_supervised_sigterm",
      });
    } finally {
      rmSync(value.root, { recursive: true, force: true });
    }
  });

  test("fails closed before shutdown when the coordinator shares the old process group", async () => {
    const value = fixture();
    const events: string[] = [];
    try {
      await expect(
        supervisor.superviseRestart(value.config, {
          now: () => "time",
          sleep: async () => undefined,
          coordinatorProcessGroupId: () => 100,
          oldProcessGroupAlive: () => true,
          requestQuit: () => {
            events.push("quit");
          },
          terminateOldProcessGroup: () => {
            events.push("terminate");
          },
          rendererPortAccepting: async () => false,
          syncLaunchWorktree: () => {
            events.push("sync");
          },
          ensureDependencies: () => {
            events.push("deps");
          },
          launchDev: () => {
            events.push("launch");
            throw new Error("unreachable");
          },
          findNewAppPid: () => null,
          rendererReady: () => false,
          notifyFailure: () => {
            events.push("notify");
          },
          releaseLock: () => {
            events.push("release-lock");
            rmSync(value.lockDir, { recursive: true, force: true });
          },
        }),
      ).rejects.toThrow("still inside the process group");
      expect(events).toEqual(["release-lock", "notify"]);
      expect(JSON.parse(readFileSync(value.receiptPath, "utf8"))).toMatchObject({
        state: "failed",
        ready: false,
        recoveryCommand: "oa-dev",
      });
    } finally {
      rmSync(value.root, { recursive: true, force: true });
    }
  });

  test("classifies only supervised TERM/143 as an expected lifecycle exit", () => {
    expect(supervisor.classifyDevExit({ code: 143, signal: null })).toBe(
      "expected_supervised_sigterm",
    );
    expect(supervisor.classifyDevExit({ code: null, signal: "SIGTERM" })).toBe(
      "expected_supervised_sigterm",
    );
    expect(supervisor.classifyDevExit({ code: 0, signal: null })).toBe("normal_exit");
    expect(supervisor.classifyDevExit({ code: 1, signal: null })).toBe("unexpected_exit");
  });
});
