/**
 * The cycle review end to end, against a staged Harbor job.
 *
 * The fixture job is committed rather than borrowed from `/tmp`: a real job
 * directory is where this command is meant to run, and it is also the one thing
 * a test cannot depend on, because `/tmp` does not survive. The fixture carries
 * the shapes that matter — an accepted trial and a rejected one, tool calls with
 * their observations, and a step whose output is full of token-shaped strings.
 *
 * The last of those is the point of the redaction test. `openagents trace
 * redact` once reported "Nothing matched the redaction rules" over a file of
 * live tokens, because a second hand-written rule list had forgotten two
 * prefixes. Everything assembled here leaves this process for another
 * conversation, so the test asserts the absence of the literal strings in the
 * prompt and in the written review, not the presence of a redaction count.
 */

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  assembleReviewRequest,
  DEFAULT_MAX_DIFF_LINES,
  DEFAULT_MAX_STEP_CHARS,
  DEFAULT_MAX_STEPS,
  NotAHarborJob,
  diffPathsOf,
  ledgerEntryIds,
  readBenchRows,
} from "../src/assemble.js";
import { replayLane, ReviewerUnavailable } from "../src/lane.js";
import { renderReviewPrompt } from "../src/prompt.js";
import { reviewPathFor, runCycleReview, slugOf, writeReviewArtifacts } from "../src/run.js";

const fixtures = fileURLToPath(new URL("./fixtures/coder-review/", import.meta.url));
const repoRoot = fileURLToPath(new URL("../../../", import.meta.url));
const cliPath = "packages/coder-review/src/cli.ts";
const jobDir = join(fixtures, "job");

/**
 * The exact strings the fixture's trajectory leaks.
 *
 * Written as fragments joined at runtime so this file does not itself carry a
 * token-shaped literal that a scanner would have to be told to ignore. The
 * fixture holds the whole strings, which is where they belong: it is the
 * artifact under test.
 */
const LEAKED = [
  ["oa", "pat", "9fQ2xLm4Rt7Vb1Zk"].join("_"),
  ["smct", "7hJ3kP0qWz"].join("_"),
  ["oa", "agent", "4dTgH8nMxQ"].join("_"),
  "sk-live-3f9aQz7bV1xR",
];

const inputs = () => ({
  suite: "tb2-quick",
  lane: "proxy",
  lever: { ref: "HEAD~1", diff: readFileSync(join(fixtures, "lever.diff"), "utf8") },
  rows: readBenchRows(join(fixtures, "rows.jsonl"), "tb2-quick", 4),
  practices: {
    path: "docs/coder/best-practices.md",
    text: readFileSync(join(fixtures, "practices.md"), "utf8"),
  },
  maxSteps: DEFAULT_MAX_STEPS,
  maxStepChars: DEFAULT_MAX_STEP_CHARS,
  maxDiffLines: DEFAULT_MAX_DIFF_LINES,
});

const recorded = (name: string): string => readFileSync(join(fixtures, name), "utf8");

