#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const fail = () => process.exit(2);
if (process.argv[2] !== "--managed-sandbox-turn") fail();

let request;
try {
  request = JSON.parse(readFileSync(0, "utf8"));
} catch {
  fail();
}

const required = (name) => {
  const value = process.env[name]?.trim();
  if (!value) fail();
  return value;
};
const project = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const controlIp = required("OA_MANAGED_SANDBOX_CONTROL_INTERNAL_IP");
const brokerPort = required("OA_MANAGED_SANDBOX_PROVIDER_BROKER_PORT");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const digest = (value) => createHash("sha256").update(value).digest("hex");
const instance = `oa-msb-${digest(request.sandboxRef).slice(0, 20)}`;
const turnKey = digest(request.turnRef).slice(0, 24);
const remoteDir = `/var/lib/openagents/managed-sandbox-turns/${turnKey}`;
const statePath = `${remoteDir}/state.json`;
const requestPath = `${remoteDir}/request.json`;

const sshArgs = (command) => [
  "compute",
  "ssh",
  `openagents@${instance}`,
  "--project",
  project,
  "--zone",
  zone,
  "--internal-ip",
  "--quiet",
  "--ssh-key-expire-after=10m",
  "--ssh-flag=-oStrictHostKeyChecking=no",
  "--ssh-flag=-oUserKnownHostsFile=/dev/null",
  "--command",
  command,
];

const ssh = (command) =>
  execFileSync(gcloud, sshArgs(command), {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
    timeout: 60_000,
  });

const response = (events) => ({
  schemaVersion: "openagents.managed_sandbox_turn_runtime.v1",
  turnRef: request.turnRef,
  resourceGeneration: request.expectedResourceGeneration,
  events,
});

const readState = () => {
  try {
    return JSON.parse(ssh(`test -f ${statePath} && cat ${statePath}`));
  } catch {
    return undefined;
  }
};

const writeResponse = (events) => {
  process.stdout.write(JSON.stringify(response(events)));
  process.exit(0);
};

if (request.action === "sync") {
  const state = readState();
  const events = Array.isArray(state?.events)
    ? state.events.filter((event) => event.turnEventSequence > (request.afterTurnSequence ?? 0))
    : [];
  writeResponse(events);
}

if (request.action === "interrupt") {
  const state = readState();
  if (!state || !Array.isArray(state.events)) fail();
  const after = request.afterTurnSequence ?? 0;
  const observedAt = new Date().toISOString();
  const requested = {
    _tag: "RuntimeInterruptRequested",
    turnRef: request.turnRef,
    resourceGeneration: request.expectedResourceGeneration,
    turnEventSequence: after + 1,
    observedAt,
    reasonRef: request.reasonRef,
  };
  const interrupted = {
    _tag: "RuntimeInterrupted",
    turnRef: request.turnRef,
    resourceGeneration: request.expectedResourceGeneration,
    turnEventSequence: after + 2,
    observedAt: new Date().toISOString(),
    reasonRef: request.reasonRef,
  };
  const payload = Buffer.from(
    JSON.stringify({ ...state, events: [...state.events, requested, interrupted] }),
  ).toString("base64");
  ssh(
    `set -eu; test -f ${remoteDir}/pid && kill $(cat ${remoteDir}/pid) 2>/dev/null || true; ` +
      `printf %s ${payload} | base64 -d > ${statePath}.tmp; mv ${statePath}.tmp ${statePath}`,
  );
  writeResponse([requested, interrupted]);
}

if (request.action !== "dispatch" || typeof request.providerCapabilityToken !== "string") fail();

const observedAt = new Date().toISOString();
const started = {
  _tag: "RuntimeStarted",
  turnRef: request.turnRef,
  resourceGeneration: request.expectedResourceGeneration,
  turnEventSequence: 1,
  observedAt,
};
const initialState = {
  schemaVersion: "openagents.managed_sandbox_guest_turn_state.v1",
  turnRef: request.turnRef,
  resourceGeneration: request.expectedResourceGeneration,
  events: [started],
};
const guestRequest = {
  ...request,
  providerBaseUrl: `http://${controlIp}:${brokerPort}`,
};
const local = mkdtempSync(join(tmpdir(), "oa-msb-turn-"));
try {
  writeFileSync(join(local, "request.json"), JSON.stringify(guestRequest), { mode: 0o600 });
  writeFileSync(join(local, "state.json"), JSON.stringify(initialState), { mode: 0o600 });
  ssh(`install -d -m 0700 ${remoteDir}`);
  const copy = spawnSync(
    gcloud,
    [
      "compute",
      "scp",
      join(local, "request.json"),
      join(local, "state.json"),
      `openagents@${instance}:${remoteDir}/`,
      "--project",
      project,
      "--zone",
      zone,
      "--internal-ip",
      "--quiet",
      "--ssh-key-expire-after=10m",
      "--scp-flag=-oStrictHostKeyChecking=no",
      "--scp-flag=-oUserKnownHostsFile=/dev/null",
    ],
    { stdio: "ignore", timeout: 60_000 },
  );
  if (copy.status !== 0) fail();
  ssh(
    `set -eu; chmod 0600 ${requestPath} ${statePath}; ` +
      `nohup /usr/bin/node /opt/openagents-managed-sandbox/managed-sandbox-guest-turn.mjs ` +
      `${requestPath} ${statePath} >/dev/null 2>&1 & echo $! > ${remoteDir}/pid`,
  );
  writeResponse([started]);
} finally {
  rmSync(local, { recursive: true, force: true });
}
