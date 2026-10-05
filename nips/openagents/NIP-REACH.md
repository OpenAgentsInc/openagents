# NIP-REACH — Reachability

`draft` `optional` — v1, 2026-09-26; amended 2026-09-29 with
[iroh hints](#iroh-hints) and the [iroh mapping](#iroh-mapping) of the direct
channel, for the
[QR pairing design](../../docs/coder/design/2026-09-29-auto-pairing.md). The
[shared contracts](contracts.md) are normative. This profile lets a client find every host its owner runs,
judge which hosts are online and compatible, try routes in a safe order, and
open a direct channel that authenticates both Nostr keys before any data
flows. It also defines a pure placement rule for new work. It introduces no
new event kinds.

Nothing in this profile grants access. A directory entry, a presence sample,
a reachability hint, and an open channel are all facts about where a host is.
The host's own grant store decides what a device may do, and it checks each
operation separately.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [Shared contracts](contracts.md) | Encoding, refusal codes, and the private `3188` artifact envelope carry every record here. |
| [CAP](NIP-CAP.md) | CAP forbids local presence in public heads and requires a separately consented application for an encrypted fleet inventory. REACH is that application: an owner-encrypted directory and recipient-encrypted presence. CAP presence states still describe individual bindings. |
| NIP-HOST (drafted separately) | Host-wide device grants, their IDs, and revocation epochs. REACH binds a direct channel to one grant ID and epoch through a grant check; it does not define grants. |
| [CTRL](NIP-CTRL.md) and [SESS](NIP-SESS.md) | Task control and engine sessions. A direct channel can carry their operations; it does not change their admission rules. |
| [ENV](NIP-ENV.md) | Environment leases. Placement here picks a host for new work; it does not allocate, lease, or move anything. |
| Block [NIP-PL](../block/NIP-PL.md) | Push wakeups. A wake can prompt a client to read presence; it proves nothing about the host. |

## Roles and encoding

The **owner** is the key that decides which hosts belong together. A **host**
is one running host process on one computer, identified by its host key. A
**device** is a client key the host enrolled; it reads presence and hints and
opens direct channels. The owner and a device may share a key on a personal
computer, but the roles stay distinct.

Every body has `v`, `requires`, optional inert `meta`, and exactly the fields
listed. Unknown versions, required features, semantic fields, or enum values
refuse. The initial `requires` list is empty. Keys are lowercase 64-hex x-only
public keys; IDs and nonces are random 64-hex values. Timestamps are Unix
seconds.

Each record travels as a private `3188` artifact: signed by its author,
NIP-44 v2 encrypted to exactly one recipient, `p`-tagged to that recipient,
with a random `h` mailbox and `t: oa:artifact:v1`. The envelope's `issued_at`
equals the event's `created_at` and the body's own time field. A reader checks
the original signer against the expected author before trusting any field.

## Owner host directory

The directory is the owner's list of hosts. The owner signs it and encrypts it
to the owner's own key, so only holders of the owner key can read it. The
schema is `openagents.host-directory.v1`:

| Field | Meaning |
| --- | --- |
| `owner` | The owner key. It equals the signer and the recipient. |
| `revision` | Unsigned counter. The highest valid revision is current. |
| `issued_at` | When the owner issued this revision. |
| `hosts` | Up to 256 entries, unique by host key. |

Each entry is `{host, label, relays, weight, added_at}`, with an optional
`worlds` list:

- `host` is the host key. It differs from the owner key.
- `label` is 1 to 64 bytes of display text without control characters. It is
  never an identity.
- `relays` lists up to eight credential-free `wss` URLs without a query or
  fragment, where the host publishes presence and accepts relay-carried
  control. As with relay hints, a loopback test relay may use `ws`.
- `weight` is the owner's placement weight, 0 to 1,000. Zero keeps the host
  listed but excludes it from placement.
- `added_at` is no later than `issued_at`.
- `worlds` lists up to 16 world instances the host serves over its direct
  channel, unique by `instance`. Each is `{instance, label, wire, content?}`:
  a nonzero instance number the world's opening challenge names, display text
  under the `label` rules, the chamber wire version, and an optional
  lowercase-hex SHA-256 of the zone content the instance binds. An entry
  without worlds omits the field. A listed world grants nothing: joining it
  takes a NIP-HOST grant with the `world` right.

Adding or removing a host, or changing its label or weight, is an owner action
that produces the next revision.
A host cannot add itself: a reader opens a directory only when the owner key
signed it to itself and the body names that owner. A host-signed copy refuses
as `identity_mismatch`, whatever its body lists. Two different bodies at the
highest revision are a `conflict`; the client shows the conflict and waits for
the owner to publish a higher revision rather than merging them. An owner
device may end the conflict by publishing one body it trusted above the
conflicting revision.

The owner reuses one random mailbox for its directory revisions so its own
devices can filter by `#h`. A holder of the owner key sees every retained
revision and selects the highest. Older revisions stay readable until their
`retain_until` passes; they never override a higher revision.

A client that does not hold the owner key cannot read the directory. It learns
about hosts through enrollment (NIP-HOST) and can list only the hosts it was
enrolled with.

### Owner authority on a client

A client reads or publishes the directory only with the owner secret key in
its own protected store. It holds that key in one of two ways, and both are
local to the device:

1. Its device key is the owner key: a grant the client holds names the
   client's own key as `owner`.
2. The person enters the owner secret key on the device. The client accepts
   it only when its public key is the `owner` of a grant the client holds,
   so a mistyped or unrelated key is refused before it is saved.

The owner key never travels in a directory, presence, hint, invitation,
grant, or relay message. This profile defines no re-encryption of the
directory to device keys: a device without the owner key stays limited to its
enrolled hosts.

A client that holds the owner key follows these rules:

- It reads retained revisions from the relays named by the grants it holds
  for that owner's hosts and by the entries of the last directory it
  trusted, and selects the current revision as above. It keeps the highest
  revision it has read or published and never replaces it with a lower one.
- It lists a directory host it holds no grant for as not enrolled. A
  directory entry grants nothing and carries no connection.
- It uses each entry's `label` as the host's display text and each entry's
  `weight` for placement.
- It publishes a new revision only after a successful read with no conflict,
  under the mailbox of the revision it read, or a fresh random mailbox when
  none exists, to the same relays.

## Host presence

A host publishes presence to each enrolled device as a separate artifact,
signed by the host key and encrypted to that device. A host may instead use a
relay whose admitted private policy restricts reads to enrolled devices; the
body is the same. The schema is `openagents.host-presence.v1`:

| Field | Meaning |
| --- | --- |
| `host`, `owner` | The host key, equal to the signer, and the owner whose directory lists it. |
| `generation` | Unsigned counter that increases whenever the host restarts, updates its binary, or restores state. |
| `protocol` | The positive NIP-REACH version the host speaks. |
| `compatibility` | `{min, max}`, the inclusive range of client versions the host accepts. |
| `capabilities` | Up to 64 unique slugs the host advertises. |
| `observed_at` | The host clock when it took the sample. |
| `telemetry` | `null`, or `{cpu_count, cpu_utilization_pct, memory_available_pct}`. |

Telemetry is deliberately coarse. `cpu_count` is 1 to 4,096 logical CPUs.
Utilization and available memory are whole percentages, 0 to 100. No other
field is permitted, so a host cannot report process names, users, paths, load
history, or finer measurements. A host may withhold telemetry with `null`;
placement then skips it.

A reader opens presence only from a host that the owner's directory, or the
device's own enrollment, names, and only when the body names the same host and
owner. Presence from any other signer refuses.

### Freshness

A reader judges freshness by its own receipt time, not by the host's clock:

1. Record `received_at` from the reader's clock when the event arrives.
2. Refuse the sample if `observed_at` is later than `received_at` plus the
   allowed skew. It is from the future. The reference skew is 30 seconds.
3. Treat the sample as stale when `now − received_at` exceeds the maximum age,
   or when `received_at − observed_at` exceeds it. The second rule catches an
   old sample replayed late. The reference maximum age is 180 seconds.
4. Keep the newest sample per host. Refuse a lower `generation` than one
   already held, and a sample that is not newer within the same generation, so
   a replayed sample cannot roll the host back.

A host republishes presence when its generation, capabilities, or reachability
changes, and otherwise no more than once every 60 seconds. The client shows
transport health and data freshness separately: a stale sample says the data
is old, not that the host is down.

### Compatibility rule

A client reads the host's advertisement instead of assuming its own version's
behavior:

- The host must accept the client's version: `compatibility.min ≤ client ≤
  compatibility.max`.
- The client must accept the host's `protocol` version.
- A client uses a feature only when the host lists its capability flag.
  Capability flags the client does not recognize are ignored, never treated as
  supported. An unrecognized entry in `requires` refuses the whole record.

Failure of either version check is `incompatible`. The client shows the host
as incompatible and does not retry until the presence changes.

## Reachability hints

A host publishes endpoint hints to each enrolled device, signed by the host
key and encrypted to that device. The schema is `openagents.reach-hints.v1`:

| Field | Meaning |
| --- | --- |
| `host` | The host key, equal to the signer. |
| `generation` | The host generation these hints describe. |
| `issued_at`, `expires_at` | Validity, at most 24 hours. `expires_at` equals the envelope's `retain_until`. |
| `hints` | Up to 16 hints, unique by transport and address. |

Each hint is `{class, transport, address, status, observed_at}`:

| Field | Values |
| --- | --- |
| `class` | `loopback`, `lan`, `tailnet`, `public`, or `relay`. |
| `transport` | `tcp` (address `host:port`), `websocket` (a `ws` or `wss` URL), or `nostr` (a relay URL). The `relay` class uses exactly the `nostr` transport. The v2 record adds `iroh` ([iroh hints](#iroh-hints)). |
| `status` | `reachable`, `unreachable`, or `unknown`: what the host last observed. |
| `observed_at` | When the host observed that status, no later than `issued_at`. |

A hint is a claim, not a grant or a proof. The address rules are:

- The `loopback` class requires a loopback address: `127.0.0.0/8`, `::1`, or a
  `localhost` name. The `lan`, `tailnet`, and `public` classes refuse loopback
  addresses.
- Unspecified, multicast, and broadcast addresses refuse.
- URLs carry no user information, query, or fragment. No hint carries a
  secret: a token in an address would leak through logs and relays.
- A relay hint uses `wss`, except that a loopback test relay may use `ws`.
- Tailnet addresses are ordinary hints. No class requires software from a
  particular network provider.

### iroh hints

A host with an iroh endpoint also describes it as a hint with transport
`iroh`. A v1 reader refuses an unknown transport, and with it the whole
record, so iroh hints travel only in `openagents.reach-hints.v2`. Its body
is the v1 body under the new version, and it adds the `iroh` transport. A
host that publishes iroh hints publishes both records to each enrolled
device for the same generation: v1 without iroh hints, for older readers,
and v2 with them. A reader that understands v2 uses the v2 record of the
current generation and ignores v1 for it; with no v2 record, it uses v1.

An `iroh` hint is
`{class, transport: "iroh", address, relay, direct, status, observed_at}`:

| Field | Values |
| --- | --- |
| `class` | `public`. iroh chooses between a direct path and its relay itself, so the hint names no narrower class. |
| `address` | The host's `EndpointId`: 64 lowercase hex characters, the iroh Ed25519 public key. |
| `relay` | `null`, or the iroh relay URL the host keeps its home connection on: an `https` URL of at most 128 bytes without user information, query, or fragment. |
| `direct` | Up to 8 unique socket addresses the host observed, each `ip:port` with an IPv6 address in brackets and a nonzero port. |
| `status`, `observed_at` | As for every hint. |

A record carries at most one `iroh` hint. The address rules above apply to
each `direct` entry: unspecified, multicast, and broadcast addresses refuse
the record. A reader on another machine drops loopback `direct` entries
before dialing. An `iroh` hint with neither a `relay` nor a `direct` entry
refuses. A reader dials the `EndpointId` with only the hint's relay and
direct addresses, plus the ones it saved at enrollment, as address sources.

The `EndpointId` is a route, never an identity. It proves nothing about the
host key, and it admits nothing: the [handshake](#handshake) proves the host
key and the grant check admits the channel, as over every other transport.

Selection with v2 hints tries the `iroh` hint first, then the v1 order below.
An iroh connection that fails, or that completes but whose handshake fails,
moves on to the next hint.

### Selection

The client orders hints and tries them in turn. Only the connecting device can
prove a route, by completing a direct-channel handshake or an authenticated
relay exchange.

1. Refuse the whole set as `stale` when it names another generation than the
   current presence, or when it has expired.
2. Skip hints the host observed as `unreachable`.
3. If the client is on another machine, skip every hint whose address is
   loopback, in any class, including a loopback relay.
4. Order direct classes before relay fallback. On another machine the order is
   `lan`, `tailnet`, `public`, `relay`. On the same machine `loopback` comes
   first.
5. Within a class, try `reachable` before `unknown`, then keep the host's order.

Selection never falls back to loopback. An empty result means that no
shareable route exists; the client reports that instead of trying a local
address. A client claims to be on the same machine only from local evidence,
such as a local socket that its own user owns. A matching address is not
evidence.

A client may also hold a local route: a loopback address on its own machine
that reaches the host through a forwarder the client itself runs, such as the
local port of an SSH tunnel. Its own forwarder is the local evidence, for that
one address only. The client may try it before the selected hints, proves it
with the same handshake, never publishes it or shares it with another device,
and drops it when the forwarder ends. The host's loopback hints still follow
the rules above.

## Direct channel

A direct channel carries host traffic over TCP, WebSocket, or an iroh
connection without a Nostr relay. Relay-carried control stays the fallback
when no direct route works.

### Frame format

Every frame is:

| Bytes | Field |
| --- | --- |
| 4 | Length of the rest of the frame, unsigned big-endian. |
| 1 | Frame kind. |
| 8 | Sequence number, unsigned big-endian. |
| rest | Body. |

The length counts the kind, sequence number, and body. A frame longer than
65,536 bytes, or shorter than 9, refuses before its body is read. Each
direction numbers its frames from 0 and increases by one; a gap, repeat, or
reordering refuses and closes the channel. Over WebSocket, each binary message
carries exactly one frame, length prefix included; the
[WebSocket mapping](#websocket-mapping) gives the rules.

| Kind | Name | Body |
| --- | --- | --- |
| 1 | Client hello | JSON, plaintext. |
| 2 | Host proof | JSON, plaintext, signed. |
| 3 | Client proof | JSON, plaintext, signed. |
| 4 | Verdict | Encrypted JSON. |
| 5 | Refusal | JSON, plaintext, sent only before the host proves its key. |
| 16 | Data | Encrypted application bytes, at most 16,384 per frame. |
| 17 | Close | Encrypted, empty. |

Unknown kinds refuse. Handshake messages are strict JSON of at most 4,096
bytes.

### WebSocket mapping

A `websocket` hint carries the same direct channel over a WebSocket
connection (RFC 6455). The handshake, transcript, encryption, sequence
numbers, frame kinds, and bounds are the ones TCP uses. Only the carriage of
frames differs:

- The client opens the hint URL exactly as written. A `ws` URL uses a plain
  connection and a `wss` URL uses TLS. The channel authenticates and encrypts
  itself either way, so a `ws` hint is valid in every direct class. A host
  serves the channel on every path of its WebSocket listener, so a forwarder
  may rewrite the path.
- A `wss` endpoint is either the host's own listener, which terminates TLS
  with a certificate for the URL's host name, or a forwarder that terminates
  TLS in front of it. The client verifies the certificate as any TLS client
  does: the chain leads to a root it trusts, and the certificate is valid
  for the URL's host name and at the current time. A failed verification
  fails that route, and the client tries the next hint.
- TLS does not replace the channel's authentication. The host and device
  keys, the grant check, and the channel encryption are the same under TLS,
  and a certificate proves nothing about the host key. TLS adds two things:
  clients that accept only `wss`, such as a browser page served over
  `https`, can connect; and observers on the path no longer see the
  handshake's plaintext fields, such as the device key, host key, and grant
  ID in the client hello. They still see the endpoint's address, the TLS
  server name, and the timing and approximate sizes of messages.
- Neither side offers or selects a subprotocol or an extension. A host
  ignores an offered subprotocol.
- Each binary message carries exactly one frame, length prefix included. A
  message whose size is not 4 plus its prefix refuses as `malformed`: a frame
  never spans messages, and a message never carries two frames. The prefix
  rules still apply: a prefix over 65,536 refuses as `limit_exceeded`, and one
  under 9 as `malformed`.
- A message over 65,540 bytes, one largest frame and its prefix, refuses as
  `limit_exceeded`. A receiver applies this bound to each WebSocket frame
  header and to the running size of a fragmented message, so an oversized
  message refuses before its payload is read.
- A text message refuses as `malformed`. Ping and pong messages carry no
  channel data and advance no sequence number.
- A WebSocket close ends the transport, as end of stream does over TCP. Only
  an encrypted close frame (kind 17) shows that the peer closed the channel;
  a WebSocket close without one is a transport failure. An endpoint sends its
  close frame before it closes the WebSocket connection.
- The host's handshake time limit covers the WebSocket upgrade as well as the
  channel handshake.

### iroh mapping

An `iroh` hint carries the same direct channel over an iroh QUIC connection.
The handshake, transcript, encryption, sequence numbers, frame kinds, and
bounds are the ones TCP uses:

- The client connects to the hint's `EndpointId` with the ALPN
  `openagents/reach/1`, and opens one bidirectional stream. The frames run on
  that stream exactly as on a TCP connection, length prefix included, in
  both directions. A connection carries exactly one channel on exactly one
  stream; the host resets any other stream.
- iroh's TLS proves only that the far end holds the `EndpointId`. The client
  still verifies the host proof against the host key it expected, and a
  wrong key refuses as `identity_mismatch`. The host still verifies the client
  proof and checks the grant. Neither side treats the other's `EndpointId`
  as an identity, keys a grant, epoch, or replay entry by it, or records it
  as a device.
- The channel keeps its own NIP-44 encryption inside QUIC's. The double
  encryption is deliberate: it keeps one channel construction for every
  transport.
- Finishing the stream ends the transport, as end of stream does over TCP.
  Only an encrypted close frame (kind 17) shows that the peer closed the
  channel; a stream reset, a connection close, or an idle timeout without one
  is a transport failure.
- A host that closes a channel because its grant stopped admitting it sends
  the close frame, finishes the stream, and then closes the connection.
- The host's handshake time limit covers the QUIC handshake as well as the
  channel handshake. Hosts accept `openagents/reach/1` only on the endpoint
  whose `EndpointId` they publish, and bound concurrent handshakes as for
  every transport.

### Handshake

1. The client sends a hello, `openagents.reach-hello.v1`, with `client` (the
   device key), `host` (the key it expects, from the directory or
   enrollment), `grant` and `epoch` (its current grant ID and revocation
   epoch), `generation` (from fresh presence), a random `nonce`, a fresh
   `ephemeral` x-only key, and `issued_at`.
2. The host refuses a hello that names another host key, lies outside a
   120-second window of its clock, or reuses a nonce it saw within twice that
   window. It otherwise sends a host proof, `openagents.reach-host-proof.v1`,
   with its own random `nonce`, a fresh `ephemeral` key, its actual
   `generation`, and a `signature`.
3. The client verifies the host's signature with the key it expected. A wrong
   key refuses as `identity_mismatch`. A signed generation that differs from
   the one the client expected refuses as `stale`; the client reads fresh
   presence before trying again. The client then sends a client proof,
   `openagents.reach-client-proof.v1`, with its `signature`.
4. The host verifies the client's signature with the device key from the hello.
   Only then does it check the grant and generation. It sends an encrypted
   verdict, `openagents.reach-verdict.v1`, whose `code` is `null` when it
   admits the channel, or a refusal code: `revoked`, `stale` (another epoch,
   an expired grant, or another generation), or `not_admitted` (unknown
   grant).

The **transcript** is SHA-256 of `openagents.reach-channel.v1`, a zero byte,
and the RFC 8785 canonical JSON of `client`, `host`, `grant`, `epoch`,
`expected_generation`, `client_nonce`, `client_ephemeral`, `issued_at`,
`host_nonce`, `host_ephemeral`, `generation`, and `max_frame` (65,536). The
host signs SHA-256 of `openagents.reach-host-proof.v1`, a zero byte, and the
transcript. The client signs the same construction with
`openagents.reach-client-proof.v1`. Both are BIP-340 signatures. Distinct
labels prevent reflecting one side's proof as the other's.

The session keys are SHA-256 of a direction label (`openagents.reach-c2h.v1`
or `openagents.reach-h2c.v1`, each with a trailing zero byte), the NIP-44 v2
conversation key of the two ephemeral keys, and the transcript. Because the
ephemeral keys are discarded after the channel closes, a later compromise of a
long-term key does not reveal recorded traffic.

An encrypted frame body is the NIP-44 v2 payload, under the direction's
session key, of the base64 encoding of the frame kind, the sequence number,
and the data. The receiver checks that the decrypted kind and sequence number
equal the header. A frame that fails authentication closes the channel.

### Refusals and rechecks

A refusal frame sent before the host proves its key is advisory: anyone on the
path could have sent it. A client treats its code as a transport failure, not
as a statement about its grant. After the device proves its key, the host
states grant refusals in the encrypted verdict, so a scanner cannot learn
whether a grant ID is revoked.

The channel binds one grant ID, one epoch, and one host generation. The host
rechecks the grant and generation before each operation and closes the channel
when either changes. Opening a channel grants no right; each operation still
requires its own right under the device's grant.

The host proof reveals the host key and generation to anyone who can open a
connection to a hinted address and names that key. Hosts listen only on
addresses they publish as hints, and they bound concurrent handshakes and
their replay memory. A full replay memory refuses new handshakes as
`limit_exceeded` rather than forgetting nonces early.

## Placement

Placement picks one host for new work. It is a pure function of the directory,
the newest accepted presence per host, the client's profile, and the reader's
clock. It never moves existing work, and it never substitutes for the host's
own admission.

1. Skip a host the client may not start work on.
2. Skip a host with weight 0. The weight comes from the owner's directory; a
   client may apply a local override.
3. Skip a host without presence, with stale or future presence, or with
   incompatible presence.
4. Skip a host that withholds telemetry.
5. Skip an overloaded host: CPU utilization at or above the limit (reference
   90 percent), or available memory below the limit (reference 10 percent).
6. Score the rest as `weight × cpu_count × (100 − cpu_utilization_pct) ×
   memory_available_pct`, in exact integer arithmetic.
7. Choose the highest score. Break ties by the lexicographically smallest host
   key, so every client with the same inputs agrees.

When no host qualifies, the client reports why each host was skipped.

## Privacy and disclosure

- The directory is readable only with the owner key. Presence and hints are
  readable only by the device each copy is encrypted to.
- Visible tags still show the signer, the recipient, the mailbox, and timing.
  Publishing presence reveals that a host is running and how often it reports.
- No record, hint, or frame carries a credential, an invitation, a prompt, a
  path, or a transcript. Terminal and task content travels only inside direct
  channel frames or relay-carried operations of their own profiles.

## Kind allocation

REACH allocates no kinds. Every record is a private `3188` artifact with a
REACH schema. A relay-replaceable kind would let a relay keep only the newest
directory, but it would add a public addressable coordinate for a private
record and a draft kind registration without adding authority: the directory
already carries its own `revision`, and readers already select the highest.
If retained revision volume becomes a problem, a later version can justify a
replaceable private kind and check the OpenAgents kind table for collisions.

## Implementation status

[`crates/coder-reach`](../../crates/coder-reach/README.md) implements the
three schemas, sealing and opening through the shared private artifact
functions, freshness and compatibility, hint validation and selection, the
handshake and frame format over any ordered byte stream, the WebSocket
mapping, and placement. The same handshake and frame tests run over TCP and
over WebSocket. Grant checks go through a trait, so the crate does not depend
on a grant store. It also splits an open channel into a reader and a writer.

[iroh hints](#iroh-hints) and the [iroh mapping](#iroh-mapping) are
implemented. `coder_reach::hints::v2` reads and writes the v2 record and its
`iroh` hint; `crates/openagents-connect` carries the channel over one QUIC
stream; and the resident host, with `--iroh`, serves `openagents/reach/1`
through the same session as TCP and publishes the v2 record beside v1.

[`crates/coder-host`](../../crates/coder-host/README.md) is the resident host
and its client. The host seals presence and hints to each enrolled device,
serves TCP direct channels and, when configured, WebSocket direct channels,
as `ws` or as `wss` with the operator's certificate and key, with the real
NIP-HOST grant store behind the grant check, rechecks the grant
before each message, and closes a channel whose grant stopped admitting it.
Its client reads the owner directory and the host's presence and hints, and
tries selected `tcp` and `websocket` routes before relay fallback under a
`coder-link` supervisor, verifying a `wss` certificate against the bundled
WebPKI roots. It reports telemetry: the logical
CPU count, the one-minute load average per CPU as CPU use, and the kernel's
share of available memory, each as a whole number, or `null` when the host
cannot read a value or its operator turns telemetry off.

The [Computers screens](../../crates/coder-computers/README.md) show each
host's supervised status, route class, and compatibility from this presence.
Their live service applies the owner-authority rules above: it holds the
owner key only as described, lists directory hosts with their labels and
weights beside enrolled hosts, shows an unenrolled directory host as not
enrolled, publishes the next revision when the owner adds an enrolled host,
and feeds directory weights to placement. The
[reach verification record](../../docs/coder/verification/2026-09-26-host-reach.md),
the [host serve record](../../docs/coder/verification/2026-09-26-host-serve.md),
the
[WebSocket channel record](../../docs/coder/verification/2026-09-27-websocket-channels.md),
the [host `wss` record](../../docs/coder/verification/2026-09-27-host-wss.md),
and the
[client directory and SSH record](../../docs/coder/verification/2026-09-27-client-directory-and-ssh.md)
list the checks that ran and their limits.

## Conformance

Fixtures must cover a directory round trip, a host-signed directory, duplicate
and conflicting revisions, presence from an unlisted signer, future and stale
samples, generation rollback, unknown capability flags and required features,
version ranges that do not overlap, mislabeled loopback hints, hints with
credentials in URLs, selection with and without shareable endpoints, placement
edge cases, and handshakes with a wrong host key, an impersonating host, a
replayed nonce, a revoked grant, a wrong epoch, a stale host generation, a
stale hello time, and an oversized frame. A WebSocket implementation runs
the handshake cases over WebSocket too, and adds a message that carries two
frames, a message cut short, and a message over the message bound. An iroh
implementation runs the handshake cases over an iroh stream, and adds a
v2 record with two `iroh` hints, an `iroh` hint with an uppercase or short
`EndpointId`, a relay URL over the bound or with a query, more than eight
`direct` entries, a second stream on one connection, and a connection
closed without a close frame.

The wire fixtures in
[`crates/coder-reach/fixtures/nip-reach.json`](../../crates/coder-reach/fixtures/nip-reach.json)
give valid directory, presence, and hint bodies; invalid bodies with the
refusal code each must produce; placement vectors with each candidate's
assessment and the chosen host; a transcript vector with the exact SHA-256
both channel proofs sign; and WebSocket message vectors, each one binary
message with the code a receiver refuses it with, or `null` when it carries
exactly one frame. `crates/coder-reach/tests/wire.rs` checks them. The transcript vector was also computed independently of the crate
from this section's construction.

Advertise `nip-reach-v1` in NIP-11 `supported_extensions` only for a relay
whose private-artifact policy delivers these records to their exact
recipients. Hosts and clients state their role in their own capability
records. Keep this draft name out of numeric `supported_nips`.
