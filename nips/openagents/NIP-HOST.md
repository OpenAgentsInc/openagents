# NIP-HOST — Host-wide device enrollment and scoped access

`draft` `optional` — v1, 2026-09-26; amended 2026-09-29 with
[connect codes](#connect-codes), [enrollment over iroh](#enrollment-over-iroh),
the [local operator socket](#local-operator-socket), and
[nearby approval](#nearby-approval-planned) for the
[QR pairing design](../../docs/coder/design/2026-09-29-auto-pairing.md). The
[shared contracts](contracts.md)
are normative. This profile lets one host admit a device with host-wide,
scoped rights. It defines enrollment by host invitation, reverse enrollment
for a headless host, grants with revocation epochs, delegation, device
listing and revocation, and a typed task-creation operation. It defines no
new event kinds.

A host is one machine's resident agent host. A device is any client key that
uses it: a phone, a desktop client, a terminal, or another host acting as a
client. The host is the only issuer of access. A relay, an invitation, an SSH
login, or network membership can introduce a device; none of them is a login.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [CTRL](NIP-CTRL.md) | CTRL scopes a grant to one task and one controller generation. HOST grants are host-wide. A HOST grant does not satisfy a CTRL scope check, and a CTRL grant does not admit any HOST operation. HOST adds `task.create`, which CTRL deliberately leaves out. |
| [SESS observer profile](NIP-SESS.md#read-only-observation-of-retained-foreign-history) | The observer profile admits selected retained-history roots for reading. HOST reuses its invitation byte layout under a different prefix, its request/reply mailbox pattern, and its retry rules. An observer grant is not a HOST grant, and a HOST `observe` right does not add history roots. |
| [CAP](NIP-CAP.md) | A host describes the HOST operations as CAP operation roles. Discovery never admits an operation. CAP forbids public local presence; HOST publishes no presence. |
| [CJ](NIP-CJ.md) | The CAP binding below invokes HOST operations through CJ execution v1. A CJ `completed` result means the HOST operation answered; the embedded reply states whether it was admitted. |
| [POL](NIP-POL.md) | Disclosure, action approvals, spending, and publication stay POL decisions. No HOST right approves a POL action, and `operate` does not raise a budget. |
| [REACH](NIP-REACH.md) | Owner host directory, presence, reachability hints, and direct channels. A direct channel binds to one HOST grant ID and epoch. A route, address, iroh `EndpointId`, or tailnet membership never grants access. The opt-in [tailnet admission](#tailnet-admission) uses Tailscale identity to hand out an invitation, never a grant. [Enrollment over iroh](#enrollment-over-iroh) carries a redemption on an iroh connection; the host's grant check still decides. |
| [TERM](NIP-TERM.md) | Terminal sessions, streams, and replay. TERM requires the HOST `terminal` right and defines no grant of its own. |
| [RUN](NIP-RUN.md) and [ENV](NIP-ENV.md) | A task admitted through `task.create` is recorded and executed under the host's task owner, RUN, and ENV. HOST admission is not execution evidence. |
| Official [NIP-46](../official/46.md) | A remote signer can hold the owner key. A permission to sign is not a HOST right; the host still checks the signer against its locally established owner. |
| Block [NIP-OA](../block/NIP-OA.md) and [NIP-AA](../block/NIP-AA.md) | Relay admission for agent keys. Relay membership never admits a HOST operation. |

## Encoding, principals, and limits

Every artifact defined below contains `v`, `requires: []`, and optional inert
`meta` only where specified; this version specifies none. Reject unknown
fields, enum values, versions, duplicate keys, and nonempty `requires`.
Common IDs are random 64-hex values. Pubkeys are 64-hex x-only keys.
Timestamps are Unix seconds. Host time, not event `created_at`, enforces
expiry. Bodies are at most 128 KiB and use the common nesting limit.

There are three principals:

- The **owner** is the key whose authority the host serves. The host
  establishes the owner relationship locally, for example with an operator
  command on that machine, a request on its
  [local operator socket](#local-operator-socket), or a desktop app that
  creates an owner key in the machine's keychain on first run. An
  invitation, approval, relay event, scan, network route, or display name
  cannot establish or change it. The owner holds every right at its host
  without a grant. The host refuses a request signed by any other key without
  a grant.
- The **host** key signs grants, replies, and enrollment requests. It is
  distinct from the owner key.
- A **device** key holds grants. It is distinct from both the host and the
  owner. A device label is optional display metadata, never an identity.

Keys stay in their own protected stores. Enrollment never copies a secret key.

## Rights

The exact rights are:

| Right | Permits |
| --- | --- |
| `observe` | Session, task, and file reads, only under the host's disclosure policy and the reading profile's own checks. |
| `operate` | Create, steer, and cancel tasks and sessions. |
| `terminal` | Open and drive terminals. |
| `review` | Write reviews and diffs. |
| `access_read` | List enrolled devices and their states. |
| `access_admin` | Issue invitations, approve or deny enrollment requests, cancel invitations, and revoke devices, within the rights the administrator holds. |

A grant carries a nonempty, duplicate-free list in the order shown above.
Any other order is malformed. No right implies another: `access_admin` does
not include `access_read`, and `operate` does not include `observe`. A
**standard device grant** is `observe`, `operate`, `terminal`, and `review`;
it excludes both access rights.

A right is necessary, never sufficient. The profile that defines an effect
still applies its own checks. For example, a terminal stream needs a
[TERM](NIP-TERM.md) session, and a file read still passes the disclosure
policy.

## Host invitations

A host invitation lets the host show a code that one device redeems. The
host's operator, or a device with `access_admin`, creates it.

### Invitation string

The string is `coder-host:` followed by unpadded base64url of these bytes,
in order: version byte `1`; host x-only public key (32 bytes); invitation ID
(32 random bytes); capability (32 independently random bytes); issue time
(8-byte unsigned big-endian seconds); expiry (8-byte unsigned big-endian
seconds); relay URL byte length (2-byte unsigned big-endian); and the exact
UTF-8 relay URL (1–256 bytes). No trailing bytes are allowed. Encoded strings
are at most 640 bytes. Expiry is exactly 300 seconds after issue. A QR code
carries the same string.

This is the SESS observer layout under a distinct prefix. A `coder-pair:`
string is never a host invitation, and a `coder-host:` string is never an
observer invitation.

### Connect codes

A **connect code** is a second carriage of the same host invitation, for a
host that serves [enrollment over iroh](#enrollment-over-iroh). The
companion desktop app shows it as a QR code. The string is
`openagents-connect:` followed by unpadded base64url (RFC 4648, section 5,
no `=`) of these bytes, in order:

| Bytes | Field | Bound |
| --- | --- | --- |
| 1 | Version | Exactly `1`. Any other value refuses the code. |
| 32 | Host x-only public key | The host's Nostr key. |
| 32 | Host `EndpointId` | The host's iroh Ed25519 public key. |
| 32 | Invitation ID | Random. |
| 32 | Capability | Independently random. |
| 8 | Issue time | Unsigned big-endian Unix seconds. |
| 8 | Expiry | Unsigned big-endian Unix seconds; exactly issue time plus 300. |
| 1 | iroh relay URL length | 0–128. Zero means no relay. |
| 0–128 | iroh relay URL | UTF-8 `https` URL without user information, query, or fragment. |
| 1 | Direct address count | 0–8. |
| 7 or 19 each | Direct address | A family byte (`4` or `6`), the address (4 or 16 bytes), and a nonzero port (2 bytes, big-endian). |
| 1 | Label length | 0–48. |
| 0–48 | Label | UTF-8 without control characters: the computer's name, for display only. |

No trailing bytes are allowed. The decoded payload is at most 476 bytes, so
the string is at most 654 bytes. A reader refuses the whole code, before it
dials anything, when the version is unknown, a length exceeds its bound, the
bytes end early or run past the end, the expiry is not exactly 300 seconds
after issue, a family byte is not `4` or `6`, a port is zero, two direct
addresses are equal, an address is unspecified, multicast, or broadcast, the
relay URL is not a valid `https` URL under the rules above, or the label is
not valid UTF-8 or has a control character. A reader on another machine
skips loopback direct addresses, as [REACH selection](NIP-REACH.md#selection)
does. The host key and the `EndpointId` are never the all-zero value.

The invitation ID, capability, times, rights, grant expiry, and issuer are
exactly the [host records](#host-records) of one invitation, and the
capability rules above apply unchanged. Only the carriage is new. The code
does not name a Nostr relay; the device learns it during
[enrollment over iroh](#enrollment-over-iroh), and the grant names it. The
`EndpointId`, relay URL, direct addresses, and label are routing and display
hints: none of them is an identity or a right.

A device does not refuse a connect code by its own clock. The host decides
expiry at redemption (see [Enrollment over iroh](#enrollment-over-iroh) for
clock skew).

Compatibility with `coder-host:` strings:

- An `openagents-connect:` string is never a `coder-host:` or `coder-pair:`
  string, and the reverse holds too. A scanner that accepts host invitations
  accepts both prefixes and parses each strictly by its own layout.
- A `coder-host:` string keeps its layout, its relay, and its redemption over
  the [bindings](#bindings) below. Hosts that predate this amendment are
  unchanged.
- A host that serves enrollment over iroh accepts `enroll.redeem` there for
  any current invitation, whichever carriage showed it.
- When iroh cannot connect, a device may redeem a connect code over the
  direct artifact binding on its configured default relay. That succeeds only
  when the invitation's relay is that relay; otherwise the host earns no
  signed reply, and the device reports that it could not reach the computer.
  A desktop app host records the default relay the phone app uses,
  `wss://relay.openagents.com/`, as its invitations' relay unless its
  operator chose another.

### Showing a connect code

A connect code is a bearer secret until it is redeemed. A host that shows
one follows these rules, in addition to those for the capability above:

- It shows the code only inside its own visible window, on an unlocked
  screen, and generates the QR image locally.
- It shows a new code every 60 seconds and cancels the code it replaced 60
  seconds after the replacement, with `invite.cancel` semantics. So no code
  is redeemable more than 120 seconds after it left the screen, and the
  300-second invitation life is only ever shortened.
- Hiding the window, locking the screen, ten idle minutes, or a successful
  redemption cancels every outstanding connect code.
- It copies the text form to the clipboard only on an explicit tap, and marks
  the clipboard entry to expire after 60 seconds where the platform allows.
- It never writes the string, the capability, or the QR image to a log, a
  file, a URL, or telemetry.

Rights for a connect code are fixed:

- The invitation's rights are `observe` and `operate`, plus `terminal` only
  when the person set **Let this phone open a terminal on this Mac** before
  the code was shown. Changing the checkbox cancels the shown code and shows
  a new one. A connect code never carries `review`, `access_read`, or
  `access_admin`.
- The grant expiry is 30 days after issue, the most a grant allows.
- The issuer is the host itself, as a local operator action.

A host refuses to show a connect code for any other rights list. A device
that receives a grant whose rights differ from these refuses it and keeps no
access record. To change rights, the person removes the device and pairs
again; a rights change in place is not defined.

The capability is a temporary bearer secret until redemption. Show it only
to the enrolling device. It must not appear in a public event, URL, log,
telemetry, or remote QR-generation service. The relay URL follows the shared
relay policy: `wss` with certificate validation and no credentials, query,
or fragment. A scanned string cannot enable a loopback test profile.

### Host records

Before displaying an invitation, the host durably stores its ID, the digest
of its capability, the relay, issue and expiry times, the rights, the grant
expiry, and the issuer: the host itself for a local operator action, the
owner, or the issuing device and its grant ID. It never stores the
recoverable capability. The grant expiry must follow the invitation expiry
and be at most 30 days after issue. A host retains at most 64 invitations.

### Redemption

A redemption is a request (see [Operations](#operations)) with no grant and
the operation `{kind: "enroll.redeem", invitation, capability}`. It is an
original signed private `3188` artifact from the device to the host, and its
mailbox is the request ID.

Under one durable lock, the host checks the original signature, the
capability digest, the relay, and the time window. An unknown invitation or
wrong capability earns no signed reply. For a matching capability, the host
refuses with a signed reply when:

- the signer is the owner or the host (`forbidden`);
- the invitation is cancelled (`revoked`) or expired (`expired`);
- the request lies outside the invitation window (`forbidden`);
- a delegating issuer's grant is no longer current or no longer holds the
  invitation's rights (`revoked`); or
- another device already redeemed it (`forbidden`).

Otherwise the host binds the first valid redemption to the signer. It
atomically stores the consumption, the new grant, and the exact signed reply
before sending anything. Same-device retries, including after a restart,
return the same grant without extending it. Revoking that grant also stops
retries from disclosing it. The host bounds admitted replies to 32 per
invitation. Uncertain persistence returns no reply. An unused invitation can
be cancelled; a redeemed one needs device revocation.

## Enrollment over iroh

A host with an iroh endpoint accepts redemption on the ALPN
`openagents/enroll/1`. iroh is transport: its TLS handshake proves that the
far end holds the `EndpointId` the device dialed, and nothing more. An
`EndpointId`, the device's or the host's, never admits a request, never
names a principal, and is never recorded as a device's identity.

The host's iroh secret key is generated independently of every Nostr key and
kept in the same protected store as the host key. The host uses a relay only
from its own configuration; it uses no third-party relay or address lookup
unless its operator configures one.

The device dials the `EndpointId` from the connect code, with the code's
relay URL and direct addresses as its only address sources, and opens one
bidirectional stream. A connection carries exactly that one stream; the
host resets any other. Each message on the stream is a 4-byte unsigned
big-endian length followed by that many bytes of strict JSON, at most 65,536
bytes. In order:

1. The device sends `openagents.connect-enroll-open.v1`:
   `{v, requires: [], invitation}`, the invitation ID from the code.
2. The host answers `openagents.connect-enroll-info.v1`:
   `{v, requires: [], host, relay, now}`, its host key, the invitation's
   relay, and its current Unix time. For an invitation ID it does not retain,
   it finishes the stream with no answer.
3. The device refuses, and sends nothing more, when `host` differs from the
   code's host key. When `now` differs from its own clock by more than 60
   seconds, it sends nothing more and tells the person how far off the
   phone's clock is, so a clock problem names itself instead of reading as
   `expired`.
4. The device sends `{v: "openagents.host-call.v1", event}`, whose `event` is
   the exact signed `enroll.redeem` request of the
   [direct artifact binding](#bindings), naming that relay.
5. The host answers `{v: "openagents.host-answer.v1", event}` with the exact
   signed reply, or finishes the stream with no answer when the direct
   artifact binding would send none.
6. Both sides finish the stream, and the device closes the connection.

In these messages, `invitation` and `host` are lowercase 64-hex values,
`relay` follows the invitation's relay rules (1–256 bytes), and `now` is an
unsigned integer of Unix seconds no greater than 2^53 − 1.

The host admits the call exactly as in [Redemption](#redemption), under the
same lock and records: the original signature, the capability digest, the
time window, and the refusals listed there. The relay check compares the
request's `relay` with the invitation's relay, although the request did not
travel over that relay. An unknown field, enum value, or version, a
nonempty `requires`, a message over the bound, or messages out of this order
end the stream with no answer. The host bounds concurrent enroll
connections, 16 in the reference, and each connection's life, 30 seconds in
the reference.

The device accepts the answer only when the reply is signed by the code's
host key, correlates with its request, and carries a grant that names that
host key, the device's own key, the relay from step 2, and the
[connect code rights](#showing-a-connect-code). Only then does it store its
access record, with the host's `EndpointId`, relay URL, and last direct
addresses beside it. Retries, including after a lost answer, resend the same
signed request on a new stream and return the same grant, as the direct
artifact binding does.

Clock rules are unchanged. A reader accepts an `issued_at` up to 60 seconds
ahead of its clock and holds `expires_at` strictly; the host's clock decides
invitation and request expiry.

## Reverse enrollment for a headless host

A headless host can't show a code to a camera, or no one is at its screen
when the device is. It can still show a short human code, for example in an
SSH session.

### Enrollment request

The host creates `openagents.host-enrollment-request.v1`:

| Field | Meaning |
| --- | --- |
| `enrollment` | Common ID. It is also the mailbox. |
| `host`, `owner` | The host key and its locally established owner. |
| `relay` | The exact relay the host serves. |
| `rights` | The most the host asks to grant. |
| `issued_at`, `expires_at` | At most 300 seconds apart. |

The request never contains the short code. The host seals one copy to the
owner and one to each device that currently holds `access_admin`, at most 16
copies. Every copy has the same artifact bytes and therefore the same digest.

The short code is 8 characters from the Crockford base32 alphabet
(`0123456789ABCDEFGHJKMNPQRSTVWXYZ`), displayed as two groups of four. That
is 40 random bits. Hosts normalize input by case, separators, and the
letters `O`, `I`, and `L`. The host stores only
`SHA-256("openagents.host-enrollment-code.v1:" || enrollment || ":" || code)`
and persists the request before displaying the code. A host retains at most
32 enrollment requests.

### Approval and denial

An approver is the owner or a device whose current grant holds
`access_admin`. It opens the request from the host it independently pins,
checks the signer, recipient, mailbox, and expiry, and asks the person to
type the code shown on the host. It then sends:

- `{kind: "enroll.approve", enrollment, request_digest, code, device, rights, grant_expires_at}`
- `{kind: "enroll.deny", enrollment, request_digest}`

`request_digest` is the digest of the exact request artifact bytes. `device`
is the key to admit. It can be the approver's own device key when the
approver acts with the owner key, or another device the administrator is
admitting.

The host admits an approval only when all of these match:

1. The signer is the owner or holds a current `access_admin` grant.
2. The digest names the exact pending request at this host and relay.
3. The request has not expired, been denied, or been closed.
4. The code digest matches. Each mismatch counts; after 5 mismatches the host
   closes the request and later approvals refuse as `rate_limited`.
5. `device` is distinct from the host and owner keys.
6. `rights` fit within both the request's rights and the approver's rights.
   A right the approver lacks refuses as `missing_right` naming it; a right
   outside the request refuses as `forbidden`.
7. The grant expiry is in the future, at most 30 days away, and no later
   than the approver's own grant expiry.

The host atomically records the approval and the new grant before replying.
The reply carries the host-signed grant envelope encrypted to `device`; the
approver forwards it when `device` is another key. Repeating the approval
for the same device returns the same grant. An approval for another device
refuses as `conflict`. Denial is terminal: later approvals refuse as
`denied`. Only the host's own records decide the outcome; the request's
public tags reveal a relationship between keys, not its state.

## Nearby approval (planned)

Planned; not part of v1 conformance. A device on the same network as a host
can ask to enroll without a code on the screen. The person approves on the
host after comparing a six-digit confirmation code shown on both screens.

The host publishes its `EndpointId` and addresses through multicast DNS under
the service name `openagents` (`_openagents._udp`); the record carries
nothing else. The exchange runs on `openagents/enroll/1`, with the same
message framing as [enrollment over iroh](#enrollment-over-iroh):

1. The device sends `openagents.connect-nearby-request.v1`:
   `{v, requires: [], device, label, commitment}`: its device key, a display
   label of 0–48 bytes without control characters, and
   `commitment = SHA-256(nonce_d)` in lowercase hex, where `nonce_d` is 32
   random bytes.
2. The host answers `openagents.connect-nearby-offer.v1`:
   `{v, requires: [], host, nonce, now}` with its host key and a random
   64-hex `nonce_h`.
3. The device sends `openagents.connect-nearby-reveal.v1`:
   `{v, requires: [], nonce}` with `nonce_d`, and the host checks it against
   the commitment.
4. Each side computes the code: the first 8 bytes, as an unsigned big-endian
   integer, of SHA-256 of `openagents.connect-sas.v1`, a zero byte, the host
   `EndpointId`, the device `EndpointId`, the host key, the device key,
   `nonce_h`, and `nonce_d` (keys and nonces as raw bytes), modulo 1,000,000,
   shown as six digits in two groups of three.
5. The host shows the device's label and the code with **Connect** and
   **Don't connect**, and the connect-code terminal checkbox. It admits the
   device only after the person clicks **Connect**.
6. On **Connect**, the host signs a grant with origin `approval`, a random
   enrollment ID, the host as issuer, and the
   [connect code rights](#showing-a-connect-code), and sends the grant
   envelope, encrypted to the device key, on the same stream, as an approval
   reply does. The grant is useless to anyone who lacks that device key,
   because every later request needs the device's signature.

In these messages, `device`, `host`, `commitment`, and both nonces are
lowercase 64-hex values, and `now` is bounded as in enrollment over iroh.
Each message is at most 4,096 bytes.

The commitment keeps either side from choosing its nonce after seeing the
other's, so a device in the middle gets one guess in a million per attempt.
The host keeps one nearby request pending at a time, accepts at most five per
ten minutes, and drops a request the person has not answered within 120
seconds. **Don't connect**, a mismatch, or a timeout grants nothing and
earns no signed reply.

## Tailnet admission

Tailnet admission lets a device on the operator's own tailnet enroll
without a QR code or a short code. It is off unless the host's operator
turns it on with an explicit rights list, for example
`coder host serve --tailnet-admission standard`.

The host listens on its own tailnet IPv4 address, port 47109 by default,
and bounds concurrent exchanges. Each exchange is one line of JSON in each
direction over TCP, within 10 seconds:

- The device sends `openagents.host-tailnet-admission-request.v1`:
  `{v, requires: [], chats}`, at most 1,024 bytes. `chats` asks for a chat
  invitation too.
- The host answers `openagents.host-tailnet-admission.v1`:
  `{v, host, label, invitation, chats, refused}`.

The host refuses, with no invitation, when:

- the request is malformed or names a required feature (`malformed`);
- the caller's address is outside Tailscale's ranges, `100.64.0.0/10` and
  `fd7a:115c:a1e0::/48` (`not_tailnet`);
- its local `tailscale whois` cannot name the caller (`unavailable`);
- the caller is a tagged device (`tagged`); or
- the caller's Tailscale user differs from the user that owns the host
  machine in `tailscale status` (`not_owner`).

Otherwise the host issues an ordinary single-use host invitation with the
operator's rights and a grant expiry of seven days, exactly as
`invite.create` would, and returns it as `invitation`. The device redeems it
over the relay with `enroll.redeem`, so the host signs the grant, and
epochs, revocation, delegation limits, and device listing are unchanged.
The invitation is a bearer capability for its five minutes, delivered only
over the WireGuard-authenticated tailnet connection to the identified
device. `host` is the host key the invitation names and `label` is the
machine's tailnet name for display; neither is an identity.

When `chats` is requested and the host serves retained history, the answer
also carries a `coder-pair:` invitation from the read-only observer in
`coder-connect` (the SESS observer profile). That grant is separate: it
admits only history reads, and a HOST grant still never admits one. The
host then runs the observer in-process on its primary relay, and serves
the same sealed observer requests on direct connections to this listener,
as NIP-SESS's [direct tailnet transport](NIP-SESS.md#direct-tailnet-transport)
defines: a line naming `openagents.history-observer-direct.v1` first, then
frames. The listener welcomes only the callers it would admit, and each
request still carries its own grant.

Tailscale identifies the caller; it never decides access by itself. The
operator's opt-in, the explicit rights, and the host's signature on every
grant do. A device that loses its grant, or a revoked device, can ask again
only while it is still the same Tailscale user's untagged device on the
tailnet and the operator keeps admission on. Turning admission off stops new
invitations; it does not revoke grants already issued.

`crates/coder-host/src/tailnet.rs` implements the listener and the client
request; `crates/openagents-mobile` probes each device on the tailnet and
redeems what it receives.

Tailnet admission is planned for deprecation once
[nearby approval](#nearby-approval-planned) ships; see the
[QR pairing design](../../docs/coder/design/2026-09-29-auto-pairing.md#names-one-surface).

## Local operator socket

A host may serve a local control socket for the owner's actions on that
machine. A caller the socket admits is the host's local operator: its
requests are commands on the host, with the same authority as an operator
command, and the host records itself as the issuer of what they create.

- On macOS the socket is
  `~/Library/Application Support/OpenAgents/control.sock`; on Linux,
  `$XDG_RUNTIME_DIR/openagents/control.sock`. Its directory has mode `0700`
  and the socket `0600`. On Windows it is a named pipe whose security
  descriptor admits only the current user.
- On every accepted connection the host reads the peer's user ID
  (`getpeereid` on macOS, `SO_PEERCRED` on Linux) and closes the connection,
  before reading a message, when it differs from the host's own.
- The socket is never bound to a network address, forwarded, or reachable
  through a relay, iroh, or a direct channel.
- Over it, the operator creates and cancels invitations (connect codes
  included), lists and revokes devices, gets and sets the auto-start policy
  and projects, and reads status. The message types belong to the
  implementation; this profile fixes only who may use the socket and what
  it can change.

A device never gains operator authority: no grant, right, or channel opens
the socket, and a device request cannot turn auto-start on or widen it.

## Grants, epochs, and revocation

A grant is `openagents.host-grant.v1`:

| Field | Meaning |
| --- | --- |
| `grant` | Common ID and mailbox of the grant envelope. |
| `host`, `owner`, `device` | Three distinct keys. |
| `relay` | The exact admitted relay. |
| `rights` | Canonical rights list. |
| `epoch` | The device's revocation epoch when the grant was issued. |
| `origin` | `{kind, id, issuer}`: `invitation` or `approval`, the invitation or enrollment ID, and the authorizing key. |
| `issued_at`, `expires_at` | At most 30 days apart. |

The host signs the grant and encrypts it to the device. The device's saved
access record, `openagents.host-access.v1`, contains the grant and the
original envelope. Offline verification establishes identity and declared
lifetime only. It cannot establish that the host has not revoked the grant.

A copied grant is not a bearer credential. Every operation needs the named
device's own signature and the host's current record.

The host keeps one revocation epoch per device key, starting at zero.
Revoking a device marks all of its grants revoked and advances its epoch by
one, atomically, before answering. Re-enrollment issues a new grant at the
new epoch. Every request names its grant and epoch. The host refuses:

- a revoked grant as `revoked`;
- an expired grant as `expired`;
- a request whose epoch differs from the grant's epoch, or a grant whose
  epoch differs from the device's current epoch, as `stale`.

Keep revocation tombstones until at least grant expiry plus the 60-second
request window. Keep device epochs as long as any grant for that device
could be presented. Restart never resets an epoch or restores a revoked
grant. Revocation also discards the device's retained replies so a retry
cannot disclose them. It cannot erase data already delivered or prove that
an admitted operation stopped.

## Delegation

A device with `access_admin` can issue invitations and approvals only for
rights it holds, and only with a grant expiry no later than its own. A
delegated invitation remains valid only while its issuer's grant stays
current and still holds those rights; redemption rechecks this. Delegation
never widens: no chain of invitations produces a right that the first
issuer lacked. Only the owner holds every right without a grant.

## Operations

A request is `openagents.host-request.v1`:

| Field | Meaning |
| --- | --- |
| `request` | Common ID, stable through retransmission. It is the mailbox. |
| `host` | The pinned host key. |
| `grant`, `epoch` | The grant ID and epoch, or both null for a redemption or an owner request. |
| `relay` | The exact relay. |
| `issued_at`, `expires_at` | At most 60 seconds apart, and within the grant. |
| `op` | One operation below, tagged by `kind`. |

| Operation `kind` | Required right | Answer |
| --- | --- | --- |
| `enroll.redeem` | None; the invitation capability | `granted` |
| `enroll.approve` | `access_admin` | `granted` |
| `enroll.deny` | `access_admin` | `denied` |
| `invite.create` | `access_admin` | `invitation` |
| `invite.cancel` | `access_admin` | `cancelled` |
| `device.list` | `access_read` | `devices` |
| `device.revoke` | `access_admin` | `revoked` |
| `task.create` | `operate` | `dispatched` |
| `task.steer` | `operate` | `dispatched` |
| `task.cancel` | `operate` | `dispatched` |
| `task.archive` | `operate` | `dispatched` |
| `task.command` | `operate` | `dispatched` |
| `task.queue` | `operate` | `queue` |
| `terminal.open` | `terminal` | `dispatched` |
| `workspace.list` | `operate` | `workspaces` |
| `spend.list` | `operate` | `spends` |
| `spend.settle` | `operate` | `settled` |

`task.create` carries `{title, prompt, workspace}`. The title is at most 200
bytes, the prompt at most 16 KiB, and the workspace a host-scoped label of at
most 128 bytes, never a path. `terminal.open` carries `{cols, rows}`, each
1–1,000; [TERM](NIP-TERM.md) defines the session and stream.
`workspace.list` carries nothing and returns `{workspaces}`: the labels
`task.create` accepts on this host, sorted and distinct, at most 64, each
1–128 bytes without control characters. The roots they name stay on the
host. A host with no task owner refuses it as `unavailable`, and an older
host that predates it refuses it as `malformed` or `unsupported`; a client
then asks for the label instead of offering a list.
`task.steer` carries `{task, revision, prompt}` and `task.cancel` carries
`{task, revision, reason}`: the host-issued task ID from a `task.create`
receipt, the task revision the device last read, and a replacement prompt of
at most 16 KiB or a single-line reason of at most 512 bytes. They follow the
[CTRL](NIP-CTRL.md) steer and cancel semantics of the host's task owner: a
steer records replacement instructions and supersedes a running context, a
cancel requests a stop, and another revision refuses as `stale`. Neither
grants execution authority. A host answers
either only after the effect's owner accepts it.
`task.command` carries `{command}`, one durable task command:
`{command, task, action, based_on, text, emulate, issued_at}`. `command` is a
64-hex ID the device mints once and replays unchanged in every later request,
however long it was offline; the host keys its command journal by the device
key and that ID, so a replay returns the recorded disposition and never runs
the command twice, and different content under the same ID refuses as
`conflict`. `action` is `send`, `queue`, `steer`, `interrupt`, or `answer`,
chosen from task state and the device's rights, never from the text.
`based_on` is the task revision the device last read, `text` the message of
at most 16 KiB (for `interrupt`, a single-line reason of at most 512 bytes),
`emulate` true only on a `steer` whose caller chose the engine's emulated
steering, and `issued_at` when the device minted the command. The host
evaluates a task's commands in arrival order:

1. A decided command returns its outcome.
2. A command more than 24 hours past `issued_at` expires (`expired`); one
   dated more than five minutes ahead refuses as `bounds`.
3. A newer `interrupt` supersedes an older undecided one, and only an
   interrupt; an interrupt never runs, queues, or steers anything, and one
   based on a turn that already ended is superseded (`stale`).
4. `send` starts the next turn of an ended task and refuses as `conflict`
   while a turn is queued or running or a queued message waits. `queue`
   waits for the current turn to end and then starts the next turn, in
   order. `steer` replaces the instructions of a turn that has not started;
   for a running turn it follows the engine's stated steering
   ([SESS](NIP-SESS.md#steering-capability)): native steering the engine
   lacks refuses as `unsupported` unless `emulate` chose the engine's
   cancel-and-continue emulation, which stops the turn and starts the next
   turn with the message before any queued message; a steer whose turn
   already ended becomes the next turn. `answer` starts the next turn with
   the answer only while the task's ended turn asked a question or asked to
   approve a step, and only when `based_on` is at least the revision that
   turn started at; otherwise it refuses as `stale` (an earlier turn's
   question) or `conflict` (no question waits, or a message is queued). The
   first answer starts a new turn, so a competing or late answer refuses.

The host records a command before it evaluates it, and records the exact
task-owner command before it applies one, applying those bytes again after a
crash. A command that waits is rechecked when it runs: its sender must still
hold `operate` under the same grant and epoch, or it refuses as `revoked`.
A turn a command starts is an inert submission unless the owner's auto-start
policy admits it, under the same bounds as a new task. `dispatched`
returns the task ID as its reference; a waiting command is dispatched too.
An older host refuses `task.command` as `malformed` or `unsupported`.
An engine that cannot go on without the person ends its turn with a
question, or with a request to approve a step before it takes it; the turn's
reply is the question, and the task's summary reports phase `waiting` with
attention `input` or `approval` and a fixed headline, never the question's
text. An answer is data for the engine: an approval answer never widens the
task's grant, workspace, routes, or spend, and is not a [POL](NIP-POL.md)
approval.
`task.queue` carries `{task, edit}` and lists or edits the task's held
messages: queued messages, and emulated steers or messages sent now that
wait for the turn's stop. `edit` is one of `{action: "list"}`,
`{action: "lease"}`, `{action: "release"}`,
`{action: "edit", command, text}`, `{action: "remove", command}`,
`{action: "reorder", commands}`, and `{action: "send_now", command}`. `lease`
takes or renews the task's queue edit lease for 60 seconds; while a device
holds it, queued messages wait even when the turn ends, so nothing runs a
message being edited. Another device's lease refuses as `conflict`, and a
device renews its own well within the minute, for example every 20 seconds.
The other changes need the device's current lease, else `conflict`. `edit`
replaces the text of the device's own held message (at most 16 KiB) and
keeps the original request, so a replay of the command still matches;
`remove` ends the device's own held message unrun, and removing it again
succeeds; `send_now` turns the device's own queued message into the
engine's emulated steering, ahead of the queue; and `reorder` names the
exact permutation of the queued messages, at most 64, and moves only them. A
command this device did not send refuses as `forbidden`, and a message that
already ran as `conflict`. Every change evaluates the task's commands again,
rechecking each waiting sender. The answer is `queue`:
`{task, revision, lease, items}`, where `lease` is `{device, expires_at}` or
null and each item is `{command, device, text, priority}` in the order the
items run, with `text` only for the requesting device's own messages and
null for another's.
`task.archive` carries `{task}`: it takes a finished or cancelled task off
every device's lists and deletes nothing. A message still waiting for the
task, and any later `task.command` other than `interrupt`, refuses as
`conflict`: nothing continues an archived task. The host stops publishing the
task's activity summary, and its history observer lists the task's
transcript as an archived chat, which chat lists leave out. A task that has
not ended refuses as `conflict`, and archiving an archived task succeeds
again. Only the host's owner restores an archived task. An older host that
predates it refuses it as `malformed` or `unsupported`.
`spend.list` and `spend.settle` carry agent spending phase 1, in which an
agent on the host asks and the owner approves each payment on the phone; the
formats are `openagents.spend-grant.v1`, `openagents.spend-request.v1`, and
`openagents.spend-receipt.v1` in
[the spend protocol](../../docs/breez/spend-protocol.md). `spend.list`
carries `{grant}`, the device's current spend grant for this host; the host
refuses it as `forbidden` unless the grant's `issuer` is the requesting key
and its `grantee` is the host, and as `stale` when its epoch is below the
last one that device sent. The host keeps it as the device's grant and
answers `spends`: `{spends}`, at most 16 `{request, receipt}` entries that
draw on that device's grants and have no final receipt yet. `spend.settle`
carries `{receipt}` for one of them; the host records it only from the
device whose grant the request draws on, refuses a `paid` receipt whose
preimage does not hash to the invoice's payment hash as `forbidden`, keeps
the first final receipt (a different later one refuses as `conflict`), and
answers `settled` with the recorded receipt. Neither operation moves money
or grants anything; the phone's wallet pays, after the owner's tap. The
host wakes the phone for a new request with a spend wake
(`openagents.spend-wake.v1`), a private artifact on a derived host-to-device
mailbox that carries nothing about the request, as the spend protocol
describes. `dispatched` returns
`{operation, reference}`: the handling receipt, not evidence that a task ran
or a terminal produced output. Other profiles can register further
operations with exactly one required right each.

A reply is `openagents.host-reply.v1` with `request`, `request_event` (the
exact original event ID), `host`, `issued_at`, `expires_at` (equal to the
request's), and `result`. Result is `{status: "ok", outcome}` or
`{status: "refused", code, missing}`. `missing` is the missing right when
`code` is `missing_right`, and null otherwise. The original signer is the
host, the recipient is the requesting key, and the mailbox is the request
ID. Clients check that correlation before reading the outcome, and check
that the outcome kind answers the operation.

### Device listing

`devices` returns one entry per grant the host retains:

| Field | Meaning |
| --- | --- |
| `device` | The grant's device key. |
| `grant`, `epoch` | The grant ID and its epoch. |
| `rights` | The grant's canonical rights list. |
| `origin` | `invitation` or `approval`. |
| `issued_at`, `expires_at` | The grant's lifetime. |
| `state` | `active`, `revoked`, or `expired`. A grant at an old epoch is `expired`. |
| `last_seen` | Host time of the host's last admitted request or direct channel from the device under this grant, or null when it has none. |

The host alone observes `last_seen`; a device's own clock or claim never sets
it. A host may record it coarsely, for example at most once a minute for an
open channel, and must not publish it anywhere but a `devices` answer to a
device that holds `access_read`. A reader that receives an entry without
`last_seen` treats it as null.

Refusal codes are `malformed`, `unsupported`, `forbidden`, `missing_right`,
`expired`, `revoked`, `stale`, `conflict`, `bounds`, `unavailable`,
`rate_limited`, `wrong_code`, and `denied`. Local transport failures are
distinct from signed refusals.

### Admission order and retention

The host validates the envelope, the request schema, freshness, its own key,
the relay, and the mailbox. It then checks the principal: the owner, or the
current grant, device, epoch, and expiry. Only then does it check the right
and act. Every operation, including an exact retry, repeats these checks.

The idempotency key is the request ID. The host retains the signed reply of
an admitted principal with the exact request event ID until the request
expires. An identical retry returns the retained bytes while the principal
is still current. Different bytes under the same request ID refuse as
`conflict`. For an operation with an external effect, the host records the
admitted request before dispatch and passes the request ID to the effect's
owner as its idempotency key. After an uncertain save, a retry dispatches
with the same key; it never mints a new logical operation. An expired
request earns no reply and triggers no effect.

## Bindings

**Direct artifact binding.** Requests and replies travel as original signed
private `3188` artifacts, as in the SESS observer profile. The host
subscribes to `3188` events addressed to it; the client subscribes to the
host's reply on the request mailbox before publishing. Retries resend the
exact signed event.

**CAP binding.** A host can instead register these CAP operation roles and
invoke them through CJ execution v1 over `25920`/`26920`/`27020`:

| Role | Input | Result |
| --- | --- | --- |
| Host access request | `openagents.host-request.v1` | `openagents.host-reply.v1` |

The CJ request carries the exact signed request artifact. Admission is
identical in both bindings. A host advertises only the binding it serves.

A relay connection lives at most 120 seconds, and CJ kinds are ephemeral, so
a request published while a host has no subscription open is lost until the
device retries. A host therefore keeps each binding's subscription open
without a gap: it subscribes on a new connection before the current one's
lifetime ends, reads both until the older one closes, and answers an event
that both deliver once. As on a direct channel, it answers each request
independently, so one slow request does not hold the others; requests are
not ordered by arrival.

The host advertises the role as one CAP `adapter` definition with the `d`
slug `host-access`, the ID `<host-key>:openagents/host-access`, transport
`nostr-cj`, `interface` `openagents.host-request.v1`, `operations` listing
every operation `kind` above, and `remote` naming the host key as the worker
and the relays it serves. Its `input` SchemaRef pins the JSON Schema for
`{v: "openagents.host-call.v1", event}` and its `output` SchemaRef pins the
one for `{v: "openagents.host-answer.v1", event}`: the direct-channel call
and answer, carrying the exact signed request and reply artifacts. A client
resolves the definition only from the host key it pinned.

Every field of the execute body is fixed by the definition and the embedded
request:

| Field | Value |
| --- | --- |
| `request`, `run` | The NIP-HOST request ID. |
| `attempt` | `1`. |
| `target` | The definition's DefinitionRef, with the digest of its JCS bytes. |
| `lock` | The one-entry `openagents.lock.v1` whose root is `target`. |
| `input` | The call, validated against the pinned input schema. |
| `context`, `requirements` | The fixed bodies `{v: "openagents.host-context.v1", requires: []}` and `{v: "openagents.host-requirements.v1", requires: []}`: the binding discloses no context and states no requirement beyond NIP-HOST admission. |
| `bounds` | `{}`. |
| `deadline` | The request's `expires_at`, mirrored in `expiration`. |
| `retain_until` | `deadline` plus 60 seconds. |

The CJ idempotency key therefore reduces to the NIP-HOST request ID. A
retransmission carries the same body. Changed bytes under a known request ID
are a changed NIP-HOST request, and the host answers with its signed
`conflict` reply, exactly as in the direct artifact binding. The CJ signer
must be the request's signer; another signer earns a CJ refusal
`not_admitted`. A target, lock, context, or requirements that is not the
host's earns `identity_mismatch`, and any other binding mismatch earns
`malformed`. These refusals come before NIP-HOST admission and have
`dispatched: false`.

Otherwise the host hands the embedded request to NIP-HOST admission and
returns a `26920` result with outcome `completed`, `dispatched: true`, and
the answer as `output`. `completed` means the NIP-HOST operation answered:
the embedded reply states whether it was admitted, and a `dispatched` reply
inside it is still only a handling receipt, not evidence that a task ran. A
request that earns no signed reply in the direct artifact binding, such as an
unknown invitation or an expired request, earns no CJ result. The operation
answers within the request, so the host sends no `accepted` or progress
feedback and answers no status, replay, or cancel control.

**Direct-channel binding.** Over an open [REACH](NIP-REACH.md) direct
channel, a device sends `{v: "openagents.host-call.v1", event}` whose `event`
is the exact signed request artifact of the direct artifact binding, and the
host answers `{v: "openagents.host-answer.v1", event}` with the exact signed
reply. The host accepts a call only when the event's signer is the device
key the channel proved and the request names the host's primary relay.
Admission, retention, and retries are those of the direct artifact binding.
A message longer than one data frame is split: each data frame starts with a
flag byte, `1` when more fragments follow and `0` for the last, and a
message is at most 256 KiB. Before it closes a channel whose grant stopped
admitting it, the host sends `{v: "openagents.host-closing.v1", code}` with
`revoked`, `stale`, or `not_admitted`.

### Nudges

A request lives 60 seconds, so a host that was asleep, offline, or
reconnecting misses requests sent meanwhile. The device keeps its durable
commands and sends them again with the same command IDs, but only while it
runs. A nudge lets the host learn, when it comes back, that a device is
waiting. It is an original signed private `3188` artifact from the device to
the host, `openagents.host-nudge.v1`: `{v, requires: [], host, device,
issued_at, expires_at}`, where `device` is the signer and `expires_at` is 24
hours after `issued_at`. Its mailbox is
`SHA-256("openagents.host-mailbox.nudges.v1\0" || conversation key)` in lowercase
hex, which only the two keys can compute. A nudge carries no command, text,
or task, and grants nothing.

The host reads nudges addressed to it as they arrive, and after a gap in its
relay subscription longer than a subscription's life it reads the nudges
stored within their lifetime. For a current nudge from a device with an
active grant, the host evaluates held commands and publishes its presence
and hints to that device at once. It answers each nudge at most once and
each device at most every 30 seconds, and ignores every other nudge. A device
that sees presence newer than its last failed attempt sends its waiting
commands again.

## Security considerations

- The relay is transport, not authority. It sees the `p` and `h` tags, so it
  learns which keys talk to a host and when. It never sees rights, codes,
  capabilities, or grants.
- An invitation string is a bearer secret for five minutes. Whoever redeems
  it first becomes the device. Show it only to that device.
- The 40-bit short code resists guessing because the host counts attempts
  per request, closes the request after 5, and expires it after five
  minutes. Only the owner and current administrators can decrypt the request
  that a guess would need.
- A client pins the host key from the invitation or from the owner's own
  records. A self-signed grant from another host does not replace it.
- Private envelopes cannot be recalled. Revocation stops future disclosure
  and admission only.
- A connect code is read off the host's own screen, which makes the screen
  the trusted channel: it gives the device both host public keys and a
  secret only the host knows. A fake computer on the path needs the host's
  iroh secret, its Nostr secret, and the capability. A person who
  photographs the screen gets one redemption race inside two minutes, and
  the real device then receives `forbidden` and says so.
- An iroh connection proves only the `EndpointId`. The host key's signature
  on the reply and grant, checked against the code, is what the device
  trusts.

## Worked wire flow

The notation uses redacted placeholders such as `<host-key>`; they are not
wire values.

1. On the host, the operator runs a local owner setup with `<owner-key>`.
2. The operator creates a standard invitation. The host persists its record,
   then displays the `coder-host:` string and its QR code.
3. The phone scans it, signs `enroll.redeem` to `<host-key>`, subscribes to
   the host's reply on the request mailbox, and publishes. The host commits
   the grant and reply, then publishes the reply. The phone saves its access
   record only after verifying the grant.
4. The phone signs `task.create` with its grant and epoch. The host checks the
   grant and the `operate` right and hands the task to its task owner.
5. A headless server shows `7K2M-Q9XA` and publishes an enrollment request to
   the owner. On the phone, the owner key opens it and the person types the
   code. The server admits the named device and returns its grant.
6. The owner revokes a lost laptop. Its epoch advances; its next request
   refuses as `revoked`, and a request carrying the old epoch refuses as
   `stale`.

With a desktop app and a connect code:

1. On first run the desktop app's host creates its host key, iroh secret key,
   and owner key in the keychain and establishes that owner locally.
2. The window shows `openagents-connect:<payload>` as a QR code for an
   invitation with rights `observe, operate`, and replaces it every minute.
3. The phone scans it, dials `<endpoint-id>` on `openagents/enroll/1`, sends
   the open message, checks the host key in the info message, and sends its
   signed `enroll.redeem`. The host commits the grant and answers. The phone
   checks both keys against the code, then saves its access record.
4. The phone opens a [REACH](NIP-REACH.md) direct channel on
   `openagents/reach/1` and signs `task.create` as in step 4 above.

## Conformance

Fixtures cover: redemption; same-device retry after restart; another
device's reuse; expired and cancelled invitations; reverse-enrollment
approval and denial; wrong codes and closure; a forged request digest;
delegation beyond held rights in both invitations and approvals; a
delegated invitation after its issuer's revocation; revocation while a
request is in flight; a stale epoch; a copied grant; a crash between
consumption and reply; exact retries and a reused request ID; and per-right
refusal, including that an `observe`-only device cannot create a task or
open a terminal. Connect codes add: a round trip of the byte layout; each
malformed case listed under [Connect codes](#connect-codes); a redemption
over iroh; a code whose host key differs from the info message or the reply;
rights other than the connect code rights; a clock more than 60 seconds off;
and a connection that opens a second stream.

Advertise `nip-host-v1` only for a configured host or client role with these
behaviors tested, in NIP-11 `supported_extensions`, not a numeric
`supported_nips` entry. A relay serving `3188` does not implement HOST.

## Implementation status

[`crates/coder-access`](../../crates/coder-access/README.md) implements the
host store, the direct artifact binding, the portable client, and a CLI. Its
behavior tests run over a synthetic NIP-42 relay. The wire fixtures in
[`crates/coder-access/fixtures/nip-host.json`](../../crates/coder-access/fixtures/nip-host.json)
give a valid body for every artifact and every operation, and invalid bodies
with the refusal code each must produce; `crates/coder-access/tests/wire.rs`
checks them.
[`crates/coder-host`](../../crates/coder-host/README.md) is the resident host
(`coder host serve`): it serves the direct artifact binding and the
direct-channel binding, and dispatches `task.create`, `task.steer`,
`task.cancel`, `task.command`, and `task.queue` to the durable task inbox and `terminal.open` to its terminal
host, answers nudges, and keeps agent spend requests for `spend.list` and
`spend.settle` beside its access store (`coder host spend`). It records each device's last-seen time for `device.list`. The
[Computers screens](../../crates/coder-computers/README.md) are its
interface on iOS, Android, and the terminal: their live service redeems
invitations, lists devices, creates narrowed invitations, and revokes
devices through `coder_host::client`.

The resident host also serves the CAP/CJ binding. `coder host serve`
publishes the `host-access` definition as a `30180` on every relay it serves
and answers `25920` requests through the same admission path as the other
bindings. [`coder_access::cj`](../../crates/coder-access/src/cj.rs) builds the
definition and checks the binding fields, and its client sends an operation
over CJ. The schemas are
[`host-call.v1.json`](schemas/host-call.v1.json) and
[`host-answer.v1.json`](schemas/host-answer.v1.json), and
[`crates/coder-access/fixtures/host-access-capability.json`](../../crates/coder-access/fixtures/host-access-capability.json)
is a reference definition that the NIP-CAP validator accepts.
`crates/coder-host/tests/cj.rs` shows, over a synthetic relay, that both
bindings give the same outcome for a granted operation, a missing right, a
revoked grant, a stale epoch, and a reused request ID.

[Connect codes](#connect-codes), [enrollment over iroh](#enrollment-over-iroh),
and the [local operator socket](#local-operator-socket) are designed, not
implemented. The
[epic](https://github.com/OpenAgentsInc/openagents/issues/9965) builds them:
`crates/openagents-connect` owns the connect code parser and its fixtures,
and the host step serves the ALPN and the socket.
[Nearby approval](#nearby-approval-planned) comes after that milestone.
