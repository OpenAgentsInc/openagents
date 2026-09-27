# Host serve verification — September 26, 2026

[Issue #9712](https://github.com/OpenAgentsInc/openagents/issues/9712) adds
`coder host serve`, the resident host in
[`crates/coder-host`](../../../crates/coder-host/README.md). It composes
NIP-HOST enrollment (`coder-access`), NIP-REACH presence, hints, and direct
channels (`coder-reach`), NIP-TERM terminals (`coder-pty`), the durable task
inbox (`coder::task::remote`), NIP-WS activity summaries, and a real
`coder-link` connector. It is part of the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).
The [host serve guide](../runtime/host-serve.md) describes operation.

## Evidence class

All evidence is synthetic, from one macOS 26.4 arm64 computer with Rust
1.97.1. The relay is the shared synthetic NIP-42 relay fixture from
`crates/coder-control/src/tests/relay.rs`: it authenticates publishers and
delivers private `3188` artifacts only to their author or recipient. It is not
the production relay. Every socket binds `127.0.0.1` port 0. Terminals are
real PTYs running `/bin/sh`. No emulator, simulator, physical device, second
computer, LAN, tailnet, or real `sshd` took part.

## Acceptance run

`crates/coder/tests/host_serve.rs` runs the scenario in
`crates/coder-host/tests/support/scenario.rs` against the durable task inbox
that `coder host serve` uses. `crates/coder-host/tests/end_to_end.rs` runs the
same scenario with an in-memory task owner. The host listens on loopback
behind a TCP forwarder the test can cut, and advertises only the forwarder,
so losing the direct path is invisible to the host until its channel breaks.

| Step | What the run asserts |
| --- | --- |
| Start | The runtime record is exactly `schema=openagents.coder.host-runtime.v1`, the process ID, and the listener port. |
| Enroll by invitation | A `standard` invitation redeems over the relay for a grant naming the host key, the relay, epoch 0, and the `standard` rights. A second device's redemption refuses as `forbidden`. |
| Discover through the owner directory | The owner publishes revision 1 listing the host and its relay; the owner key reads it back, and the device key reads nothing. The host's presence reports generation 7, the owner, and the `terminal` capability. Same-machine selection puts the forwarder's TCP hint first; other-machine selection offers no TCP route. |
| Connect directly | The `coder-link` registry reaches `Connected` with route `Direct(forwarder)`, and a ping answers. |
| Terminal | An open in the `checkout` workspace returns the terminal generation derived from the host key and generation 7 and a 24 by 80 size. After attach, input reports exactly the bytes written, and the screen shows `direct-ok`. A reference from generation 6 refuses as `lost`. |
| Create a task | `task.create` returns a `dispatched` receipt; the task inbox holds revision 1, the title and prompt, status `queued`, and nothing started. The device's activity summary for it has sequence 1, phase `queued`, and the headline `Task queued`. An unadmitted workspace label refuses as `forbidden`. |
| Drop the direct channel | Cutting the forwarder moves the registry out of `Connected` with failure `Closed`, and the link reports closed without a host code. The registry reconnects with route `Relay(relay)`. A relay attach gets a new attachment ID, relay input shows `relay-ok`, and `task.steer` at revision 1 returns `dispatched`: the inbox holds revision 2 and the new prompt. A second steer at revision 1 refuses as `stale`. Output requested before detaching is not yet on the screen. |
| Reconnect and catch up | Restoring the forwarder and sending `RetryNow` moves the registry back to `Direct(forwarder)`. Attaching after the last applied sequence number replays the missed output: `later-ok` appears, with no gap, no missing range, and each of `direct-ok`, `relay-ok`, and `later-ok` exactly once. The device's summary for the task reaches sequence 2, and `DataCurrent` makes freshness current. |
| Revoke | The owner's `device.revoke` over the relay returns epoch 1 and one grant. |
| Every later operation refused | The open channel closes with host code `revoked` and the registry settles in `Blocked(Revoked)`; a call on it fails as `Closed(revoked)`. A new direct handshake refuses in the host's signed verdict as `revoked`. Over the relay, `task.create`, `task.steer`, `task.cancel`, and `terminal.open` each refuse as `revoked`, and terminal input, attach, and open each return a result refused as `revoked`. `RetryNow` settles back in `Blocked(Revoked)`. The task inbox still holds revision 2, status `queued`, with nothing started. |
| Stop | Shutdown removes the runtime record. |

