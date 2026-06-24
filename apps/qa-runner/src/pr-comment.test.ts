// PR-comment + gh-attach composition tests (#6185) — unit + dry-run, NO network.
//
// Proves the PR-evidence comment is composed honestly with a fake gh-attach
// runner: the gh-attach'd embed is used when the upload succeeds; the in-eval
// relative video ref is used when gh-attach is unavailable/unauthenticated (no
// broken/fake embed); the /pro link is always present; and a failing variant is
// reported as failing (no fake green).

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { type EvalVariant, runEval } from "./evals";
import { makeFakeChromium } from "./fake-chromium";
import {
  ghAttachUpload,
  ghAttachVariantVideos,
  type GhAttachRunner,
} from "./gh-attach";
import {
  composePrComment,
  PR_COMMENT_MARKER,
} from "./pr-comment";
import { loginRegressionSteps, loginRegressionStepsWrong } from "./scenarios";
import { makeTarget } from "./target";
import { verifyCommitments } from "./verify";
import { captureFailurePattern, suggestFromPattern } from "./failure-learning";
import type { QaRunResult } from "./result";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-pr-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const passingChromium = () =>
  makeFakeChromium({
    pages: {
      "/login": {
        text: "Log in to OpenAgents",
        html: "<form>Log in to OpenAgents</form>",
      },
    },
  });

const variants = (): ReadonlyArray<EvalVariant> => [
  {
    id: "mcp-on",
    label: "MCP on",
    brain: () => scriptedBrain(loginRegressionSteps()),
    backend: () => localBackend({ chromium: passingChromium() }),
  },
  {
    id: "mcp-off",
    label: "MCP off",
    brain: () => scriptedBrain(loginRegressionStepsWrong()),
    backend: () => localBackend({ chromium: passingChromium() }),
  },
];

const runSampleEval = () =>
  Effect.runPromise(
    runEval({
      id: "login-mcp-compare",
      title: "Login under MCP on vs off",
      target: makeTarget({ name: "fake", baseUrl: "https://example.test" }),
      scenario: { id: "login-regression", label: "/login renders sign-in" },
      variants: variants(),
      artifactDir: dir,
      now: () => new Date("2026-06-24T00:00:00.000Z"),
    }),
  );

// A fake gh-attach that "uploads" by echoing a deterministic asset URL.
const fakeOkRunner: GhAttachRunner = {
  run: async args => {
    const file = args[args.length - 1] ?? "file";
    const base = file.split("/").pop() ?? "file";
    return {
      ok: true,
      stdout: `![${base}](https://github.com/o/r/assets/1/${base})\n`,
    };
  },
};

const fakeUnavailableRunner: GhAttachRunner = {
  run: async () => ({ ok: false, stdout: "" }),
};

describe("ghAttachUpload (fake runner)", () => {
  test("returns the embeddable markdown on success", async () => {
    const md = await ghAttachUpload(fakeOkRunner, "/tmp/clip.webm", {
      repo: "o/r",
    });
    expect(md).toBe(
      "![clip.webm](https://github.com/o/r/assets/1/clip.webm)",
    );
  });

  test("returns null when gh-attach is unavailable (honest, no fake embed)", async () => {
    const md = await ghAttachUpload(fakeUnavailableRunner, "/tmp/clip.webm");
    expect(md).toBeNull();
  });

  test("returns null when the runner throws (binary missing)", async () => {
    const throwing: GhAttachRunner = {
      run: async () => {
        throw new Error("gh-attach: command not found");
      },
    };
    const md = await ghAttachUpload(throwing, "/tmp/clip.webm");
    expect(md).toBeNull();
  });

  test("ghAttachVariantVideos maps only the successful uploads", async () => {
    const map = await ghAttachVariantVideos(fakeOkRunner, [
      { variantId: "a", filePath: "/tmp/a.webm" },
      { variantId: "b", filePath: "/tmp/b.webm" },
    ]);
    expect(Object.keys(map).sort()).toEqual(["a", "b"]);
    expect(map.a).toContain("a.webm");
  });
});

