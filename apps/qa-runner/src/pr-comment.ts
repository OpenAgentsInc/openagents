// PR-evidence comment composer (#6185).
//
// Composes the PR comment body the CI loop posts: a pass/fail comparison table,
// the per-variant video (gh-attach'd embed when available, else the in-eval
// relative ref), and the shareable `/pro/evals/<id>` link. The body is composed
// PURELY from an already-public-safe `EvalResult` via `renderEvalMarkdown`, so
// it is unit-testable and dry-run-provable with NO network.
//
// HONEST: real-model runs are flag-gated + capped upstream (the CI workflow);
// this composer reports whatever the eval actually produced — a failing variant
// shows as failing (no fake green).
//
// #6192: when the run declared COMMITMENTS, the composer surfaces the verify
// investigator VERDICT (CONFIRMED/REFUTED/INCONCLUSIVE) at the TOP of the
// comment with the contradicting/observed evidence inline — so a reviewer sees
// the verdict before the table. A REFUTED verdict is shown as REFUTED (never a
// fake pass), satisfying "verify the output before you post."

import type { EvalResult } from "./evals";
import { renderEvalMarkdown } from "./evals-report";
import {
  ghAttachVariantVideos,
  type GhAttachOptions,
  type GhAttachRunner,
} from "./gh-attach";
import {
  renderVerdictEvidence,
  renderVerdictLine,
  type VerifyReport,
} from "./verify";

export interface ComposePrCommentInput {
  readonly result: EvalResult;
  /** Base URL for the shareable /pro link. */
  readonly proBaseUrl: string;
  /** Absolute/relative path on disk to each variant's video, for gh-attach. */
  readonly variantVideoPaths?: ReadonlyArray<{
    variantId: string;
    filePath: string;
  }>;
  /** gh-attach runner; when omitted, no upload is attempted (relative refs). */
  readonly ghAttach?: GhAttachRunner;
  readonly ghAttachOptions?: GhAttachOptions;
  /**
   * Optional verify-stage report (#6192). When present, its verdict line +
   * per-commitment evidence are rendered at the top of the comment. Honest: a
   * REFUTED verdict is shown as REFUTED, with the contradicting evidence inline.
   */
  readonly verify?: VerifyReport;
}

// Render the verify verdict block: a headline verdict line + a collapsed
// evidence list. Pure; returns "" when there is no report.
const renderVerifyBlock = (verify: VerifyReport | undefined): string => {
  if (verify === undefined) return "";
  const lines: string[] = [renderVerdictLine(verify), ""];
  const evidence = renderVerdictEvidence(verify);
  if (evidence.length > 0) {
    lines.push("<details><summary>Commitment evidence</summary>", "");
    lines.push(...evidence);
    lines.push("", "</details>", "");
  }
  return `${lines.join("\n")}\n`;
};

// A stable marker so the CI loop can find-and-update its own prior comment
// instead of stacking duplicates (the workflow uses this with `gh pr comment`).
export const PR_COMMENT_MARKER = "<!-- openagents-chill-eval -->";

export const composePrComment = async (
  input: ComposePrCommentInput,
): Promise<string> => {
  let variantVideoMarkdown: Record<string, string> | undefined;
  if (
    input.ghAttach !== undefined &&
    input.variantVideoPaths !== undefined &&
    input.variantVideoPaths.length > 0
  ) {
    const uploaded = await ghAttachVariantVideos(
      input.ghAttach,
      input.variantVideoPaths,
      input.ghAttachOptions ?? {},
    );
    if (Object.keys(uploaded).length > 0) variantVideoMarkdown = uploaded;
  }

  const body = renderEvalMarkdown(input.result, {
    proBaseUrl: input.proBaseUrl,
    ...(variantVideoMarkdown !== undefined ? { variantVideoMarkdown } : {}),
  });

  // The verify verdict (when present) leads the comment — verdict first, then
  // the comparison. Prepend the stable marker so the CI loop can upsert this.
  const verifyBlock = renderVerifyBlock(input.verify);
  return `${PR_COMMENT_MARKER}\n${verifyBlock}${body}\n`;
};
