# Terminal sessions verification — September 26, 2026

[Issue #9708](https://github.com/OpenAgentsInc/openagents/issues/9708) adds
[NIP-TERM](../../../nips/openagents/NIP-TERM.md) and
[`crates/coder-pty`](../../../crates/coder-pty/README.md): host terminals on
real PTYs with sequenced output, a bounded replay buffer, attach and detach
bookkeeping, explicit gaps, idle expiry, and shutdown cleanup, plus the
portable client state that applies their frames. It is part of the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).

## Evidence class

Host tests are real-process tests on one computer: macOS 26.4 (build 25E246),
Apple silicon, Rust 1.97.1. Every terminal is an actual PTY from `openpty`
running `/bin/sh`, `/bin/cat`, `stty`, or `sleep`. Frames travel through an
in-process channel sink. No relay, NIP-REACH channel, second computer,
emulator, simulator, or physical mobile device was involved. The only process
groups the tests signal are ones their own hosts created.

## Checks that ran

Each command ran from the worktree with the worktree's own target directory:

```sh
CARGO_TARGET_DIR=<worktree>/target cargo test -p coder-pty
CARGO_TARGET_DIR=<worktree>/target cargo test -p coder-pty --no-default-features
CARGO_TARGET_DIR=<worktree>/target cargo clippy -p coder-pty --all-targets -- -D warnings
CARGO_TARGET_DIR=<worktree>/target cargo clippy -p coder-pty --no-default-features --all-targets -- -D warnings
cargo fmt -p coder-pty --check
RUSTDOCFLAGS='-D warnings' cargo doc -p coder-pty --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p coder-pty --no-deps --no-default-features
cargo check -p coder-pty --target wasm32-unknown-unknown
cargo check -p coder-pty --no-default-features --target wasm32-unknown-unknown
cargo check -p coder-pty --target x86_64-unknown-linux-musl
cargo check -p coder-pty --target aarch64-linux-android
cargo check -p coder-pty --target aarch64-apple-ios
```

Results: 19 unit tests, 20 real-PTY tests, and 3 fixture tests passed; the
client-only build passed its 19 unit tests and 3 fixture tests. Clippy and
rustdoc reported no warnings in either feature set, and formatting matched.
The target checks compiled; they do not run anything. The real-PTY suite then
ran 20 more times with 64 test threads, all passing, and no test process
remained afterwards. No other crate's tests ran, and the workspace gate did
not run.

The first stress runs failed intermittently: concurrent `openpty` calls on
macOS returned error -6, because macOS builds `openpty` on the non-reentrant
`ptsname`. The host now serializes PTY allocation and spawning within the
process, and the failure did not recur in the 20 runs above.

## Acceptance coverage

| Acceptance item | Test in `tests/pty.rs` |
| --- | --- |
| Echo round trip through a real PTY | `echo_round_trips_through_a_real_pty`, `the_process_sees_a_terminal_and_its_workspace` |
| Resize | `resize_reaches_the_process` (`stty size` reads 24 80, then 40 100) |
| Two attached readers | `two_attached_readers_see_the_same_frames` |
| Detach and reattach with replay | `a_detached_client_reattaches_and_replays_what_it_missed` (second attachment starts at exactly the next sequence number, no repeat, no gap) |
| Replay after the buffer wrapped reports a gap | `replay_after_the_ring_wrapped_reports_a_gap` (4 KiB ring, about 18 KiB of output; the gap names its range and byte count) |
| Input without the right refused | `input_without_the_terminal_right_is_refused` (input, resize, signal, close, and interactive attach refused; the refused text never reaches the PTY), `observers_read_only_when_the_host_allows_it` |
| Process exit reported with status | `process_exit_is_reported_with_its_status` (code 7, then signal 15), `a_signal_reaches_the_foreground_process` |
| Host shutdown kills the process group | `host_shutdown_kills_the_process_group`, `dropping_the_host_kills_the_process_group` (the shell ignores `SIGHUP` and `SIGTERM` and has a background child; `supervise::running` reports the group empty afterwards) |
| `cargo test`, Clippy, and format | The checks above |

Additional tests cover close with an exact retry, idle expiry that spares
attached terminals, `lost` after a host generation change, exact input retries
and reused request IDs, workspace escape through a symbolic link, the
environment allowlist and cleared host environment, revocation of an existing
attachment, per-attachment rate limiting, and a terminal inside a
`coder-boundary` write boundary. Unit tests cover the ring, the client state
(duplicates, frames ahead of the expected one, gaps, exit), the screen buffer
(escape sequences, carriage returns, split UTF-8, bounds), wire validation,
and that the largest frame fits one NIP-REACH data frame. `tests/wire.rs`
checks every fixture in `crates/coder-pty/fixtures/nip-term.json`.

## Limits

- Only macOS ran. The Linux, Android, and iOS host paths compiled for musl,
  Android, and iOS targets but did not run; the glibc `libutil` link was not
  exercised.
- On platforms without a Unix PTY the host refuses every open. The
  `wasm32-unknown-unknown` build compiles that path; no test ran it.
- The rights check and frame sink are test doubles. NIP-HOST grants,
  NIP-REACH channels, and sealed `3188` artifacts are wired by the resident
  host in [#9712](https://github.com/OpenAgentsInc/openagents/issues/9712).
- A terminal's tree has no memory cap, and a descendant that calls `setsid`
  escapes close, expiry, and shutdown.
- Terminals, rings, and deduplication memory are process-local. A restart
  loses every terminal, which clients see as `lost`.
- The screen buffer is a plain-text scrollback, not a terminal emulator. No
  renderer or Rust Native view was built or observed.
- No fuzzing, property tests, or formal model ran for the wire parser, the
  ring, or the client state.
