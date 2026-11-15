# Nostr application services

2026-11-15, #13626, part of [the Arcade delivery](../arcade/08-delivery.md).
This defines the minimal installed public-feed read contract and the alpha
custody boundary for later messaging. Neither contract is an installed service
until its host consumer ships. Existing account identity/settings, login,
backup, clipboard and chat contracts retain their identities and permissions.

## Ownership and identity

The account's current public identity comes from
[`account-identity-v1`](account-identity-v1.md). A managed identity stays on its
account; an independently linked identity stays independently held. Opening
the feed, reconnecting or importing a login key never mints or rotates a
canonical identity. Login authenticators, device refresh credentials and
wallet seeds are separate keys.

The current managed identity secret is sealed on the server. For this first
messaging implementation, its provider is the account's server custody service.
An independently linked identity uses a device-held key or an explicitly
authorized remote signer. The generic UI receives public key references and
action results, never a secret key, conversation key or ephemeral secret.
No automatic backup download provisions a device; the existing backup still
requires its explicit fresh-verification/device-authentication ceremony.

**Trust boundary:** the service holding a managed secret can sign and decrypt
for that identity. Relay encryption does not hide managed-account messages
from that custodian. User-facing privacy claims must name that boundary.
Independently held keys must not silently fall back to server custody when a
device or remote signer is unavailable.

`nostr-capability.coax` supplies a necessary gate over trusted facts: live,
human, full/unfrozen session; matching current identity and provider; approved
purpose; valid source/provider combination. These facts are host/server
observations, never fields a caller can assert. The gate does not replace
per-action payload validation, actual key possession, cryptographic checks,
fresh consent where needed or current server authorization. An email session
does not become L2 merely because its managed key exists.

## Public relay reads: version 1

Schema/profile: `proto/coax-native/relay-read-v1.json`.
One codec: `coax/native/relay-read-wire.coax`; installed admission:
`relay-read-policy.coax`. Native operation **20**, success event **120**,
failure event **220**. Existing operation 19 is clipboard v2 and is preserved.
The web adapter consumes the same request/observation codec with browser
WebSocket effects. Native and browser effects do not decide which notes match.

Every request is a closed object with these required fields:

| Field | Meaning |
| --- | --- |
| `schema` | Exact registered version |
| `action` | `open`, `send`, `poll`, `close`, `reset` or `clock` |
| `slot` | 0–5; reset uses 0 |
| `generation` | Caller-owned positive nonwrapping 31-bit generation; reset uses 0 |
| `sequence` | Poll's acknowledged delivery serial, 0 before delivery; close can acknowledge too |
| `payload` | Open: exact saved WSS URL; send: complete REQ/CLOSE JSON; otherwise empty |

Open/send/clock use sequence 0. Reset uses slot/generation/sequence 0 and empty
payload, and completes with an empty service body after retiring all slots.
Clock uses a positive caller generation, empty payload and no socket; its
observation has kind `clock`, sequence 0 and exact nonnegative integer Unix
seconds (at most 9,007,199,254,740,991) as a decimal payload. This is a wall
clock observation for time filters, separate from elapsed animation ticks.
The native clock effect and browser Date supply the number; Coax encodes and
validates it. It is not a new delivery serial or authentication fact.
Unknown, duplicate, escaped-alias, missing or wrongly typed fields
are refused. Decoded strings are complete UTF-8; incomplete strings are never
returned as successful prefixes. The request contains no account, cookie,
authorization header, private key or signer handle.

Codec writers return the positive byte count of a complete record or -1.
Bytes in an output buffer after -1 are unspecified and must never be
consumed, queued or published; the adapter clears scratch before reuse.
Decoded record fields are usable only when the reader returns true with
`ok=true`. Preflight checks duplicated by a final whole-record validation can
therefore be equivalent for admission; each such mutation annotation names
the repeated validator and never claims failed scratch is a valid packet.

### Effects and admission

