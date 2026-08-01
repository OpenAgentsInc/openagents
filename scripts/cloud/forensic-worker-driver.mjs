#!/usr/bin/env node

import {
  constants as fsConstants,
  accessSync,
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
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

export const prepareStopAt = ({ turnRoot, sourceRoot, artifactPath, runtimeRoot }) => {
  const scratchDirectories = directories(turnRoot, true);
  if (scratchDirectories.some(workloadIsLiveAt)) {
    refuse("forensic_process_still_active");
  }
  rmSync(turnRoot, { recursive: true, force: true });
  rmSync(sourceRoot, { recursive: true, force: true });
  rmSync(artifactPath, { recursive: true, force: true });
  rmSync(runtimeRoot, { recursive: true, force: true });
  const scratchPathsRemaining = [turnRoot, sourceRoot, artifactPath, runtimeRoot].filter(
    existsSync,
  );
  if (scratchPathsRemaining.length !== 0) {
    refuse("forensic_scratch_still_present");
  }
  return {
    schema: "openagents.forensic_worker_prepare_stop.v1",
    driverRef: DRIVER_REF,
    zeroProcess: true,
    zeroScratch: true,
    activeProcessGroups: 0,
    scratchPathsRemaining: 0,
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
