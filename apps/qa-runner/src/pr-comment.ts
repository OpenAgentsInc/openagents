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
import type { FailureSuggestion } from "./failure-learning";
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
  /**
   * Optional failure-learning suggestion (#6195). When a failed/REFUTED run (or
   * a low eval) produced a captured failure pattern, this renders the captured
   * pattern + the copy-paste fix/scenario-update snippet as a section. Honest: it
   * exists BECAUSE the run did not pass; a downgraded/inert mutating strategy is
   * stated plainly (no silent repo mutation, no fake green).
   */
  readonly failureSuggestion?: FailureSuggestion;
}

// Render the failure-learning suggestion block (#6195): the captured pattern +
// the copy-paste fix snippet, plus the honest strategy line. Pure; returns ""
// when there is no suggestion (a clean pass produces none).
const renderFailureLearningBlock = (
  suggestion: FailureSuggestion | undefined,
): string => {
  if (suggestion === undefined) return "";
  const lines: string[] = [];
  lines.push("### Failure learning (#6195)");
  lines.push("");
  // The honest strategy line: which strategy ran, and whether a mutating
  // strategy was downgraded (default suggest-only) or is plan-only (armed).
  const r = suggestion.resolved;
  if ("downgradedFrom" in r) {
    lines.push(`> Strategy: \`suggest_in_report\` (downgraded from \`${r.downgradedFrom}\` — ${r.downgradeReason}).`);
  } else if (r.strategy === "auto_commit" || r.strategy === "open_pr") {
    lines.push(`> Strategy: \`${r.strategy}\` (PLAN ONLY — no repo mutation is executed here; the executor is owner-gated).`);
  } else {
    lines.push("> Strategy: `suggest_in_report` (default — manual review).");
  }
  lines.push("");
  lines.push("<details><summary>Captured failure pattern + suggested fix</summary>");
  lines.push("");
  lines.push(suggestion.snippet);
  if (suggestion.mutationPlan !== undefined) {
    lines.push("");
    lines.push(`Planned mutation (\`${suggestion.mutationPlan.kind}\`, executed=${suggestion.mutationPlan.executed}):`);
    lines.push(`- ${suggestion.mutationPlan.description}`);
  }
  lines.push("");
  lines.push("</details>");
  lines.push("");
  return `${lines.join("\n")}\n`;
};

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
  // the comparison, then (when a failure was captured) the failure-learning
  // suggestion. Prepend the stable marker so the CI loop can upsert this.
  const verifyBlock = renderVerifyBlock(input.verify);
  const failureBlock = renderFailureLearningBlock(input.failureSuggestion);
  return `${PR_COMMENT_MARKER}\n${verifyBlock}${body}\n${failureBlock}`;
};
