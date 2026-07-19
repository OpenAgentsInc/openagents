import { Schema } from "effect";

import {
  GitDiffResultSchema,
  GitStatusResultSchema,
} from "../git-github-contract.ts";
import { IdePathIndexIdentitySchema } from "./path-index-contract.ts";
import {
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdeCheckpointRefSchema,
  IdeProjectRefSchema,
  IdeProposalRefSchema,
  IdeReviewActionSchema,
  IdeReviewRefSchema,
  IdeReviewSourceSchema,
  IdeReviewVersionRefSchema,
  type IdeDocumentRef,
  type IdeFileRef,
  type IdeReviewAction,
  type IdeReviewSource,
} from "./project-contract.ts";

export const IdeReviewSelectionSchema = Schema.Struct({
  startLine: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  startSide: Schema.Literals(["base", "target"]),
  endLine: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  endSide: Schema.Literals(["base", "target"]),
}).annotate({ identifier: "IdeReviewSelection" });
export type IdeReviewSelection = typeof IdeReviewSelectionSchema.Type;

export const IdeReviewBindingSchema = Schema.Struct({
  baseVersionRef: IdeReviewVersionRefSchema,
  baseGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  targetVersionRef: IdeReviewVersionRefSchema,
  targetGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
}).annotate({ identifier: "IdeReviewBinding" });
export type IdeReviewBinding = typeof IdeReviewBindingSchema.Type;

/** Bounded callback data from rendering mechanics; contains no mutation authority. */
export const IdeReviewIntentSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-review-intent.v1"),
  reviewRef: IdeReviewRefSchema,
  action: IdeReviewActionSchema,
  binding: IdeReviewBindingSchema,
  selection: Schema.NullOr(IdeReviewSelectionSchema),
  layout: Schema.Literals(["unified", "split"]),
  contextLines: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(1),
    Schema.isLessThanOrEqualTo(100),
  ),
}).annotate({ identifier: "IdeReviewIntent" });
export type IdeReviewIntent = typeof IdeReviewIntentSchema.Type;

const reviewCommandFields = {
  reviewRef: IdeReviewRefSchema,
  action: IdeReviewActionSchema,
  binding: IdeReviewBindingSchema,
};

/** Canonical command destination; the Pierre callback can never construct it. */
export const IdeReviewCommandSchema = Schema.TaggedUnion({
  Projection: reviewCommandFields,
  DocumentMutation: {
    ...reviewCommandFields,
    documentRef: IdeDocumentRefSchema,
  },
  WorkspaceMutation: {
    ...reviewCommandFields,
    projectRef: IdeProjectRefSchema,
  },
  CheckpointMutation: {
    ...reviewCommandFields,
    checkpointRef: IdeCheckpointRefSchema,
  },
  ProposalMutation: {
    ...reviewCommandFields,
    proposalRef: IdeProposalRefSchema,
  },
}).annotate({ identifier: "IdeReviewCommand" });
export type IdeReviewCommand = typeof IdeReviewCommandSchema.Type;

export const IdeReviewActionDispositionSchema = Schema.TaggedUnion({
  Dispatch: {
    reviewRef: IdeReviewRefSchema,
    action: IdeReviewActionSchema,
    authority: Schema.Literals(["projection", "document", "workspace", "checkpoint", "proposal"]),
    binding: IdeReviewBindingSchema,
    command: IdeReviewCommandSchema,
  },
  Refused: {
    reviewRef: IdeReviewRefSchema,
    action: IdeReviewActionSchema,
    reason: Schema.Literals([
      "not_allowed",
      "source_stale",
      "source_unavailable",
      "base_generation_replaced",
      "target_generation_replaced",
      "no_canonical_mutation_authority",
    ]),
  },
}).annotate({ identifier: "IdeReviewActionDisposition" });
export type IdeReviewActionDisposition = typeof IdeReviewActionDispositionSchema.Type;

export const reviewBinding = (source: IdeReviewSource): IdeReviewBinding =>
  IdeReviewBindingSchema.make({
    baseVersionRef: source.base.versionRef,
    baseGeneration: source.base.generation,
    targetVersionRef: source.target.versionRef,
    targetGeneration: source.target.generation,
  });

