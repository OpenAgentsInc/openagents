import { Schema } from "effect";

import { IdeReviewSourceSchema, type IdeReviewSource } from "./project-contract.ts";

const patch = (path: string, before: string, after: string): string => [
  `diff --git a/${path} b/${path}`,
  `--- a/${path}`,
  `+++ b/${path}`,
  "@@ -1 +1 @@",
  `-${before}`,
  `+${after}`,
  "",
].join("\n");

const endpoint = (label: string, version: string, generation: number, contentPatch: string) => ({
  label,
  versionRef: `ide.review-version.${version}`,
  generation,
  encoding: "utf-8" as const,
  lineEnding: "lf" as const,
  content: {
    _tag: "Available" as const,
    redacted: false,
    bytes: new TextEncoder().encode(contentPatch).byteLength,
  },
});

const common = (name: string, pathRef: string, contentPatch: string) => ({
  schemaVersion: "openagents.desktop.ide-review-source.v1" as const,
  reviewRef: `ide.review.fixture-${name}`,
  projectRef: "ide.project.fixture",
  rootRef: "ide.root.fixture",
  worktreeRef: "ide.worktree.fixture",
  fileRef: `ide.file.fixture-${name}`,
  documentRef: `ide.document.fixture-${name}`,
  pathRef,
  scope: "single_file" as const,
  patch: contentPatch,
  language: "typescript",
  lifecycle: { _tag: "Ready" as const },
});

/** Offline fixture corpus used by adapter, accessibility, and packaged proofs. */
export const ideReviewSourceFixtures = (): ReadonlyArray<IdeReviewSource> => {
  const gitHeadIndexPatch = patch("src/staged.ts", "const staged = false", "const staged = true");
  const gitIndexWorktreePatch = patch("src/worktree.ts", "const draft = 1", "const draft = 2");
  const gitHeadWorktreePatch = patch("src/aggregate.ts", "const aggregate = 1", "const aggregate = 3");
  const savedDraftPatch = patch("src/draft.ts", "const title = 'saved'", "const title = 'draft'");
  const conflictPatch = patch("src/conflict.ts", "const owner = 'draft'", "const owner = 'disk'");
  const checkpointPatch = patch("src/checkpoint.ts", "const checkpoint = 4", "const checkpoint = 5");
  const proposalPatch = patch("src/proposal.ts", "const accepted = false", "const accepted = true");
  const candidatePatch = patch("src/candidate.ts", "const candidate = 'A'", "const candidate = 'B'");
  const values: ReadonlyArray<unknown> = [
    {
      _tag: "GitHeadIndex",
      ...common("git-head-index", "src/staged.ts", gitHeadIndexPatch),
      origin: "git",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "refresh"],
      base: endpoint("HEAD", "fixture-head", 7, gitHeadIndexPatch),
      target: endpoint("Index (staged)", "fixture-index", 7, gitHeadIndexPatch),
      gitSnapshotRef: "ide.git-snapshot.fixture-staged",
      headRef: "ide.commit.fixture-head",
      indexRef: "ide.review-version.fixture-index",
      gitSnapshotGeneration: 7,
    },
    {
      _tag: "GitIndexWorktree",
      ...common("git-index-worktree", "src/worktree.ts", gitIndexWorktreePatch),
      origin: "git",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "refresh"],
      base: endpoint("Index", "fixture-index", 7, gitIndexWorktreePatch),
      target: endpoint("Working tree", "fixture-worktree", 7, gitIndexWorktreePatch),
      gitSnapshotRef: "ide.git-snapshot.fixture-worktree",
      indexRef: "ide.review-version.fixture-index",
      worktreeStateRef: "ide.review-version.fixture-worktree",
      gitSnapshotGeneration: 7,
    },
    {
      _tag: "GitHeadWorktree",
      ...common("git-head-worktree", "src/aggregate.ts", gitHeadWorktreePatch),
      scope: "aggregate",
      fileRef: null,
      documentRef: null,
      pathRef: null,
      origin: "git",
      allowedActions: ["select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "refresh"],
      base: endpoint("HEAD", "fixture-head", 7, gitHeadWorktreePatch),
      target: endpoint("Working tree aggregate", "fixture-head-worktree", 7, gitHeadWorktreePatch),
      gitSnapshotRef: "ide.git-snapshot.fixture-aggregate",
      headRef: "ide.commit.fixture-head",
      worktreeStateRef: "ide.review-version.fixture-head-worktree",
      gitSnapshotGeneration: 7,
    },
    {
      _tag: "SavedDraft",
      ...common("saved-draft", "src/draft.ts", savedDraftPatch),
      origin: "editor",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "reject", "undo"],
      base: endpoint("Saved disk revision", "fixture-disk-saved", 11, savedDraftPatch),
      target: endpoint("Unsaved draft", "fixture-draft", 12, savedDraftPatch),
      diskRevisionRef: "ide.disk-revision.fixture-saved",
      documentGeneration: 12,
    },
    {
      _tag: "DraftExternalConflict",
      ...common("draft-conflict", "src/conflict.ts", conflictPatch),
      origin: "external_change",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "accept", "reject", "undo"],
      base: endpoint("Current draft", "fixture-conflict-draft", 12, conflictPatch),
      target: endpoint("Externally changed disk", "fixture-conflict-disk", 13, conflictPatch),
      expectedDiskRevisionRef: "ide.disk-revision.fixture-expected",
      actualDiskRevisionRef: "ide.disk-revision.fixture-actual",
      draftDocumentGeneration: 12,
    },
    {
      _tag: "CheckpointCurrent",
      ...common("checkpoint-current", "src/checkpoint.ts", checkpointPatch),
      origin: "checkpoint",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "apply", "undo"],
      base: endpoint("Checkpoint", "fixture-checkpoint", 3, checkpointPatch),
      target: endpoint("Current document", "fixture-checkpoint-current", 14, checkpointPatch),
      checkpointRef: "ide.checkpoint.fixture-one",
      attachmentGeneration: 3,
      currentDocumentGeneration: 14,
    },
    {
      _tag: "AgentProposal",
      ...common("agent-proposal", "src/proposal.ts", proposalPatch),
      origin: "agent",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context", "accept", "reject", "apply", "undo"],
      base: endpoint("Exact proposal base", "fixture-proposal-base", 15, proposalPatch),
      target: endpoint("Agent proposal", "fixture-proposal", 1, proposalPatch),
      proposalRef: "ide.proposal.fixture-one",
      attachmentGeneration: 3,
      proposalBaseDocumentGeneration: 15,
      currentDocumentGeneration: 15,
    },
    {
      _tag: "CandidateComparison",
      ...common("candidate-comparison", "src/candidate.ts", candidatePatch),
      origin: "comparison",
      allowedActions: ["open", "reveal", "select", "expand_context", "collapse_context", "change_layout", "copy", "add_context"],
      base: endpoint("Candidate A", "fixture-candidate-a", 2, candidatePatch),
      target: endpoint("Candidate B", "fixture-candidate-b", 4, candidatePatch),
      candidateARef: "ide.candidate.fixture-a",
      candidateBRef: "ide.candidate.fixture-b",
      candidateAGeneration: 2,
      candidateBGeneration: 4,
    },
  ];
  return values.map((value) => Schema.decodeUnknownSync(IdeReviewSourceSchema)(value));
};