## Other checks

- `crates/coder-host/tests/serve.rs`: the ready record decodes with
  `coder-service`'s own `ReadyRecord` and carries the generation, version,
  protocol version, and sorted valid capability flags. NIP-HOST
  `terminal.open` over the relay opens a shell and returns a terminal ID the
  device attaches to over a direct channel. An `observe`-only device's
  `terminal.open` refuses as `missing_right` naming `terminal`, its
  `task.steer` names `operate`, and its terminal input, open, and observe
  attach refuse as `not_admitted`. A task operation on a host without a task
  owner refuses as `unavailable`. A handshake that expects another
  generation refuses as `stale`, and one from a key the host never enrolled
  refuses as `not_admitted`.
- `crates/coder/tests/host_cli.rs` runs the built `coder` binary as separate
  processes: `host init` prints the host key and writes `serve.json` with mode
  `0600`; `host public-key` prints the same key; `host invite` prints exactly
  one `coder-host:` line in the characters the SSH launcher's script accepts;
  `host serve --owner` with another owner exits 1 before anything binds;
  `host serve --loopback --owner` with the same owner, started with the host
  service's environment,
  writes the ready record for generation 4 and the runtime record with its
  own process ID, answers a redemption over the relay, and exits zero on
  `SIGTERM` after removing the runtime record. `host list --json` shows the
  new device as `active`.
- Unit tests: channel message fragments and bounds, message decoding and
  refusal, mailbox derivation from both ends, terminal generation and
  workspace IDs, relay frame reordering (`coder-host`); the durable inbox's
  inert, idempotent creation, steer and cancel revisions, and label admission
  (`coder::task::remote`); split channel halves (`coder-reach`); per-right
  refusal of `task.steer` and `task.cancel` (`coder-access`); loopback `ws`
  directory relays (`coder-reach`).

## Commands

Each command ran from the worktree with a separate target directory,
`CARGO_PROFILE_DEV_DEBUG=0`, and `CARGO_INCREMENTAL=0`:

```sh
cargo test -p coder-host
cargo test -p coder-reach
cargo test -p coder-access
cargo test -p coder --lib task::remote
cargo test -p coder --test host_serve --test host_cli
cargo clippy -p coder-host -p coder-reach -p coder-access --all-targets -- -D warnings
cargo clippy -p coder-access --no-default-features --lib -- -D warnings
cargo clippy -p coder --lib --bins --test host_serve --test host_cli -- -D warnings
cargo check -p coder-control -p coder-labor
cargo fmt -p coder-host -p coder-reach -p coder-access -p coder -- --check
```

Results: `coder-host` 5 unit, 1 end-to-end, and 3 serve tests; `coder-reach`
20 unit and 11 socket tests; `coder-access` 14 library and 2 CLI tests;
`coder` 2 inbox unit tests, the acceptance run, and the CLI process test. All
passed. The end-to-end run passed five consecutive times in `coder-host` and
three times against the durable inbox, each in about 4.2 seconds. Clippy
reported no warnings and formatting matched. The workspace gate did not run,
and no other crate's tests ran.

## Limits

- Synthetic evidence only: one computer, the synthetic relay, loopback
  sockets, and a forwarder standing in for a network path. No production
  relay, LAN, tailnet, real `sshd`, or device of any kind was involved.
- The host service's launchd or systemd runtime did not run `coder host
  serve`; the CLI test reproduces its environment variables in a child
  process.
- `coder-ssh`'s own tests still use its shell stand-in; the real host
  satisfies the contract through the CLI test's runtime record and
  one-line invitation.
- Presence withholds telemetry, so placement skips this host.
- The direct listener speaks TCP only. The NIP-HOST CAP/CJ binding is not
  served.
- Relay-carried terminal frames are one artifact each. The run proves
  correctness, not throughput.
- Terminals do not survive a host restart.
