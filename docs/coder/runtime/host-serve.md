# Host serve

`coder host serve` runs one resident Coder host on a computer. It answers
device enrollment, publishes presence and reachability hints to enrolled
devices, accepts authenticated direct channels, relay-carried operations, and
NIP-CJ execution requests, serves terminals, hands task operations to the durable task inbox, and
publishes activity summaries.
[Issue #9712](https://github.com/OpenAgentsInc/openagents/issues/9712)
delivers it as part of the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).
The implementation is [`crates/coder-host`](../../../crates/coder-host/README.md),
and the [verification record](../verification/2026-09-26-host-serve.md) and
the [WebSocket channel record](../verification/2026-09-27-websocket-channels.md),
and the [host `wss` record](../verification/2026-09-27-host-wss.md) hold the
evidence.

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

`init` also records a WebSocket listener, and the host service serves it with
no arguments: `--listen-websocket ADDR`, `--allow-nonloopback`,
`--advertise CLASS=HOST:PORT|URL`, and the three `--websocket-tls-*` options
take the values `serve` takes, and `init` refuses a combination `serve` would
refuse. [`coder link setup`](../guides/link-devices.md) writes these for a
tailnet listener. A `serve` option replaces the recorded value for that
start; `--listen-websocket` and the TLS options replace the recorded
listener together.

## Run it