describe("assembling a review request", () => {
  it("reads both trials, their outcomes, and the figures behind them", () => {
    const { request } = assembleReviewRequest(jobDir, inputs());

    expect(request.jobId).toBe("f1x7u12e-0000-4000-8000-000000000001");
    expect(request.trials.map((trial) => trial.task)).toEqual(["pin-a-version", "regex-log"]);
    expect(request.trials.map((trial) => trial.outcome)).toEqual(["rejected", "accepted"]);

    const accepted = request.trials.find((trial) => trial.task === "regex-log")!;
    expect(accepted.modelId).toBe("fixture-model");
    expect(accepted.agentVersion).toBe("0.4.0");
    expect(accepted.promptTokens).toBe(15_300);
    expect(accepted.toolCalls).toBe(3);
    expect(accepted.wallClockSeconds).toBeCloseTo(130, 0);
    expect(accepted.threadId).toBe("11111111-2222-4333-8444-555555555555");
    expect(accepted.instructionSource).toBe("trajectory_first_user_step");
    expect(accepted.truncation).toMatchObject({ tail_only: false, kept_steps: 5, total_steps: 5 });
  });

  it("keeps each tool call next to its own observation", () => {
    const { request } = assembleReviewRequest(jobDir, inputs());
    const batched = request.trials
      .find((trial) => trial.task === "pin-a-version")!
      .steps.find((step) => step.toolCalls.length === 2)!;

    expect(batched.toolCalls.map((call) => call.name)).toEqual(["shell", "shell"]);
    expect(batched.toolCalls[0]!.arguments).toContain("pip freeze");
    expect(batched.toolCalls[0]!.observation).toContain("requests==2.32.3");
    expect(batched.toolCalls[1]!.arguments).toContain("cat requirements.txt");
    expect(batched.toolCalls[1]!.observation).not.toContain("requests==2.32.3");
  });

  it("keeps the tail and says how much it dropped", () => {
    const { request, index } = assembleReviewRequest(jobDir, { ...inputs(), maxSteps: 2 });
    const accepted = request.trials.find((trial) => trial.task === "regex-log")!;

    expect(accepted.truncation).toMatchObject({
      tail_only: true,
      dropped_leading_steps: 3,
      kept_steps: 2,
      total_steps: 5,
    });
    expect(accepted.steps.map((step) => step.stepId)).toEqual(["4", "5"]);
    // A dropped step is not citable. That is what makes the truncation notice
    // load-bearing rather than decorative.
    expect(index.trajectorySteps.has("regex-log#step-1")).toBe(false);
    expect(index.trajectorySteps.has("regex-log#step-5")).toBe(true);
  });

  it("builds the citable set from the request it actually assembled", () => {
    const { index } = assembleReviewRequest(jobDir, inputs());

    expect([...index.trialOutcomes].sort()).toEqual(["pin-a-version", "regex-log"]);
    expect(index.benchRows).toContain("tb2-quick#2026-08-26T09:10:00.000Z");
    expect([...index.ledgerEntries].sort()).toEqual(["M1", "T1"]);
    expect([...index.diffPaths]).toEqual(["crates/openagents-cli/src/tools.rs"]);
  });

  it("refuses a directory that is not a Harbor job instead of reviewing nothing", () => {
    expect(() =>
      assembleReviewRequest(mkdtempSync(join(tmpdir(), "not-a-job-")), inputs()),
    ).toThrow(NotAHarborJob);
  });

  it("refuses a trial directory, which carries a result.json and no trials", () => {
    // The near-miss that would otherwise assemble quietly: a job of zero trials
    // still gets scored, and the score would be about nothing.
    expect(() =>
      assembleReviewRequest(join(fixtures, "job", "regex-log__F1xTur3"), inputs()),
    ).toThrow(/holds no trial directories/u);
  });
});

describe("redaction", () => {
  it("keeps every token-shaped string in the fixture out of the assembled request", () => {
    const { request, index } = assembleReviewRequest(jobDir, inputs());
    const serialized = JSON.stringify(request);
    const prompt = renderReviewPrompt(request, index);

    for (const secret of LEAKED) {
      expect(serialized).not.toContain(secret);
      expect(prompt).not.toContain(secret);
    }
    expect(request.redaction.appliedBeforeExternalInference).toBe(true);
    expect(request.redaction.total).toBeGreaterThan(0);
  });

  it("keeps the operator's home path out of it too, and says how many it replaced", () => {
    const { request } = assembleReviewRequest(jobDir, inputs());
    expect(JSON.stringify(request)).not.toContain("/Users/fixture-operator");
    expect(Object.keys(request.redaction.counts).length).toBeGreaterThan(0);
  });
});

describe("the reviewer prompt", () => {
  it("prints the refs a citation is checked against", () => {
    const { request, index } = assembleReviewRequest(jobDir, inputs());
    const prompt = renderReviewPrompt(request, index);

    expect(prompt).toContain("- trial:regex-log#step-3");
    expect(prompt).toContain("- trial:pin-a-version#outcome");
    expect(prompt).toContain("- ledger:T1");
    expect(prompt).toContain("- diff:crates/openagents-cli/src/tools.rs");
    expect(prompt).toContain("- row:tb2-quick#2026-08-26T09:10:00.000Z");
  });

  it("carries the trials, the lever, the rows, and the ledger", () => {
    const { request, index } = assembleReviewRequest(jobDir, inputs());
    const prompt = renderReviewPrompt(request, index);

    expect(prompt).toContain("Pinned requests and click");
    expect(prompt).toContain("Batch independent commands into one");
    expect(prompt).toContain("T1. Batch independent commands into one tool call");
    expect(prompt).toContain("call shell:");
  });

  it("says when a trial was truncated, in the prompt the reviewer reads", () => {
    const { request, index } = assembleReviewRequest(jobDir, { ...inputs(), maxSteps: 2 });
    expect(renderReviewPrompt(request, index)).toContain("tail only");
  });
});

