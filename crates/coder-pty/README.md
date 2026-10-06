# coder-pty

Host terminal sessions over real PTYs, and the portable client state that
reads them. This crate implements [NIP-TERM](../../nips/openagents/NIP-TERM.md):
a device with the `terminal` right opens a terminal on a host, types into it,
resizes it, detaches, and reattaches from another device to see the output it
missed, bounded and in order.

## Halves

| Module | Feature | Platforms | What it does |
| --- | --- | --- | --- |
| `wire` | always | all | The NIP-TERM request, result, and frame bodies, with validation. |
| `ext` | always | all | The NIP-TERM extensions' wire contract: features and negotiation, record streams (framing, CRC-32C, assembly, and order), the snapshot join, history, block pages, session records, and effect frames. The host serves the effects, snapshot, and blocks features. |
| `emulator` | always | all | The seam for a host's authoritative emulator per terminal: output in, query replies and effects out. `coder_vt::Authority` implements it. |
| `ring` | always | all | The bounded replay buffer: sequence numbers, discard, and missed ranges. |
| `client` | always | all | `TerminalState` applies frames, ignores duplicates, detects lost frames, records gaps, and keeps a bounded plain-text `Screen`. |
| `host` | `host` (default) | Unix, Windows | `Host` owns PTYs (a Windows pseudoconsole), process groups (job objects), rings, attachments, budgets, idle expiry, and shutdown. |

A client build, such as the mobile library or a renderer, uses
`default-features = false` and gets no system calls. The screen buffer is a
plain-text scrollback that drops escape sequences; it is not a terminal
emulator, and Rust Native gets no terminal widget from it. A renderer that
needs full emulation reads the output bytes from `Applied::Output`.

## Host

```rust
use coder_pty::host::{self, Config, Host};
use coder_pty::wire::{Attach, Launch, Mode, Open, Size};

let config = Config::new().workspace(workspace_id, "/path/to/checkout");
let host = Host::new(config, rights); // rights: Arc<dyn host::Rights>
let (_, opened) = host.open(device, &Open::new(request, workspace_id, "", Launch::Shell, Size::new(24, 80)))?;
let (sink, frames) = host::channel(256);
host.attach(device, &Attach::new(request2, terminal, Mode::Interact, 0, 64 * 1024), Box::new(sink))?;
```

