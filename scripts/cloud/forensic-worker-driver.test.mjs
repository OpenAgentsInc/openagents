import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  prepareStopAt,
  workloadIsLiveAt,
} from "./forensic-worker-driver.mjs";

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
  assert.match(source, /RUNTIME_ROOT = "\/run\/openagents-managed-sandbox"/u);
  assert.match(source, /activeProcessGroups: 0/u);
  assert.match(source, /scratchPathsRemaining: 0/u);
});

test("forensic stop removes turn, io, source, artifact, and runtime scratch roots", () => {
  const root = mkdtempSync(join(tmpdir(), "oa-forensic-stop-"));
  const roots = {
    turnRoot: join(root, "turns"),
    sourceRoot: join(root, "source"),
    artifactPath: join(root, "forensic-artifact.tar.zst"),
    runtimeRoot: join(root, "run"),
  };
  try {
    mkdirSync(join(roots.turnRoot, "turn-fixture"), { recursive: true });
    mkdirSync(join(roots.turnRoot, "io-fixture"), { recursive: true });
    writeFileSync(join(roots.turnRoot, "turn-fixture", "pid"), "2147483647");
    mkdirSync(roots.sourceRoot, { recursive: true });
    mkdirSync(roots.runtimeRoot, { recursive: true });
    writeFileSync(roots.artifactPath, "artifact");
    const proof = prepareStopAt(roots);
    assert.deepEqual(proof, {
      schema: "openagents.forensic_worker_prepare_stop.v1",
      driverRef: "driver.openagents.forensic-worker.v1",
      zeroProcess: true,
      zeroScratch: true,
      activeProcessGroups: 0,
      scratchPathsRemaining: 0,
    });
    for (const path of Object.values(roots)) assert.equal(existsSync(path), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("missing and malformed workload identity cannot prove zero process", () => {
  const root = mkdtempSync(join(tmpdir(), "oa-forensic-identity-"));
  try {
    const missing = join(root, "turn-missing");
    const malformed = join(root, "turn-malformed");
    const ioScratch = join(root, "io-scratch");
    mkdirSync(missing);
    mkdirSync(malformed);
    mkdirSync(ioScratch);
    writeFileSync(join(malformed, "pgid"), "not-a-process-group");
    assert.equal(workloadIsLiveAt(missing), true);
    assert.equal(workloadIsLiveAt(malformed), true);
    assert.equal(workloadIsLiveAt(ioScratch), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
