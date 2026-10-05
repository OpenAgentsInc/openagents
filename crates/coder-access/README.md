# Coder Access

Coder Access admits devices to one Coder host with host-wide, scoped rights.
It implements the [NIP-HOST draft](../../nips/openagents/NIP-HOST.md): host
invitations, reverse enrollment for a headless host, host-signed grants with
revocation epochs, delegation, device listing and revocation, and a typed
`task.create` operation. Requests and replies are original signed private
`3188` artifacts over NIP-42 authenticated relay connections. The resident
host in [`coder-host`](../coder-host/README.md) also carries them over direct
channels and inside NIP-CJ execution requests.

The host is the only issuer of access. An invitation, an approval, or relay
delivery introduces a device. Only the host's current grant record admits an
operation, and every operation checks it again.

## Rights

| Right | Permits |
| --- | --- |
| `observe` | Session, task, and file reads under the disclosure policy. |
| `operate` | Create, steer, and cancel tasks and sessions. |
| `terminal` | Open and drive terminals. |
| `review` | Write reviews and diffs. |
| `access_read` | List enrolled devices. |
| `access_admin` | Invite, approve, deny, cancel, and revoke, within held rights. |
| `world` | Join the host's world instances over a direct channel. |

No right implies another. The `standard` preset is `observe`, `operate`,
`terminal`, and `review`. The `admin` preset is `access_read` and
`access_admin`. The pairing grant holds every right except `world`, which
clients built before it existed cannot parse; grant `world` explicitly. A
refusal for a missing right names that right.

## Set up a host

Establish the owner on the host itself. Nothing received over a relay can do
this.

```sh
cargo run -p coder-access -- init --owner <owner-public-key>
```

The owner key accepts lower-case hex or an `npub`. The placeholder is not a
real key. The command creates a host key and a private store under
`~/.openagents/coder-access/`, or the directory passed with `--state` before
the command. The directory has mode `0700`; the key, lock, and state files
have mode `0600`. Initializing again with the same owner does nothing;
another owner is refused. To change the owner, create a new store.

## Enroll a device with an invitation

```sh
cargo run -p coder-access -- invite --relay wss://relay.example/ --rights standard
```

The host saves the invitation, then prints its ID and a `coder-host:` paste
string, and draws a QR code in the terminal. `--no-qr` skips the QR code.
The invitation admits one device and expires after five minutes. Show it
only to the device you are enrolling: whoever redeems it first becomes that
device. `--grant-secs` sets the grant lifetime, seven days by default and at
most 30 days. `cancel --invitation ID` cancels an unused invitation.

On the device, redeem it with the device's own key and save the result only
on success:

```rust,ignore
let access = coder_access::client::redeem(&scanned, &device_secret, RelayPolicy::Production).await?;
let client = coder_access::Client::device(access, device_secret, RelayPolicy::Production)?;
```

A same-device retry, including after a host restart, returns the same grant.
Another device is refused.

## Enroll with a short code

Use reverse enrollment when the host can't show a scannable code:

```sh
cargo run -p coder-access -- request --relay wss://relay.example/ --rights standard
```

The host saves the request, publishes it encrypted to the owner and to each
device that currently holds `access_admin`, and prints an eight-character
code. It then serves the relay until the request is approved, denied, or
expires after five minutes.

On a device that holds the owner key, or on an administrator device, approve
it with the code shown on the host:

```sh
CODER_ACCESS_KEY_FILE=<private-key-file> cargo run -p coder-access -- approve \
  --relay wss://relay.example/ --host <host-public-key> \
  --device <new-device-public-key> --code <code>
```

`CODER_ACCESS_KEY_FILE` names a private, singly linked `0600` file holding the
approver's hex secret key. Commands never take a secret as an argument. An
administrator device also passes `--access FILE`, its saved access record.
`--rights` narrows the grant, `--deny` denies instead, and `--enrollment ID`
selects one of several requests. The command prints the host-signed grant
envelope for the new device. After five wrong codes, the host closes the
request.

## List, revoke, and serve

Each listed device carries `last_seen`: when the host last admitted a request
or, through `Host::touch`, a direct channel from it. The host records it
itself, at most once a minute for an open channel.

```sh
cargo run -p coder-access -- list [--json]
cargo run -p coder-access -- revoke --device <device-public-key>
cargo run -p coder-access -- serve-once --relay wss://relay.example/
```

