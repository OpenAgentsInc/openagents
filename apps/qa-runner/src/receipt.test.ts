// Receipt tests (issue #6188, "run = verified receipt"). A run result gains an
// ADDITIVE, namespaced, public-safe, dereferenceable `receipt` tied to the run's
// honest verification class. No existing field is touched; the augmented result
// still passes the public-safety tripwire; the receipt is idempotent.

import { mkdtempSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import {
  attachReceipt,
  buildQaRunReceipt,
  decodeQaRunResultWithReceipt,
  deriveLocalTraceRef,
  QA_RUN_RECEIPT_SCHEMA_VERSION,
  writeReceiptForRun,
} from "./receipt";
import type { QaRunResult } from "./result";

const passResult = (): QaRunResult => ({
  schemaVersion: "openagents.qa_runner.result.v1",
  status: "pass",
  target: { name: "openagents.com", baseUrl: "https://openagents.com" },
  brain: "scripted",
  backend: "local",
  startedAt: "2026-06-24T12:00:00.000Z",
  endedAt: "2026-06-24T12:00:05.000Z",
  durationMs: 5000,
  steps: [
    { index: 0, kind: "navigate", label: "open /login", status: "ok" },
    { index: 1, kind: "assert", label: "stays at /login", status: "ok" },
    { index: 2, kind: "assert", label: "body contains Log in", status: "ok" },
  ],
  artifacts: { screenshots: ["step-0.png"] },
});

const failResult = (): QaRunResult => ({
  ...passResult(),
  status: "fail",
  steps: [
    { index: 0, kind: "navigate", label: "open /login", status: "ok" },
    { index: 1, kind: "assert", label: "stays at /login", status: "failed" },
  ],
  failure: "stays at /login: assertion failed",
});

describe("buildQaRunReceipt", () => {
  test("a passing run with outcome assertions carries an honest exact_trace_replay receipt", () => {
    const receipt = buildQaRunReceipt(passResult());
    expect(receipt.schemaVersion).toBe(QA_RUN_RECEIPT_SCHEMA_VERSION);
    expect(receipt.verificationClass).toBe("exact_trace_replay");
    expect(receipt.assertionCount).toBe(2);
    expect(receipt.resultPath).toBe("result.json");
    // dereferenceable, public-safe ref (no secrets, derived from digest)
    expect(receipt.receiptRef).toMatch(/^receipt:qa_runner:openagents-com:[0-9a-f]{16}$/);
    expect(receipt.resultDigest).toMatch(/^[0-9a-f]{64}$/);
  });

  test("a failing run never carries a verified class (no exactness inflation)", () => {
    const receipt = buildQaRunReceipt(failResult());
    expect(receipt.verificationClass).toBe("none");
  });

  test("a passing run with no outcome assertions is only seeded", () => {
    const noAssert: QaRunResult = {
      ...passResult(),
      steps: [{ index: 0, kind: "navigate", label: "open /", status: "ok" }],
    };
    expect(buildQaRunReceipt(noAssert).verificationClass).toBe("seeded");
  });

  test("the receipt is deterministic for a fixed result", () => {
    expect(buildQaRunReceipt(passResult())).toEqual(buildQaRunReceipt(passResult()));
  });
});

describe("attachReceipt (additive, namespaced, public-safe)", () => {
  test("adds exactly the `receipt` key and touches no existing field", () => {
    const base = passResult();
    const augmented = attachReceipt(base);
    // every original field is unchanged
    for (const key of Object.keys(base) as Array<keyof QaRunResult>) {
      expect(augmented[key]).toEqual(base[key]);
    }
    // exactly one new top-level key
    expect(Object.keys(augmented).filter(k => !(k in base))).toEqual(["receipt"]);
    // it decodes under the with-receipt schema (tripwire-clean)
    expect(decodeQaRunResultWithReceipt(augmented)).toEqual(augmented);
  });

  test("the augmented result is merge-trivial with a peer additive `verify` field", () => {
    // simulate the concurrent lane's additive field; our receipt must coexist.
    const base = passResult();
    const withVerify = { ...base, verify: { ok: true } };
    const merged = { ...withVerify, receipt: buildQaRunReceipt(base) };
    expect(merged.verify).toEqual({ ok: true });
    expect(merged.receipt.schemaVersion).toBe(QA_RUN_RECEIPT_SCHEMA_VERSION);
  });
});

describe("writeReceiptForRun (the post-run path this lane owns)", () => {
  test("reads the runner-written result.json, attaches the receipt, and is idempotent", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-receipt-"));
    writeFileSync(join(dir, "result.json"), `${JSON.stringify(passResult(), null, 2)}\n`);

    const first = writeReceiptForRun(dir);
    const afterFirst = JSON.parse(readFileSync(join(dir, "result.json"), "utf8"));
    expect(afterFirst.receipt.receiptRef).toBe(first.receiptRef);

    // re-running computes the SAME receipt (digest is over the receipt-free result)
    const second = writeReceiptForRun(dir);
    expect(second).toEqual(first);
    const afterSecond = JSON.parse(readFileSync(join(dir, "result.json"), "utf8"));
    expect(afterSecond.receipt).toEqual(afterFirst.receipt);
  });
});

