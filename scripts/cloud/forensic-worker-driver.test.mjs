import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
  assert.doesNotMatch(source, /hostname|username|projectId|credential|token/u);
});
