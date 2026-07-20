import { Schema as S } from "effect";

import {
  brandedTurnRef,
  MAX_TURN_CONTEXT_CHARS,
  TurnGeneration,
  TurnThreadRef,
  TurnTimestamp,
} from "./turn.js";

/**
 * AFS-00 frozen cross-surface context envelope.
 *
 * This module owns only the cross-surface context envelope and its references.
 * The detailed IDE context and proposal schemas belong to
 * `@openagentsinc/ide-runtime`. The envelope carries references and the
 * deterministic truth flags that Desktop, web, and mobile decoders must agree
 * on. It never carries raw file bytes, raw tool data, or private transcript
 * data.
 *
 * Only a host context service can create an effective manifest. The renderer
 * can request context. It cannot create an authoritative manifest from
 * arbitrary note text.
 */
export const CONTEXT_ENVELOPE_SCHEMA_LITERAL = "openagents.agent_turn_context_envelope.v1" as const;

/** A context manifest reference names one effective host-created manifest. */
export const ContextManifestRef = brandedTurnRef("ContextManifestRef");
export type ContextManifestRef = typeof ContextManifestRef.Type;

export const ProjectRef = brandedTurnRef("ProjectRef");
export type ProjectRef = typeof ProjectRef.Type;

export const WorktreeRef = brandedTurnRef("WorktreeRef");
export type WorktreeRef = typeof WorktreeRef.Type;

/** The kinds of context source a manifest can bind. */
export const ContextSourceKind = S.Literals([
  "active_file",
  "selection",
  "explicit_file",
  "explicit_directory",
  "open_document",
  "diagnostics",
  "language_facts",
  "diff",
  "source_control",
  "task",
  "test",
  "debug",
  "output",
  "local_lexical",
  "local_symbol",
  "semantic_remote",
  "model_summary",
  "released_context_artifact",
]);
export type ContextSourceKind = typeof ContextSourceKind.Type;

/**
 * A single bound context item. A model summary is a derived item with
 * provenance. It never replaces the underlying deterministic facts.
 */
export const ContextItem = S.Struct({
  kind: ContextSourceKind,
  itemRef: brandedTurnRef("ContextItemRef"),
  derived: S.Boolean,
  byteLength: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  truncated: S.Boolean,
  redacted: S.Boolean,
});
export type ContextItem = typeof ContextItem.Type;

/**
 * The cross-surface context envelope. It binds project, root, worktree,
 * document, selection, attachment, generation, source, byte limit, truncation
 * truth, and redaction truth. Its items are references and truth flags, not raw
 * content.
 */
export const WorkContextEnvelope = S.Struct({
  schema: S.Literal(CONTEXT_ENVELOPE_SCHEMA_LITERAL),
  manifestRef: ContextManifestRef,
  threadRef: TurnThreadRef,
  projectRef: S.optionalKey(ProjectRef),
  worktreeRef: S.optionalKey(WorktreeRef),
  generation: TurnGeneration,
  createdAt: TurnTimestamp,
  items: S.Array(ContextItem).check(S.isMaxLength(512)),
  totalByteLength: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  byteLimit: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(MAX_TURN_CONTEXT_CHARS),
  ),
  truncated: S.Boolean,
  redacted: S.Boolean,
});
export type WorkContextEnvelope = typeof WorkContextEnvelope.Type;
