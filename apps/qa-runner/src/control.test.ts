// Control-plane tests (#6196): the in-process engine the HTTP daemon drives.
// Deterministic MOCK path only — scriptedBrain + fake chromium, NO network/spend.

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { NotArmedError, BadRequestError, NotFoundError, QaControl } from "./control";
import type { FetchLike } from "./publish-trace";
import { QA_SWARM_RUN_PROJECTION_SCHEMA } from "./swarm";

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

describe("submitSwarmRun (fixture path)", () => {
  test("composes qa-runner fanout into a QA Swarm projection and share URL", async () => {
    const control = mkControl();
    const job = control.submitSwarmRun({
      maxRuns: 2,
      maxWorkers: 2,
      target: "https://example.test",
      targetName: "Example Target",
    });
    expect(job.kind).toBe("swarm");
    expect(job.mode).toBe("mock");
    expect(job.qaShareUrl).toContain("https://openagents.com/qa/qa-run.swarm.example.test");

    const done = await control.wait(job.id);
    expect(done.status).toBe("succeeded");

    const artifacts = control.swarmRunArtifacts(job.id);
    expect(artifacts.qaShareUrl).toContain("/qa/");
    expect(artifacts.swarm).not.toBeNull();
    expect(artifacts.swarm!.projection.schemaVersion).toBe(QA_SWARM_RUN_PROJECTION_SCHEMA);
    expect(artifacts.swarm!.projection.verdict).toBe("warning");
    expect(artifacts.swarm!.childRunIds.length).toBe(2);
    expect(artifacts.swarm!.tiers.some(tier => tier.backend === "gce-tier-2" && tier.status === "skipped")).toBe(true);
    expect(artifacts.swarm!.tiers.some(tier => tier.backend === "cf-browser-rendering" && tier.status === "skipped")).toBe(true);
    expect(existsSync(artifacts.swarm!.projectionPath)).toBe(true);
  });

  test("accepts a stable public runRef for externally reviewed packets", async () => {
    const control = mkControl();
    const job = control.submitSwarmRun({
      runRef: "qa-run.executor.qs7-public-home",
      target: "https://executor.sh",
      targetName: "Executor",
    });

    expect(job.qaShareUrl).toBe("https://openagents.com/qa/qa-run.executor.qs7-public-home");

    await control.wait(job.id);
    const artifacts = control.swarmRunArtifacts(job.id);
    expect(artifacts.qaShareUrl).toBe("https://openagents.com/qa/qa-run.executor.qs7-public-home");
    expect(artifacts.swarm?.projection.runRef).toBe("qa-run.executor.qs7-public-home");
  });

  test("rejects missing target and invalid caps", () => {
    const control = mkControl();
    // @ts-expect-error intentionally missing target
    expect(() => control.submitSwarmRun({})).toThrow(BadRequestError);
    expect(() =>
      control.submitSwarmRun({ target: "https://example.test", maxWorkers: 0 }),
    ).toThrow(BadRequestError);
  });

  test("real swarm runs are refused unless armed", () => {
    const control = mkControl({ allowReal: false });
    expect(() =>
      control.submitSwarmRun({ real: true, target: "https://openagents.com" }),
    ).toThrow(NotArmedError);
  });
});

// ---------------------------------------------------------------------------
// #6210: trace publishing -> /trace/{uuid} as the shareable link. A FAKE local
// ingest (no network) returns a deterministic uuid per Idempotency-Key.
// ---------------------------------------------------------------------------

const fakeIngestControl = () => {
  const posts: Array<{ headers: Record<string, string>; body: string }> = [];
  let n = 0;
  const fetch: FetchLike = async (_url, init) => {
    posts.push({ headers: init.headers, body: init.body });
    const uuid = `00000000-0000-4000-8000-00000000000${(n++).toString(16)}`;
    return {
      ok: true,
      status: 201,
      text: async () => JSON.stringify({ uuid, url: `/trace/${uuid}`, visibility: "unlisted" }),
    };
  };
  return { fetch, posts };
};

