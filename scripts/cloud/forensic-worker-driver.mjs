#!/usr/bin/env node

import {
  constants as fsConstants,
  accessSync,
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  rmSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const DRIVER_REF = "driver.openagents.forensic-worker.v1";
const BUBBLEWRAP = "/usr/bin/bwrap";
const WORKSPACE = "/workspace";
const TURN_ROOT = "/var/lib/openagents/managed-sandbox-turns";
const SOURCE_ROOT = `${WORKSPACE}/source`;
const ARTIFACT_PATH = `${WORKSPACE}/forensic-artifact.tar.zst`;
const RUNTIME_ROOT = "/run/openagents-managed-sandbox";
const NETWORK_ROOT = "/sys/class/net";

const refuse = (reason) => {
  process.stderr.write(`${JSON.stringify({ error: "forensic_worker_refused", reason })}\n`);
  process.exit(1);
};

const emit = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);

const liveIdentifier = (path, processGroup) => {
  try {
    const identifier = Number.parseInt(readFileSync(path, "utf8"), 10);
    if (!Number.isSafeInteger(identifier) || identifier < 1) return true;
    process.kill(processGroup ? -identifier : identifier, 0);
    return true;
  } catch (error) {
    return error?.code !== "ESRCH";
  }
};

const directories = (turnRoot, includeIo) =>
  existsSync(turnRoot)
    ? readdirSync(turnRoot, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && (includeIo || !entry.name.startsWith("io-")))
        .map((entry) => join(turnRoot, entry.name))
    : [];

const turnDirectories = () => directories(TURN_ROOT, false);

export const workloadIsLiveAt = (directory) =>
  existsSync(join(directory, "pgid"))
    ? liveIdentifier(join(directory, "pgid"), true)
    : existsSync(join(directory, "pid"))
      ? liveIdentifier(join(directory, "pid"), false)
      : !basename(directory).startsWith("io-");

// A residue check must not follow symlinks: `existsSync` resolves them, so a
// dangling symlink left behind after removal would be reported as clean.
const pathPresent = (path) => {
  try {
    lstatSync(path);
    return true;
  } catch {
    return false;
  }
};

const pathIsUnder = (path, root) => path === root || path.startsWith(`${root}/`);

