# Host serve

`coder host serve` runs one resident Coder host on a computer. It answers
device enrollment, publishes presence and reachability hints to enrolled
devices, accepts authenticated direct channels and relay-carried operations,
serves terminals, hands task operations to the durable task inbox, and
publishes activity summaries.
[Issue #9712](https://github.com/OpenAgentsInc/openagents/issues/9712)
delivers it as part of the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).
The implementation is [`crates/coder-host`](../../../crates/coder-host/README.md),
and the [verification record](../verification/2026-09-26-host-serve.md) holds
the evidence.

## Set up the host

Establish the owner on the computer itself and record the relays and
workspaces the host serves:

```sh
coder host init --owner <owner-public-key> --relay wss://relay.example/ \
  --workspace checkout=/path/to/checkout
```

The command prints the host's public key. The owner key accepts lower-case
hex or an `npub`; the placeholder is not a real key. The host key and grants
live in the `coder-access` store under `~/.openagents/coder-access/`. The
relays and workspaces go to `~/.openagents/host/serve.json`, mode `0600`, so
`coder host serve` runs with no arguments.

A workspace label is what devices name. A device never sends a path: a task
names the label, and a terminal names the workspace ID derived from it.

## Run it

```sh
coder host serve
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--relay URL` | The settings from `init` | A relay to serve. Repeat for more. The first is the primary relay. |
| `--workspace LABEL=PATH` | The settings from `init` | An admitted workspace. |
| `--listen ADDR` | `OPENAGENTS_HOST_LISTEN`, else `127.0.0.1:0` | The direct-channel listener. |
| `--allow-nonloopback` | Off | Permit a listener on a LAN or tailnet address. |
| `--advertise CLASS=HOST:PORT` | None | Advertise another `lan`, `tailnet`, or `public` address, such as a forwarder. |
| `--generation N` | `OPENAGENTS_HOST_GENERATION`, else the next counter value | The NIP-REACH host generation. |
| `--runtime FILE`, `--no-runtime` | `~/.openagents/host/runtime` | The runtime record SSH launchers read. |
| `--tasks DIR` | `~/.openagents/tasks` | The durable task inbox. |
| `--owner KEY` | None | Establish the owner on first start; the same owner is a no-op. |

Every `coder host` command also takes `--state DIR` for the access store,
`--root DIR` for `~/.openagents/host`, and `--loopback-test`, which permits
`ws://` to a numeric loopback relay for fixtures only.

`SIGTERM` or `SIGINT` stops the host: every terminal's process tree ends, and
the runtime record is removed.

## Enroll a device

```sh
coder host invite
```

The command prints one `coder-host:` line and nothing else. The invitation
admits one device for five minutes; show it only to that device. On the
device, `coder_access::client::redeem` exchanges it for a grant over the
relay while the host serves. `--rights` narrows the grant, and
`--grant-secs` sets its lifetime.

`coder host list [--json]` shows enrolled devices, and
`coder host revoke --device KEY` revokes one. A running host closes that
device's channels, ends its terminal attachments, and refuses its requests
on their next check. For a host without a screen, use reverse enrollment
through `coder-access request` and `approve`.

## Run it as a service

The host service in [`coder-service`](host-service.md) starts
`coder host serve` by default. Install it with the host's own key, so the
host descriptor and every record the host signs name the same identity:

```sh
coder-service service install --host-key "$(coder host public-key)"
```

Under the service, the host reads `OPENAGENTS_HOST_LISTEN` and
`OPENAGENTS_HOST_GENERATION`, and writes the ready record to
`OPENAGENTS_HOST_READY_FILE` with the version from
`OPENAGENTS_HOST_VERSION` once it serves: the listener is bound and the first
relay subscription is up, or its 10-second wait has ended.

## Reach it over SSH

`coder-ssh` starts or adopts a host on a machine you can reach with `ssh`.
Give its runner these arguments:

```rust,ignore
let runner = Runner::new(
    vec!["host".into(), "serve".into(), "--loopback".into(),
         "--owner".into(), owner_key, "--relay".into(), relay.clone()],
    vec!["host".into(), "invite".into(), "--relay".into(), relay],
)?;
```

`serve` writes `~/.openagents/host/runtime` with its process ID and the
direct listener's port once it serves, and `invite` prints one line. The
tunnel forwards a local port to that listener, and the device opens a direct
channel through it; the channel still proves the host key.

## How a device connects

A client uses `coder_host::client`:

1. With the owner key, `fetch_directory` reads the owner's host directory.
2. `fetch_reach` reads the host's presence and hints sealed to the device.
3. `client::Connector` plugs into a `coder_link::Registry`. Each attempt
   tries the selected direct routes, then falls back to the relay.
   Selection never offers a loopback address to another machine.
4. A `Link` carries NIP-HOST operations and NIP-TERM requests on either
   route. `fetch_summaries` reads activity summaries after a reconnect.

A handshake that the host refuses after proving its key blocks the
connection instead of falling back: a revoked grant is revoked on every
route. A relay link's probe fails while a direct route answers, so sending
`Signal::RetryNow` moves a relay connection back to a direct one.

## Authority

- Creating a task is an inert submission to the task inbox. It records
  intent and starts nothing; the local owner still needs its own execution
  grant. Pairing and enrollment never grant execution authority.
- `task.steer` records a correction and `task.cancel` requests a
  cancellation, with the semantics of the [task owner](task-owner.md). Each
  names the revision the device last read; another revision refuses as
  `stale`.
- Every operation needs one right: `operate` for tasks, `terminal` for
  terminals, and `access_admin` or `access_read` for access operations.
- Summaries carry a generic headline for the phase, such as `Task queued`,
  never a title or prompt a device sent.

## Limits

- The direct listener speaks TCP only; the WebSocket mapping is not
  implemented.
- Presence withholds telemetry, so placement skips the host.
- Terminals do not survive a restart; references to them refuse as `lost`.
- The CAP/CJ binding of NIP-HOST is not served.
- Evidence is synthetic and from one machine; see the verification record.
