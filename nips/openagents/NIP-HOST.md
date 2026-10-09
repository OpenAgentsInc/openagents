# NIP-HOST — Host Access

`draft` `optional` — v1, 2026-09-26; amended 2026-09-29 with
[connect codes](#connect-codes), [enrollment over iroh](#enrollment-over-iroh),
the [local operator socket](#local-operator-socket), and
[nearby approval](#nearby-approval-planned) for the
[QR pairing design](../../docs/coder/design/2026-09-29-auto-pairing.md), and
2026-09-30 with the [thread operations](#operations) (`thread.list`,
`thread.read`, `thread.send`, `thread.stop`, `thread.run`) that carry the
host's chat threads to a phone and start Coder from an offer on one. The
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
| `world` | Join the host's world instances over a [REACH](NIP-REACH.md) direct channel. The world host checks the grant at the handshake, before every world request, and on a timer. |

A grant carries a nonempty, duplicate-free list in the order shown above.
Any other order is malformed. No right implies another: `access_admin` does
not include `access_read`, and `operate` does not include `observe`. A
**standard device grant** is `observe`, `operate`, `terminal`, and `review`;
it excludes both access rights. The **pairing grant** is every right except `world`:
`observe`, `operate`, `terminal`, `review`, `access_read`, and
`access_admin`. It is what a connect code and a nearby approval carry,
because the device paired that way is the owner's own phone and does what
the owner does at the computer. It leaves out `world` because clients built
before that right existed refuse a grant that names it; an administrator
grants `world` explicitly.

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
the string is at most 654 bytes.

The same payload has a second text form, the **connect link**:
`https://openagents.com/connect#` followed by the same unpadded base64url,
at most 666 bytes. The payload is the URL fragment, which a browser never
sends to a server. A host shows the link in its QR code, so a phone's system
camera, not only the companion app's scanner, can read it: the phone app
claims the link as an iOS universal link and a verified Android App Link and
opens straight into pairing, and on a phone without the app, the link opens
a static `https://openagents.com/connect` page that says where to get it.
That page runs no script and never reads or transmits the fragment.
openagents.com serves the matching `/.well-known/apple-app-site-association`
and `/.well-known/assetlinks.json` for `/connect` only.

A reader accepts both forms and treats them as one code. It recognizes the
link only by that exact prefix: another scheme, host, or path, a query, or a
payload outside the fragment is not a connect code. Every rule below applies
to the payload whichever form carried it. A reader refuses the whole code, before it
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
- Its QR code shows the connect link; its copy action copies the
  `openagents-connect:` text.
- It copies the text form to the clipboard only on an explicit tap, and marks
  the clipboard entry to expire after 60 seconds where the platform allows.
- It never writes the string, the capability, or the QR image to a log, a
  file, a URL other than the connect link's fragment, or telemetry.

Rights for a connect code are fixed:

- The invitation's rights are the pairing grant: `observe`, `operate`,
  `terminal`, `review`, `access_read`, and `access_admin`. Nothing on the
  host's screen or in the local control request narrows or widens them;
  there is no terminal checkbox. The same holds for a code the command line
  shows (`openagents connect invite`) and for one set up over SSH.
- The grant expiry is 30 days after issue, the most a grant allows.
- The issuer is the host itself, as a local operator action.

A host refuses to show a connect code for any other rights list. A device
that receives a grant whose rights differ from these refuses it and keeps no
access record, with one allowance while computers update: a device also
keeps the rights earlier connect codes carried, `observe` and `operate`
with or without `terminal`. To narrow a phone, the person removes it; a
rights change in place is not defined.

The capability is a temporary bearer secret until redemption. Show it only
to the enrolling device. It must not appear in a public event, URL, log,
telemetry, or remote QR-generation service; the one exception is the
connect link's fragment, which stays on the phone that reads it. A phone
app that opens from a connect link passes it only to its own pairing and
keeps no copy. The relay URL follows the shared
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
   **Don't connect**, and nothing else: no rights choice. It admits the
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

Tailnet admission is kept as an optional admission path alongside
[nearby approval](#nearby-approval-planned) and QR pairing; see the
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

The socket may also broker typed task operations as the locally established
owner. The resident host prepares the signed NIP-HOST request and verifies
the reply with the portable client, and routes it through the same request
admission and task-owner dispatch as a device request. The window holds no
signing secret. A broker must preserve request identities and exact pending
packets across retries, and must not bypass operation validation, current
owner checks, retained replies, or task execution policy. Read-only task
history remains under the separate observer profile and its source bounds.

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

### Renewal

A grant lasts at most 30 days, and a device that keeps using a host must not
have to pair again. So the host renews a grant on its own, without a
request, while the device is connected:

- When a [REACH](NIP-REACH.md) direct channel opens under a grant, and again
  at most once an hour while it stays open, the host checks whether the
  grant is due: it is current (not revoked, not expired, at the device's
  current epoch), it is the device's newest current grant, its issuer is the
  host or the owner, and a quarter or less of its lifetime remains.
- If it is due, the host signs a new grant with a new ID and the same
  device, relay, rights, epoch, origin, and lifetime, issued now, and sends
  the exact grant envelope on the channel as
  `{v: "openagents.host-grant-renewal.v1", event}`. The channel's grant
  binding moves to the new grant.
- The renewed grant stays admitted until its own expiry, which is at most a
  quarter of its lifetime away, so a request already signed under it is not
  refused and a device that does not read renewals keeps working until
  then. Revoking the device revokes both.
- A device accepts a renewal only when it opens as a grant from the same
  host to its own key with the same owner, relay, rights, epoch, and
  origin, a new ID, an issue time no earlier than its current grant's, and
  a later expiry. It then stores the new grant and its envelope in place of
  the old ones. Anything else is ignored and the current access is kept.
- A revoked, expired, or stale grant never renews, and neither does a grant
  a device delegated with `access_admin`: its issuer's own grant bounds it.

This is an exception to one grant per device key: for at most a quarter of
a grant's lifetime, a device holds its renewed grant and the renewal.

A device clock may be up to 60 seconds behind the host's. The host admits a
request dated up to 60 seconds before the invitation or grant it names, and
a device accepts a reply dated up to 60 seconds before its request. The
host's clock still decides expiry.

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
| `task.list` | `observe` | `tasks` |
| `task.read` | `observe` | `task` |
| `task.original` | `observe` | `task_original` |
| `project.list` | `observe` and current project policy | `project_list` |
| `project.read` | `observe` and current project policy | `project_read` |
| `project.original` | `observe` and current project policy | `project_original` |
| `cloud.projects` | `observe` and current operator policy | `cloud_projects` |
| `cloud.catalog` | `observe` and current operator policy | `cloud_catalog` |
| `cloud.list` | `observe` and current operator policy | `cloud_list` |
| `cloud.read` | `observe` and current operator policy | `cloud_read` |
| `cloud.original` | `observe` and current operator policy | `cloud_original` |
| `cloud.submit`, `cloud.continue`, `cloud.cancel`, `cloud.follow` | `operate` and current operator policy | `cloud_accepted` |
| `task.steer` | `operate` | `dispatched` |
| `task.cancel` | `operate` | `dispatched` |
| `task.archive` | `operate` | `dispatched` |
| `task.command` | `operate` | `dispatched` |
| `task.command.at_revision` | `operate` | `dispatched` |
| `task.queue` | `operate` | `queue` |
| `task.queue.at_revision` | `operate` | `queue_at_revision` |
| `request.operation` | `observe`, then the original effect's current right | `request_operation` |
| `terminal.open` | `terminal` | `dispatched` |
| `task.terminal.open` | `terminal` | `dispatched` |
| `computer` | `terminal` | `computer` |
| `workspace.list` | `operate` | `workspaces` |
| `spend.list` | `operate` | `spends` |
| `spend.settle` | `operate` | `settled` |
| `chats.invite` | `observe` | `chats` |
| `verse.private` | `observe` | `verse_private` |
| `thread.list` | `observe` | `threads` |
| `thread.read` | `observe` | `thread` |
| `thread.send` | `operate` | `dispatched` |
| `thread.stop` | `operate` | `dispatched` |
| `thread.run` | `operate` | `dispatched` |
| `studio.snapshot` | `observe` | `studio` |
| `studio.update` | `observe` | `studio_update` |
| `studio.review.open` | `observe` | `review` |
| `studio.goal.submit`, `studio.seat.message`, `studio.seat.pause`, `studio.seat.resume`, `studio.seat.stop`, `studio.task.reassign`, `studio.task.cancel`, `studio.task.retry`, `studio.task.prioritize`, `studio.decision.answer`, `studio.decision.always` | `operate` | `dispatched` |
| `studio.merge.decide` | `review` | `merged` |

The observe-only task reads carry `{query}` and return the portable
`coder-access::task_read` records. The host checks its current Observe grant
and explicitly admitted workspace label before the task owner checks its
own disclosure policy. These reads expose that owner's canonical task journal,
ATIF evidence, artifact manifest, and retained artifact bytes. They do not open
retained Codex or Claude history roots, which keep their separate SESS observer
admission, and create no execution, enrollment, review, or spending authority.

`task.list` names one admitted workspace label, a limit of 1–64, and an optional
cursor binding that workspace, the complete list snapshot digest, and next row.
`task.read` names a workspace, task ID, optional exact revision, limit of 1–64,
and optional prefix cursor. A null revision is allowed only without a cursor
and opens the current head. Returned scope always pins the exact revision,
one-based task turn, and intent digest. A continuation also binds an opaque
trace alias, previous raw snapshot digest and byte length, next logical step,
and delivered step-prefix digest. Appends may continue only if both previous
raw bytes and the delivered logical prefix still match. A changed revision,
turn, workspace, intent, rewritten prefix, or shortened source refuses.

Task outcomes contain at most 48 KiB of JSON. Original ATIF step objects remain intact;
a step over 16 KiB becomes an explicit oversized gap with its original source
pin. Execution, verification, termination, delivery, cleanup, and integration
remain separate states. Child references come only from structured delegation
records and confer no child-task or control authority. A summary that omits
entries identifies the remaining original source instead of silently clipping.

`task.original` pins the complete task scope, an opaque `task`, `trace:<turn>`,
`manifest:<turn>`, or `artifact:<path-digest>` alias, SHA-256 digest, byte count,
and optional byte-prefix cursor. Sources are at most 64 MiB; chunks are at most
16 KiB before base64 encoding. Every read rechecks the source and full digest;
appending to a pinned original requires reopening it. Aliases never admit a
caller path, endpoint, or symlink. Missing, damaged, unavailable, stale, and
oversized evidence stays explicit. The host rechecks expiry and current Observe
standing before signing a successful read reply.

`task.create` carries `{title, prompt, workspace}`. The title is at most 200
bytes, the prompt at most 16 KiB, and the workspace a host-scoped label of at
most 128 bytes, never a path. Two optional members follow, each omitted
when empty so a create without them encodes exactly as before. `images` names
images the device already sent with `artifact.put`, each `{digest,
media_type, size, name}`; the host binds only complete images that same
device sent. `engine`
is the coding engine the person asked for, the typed `engine` of the chat's
[CJ](NIP-CJ.md) `run_coder` offer and never words from the conversation: one
of `codex`, `claude_code`, `grok_build`, `opencode`, or `devin`; any other
value is malformed. It is a request, not permission. The host puts that
engine's routes first only among the routes its owner's auto-start policy
already admits; it never adds a route, a model, or a limit, and without a
policy the task stays inert as always. When the engine does not start the
task (the policy admits no route for it, it is not signed in, it is refused
for a limit, or it is near its usage threshold), the task's activity summary
headline says so while the task runs, from the host's own typed state: "You
asked for Devin; it is not one of the engines this computer's Coder policy
allows, so Codex is running." A host whose `task.create` reads `engine`
advertises the `task-engine` capability in its [REACH](NIP-REACH.md)
presence. A host that predates it rejects the unknown member, so a device
sends `engine` only to a host that advertises `task-engine`, and otherwise
sends the request without it; that host runs its own default. A host whose
owner starts Coder at once for a chat's coding reply (its own `coder.start`
setting is `at_once` and its auto-start policy is on) advertises the
`coder-start-at-once` capability in its presence, recomputed each time it
publishes presence. A device may then send `task.create` for its chat's
`run_coder` offer without waiting for a tap, once per reply; without the
capability (an older host, an owner who asks first, or a policy that is off)
it waits for the person to choose **Run Coder**. The capability grants
nothing: the host checks the grant and `operate` right on the request as
always, and its policy decides whether and how the task runs. A host also
names the coding agents on its computer in its presence (added 2026-10-01),
one capability each, `engine-<state>-<engine>`: `state` is `ready`,
`not_signed_in`, `limited` (at its usage limit), or `not_enabled` (installed
or signed in, but not allowed by the owner's settings), and `engine` is a
word of at most 16 lowercase ASCII letters, digits, `-`, or `_`, such as
`engine-ready-codex` or `engine-not_enabled-devin`. They are the list the
host's own chats send as [CJ](NIP-CJ.md) `context.computer.engines`: the
engines the owner's settings allow, in that order, then the others
installed there. At most 8, each engine once. The host reads them off its
runtime at most every 15 seconds and publishes presence again when they
change (a sign-in, a sign-out, a usage limit, or a settings change). They
carry no account, token, or usage figure and grant nothing. A device keeps
them with the computer's record and names them in its chat's CJ context as
`{place: "paired", name, engines}`. A reader leaves out a flag whose state it
does not know, and one that predates the flags ignores them as unknown
capabilities. `terminal.open` carries `{cols, rows}`, each
1–1,000; [TERM](NIP-TERM.md) defines the session and stream.
`task.terminal.open` carries `{task, cols, rows}` with the same size bounds
and a studio task ID. The task owner resolves its current worktree locally;
the client supplies no path. Missing, archived, unsupported, or read-only
bindings refuse the operation. The dispatched reference identifies a separate
TERM shell, not the agent's input stream. Subsequent attachment and input
recheck the exact task worktree binding and the current terminal grant;
archive, changed bindings, revocation, or a lost generation refuse them.
Detach and close affect only that shell. Human worktree edits invalidate
existing exact tree reviews; opening a shell supplies no review or merge
approval. The terminal descriptor reports its directory, generation, and
interactive access mode; clients retain the selected host and task identity.

`computer` (added 2026-10-09) reaches the host's computer itself without a
terminal: `{computer: {action, ...}}` with one of five actions, each needing
`terminal`, because a device that can open a shell there can already do it.
The answer is `{kind: "computer", computer: {kind, ...}}`.

| `action` | Members | Answer |
| --- | --- | --- |
| `screenshot` | `source`: `{kind: "screen", screen}` or `{kind: "android", serial}`; `screen` and `serial` are optional names of at most 128 ASCII letters, digits, `-`, `_`, `.`, or `:` | `file`, a PNG the host keeps |
| `apps` | none | `apps`: `{apps, source}`, at most 512 windows, each `{name, title, pid, focused}` |
| `stat` | `path` | `file`: `{path, size, digest, media_type}` |
| `read` | `path`, `offset`, `length` (1 to 32 KiB) | `chunk`: `{offset, data}`, base64, shorter only at the end |
| `write` | `put`: `{path, size, digest, offset, data, overwrite}` | `written`: `{received, complete}` |

Any action may instead answer `unable`: `{reason}`, a sentence of at most 512
characters for a person (no screen session, no capture tool, no such file).
Authority refusals stay refusal codes. A path is absolute or starts with
`~/`, at most 4,096 bytes and without NUL. A file is at most 256 MiB, a
screenshot 64 MiB, and a digest is `sha256:` and 64 lowercase hex digits.

A device reads a file by `stat`, then `read` in order, and checks the digest
of what it assembled against `stat`'s, so a file that changed meanwhile is
refused rather than delivered torn. A `write` chunk names the whole file's
size and digest; every chunk but the last is exactly 32 KiB at an offset that
is a multiple of it. The host keeps the chunks in a hidden partial file beside
the destination, named by the digest, and moves the file into place only when
every byte arrived and the digest matched; a mismatch drops the partial and
refuses as `conflict`. An existing file is replaced only with `overwrite`;
without it the host refuses as `conflict`. A chunk at an offset the host
already holds, or past it, changes nothing, and `received` says where to go
on; the last chunk sent again after the file is in place answers `complete`.
A screenshot goes to an owner-only `captures/` folder beside the access
store, of which the host keeps the newest 8. On Linux the host finds the
screen session even when the service manager started it outside one: the
desk protocol of the Coder compositor or Hyprland, then `grim` on the
session's Wayland socket, then `maim`, `scrot`, or `import` on its X11
display; on macOS `screencapture`; an Android device through `adb exec-out
screencap -p`. None of the replies is retained (see
[Admission order and retention](#admission-order-and-retention)).
`workspace.list` carries nothing and returns `{workspaces}`: the labels
`task.create` accepts on this host, sorted and distinct, at most 64, each
1–128 bytes without control characters. The roots they name stay on the
host. A host with no task owner refuses it as `unavailable`, and an older
host that predates it refuses it as `malformed` or `unsupported`; a client
then asks for the label instead of offering a list.
`chats.invite` carries nothing and returns `{invitation, expires_at}`: a
single-use `coder-pair:` invitation of at most 4,096 bytes to the host's
read-only Coder chats, the same one the iroh enroll reply and
[tailnet admission](#tailnet-admission) carry, and when the chat grant it
carries ends. It admits only the host's Coder task store. A device holding
`observe` asks for one after pairing by any path whose answer carried none
(nearby approval, or a connect code redeemed on the relay), and again before
its chat grant ends. A host that serves no chats refuses it as
`unavailable`, and an older host as `malformed` or `unsupported`.
`verse.private` carries `{world_key}`, the device's Verse world public key
as 64 lowercase hex characters, and returns `verse_private`:
`{placements}`, the owner's private Verse placements file
(`openagents.verse.private-placements.v1`, at most 65,536 bytes) as the
host's Verse home holds it, or `null` when it holds none
([Private assets](../../docs/verse/private-assets.md#phones)). The host
notes the world key so its owner can grant it; the answer grants nothing,
because only an asset's manifest names who may load its pack. A host
without a Verse home refuses it as `unsupported`, and a placements file it
can't read as `unavailable`. Like `thread.list`, its reply is not retained.
`thread.list`, `thread.read`, and `thread.send` carry the host's chat
threads to a device: the conversations with OpenAgents that the host keeps
in its own chat store, which the owner starts in the desktop app or with
`openagents chat` on that computer
([glossary](../../docs/glossary.md), Thread). Thread and send IDs are 32
lowercase hex characters. `thread.list` carries nothing and returns
`threads`: `{threads}`, at most 128 rows, newest first, with archived threads
left out; each row is `{thread, title, started, updated, pinned, coder}`, a
title of at most 160 bytes without control characters, and `coder` null or
the Coder task the thread started, `{host, task, project, at}`.
`thread.read` carries `{thread, before}`, `before` null for the newest turns
or the index of the first turn not to include, and returns `thread`:
`{thread: {thread, title, start, total, turns, busy, partial, failure,
coder}}`. `turns` holds at most 64 turns from index `start`, oldest first,
each `{role, text, at, stopped, model, request, extras}`: `role` is `user` or
`assistant`, `at` when the host saved it or null, `stopped` whether the
reply stopped before its result, `model` the model the worker named or
null, and `request` the send ID that created a message, whichever device
sent it, or null. `extras` is optional. An older page omits it, which means
the turn has none. When present it is `{offers, followups, cards}`: `offers`
holds at most 4 objects the chat router already accepts (a Run Coder offer,
an open-screen offer, a read-only command, or an eval offer), each at most
8 KiB; `followups` holds at most 3 chips, each `{label, answer}`, with
`label` of 1 to 80 characters and no control characters, and `answer` a
short tag or absent; `cards` holds at most 4 card objects the conversation
parser already accepts, each at most 8 KiB. The router's typed judgment
(tier, answer, route, bank, and judgment text) stays on the host. `busy`
says a reply is streaming, `partial` is the reply so far (empty when none),
and `failure` why the last message has no reply, at most 1,024 bytes, or
null. A Coder run that `openagents chat` started on the host's computer is
bound to its thread under the host name `local`. When the host's task store
holds that task, together with the run's own record for that thread, the
host names its own key as `coder.host` in the row and the page, so a
device opens the task's chat through the history observer, follows its
events, and stops it with the task's own interrupt, exactly as for any task
on this host. When the run used another task store, `coder` is null and the
page carries `outside`: `{task, project, at}`, the task in that other store,
with the same bounds as `coder`. A page never carries both, and `outside`
is omitted when absent, so every other page encodes as before and an older
page has none. A device shows `outside` as words that say the run is on
that computer outside its host, with no Coder control for it. A `threads` or `thread` outcome encodes in at most 48 KiB, so a reply
fits one relay frame: the host drops the oldest turns' extras first, then
drops the oldest turns, then keeps only the end of a single turn or partial
that is still too large, marked with `…`. A device reads the reply as it
streams by reading the thread again, for example every 300 milliseconds
while `busy`.
`thread.send` carries `{thread, request, text}`: the send ID the device
mints once and replays unchanged, and a message of 1 byte to 32 KiB that is
not blank. The host appends the message to the thread and asks OpenAgents
for the reply with the thread as context, as a send from its own window
does, and answers `dispatched` with the thread ID as its reference once the
message is saved; that is a handling receipt, not the reply. The host
appends a message once per send ID: a replay with the same text is
dispatched again with no second message, and different text under a send
ID the thread holds refuses as `conflict`, as does a send while the thread
is answering or to an archived thread. An unknown thread, or a host that
keeps no chat store, refuses as `unavailable`, and an older host refuses all
three as `malformed` or `unsupported`.
`thread.stop` carries `{thread, request}`: the send ID of the message whose
reply the device stops, or null for a message sent without one. While the
thread is answering that message, the host stops receiving the reply
through its chat service's own stop, as a stop in its own window does:
what streamed is kept as the reply, marked `stopped`, and a reply with
nothing streamed yet leaves the message marked `stopped` with the failure
"Stopped receiving this reply. The hosted worker may still finish." That
wording is exact: the host stops listening, and the hosted worker may
still finish, but nothing it sends later reaches the thread. When the
thread is not answering that message (the reply ended, it was stopped
already, or a newer message is being answered) nothing changes. Either way
the host answers `dispatched` with the thread ID as its reference, so a
stop is idempotent per thread and send ID, and a stale one never stops a
later reply. An unknown thread refuses as `unavailable`, and an older host
refuses `thread.stop` as `malformed` or `unsupported`. A device shows a
stop control for a host's thread only once it knows the host can stop:
it sends `thread.stop` with a freshly minted send ID, which no message
holds and so changes nothing, and a `dispatched` answer means the host
stops; a refusal, from an older host or without `operate`, means no stop
control, never one that does nothing. Stopping a thread's reply does not
stop Coder work the thread started; a device offers that separately,
through the task's own stop.
`thread.run` carries `{thread}`. It asks the host to start Coder for that
thread through the same handoff the desktop uses: one stable request key
per thread and host, then `task.create`, and the thread's record of the
task. The host answers `dispatched` with the task ID as its reference.
When the thread already names a Coder task, the host answers `dispatched`
again with that task's ID and starts no second task. A reply with no
current Coder offer, a thread that is still answering, or an archived
thread refuses as `conflict`. A host with no project refuses as
`forbidden`. An unknown thread, or a host that keeps no chat store,
refuses as `unavailable`. An older host refuses `thread.run` as
`malformed` or `unsupported`. A device does not probe `thread.run` when a
thread opens. It shows the Run Coder chip from the offer on the page, and
sends `thread.run` when the person accepts that chip. A `malformed` or
`unsupported` refusal means the device drops the chip. `thread.run` grants
no execution authority beyond that handoff: the host's auto-start policy
still decides whether the task runs.
Unlike every other operation, the host does not retain a `thread.list`,
`thread.read`, or `verse.private` reply (see [Admission order and retention](#admission-order-and-retention)).

`studio.agent.crew.status` carries no fields and returns the owner's current
crew-control digest and retained cleanup results. `studio.agent.crew.control`
carries `{control: {cohort, selection, action, expected, reason}}`, with
`action` `stop`, `pause`, or `resume`; `selection` is `{kind: "all-sales"}` or
`{kind: "members", members: [NAME, ...]}` for 1 to 32 exact native sales
members. Both require the owner's own key, not a delegated device. Resume
requires the same named selection and exact current `sha256:` control digest
as `expected`. The selected Unix host persists the new revocation epoch before
member cleanup. A response distinguishes retained failures and unknown effects;
it grants no approval or delivery authority. An external outbox seals its own
exact approved pending handoff under the epoch lock and delivers after releasing
it; an already admitted unknown outcome remains unknown across restart.

The `studio.*` operations carry the
[Agent Studio](../../docs/verse/agent-studio.md#the-client-is-a-view): the
host's studio coordinator is the source of truth, and a device sends
intents and draws what the host answers. The wire types are
`coder_access::studio`. A studio task, goal, or decision identity is 1 to
128 ASCII letters, digits, dots, hyphens, and underscores, starting with a
letter or digit; a seat name is 1 to 32 lowercase letters, digits, and
hyphens. `studio.snapshot` carries nothing and returns `studio`:
`{snapshot: {stream, sequence, view}}`, where `view` holds goals, seats
(activity, station, task, route, look, paused), tasks (status,
dependencies, seat, board position), open decisions, repository summaries
by workspace label, and at most 8 log lines per seat, each list in key
order. `studio.update` carries `{stream, since}` and returns
`studio_update`: `{update: {stream, from, sequence, put, removed}}`, the
items that changed since `since`, whole, and the keys that went away.
`stream` names one host process; a host that started again, that no longer
holds `since`, or whose change does not fit one update refuses as `stale`,
and the device reads a fresh snapshot. A view or update encodes in at most
48 KiB; the host drops the oldest log lines, then the oldest finished goals,
to fit. A log line is display text the host derives from the task's ATIF
steps: the activity and the tool's name with the purpose the surface
showed, or the first line of what the agent said, never a call's arguments
or output. `studio.goal.submit` carries `{text, workspace, lead}` (a goal
of at most 4 KiB, a workspace label, and a lead seat or null for the first
lead); `studio.seat.message` carries `{seat, text}` (null for every seat,
at most 4 KiB); `studio.seat.pause`, `studio.seat.resume`, and
`studio.seat.stop` carry `{seat}`; `studio.task.reassign` carries `{task,
seat}`; and `studio.task.cancel`, `studio.task.retry`, and
`studio.task.prioritize` carry `{task}`. Pause keeps a seat's task and
holds back new ones; stop also cancels its queued or running task and
returns it to the board under a new task identity. Reassign and prioritize
apply to a planned task that has not started, and retry to a failed or
cancelled one. `studio.decision.answer` carries `{decision, based_on, text,
command, issued_at}`: a waiting task's question or approval is answered
through the `task.command` `answer` path under the device's 64-hex
`command` ID, and a goal's plan decision takes a plan as `text`; a
`based_on` other than the decision's refuses as `stale`. An approval
decision may carry `approval: {tool, command, cwd, reason, risk, always}`:
the step its engine named, the host's `low`, `medium`, or `high` risk,
and, for a step that is not high risk, the exact text of the standing rule
the host would keep. `studio.decision.always` carries `{decision,
based_on, rule, command, issued_at}`: it approves the step through the
`studio.decision.answer` path, then records a rule for that seat and
exactly that tool, command, and directory. A `rule` other than the text
the host offers for the step now refuses as `stale`. The host applies a
rule to a later matching approval of that seat once, as the device that
recorded it, after rechecking that device's grant; a client never applies
one.
`studio.review.open` carries `{task}` and answers `review` as
`task.review` does. `studio.merge.decide` carries `{decision: {task, base,
head_commit, head, verdict, text, command, issued_at}}`, where `verdict` is
`merge`, `request_changes` (with the changes asked for as `text`), or
`reject` (with an optional reason). The host reads the review again and
refuses a decision whose three revisions differ as `stale`, so the device
reloads the review; a retried merge whose publication already holds those
revisions answers it again. **Merge** hands the change to the `task.publish`
landing path, **Request changes** is the task's next turn through the
command journal under `command`, and **Reject** records the decision and
keeps the worktree until archive. It answers `merged`: `{merged: {task,
base, head_commit, head, verdict, publication}}`, with the publication for
a merge only. An intent answers once per request ID. The host does not
retain a `studio.snapshot`, `studio.update`, or `studio.review.open` reply.
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
`task.command.at_revision` carries `{command, revision}`. The command's
`based_on` must equal `revision`. The native owner holds the task's write lock
across the exact revision check and transition. An unchanged command retry
returns its recorded result after the task advances; a changed request under
the same command ID refuses. Older hosts reject this additive operation.

`task.queue.at_revision` carries `{task, revision, edit, queue_digest}`.
A `list` may omit the digest and is an unretained read that still requires
`operate`. Every edit, including taking, renewing, or releasing a lease, names
the exact current SHA-256 queue snapshot. Queue content and leases have their
own digest because editing them does not advance the task revision. The reply
contains `{queue, revision, queue_digest}` and is at most 48 KiB. Under the
command-journal and task write locks, the owner records the request ID and
exact action before dispatch and seals its result afterward. An unchanged
retry returns that result without renewing a lease or repeating a send. An
interrupted request without a sealed result stays unknown and never dispatches
again; the caller reads the canonical queue to reconcile it.

`request.operation` carries the original `{request, request_event}` and reads
its retained native result without dispatching an effect. It returns those
references and `result`, either the original typed success or refusal, or null
for an unknown or absent result. Task create, steer, cancel, exact command,
exact queue edit, publication, and cloud effect results remain recoverable for 48 hours after
the original request expires. The host checks the original signer, the same
current original grant and epoch, and the original required right before
revealing a result. A new recovery read never extends or replaces the effect's
signed envelope. An `unavailable` refusal can still describe an uncertain
effect; it does not prove that nothing changed.

Project reads return `coder-access::project` evidence without running a
supervisor. Cloud operations use `coder-access::cloud` DTOs: named native
workspace, project, and profile aliases; exact source, profile, job revision,
and attempt pins; and original source descriptors. Replies are at most 48 KiB,
and original byte chunks are at most 16 KiB before base64 encoding. Every cloud
read, effect, exact retry, and recovery checks the current operator policy in
addition to the original host grant. `cloud.follow` reconciles the original
provider task; only `cloud.continue` opens another turn. No operation grants
retail spending or chooses ambient provider credentials.

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
expires. The exception is a read with no effect, `task.list`, `task.read`,
`task.original`, the project and cloud reads, `thread.list`, `thread.read`, the studio reads, and `computer`: its reply is not retained, so a device that polls a streaming
thread never fills the host's store, and an exact retry reads again and may
answer newer content. An identical retry returns the retained bytes while the principal
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
2. The window shows the connect link `https://openagents.com/connect#<payload>`
   as a QR code for an invitation with rights `observe, operate`, and
   replaces it every minute.
3. The phone scans it, with the app's scanner or the system camera, which
   opens the app with the link. The app dials `<endpoint-id>` on `openagents/enroll/1`, sends
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
open a terminal. Connect codes add: a round trip of the byte layout and of
both text forms, the link carrying the payload only in its fragment; each
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
the [local operator socket](#local-operator-socket), and
[renewal](#renewal) are implemented.
[`crates/openagents-connect`](../../crates/openagents-connect) owns the
connect code parser and its fixtures, the ALPNs, and the control protocol.
`coder host serve --iroh --control` (the resident host in
[`crates/coder-host`](../../crates/coder-host)) serves the enroll ALPN
through the same redemption as the direct artifact binding, the reach ALPN
through the same direct-channel session as TCP, and the socket with the
peer user check; `openagents connect` is its command-line client. The
reference enroll exchange is one message each way: the device sends
`openagents.connect-enroll-request.v1` with the signed `enroll.redeem`, and
the host answers `openagents.connect-enroll-reply.v1` with its time and the
signed reply, or no reply. Beside a grant, and only when the redeeming
device now holds a current grant with `observe`, the reply may also carry
`chats`: a single-use `coder-pair:` invitation to the host's read-only Coder
chats, the same one [tailnet admission](#tailnet-admission) hands over, so a
phone reads the tasks it starts there. A refused device gets none, and a
redemption on the relay carries none; that device, a device paired nearby,
and any device whose chat grant nears its end ask with `chats.invite`. The info step above is not yet served, so the
device signs the relay it uses by default, which a desktop host serves.
`crates/coder-host/tests/iroh.rs` and `tests/control.rs` cover a
redemption over iroh, a second device refused `forbidden`, a terminal only
with `terminal`, revocation closing an open channel, an unknown key reaching
only enrollment, clock skew, renewal on a channel, the socket's modes and
peer check, and minting, rotating, and cancelling codes.

The thread operations are implemented. The resident host answers them from
the same chat store and service its local operator socket uses
(`crates/coder-host/src/serve/threads.rs`), so a thread the desktop app or
`openagents chat` started is the one a phone reads, and a phone's follow-up
shows in both. `crates/coder-host/tests/threads.rs` runs a host whose threads
ask a scripted chat worker on a local relay: a phone paired with a connect
code lists and reads a thread the owner started on the socket, sends a
follow-up whose reply it reads as it streams, replays the send without a
second message, is refused `conflict` for other text under its send ID, and
the owner reads the follow-up back; an `observe`-only device reads but is
refused the send as `missing_right`; and polling reads leaves the access
store's size unchanged. In the same file a phone stops a reply while it
streams: a stop for another send ID leaves it streaming, the stop for its
own keeps the partial as a stopped reply that later partials never reach,
a repeated stop changes nothing, the owner reads the stopped reply over
the socket as the desktop and `openagents chat read` do, the next message
is answered in full, and an `observe`-only device is refused the stop. The OpenAgents phone lists each paired computer's
threads beside its own, labelled with the computer, opens one to read its
turns as the reply streams, and sends a follow-up through the computer
([`crates/openagents-chat-app/src/host_threads.rs`](../../crates/openagents-chat-app/src/host_threads.rs)).
When a thread opens, the phone sends the no-op `thread.stop` under a fresh
send ID, once per computer while it runs; only after a `dispatched` answer
does its composer carry a stop while a reply streams, and the stop names
the message being answered. An older computer, or a phone without
`operate`, gets a composer with no stop icon, never a dead one. After a
stop, a thread whose Coder task is still running offers **Stop Coder too**,
which sends that task's own interrupt. A page with no extras shows no
cards or chips. When a reply carries a Run Coder offer, the phone shows
the same **Run Coder** chip and follow-up chips it shows on its own
threads, from the shared card code, and the cards on that turn. Accepting
**Run Coder** sends `thread.run`. Accepting a follow-up chip sends
`thread.send` with that chip's label. The phone does not probe
`thread.run` when a thread opens. The phone keeps each computer's
`thread.list` answer and each thread's settled turns as last read in its
own encrypted store, so a relaunch lists and opens them with the computer
off, marked with when they were read, and any read that answers replaces
the kept copy. A kept copy grants nothing: every send, stop, and run still
goes to the computer under the device's grant. A follow-up typed while the
computer is unreachable waits in a durable outbox under the send ID minted
when it was typed, and goes, from the open thread or after the computer's
next `thread.list` answer, until the computer accepts or refuses it; since
`thread.send` appends at most one message per send ID, a resend after a
crash or a relaunch is never a second message. An older computer refuses `thread.run`
as `malformed` or `unsupported`, and the phone drops the chip. A thread
that already names a Coder task shows no second **Run Coder** chip.
`crates/coder-host/tests/threads.rs` also runs that offer: the phone reads
the Run Coder offer, the follow-up chip, and the card, with no judgment
on the page, and `thread.run` starts one task that the owner reads back
on the socket. A second `thread.run` returns the same task. An
`observe`-only device is refused the run. In the same file a thread whose
Coder run `openagents chat` started on the computer, bound as host `local`
in the task store the host serves, names the host's own key in the row and
the page; a phone stops the thread's reply and sends the task's interrupt
(**Stop Coder too**), which reaches the task owner, while an
`observe`-only device reads the link and is refused the interrupt; a run in
a store the host does not serve comes back as `outside` with no link. The
phone shows it as words, with no Open Coder or Stop Coder too control
(`a_local_run_outside_the_computers_host_is_said_plainly_with_no_dead_controls`
in `crates/openagents-mobile/src/coder_tab_tests.rs`).