```sh
coder host serve
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--relay URL` | The settings from `init` | A relay to serve. Repeat for more. The first is the primary relay. |
| `--workspace LABEL=PATH` | The settings from `init` | An admitted workspace. |
| `--listen ADDR` | `OPENAGENTS_HOST_LISTEN`, else `127.0.0.1:0` | The TCP direct-channel listener. |
| `--listen-websocket ADDR` | None | Also listen for WebSocket direct channels, and advertise the listener as a `websocket` hint. |
| `--websocket-tls-cert FILE`, `--websocket-tls-key FILE`, `--websocket-name NAME` | None | Terminate TLS on the WebSocket listener with this PEM certificate chain and private key, and advertise it as `wss://NAME:PORT/`. Give all three or none. See [Serve `wss` without a forwarder](#serve-wss-without-a-forwarder). |
| `--allow-nonloopback` | Off | Permit a listener on a LAN or tailnet address. |
| `--advertise CLASS=HOST:PORT` or `CLASS=URL` | None | Advertise another `lan`, `tailnet`, or `public` endpoint, such as a forwarder. A `ws` or `wss` URL is a WebSocket endpoint. An endpoint that repeats a listener's own address replaces that listener's hint, so the class you state wins. |
| `--generation N` | `OPENAGENTS_HOST_GENERATION`, else the next counter value | The NIP-REACH host generation. See [Host generation](#host-generation). |
| `--runtime FILE`, `--no-runtime` | `~/.openagents/host/runtime` | The runtime record SSH launchers read. |
| `--tasks DIR` | `~/.openagents/tasks` | The durable task inbox. |
| `--owner KEY` | None | Establish the owner on first start; the same owner is a no-op. |
| `--no-telemetry` | Off | Withhold CPU and memory telemetry from presence; placement then skips the host. |

Every `coder host` command also takes `--state DIR` for the access store,
`--root DIR` for `~/.openagents/host`, and `--loopback-test`, which permits
`ws://` to a numeric loopback relay for fixtures only.

`SIGTERM` or `SIGINT` stops the host: every terminal's process tree ends, and
the runtime record is removed.

### Workspace roots

At start the host checks each workspace root. A root under `--root` that does
not exist is created and logged as `created workspace`. Any other missing root
is logged as `workspace LABEL root PATH is missing`, and the host still
serves. `terminal.open` then answers `unavailable` until the directory exists
again, which needs no restart; a host with no workspace at all answers
`unsupported`. The `openagents computer exec` message names which of the two
happened.

### Host generation

Clients refuse a host generation lower than one they already hold, so a
host root keeps one generation counter, `~/.openagents/host/generation`,
for every way the host starts:

- A standalone `coder host serve` advances the counter and serves as the
  new value.
- The host service launcher advances the same counter, passes the value in
  `OPENAGENTS_HOST_GENERATION`, and names its root in
  `OPENAGENTS_HOST_GENERATION_ROOT`. The host claims that value in that
  root before it serves.
- `--generation N` is claimed the same way.

A claim refuses a value lower than the counter, or one another start
already used, and the host exits before it binds anything. Each change
holds a lock and replaces the record atomically, so concurrent starts get
distinct values, and a crash after the counter advanced skips that value
instead of reusing it. The next value is at least the Unix time in
seconds, so a lost record still yields a higher generation. A damaged
record refuses to start; it is never reset automatically. `coder-service`
owns the counter's rules in `coder_service::generation`.

## Serve WebSocket direct channels

A client that cannot open a raw TCP connection, such as a web client, uses a
WebSocket direct channel. Start the host with a WebSocket listener:

```sh
coder host serve --listen-websocket 127.0.0.1:0
```

The host logs the bound address and advertises it as a hint of the form
`ws://127.0.0.1:PORT/`. The channel runs the same handshake as over TCP: both
keys are proved, the grant is checked, every frame is encrypted and
sequenced, and a revoked grant closes the channel. To reach it from another
machine, bind a LAN or tailnet address with `--allow-nonloopback`, or put a
forwarder in front of it and advertise the forwarder's URL, for example
`--advertise public=wss://host.example/reach`. The host serves every path,
so the forwarder may rewrite the path.

### Serve `wss` without a forwarder

The WebSocket listener can terminate TLS itself. You supply a certificate
chain and its private key as PEM files, for example from `tailscale cert` or
an ACME client. The host never obtains or renews a certificate.

1. Get a certificate for the DNS name that devices dial. For example, on a
   tailnet:

   ```sh
   tailscale cert --cert-file ~/.openagents/host/tls/chain.pem \
     --key-file ~/.openagents/host/tls/key.pem box.example.ts.net
   chmod 600 ~/.openagents/host/tls/key.pem
   ```

1. Start the host with the listener, the files, and the name:

   ```sh
   coder host serve --listen-websocket 100.101.102.103:8443 --allow-nonloopback \
     --websocket-tls-cert ~/.openagents/host/tls/chain.pem \
     --websocket-tls-key ~/.openagents/host/tls/key.pem \
     --websocket-name box.example.ts.net
   ```

The host advertises the listener as `wss://box.example.ts.net:8443/`, in
the same class as the TCP listener's hint, and no longer serves plain `ws`
on it. Before it binds anything, the host refuses to
start when:

- The key file is missing or unreadable, is not a regular file, is not owned
  by your user, or can be read or written by group or others.
- Either file holds no usable PEM item.
- The key does not match the chain's first certificate.
- The first certificate is not valid for `--websocket-name`, which must be a
  DNS name, not an IP address.

The host does not check the chain against a root or the certificate's
expiry; a device checks both when it dials, against the public WebPKI roots,
and a device that cannot verify the certificate tries the next hint. When
the name and the bound address disagree about loopback, for example a
loopback listener behind a TCP forwarder, the host leaves out the listener's
own hint; advertise the forwarded endpoint with `--advertise`.

To rotate the certificate, replace both files and restart the host. The host
reads them only at start.

TLS is an addition to the channel's security, not its basis. The channel
already proves both keys, checks the grant, and encrypts every frame, and a
certificate proves nothing about the host key. TLS lets clients that accept
only `wss`, such as a page served over `https`, connect, and it hides the
handshake's plaintext fields, such as the device key, host key, and grant
ID, from observers on the path. Those observers still see the address, the
TLS server name, and the timing and approximate sizes of messages.

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
on their next check.

### Enroll from a host without a screen

When no one can scan an invitation at the host, ask from the host instead:

```sh
coder host request
```

The command records a NIP-HOST enrollment request, publishes one sealed
copy to the owner and to each device that holds `access_admin`, and prints
the request ID and an eight-character code, such as `code 7KQ4-M2XD`. Read
the code to the person approving; the request never carries it. The running
`coder host serve` answers the approval or denial on the request's relay,
so it must be serving that relay. The command waits until the request is
approved, denied, closed after five wrong codes, or expired after five
minutes, and exits with status 0 only when it is approved. `--relay`
defaults to the first relay from `init`, and `--rights` narrows what the
request asks for; the default is `standard`.

The approver opens the request with
`coder_access::client::pending_enrollments`, types the code, and sends the
approval as the owner or from a device with `access_admin`. The host
refuses an approver without `access_admin`, or a right the approver lacks,
as `missing_right`; a right outside the request as `forbidden`; each wrong
code as `wrong_code`, and every approval after the fifth as
`rate_limited`; and an expired request as `expired`.

## Run it as a service

The host service in [`coder-service`](host-service.md) starts
`coder host serve` by default. Install it with the host's own key, so the
host descriptor and every record the host signs name the same identity:

```sh
coder-service service install --host-key "$(coder host public-key)"
```

Under the service, the host reads `OPENAGENTS_HOST_LISTEN`, claims the
generation from `OPENAGENTS_HOST_GENERATION` in the counter that
`OPENAGENTS_HOST_GENERATION_ROOT` names, and writes the ready record to
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
   tries the selected direct routes, `tcp` and `websocket` hints alike in
   the selected order, then falls back to the relay. Selection never offers
   a loopback address to another machine.
4. A `Link` carries NIP-HOST operations and NIP-TERM requests on either
   route. `fetch_summaries` reads activity summaries after a reconnect.

A device can also send any NIP-HOST operation as a NIP-CJ execution job. The
host publishes its `host-access` capability (`kind:30180`) on every relay it
serves; `coder_access::cj::fetch_capability` reads it from the pinned host
key, and `Client::call_cj` sends the operation. The host admits it exactly as
a relay-carried request, so the same request ID returns the same signed reply
through either binding. A `completed` result means the operation answered;
the reply inside it says whether it was admitted.

A handshake that the host refuses after proving its key blocks the
connection instead of falling back: a revoked grant is revoked on every
route. A relay link's probe fails while a direct route answers, so sending
`Signal::RetryNow` moves a relay connection back to a direct one.

Enrolled devices see this host on their
[Computers screens](../../../crates/coder-computers/README.md). The status
comes from each device's connection supervisor, the Access screen lists
devices with the time the host last saw each one, and a new invitation shows
as a QR code rendered on the device that created it.

## Authority

- Creating a task is an inert submission to the task inbox. It records
  intent and starts nothing; the local owner still needs its own execution
  grant. Pairing and enrollment never grant execution authority. The one
  exception is the owner's [auto-start policy](host-autostart.md), which only
  a command on the host turns on, is off by default, and starts eligible
  tasks within its workspace, concurrency, and engine bounds.
- `task.steer` records a correction and `task.cancel` requests a
  cancellation, with the semantics of the [task owner](task-owner.md). Each
  names the revision the device last read; another revision refuses as
  `stale`.
- Every operation needs one right: `operate` for tasks, `terminal` for
  terminals, and `access_admin` or `access_read` for access operations.
- Summaries carry a generic headline for the phase, such as `Task queued`,
  never a title or prompt a device sent.

## Limits

- The host reads its TLS certificate and key only at start; rotating them
  needs a restart, and the host does not warn before the certificate
  expires.
- Telemetry is coarse and local: CPU use is the load average per CPU, not a
  measured utilization.
- Terminals do not survive a restart; references to them refuse as `lost`.
- The CAP/CJ binding sends no `accepted` or progress feedback and ignores
  status, replay, and cancel controls; each operation answers within its
  request. A CJ request sent while the host reconnects its subscription is
  lost, and the device retries it.
- Evidence is synthetic and from one machine; see the verification record.
