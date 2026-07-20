#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const fail = () => process.exit(2);
if (process.argv[2] !== "--managed-sandbox-guest-io") fail();

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
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const digest = (value) => createHash("sha256").update(value).digest("hex");
const instance = `oa-msb-${digest(request.sandboxRef).slice(0, 20)}`;
const operationKey = digest(request.operationRef).slice(0, 24);
const remoteDir = `/var/lib/openagents/managed-sandbox-turns/io-${operationKey}`;
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
    maxBuffer: 24 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
    timeout: (request.timeoutMillis ?? 30_000) + 90_000,
  });

const local = mkdtempSync(join(tmpdir(), "oa-msb-io-"));
try {
  const localRequest = join(local, "request.json");
  writeFileSync(localRequest, JSON.stringify(request), { mode: 0o600 });
  ssh(`install -d -m 0700 ${remoteDir}`);
  const copy = spawnSync(
    gcloud,
    [
      "compute",
      "scp",
      localRequest,
      `openagents@${instance}:${requestPath}`,
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
  const response = ssh(
    `set -eu; trap 'rmdir ${remoteDir} 2>/dev/null || true' EXIT; ` +
      `chmod 0600 ${requestPath}; ` +
      `/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py ${requestPath}`,
  );
  process.stdout.write(response);
} catch {
  fail();
} finally {
  rmSync(local, { recursive: true, force: true });
}