describe("traceRef — execution-trace evidence (issue #6216)", () => {
  test("buildQaRunReceipt omits traceRef by default (honest: no fabricated value)", () => {
    expect(buildQaRunReceipt(passResult()).traceRef).toBeUndefined();
    expect("traceRef" in buildQaRunReceipt(passResult())).toBe(false);
  });

  test("buildQaRunReceipt carries a supplied traceRef (the published uuid)", () => {
    const uuid = "3f2a1b6c-0000-4000-8000-000000000abc";
    const receipt = buildQaRunReceipt(passResult(), { traceRef: uuid });
    expect(receipt.traceRef).toBe(uuid);
  });

  test("a blank/whitespace traceRef is dropped (never fabricated/empty)", () => {
    expect(buildQaRunReceipt(passResult(), { traceRef: "" }).traceRef).toBeUndefined();
    expect(buildQaRunReceipt(passResult(), { traceRef: "   " }).traceRef).toBeUndefined();
  });

  test("traceRef does NOT change the digest or receiptRef (stable upgrade)", () => {
    const base = buildQaRunReceipt(passResult());
    const withTrace = buildQaRunReceipt(passResult(), { traceRef: "uuid-xyz" });
    expect(withTrace.resultDigest).toBe(base.resultDigest);
    expect(withTrace.receiptRef).toBe(base.receiptRef);
  });

  test("the supplied traceRef survives the public-safety tripwire and decodes", () => {
    const augmented = attachReceipt(passResult(), { traceRef: "uuid-xyz" });
    expect(decodeQaRunResultWithReceipt(augmented)).toEqual(augmented);
    expect(augmented.receipt.traceRef).toBe("uuid-xyz");
  });

  test("deriveLocalTraceRef returns the run's local ATIF trajectory_id", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-run-trace-"));
    writeFileSync(join(dir, "result.json"), `${JSON.stringify(passResult(), null, 2)}\n`);
    // The publisher assigns `${basename(runDir)}-trajectory` when no override.
    expect(deriveLocalTraceRef(dir)).toBe(`${dir.split("/").pop()}-trajectory`);
  });

  test("deriveLocalTraceRef is undefined (honest) when no result.json exists", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-no-result-"));
    expect(deriveLocalTraceRef(dir)).toBeUndefined();
  });

  test("writeReceiptForRun falls back to the honest local trajectory_id when unarmed", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-receipt-local-"));
    writeFileSync(join(dir, "result.json"), `${JSON.stringify(passResult(), null, 2)}\n`);
    const receipt = writeReceiptForRun(dir);
    expect(receipt.traceRef).toBe(`${dir.split("/").pop()}-trajectory`);
    const onDisk = JSON.parse(readFileSync(join(dir, "result.json"), "utf8"));
    expect(onDisk.receipt.traceRef).toBe(receipt.traceRef);
  });

  test("writeReceiptForRun upgrades traceRef to the published uuid when supplied", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-receipt-upgrade-"));
    writeFileSync(join(dir, "result.json"), `${JSON.stringify(passResult(), null, 2)}\n`);

    // first the honest local fallback
    const local = writeReceiptForRun(dir);
    expect(local.traceRef).toBe(`${dir.split("/").pop()}-trajectory`);

    // then the published-uuid upgrade — same digest/receiptRef, new traceRef
    const uuid = "3f2a1b6c-0000-4000-8000-000000000abc";
    const upgraded = writeReceiptForRun(dir, { traceRef: uuid });
    expect(upgraded.traceRef).toBe(uuid);
    expect(upgraded.resultDigest).toBe(local.resultDigest);
    expect(upgraded.receiptRef).toBe(local.receiptRef);
    const onDisk = JSON.parse(readFileSync(join(dir, "result.json"), "utf8"));
    expect(onDisk.receipt.traceRef).toBe(uuid);
  });
});