Open only an explicitly selected URL from the current account's saved relays.
The Coax app reconciles that selection; the host applies the shared URL and
read-command admission before any SDK effect. WSS only, at most 256 bytes;
preserve its exact path/query. Refuse credentials/fragments/unsafe URI forms
using the shared URI module. Native sessions disable cookies/credential
storage and never attach account bearer tokens. Native WebSocket redirects
must not bypass the approved destination; refuse redirects. The browser
WebSocket API cannot disable browser-managed cookies: its adapter sends no
application tokens/headers, retains browser origin rules and must not claim
credential-free transport. Any stricter isolation requires a separately
qualified carrier; it cannot be inferred from calling WebSocket.

Send admits complete NIP-01 REQ (one or more filter objects, nonempty
subscription ID up to 64 decoded bytes) and CLOSE frames. It does not admit
EVENT or AUTH. Filters and relay replies use the existing Nostr codecs, not a
second JSON/NIP implementation. This capability cannot sign, publish or read
encrypted inboxes requiring AUTH. A relay demanding authentication is shown as
unavailable for public reads; messaging adds an explicitly authorized service.

Limits: six slots; outgoing frame 8 KiB; incoming text frame 64 KiB; encoded
bridge record 400,000 bytes. The latter holds worst-case JSON escaping of a
whole 64 KiB frame inside the app's 1 MiB event inbox. Binary frames and
invalid UTF-8 refuse the connection. The platform applies SDK message limits
before decoding/copying into retained buffers.

### Observations and acknowledgement

An observation is the same closed field set, with `kind` replacing `action`.
Kinds: `connecting`, `open`, `message`, `closed`, `overflow`, `idle`, `failed`, `clock`.
`payload` is the exact untrusted text frame for `message`, or bounded public
diagnostic text for a terminal state. It is not a verified event or authority.
The application checks the Nostr hash/signature and its current filter before
retaining/displaying content; relay metadata is verified separately.

Each slot retains ordered delivery records with monotonic serials until a
later poll acknowledges the serial the app processed. A poll replays the first
unacknowledged record, so an interrupted UI callback cannot silently lose it.
The host refuses an acknowledgement beyond what it delivered. An idle
observation uses the current acknowledged serial; it is not a new record.
At most 64 records and 1 MiB of decoded payloads are retained per slot.
Exhaustion closes the socket and exposes overflow/incomplete status; no
successful partial event or hidden dropped record is allowed. Event IDs
deduplicate repeated deliveries above this transport.

Every asynchronous callback captures slot, generation and the native program
epoch (browser adapter mount epoch). Replacing/closing invalidates the owner
before returning; late SDK completion cannot populate a replacement slot.
Generations/serials retire at 2,147,483,647 rather than wrapping. Closing a
retired generation does not close its replacement. Reset/logout/module
replacement retire every slot and erase queued records. Background suspends
reads; foreground resumes with a fresh generation and shared cursors. Source
clocks, retries and page/filter generations remain Coax state.

The feed owns the first implementation in #13627. It needs no private key,
custody service, messaging journal or full animation catalog. Messaging #13628
extends this session/transport through an explicitly versioned capability;
it cannot widen the read-only v1 send gate in place. Publishing and AUTH need
an independently authorized/versioned capability on the same socket carrier;
the existing read-only operation keeps refusing both frames.

## Messaging custody: alpha envelope

Profile: `proto/coax-native/nostr-custody-v1alpha1.json`.
Codec: `coax/native/nostr-custody-wire.coax`. Operation **21** is reserved;
the feed does not install it or pretend a stub succeeded.

Required envelope fields are `schema`, `action`, `operation`, `identity`,
`payload`. The operation ID is 64 lowercase hex bytes, chosen before dispatch
and reused when its result is uncertain. Identity is the expected 64-hex
public key, verified against the current authorized identity; it cannot select
another account. Payload is a complete UTF-8 string up to 64 KiB, and the
encoded envelope is at most 400,000 bytes. Unknown/repeated fields refuse.
No `account`, `strength`, private-key import or arbitrary hash-signing action
is accepted. Envelope admission alone never performs cryptography.

