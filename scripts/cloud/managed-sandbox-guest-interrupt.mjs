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

// Read every live process as (pid, processGroupId, sessionId). Zombies are
// excluded: they hold no resources and cannot be signalled.
const liveProcesses = () => {
  if (process.platform !== "linux" || !existsSync("/proc")) fail("linux_proc_required");
  const observed = [];
  for (const entry of readdirSync("/proc", { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^\d+$/u.test(entry.name)) continue;
    try {
      const stat = readFileSync(`/proc/${entry.name}/stat`, "utf8");
      const close = stat.lastIndexOf(")");
      if (close < 0) fail("process_stat_invalid");
      const fields = stat.slice(close + 2).split(" ");
      const state = fields[0];
      if (state === "Z") continue;
      const processGroupId = Number.parseInt(fields[2] ?? "", 10);
      const sessionId = Number.parseInt(fields[3] ?? "", 10);
      if (!Number.isSafeInteger(processGroupId) || !Number.isSafeInteger(sessionId)) {
        fail("process_stat_invalid");
      }
      observed.push({ pid: Number.parseInt(entry.name, 10), processGroupId, sessionId });
    } catch (error) {
      if (error?.code !== "ENOENT") fail("process_observation_failed");
    }
  }
  return observed;
};

const processGroupMembers = (processGroupId) =>
  liveProcesses()
    .filter((observed) => observed.processGroupId === processGroupId)
    .map((observed) => observed.pid);

// A descendant that calls setpgid() leaves the process group and therefore
// survives `kill(-pgid)` unseen. It stays in the session, so the session is
// what exposes it. Sessions observed before the signal are the ones that
// belong to this turn.
// Both selectors are pure over an observed process table so they can be
// falsified on any platform; production passes the live /proc reading.
export const sessionsIn = (processes, processGroupId) => [
  ...new Set(
    processes
      .filter((observed) => observed.processGroupId === processGroupId)
      .map((observed) => observed.sessionId),
  ),
];

export const escapedDescendantsIn = (processes, processGroupId, sessions, selfPid) =>
  processes.filter(
    (observed) =>
      observed.processGroupId !== processGroupId &&
      sessions.includes(observed.sessionId) &&
      observed.pid !== selfPid &&
      observed.sessionId !== 0,
  );

const sessionsOf = (processGroupId) => sessionsIn(liveProcesses(), processGroupId);

const escapedDescendants = (processGroupId, sessions) =>
  escapedDescendantsIn(liveProcesses(), processGroupId, sessions, process.pid);

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
  // Capture the sessions while the group is still intact, so a descendant that
  // leaves the group afterwards is still attributable to this turn.
  const sessions = sessionsOf(processGroupId);
  if (processGroupMembers(processGroupId).length > 0) {
    signal(processGroupId, "SIGTERM");
    if (!(await waitForExit(processGroupId))) {
      escalatedToSigkill = true;
      signal(processGroupId, "SIGKILL");
      if (!(await waitForExit(processGroupId))) fail("process_group_still_active");
    }
  }

  // Signal anything that escaped the group but remains in an observed session,
  // then measure. Every field below is the final observation, not an assertion
  // that the wait loop above must have succeeded.
  for (const escapee of escapedDescendants(processGroupId, sessions)) {
    try {
      process.kill(escapee.pid, "SIGKILL");
    } catch (error) {
      if (error?.code !== "ESRCH") fail("escaped_descendant_signal_failed");
    }
  }
  const deadline = Date.now() + WAIT_MILLIS;
  let remaining = escapedDescendants(processGroupId, sessions);
  while (remaining.length > 0 && Date.now() < deadline) {
    await sleep(POLL_MILLIS);
    remaining = escapedDescendants(processGroupId, sessions);
  }
  const groupRemaining = processGroupMembers(processGroupId).length;
  const descendantsRemaining = remaining.length;
  if (groupRemaining !== 0) fail("process_group_still_active");
  if (descendantsRemaining !== 0) fail("escaped_descendants_still_active");

  // The emitted shape is unchanged: consumers pin `zeroProcessGroup: true` and
  // `descendantsRemaining: 0`. What changed is that both are now the measured
  // result of the observations above rather than literals, and the refusals
  // above guarantee the pinned values can only be reached honestly.
  return {
    schemaVersion: "openagents.managed_sandbox_interrupt_proof.v1",
    processGroupId,
    zeroProcessGroup: groupRemaining === 0,
    descendantsRemaining,
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
