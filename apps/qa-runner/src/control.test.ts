// Control-plane tests (#6196): the in-process engine the HTTP daemon drives.
// Deterministic MOCK path only — scriptedBrain + fake chromium, NO network/spend.

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { NotArmedError, BadRequestError, NotFoundError, QaControl } from "./control";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-control-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const mkControl = (overrides = {}) =>
  new QaControl({ storeDir: dir, proBaseUrl: "https://openagents.com", ...overrides });

describe("submitRun (mock path)", () => {
  test("submit -> run -> artifacts, purely in-process, no network", async () => {
    const control = mkControl();
    const job = control.submitRun({ scenario: "login-regression" });
    expect(job.kind).toBe("run");
    expect(job.mode).toBe("mock");
    expect(["queued", "running"]).toContain(job.status);
    // honest receipt: the mock path is never spend-capable
    expect(job.receipt.spendCapable).toBe(false);

    const done = await control.wait(job.id);
    expect(done.status).toBe("succeeded");

    const art = control.runArtifacts(job.id);
    expect(art.status).toBe("succeeded");
    expect(art.proUrl).toBe(`https://openagents.com/pro/runs/${job.id}`);
    expect(art.video).toBeTruthy();
    expect(art.result).not.toBeNull();
    expect(art.result!["status"]).toBe("pass");
    // the additive receipt the post-run helper wrote is present + dereferenceable
    expect(art.receipt).not.toBeNull();
    expect(art.receipt!["resultPath"]).toBe("result.json");
    // artifacts really exist on disk
    expect(existsSync(join(dir, job.id, "result.json"))).toBe(true);
  });

  test("the intentionally-wrong scenario FAILS honestly (no fake green)", async () => {
    const control = mkControl();
    const job = control.submitRun({ scenario: "login-regression-wrong" });
    const done = await control.wait(job.id);
    expect(done.status).toBe("succeeded"); // the JOB completed
    const art = control.runArtifacts(job.id);
    expect(art.result!["status"]).toBe("fail"); // but the RUN honestly failed
  });

  test("a real run is REFUSED when the daemon is not armed", () => {
    const control = mkControl({ allowReal: false });
    expect(() => control.submitRun({ real: true, target: "https://openagents.com" })).toThrow(
      NotArmedError,
    );
  });

  test("rejects an unknown scenario", () => {
    const control = mkControl();
    // @ts-expect-error intentionally bad scenario
    expect(() => control.submitRun({ scenario: "nope" })).toThrow(BadRequestError);
  });

  test("status of an unknown id throws NotFound", () => {
    const control = mkControl();
    expect(() => control.status("run_missing")).toThrow(NotFoundError);
  });
});

describe("submitEval (mock path)", () => {
  test("compares >= 2 variants and yields a dereferenceable comparison", async () => {
    const control = mkControl();
    const job = control.submitEval({
      title: "baseline vs candidate",
      variants: [
        { id: "baseline", scenario: "login-regression", note: "current /login" },
        { id: "candidate", scenario: "login-regression-wrong", note: "regressed" },
      ],
    });
    expect(job.kind).toBe("eval");
    const done = await control.wait(job.id);
    expect(done.status).toBe("succeeded");

    const res = control.evalComparison(job.id);
    expect(res.proUrl).toBe(`https://openagents.com/pro/evals/${job.id}`);
    expect(res.comparison).not.toBeNull();
    expect(res.comparison!.variants.length).toBe(2);
    expect(res.comparison!.baselineVariantId).toBe("baseline");
    // honest: the fixture path is NOT decision-grade
    expect(res.comparison!.decisionGrade).toBe(false);
    // baseline passes, regressed candidate fails -> a real delta
    const baseline = res.comparison!.variants.find((v) => v.variantId === "baseline")!;
    const candidate = res.comparison!.variants.find((v) => v.variantId === "candidate")!;
    expect(baseline.passRate).toBe(1);
    expect(candidate.passRate).toBe(0);
  });

  test("rejects < 2 variants", () => {
    const control = mkControl();
    expect(() => control.submitEval({ variants: [{ id: "only" }] })).toThrow(BadRequestError);
  });
});
