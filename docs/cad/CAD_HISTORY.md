# CAD History Stack

Undo/redo core is implemented in `crates/cad/src/history.rs`.

## Command Granularity Rules

- Single parameter edit is a single step.
- Grouped gestures are coalesced when:
  - command type is `SetParameter`
  - parameter name matches
  - `gesture_id` matches and is non-empty

## Stack Policy

- Session-scoped (`session_id` on stack).
- Capacity-bounded by `max_steps`.
- Overflow evicts oldest undo entries.
- Pushing new transitions clears redo branch.
- `reset_session(...)` clears undo/redo stacks safely.

## Snapshot Invariants

Each history entry stores `before` + `after` snapshots including:

- geometry hash
- stable IDs / semantic refs
- warning set
- analysis snapshot

Replay tests verify forward/backward transitions preserve these payloads.

## Verification

- `cargo test -p openagents-cad`
