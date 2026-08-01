#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { pathToFileURL } from "node:url";

const TURN_ROOT = "/var/lib/openagents/managed-sandbox-turns";
const WAIT_MILLIS = 5_000;
const POLL_MILLIS = 50;

const fail = (reason) => {
  process.stderr.write(
    `${JSON.stringify({ error: "managed_sandbox_interrupt_failed", reason })}\n`,
  );
  process.exit(2);
};

const sleep = (millis) => new Promise((resolve) => setTimeout(resolve, millis));

const processGroupMembers = (processGroupId) => {
  if (process.platform !== "linux" || !existsSync("/proc")) fail("linux_proc_required");
  const members = [];
  for (const entry of readdirSync("/proc", { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^\d+$/u.test(entry.name)) continue;
    try {
      const stat = readFileSync(`/proc/${entry.name}/stat`, "utf8");
      const close = stat.lastIndexOf(")");
      if (close < 0) fail("process_stat_invalid");
      const fields = stat.slice(close + 2).split(" ");
      const state = fields[0];
      const observedProcessGroupId = Number.parseInt(fields[2] ?? "", 10);
      if (observedProcessGroupId === processGroupId && state !== "Z") {
        members.push(Number.parseInt(entry.name, 10));
      }
    } catch (error) {
      if (error?.code !== "ENOENT") fail("process_observation_failed");
    }
  }
  return members;
};

const waitForExit = async (processGroupId) => {
  const deadline = Date.now() + WAIT_MILLIS;
  while (Date.now() < deadline) {
    if (processGroupMembers(processGroupId).length === 0) return true;
    await sleep(POLL_MILLIS);
  }
  return processGroupMembers(processGroupId).length === 0;
};

const signal = (processGroupId, signalName) => {
  try {
    process.kill(-processGroupId, signalName);
  } catch (error) {
    if (error?.code !== "ESRCH") fail("process_group_signal_failed");
  }
};

export const interruptProcessGroup = async (processGroupId) => {
  if (!Number.isSafeInteger(processGroupId) || processGroupId < 2) {
    fail("process_group_identity_invalid");
  }
  let escalatedToSigkill = false;
  if (processGroupMembers(processGroupId).length > 0) {
    signal(processGroupId, "SIGTERM");
    if (!(await waitForExit(processGroupId))) {
      escalatedToSigkill = true;
      signal(processGroupId, "SIGKILL");
      if (!(await waitForExit(processGroupId))) fail("process_group_still_active");
    }
  }
  return {
    schemaVersion: "openagents.managed_sandbox_interrupt_proof.v1",
    processGroupId,
    zeroProcessGroup: true,
    descendantsRemaining: 0,
    escalatedToSigkill,
  };
};

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [turnDirectory] = process.argv.slice(2);
  if (
    typeof turnDirectory !== "string" ||
    !new RegExp(`^${TURN_ROOT}/[a-f0-9]{24}$`, "u").test(turnDirectory)
  ) {
    fail("turn_directory_invalid");
  }
  let processGroupId;
  try {
    processGroupId = Number.parseInt(readFileSync(`${turnDirectory}/pgid`, "utf8"), 10);
  } catch {
    fail("process_group_identity_missing");
  }
  process.stdout.write(`${JSON.stringify(await interruptProcessGroup(processGroupId))}\n`);
}
