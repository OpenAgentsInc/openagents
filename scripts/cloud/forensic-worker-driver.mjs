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
import { join } from "node:path";

const DRIVER_REF = "driver.openagents.forensic-worker.v1";
const BUBBLEWRAP = "/usr/bin/bwrap";
const WORKSPACE = "/workspace";
const TURN_ROOT = "/var/lib/openagents/managed-sandbox-turns";
const SOURCE_ROOT = `${WORKSPACE}/source`;
const ARTIFACT_PATH = `${WORKSPACE}/forensic-artifact.tar.zst`;
const NETWORK_ROOT = "/sys/class/net";

const refuse = (reason) => {
  process.stderr.write(`${JSON.stringify({ error: "forensic_worker_refused", reason })}\n`);
  process.exit(1);
};

const emit = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);

const livePid = (path) => {
  try {
    const pid = Number.parseInt(readFileSync(path, "utf8"), 10);
    if (!Number.isSafeInteger(pid) || pid < 1) return false;
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const turnDirectories = () =>
  existsSync(TURN_ROOT)
    ? readdirSync(TURN_ROOT, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && !entry.name.startsWith("io-"))
        .map((entry) => join(TURN_ROOT, entry.name))
    : [];

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
    if (livePid(join(directory, "pid"))) activeTurns += 1;
    try {
      const state = JSON.parse(readFileSync(join(directory, "state.json"), "utf8"));
      const usageEvents = Array.isArray(state.events)
        ? state.events.filter((event) => event?._tag === "RuntimeUsageRecorded")
        : [];
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

const prepareStop = () => {
  const directories = turnDirectories();
  if (directories.some((directory) => livePid(join(directory, "pid")))) {
    refuse("forensic_process_still_active");
  }
  for (const directory of directories) rmSync(directory, { recursive: true, force: true });
  rmSync(SOURCE_ROOT, { recursive: true, force: true });
  rmSync(ARTIFACT_PATH, { force: true });
  if (turnDirectories().length !== 0 || existsSync(SOURCE_ROOT) || existsSync(ARTIFACT_PATH)) {
    refuse("forensic_scratch_still_present");
  }
  emit({
    schema: "openagents.forensic_worker_prepare_stop.v1",
    driverRef: DRIVER_REF,
    zeroProcess: true,
    zeroScratch: true,
  });
};

if (process.argv.length !== 3) refuse("unsupported_operation");
if (process.argv[2] === "preflight") preflight();
else if (process.argv[2] === "usage") usage();
else if (process.argv[2] === "prepare-stop") prepareStop();
else refuse("unsupported_operation");
