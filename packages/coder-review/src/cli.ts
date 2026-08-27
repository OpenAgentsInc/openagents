/**
 * `coder:review` — review one autoimprovement cycle from its artifacts.
 *
 * `docs/coder/runbook.md` §6 says to run the review as a separate conversation
 * whose only inputs are the artifacts, and then hands a human a prompt skeleton
 * to fill in by copying transcripts. This command is that step done
 * mechanically:
 *
 *     pnpm run coder:review -- /tmp/gym-jobs/<job>/<run> \
 *       --suite tb2-quick --lane proxy --lever HEAD~1 \
 *       --slug tool-description-batching
 *
 * It assembles the request from the job directory, redacts it with the one rule
 * list, renders the §6 prompt with the citable evidence refs printed in it,
 * asks the reviewer, checks every citation against what the reviewer was
 * actually given, and writes `docs/coder/reviews/YYYY-MM-DD-<slug>.md` plus the
 * machine-readable review beside it.
 *
 * `--offline <file>` replays a recorded reviewer response instead of asking a
 * model. It replays; it never generates. A command whose offline mode invented
 * a plausible score would be the exact failure the review loop exists to catch,
 * so there is no fallback path in this file that produces a review without a
 * reviewer having written one.
 *
 * EXIT CODES. 0 the review was accepted and written. 1 the reviewer answered
 * and the answer was refused — the named reasons are on stderr, and nothing was
 * written. 2 the review could not be run at all: bad arguments, no job
 * directory, no reviewer. A refusal is a finding; an unrunnable command is not.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";

import {
  assembleReviewRequest,
  DEFAULT_MAX_DIFF_LINES,
  DEFAULT_MAX_STEP_CHARS,
  DEFAULT_MAX_STEPS,
  readBenchRows,
} from "./assemble.js";
import { replayLane, responsesLane, ReviewerUnavailable } from "./lane.js";
import type { ReviewerLane } from "./lane.js";
import { renderRejections, renderReviewPrompt } from "./prompt.js";
import { reviewPathFor, runCycleReview, slugOf, writeReviewArtifacts } from "./run.js";

const USAGE = `Usage: coder:review <job-dir> [options]

Arguments:
  <job-dir>              A completed Harbor job directory: the one holding
                         result.json and one directory per trial.

The lever (one is required — a review that does not know what changed cannot
say whether the change caused the delta):
  --lever <ref>          Take the lever's diff from \`git diff <ref>\`.
  --diff <file>          Take the lever's diff from a file instead of git.
  --no-diff              This cycle changed nothing (a baseline run). Says so
                         in the prompt rather than showing an empty diff.

Options:
  --suite <name>         Suite this cycle measured. Default: tb2-quick
  --lane <proxy|local>   Lane the run used. Default: proxy
  --store <file>         bench-results store to read rows from.
                         Default: bench-results/<suite>.jsonl
  --rows <n>             How many trailing store rows to show. Default: 4
  --practices <file>     The ledger. Default: docs/coder/best-practices.md
  --reviews-dir <dir>    Where the review lands. Default: docs/coder/reviews
  --slug <name>          Lever slug for the filename. Default: from --lever
  --title <text>         Review file title. Default: from the slug and suite
  --offline <file>       Replay a recorded reviewer response from this file
                         instead of asking a model. Replays only; a replay lane
                         has no opinion and its ref says so in the review file.
  --api-url <url>        Reviewer origin. Default: $OPENAGENTS_CODER_API_URL,
                         else $OPENAGENTS_API_URL, else http://localhost:4000
  --reviewer-model <id>  Model to review with. Prefer one from a different
                         family than the cycle ran on where the finding is
                         load-bearing (autoimprove §6).
  --print-prompt         Print the assembled prompt and exit. Asks no reviewer
                         and writes nothing: this is how the manual §6 lane
                         gets a prompt it did not hand-assemble.
  --max-steps <n>        Trajectory steps kept per trial, from the tail.
  --max-step-chars <n>   Characters kept per step and per tool result.
  --max-diff-lines <n>   Lines kept of the lever's diff.
  --json                 Emit the outcome as JSON on stdout.
  -h, --help             Show this help.

Exit codes: 0 accepted and written, 1 the reviewer's answer was refused (the
named reasons are on stderr and nothing was written), 2 the review could not be
run at all.`;

/** The reviewer answered and its answer did not survive the checks. */
const REFUSED_EXIT = 1;
/** The review could not be run. Distinct from a refusal, which is a finding. */
const UNRUNNABLE_EXIT = 2;

