# Diff And Patch Review UI Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #24 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should present file changes, structured diffs,
patch previews, approval or rejection flows, comments, and edit receipts.

## Target

Build a diff review system that turns every proposed or completed file change
into a structured review artifact.

The UI should be useful in a terminal, but the domain model should not depend
on a terminal renderer. Diffs should be typed data with file groups, hunks,
line ranges, edit intent, safety metadata, and receipt status.

## User-Visible Capability

The user should be able to:

- See changed files grouped by operation.
- Inspect additions, removals, replacements, and new files.
- Review structured hunks with stable line numbers.
- Expand or collapse large file previews.
- See syntax-aware highlighting when available.
- Fall back to plain text when highlighting is unavailable.
- Approve, reject, or comment on proposed edits.
- Understand whether a preview is pending, applied, rejected, or superseded.
- See counts of added and removed lines.
- Link each accepted edit to a durable receipt.

The review surface should stay readable in narrow terminals and avoid layout
shift while content streams.

## Diff Domain Model

Represent diffs as typed data:

- Patch id.
- Workspace ref.
- File path ref.
- Operation: add, modify, delete, rename, mode change, binary change.
- Old and new file identity.
- Hunks with old and new ranges.
- Lines with kind: context, addition, removal, metadata.
- Syntax language hint.
- Truncation metadata.
- Renderer capability requirements.
- Approval state.
- Applied receipt ref.
- Verification refs.

The domain object should preserve enough information to render split, unified,
compact, and machine-readable views without recomputing from raw text.

## Core Design

Define a `PatchReviewService` that owns diff normalization, preview creation,
approval flow, application handoff, and receipts.

Suggested service boundary:

```ts
interface PatchReviewService {
  normalize(request: PatchNormalizeRequest): Effect.Effect<StructuredPatch, PatchReviewError>
  preview(request: PatchPreviewRequest): Effect.Effect<PatchPreview, PatchReviewError>
  decide(request: PatchDecisionRequest): Effect.Effect<PatchDecisionReceipt, PatchReviewError>
  comment(request: PatchCommentRequest): Effect.Effect<PatchCommentReceipt, PatchReviewError>
  receipt(request: PatchReceiptRequest): Effect.Effect<PatchReceipt, PatchReviewError>
}
```

The file-editing service should produce patch candidates. The review service
should never directly invent file changes without an explicit patch candidate
and workspace authority.

## Terminal Rendering

The terminal renderer should support:

- Unified hunk view.
- Optional split view where width allows.
- Stable gutters for old and new line numbers.
- Dimmed context lines.
- Colored additions and removals.
- Word-level highlighting when available.
- Width-aware wrapping.
- Binary or very large file placeholders.
- Condensed summaries for long transcripts.
- Full review panel for explicit inspection.

Rendering should tolerate terminal resize. Line-number gutters and content
columns should have independent width constraints so long code lines do not
break the whole layout.

## Approval Flow

Patch review should distinguish:

- Previewed but not applied.
- Applied by an approved edit.
- Auto-applied under an explicit policy.
- Rejected by the user.
- Superseded by a later edit.
- Failed during application.
- Reverted by a later operation.

Approval records should include who or what approved the edit, policy refs,
time, target workspace, patch id, and resulting artifact refs.

## Review Comments

Comments should attach to:

- Whole patch.
- File.
- Hunk.
- Specific old or new line.

Comments should be durable enough for the agent loop to act on them, but they
should not be confused with applied code. A comment is review input, not a file
edit receipt.

## Receipts

Every applied edit should produce a receipt with:

- Patch id.
- File refs.
- Hash or mtime evidence where practical.
- Added and removed line counts.
- Whether the file was created, changed, deleted, or left unchanged.
- Verification commands run after the edit.
- Public-safe summary.
- Private local evidence refs when needed.

Receipts should be safe to include in final user summaries without leaking raw
private content by default.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for patch normalization, review decisions, and receipts.
- `Schema` for patches, hunks, lines, comments, decisions, and receipts.
- `Layer` for renderer, syntax highlighter, filesystem, and git providers.
- `Stream` for progressive diff rendering.
- `Queue` for review decisions and comments.
- `Ref` for expanded files, selected hunk, and UI review state.
- `Scope` for temporary patch files and renderer cleanup.

The patch model should be shared by terminal, web, and headless flows. Only the
presentation layer should be terminal-specific.

## Safety Rules

- Do not apply a patch outside the active workspace boundary.
- Do not auto-approve destructive edits without explicit policy.
- Do not hide rejected or failed edits from the transcript.
- Do not render raw binary data in the terminal.
- Do not treat a preview as proof that the file changed.
- Do not let syntax highlighting failure block review.
- Do not lose line-number mapping when wrapping or truncating.
- Do not include secret-bearing file contents in public-safe receipts.
- Do not let comments mutate files unless converted into an explicit edit.

## Tests

Minimum regression coverage:

- Normalize add, modify, delete, rename, and binary patches.
- Render narrow and wide terminal diff views.
- Preserve old and new line numbers through wrapping.
- Show added and removed line counts.
- Fall back cleanly when highlighting is unavailable.
- Truncate very large diffs with explicit continuation metadata.
- Approve and apply a patch with receipt generation.
- Reject a patch and preserve rejection state.
- Supersede an old patch with a later edit.
- Attach comments to patch, file, hunk, and line scopes.
- Keep public-safe receipts free of raw private content.
- Restore review UI state after terminal resize.

## OpenAgents Translation Notes

When promoted, map patch reviews to OpenAgents artifact refs, approval refs,
workspace refs, operator review surfaces, and public-safe closeout receipts.
Verify live issue state before claiming patch review, approval, or receipt
behavior is implemented.

## Decision

Diff review should be a structured artifact pipeline, not a terminal-only text
dump. The agent should separate patch candidates, review decisions, applied
edits, comments, and receipts so future web, terminal, and headless surfaces
can all reason about the same change.