Revocation marks every grant of that device revoked and advances its epoch
before the command returns. Later requests refuse as `revoked`, and a request
that names an old epoch refuses as `stale`. Revocation cannot erase data a
device already received.

A device key holds one grant. When a device enrolls again (it redeems another
invitation, or an administrator approves it again), the new grant supersedes
its earlier ones in the same commit: they are revoked without advancing the
device's epoch, and they leave the store once no retained invitation or
enrollment names them. Grants other devices hold, including ones this device
delegated, are untouched. An unredeemed invitation the device issued under a
grant that was live until then moves to the new grant only when the new grant
holds `access_admin` and every invited right and outlives the invited grant;
otherwise it is refused as `revoked`. A grant that was already revoked hands
on nothing.

`serve-once` answers one request and exits; use it for tests. A resident host
service composes the library's `host::serve` loop. The global
`--loopback-test` option permits `ws` to a numeric loopback address for
synthetic fixtures only.

## Library

Build with `default-features = false` for a thin or mobile client. That build
contains the rights, protocol, and client modules without the host store or
QR rendering, and uses `coder-connect` without its host feature.

- `client::redeem`, `prepare_redeem`, and `finish_redeem` redeem invitations.
- `Client::device` and `Client::owner` prepare, send, and verify operations.
  `prepare` returns the exact signed packet; resend it unchanged to retry.
- `client::pending_enrollments` and `OpenedEnrollment::approve` or `deny`
  build reverse-enrollment decisions.
- `Access::from_authorization` accepts a grant envelope an approver forwarded.
- `cj` is the CAP/CJ binding. `Capability` builds the host's `host-access`
  NIP-CAP definition, pins the
  [`host-call.v1`](../../nips/openagents/schemas/host-call.v1.json) and
  [`host-answer.v1`](../../nips/openagents/schemas/host-answer.v1.json)
  schemas, and signs the `kind:30180` manifest. `cj::intake` checks a
  `kind:25920` request's binding fields before the host admits the embedded
  request, and `Answering::answer` seals the result. On a device,
  `cj::fetch_capability` reads the manifest from the pinned host, and
  `Client::call_cj` or `send_cj` sends an operation over CJ and verifies the
  reply exactly as `send` does. A `completed` CJ result means the operation
  answered, not that a task ran.
  [`fixtures/host-access-capability.json`](fixtures/host-access-capability.json)
  is a reference definition for a placeholder host key.
- `host::Host` owns the store. `host::Dispatch` connects `task.create`,
  `task.steer`, `task.cancel`, and `terminal.open` to their owners; the
  default `Unconnected` dispatcher refuses them as `unavailable`. The
  resident host in `coder-host` dispatches them to the task inbox and its
  terminal host, and signs everything with `Host::signing_key`, so the host
  has one identity.

Offline verification of a saved access record checks identity and declared
expiry only. It cannot establish that the host has not revoked it.

## Reuse

The crate reuses `coder-connect` for the invitation byte layout, the private
store, private-artifact sealing and opening, relay policy, and the finite
relay session and receiver, and `nostr-transport` for artifact publication.
The history observer keeps working unchanged. A `coder-pair:` observer
invitation is never host access, and a `coder-host:` invitation is never
observer access.

## Bounds

The store retains at most 128 grants, 64 invitations, 32 enrollment
requests, 1,024 retained replies, and 1,024 device epochs. A dead grant
(revoked, superseded, expired, or at an old epoch) never blocks a new one:
while the store holds 128 grants, the one that stopped being live longest ago
leaves it, unless a retained invitation or enrollment names it. Only 128 live
or named grants refuse a new device, as `bounds`. Requests are valid
for at most 60 seconds. Invitations and enrollment requests live five
minutes. An invitation admits at most 32 replies. Revocation tombstones are
kept until grant expiry plus the request window.

## Verification

```sh
cargo test -p coder-access
cargo clippy -p coder-access --all-targets -- -D warnings
cargo clippy -p coder-access --no-default-features --lib -- -D warnings
cargo fmt -p coder-access --check
```

`tests/capability.rs` checks the reference definition against the NIP-CAP
validator and the checked-in schemas. The fixtures use throwaway keys and the shared synthetic NIP-42 relay from
`coder-control`. The [verification record](../../docs/coder/verification/2026-09-26-host-access.md)
states what they establish and what they don't.