interface Arguments {
  readonly jobDir: string;
  readonly suite: string;
  readonly lane: string;
  readonly leverRef: string | null;
  readonly diffPath: string | null;
  readonly noDiff: boolean;
  readonly storePath: string | null;
  readonly rows: number;
  readonly practicesPath: string;
  readonly reviewsDir: string;
  readonly slug: string | null;
  readonly title: string | null;
  readonly offlinePath: string | null;
  readonly apiUrl: string | null;
  readonly reviewerModel: string | null;
  readonly printPrompt: boolean;
  readonly maxSteps: number | null;
  readonly maxStepChars: number | null;
  readonly maxDiffLines: number | null;
  readonly json: boolean;
}

/** `--` is what pnpm forwards into argv, never an argument. */
const parseArguments = (argv: ReadonlyArray<string>): Arguments | "help" => {
  let jobDir: string | null = null;
  let suite = "tb2-quick";
  let lane = "proxy";
  let leverRef: string | null = null;
  let diffPath: string | null = null;
  let noDiff = false;
  let storePath: string | null = null;
  let rows = 4;
  let practicesPath = "docs/coder/best-practices.md";
  let reviewsDir = "docs/coder/reviews";
  let slug: string | null = null;
  let title: string | null = null;
  let offlinePath: string | null = null;
  let apiUrl: string | null = null;
  let reviewerModel: string | null = null;
  let printPrompt = false;
  let maxSteps: number | null = null;
  let maxStepChars: number | null = null;
  let maxDiffLines: number | null = null;
  let json = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]!;
    if (argument === "--") continue;
    if (argument === "-h" || argument === "--help") return "help";
    if (argument === "--json") {
      json = true;
      continue;
    }
    if (argument === "--no-diff") {
      noDiff = true;
      continue;
    }
    if (argument === "--print-prompt") {
      printPrompt = true;
      continue;
    }
    if (argument === "--suite") {
      suite = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--lane") {
      lane = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--lever") {
      leverRef = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--diff") {
      diffPath = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--store") {
      storePath = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--rows") {
      rows = expectCount(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--practices") {
      practicesPath = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--reviews-dir") {
      reviewsDir = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--slug") {
      slug = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--title") {
      title = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--offline") {
      offlinePath = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--api-url") {
      apiUrl = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--reviewer-model") {
      reviewerModel = expectValue(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--max-steps") {
      maxSteps = expectCount(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--max-step-chars") {
      maxStepChars = expectCount(argv, (index += 1), argument);
      continue;
    }
    if (argument === "--max-diff-lines") {
      maxDiffLines = expectCount(argv, (index += 1), argument);
      continue;
    }
    if (argument.startsWith("-")) throw new Error(`unknown option: ${argument}`);
    if (jobDir !== null) throw new Error(`unexpected extra argument: ${argument}`);
    jobDir = argument;
  }

  if (jobDir === null) throw new Error("missing required <job-dir> argument");
  if (lane !== "proxy" && lane !== "local") {
    throw new Error(`--lane must be proxy or local, got: ${lane}`);
  }
  const leverSources = [leverRef !== null, diffPath !== null, noDiff].filter(Boolean).length;
  if (leverSources === 0) {
    throw new Error(
      "name the lever: --lever <ref>, --diff <file>, or --no-diff for a cycle that changed nothing. A review that cannot see the change cannot attribute the delta to it.",
    );
  }
  if (leverSources > 1) {
    throw new Error("--lever, --diff, and --no-diff are three answers to one question; pick one");
  }

  return {
    jobDir,
    suite,
    lane,
    leverRef,
    diffPath,
    noDiff,
    storePath,
    rows,
    practicesPath,
    reviewsDir,
    slug,
    title,
    offlinePath,
    apiUrl,
    reviewerModel,
    printPrompt,
    maxSteps,
    maxStepChars,
    maxDiffLines,
    json,
  };
};

const expectValue = (argv: ReadonlyArray<string>, index: number, option: string): string => {
  const value = argv[index];
  if (value === undefined || value.startsWith("-")) throw new Error(`${option} needs a value`);
  return value;
};

const expectCount = (argv: ReadonlyArray<string>, index: number, option: string): number => {
  const value = Number(expectValue(argv, index, option));
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${option} needs a whole number of at least 1`);
  }
  return value;
};

/** The diff of the lever, from wherever this invocation says it lives. */
const leverOf = (parsed: Arguments): { readonly ref: string; readonly diff: string } => {
  if (parsed.noDiff) return { ref: "none", diff: "" };
  if (parsed.diffPath !== null) {
    return { ref: parsed.diffPath, diff: readFileSync(parsed.diffPath, "utf8") };
  }
  const ref = parsed.leverRef!;
  try {
    return {
      ref,
      diff: execFileSync("git", ["diff", ref], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }),
    };
  } catch (cause) {
    throw new Error(
      `\`git diff ${ref}\` failed: ${String(cause)}. Pass --diff <file> if the lever is not a ref in this checkout.`,
    );
  }
};

