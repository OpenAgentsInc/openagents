# First Nostr mirror slice

This is a field/consumer map for the exact sources in MANIFEST.json. It is not
a conformance claim. The Block projections use the v1alpha1 event/JSON helpers;
an opt-in v1alpha2 event profile constrains kinds to 0..65535. Neither profile
allocates new Nostr kinds or wire protocols.

| Source | Projection | Important preserved meaning |
| --- | --- | --- |
| Official 01/50 | Event, SignedEvent, UnsignedEvent, EventRef, Filter, request/relay frame shapes | Original JSON, fixed-size binary IDs/keys/signatures, uint64 Unix seconds, kind range, ordered string-array tags, exact content, missing versus empty filters; unknown extension JSON retained. |
| Official 42 | Authentication | Original kind 22242 event, relay URL and challenge; freshness and per-connection key proof stay external. |
| OA | OwnerAttestation | Exact condition string, owner key and signature bytes; never reorder clauses or reconstruct the signature preimage from parsed conditions. |
| AA | AgentAuthentication | NIP-42 event plus optional OA credential; direct membership versus virtual membership is a live relay decision. AA evaluates timestamp clauses rather than kind clauses at login. |
| AE | Engram, DecryptedBody, Core, Memory | 30174 ciphertext envelope, blinded d and owner p; private plaintext has slug/profile or slug/value, with explicit null tombstones and unknown JSON fields. Signature precedes decryption; HMAC address derivation is not performed here. |
| AO | ObserverFrame, Telemetry, Control | 24200 recipient/agent/frame direction; private seq/timestamp/kind/payload and nullable correlation fields. Unknown control/frame kinds do not authorize actions. Ephemeral frames must not be persisted, indexed or logged. |
| AM | TurnMetricEvent, TurnMetric, Usage, PricingIdentity | 44200 ciphertext; exact counters and decimal costs, nullable usage, omitted cache counts, cumulative session/turn sequence, delta reliability and independently supplied pricing identity. No channel tag or inferred price. |
| AP | PersonaEvent/Persona, TeamCatalog, AgentSettings, AgentInstructions, TeamConfig, ConfigState | 30175/30178, nullable prompts/commands/model fields, portable command rules, client-defined share body, versioned AR-backed settings/instructions/team config. Unknown fields stay in raw JSON. Missing/deleted/unavailable config are distinct. |
| FI | Assertion, AssertionClaims, Audience, DisconnectCommand | Original compact JWS and decoded header/claim JSON, exact issuer+subject, string-or-array audience, exact NumericDates, explicit agent-owner claim and access-token client ID. Signature, issuer/community policies, JWKS, time/deny maps and owner validity are not transport claims. |
| FA | LoginStart, CodeExchange, Session, AssertionRequest/Response, Denial | HTTP query/body values, credential-bearing handoff/session/assertion bytes, RFC3339 expiry and private nullable profile projection. HTTP status, origin/redirect rules, secret binding, proof and retry logic remain runtime obligations. |
| IA | ArchiveRequest/Delta/Snapshot | 9035/9036, 8002/8003, 13535; protected request, target, consent path, actor/owner, request reference and optional replacement/reason. Archive UI state is relay-scoped and does not revoke access or transfer authorship. |
| CL | ChannelIdentity/Mutation, Labels, MembershipNotice | First t is type, later t are labels; uppercase P creator and separate creation timestamp; missing old tags mean unknown. Labels absent versus empty distinguish unchanged/clear. System/unlisted visibility and recipient-only notice reads are live relay rules. |
| AR | ArtifactRevision | 45010 complete-snapshot identity/home/type/op/title/root/prev and unknown annotation tags; content remains opaque. Create lacks prev; later revisions need it. CAS, idempotence, moves, read visibility, deletion and retention are not implemented by decoding. |
| CW | Cursor, ChannelRequest, ThreadRequest, ThreadSummary, Bounds | Composite timestamp+ID cursor, row/depth budgets, summary counts, nullable last reply, participant order, explicit exhaustion/next cursor, request binding and relay signer evidence. Missing bounds never prove exhaustion. |
| DV | VisibilitySnapshot | 30622 viewer d=p and hidden-channel h set; original duplicates/order retained in the envelope. This is owner-private relay projection, not membership removal. |
| WP | WorkspaceProfileCommand | 9033 icon value; absent/empty clears, validated http(s)/image data URL sets. Live relay admin/owner authorization is required; NIP-11 presentation remains public. |
| MP | Project, RepositoryMember | Kind 30621 source event retained byte-for-byte; project `d` and optional metadata retain presence; ordered `a` coordinates and optional relay hints preserve their original strings. Shape/bounds alone do not verify the signer, compare projections with original tags, validate repository coordinates, determine listing visibility, or grant repository/channel authority. |

## Boundaries and limits

The reference adapter uses local input bounds: 524288 original event JSON bytes,
1048576 SignedEvent protobuf bytes, 2048 tags, 64 strings per tag, 262144 content
UTF-8 bytes, and source-specific 65535/131072-byte JSON body limits. These are
consumer safety limits, not amendments imposing universal Nostr limits. IDs,
keys and signatures have exact-size annotations; newly required fields use
explicit presence. Source-specific constraints still require semantic checks.

Raw unknown JSON is retained instead of passing through google.protobuf.Struct,
which can round integers. Optional scalar values and oneofs preserve missing,
null and zero distinctions. Decimal numeric projections preserve value while
the original object bytes preserve lexical form. Opaque ciphertext, JSON and
JWS bytes must not be rewritten from a subset of typed fields.

Private/decrypted AE/AO/AM and AP config, FI/FA credentials and identity metadata
require private lifetime/storage policies; none belong in public telemetry or
logs. No schema Boolean may substitute for verification or membership. A caller
must verify source evidence and compare any derived fields before relying on
it, and must separately enforce actual authority, allowed unknown-field policy,
revision ordering, audience and privacy before a public/durable transition.

## Proof and remaining work

The fixtures include the exact complete event example from pinned NIP-OA and a
canonical Event protobuf encoded from it. Tests recompute its NIP-01 ID, preserve
original JSON and tags across protobuf/common.v1 round trips, and consume the
same binary fixture in compiled Coax. This does not test its BIP-340 signature.
Synthetic negative vectors cover duplicate/missing fields, malformed keys,
filter emptiness, null/zero, large counters, plaintext limits, token classes,
owner mismatch, artifact prev shape, channel metadata and profile isolation.

The selected descriptor profiles already import into Coax typed wire views,
presence/oneof accessors, repeated iterators and bounded caller-buffer codecs.
`coax/tests/proto_import.py` exercises these against native C ABI fixtures,
sanitizers and pinned protoc, including unknown retention and strict duplicate
policy. This establishes supported wire-codec behavior, not NIP conformance.

The reference projector only implements the subsets exercised by those tests.
Remaining work includes complete domain/unknown-field validation, signed
JSON/JWS proofs, encryption/decryption, actual transport parity, live
admission/read policy, bounded pagination and transactional storage. No decoded
owner claim, channel label, revision or command is authority by itself. HOST
grants and REACH freshness stay independent of relay membership and routing.

The [Coax Nostr plan](../../coax/nostr.md) and its
[task seed](../../apps/openagents-web/projects/coax/work/nostr.json) cover the
shared stack and later runtime adapters. Keep schema coverage, codec support,
cryptographic verification and product conformance as separate status fields.
The existing alpha adapter limits above remain unchanged; new consumer limits
need explicit profiles and boundary refusals, not an in-place schema rewrite.
