import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import test from "node:test";

const source = readFileSync(new URL("./forensic-worker-driver.mjs", import.meta.url), "utf8");

test("forensic worker preflight is Linux and bubblewrap fail-closed", () => {
  assert.match(source, /process\.platform !== "linux"/u);
  assert.match(source, /const BUBBLEWRAP = "\/usr\/bin\/bwrap"/u);
  assert.match(source, /"--unshare-net"/u);
  assert.match(source, /"--die-with-parent"/u);
  assert.match(source, /timeout: 5_000/u);
  assert.doesNotMatch(source, /shell:/u);
});

test("forensic worker preflight projects no host or credential values", () => {
  assert.match(source, /driver\.openagents\.forensic-worker\.v1/u);
  assert.match(source, /workspaceRoot: "workspace"/u);
  assert.doesNotMatch(source, /process\.env/u);
  assert.doesNotMatch(source, /hostname|username|projectId|credential|accessToken|process\.env/u);
});

test("forensic worker usage and stop proofs are fixed guest operations", () => {
  const driver = new URL("./forensic-worker-driver.mjs", import.meta.url);
  const usage = spawnSync(process.execPath, [driver.pathname, "usage"], { encoding: "utf8" });
  assert.equal(usage.status, 0);
  const observed = JSON.parse(usage.stdout);
  assert.deepEqual(observed, {
    exact: observed.exact,
    tokens: 0,
    sourceBytes: 0,
    artifactBytes: 0,
    networkBytes: observed.networkBytes,
    activeTurns: 0,
  });
  assert.equal(Number.isSafeInteger(observed.networkBytes), true);
  assert.ok(observed.networkBytes >= 0);
  assert.equal(typeof observed.exact, "boolean");
  if (process.platform !== "linux") assert.equal(observed.exact, false);
  assert.match(source, /\/sys\/class\/net/u);
  assert.doesNotMatch(source, /networkBytes: sourceBytes \+ artifactBytes/u);
  const stopped = spawnSync(process.execPath, [driver.pathname, "prepare-stop"], {
    encoding: "utf8",
  });
  assert.equal(stopped.status, 0);
  assert.match(stopped.stdout, /"zeroProcess":true/iu);
  assert.match(stopped.stdout, /"zeroScratch":true/iu);
});
