#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  fsyncSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const receiptSchema = "openagents.desktop.dev_restart_receipt.v1";
const coordinatorVersion = 1;

const isoNow = () => new Date().toISOString();
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

const bounded = (value, max = 2_000) =>
  String(value ?? "")
    .replaceAll(/[\r\n\t]+/gu, " ")
    .slice(0, max);

const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });
  if (result.status !== 0) {
    const detail = bounded(
      result.stderr || result.stdout || result.error?.message || "unknown failure",
    );
    throw new Error(
      `${path.basename(command)} ${args[0] ?? ""} failed (${String(result.status)}): ${detail}`,
    );
  }
  return result.stdout ?? "";
};

export const classifyDevExit = ({ code, signal }) => {
  if (signal === "SIGTERM" || code === 143) return "expected_supervised_sigterm";
  if (signal === null && code === 0) return "normal_exit";
  return "unexpected_exit";
};

export const writeRestartReceipt = (receiptPath, document) => {
  mkdirSync(path.dirname(receiptPath), { recursive: true, mode: 0o700 });
  const next = `${receiptPath}.next-${process.pid}`;
  writeFileSync(
    next,
    `${JSON.stringify({ schemaVersion: receiptSchema, coordinatorVersion, ...document }, null, 2)}\n`,
    {
      encoding: "utf8",
      mode: 0o600,
    },
  );
  const nextFd = openSync(next, "r");
  fsyncSync(nextFd);
  closeSync(nextFd);
  renameSync(next, receiptPath);
  const parentFd = openSync(path.dirname(receiptPath), "r");
  fsyncSync(parentFd);
  closeSync(parentFd);
};

const pidAlive = (pid) => {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
};

const processGroupId = (pid) => {
  const result = spawnSync("/bin/ps", ["-o", "pgid=", "-p", String(pid)], { encoding: "utf8" });
  if (result.status !== 0) return null;
  const pgid = Number.parseInt((result.stdout ?? "").trim(), 10);
  return Number.isSafeInteger(pgid) && pgid > 0 ? pgid : null;
};

const processGroupAlive = (pgid) => {
  const result = spawnSync("/bin/ps", ["-axo", "pgid="], { encoding: "utf8" });
  if (result.status !== 0) throw new Error("could not inspect the old process group");
  return (result.stdout ?? "").split(/\s+/u).some((value) => Number.parseInt(value, 10) === pgid);
};

const waitUntil = async (predicate, timeoutMs, intervalMs = 100) => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return true;
    await sleep(intervalMs);
  }
  return Boolean(await predicate());
};

const portAccepting = (host, port) =>
  new Promise((resolve) => {
    const socket = net.createConnection({ host, port });
    const done = (value) => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(250);
    socket.once("connect", () => done(true));
    socket.once("timeout", () => done(false));
    socket.once("error", () => done(false));
  });

const findDevAppPid = () => {
  const result = spawnSync("/usr/bin/pgrep", ["-x", "OpenAgents Dev"], { encoding: "utf8" });
  if (result.status !== 0) return null;
  const pid = Number.parseInt((result.stdout ?? "").trim().split(/\s+/u)[0] ?? "", 10);
  return Number.isSafeInteger(pid) && pid > 0 ? pid : null;
};

const requestGracefulQuit = (oldAppPid) => {
  try {
    process.kill(oldAppPid, "SIGTERM");
  } catch (error) {
    if (error?.code !== "ESRCH") throw error;
  }
};

const notifyFailure = (message) => {
  const escaped = bounded(message, 220).replaceAll("\\", "\\\\").replaceAll('"', '\\"');
  spawnSync(
    "/usr/bin/osascript",
    ["-e", `display notification \"${escaped}\" with title \"OpenAgents restart failed\"`],
    {
      stdio: "ignore",
    },
  );
};

