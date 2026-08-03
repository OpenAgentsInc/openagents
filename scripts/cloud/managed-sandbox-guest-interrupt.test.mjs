import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  escapedDescendantsIn,
  interruptProcessGroup,
  sessionsIn,
} from "./managed-sandbox-guest-interrupt.mjs";

// The end-to-end interrupt test below can only run on Linux. These selector
// tests carry the escape-detection contract on every platform, so the logic
// that decides whether an interrupt was honest is not left unverified.
test("a descendant that leaves the process group is still attributed to the turn", () => {
  const processes = [
    { pid: 100, processGroupId: 100, sessionId: 100 },
    { pid: 101, processGroupId: 100, sessionId: 100 },
    // setpgid() escapee: new process group, same session.
    { pid: 102, processGroupId: 102, sessionId: 100 },
    // Unrelated process in another session.
    { pid: 200, processGroupId: 200, sessionId: 200 },
  ];
  const sessions = sessionsIn(processes, 100);
  assert.deepEqual(sessions, [100]);

  const escaped = escapedDescendantsIn(processes, 100, sessions, 999);
  assert.deepEqual(
    escaped.map((observed) => observed.pid),
    [102],
    "the escapee must be visible even though kill(-100) cannot reach it",
  );
});

test("escape detection excludes the observer and unattributable sessions", () => {
  const processes = [
    { pid: 100, processGroupId: 100, sessionId: 100 },
    // The interrupt process itself must never count as residue.
    { pid: 555, processGroupId: 555, sessionId: 100 },
    // Session 0 is not attributable to a turn.
    { pid: 300, processGroupId: 300, sessionId: 0 },
  ];
  const escaped = escapedDescendantsIn(processes, 100, sessionsIn(processes, 100), 555);
  assert.deepEqual(escaped, []);
});

test("an intact process group reports no escapees", () => {
  const processes = [
    { pid: 100, processGroupId: 100, sessionId: 100 },
    { pid: 101, processGroupId: 100, sessionId: 100 },
  ];
  const escaped = escapedDescendantsIn(processes, 100, sessionsIn(processes, 100), 999);
  assert.deepEqual(escaped, []);
});

const waitForFile = async (path) => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      return Number.parseInt(readFileSync(path, "utf8"), 10);
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }
  throw new Error("child pid was not recorded");
};

test(
  "interrupt kills an ignored-SIGTERM workload and its descendant process group",
  { skip: process.platform !== "linux" },
  async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-interrupt-"));
    const childPath = join(root, "child-pid");
    const workload = spawn(
      process.execPath,
      [
        "-e",
        `const {spawn}=require("node:child_process");` +
          `const {writeFileSync}=require("node:fs");` +
          `process.on("SIGTERM",()=>{});` +
          `const child=spawn(process.execPath,["-e","process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"],{stdio:"ignore"});` +
          `writeFileSync(${JSON.stringify(childPath)},String(child.pid));setInterval(()=>{},1000);`,
      ],
      { detached: true, stdio: "ignore" },
    );
    try {
      const childPid = await waitForFile(childPath);
      const proof = await interruptProcessGroup(workload.pid);
      assert.equal(proof.zeroProcessGroup, true);
      assert.equal(proof.descendantsRemaining, 0);
      assert.equal(proof.escalatedToSigkill, true);
      assert.throws(() => process.kill(childPid, 0), /ESRCH/u);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);