// Observe every live process that still references a guarded root through its
// working directory, filesystem root, executable, or any open descriptor. This
// is the only observation that survives a descendant escaping its process group
// with setsid()/setpgid(), so it is what makes "zero process" a measurement
// rather than an assertion.
export const observeGuardedProcessesAt = (roots) => {
  if (process.platform !== "linux" || !existsSync("/proc")) {
    return { supported: false, processes: [], processGroups: [] };
  }
  const processes = [];
  const processGroups = new Set();
  const self = process.pid;
  for (const entry of readdirSync("/proc", { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^\d+$/u.test(entry.name)) continue;
    const pid = Number.parseInt(entry.name, 10);
    if (pid === self) continue;
    const references = [`/proc/${pid}/cwd`, `/proc/${pid}/root`, `/proc/${pid}/exe`];
    try {
      for (const descriptor of readdirSync(`/proc/${pid}/fd`)) {
        references.push(`/proc/${pid}/fd/${descriptor}`);
      }
    } catch (error) {
      // A process that exits mid-scan is not residue. Any other failure means
      // the observation is incomplete and must not be reported as clean.
      if (error?.code !== "ENOENT" && error?.code !== "ESRCH") {
        return { supported: false, processes: [], processGroups: [] };
      }
    }
    let referenced = false;
    for (const reference of references) {
      let target;
      try {
        target = readlinkSync(reference);
      } catch {
        continue;
      }
      if (roots.some((root) => pathIsUnder(target, root))) {
        referenced = true;
        break;
      }
    }
    if (!referenced) continue;
    processes.push(pid);
    try {
      const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
      const close = stat.lastIndexOf(")");
      const fields = close < 0 ? [] : stat.slice(close + 2).split(" ");
      const processGroupId = Number.parseInt(fields[2] ?? "", 10);
      if (Number.isSafeInteger(processGroupId)) processGroups.add(processGroupId);
    } catch (error) {
      if (error?.code !== "ENOENT" && error?.code !== "ESRCH") {
        return { supported: false, processes: [], processGroups: [] };
      }
    }
  }
  return {
    supported: true,
    processes,
    processGroups: [...processGroups].toSorted((left, right) => left - right),
  };
};

const fileSize = (path) => {
  if (!existsSync(path)) return { bytes: 0, exact: true };
  try {
    const status = lstatSync(path);
    return status.isFile()
      ? { bytes: status.size, exact: Number.isSafeInteger(status.size) }
      : { bytes: 0, exact: false };
  } catch {
    return { bytes: 0, exact: false };
  }
};

const treeSize = (root) => {
  if (!existsSync(root)) return { bytes: 0, exact: true };
  let bytes = 0;
  let exact = true;
  const pending = [root];
  while (pending.length > 0) {
    const path = pending.pop();
    try {
      const status = lstatSync(path);
      if (status.isSymbolicLink()) exact = false;
      else if (status.isFile()) bytes += status.size;
      else if (status.isDirectory()) {
        for (const entry of readdirSync(path)) pending.push(join(path, entry));
      } else exact = false;
      if (!Number.isSafeInteger(bytes)) exact = false;
    } catch {
      exact = false;
    }
  }
  return { bytes, exact };
};

const networkUsage = () => {
  if (process.platform !== "linux" || !existsSync(NETWORK_ROOT)) {
    return { bytes: 0, exact: false };
  }
  let bytes = 0;
  let observed = false;
  try {
    for (const interfaceName of readdirSync(NETWORK_ROOT)) {
      if (interfaceName === "lo") continue;
      observed = true;
      for (const counter of ["rx_bytes", "tx_bytes"]) {
        const value = Number.parseInt(
          readFileSync(join(NETWORK_ROOT, interfaceName, "statistics", counter), "utf8"),
          10,
        );
        if (!Number.isSafeInteger(value) || value < 0) return { bytes: 0, exact: false };
        bytes += value;
        if (!Number.isSafeInteger(bytes)) return { bytes: 0, exact: false };
      }
    }
  } catch {
    return { bytes: 0, exact: false };
  }
  return { bytes, exact: observed };
};

const preflight = () => {
  if (process.platform !== "linux") refuse("linux_required");
  try {
    accessSync(BUBBLEWRAP, fsConstants.X_OK);
    accessSync(WORKSPACE, fsConstants.R_OK | fsConstants.X_OK);
    const probe = spawnSync(
      BUBBLEWRAP,
      [
        "--die-with-parent",
        "--unshare-net",
        "--unshare-pid",
        "--unshare-uts",
        "--unshare-ipc",
        "--ro-bind",
        "/",
        "/",
        "--bind",
        WORKSPACE,
        WORKSPACE,
        "--tmpfs",
        "/run",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--chdir",
        WORKSPACE,
        "/bin/true",
      ],
      {
        cwd: WORKSPACE,
        env: { PATH: "/usr/bin:/bin" },
        encoding: "utf8",
        timeout: 5_000,
        windowsHide: true,
      },
    );
    if (probe.error !== undefined || probe.status !== 0 || probe.signal !== null) {
      refuse("bubblewrap_probe_failed");
    }
  } catch {
    refuse("runtime_dependency_missing");
  }
  emit({
    schema: "openagents.forensic_worker_preflight.v1",
    driverRef: DRIVER_REF,
    linux: true,
    bubblewrap: true,
    networkNamespace: "unshared",
    workspaceRoot: "workspace",
  });
};

const usage = () => {
  let exact = true;
  let tokens = 0;
  let activeTurns = 0;
  for (const directory of turnDirectories()) {
    if (workloadIsLiveAt(directory)) activeTurns += 1;
    try {
      const state = JSON.parse(readFileSync(join(directory, "state.json"), "utf8"));
      const usageEvents = Array.isArray(state.events)
        ? state.events.filter((event) => event?._tag === "RuntimeUsageRecorded")
        : [];
      if (usageEvents.length !== 1) exact = false;
      for (const event of usageEvents) {
        if (event?.usage?.exact !== true) exact = false;
        const inputTokens = event?.usage?.inputTokens;
        const outputTokens = event?.usage?.outputTokens;
        if (!Number.isSafeInteger(inputTokens) || !Number.isSafeInteger(outputTokens))
          exact = false;
        else tokens += inputTokens + outputTokens;
      }
    } catch {
      exact = false;
    }
  }
  const source = treeSize(SOURCE_ROOT);
  const artifact = fileSize(ARTIFACT_PATH);
  const network = networkUsage();
  emit({
    exact: exact && source.exact && artifact.exact && network.exact,
    tokens,
    sourceBytes: source.bytes,
    artifactBytes: artifact.bytes,
    networkBytes: network.bytes,
    activeTurns,
  });
};

export const prepareStopAt = ({
  turnRoot,
  sourceRoot,
  artifactPath,
  runtimeRoot,
  observeGuardedProcesses = observeGuardedProcessesAt,
}) => {
  const guardedRoots = [turnRoot, sourceRoot, artifactPath, runtimeRoot];

  // The turn-directory scan only covers recorded pid/pgid files beneath
  // `turnRoot`. The guarded-root observation additionally covers the source,
  // artifact, and runtime roots, which were previously removed with no
  // liveness check at all.
  if (directories(turnRoot, true).some(workloadIsLiveAt)) {
    refuse("forensic_process_still_active");
  }
  const before = observeGuardedProcesses(guardedRoots);
  if (process.platform === "linux" && !before.supported) {
    refuse("forensic_process_observation_unavailable");
  }
  if (before.processes.length !== 0) {
    refuse("forensic_process_still_active");
  }

  rmSync(turnRoot, { recursive: true, force: true });
  rmSync(sourceRoot, { recursive: true, force: true });
  rmSync(artifactPath, { recursive: true, force: true });
  rmSync(runtimeRoot, { recursive: true, force: true });

  // Re-observe after removal. These counts are the proof; they are never
  // assumed from the fact that removal returned without throwing.
  const after = observeGuardedProcesses(guardedRoots);
  if (process.platform === "linux" && !after.supported) {
    refuse("forensic_process_observation_unavailable");
  }
  const scratchPathsRemaining = guardedRoots.filter(pathPresent).length;
  const activeProcessGroups = after.processGroups.length;
  if (after.processes.length !== 0) {
    refuse("forensic_process_still_active");
  }
  if (scratchPathsRemaining !== 0) {
    refuse("forensic_scratch_still_present");
  }
  return {
    schema: "openagents.forensic_worker_prepare_stop.v1",
    driverRef: DRIVER_REF,
    processObservation: after.supported ? "proc" : "unavailable",
    zeroProcess: after.processes.length === 0 && activeProcessGroups === 0,
    zeroScratch: scratchPathsRemaining === 0,
    activeProcessGroups,
    scratchPathsRemaining,
  };
};

const prepareStop = () =>
  emit(
    prepareStopAt({
      turnRoot: TURN_ROOT,
      sourceRoot: SOURCE_ROOT,
      artifactPath: ARTIFACT_PATH,
      runtimeRoot: RUNTIME_ROOT,
    }),
  );

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv.length !== 3) refuse("unsupported_operation");
  if (process.argv[2] === "preflight") preflight();
  else if (process.argv[2] === "usage") usage();
  else if (process.argv[2] === "prepare-stop") prepareStop();
  else refuse("unsupported_operation");
}