const realDependencies = (config) => ({
  now: isoNow,
  sleep,
  coordinatorProcessGroupId: () => processGroupId(process.pid),
  oldProcessGroupAlive: () => processGroupAlive(config.oldProcessGroupId),
  requestQuit: () => requestGracefulQuit(config.oldAppPid),
  terminateOldProcessGroup: (signal) => {
    try {
      process.kill(-config.oldProcessGroupId, signal);
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  },
  rendererPortAccepting: () => portAccepting("127.0.0.1", config.rendererPort),
  syncLaunchWorktree: () => {
    if (!existsSync(path.join(config.launchRepo, ".git"))) {
      run("git", [
        "-C",
        config.sourceRepo,
        "worktree",
        "add",
        "--detach",
        config.launchRepo,
        config.targetSha,
      ]);
    }
    run("git", ["-C", config.launchRepo, "reset", "--hard", "--quiet", "HEAD"]);
    run("git", ["-C", config.launchRepo, "clean", "-fdq"]);
    run("git", [
      "-C",
      config.launchRepo,
      "switch",
      "--detach",
      "--force",
      "--quiet",
      config.targetSha,
    ]);
    const actual = run("git", ["-C", config.launchRepo, "rev-parse", "HEAD"]).trim();
    if (actual !== config.targetSha)
      throw new Error(
        `launch worktree resolved ${actual || "nothing"}, expected ${config.targetSha}`,
      );
  },
  ensureDependencies: () => {
    const lockRef = run("git", [
      "-C",
      config.launchRepo,
      "rev-parse",
      "HEAD:pnpm-lock.yaml",
    ]).trim();
    const nodeVersion = process.version;
    const pnpmVersion = run(config.pnpmPath, ["--version"], { cwd: config.launchRepo }).trim();
    const depsKey = `${lockRef}:${nodeVersion}:${pnpmVersion}`;
    const depsMarker = path.join(config.launchRepo, "node_modules", ".oa-deps-key");
    const electronPackage = path.join(
      config.launchRepo,
      "apps",
      "openagents-desktop",
      "node_modules",
      "electron",
    );
    const electronApp = path.join(electronPackage, "dist", "Electron.app");
    const electronBinary = path.join(electronApp, "Contents", "MacOS", "Electron");
    const framework = path.join(
      electronApp,
      "Contents",
      "Frameworks",
      "Electron Framework.framework",
      "Electron Framework",
    );
    const markerMatches =
      existsSync(depsMarker) && readFileSync(depsMarker, "utf8").trim() === depsKey;
    if (!existsSync(path.join(config.launchRepo, "node_modules")) || !markerMatches) {
      run(config.pnpmPath, ["install", "--frozen-lockfile", "--ignore-scripts"], {
        cwd: config.launchRepo,
        stdio: "inherit",
      });
      writeFileSync(depsMarker, `${depsKey}\n`, { encoding: "utf8", mode: 0o600 });
    }
    if (!existsSync(electronBinary) || !existsSync(framework)) {
      rmSync(path.join(electronPackage, "dist"), { recursive: true, force: true });
      run(config.nodePath, [path.join(electronPackage, "install.js")], {
        cwd: config.launchRepo,
        stdio: "inherit",
      });
    }
    if (!existsSync(electronBinary) || !existsSync(framework))
      throw new Error("Electron runtime failed integrity verification");
  },
  launchDev: () => {
    const logFd = openSync(config.logPath, "a", 0o600);
    const child = spawn(config.pnpmPath, ["--dir", "apps/openagents-desktop", "dev"], {
      cwd: config.launchRepo,
      env: { ...process.env, OPENAGENTS_DESKTOP_LAUNCH_CWD: config.ownerLaunchCwd },
      // Give the replacement its own group. A later launchd-owned coordinator
      // can drain that group without killing the coordinator that launched it.
      detached: true,
      stdio: ["ignore", logFd, logFd],
    });
    child.once("spawn", () => closeSync(logFd));
    child.once("error", () => closeSync(logFd));
    return {
      pid: child.pid ?? null,
      exited: new Promise((resolve, reject) => {
        child.once("error", reject);
        child.once("exit", (code, signal) => resolve({ code, signal }));
      }),
      alive: () => child.pid !== undefined && pidAlive(child.pid),
      terminate: (signal) => {
        if (child.pid === undefined) return;
        try {
          process.kill(-child.pid, signal);
        } catch (error) {
          if (error?.code !== "ESRCH") throw error;
        }
      },
    };
  },
  findNewAppPid: () => {
    const pid = findDevAppPid();
    return pid !== null && pid !== config.oldAppPid ? pid : null;
  },
  rendererReady: () => {
    if (!existsSync(config.logPath)) return false;
    const log = readFileSync(config.logPath, "utf8");
    return log.includes("[openagents-desktop] renderer dev server ready at");
  },
  notifyFailure,
  releaseLock: () => rmSync(config.lockDir, { recursive: true, force: true }),
});

export const superviseRestart = async (config, injectedDependencies) => {
  const deps = injectedDependencies ?? realDependencies(config);
  const base = {
    requestRef: config.requestRef,
    oldCommit: config.oldCommit,
    targetCommit: config.targetSha,
    oldAppPid: config.oldAppPid,
    oldProcessGroupId: config.oldProcessGroupId,
    coordinatorPid: process.pid,
    logPath: config.logPath,
    requestedAt: config.requestedAt,
  };
  let readyAt = null;
  let newDevPid = null;
  let newAppPid = null;
  let launchedChild = null;
  try {
    const coordinatorProcessGroupId = deps.coordinatorProcessGroupId();
    if (coordinatorProcessGroupId === config.oldProcessGroupId) {
      throw new Error(
        "restart coordinator is still inside the process group it was asked to terminate",
      );
    }
    writeRestartReceipt(config.receiptPath, {
      ...base,
      state: "handoff_owned",
      handoffOwnedAt: deps.now(),
    });
    await deps.sleep(config.graceMs);
    deps.requestQuit();
    let stopped = await waitUntil(() => !deps.oldProcessGroupAlive(), config.quitTimeoutMs);
    if (!stopped) {
      deps.terminateOldProcessGroup("SIGTERM");
      stopped = await waitUntil(() => !deps.oldProcessGroupAlive(), config.groupTerminateTimeoutMs);
    }
    if (!stopped) {
      deps.terminateOldProcessGroup("SIGKILL");
      stopped = await waitUntil(() => !deps.oldProcessGroupAlive(), config.forceQuitTimeoutMs);
    }
    if (!stopped)
      throw new Error(`old OpenAgents Dev process group ${config.oldProcessGroupId} did not exit`);
    const portReleased = await waitUntil(
      () => deps.rendererPortAccepting().then((value) => !value),
      config.portReleaseTimeoutMs,
    );
    if (!portReleased)
      throw new Error(`renderer port ${config.rendererPort} remained occupied after old app exit`);

    writeRestartReceipt(config.receiptPath, {
      ...base,
      state: "synchronizing",
      oldProcessExitedAt: deps.now(),
    });
    deps.syncLaunchWorktree();
    deps.ensureDependencies();
    const child = deps.launchDev();
    launchedChild = child;
    newDevPid = child.pid;
    const becameReady = await waitUntil(async () => {
      if (!child.alive()) return false;
      newAppPid = deps.findNewAppPid();
      return newAppPid !== null && deps.rendererReady() && (await deps.rendererPortAccepting());
    }, config.readinessTimeoutMs);
    if (!becameReady)
      throw new Error("replacement OpenAgents Dev did not become ready before the deadline");

    readyAt = deps.now();
    writeRestartReceipt(config.receiptPath, {
      ...base,
      state: "ready",
      ready: true,
      readyAt,
      newDevPid,
      newAppPid,
    });
    deps.releaseLock();
    const exit = await child.exited;
    writeRestartReceipt(config.receiptPath, {
      ...base,
      state: "stopped",
      ready: true,
      readyAt,
      stoppedAt: deps.now(),
      newDevPid,
      newAppPid,
      exitCode: exit.code,
      exitSignal: exit.signal,
      exitClassification: classifyDevExit(exit),
    });
    return { state: "stopped", readyAt, newDevPid, newAppPid, exit };
  } catch (error) {
    try {
      launchedChild?.terminate?.("SIGTERM");
    } catch {
      /* receipt remains authoritative */
    }
    const detail = bounded(error?.stack || error?.message || error);
    writeRestartReceipt(config.receiptPath, {
      ...base,
      state: "failed",
      ready: false,
      failedAt: deps.now(),
      newDevPid,
      newAppPid,
      error: detail,
      recoveryCommand: "oa-dev",
    });
    deps.releaseLock();
    deps.notifyFailure(`${detail}. Run oa-dev to recover.`);
    throw error;
  }
};

const parsePositiveInteger = (value, label) => {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed <= 0)
    throw new Error(`${label} must be a positive integer`);
  return parsed;
};

