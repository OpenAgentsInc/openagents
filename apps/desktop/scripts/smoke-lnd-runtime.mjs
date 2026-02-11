#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");

const args = process.argv.slice(2);
const wantsJson = args.includes("--json");

const npmExecutable = process.platform === "win32" ? "npm.cmd" : "npm";
const startedAt = Date.now();
const result = spawnSync(npmExecutable, ["test", "--", "tests/lndRuntimeSmoke.test.ts"], {
  cwd: appRoot,
  encoding: "utf8",
  stdio: "pipe",
});
const durationMs = Date.now() - startedAt;

const summary = {
  ok: result.status === 0,
  status: result.status,
  durationMs,
  testFile: "tests/lndRuntimeSmoke.test.ts",
};

if (wantsJson) {
  process.stdout.write(`${JSON.stringify(summary)}\n`);
} else {
  if (summary.ok) {
    process.stdout.write(`LND runtime smoke passed in ${durationMs}ms\n`);
  } else {
    process.stdout.write(`LND runtime smoke failed in ${durationMs}ms\n`);
    if (result.stdout) process.stdout.write(`${result.stdout}\n`);
    if (result.stderr) process.stderr.write(`${result.stderr}\n`);
  }
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
