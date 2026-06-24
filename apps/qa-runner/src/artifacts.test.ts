// Artifacts-reader tests (#6196): read-only projection of a run's artifacts,
// including the ADDITIVE verify verdict + receipt passthrough.

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { readRunArtifacts } from "./artifacts";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-artifacts-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const writeResult = (result: unknown) =>
  writeFileSync(join(dir, "result.json"), JSON.stringify(result, null, 2));

describe("readRunArtifacts", () => {
  test("returns nulls when nothing is on disk (honest; nothing fabricated)", () => {
    const a = readRunArtifacts(dir);
    expect(a.result).toBeNull();
    expect(a.video).toBeNull();
    expect(a.verify).toBeNull();
    expect(a.receipt).toBeNull();
    expect(a.committedTest).toBeNull();
    expect(a.screenshots).toEqual([]);
  });

  test("surfaces video/trace/screenshots off a real result.json", () => {
    writeResult({
      status: "pass",
      artifacts: {
        video: "video.webm",
        videoFormat: "webm",
        trace: "trace.zip",
        screenshots: ["0.png", "1.png"],
      },
    });
    const a = readRunArtifacts(dir);
    expect(a.video).toBe("video.webm");
    expect(a.videoFormat).toBe("webm");
    expect(a.trace).toBe("trace.zip");
    expect(a.screenshots).toEqual(["0.png", "1.png"]);
  });

  test("passes through the additive verify verdict (object form)", () => {
    writeResult({ status: "pass", artifacts: { screenshots: [] }, verify: { verdict: "CONFIRMED" } });
    expect(readRunArtifacts(dir).verify).toBe("CONFIRMED");
  });

  test("passes through the additive verify verdict (bare-string form)", () => {
    writeResult({ status: "fail", artifacts: { screenshots: [] }, verify: "REFUTED" });
    expect(readRunArtifacts(dir).verify).toBe("REFUTED");
  });

  test("ignores an unknown verify shape (honest null, not a guess)", () => {
    writeResult({ status: "pass", artifacts: { screenshots: [] }, verify: { foo: "bar" } });
    expect(readRunArtifacts(dir).verify).toBeNull();
  });

  test("passes through the additive receipt if present", () => {
    writeResult({
      status: "pass",
      artifacts: { screenshots: [] },
      receipt: { schemaVersion: "openagents.qa_runner.receipt.v1", verificationClass: "exact_trace_replay" },
    });
    const a = readRunArtifacts(dir);
    expect(a.receipt).not.toBeNull();
    expect(a.receipt!["verificationClass"]).toBe("exact_trace_replay");
  });

  test("reports a committed e2e test ref when one exists in the dir", () => {
    writeResult({ status: "pass", artifacts: { screenshots: [] } });
    writeFileSync(join(dir, "login-regression.e2e.test.ts"), "// test");
    expect(readRunArtifacts(dir).committedTest).toBe("login-regression.e2e.test.ts");
  });
});