const main = async () => {
  const [mode, encoded] = process.argv.slice(2);
  if (mode !== "supervise" || !encoded)
    throw new Error("usage: oa-dev-supervisor.mjs supervise <base64url-config>");
  const config = JSON.parse(Buffer.from(encoded, "base64url").toString("utf8"));
  config.oldAppPid = parsePositiveInteger(String(config.oldAppPid), "oldAppPid");
  config.oldProcessGroupId = parsePositiveInteger(
    String(config.oldProcessGroupId),
    "oldProcessGroupId",
  );
  config.rendererPort = parsePositiveInteger(String(config.rendererPort ?? 5734), "rendererPort");
  for (const field of [
    "sourceRepo",
    "launchRepo",
    "targetSha",
    "receiptPath",
    "lockDir",
    "logPath",
    "requestRef",
    "ownerLaunchCwd",
    "nodePath",
    "pnpmPath",
  ]) {
    if (typeof config[field] !== "string" || config[field].length === 0)
      throw new Error(`${field} is required`);
  }
  if (!/^[0-9a-f]{40}$/u.test(config.targetSha))
    throw new Error("targetSha must be an exact 40-character Git object ID");
  process.env.PATH = `${path.dirname(config.nodePath)}:${path.dirname(config.pnpmPath)}:/usr/bin:/bin:/usr/sbin:/sbin`;
  await superviseRestart(config);
};

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`[oa-dev restart supervisor] ${bounded(error?.stack || error)}`);
    process.exitCode = 1;
  });
}