const reviewerOf = (parsed: Arguments): ReviewerLane => {
  if (parsed.offlinePath !== null) {
    const path = parsed.offlinePath;
    if (!existsSync(path)) {
      throw new ReviewerUnavailable(
        `--offline ${path} does not exist. The replay lane returns a recorded reviewer response; it does not write one.`,
      );
    }
    // The basename, not the path: the ref is written into the review file and
    // into every candidate's lineage, and a local path is a local fact that
    // means nothing to the next reader of `docs/coder/reviews/`.
    return replayLane(basename(path), () => readFileSync(path, "utf8"));
  }
  const origin =
    parsed.apiUrl ??
    process.env["OPENAGENTS_CODER_API_URL"] ??
    process.env["OPENAGENTS_API_URL"] ??
    "http://localhost:4000";
  return responsesLane({
    origin,
    token: process.env["OPENAGENTS_TOKEN"],
    model: parsed.reviewerModel ?? undefined,
  });
};

// Not exported: this module runs on import, so a caller that reached `main`
// would have already run the command once to get at it. The tests drive it the
// way pnpm does, as a process.
const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let parsed: Arguments | "help";
  try {
    parsed = parseArguments(argv);
  } catch (error) {
    process.stderr.write(`${(error as Error).message}\n\n${USAGE}\n`);
    return UNRUNNABLE_EXIT;
  }
  if (parsed === "help") {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }

  const recordedAt = new Date().toISOString();
  const slug = slugOf(
    parsed.slug ??
      parsed.leverRef ??
      (parsed.diffPath === null ? "baseline" : basename(parsed.diffPath)),
  );

  try {
    const lever = leverOf(parsed);
    const storePath = parsed.storePath ?? join("bench-results", `${parsed.suite}.jsonl`);
    const practices = {
      path: parsed.practicesPath,
      text: readFileSync(parsed.practicesPath, "utf8"),
    };

    const inputs = {
      suite: parsed.suite,
      lane: parsed.lane,
      lever,
      rows: readBenchRows(storePath, parsed.suite, parsed.rows),
      practices,
      maxSteps: parsed.maxSteps ?? DEFAULT_MAX_STEPS,
      maxStepChars: parsed.maxStepChars ?? DEFAULT_MAX_STEP_CHARS,
      maxDiffLines: parsed.maxDiffLines ?? DEFAULT_MAX_DIFF_LINES,
    };

    // `--print-prompt` asks nobody. The prompt is the artifact the manual §6
    // lane needs, and producing it must not depend on a reviewer being
    // reachable — otherwise the fallback for a down lane is hand-assembly,
    // which is the step this command replaces.
    if (parsed.printPrompt) {
      const assembled = assembleReviewRequest(parsed.jobDir, inputs);
      process.stdout.write(`${renderReviewPrompt(assembled.request, assembled.index)}\n`);
      return 0;
    }

    const outcome = await runCycleReview({
      jobDir: parsed.jobDir,
      ...inputs,
      reviewer: reviewerOf(parsed),
    });

    if (!outcome.result.ok) {
      process.stderr.write(`${renderRejections(outcome.result.rejections, outcome.reviewerRef)}\n`);
      if (parsed.json) {
        process.stdout.write(
          `${JSON.stringify(
            {
              accepted: false,
              reviewer: outcome.reviewerRef,
              rejections: outcome.result.rejections,
            },
            null,
            2,
          )}\n`,
        );
      }
      return REFUSED_EXIT;
    }

    const artifacts = writeReviewArtifacts({
      outcome,
      markdownPath: reviewPathFor(parsed.reviewsDir, recordedAt, slug),
      title: parsed.title ?? `Cycle review: ${slug} on ${parsed.suite}`,
      recordedAt,
    });

    process.stdout.write(
      parsed.json
        ? `${JSON.stringify(
            {
              accepted: true,
              reviewer: outcome.reviewerRef,
              score: outcome.result.review.score,
              proposals: outcome.result.review.proposals.length,
              markdownPath: artifacts.markdownPath,
              jsonPath: artifacts.jsonPath,
            },
            null,
            2,
          )}\n`
        : [
            `Score ${String(outcome.result.review.score)}/10 from ${outcome.reviewerRef}.`,
            `${String(outcome.result.review.proposals.length)} proposal(s), ${String(
              outcome.result.review.ledgerOperations.length,
            )} ledger operation(s).`,
            `Wrote ${artifacts.markdownPath}`,
            `Wrote ${artifacts.jsonPath}`,
            "Adopting a proposal is a separate act. Each enters the runbook at §3.",
            "",
          ].join("\n"),
    );
    return 0;
  } catch (error) {
    process.stderr.write(`coder:review: ${(error as Error).message}\n`);
    return UNRUNNABLE_EXIT;
  }
};

void main(process.argv.slice(2)).then((code) => {
  process.exitCode = code;
});