| Action | Required effect contract before implementation |
| --- | --- |
| `capabilities` | Report actual current public identity/source/provider readiness and supported actions; no key generation or privilege increase |
| `relay-auth` | Sign a verified current NIP-42 challenge bound to the explicit WSS URL, connection generation and device/session; refuse stale/reused ownership |
| `seal` | Validate a complete NIP-17 rumor and every recipient's verified relay readiness before producing NIP-59 wraps; sender copy included; private/ephemeral keys stay with provider |
| `open` | Verify original wrap ID/signature/recipient, NIP-44 integrity, seal identity and rumor admission before returning bounded plaintext to the authorized conversation |
| `publish-list` | Reconstruct a named supported follow/relay/mute/group/app-data list after reading current verified state, preserving unknown tags; no arbitrary event template |
| `group` | Construct a supported NIP-29 action with explicit relay/group/role/target and verified authority; client sign does not prove relay acceptance |

These payloads/responses must receive their own versioned profiles, fixtures
and codecs in #13628–#13632 before use. The table fixes their purposes and
trust requirements without declaring a generic raw-sign endpoint. Profile
publication continues through the existing saved-profile service. NIP-46 is
an explicit external provider, never a fallback that changes identity.

Managed cryptography executes in the authenticated account service with
Coax-built policy/crypto and sealed-key native effects. The server validates
the session/device, canonical identity, action schema and permitted purpose;
it never opens a caller-supplied relay connection. It returns bounded signed
events/envelopes for the client to publish. Local and remote providers satisfy
the same purpose contract with their actual possession/permission checks.
Plaintext and key scratch are wiped after use and excluded from logs/metrics.

An operation is persisted/resolved before uncertain retry can make another
envelope. Return the same public event/result for that operation, or an
explicit expired/unavailable result; never claim a retried send is a new
message. Durable signed ciphertext/outbox bytes are retained before publish.
Plaintext-at-rest policy, indexes, account namespace, retention/pruning and
wipe belong to the registered messaging store profile in #13628.

Refusals remain distinct: unsupported, not-ready, identity-mismatch,
session-expired, purpose-refused, revoked, capacity, cancelled. Missing entropy,
key material, seal access or network produces refusal; no replacement key,
backup export or hosted-agent message is substituted. Cancellation retires
the operation's effect owner without falsely acknowledging relay delivery.

## Conversation and release integration

The registered `coax.native.conversation.v1alpha1` presentation codec/profile
uses closed fields `schema`, `kind`, `id`, `scope`. `kind` distinguishes
`account-v1`, `account-v2`, `environment`, `project`, `world`, `nip17`, `nip29`.
ID and scope are each 1–256 complete UTF-8 bytes; the encoded record is at most
4096 bytes. This is a reference, not permission or a stored transcript.
Its own registered fixture is checked by `nostr-profile.coax`.

History references qualify their destination: existing account thread,
environment/session, project/world binding, NIP-17 participant set or
relay-qualified NIP-29 group. Their original protocol/storage IDs remain
unchanged. The participant-set ID is local; the relay/group pair scopes a
public group. Shared display does not move agent commands onto Nostr or make
message content executable. Persisted index envelopes get their own proto
shape/codec when the corresponding messaging consumer lands.

Transport generation and active-filter generation are separate. Changing a
filter cancels its subscription and rejects old-generation callbacks; public
kind-1 feed state cannot become an inbox event or vice versa. Existing
durable-chat v2alpha1 and legacy v1 readers retain their contracts.

Qualification: pinned request/observation bytes and closed-field/boundary
checks; trusted-custody fact matrix; URL and read-only frame refusals; exact
untrusted frame round trips. Later host adoption adds real TLS/redirect,
queue/ack/cancellation, foreground lifecycle and raw-byte effects tests.
Messaging adds independent NIP-44/59 vectors, identity mismatch, revoked
provider, operation recovery and account-isolation evidence. Defining these
contracts does not qualify an installed host or production signing API.

The first TestFlight upload occurs after the feed works. Upload accepted,
Apple processing, tester assignment and actual availability are recorded
separately. Work continues during processing, as the owner requested.
