# NIP-HOST — Host-wide device enrollment and scoped access

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
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
| [REACH](NIP-REACH.md) | Owner host directory, presence, reachability hints, and direct channels. A direct channel binds to one HOST grant ID and epoch. A route, address, or tailnet membership never grants access. |
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
  command on that machine. An invitation, approval, relay event, or display
  name cannot establish or change it. The owner holds every right at its host
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
| `terminal.open` | `terminal` | `dispatched` |

`task.create` carries `{title, prompt, workspace}`. The title is at most 200
bytes, the prompt at most 16 KiB, and the workspace a host-scoped label of at
most 128 bytes, never a path. `terminal.open` carries `{cols, rows}`, each
1–1,000; [TERM](NIP-TERM.md) defines the session and stream.
`task.steer` carries `{task, revision, prompt}` and `task.cancel` carries
`{task, revision, reason}`: the host-issued task ID from a `task.create`
receipt, the task revision the device last read, and a replacement prompt of
at most 16 KiB or a single-line reason of at most 512 bytes. They follow the
[CTRL](NIP-CTRL.md) steer and cancel semantics of the host's task owner: a
steer records replacement instructions and supersedes a running context, a
cancel requests a stop, and another revision refuses as `stale`. Neither
grants execution authority. A host answers
either only after the effect's owner accepts it. `dispatched` returns
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

## Conformance

Fixtures cover: redemption; same-device retry after restart; another
device's reuse; expired and cancelled invitations; reverse-enrollment
approval and denial; wrong codes and closure; a forged request digest;
delegation beyond held rights in both invitations and approvals; a
delegated invitation after its issuer's revocation; revocation while a
request is in flight; a stale epoch; a copied grant; a crash between
consumption and reply; exact retries and a reused request ID; and per-right
refusal, including that an `observe`-only device cannot create a task or
open a terminal.

Advertise `nip-host-v1` only for a configured host or client role with these
behaviors tested, in NIP-11 `supported_extensions`, not a numeric
`supported_nips` entry. A relay serving `3188` does not implement HOST.

## Implementation status

[`crates/coder-access`](../../crates/coder-access/README.md) implements the
host store, the direct artifact binding, the portable client, and a CLI. Its
fixtures run over a synthetic NIP-42 relay.
[`crates/coder-host`](../../crates/coder-host/README.md) is the resident host
(`coder host serve`): it serves the direct artifact binding and the
direct-channel binding, and dispatches `task.create`, `task.steer`, and
`task.cancel` to the durable task inbox and `terminal.open` to its terminal
host. The CAP/CJ binding and interface screens are not implemented.