- **Authority.** `Rights::holds(principal, right)` is asked on every
  operation, and `Host::tick` rechecks every attachment. The resident host
  ([#9712](https://github.com/OpenAgentsInc/openagents/issues/9712)) backs it
  with NIP-HOST grants.
- **Transport.** `FrameSink::deliver` receives each attachment's frames. It
  runs while the host holds the terminal's state, so it queues and returns;
  `SinkError::Full` keeps the attachment's place, and the host resumes it on
  the next output or tick, with a gap frame if the ring moved past it. The
  resident host backs it with a NIP-REACH direct channel or sealed `3188`
  artifacts.
- **Process ownership.** The child calls `setsid` and takes the PTY as its
  controlling terminal, so it leads its own session and process group, the
  `crates/supervise` convention. Close, idle expiry, and shutdown send the
  group `SIGHUP`, `SIGTERM`, and `SIGCONT`, wait `supervise::GRACE`
  (250 milliseconds), send `SIGKILL`, and reap the child before recording its
  exit. They then wait, within the same grace period, until the group is
  empty: a killed descendant stays in the group as a zombie until init or a
  subreaper reaps it. A child that exits on its own takes the rest of its
  group with it.
- **Boundaries.** `Config::wrap` takes a `Wrap` that turns a program and its
  arguments into the command to spawn. A `coder_boundary::Boundary` fits
  directly; `tests/pty.rs` runs a terminal inside a write boundary. The host
  sets the working directory, clears the environment, and applies its base
  variables and the allowlisted requested ones on the wrapped command.
- **Bounds.** The defaults are a 1 MiB, 4,096-frame ring per terminal, 8 KiB
  output frames, 256 KiB per second per attachment, 16 terminals, 8
  attachments per terminal, a 30-minute idle period, and 1,024 remembered
  request IDs.
- **Side effects.** With `Config::emulator`, each terminal's emulator parses
  every output byte once. The host writes query replies to the terminal
  while no `interact` attachment predates the effects feature, and sends
  bells, title and directory changes, and clipboard writes as live effect
  frames to attachments that named it; a clipboard write goes only to the
  principal that typed last. `coder-vt`'s `tests/authority.rs` checks one
  reply for two devices and no repeat on reattach.
- **Snapshots.** When the emulator writes snapshots (`Emulators::snapshots`),
  the host serves the snapshot feature: an attach by snapshot sends the
  emulator's snapshot stream through the sink, every part before the next
  sequenced frame, and continues after its `through`; an attachment that
  joined by snapshot and falls behind the ring receives a fresh snapshot
  instead of a gap; and `Host::history` sends a history stream on the
  principal's own attachment. A sink carries record streams only when it
  says so (`FrameSink::carries_records`); `deliveries` is an in-process one.
  `coder-vt`'s `tests/join.rs` checks each path on real PTYs.
- **Typist.** At most one `interact` attachment types at a terminal:
  input, resize, and signal from anyone else refuse as `not_typist`, and
  `Host::seat` takes or releases the role. The first to type at a terminal
  without one takes it, and it ends with its attachment. A client without the
  typist feature counts as its device. `tests/typist.rs` covers racing
  devices, take and release, detach and revocation, a second route of the
  same device, and an older client.
- **Proposals.** `Host::proposal` retains bounded pending shell proposals and exact-revision decisions under the current terminal right and an interact attachment. It uses the same approval checks as `terminal-core`, records input disposition before writing, and never repeats it.
- **Block journal.** When the emulator keeps one (`Emulators::blocks`),
  `Host::block_page` reads a page of it under the `terminal` right or the
  observer policy, and sets each block's `retained` from the ring.

## Limits

- **Unix and Windows.** Elsewhere `Host::open` refuses as `unavailable`.
  On Windows a terminal is a pseudoconsole (ConPTY, Windows 10 1809 or
  later) whose program starts suspended inside a kill-on-close job object;
  a hang-up closes the pseudoconsole (`CTRL_CLOSE_EVENT`), an interrupt
  types Ctrl+C, and a kill ends the job. The default shell is `%ComSpec%`.
  `tests/windows.rs` passes under Wine 11, which does not implement
  `ResizePseudoConsole`, so the resize case skips there. Tested on
  macOS and on Linux with glibc 2.42 (NixOS); the
  [Linux runs record](../../docs/coder/verification/2026-09-27-linux-runs.md)
  holds the Linux results. On glibc the crate links `libutil`, where
  releases before 2.34 keep `openpty`; on 2.34 and later `openpty` resolves
  from `libc`, and the linker drops `libutil` as unneeded. Musl and Android
  builds compile but were not run.
- **No memory cap.** A terminal's tree is not placed under the supervisor's
  memory scope or watch. A terminal is interactive and long-lived, which the
  supervisor's job model does not cover; the resident host must bound it
  another way if it needs to.
- **Escaped descendants.** A descendant that calls `setsid` leaves the group
  and outlives close, expiry, and shutdown, as it does under the supervisor.
- **Descriptor window.** PTY allocation and spawning are serialized inside
  this crate, but a fork elsewhere in the process between `openpty` and
  setting close-on-exec could inherit a PTY descriptor.
- **Process-local state.** Terminals, rings, and deduplication memory live in
  memory. A host restart loses every terminal, which is reported as `lost`,
  and the deduplication window restarts empty.
- **Wired elsewhere.** This crate includes no NIP-HOST grant store,
  NIP-REACH channel, or `3188` sealing. The resident host in
  [`coder-host`](../coder-host/README.md) fills the traits above with them.

## Tests

```sh
cargo test -p coder-pty
cargo clippy -p coder-pty --all-targets -- -D warnings
cargo fmt -p coder-pty --check
```

`tests/pty.rs` uses real PTYs and real processes: echo round trip, a process
that sees a terminal, resize, two readers, detach and reattach with replay,
replay after the ring wrapped, refused input without the right, revocation,
exit codes and signals, a foreground signal, close, host shutdown and host
drop killing a process group that ignores `SIGHUP` and `SIGTERM`, idle
expiry, lost generations, exact input retries, workspace and environment
admission, per-attachment rates, and a terminal inside a write boundary.
`tests/wire.rs` checks the NIP-TERM fixtures in `fixtures/nip-term.json`.

### Agent typing

An owner explicitly hands off a terminal's typist seat with a thread and run.
`Host::agent_producer` connects a host-local producer to that exact private
lease; the resident host exposes it through `Running::terminal_agent_producer`.
`AgentProducer::produce` validates generated input against those identities,
records a byte-free attempt in a private per-thread directory, and calls the
host's input admission. An interrupted attempt remains unknown and cannot be
replayed. Each lease admits at most 256 distinct input IDs. The producer has
no reader, and typing authority grants no observation of blocks or screens.
Persistent evidence currently requires Unix private directory permissions;
other platforms refuse until their directory privacy can be validated.
An owner key reclaims the seat; shares alone cannot reclaim it. The native
and Verse pane title shows a leading agent badge. No model starts on handoff.
