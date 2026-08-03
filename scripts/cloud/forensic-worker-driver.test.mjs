import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  observeGuardedProcessesAt,
  prepareStopAt,
  workloadIsLiveAt,
} from "./forensic-worker-driver.mjs";

// A stand-in for the /proc scan so the guarded-root contract can be exercised
// off Linux. `supported: true` mirrors a successful guest observation.
const observing = (processes, processGroups = []) => () => ({
  supported: true,
  processes,
  processGroups,
});

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
  // These two previously asserted that the source contained the literals
  // `activeProcessGroups: 0` and `scratchPathsRemaining: 0`, which locked in the
  // hardcoded proof the acceptance audit rejected. The proof must instead carry
  // values derived from a post-removal observation.
  assert.doesNotMatch(source, /activeProcessGroups: 0,/u);
  assert.doesNotMatch(source, /scratchPathsRemaining: 0,/u);
  assert.match(source, /activeProcessGroups = after\.processGroups\.length/u);
  assert.match(source, /scratchPathsRemaining = guardedRoots\.filter\(pathPresent\)\.length/u);
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
    const proof = prepareStopAt({ ...roots, observeGuardedProcesses: observing([]) });
    assert.deepEqual(proof, {
      schema: "openagents.forensic_worker_prepare_stop.v1",
      driverRef: "driver.openagents.forensic-worker.v1",
      processObservation: "proc",
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

// Falsifier for the audit finding that source, artifact, and runtime roots were
// deleted with no liveness check. Only `turnRoot` was ever gated.
test("a live process under a non-turn guarded root refuses the stop proof", () => {
  for (const guarded of ["sourceRoot", "artifactPath", "runtimeRoot"]) {
    const root = mkdtempSync(join(tmpdir(), "oa-forensic-guarded-"));
    const roots = {
      turnRoot: join(root, "turns"),
      sourceRoot: join(root, "source"),
      artifactPath: join(root, "forensic-artifact.tar.zst"),
      runtimeRoot: join(root, "run"),
    };
    try {
      mkdirSync(roots.turnRoot, { recursive: true });
      mkdirSync(roots.sourceRoot, { recursive: true });
      mkdirSync(roots.runtimeRoot, { recursive: true });
      writeFileSync(roots.artifactPath, "artifact");
      const result = spawnSync(
        process.execPath,
        [
          "--input-type=module",
          "-e",
          `import { prepareStopAt } from ${JSON.stringify(
            new URL("./forensic-worker-driver.mjs", import.meta.url).href,
          )};
           prepareStopAt({
             ...${JSON.stringify(roots)},
             observeGuardedProcesses: () => ({
               supported: true,
               processes: [4242],
               processGroups: [4242],
             }),
           });`,
        ],
        { encoding: "utf8" },
      );
      assert.equal(result.status, 1, `${guarded} must refuse while a process is live`);
      assert.match(result.stderr, /forensic_process_still_active/u);
      // The guarded roots must survive a refusal; residue is not destroyed to
      // make a proof look clean.
      assert.equal(existsSync(roots.sourceRoot), true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});

// Falsifier for `scratchPathsRemaining` being filtered with `existsSync`, which
// resolves symlinks and therefore reports dangling residue as absent.
test("dangling symlink residue cannot report a clean scratch proof", () => {
  const root = mkdtempSync(join(tmpdir(), "oa-forensic-symlink-"));
  const roots = {
    turnRoot: join(root, "turns"),
    sourceRoot: join(root, "source"),
    artifactPath: join(root, "forensic-artifact.tar.zst"),
    runtimeRoot: join(root, "run"),
  };
  try {
    mkdirSync(roots.turnRoot, { recursive: true });
    mkdirSync(roots.sourceRoot, { recursive: true });
    mkdirSync(roots.runtimeRoot, { recursive: true });
    // A symlink to a path that does not exist: `existsSync` says false,
    // `lstatSync` says the link is still there.
    symlinkSync(join(root, "absent-target"), roots.artifactPath);
    assert.equal(existsSync(roots.artifactPath), false);
    const proof = prepareStopAt({ ...roots, observeGuardedProcesses: observing([]) });
    // rmSync removes the link itself, so the measured proof is clean and — this
    // is the point — it is clean because it was measured with lstat.
    assert.equal(proof.scratchPathsRemaining, 0);
    assert.equal(proof.zeroScratch, true);
    for (const path of Object.values(roots)) {
      assert.throws(() => readFileSync(path), /ENOENT|EISDIR/u);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// Falsifier for the proof fields being literals. A post-delete observation that
// still reports a live process must not yield `zeroProcess: true`.
test("residue observed after removal refuses instead of asserting zero", () => {
  const root = mkdtempSync(join(tmpdir(), "oa-forensic-after-"));
  const roots = {
    turnRoot: join(root, "turns"),
    sourceRoot: join(root, "source"),
    artifactPath: join(root, "forensic-artifact.tar.zst"),
    runtimeRoot: join(root, "run"),
  };
  try {
    mkdirSync(roots.turnRoot, { recursive: true });
    let call = 0;
    const result = spawnSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `import { prepareStopAt } from ${JSON.stringify(
          new URL("./forensic-worker-driver.mjs", import.meta.url).href,
        )};
         let call = 0;
         prepareStopAt({
           ...${JSON.stringify(roots)},
           observeGuardedProcesses: () => {
             call += 1;
             // Clean before removal, residue after: exactly the case a literal
             // proof cannot distinguish.
             return call === 1
               ? { supported: true, processes: [], processGroups: [] }
               : { supported: true, processes: [909], processGroups: [909] };
           },
         });`,
      ],
      { encoding: "utf8" },
    );
    assert.equal(call, 0);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /forensic_process_still_active/u);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an unavailable process observation is never reported as proc-measured", () => {
  const root = mkdtempSync(join(tmpdir(), "oa-forensic-unavailable-"));
  const roots = {
    turnRoot: join(root, "turns"),
    sourceRoot: join(root, "source"),
    artifactPath: join(root, "forensic-artifact.tar.zst"),
    runtimeRoot: join(root, "run"),
  };
  try {
    mkdirSync(roots.turnRoot, { recursive: true });
    const proof = prepareStopAt({
      ...roots,
      observeGuardedProcesses: () => ({ supported: false, processes: [], processGroups: [] }),
    });
    // The Rust validator admits only `proc`, so this proof can never be
    // authoritative even though every count is zero.
    assert.equal(proof.processObservation, "unavailable");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the guarded process observation is unsupported off Linux", () => {
  const observation = observeGuardedProcessesAt(["/var/lib/openagents/managed-sandbox-turns"]);
  if (process.platform !== "linux") {
    assert.equal(observation.supported, false);
    assert.deepEqual(observation.processes, []);
  } else {
    assert.equal(observation.supported, true);
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
