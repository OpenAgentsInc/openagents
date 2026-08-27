/**
 * One cycle review, end to end: artifacts in, checked review out.
 *
 * The order is the whole design. Assemble the request from the job directory
 * and redact it; build the evidence index from that redacted request, so the
 * set a citation is checked against is exactly the set the reviewer read;
 * render the prompt; ask the lane; parse and check. A review that survives all
 * of that is written to `docs/coder/reviews/`. One that does not is not
 * written at all — a refused review leaves no file, because a file in that
 * directory is a record of a judgment, and a refusal is a record of a reviewer
 * writing about artifacts it was not given.
 *
 * Nothing in this module scores anything. The score comes from the reviewer or
 * the run fails; there is no fallback that produces a number. That is the
 * single most important property here, and it is enforced structurally: the
 * only paths out of {@link runCycleReview} are "the reviewer answered and the
 * answer checked out", "the reviewer answered and the answer was refused, by
 * name", and a thrown {@link ReviewerUnavailable}.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import type { ReviewBenchRow, ReviewRequest } from "./assemble.js";
import { DEFAULT_MAX_DIFF_LINES, DEFAULT_MAX_STEPS, DEFAULT_MAX_STEP_CHARS } from "./assemble.js";
import { assembleReviewRequest } from "./assemble.js";
import type { EvidenceIndex, ParseCycleReviewResult } from "./candidate.js";
import { parseCycleReview } from "./candidate.js";
import type { ReviewerLane } from "./lane.js";
import { renderReviewMarkdown, renderReviewPrompt } from "./prompt.js";

/** The schema id the written review JSON carries. */
export const CODER_REVIEW_DOCUMENT_SCHEMA = "openagents.coder_review_document.v1";

export interface CycleReviewOptions {
  /** The Harbor job directory this cycle produced. */
  readonly jobDir: string;
  readonly suite: string;
  readonly lane: string;
  readonly lever: { readonly ref: string; readonly diff: string };
  readonly rows: ReadonlyArray<ReviewBenchRow>;
  readonly practices: { readonly path: string; readonly text: string };
  readonly reviewer: ReviewerLane;
  readonly maxSteps?: number | undefined;
  readonly maxStepChars?: number | undefined;
  readonly maxDiffLines?: number | undefined;
  readonly signal?: AbortSignal | undefined;
}

export interface CycleReviewOutcome {
  readonly request: ReviewRequest;
  readonly index: EvidenceIndex;
  readonly prompt: string;
  /** Exactly what the reviewer returned, before any parsing. */
  readonly raw: string;
  readonly reviewerRef: string;
  readonly producedBy: string;
  readonly result: ParseCycleReviewResult;
}

/**
 * Assemble, ask, and check. Writes nothing.
 *
 * Kept separate from the writing step so a caller can render the prompt, run
 * the reviewer, and inspect the refusals without a directory being touched —
 * and so the tests can prove the refusal path without staging a filesystem.
 */
export const runCycleReview = async (options: CycleReviewOptions): Promise<CycleReviewOutcome> => {
  const { request, index } = assembleReviewRequest(options.jobDir, {
    suite: options.suite,
    lane: options.lane,
    lever: options.lever,
    rows: options.rows,
    practices: options.practices,
    maxSteps: options.maxSteps ?? DEFAULT_MAX_STEPS,
    maxStepChars: options.maxStepChars ?? DEFAULT_MAX_STEP_CHARS,
    maxDiffLines: options.maxDiffLines ?? DEFAULT_MAX_DIFF_LINES,
  });

  const prompt = renderReviewPrompt(request, index);
  const raw = await options.reviewer.ask(prompt, options.signal);
  const producedBy = producedByRef(request, options.reviewer);

  return {
    request,
    index,
    prompt,
    raw,
    reviewerRef: options.reviewer.ref,
    producedBy,
    result: parseCycleReview(raw, index, producedBy),
  };
};

/**
 * The producer stamped into every candidate's lineage.
 *
 * It names the job and the lane together because a candidate outlives the
 * review that emitted it: once it is in a pool, "which run was this reflecting
 * on, and who wrote it" is the question that decides whether it still applies.
 */
export const producedByRef = (request: ReviewRequest, reviewer: ReviewerLane): string =>
  `coder-review:${request.jobDir}:${reviewer.ref}`;

export interface ReviewArtifacts {
  readonly markdownPath: string;
  readonly jsonPath: string;
  readonly markdown: string;
  readonly json: string;
}

/**
 * Write the accepted review as the pair the loop consumes.
 *
 * The markdown is for `docs/coder/reviews/` and for a human deciding what to
 * adopt. The JSON beside it is for the adopt step, which autoimprove §7.5 says
 * should become a diff rather than a reading exercise. They are written
 * together so the directory never holds a review whose proposals cannot be
 * read back mechanically.
 */
export const writeReviewArtifacts = (options: {
  readonly outcome: CycleReviewOutcome;
  readonly markdownPath: string;
  readonly title: string;
  readonly recordedAt: string;
}): ReviewArtifacts => {
  const { outcome, markdownPath, title, recordedAt } = options;
  if (!outcome.result.ok) {
    throw new Error(
      "a refused review is not written. Render the rejections instead; renderRejections says which ref failed and why.",
    );
  }

  const markdown = renderReviewMarkdown({
    request: outcome.request,
    review: outcome.result.review,
    reviewerRef: outcome.reviewerRef,
    recordedAt,
    title,
  });
  const jsonPath = markdownPath.replace(/\.md$/u, ".json");
  const json = `${JSON.stringify(
    {
      schema: CODER_REVIEW_DOCUMENT_SCHEMA,
      recordedAt,
      reviewer: outcome.reviewerRef,
      producedBy: outcome.producedBy,
      request: {
        schema: outcome.request.schema,
        jobDir: outcome.request.jobDir,
        jobId: outcome.request.jobId,
        suite: outcome.request.suite,
        lane: outcome.request.lane,
        lever: { ref: outcome.request.lever.ref, paths: outcome.request.lever.paths },
        trials: outcome.request.trials.map((trial) => ({
          task: trial.task,
          outcome: trial.outcome,
          modelId: trial.modelId,
          agentVersion: trial.agentVersion,
          promptTokens: trial.promptTokens,
          completionTokens: trial.completionTokens,
          cachedInputTokens: trial.cachedInputTokens,
          toolCalls: trial.toolCalls,
          wallClockSeconds: trial.wallClockSeconds,
          truncation: trial.truncation,
        })),
        rows: outcome.request.rows.map((row) => `${row.suite}#${row.recordedAt}`),
        practices: outcome.request.practices.path,
        redaction: outcome.request.redaction,
      },
      review: outcome.result.review,
    },
    null,
    2,
  )}\n`;

  mkdirSync(dirname(markdownPath), { recursive: true });
  writeFileSync(markdownPath, markdown, "utf8");
  writeFileSync(jsonPath, json, "utf8");

  return { markdownPath, jsonPath, markdown, json };
};

/** `docs/coder/reviews/YYYY-MM-DD-<lever-slug>.md`, as the runbook §6 names it. */
export const reviewPathFor = (reviewsDir: string, recordedAt: string, leverSlug: string): string =>
  join(reviewsDir, `${recordedAt.slice(0, 10)}-${slugOf(leverSlug)}.md`);

/** A filename-safe slug. An empty or all-punctuation lever name becomes `cycle`. */
export const slugOf = (value: string): string => {
  const slug = value
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/gu, "-")
    .replaceAll(/^-+|-+$/gu, "");
  return slug === "" ? "cycle" : slug.slice(0, 60);
};