const mutatingActions: ReadonlySet<IdeReviewAction> = new Set([
  "accept",
  "reject",
  "apply",
  "undo",
]);

const mutationAuthority = (
  source: IdeReviewSource,
): "document" | "workspace" | "checkpoint" | "proposal" | null => {
  switch (source._tag) {
    case "SavedDraft":
    case "DraftExternalConflict":
      return "document";
    case "CheckpointCurrent":
      return source.scope === "aggregate" ? "workspace" : "checkpoint";
    case "AgentProposal":
      return "proposal";
    case "GitHeadIndex":
    case "GitIndexWorktree":
    case "GitHeadWorktree":
    case "CandidateComparison":
      return null;
  }
};

const projectionCommand = (
  source: IdeReviewSource,
  action: IdeReviewAction,
): IdeReviewCommand => IdeReviewCommandSchema.cases.Projection.make({
  reviewRef: source.reviewRef,
  action,
  binding: reviewBinding(source),
});

const mutationCommand = (
  source: IdeReviewSource,
  action: IdeReviewAction,
): IdeReviewCommand | null => {
  const fields = { reviewRef: source.reviewRef, action, binding: reviewBinding(source) };
  switch (source._tag) {
    case "SavedDraft":
    case "DraftExternalConflict":
      return source.documentRef === null
        ? null
        : IdeReviewCommandSchema.cases.DocumentMutation.make({
            ...fields,
            documentRef: source.documentRef,
          });
    case "CheckpointCurrent":
      return source.scope === "aggregate"
        ? IdeReviewCommandSchema.cases.WorkspaceMutation.make({
            ...fields,
            projectRef: source.projectRef,
          })
        : IdeReviewCommandSchema.cases.CheckpointMutation.make({
            ...fields,
            checkpointRef: source.checkpointRef,
          });
    case "AgentProposal":
      return IdeReviewCommandSchema.cases.ProposalMutation.make({
        ...fields,
        proposalRef: source.proposalRef,
      });
    case "GitHeadIndex":
    case "GitIndexWorktree":
    case "GitHeadWorktree":
    case "CandidateComparison":
      return null;
  }
};

/**
 * Converts bounded renderer intent into a canonical-route decision. This does
 * not perform mutation. Every mutating action must present the exact source
 * binding again; a line number or current widget selection is never enough.
 */
export const reviewActionDisposition = (
  source: IdeReviewSource,
  intent: IdeReviewIntent,
): IdeReviewActionDisposition => {
  const refused = (
    reason: Extract<IdeReviewActionDisposition, { _tag: "Refused" }>["reason"],
  ): IdeReviewActionDisposition =>
    IdeReviewActionDispositionSchema.cases.Refused.make({
      reviewRef: source.reviewRef,
      action: intent.action,
      reason,
    });

  if (intent.reviewRef !== source.reviewRef || !source.allowedActions.includes(intent.action)) {
    return refused("not_allowed");
  }
  if (source.lifecycle._tag === "Unavailable") return refused("source_unavailable");
  if (source.lifecycle._tag === "Stale") {
    return intent.action === "refresh" && source.lifecycle.refreshable
      ? IdeReviewActionDispositionSchema.cases.Dispatch.make({
          reviewRef: source.reviewRef,
          action: intent.action,
          authority: "projection",
          binding: reviewBinding(source),
          command: projectionCommand(source, intent.action),
        })
      : refused("source_stale");
  }

  if (!mutatingActions.has(intent.action)) {
    return IdeReviewActionDispositionSchema.cases.Dispatch.make({
      reviewRef: source.reviewRef,
      action: intent.action,
      authority: "projection",
      binding: reviewBinding(source),
      command: projectionCommand(source, intent.action),
    });
  }

  if (
    intent.binding.baseVersionRef !== source.base.versionRef ||
    intent.binding.baseGeneration !== source.base.generation
  ) {
    return refused("base_generation_replaced");
  }
  if (
    intent.binding.targetVersionRef !== source.target.versionRef ||
    intent.binding.targetGeneration !== source.target.generation
  ) {
    return refused("target_generation_replaced");
  }
  const authority = mutationAuthority(source);
  const command = mutationCommand(source, intent.action);
  if (authority === null || command === null) return refused("no_canonical_mutation_authority");
  return IdeReviewActionDispositionSchema.cases.Dispatch.make({
    reviewRef: source.reviewRef,
    action: intent.action,
    authority,
    binding: reviewBinding(source),
    command,
  });
};

