#!/usr/bin/env node
/* global process, console */

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appDir = path.resolve(scriptDir, "..");

const command = [
  "vitest",
  "run",
  "tests/convex/lightning-paywalls.test.ts",
  "tests/worker/lightning-paywalls-endpoint.test.ts",
  "--reporter=dot",
];

const startedAt = Date.now();
const result = spawnSync(process.platform === "win32" ? "npx.cmd" : "npx", command, {
  cwd: appDir,
  encoding: "utf8",
  env: {
    ...process.env,
    NO_COLOR: "1",
  },
});
const endedAt = Date.now();

const tail = (value, lines = 20) =>
  String(value ?? "")
    .split(/\r?\n/)
    .filter((line) => line.length > 0)
    .slice(-lines);

const output = {
  ok: result.status === 0,
  command: [process.platform === "win32" ? "npx.cmd" : "npx", ...command],
  cwd: appDir,
  startedAtIso: new Date(startedAt).toISOString(),
  endedAtIso: new Date(endedAt).toISOString(),
  durationMs: endedAt - startedAt,
  exitCode: result.status ?? 1,
  signal: result.signal ?? null,
  stdoutTail: tail(result.stdout),
  stderrTail: tail(result.stderr),
};

console.log(JSON.stringify(output));

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
