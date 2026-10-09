# coder-host

`coder-host` is the resident Coder host and the client that connects to it.
One process answers enrollment, publishes presence and reachability hints,
accepts direct channels and relay-carried operations, serves terminals,
hands task operations to the task owner, and publishes activity summaries.
The `coder` binary exposes it as `coder host`. The
[host serve guide](../../docs/coder/runtime/host-serve.md) covers setup and
operation; this README covers the crate.

## Native project and operator adapters

`Tasks` delegates optional project observation and operator cloud work to
`projects::Projects` and `cloud::Cloud`. `Inbox::with_projects` and
`Inbox::with_cloud` register these owners. The CLI enables them with explicit
`--project-observer` and `--cloud-operator` policy files; an unconfigured owner
returns `unsupported`.

`project.list`, `project.read`, and `project.original` read retained supervisor
evidence without running its controller. `cloud.projects`, `cloud.catalog`,
`cloud.list`, `cloud.read`, and `cloud.original` read operator-admitted profiles
and canonical jobs. These operations require `observe`. Cloud submission,
continuation, cancellation, and reconciliation require `operate`, exact source
and profile pins, and the operator policy independently of the host grant.
Every path also checks the host's configured workspace aliases.

Cloud effects retain their original request identity and typed outcome.
`request.operation` rechecks the original native grant and current operator
admission before revealing that outcome. Workers use `cloud::authority` to
recheck native standing outside the synchronous access-store lock. See the
[resident operator policy](../../docs/cloud/README.md#resident-operator-bridge)
for explicit provider configuration and uncertainty handling.

## What it composes

| Profile | Crate | How the host uses it |
| --- | --- | --- |
| [NIP-HOST](../../nips/openagents/NIP-HOST.md) | `coder-access` | The access store holds the one host key, the owner, and the grants. Every NIP-HOST request, as a relay artifact, on a direct channel, or inside a CJ execution request, goes through `Host::handle`. |
| [NIP-REACH](../../nips/openagents/NIP-REACH.md) | `coder-reach` | Presence and hints are sealed to each enrolled device. The direct-channel `Acceptor` checks grants through `authority::Grants`, which reads the real store. |
| [NIP-TERM](../../nips/openagents/NIP-TERM.md) | `coder-pty` | The terminal host asks `authority::Grants` for rights, and delivers frames into a channel queue or as sealed relay artifacts. |
| CTRL semantics | `coder` task inbox | `tasks::Tasks` receives `task.create`, `task.steer`, and `task.cancel`. The `coder` binary supplies `coder::task::remote::Inbox`. |
| [NIP-WS](../../nips/openagents/NIP-WS.md#audience-bound-activity-summaries) | `nostr::activity_summary` | Each task change publishes a generic summary to every device that holds `observe`. |
| Connection supervision | `coder-link` | `client::Connector` is a real `Connector`: direct routes first, relay fallback second. |

## Modules

| Module | What it does |
| --- | --- |
| `serve` | `start` runs a host and returns `Running`. Submodules serve direct channels over TCP and WebSocket, the relay loops, the CAP/CJ binding, NIP-TERM operations, and NIP-HOST dispatch. |
| `authority` | The grant store as the channel, terminal, and publication paths see it: serialized store access, a snapshot that reloads when the store file changes, and a device's standing. |
| `message` | Direct-channel messages: host calls and answers, pings, the closing message, NIP-TERM bodies, and fragmentation. |
| `mailbox` | Mailboxes derived from the host and device's NIP-44 conversation key, the terminal generation, and workspace IDs. |
| `client` | `Device`, directory and reach fetches, summaries, nudges, `Link`, `Connector`, `Ordered` frame ordering, and the `websocket` hint dialer. |
| `computer` | NIP-HOST `computer` for a device that holds `terminal`: a screenshot (the desk protocol, `grim`, X11 tools, `screencapture`, or `adb`), the open windows, and file chunks read and written, every file checked against its SHA-256 digest and none replaced without `overwrite`. |
| `tasks` | The task-owner trait (creation, steering, cancellation, archiving, durable commands, and queue edits) and `NoTasks`. |
| `nudge` | A device's stored note to its host that commands wait; the host answers with fresh presence when it reads one, even after being away. |
| `spend` | Agent spending, phase 1: the book of spend requests an agent asks the owner's phone to pay (`coder host spend request|list|show`), which the phone reads and answers with NIP-HOST `spend.list` and `spend.settle`. Nothing here pays. See [the spend protocol](../../docs/breez/spend-protocol.md). |
| `config` | The host configuration. |
| `tls` | Loads and checks the operator's certificate chain and key for the WebSocket listener. |
| `telemetry` | Coarse CPU and memory samples for presence. |
| `generation` | Which NIP-REACH generation `serve` runs as, from the host root's one counter in `coder_service::generation`. |
| `enroll` | Reverse enrollment for a host without a screen: publish a request, show its code, and read the outcome the running host recorded. |
| `cli` | `coder host init`, `public-key`, `invite`, `request`, `list`, `revoke`, `serve`, and `spend`. |

The default `host` feature builds the host: `serve`, `cli`, `authority`,
`config`, `telemetry`, `generation`, `enroll`, and `tls`. A client, such as the mobile library through
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
observer grant never admits a host operation, except that NIP-HOST
`chats.invite` hands a device holding `observe` a `coder-pair:` invitation.
The host process serves the observer in-process with tailnet admission and
chats on (below), or with iroh on and a Coder task directory; the iroh enroll
reply carries an invitation, and a device paired any other way, or whose
chat grant nears its end, asks with `chats.invite`.

## Tailnet admission

`coder host serve --tailnet-admission RIGHTS` (or `coder host init` with the
same option, recorded in `serve.json`) listens on this machine's tailnet
address, port 47109, and hands a single-use invitation with `RIGHTS` to a
caller that `tailscale whois` names as this machine's own untagged Tailscale
user ([NIP-HOST tailnet admission](../../nips/openagents/NIP-HOST.md#tailnet-admission)).
Unless `--no-tailnet-chats` is given and `~/.codex` or `~/.claude` exists, it
also returns a `coder-pair:` chat invitation and serves read-only history
on the primary relay in-process. The `tailscale` command comes from `PATH`,
else the macOS app. `tailnet::request` is the device side.

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

## WebSocket direct channels

`Config::listen_websocket` (`--listen-websocket ADDR`) adds a WebSocket
listener beside the TCP listener. It follows the same loopback rule, and the
host advertises it as a `websocket` hint, `ws://ADDR/`, in the same class as
the TCP listener's hint. An `Advertised` address that is a `ws` or `wss` URL
is a `websocket` hint for a forwarder in front of that listener. Both
listeners hand their connections to the same session, so a WebSocket channel
runs the same handshake, grant check, recheck, closing message, and message
binding as a TCP channel, carried one frame per binary message as
[NIP-REACH](../../nips/openagents/NIP-REACH.md#websocket-mapping) maps it.
The upgrade must finish within the handshake timeout.

`Config::websocket_tls` (`--websocket-tls-cert FILE --websocket-tls-key
FILE --websocket-name NAME`) makes the WebSocket listener terminate TLS
itself, so a `wss` hint needs no forwarder. The operator supplies the
certificate chain and private key as PEM files, for example from `tailscale
cert` or an ACME client; the host never obtains or renews a certificate. The
listener's hint becomes `wss://NAME:PORT/`, and the listener no longer serves
plain `ws`. The `tls` module reads both files once at start and refuses to
serve when the key file is missing, is not a regular file, is not owned by
this user, or is open to group or others; when either file holds no usable
PEM item; when the key does not match the leaf certificate; or when the leaf
certificate is not valid for `NAME`. When `NAME` and the bound address
disagree about loopback, as for a loopback listener behind a TCP forwarder,
the listener's own hint is left out, and the operator advertises the
forwarded endpoint with `--advertise`. To rotate the files, replace them and
restart the host.

TLS adds clients that accept only `wss`, and it hides the channel
handshake's plaintext fields from observers on the path. It does not replace
the channel's own authentication or encryption, and a certificate proves
nothing about the host key.

The client's `Connector` tries every selected direct hint in order: a `tcp`
hint over TCP, a `websocket` hint through `client::WebSocketStream`, over TLS
for a `wss` URL. `Link::direct` takes either stream. A `wss` certificate is
verified against the bundled WebPKI roots; `client::WebSocketTls::test_roots`
and `Connector::set_websocket_tls` replace those roots for tests only.
`Connector::set_local_route` gives one host a loopback address that only this
process can use, such as the forwarded port of an SSH tunnel it runs. Each
attempt, and each probe of a relay connection, tries that address over TCP
before the hints. It is same-machine evidence for that one address, so the
connector's locality still governs the hints, and a non-loopback address is
refused.
`client::connect_websocket` dials one URL the same way.

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

## CAP/CJ binding

At start the host publishes its `host-access` capability, a NIP-CAP
`kind:30180` definition with the `d` slug `host-access`, on every relay it
serves. It then subscribes to NIP-CJ execution requests (`kind:25920`)
addressed to its key and answers each with a `kind:26920` result.
`coder_access::cj::intake` checks the binding fields: the CJ signer is the
request's signer, the target, lock, context, and requirements are the
host's, and the input matches the pinned `openagents.host-call.v1` schema.
The embedded request then goes to the same admission as the relay artifact
and direct-channel bindings, so grants, epochs, rights, retained replies, and
refusals are shared. A request ID answered through one binding returns the
same signed reply through another.

A `completed` result carries the signed reply as
`{v: "openagents.host-answer.v1", event}`. It means the operation answered;
the reply says whether it was admitted, and a `dispatched` reply is still
only a handling receipt. A device sends an operation over CJ with
`coder_access::cj::fetch_capability` and `Client::call_cj`.

## Relay binding for terminals

A device seals each NIP-TERM request as a private `3188` artifact to the host,
with the request ID as its mailbox and the body's `v` as its schema. The host
answers only a key it enrolled, only within 60 seconds of the request's issue
time, and seals the result to the device under the same mailbox. An attach
delivers frames as artifacts under the attachment ID. A relay can return
retained frames in any order after a reconnect, so a client feeds them through
`client::Ordered`.

## Limits

- The host reads its TLS certificate and key only at start. It does not
  reload rotated files, check the chain against a root, or warn before the
  certificate expires.
- The CAP/CJ binding answers each request within the request, so it sends
  no `accepted` or progress feedback and ignores status, replay, and cancel
  controls. Execution kinds are ephemeral: a request sent while the host's
  subscription reconnects is lost, and the device retries the same request.
- Terminal state is process-local. A restart reports every terminal as
  `lost`.

## Tests

```sh
cargo test -p coder-host
cargo clippy -p coder-host --all-targets -- -D warnings
cargo fmt -p coder-host --check
```

`tests/cj.rs` runs the CAP/CJ binding against the relay artifact binding;
its [verification record](../../docs/coder/verification/2026-09-27-host-cj-binding.md)
lists what it establishes. `tests/end_to_end.rs` runs the acceptance scenario in
`tests/support/scenario.rs` with an in-memory task owner; the `coder` crate's
`tests/host_serve.rs` runs the same scenario with the durable task inbox.
`tests/websocket.rs` enrolls a device, connects over a `websocket` hint, runs
a terminal command, and sees revocation close the channel.
`tests/wss.rs` does the same over `wss` that the host terminates, checks the
listener's `wss` hint, refuses an untrusted issuer, a wrong name, and plain
`ws`, and refuses to start with unusable TLS files. Its certificates and keys
in `tests/fixtures/tls` are test-only.
The [verification record](../../docs/coder/verification/2026-09-26-host-serve.md),
the
[WebSocket channel record](../../docs/coder/verification/2026-09-27-websocket-channels.md),
and the [host `wss` record](../../docs/coder/verification/2026-09-27-host-wss.md)
list what they establish.

`tests/headless.rs` runs reverse enrollment of a headless host through the
resident host and a local relay: approval, denial, five wrong codes, an
approver without `access_admin`, and an expired request. Its
[verification record](../../docs/coder/verification/2026-09-27-host-generation-and-headless.md)
also covers the shared generation counter.

### Optional Verse assets

Private Verse placement requests require the explicit `verse-assets` feature.
The default host build has no Verse dependencies and returns `Unsupported` for
these requests. Build with `--features verse-assets` when this integration is needed.

`scripts/coderdev` builds only `coder-new` and `microcoder`, not the Verse-bearing
OpenAgents CLI companion. CLI tools can still use an independently installed
`openagents` executable; coderdev does not rebuild it. Run
`scripts/test-coderdev-no-verse.sh` to verify the development dependency graph.
