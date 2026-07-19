import { Exit, Schema } from "effect";

import { GitReviewSourceInputSchema, gitReviewSource } from "../../ide/review-contract.ts";
import { IdeDocumentRefSchema, type IdeReviewSource } from "../../ide/project-contract.ts";
import type { DesktopShellState } from "../shell.ts";

/** Compose existing exact Git, project, and document identities for the view. */
export const activeGitReviewSource = (state: DesktopShellState): IdeReviewSource | null => {
  const identity = state.workspaceBrowser.pathIndexSnapshot?.identity;
  const status = state.git.status;
  const diff = state.git.diff;
  if (identity === undefined || status === null || diff === null) return null;
  const tab = state.workspaceEditor.tabs.find((candidate) => candidate.pathRef === diff.path);
  const decodedDocumentRef = Schema.decodeUnknownExit(IdeDocumentRefSchema)(tab?.documentRef ?? null);
  const decoded = Schema.decodeUnknownExit(GitReviewSourceInputSchema)({
    identity,
    status,
    statusGeneration: state.git.statusGeneration,
    diff,
    fileRef: null,
    documentRef: Exit.isSuccess(decodedDocumentRef) ? decodedDocumentRef.value : null,
  });
  return Exit.isSuccess(decoded) ? gitReviewSource(decoded.value) : null;
};
