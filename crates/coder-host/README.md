# coder-host

`coder-host` is the resident Coder host and the client that connects to it.
One process answers enrollment, publishes presence and reachability hints,
accepts direct channels and relay-carried operations, serves terminals,
hands task operations to the task owner, and publishes activity summaries.
The `coder` binary exposes it as `coder host`. The
[host serve guide](../../docs/coder/runtime/host-serve.md) covers setup and
operation; this README covers the crate.

## What it composes

| Profile | Crate | How the host uses it |
| --- | --- | --- |
| [NIP-HOST](../../nips/openagents/NIP-HOST.md) | `coder-access` | The access store holds the one host key, the owner, and the grants. Every NIP-HOST request, on a relay or a direct channel, goes through `Host::handle`. |
| [NIP-REACH](../../nips/openagents/NIP-REACH.md) | `coder-reach` | Presence and hints are sealed to each enrolled device. The direct-channel `Acceptor` checks grants through `authority::Grants`, which reads the real store. |
| [NIP-TERM](../../nips/openagents/NIP-TERM.md) | `coder-pty` | The terminal host asks `authority::Grants` for rights, and delivers frames into a channel queue or as sealed relay artifacts. |
| CTRL semantics | `coder` task inbox | `tasks::Tasks` receives `task.create`, `task.steer`, and `task.cancel`. The `coder` binary supplies `coder::task::remote::Inbox`. |
| [NIP-WS](../../nips/openagents/NIP-WS.md#audience-bound-activity-summaries) | `nostr::activity_summary` | Each task change publishes a generic summary to every device that holds `observe`. |
| Connection supervision | `coder-link` | `client::Connector` is a real `Connector`: direct routes first, relay fallback second. |

## Modules

| Module | What it does |
| --- | --- |
| `serve` | `start` runs a host and returns `Running`. Submodules serve direct channels, the relay loops, NIP-TERM operations, and NIP-HOST dispatch. |
| `authority` | The grant store as the channel, terminal, and publication paths see it: serialized store access, a snapshot that reloads when the store file changes, and a device's standing. |
| `message` | Direct-channel messages: host calls and answers, pings, the closing message, NIP-TERM bodies, and fragmentation. |
| `mailbox` | Mailboxes derived from the host and device's NIP-44 conversation key, the terminal generation, and workspace IDs. |
| `client` | `Device`, directory and reach fetches, summaries, `Link`, `Connector`, and `Ordered` frame ordering. |
| `tasks` | The task-owner trait and `NoTasks`. |
| `config` | The host configuration. |
| `telemetry` | Coarse CPU and memory samples for presence. |
| `cli` | `coder host init`, `public-key`, `invite`, `list`, `revoke`, and `serve`. |

The default `host` feature builds the host: `serve`, `cli`, `authority`,
`config`, and `telemetry`. A client, such as the mobile library through
[`coder-computers`](../coder-computers/README.md), disables default features
and keeps `client`, `mailbox`, `message`, and `tasks`, with the portable
halves of `coder-access` and `coder-pty`.

## One host identity

The `coder-access` store at `~/.openagents/coder-access/host.key` is the host
key. The host signs grants and replies, presence, hints, direct-channel
proofs, terminal results and frames, and summaries with it. Install the host
service with that key:

```sh
coder-service service install --host-key "$(coder host public-key)"
```

The read-only history observer in `coder-connect` stays a separate
capability with its own key under `~/.openagents/coder-connect/` and its own
`coder-pair:` pairing. A host grant never admits an observer read, and an
observer grant never admits a host operation. The host process does not serve
the observer.

## Direct-channel binding

A direct channel carries messages as JSON objects, each named by `v`:

| Direction | `v` | Contents |
| --- | --- | --- |
| Device to host | `openagents.host-call.v1` | `event`: the exact signed NIP-HOST request artifact. |
| Host to device | `openagents.host-answer.v1` | `event`: the exact signed NIP-HOST reply. |
| Either | `openagents.host-ping.v1`, `openagents.host-pong.v1` | `nonce`, 1 to 64 bytes. |
| Host to device | `openagents.host-closing.v1` | `code`: `revoked`, `stale`, or `not_admitted`, sent before the host closes the channel. |
| Device to host | `openagents.terminal-*.v1` | NIP-TERM requests. |
| Host to device | `openagents.terminal-result.v1`, `openagents.terminal-frame.v1` | NIP-TERM results and frames. |

Each data frame starts with one flag byte, `1` when more fragments follow and
`0` for the last, so a message can exceed one frame. A message is at most
256 KiB. The host accepts a call only when the event's signer is the device
the channel proved, and only for requests that name the host's primary relay.
The host rechecks the channel's grant before each message and every
`recheck_every` (500 milliseconds by default).

## Presence telemetry and last seen

Presence carries NIP-REACH telemetry so placement can rank the host: the
logical CPU count, the one-minute load average per CPU as CPU use, and the
share of memory available to new work (`MemAvailable` on Linux, the kernel's
memory status level on macOS), each a whole number. A value the host cannot
read withholds the whole sample, and `--no-telemetry` (or
`Config::telemetry = false`) turns it off.

The host records when it last saw each device: every admitted NIP-HOST
request, and each direct channel at admission and then at most once a minute
while messages arrive. `device.list` reports it as `last_seen`.

## Relay binding for terminals

A device seals each NIP-TERM request as a private `3188` artifact to the host,
with the request ID as its mailbox and the body's `v` as its schema. The host
answers only a key it enrolled, only within 60 seconds of the request's issue
time, and seals the result to the device under the same mailbox. An attach
delivers frames as artifacts under the attachment ID. A relay can return
retained frames in any order after a reconnect, so a client feeds them through
`client::Ordered`.

## Limits

- The direct listener speaks TCP only. The WebSocket mapping in NIP-REACH is
  not implemented.
- A standalone host takes its generation from a counter file with a clock
  floor; the host service passes its own generation. Switching between the
  two can make presence readers refuse the lower generation.
- The CAP/CJ binding of NIP-HOST is not served; only the direct artifact
  binding and the direct-channel binding run.
- Terminal state is process-local. A restart reports every terminal as
  `lost`.

## Tests

```sh
cargo test -p coder-host
cargo clippy -p coder-host --all-targets -- -D warnings
cargo fmt -p coder-host --check
```

`tests/end_to_end.rs` runs the acceptance scenario in
`tests/support/scenario.rs` with an in-memory task owner; the `coder` crate's
`tests/host_serve.rs` runs the same scenario with the durable task inbox.
The [verification record](../../docs/coder/verification/2026-09-26-host-serve.md)
lists what they establish.