const armedPublish = { url: "https://openagents.com/api/traces", token: "oa_agent_TEST123456789" };

describe("#6210 trace publishing", () => {
  test("a RUN publishes a trace and runArtifacts links /trace/{uuid} (not /pro/evals)", async () => {
    const { fetch, posts } = fakeIngestControl();
    const control = mkControl({ publishTrace: armedPublish, publishFetch: fetch });
    const job = control.submitRun({ scenario: "login-regression" });
    await control.wait(job.id);

    const art = control.runArtifacts(job.id);
    expect(art.traceUrl).toBe(
      "https://openagents.com/trace/00000000-0000-4000-8000-000000000000",
    );
    expect(art.traceUrl).not.toContain("/pro/evals");
    // the operator-console deep link is retained but is NOT the shareable link
    expect(art.proUrl).toBe(`https://openagents.com/pro/runs/${job.id}`);
    // it really POSTed with the agent bearer + idempotency key
    expect(posts.length).toBe(1);
    expect(posts[0]!.headers.authorization).toBe(`Bearer ${armedPublish.token}`);
    expect(posts[0]!.headers["idempotency-key"]).toBeTruthy();
    // #6216: the run receipt's execution-trace evidence (`traceRef`) is upgraded
    // to the PUBLISHED uuid so the settlement receipt points at the shareable trace.
    expect(art.receipt!["traceRef"]).toBe("00000000-0000-4000-8000-000000000000");
  });

  test("an EVAL publishes per-variant traces; the comparison links /trace/{uuid}", async () => {
    const { fetch } = fakeIngestControl();
    const control = mkControl({ publishTrace: armedPublish, publishFetch: fetch });
    const job = control.submitEval({
      title: "baseline vs candidate",
      variants: [
        { id: "baseline", scenario: "login-regression" },
        { id: "candidate", scenario: "login-regression-wrong" },
      ],
    });
    await control.wait(job.id);

    const res = control.evalComparison(job.id);
    expect(res.traceUrl).toContain("https://openagents.com/trace/compare/");
    expect(res.traceUrl).not.toContain("/pro/evals");
    expect(Object.keys(res.variantTraceUrls ?? {}).sort()).toEqual(["baseline", "candidate"]);
    expect(res.variantTraceUrls!.baseline).toContain("/trace/");
  });

  test("UNARMED (no publish config): honest no-op — traceUrl null, with a note", async () => {
    // No publishTrace config + a fetch that throws if ever called.
    const control = mkControl({
      publishFetch: (async () => {
        throw new Error("must not publish when unarmed");
      }) as unknown as FetchLike,
    });
    // Force unarmed regardless of CI env by clearing the env keys.
    const keys = ["QA_TRACE_PUBLISH_URL", "QA_TRACE_PUBLISH_TOKEN", "OPENAGENTS_AGENT_TOKEN", "OPENAGENTS_AGENT_PENDING_TOKEN"];
    const saved = keys.map((k) => [k, process.env[k]] as const);
    for (const k of keys) delete process.env[k];
    try {
      const job = control.submitRun({ scenario: "login-regression" });
      await control.wait(job.id);
      const art = control.runArtifacts(job.id);
      expect(art.traceUrl).toBeNull();
      expect(art.traceNote).toBeTruthy();
      // the run itself still succeeded + is dereferenceable
      expect(art.result!["status"]).toBe("pass");
      // #6216: even UNARMED, the receipt carries HONEST execution-trace evidence —
      // the run's local ATIF trajectory_id (never a fabricated uuid).
      expect(art.receipt!["traceRef"]).toBe(`${job.id}-trajectory`);
    } finally {
      for (const [k, v] of saved) {
        if (v === undefined) delete process.env[k];
        else process.env[k] = v;
      }
    }
  });
});