describe("the replay lane", () => {
  it("returns the recording without consulting the prompt", async () => {
    let asked: unknown;
    const lane = replayLane("fixture", () => "recorded");
    asked = await lane.ask("a prompt the lane must not read");
    expect(asked).toBe("recorded");
    expect(lane.ref).toBe("replay:fixture");
  });

  it("fails rather than filling in a review when the recording is empty", () => {
    expect(() => replayLane("fixture", () => "  ").ask("prompt")).toThrow(ReviewerUnavailable);
  });
});

describe("running one cycle review", () => {
  it("accepts a review whose citations resolve and writes both artifacts", async () => {
    const outcome = await runCycleReview({
      jobDir,
      ...inputs(),
      reviewer: replayLane("accepted", () => recorded("reviewer-accepted.txt")),
    });

    if (!outcome.result.ok) throw new Error(JSON.stringify(outcome.result.rejections, null, 2));
    expect(outcome.result.review.score).toBe(6);
    expect(outcome.result.review.proposals).toHaveLength(2);
    expect(outcome.result.review.violations[0]!.entry).toBe("T1");
    expect(outcome.producedBy).toBe("coder-review:job:replay:accepted");

    const dir = mkdtempSync(join(tmpdir(), "coder-review-"));
    const artifacts = writeReviewArtifacts({
      outcome,
      markdownPath: reviewPathFor(dir, "2026-08-26T10:00:00.000Z", "tool-description-batching"),
      title: "Cycle review: tool-description-batching on tb2-quick",
      recordedAt: "2026-08-26T10:00:00.000Z",
    });

    expect(artifacts.markdownPath.endsWith("2026-08-26-tool-description-batching.md")).toBe(true);
    expect(existsSync(artifacts.jsonPath)).toBe(true);

    const markdown = readFileSync(artifacts.markdownPath, "utf8");
    expect(markdown).toContain("**Score 6/10.**");
    expect(markdown).toContain("replay:accepted");
    expect(markdown).toContain("`trial:regex-log#step-3`");
    expect(markdown).toContain("| `regex-log` | accepted |");
    for (const secret of LEAKED) expect(markdown).not.toContain(secret);

    const document = JSON.parse(readFileSync(artifacts.jsonPath, "utf8")) as {
      readonly review: { readonly proposals: ReadonlyArray<{ readonly candidateId: string }> };
      readonly request: { readonly redaction: { readonly total: number } };
    };
    expect(document.review.proposals[0]!.candidateId.startsWith("candidate:")).toBe(true);
    expect(document.request.redaction.total).toBeGreaterThan(0);
  });

  it("refuses a review that invented a step, by name, and writes nothing", async () => {
    const outcome = await runCycleReview({
      jobDir,
      ...inputs(),
      reviewer: replayLane("invented", () => recorded("reviewer-invented-evidence.txt")),
    });

    if (outcome.result.ok) throw new Error("a review citing step-99 was accepted");
    const named = outcome.result.rejections.map((rejection) => rejection.reason);
    expect(named).toContain("evidence_ref_unresolved");
    expect(named).toContain("proposal_without_trajectory_evidence");

    const unresolved = outcome.result.rejections.find(
      (rejection) => rejection.reason === "evidence_ref_unresolved",
    )!;
    expect(unresolved.path).toBe("$.proposals[0].evidence[1].ref");
    expect(unresolved.detail).toContain("step-99");

    expect(() =>
      writeReviewArtifacts({
        outcome,
        markdownPath: join(mkdtempSync(join(tmpdir(), "coder-review-")), "x.md"),
        title: "t",
        recordedAt: "2026-08-26T10:00:00.000Z",
      }),
    ).toThrow(/refused review is not written/u);
  });
});

describe("small readers", () => {
  it("finds ledger ids and diff paths the way the refs spell them", () => {
    expect(ledgerEntryIds(readFileSync(join(fixtures, "practices.md"), "utf8"))).toEqual([
      "T1",
      "M1",
    ]);
    expect(diffPathsOf(readFileSync(join(fixtures, "lever.diff"), "utf8"))).toEqual([
      "crates/openagents-cli/src/tools.rs",
    ]);
  });

  it("slugs a lever name into a filename, and never into an empty one", () => {
    expect(slugOf("Tool description: batching!")).toBe("tool-description-batching");
    expect(slugOf("///")).toBe("cycle");
  });
});

