# Turn Diff Tracker

File: `codex-rs/core/src/turn_diff_tracker.rs`

Tracks aggregated diffs across all file changes within a single turn so the UI
can show a unified patch.

## Design

- Snapshots baselines lazily when a path is first touched during the turn.
- Keeps a stable internal uuid per external path to track renames.
- Computes diffs entirely in memory using the `similar` crate with git‑style
  headers and mode changes (symlink/exec bits on Unix).
- Computes blob OIDs using `git hash-object` when possible for determinism,
  falling back to content hashing.
- Renders paths relative to the repo root (when found) for readable diffs.

## Output

- `get_unified_diff()` sorts files lexicographically by repo‑relative path and
  concatenates diffs with a trailing newline.

## Edge cases

- Missing files are treated as `/dev/null` on the left.
- Symlink diffs encode the link target as the blob.
- Windows roots are handled to avoid walking past the drive root.