const fnv1a = (value: string): string => {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(36).padStart(7, "0");
};

const opaquePart = (namespace: string, value: string): string =>
  `${namespace}-${fnv1a(value)}-${fnv1a(`${value.length}:${value}`)}`;

const languageForPath = (pathRef: string): string | null => {
  const extension = pathRef.split(".").at(-1)?.toLocaleLowerCase();
  const languages: Readonly<Record<string, string>> = {
    c: "c",
    cpp: "cpp",
    css: "css",
    go: "go",
    html: "html",
    java: "java",
    js: "javascript",
    json: "json",
    jsx: "javascript",
    md: "markdown",
    py: "python",
    rs: "rust",
    ts: "typescript",
    tsx: "typescript",
    yaml: "yaml",
    yml: "yaml",
  };
  return extension === undefined ? null : languages[extension] ?? null;
};

const availableContent = (patch: string) => ({
  _tag: "Available" as const,
  redacted: false,
  bytes: new TextEncoder().encode(patch).byteLength,
});

const unavailableContent = {
  _tag: "Unavailable" as const,
  reason: "generation_replaced" as const,
};

export const GitReviewSourceInputSchema = Schema.Struct({
  identity: IdePathIndexIdentitySchema,
  status: GitStatusResultSchema,
  statusGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  diff: GitDiffResultSchema,
  fileRef: Schema.NullOr(IdeFileRefSchema),
  documentRef: Schema.NullOr(IdeDocumentRefSchema),
}).annotate({ identifier: "GitReviewSourceInput" });
export type GitReviewSourceInput = typeof GitReviewSourceInputSchema.Type;

/**
 * Builds the production read-only Git source from exact host status/diff
 * fences. Opaque UI refs are deterministic but deliberately non-authoritative.
 */
export const gitReviewSource = (input: GitReviewSourceInput): IdeReviewSource => {
  const sourceKey = `${input.status.repositoryRef}:${input.status.statusRef}:${input.diff.path}`;
  const refPart = opaquePart("git", sourceKey);
  const exact =
    input.status.repositoryRef === input.diff.repositoryRef &&
    input.status.statusRef === input.diff.statusRef;
  const patch = exact ? input.diff.content : null;
  const content = patch === null ? unavailableContent : availableContent(patch);
  const generation = Math.max(1, Math.trunc(input.statusGeneration));
  const common = {
    schemaVersion: "openagents.desktop.ide-review-source.v1" as const,
    reviewRef: `ide.review.${refPart}`,
    projectRef: input.identity.projectRef,
    rootRef: input.identity.rootRef,
    worktreeRef: input.identity.worktreeRef,
    fileRef: input.fileRef ?? (`ide.file.${refPart}` as IdeFileRef),
    documentRef: input.documentRef ?? null,
    pathRef: input.diff.path,
    scope: "single_file" as const,
    patch,
    language: languageForPath(input.diff.path),
    origin: "git" as const,
    allowedActions: [
      "open",
      "reveal",
      "select",
      "expand_context",
      "collapse_context",
      "change_layout",
      "copy",
      "add_context",
      "refresh",
    ] as const,
    lifecycle: exact
      ? ({ _tag: "Ready" } as const)
      : ({ _tag: "Stale", reason: "git_snapshot_replaced", refreshable: true } as const),
  };
  const headVersionRef = `ide.review-version.head-${opaquePart("v", input.status.headRef ?? input.status.statusRef)}`;
  const indexVersionRef = `ide.review-version.index-${opaquePart("v", input.status.statusRef)}`;
  const worktreeVersionRef = `ide.review-version.worktree-${opaquePart("v", input.status.statusRef)}`;
  const gitSnapshotRef = `ide.git-snapshot.${opaquePart("snapshot", `${input.status.repositoryRef}:${input.status.statusRef}`)}`;
  const headRef = input.status.headRef === null
    ? null
    : `ide.commit.${opaquePart("head", input.status.headRef)}`;

  const source = input.diff.source === "staged"
    ? {
        _tag: "GitHeadIndex" as const,
        ...common,
        base: {
          label: "HEAD",
          versionRef: headVersionRef,
          generation,
          encoding: "utf-8" as const,
          lineEnding: "unknown" as const,
          content,
        },
        target: {
          label: "Index (staged)",
          versionRef: indexVersionRef,
          generation,
          encoding: "utf-8" as const,
          lineEnding: "unknown" as const,
          content,
        },
        gitSnapshotRef,
        headRef,
        indexRef: indexVersionRef,
        gitSnapshotGeneration: generation,
      }
    : {
        _tag: "GitIndexWorktree" as const,
        ...common,
        base: {
          label: "Index",
          versionRef: indexVersionRef,
          generation,
          encoding: "utf-8" as const,
          lineEnding: "unknown" as const,
          content,
        },
        target: {
          label: "Working tree",
          versionRef: worktreeVersionRef,
          generation,
          encoding: "utf-8" as const,
          lineEnding: "unknown" as const,
          content,
        },
        gitSnapshotRef,
        indexRef: indexVersionRef,
        worktreeStateRef: worktreeVersionRef,
        gitSnapshotGeneration: generation,
      };

  return Schema.decodeUnknownSync(IdeReviewSourceSchema)(source);
};

