# Retained-file viewer verification

Issue [#9594](https://github.com/OpenAgentsInc/openagents/issues/9594) adds file
inspection to both Gym transcript views. The implementation and documentation
are in commit `aa5529364b3c182ba835e04bd84f3bbbb4803fae`.

## Manual Rust gate

The pinned Rust 1.97.1 gate ran on coderos in the Codex-owned verification
worktree, with its separate Cargo target. The other agent's checkout and draft
PR were unchanged. The [run record](gate/run.json) and adjacent logs retain the
commands, source, elapsed times, and results.

```sh
CARGO_TARGET_DIR=~/.cache/openagents/target-iteration-speed-verification \
  ./scripts/verify-rust.sh --crates gym \
  --phases preflight,fmt,clippy,clippy-features,tests,tests-features
```

All six requested phases passed in 140.1 seconds. Default Gym library tests
passed 481/481; feature-enabled library tests passed 573/573, with the retained
file-viewer acceptance ignored in that routine suite. The binary and integration
tests also passed. Existing opt-in integration tests remained ignored.

The record is `partial` because this is a scoped Gym gate. Other crates,
PostgreSQL, dependency policy, Metal, and soak were not rerun for this viewer
change. The record's dirty flag comes from the pre-existing untracked
`scripts/__pycache__/`; its tracked diff digest is empty.

On macOS, the [focused tests](focused-tests.log) passed all 11 viewer tests,
and [strict all-target Gym Clippy with TUI](macos-clippy.log) passed. The tests
cover history boundaries, reads and edits, snapshot selection, hashes, unsafe
paths, long paths, Unicode cells, scrolling, resizing, and both UI paths.

## Retained TB4 acceptance

The separately invoked acceptance read the committed bundle for
`session-window-debug__3KVqBUz`, with no model calls or benchmark execution.
It opened `/app/app/gc.py` and verified both candidate copies and the final
export against their retained SHA-256 values.

The [inventory](file-viewer.json) records all three sources. The
[initial screen](missing-at-step.txt) keeps later snapshots hidden; the
[candidate screen](retained-candidate.txt) follows explicit Tab selection and
labels the source. These files preserve the actual paths of the measured
worktree.

## Native terminal acceptance

A real macOS PTY ran the built `gym-terminal` at 180 columns by 42 rows with
Jev disabled. It opened the retained local trial in head-to-head mode, clicked
`app/events.py`, opened the viewer, exercised Page Down and End, returned with
Esc, and quit with exit code zero. No model calls were made. The first
automation selector expected an absolute path; it was corrected to recognize
the relative path that the real transcript displays. No product change was
needed for that selector correction.

The [PTY record](pty.json) identifies the trial and click. Retained screens:
[transcript](pty-transcript.txt), [opened file](pty-opened.txt),
[selected evidence](pty-selected.txt), [scrolled](pty-scrolled.txt),
[end](pty-bottom.txt), and [returned transcript](pty-returned.txt).

The viewer does not claim to reconstruct arbitrary shell mutations. Recorded
reads can be partial; patches remain patches. Snapshot capture times absent
from retained evidence remain unknown. Those limits are explained in the
[file-viewer guide](../../retained-files.md).