/**
 * The command as `pnpm run coder:review` invokes it, separator and all.
 *
 * Driven as a process because that is the surface: `check:fast` runs the CLI
 * invocability guard for exactly the failure where a command is tested through
 * its module and never once reachable the way it ships.
 */
describe("the command", () => {
  const run = (args: ReadonlyArray<string>): { code: number; stdout: string; stderr: string } => {
    try {
      const stdout = execFileSync(process.execPath, ["--import", "tsx", cliPath, ...args], {
        cwd: repoRoot,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      });
      return { code: 0, stdout, stderr: "" };
    } catch (error) {
      const failure = error as { status?: number; stdout?: string; stderr?: string };
      return {
        code: failure.status ?? -1,
        stdout: failure.stdout ?? "",
        stderr: failure.stderr ?? "",
      };
    }
  };

  const common = (reviewsDir: string): ReadonlyArray<string> => [
    // The separator pnpm forwards. A command that dies on it is documented and
    // uninvocable at the same time.
    "--",
    jobDir,
    "--suite",
    "tb2-quick",
    "--lane",
    "proxy",
    "--diff",
    join(fixtures, "lever.diff"),
    "--store",
    join(fixtures, "rows.jsonl"),
    "--practices",
    join(fixtures, "practices.md"),
    "--reviews-dir",
    reviewsDir,
    "--slug",
    "tool-description-batching",
  ];

  it("writes the review and the machine-readable document, and exits zero", () => {
    const reviewsDir = join(mkdtempSync(join(tmpdir(), "coder-review-cli-")), "reviews");
    const result = run([
      ...common(reviewsDir),
      "--offline",
      join(fixtures, "reviewer-accepted.txt"),
    ]);

    expect(result.stderr).toBe("");
    expect(result.code).toBe(0);
    expect(result.stdout).toContain("Score 6/10 from replay:");
    expect(result.stdout).toContain("2 proposal(s)");

    const written = readdirSync(reviewsDir).sort();
    expect(written).toHaveLength(2);
    expect(written[0]!.endsWith("-tool-description-batching.json")).toBe(true);
    expect(written[1]!.endsWith("-tool-description-batching.md")).toBe(true);

    const markdown = readFileSync(join(reviewsDir, written[1]!), "utf8");
    expect(markdown).toContain("**Score 6/10.**");
    for (const secret of LEAKED) expect(markdown).not.toContain(secret);
  });

  it("exits one on a refused review, names the reason, and leaves the directory empty", () => {
    const reviewsDir = join(mkdtempSync(join(tmpdir(), "coder-review-cli-")), "reviews");
    const result = run([
      ...common(reviewsDir),
      "--offline",
      join(fixtures, "reviewer-invented-evidence.txt"),
    ]);

    expect(result.code).toBe(1);
    expect(result.stderr).toContain("evidence_ref_unresolved");
    expect(result.stderr).toContain("step-99");
    expect(result.stderr).toContain("proposal_without_trajectory_evidence");
    expect(result.stderr).toContain("Nothing was written");
    expect(existsSync(reviewsDir)).toBe(false);
  });

  it("prints the assembled prompt without asking a reviewer or writing a file", () => {
    const reviewsDir = join(mkdtempSync(join(tmpdir(), "coder-review-cli-")), "reviews");
    const result = run([...common(reviewsDir), "--print-prompt"]);

    expect(result.code).toBe(0);
    expect(result.stdout).toContain("You are reviewing one autoimprovement cycle");
    expect(result.stdout).toContain("- trial:regex-log#step-3");
    for (const secret of LEAKED) expect(result.stdout).not.toContain(secret);
    expect(existsSync(reviewsDir)).toBe(false);
  });

  it("refuses to run when the lever is not named", () => {
    const result = run(["--", jobDir, "--offline", join(fixtures, "reviewer-accepted.txt")]);
    expect(result.code).toBe(2);
    expect(result.stderr).toContain("name the lever");
  });

  it("refuses a replay file that does not exist rather than reviewing without one", () => {
    const reviewsDir = join(mkdtempSync(join(tmpdir(), "coder-review-cli-")), "reviews");
    const result = run([...common(reviewsDir), "--offline", join(fixtures, "nothing-here.txt")]);
    expect(result.code).toBe(2);
    expect(result.stderr).toContain("does not exist");
  });
});