export const ideReviewIntent = (
  source: IdeReviewSource,
  action: IdeReviewAction,
  options: Readonly<{
    selection?: IdeReviewSelection | null;
    layout?: "unified" | "split";
    contextLines?: number;
  }> = {},
): IdeReviewIntent =>
  IdeReviewIntentSchema.make({
    schemaVersion: "openagents.desktop.ide-review-intent.v1",
    reviewRef: source.reviewRef,
    action,
    binding: reviewBinding(source),
    selection: options.selection ?? null,
    layout: options.layout ?? "unified",
    contextLines: Math.max(1, Math.min(100, Math.trunc(options.contextLines ?? 20))),
  });

const hunkHeaderPattern = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/u;

/** Selects only the user-chosen base/target line range from a bounded patch. */
export const boundedSelectedReviewPatch = (
  patch: string,
  selection: IdeReviewSelection | null,
): string => {
  if (selection === null) return patch.slice(0, 120_000);
  const start = Math.min(selection.startLine, selection.endLine);
  const end = Math.max(selection.startLine, selection.endLine);
  const sameSide = selection.startSide === selection.endSide;
  const matches = (side: "base" | "target", line: number): boolean => {
    if (sameSide) return side === selection.startSide && line >= start && line <= end;
    return side === selection.startSide ? line >= selection.startLine : side === selection.endSide && line <= selection.endLine;
  };
  const headers = patch.split("\n").filter((line) =>
    line.startsWith("diff --git ") || line.startsWith("--- ") || line.startsWith("+++ "),
  );
  const chosen: string[] = [];
  let oldLine = 0;
  let newLine = 0;
  let currentHunk = "";
  let emittedHunk = "";
  for (const line of patch.split("\n")) {
    const header = hunkHeaderPattern.exec(line);
    if (header !== null) {
      oldLine = Number(header[1]);
      newLine = Number(header[2]);
      currentHunk = line;
      continue;
    }
    if (currentHunk === "" || line.startsWith("\\ No newline")) continue;
    const deletion = line.startsWith("-") && !line.startsWith("---");
    const addition = line.startsWith("+") && !line.startsWith("+++");
    const include = deletion
      ? matches("base", oldLine)
      : addition
        ? matches("target", newLine)
        : matches("base", oldLine) || matches("target", newLine);
    if (include) {
      if (emittedHunk !== currentHunk) {
        chosen.push(currentHunk);
        emittedHunk = currentHunk;
      }
      chosen.push(line);
    }
    if (!addition) oldLine += 1;
    if (!deletion) newLine += 1;
  }
  return [...headers, ...chosen, ""].join("\n").slice(0, 120_000);
};