describe("composePrComment", () => {
  test("uses the gh-attach'd embed when upload succeeds + always includes the /pro link", async () => {
    const outcome = await runSampleEval();
    const onVideo = outcome.result.variants
      .find(v => v.variantId === "mcp-on")!
      .runs[0]!.video!;
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      variantVideoPaths: [
        { variantId: "mcp-on", filePath: join(dir, onVideo) },
      ],
      ghAttach: fakeOkRunner,
      ghAttachOptions: { repo: "OpenAgentsInc/openagents" },
    });
    expect(body.startsWith(PR_COMMENT_MARKER)).toBe(true);
    expect(body).toContain("github.com/o/r/assets")
    expect(body).toContain(
      "https://openagents.com/pro/evals/login-mcp-compare",
    );
    // honest: a failing variant is reported as failing.
    expect(body).toContain("some variants failed")
  });

  test("falls back to the relative video ref when gh-attach is unavailable", async () => {
    const outcome = await runSampleEval();
    const onVideo = outcome.result.variants
      .find(v => v.variantId === "mcp-on")!
      .runs[0]!.video!;
    // the artifact really exists on disk (dereferenceable)
    expect(existsSync(join(dir, onVideo))).toBe(true);
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      variantVideoPaths: [
        { variantId: "mcp-on", filePath: join(dir, onVideo) },
      ],
      ghAttach: fakeUnavailableRunner,
    });
    // no broken embed; the relative ref is shown instead.
    expect(body).not.toContain("github.com/o/r/assets");
    expect(body).toContain(onVideo);
    expect(body).toContain(
      "https://openagents.com/pro/evals/login-mcp-compare",
    );
  });

  test("composes without any gh-attach runner (pure dry-run)", async () => {
    const outcome = await runSampleEval();
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
    });
    expect(body).toContain(PR_COMMENT_MARKER);
    expect(body).toContain("Chill-eval");
    expect(body).toContain("/pro/evals/login-mcp-compare");
  });

  test("#6192: a REFUTED verify verdict is surfaced (no fake pass), with inline evidence", async () => {
    const outcome = await runSampleEval();
    // a FALSE claim: the run says /login redirects away (it does not) -> REFUTED
    const verify = verifyCommitments({
      commitments: [
        {
          id: "claims-redirect",
          claim: "/login redirects away (FALSE claim)",
          evidence: "step-pass",
          match: "redirects away from /login",
        },
      ],
      steps: [
        { index: 0, kind: "assert", label: "redirects away from /login (intentionally wrong)", status: "failed" },
      ],
      runStatus: "fail",
    });
    expect(verify.verdict).toBe("REFUTED");

    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      verify,
    });
    // the verdict leads the comment
    expect(body).toContain("Verify verdict: REFUTED");
    // the contradicting evidence is inline
    expect(body).toContain("claims-redirect");
    expect(body).toContain("Commitment evidence");
    // never a fake green for a refuted claim
    expect(body).not.toContain("Verify verdict: CONFIRMED");
  });

  test("#6192: a CONFIRMED verdict renders when commitments are observed-ok", async () => {
    const outcome = await runSampleEval();
    const verify = verifyCommitments({
      commitments: [
        {
          id: "renders",
          claim: "renders sign-in copy",
          evidence: "step-pass",
          match: 'body contains "Log in to OpenAgents"',
        },
      ],
      steps: [
        { index: 0, kind: "assert", label: 'body contains "Log in to OpenAgents"', status: "ok" },
      ],
      runStatus: "pass",
    });
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      verify,
    });
    expect(body).toContain("Verify verdict: CONFIRMED");
    expect(body).toContain("1/1 confirmed");
  });

  test("#6192: no verify report -> no verdict block (additive)", async () => {
    const outcome = await runSampleEval();
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
    });
    expect(body).not.toContain("Verify verdict");
  });

  // -------------------------------------------------------------------------
  // #6195: failure-learning suggestion section (default suggest-only).
  // -------------------------------------------------------------------------

  const refutedRunResult = (): QaRunResult => ({
    schemaVersion: "openagents.qa_runner.result.v1",
    status: "fail",
    target: { name: "openagents.com", baseUrl: "https://example.test" },
    brain: "scripted",
    backend: "local",
    startedAt: "2026-06-24T00:00:00.000Z",
    endedAt: "2026-06-24T00:00:01.000Z",
    durationMs: 1000,
    steps: [
      { index: 0, kind: "assert", label: "redirects away from /login (intentionally wrong)", status: "failed" },
    ],
    artifacts: { screenshots: [] },
    failure: "assertion failed",
    verify: {
      verdict: "REFUTED",
      observed: true,
      findings: [
        {
          id: "claims-redirect",
          claim: "/login redirects away (FALSE claim)",
          verdict: "REFUTED",
          evidenceSummary: 'observed step "redirects away from /login" = failed',
        },
      ],
    },
  });

  test("#6195: a captured failure pattern renders the default suggest-only section", async () => {
    const outcome = await runSampleEval();
    const pattern = captureFailurePattern(refutedRunResult())!;
    const suggestion = suggestFromPattern(pattern);
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      failureSuggestion: suggestion,
    });
    expect(body).toContain("Failure learning (#6195)");
    expect(body).toContain("Strategy: `suggest_in_report` (default");
    expect(body).toContain("Captured failure pattern + suggested fix");
    expect(body).toContain("claims-redirect");
    // suggest-only: no planned mutation surfaced
    expect(body).not.toContain("Planned mutation");
  });

  test("#6195: an armed open_pr surfaces a PLAN-ONLY mutation (never executed)", async () => {
    const outcome = await runSampleEval();
    const pattern = captureFailurePattern(refutedRunResult())!;
    const suggestion = suggestFromPattern(pattern, { strategy: "open_pr", armMutations: true });
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      failureSuggestion: suggestion,
    });
    expect(body).toContain("Strategy: `open_pr` (PLAN ONLY");
    expect(body).toContain("Planned mutation (`open_pr`, executed=false)");
  });

  test("#6195: a downgraded auto_commit states the downgrade honestly", async () => {
    const outcome = await runSampleEval();
    const pattern = captureFailurePattern(refutedRunResult())!;
    // requested auto_commit but NOT armed -> downgraded to suggest-only
    const suggestion = suggestFromPattern(pattern, { strategy: "auto_commit" });
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      failureSuggestion: suggestion,
    });
    expect(body).toContain("downgraded from `auto_commit`");
    expect(body).not.toContain("Planned mutation");
  });

  test("#6195: no failure suggestion -> no failure-learning section (additive)", async () => {
    const outcome = await runSampleEval();
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
    });
    expect(body).not.toContain("Failure learning (#6195)");
  });
});
