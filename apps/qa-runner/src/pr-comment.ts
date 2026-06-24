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

import type { EvalResult } from "./evals";
import { renderEvalMarkdown } from "./evals-report";
import {
  ghAttachVariantVideos,
  type GhAttachOptions,
  type GhAttachRunner,
} from "./gh-attach";

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
}

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

  // Prepend the stable marker so the CI loop can upsert this comment.
  return `${PR_COMMENT_MARKER}\n${body}\n`;
};
