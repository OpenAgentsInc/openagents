# Apply Patch System

Crates:
- `codex-rs/apply-patch` — grammar and CLI for safe file edits.
- `codex-rs/core/src/tool_apply_patch.rs` — tool definition and docs.
- `codex-rs/core/src/openai_tools.rs` — freeform/function tool selection.

## Grammar

A restricted, file‑oriented diff envelope:

```
*** Begin Patch
*** Add File: path
+...
*** Update File: path
@@ header
- old
+ new
*** Delete File: path
*** End Patch
```

- Always declares intent (`Add/Update/Delete`) and paths are relative.
- Hunks include 3 lines of context by default; multiple `@@` blocks supported.
- Safe to parse and validate before touching the filesystem.

## Freeform vs function

- Freeform: model outputs patch directly in the grammar (better for GPT‑5).
- Function: a single `input` string inside a tool for models that benefit from
  structured calls (e.g., GPT‑OSS).

## Integration with diffs

- `TurnDiffTracker::on_patch_begin` snapshots baselines before applying.
- Unified diff is computed in memory and attached at the end of the turn.

