import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { interruptProcessGroup } from "./managed-sandbox-guest-interrupt.mjs";

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
