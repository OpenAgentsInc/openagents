import { describe, expect, test } from "vite-plus/test";
import { Effect, Exit } from "effect";
import {
  AppleFmBridgeLaunchError,
  discoverAppleFmBridgeHelper,
  launchAppleFmBridge,
  type AppleFmBridgeSpawnedProcess,
} from "./bridge-process.js";

/** Minimal fetch fake: a scripted sequence of health verdicts. */
function fakeHealthFetch(sequence: ReadonlyArray<"ready" | "not_ready" | "throw">): typeof fetch {
  let index = 0;
  return (async () => {
    const verdict = sequence[Math.min(index, sequence.length - 1)];
    index += 1;
    if (verdict === "throw") {
      throw new Error("connection refused");
    }
    return {
      ok: true,
      json: async () => ({ ready: verdict === "ready", model: "apple-foundation-model" }),
    } as unknown as Response;
  }) as unknown as typeof fetch;
}

function recordingSpawn(): { spawn: (command: ReadonlyArray<string>) => AppleFmBridgeSpawnedProcess; killed: () => boolean; commands: ReadonlyArray<ReadonlyArray<string>> } {
  const commands: Array<ReadonlyArray<string>> = [];
  let killed = false;
  return {
    commands,
    killed: () => killed,
    spawn: (command) => {
      commands.push(command);
      return {
        kill: () => {
          killed = true;
        },
        exited: new Promise<number>(() => {}),
      };
    },
  };
}

describe("Apple FM bridge one-shot launcher", () => {
  test("adopts an already-healthy bridge without spawning", async () => {
    const rec = recordingSpawn();
    const handle = await Effect.runPromise(
      launchAppleFmBridge({
        baseUrl: "http://127.0.0.1:11435",
        fetch: fakeHealthFetch(["ready"]),
        spawn: rec.spawn,
        sleep: async () => {},
        pickFreePort: async () => 15999,
      }),
    );
    expect(handle.adopted).toBe(true);
    expect(handle.baseUrl).toBe("http://127.0.0.1:11435");
    expect(rec.commands.length).toBe(0);
    handle.stop();
    expect(rec.killed()).toBe(false);
  });

  test("launches the helper, polls to ready, and reports the chosen port", async () => {
    const rec = recordingSpawn();
    const handle = await Effect.runPromise(
      launchAppleFmBridge({
        helperPath: "/tmp/foundation-bridge",
        adoptIfHealthy: false,
        fetch: fakeHealthFetch(["not_ready", "not_ready", "ready"]),
        spawn: rec.spawn,
        sleep: async () => {},
        pickFreePort: async () => 15999,
        platform: "darwin",
      }),
    );
    expect(handle.adopted).toBe(false);
    expect(handle.port).toBe(15999);
    expect(handle.baseUrl).toBe("http://127.0.0.1:15999");
    expect(rec.commands[0]).toEqual(["/tmp/foundation-bridge", "--port", "15999"]);
    handle.stop();
    expect(rec.killed()).toBe(true);
  });

  test("fails closed with helper_not_found when no helper exists", async () => {
    const exit = await Effect.runPromiseExit(
      launchAppleFmBridge({
        adoptIfHealthy: false,
        platform: "darwin",
        fileExists: () => false,
        env: {},
        cwd: "/nonexistent",
        fetch: fakeHealthFetch(["not_ready"]),
        spawn: recordingSpawn().spawn,
        sleep: async () => {},
      }),
    );
    expect(Exit.isFailure(exit)).toBe(true);
    if (Exit.isFailure(exit) && exit.cause._tag === "Fail") {
      expect(exit.cause.error).toBeInstanceOf(AppleFmBridgeLaunchError);
      expect((exit.cause.error as AppleFmBridgeLaunchError).failureClass).toBe("helper_not_found");
    }
  });

  test("fails closed with unsupported_platform off macOS when no helper path is given", async () => {
    const exit = await Effect.runPromiseExit(
      launchAppleFmBridge({
        adoptIfHealthy: false,
        platform: "linux",
        fetch: fakeHealthFetch(["not_ready"]),
        spawn: recordingSpawn().spawn,
        sleep: async () => {},
      }),
    );
    expect(Exit.isFailure(exit)).toBe(true);
    if (Exit.isFailure(exit) && exit.cause._tag === "Fail") {
      expect((exit.cause.error as AppleFmBridgeLaunchError).failureClass).toBe("unsupported_platform");
    }
  });

  test("times out and stops the child when the bridge never reports ready", async () => {
    const rec = recordingSpawn();
    const exit = await Effect.runPromiseExit(
      launchAppleFmBridge({
        helperPath: "/tmp/foundation-bridge",
        adoptIfHealthy: false,
        platform: "darwin",
        readinessTimeoutMs: 300,
        readinessIntervalMs: 100,
        fetch: fakeHealthFetch(["not_ready"]),
        spawn: rec.spawn,
        sleep: async () => {},
        pickFreePort: async () => 15999,
      }),
    );
    expect(Exit.isFailure(exit)).toBe(true);
    if (Exit.isFailure(exit) && exit.cause._tag === "Fail") {
      expect((exit.cause.error as AppleFmBridgeLaunchError).failureClass).toBe("health_timeout");
    }
    expect(rec.killed()).toBe(true);
  });

  test("discovers a helper from the explicit env path", () => {
    const discovered = discoverAppleFmBridgeHelper({
      env: { OPENAGENTS_APPLE_FM_BRIDGE_PATH: "/opt/foundation-bridge" },
      fileExists: (path) => path === "/opt/foundation-bridge",
    });
    expect(discovered?.source).toBe("env");
    expect(discovered?.path).toBe("/opt/foundation-bridge");
  });
});
