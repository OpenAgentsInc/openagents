# Coax Nostr: shared crypto, clients and authority

Plan, reconciled 2026-10-28 against remote main `1357c44a52`. The implementation
is incremental: the target is one reusable Coax stack for web, iOS, native tools
and Verse. This document is not a claim that the target already exists.

The executable work breakdown is
[`apps/openagents-web/projects/coax/work/nostr.json`](../apps/openagents-web/projects/coax/work/nostr.json).
Published task slugs and event IDs stay stable. `blocked_by` defines readiness;
`blocks` is its derived inverse. Milestone names group work, not a requirement to
finish every optional optimization before another group starts. The Coax language
roadmap remains the project's first initiative; this is its Nostr initiative.

## 1. Baseline and scope

### 1.1 Already shipped

| Evidence | What it establishes | What it does not establish |
| --- | --- | --- |
| `nips/manifest.json`, `nips/upstream-lock.json` | Pinned source inventory: 100 official, 20 Block, 36 OpenAgents NIPs | Runtime support for every specification |
| `proto/nostr/README.md`, descriptors, generated Coax codecs | 16 alpha schemas, bounded wire views, presence/oneofs/repeats, unknown retention and protoc interoperability | Signature verification, decryption, membership or executable authority |
| `coax/lib/proto/fingerprint.coax` | Checked SHA-256 for compiler fingerprints | A qualified streaming crypto API or approved signing path |
| `coax/web/project/{publish,model,snapshot,relay}.ts` | Real signed project events, NIP-42, verified build-time snapshots, task comments/plans/statuses | Coax crypto, independently authenticated workers or fenced claims |
| `coax/web/unlock/` | Working TypeScript NIP-44/59 encrypted-deck path | Completed Coax migration |
| `e2f8432ea2` | Compound assignments through indexed views emit valid C | General compiler correctness |
| `b9a5f216f0` | u32/u64 `value_barrier` and focused native/Wasm codegen tests | Constant-time or secure-erasure proof |
| `8f23a77f35` | Bounded Coax JSON parser/writer and captured-relay/JSONTestSuite tests | Duplicate-key/domain admission or safe-integer profile reconciliation |
| `df008f96a4`, `socket.coax`, `project-live.coax` | Host WebSocket byte pipe, subscriptions, frame counter and reconnect probe | Parsed live task folding, Coax event verification or AUTH readiness |
| `2ace224626` | Live relay parsing/folding, details and project-agnostic page updates | Local signature verification, complete catch-up or fenced claims |
| `9c556d7c05` | Pinned upstream crypto vectors and integrity/provenance checks | Algorithm conformance |
| `bc88057ec1` | Bounded Coax hex and canonical padded base64 with native oracle/sanitizer checks | Full Nostr event admission |
| `54ea8f5626` | Canonical Coax profiles, generated matrix and bounded preflight policy | Truth of untrusted measurements or completed crypto profiles |

The current execution also has streaming SHA-256, HMAC/HKDF, bounded codecs,
field/scalar arithmetic, exact-chain field inverse/root, resumable PBKDF2,
identified-buffer erasure and integer-profile JCS. Each slice retains its own
native/Wasm/oracle/mutation evidence; that is not a completed Nostr client or
approval for production secrets. The timing controls in
[`tools/coax-nostr/timing/`](../tools/coax-nostr/timing/README.md) record real
million-observation arm64, x86-64 and Chrome/Wasm runs, including deliberately
leaky branch/index fixtures. No-detection is scoped evidence, never proof.


The live fold now stages candidate tables and performs bounded history/known-root
catch-up before replacing the first-paint snapshot. `f4b21878c7` matched all five
tables of a verified public capture (156 issues/four workers) even with an
artificial37-event relay cap. Refusal, timeout, output exhaustion and ambiguous
stalled timestamp cohorts retain the previous view. This remains scoped relay
trust until local verification lands, not proof that an arbitrary relay omitted
nothing. The plural `/projects/<slug>` route and task details are shipped.

Further foundations include crypto bytes (`4fcef5f7c1`), bounded Bech32/NIP-19
(`c29005554f`), the explicit native vector runner (`1b269c7ae3`), streaming SHA-256
(`3189fb89db`) and the entropy contract (`7ba394d365`). These do not complete a
production crypto/signing profile. Entropy has Mac, iOS Simulator and actual
browser evidence; its Linux branch has not run in the recorded test environment.

The new structural JSON adapter/policy and its scope are documented in
[`docs/verification/coax-nostr-json-admission-2026-10-28.md`](../docs/verification/coax-nostr-json-admission-2026-10-28.md).
It preserves generic parser behavior and original bytes while refusing decoded
key ambiguity, unsafe selected integers and explicit consumer-budget violations.
The separate `nostr-json-parser-checks` closure now kills971/971 mutants
and matches a frozen decoder on42328 native sanitizer cases per mode, including
exact token/range/decode/error and bounded writer output. Its evidence is in
[`coax-json-decoder-checks-2026-10-28.md`](../docs/verification/coax-json-decoder-checks-2026-10-28.md).
This qualifies the explicit decoder check closure, not full event/JWS verification
or every target; no survivor exemptions or hidden parser replacements are used.

### 1.2 Supported profiles, not an unlimited promise

`nostr-support-profiles` defines a machine-readable support/limits matrix with
source pin, target, implementation state, consumer and acceptance evidence:

| Profile | Minimum scope | Boundary |
| --- | --- | --- |
| `core-read` | NIP-01 JSON/events/filters, BIP-340 verification, NIP-19, basic relay flow | No secrets or write authority |
| `core-write` | Signing, CSPRNG, NIP-42, NIP-44/59, signer interface | Production secrets require the review/cutover gate |
| `phone-signer` | Keychain custody, NIP-46 and NIP-98 website sign-in | Approval and background availability are explicit |
| `openagents-host` | Original HOST grants, REACH freshness, WORK/COORD/CAP/POL/CTX/RUN | Network membership never grants execution |
| `block-identity` | OA/AA/FI/FA, community/issuer/audience and revocation | Algorithm allowlist is deployment policy |
| `block-channel` | CL/DV/CW/AR/IA/WP and required GS/MP/PL/RS projections | Read/write authority and completeness remain live decisions |
| `agent-private` | AE/AP/AO/AM privacy, configuration and exact counters | No plaintext public snapshots or durable observer logs |
| `key-backup` (optional) | NIP-49 encrypted import/export | Separate memory/KDF/normalization budget |

Additional official NIPs (for example deletion, expiration, relay discovery,
private messaging and search) enter only with a named consumer and conformance
fixtures. Importing all NIP text does not advertise all NIPs. BIP-32/39 wallet
derivation, ECDSA and MuSig2 are separate wallet/Bitcoin scopes. We are building a
client, not replacing the existing relay. TLS stays in OS/browser transports.

## 2. Architecture and compatibility

`coax/lib/crypto/` knows nothing about Nostr. `coax/lib/nostr/` contains pure
bounded codecs, event handling and protocol state machines. Domain adapters load
selected root `proto/` profiles. Web, iOS, native tools and Verse reuse those
sources. Host languages perform effects: entropy, sockets/TLS, Keychain, storage,
clipboard, camera/QR acquisition and GPU/input bindings.

Every output uses caller-owned bounded storage and a status result. A view does
not own or extend its input's lifetime. No module retains a global secret. Native
C ABI adapters must state aliasing, capacity, lifetime and cancellation rules;
these are not established by Coax's limited local-borrow diagnostic.

Nostr signing preimages remain exact NIP-01 JSON, never protobuf bytes or JCS.
Protobuf is the internal interchange/projection contract. Retain original signed
JSON, ordered tags and exact content, compare projections to originals, and keep
unknown fields/presence. No transported `verified` Boolean creates authority.
JWS uses its original signing input; owner attestations retain their original
condition strings. Published proto snapshots and upstream NIP copies stay frozen.

The existing TypeScript libraries remain the working path and independent test
oracles until the cutover gate. New product computation is Coax; tests may use
pinned independent libraries/tooling. There is no Python dependency in ordinary
compiler/app generation. Reconciliation changes no runtime or published schema.

## 3. Modules and consumers

```text
coax/lib/crypto/
  bytes.coax              endian loads/stores, checked lengths, ct equality/select
  zeroize.coax            explicit secure-erasure contract and caller scratch
  sha256.coax             streaming/one-shot, BIP-340 tagged hashes
  hmac.coax, hkdf.coax     HMAC-SHA256 and HKDF-SHA256
  pbkdf2.coax             phrase-gate KDF only
  chacha20.coax           RFC 8439 block/stream, caller nonce/counter
  secp_field.coax         F_p, bounded 10x26 limbs
  secp_scalar.coax        Z_n, 8x32 limbs with proven carry schedule
  secp_group.coax         points, complete secret-path exceptional handling
  secp_mult.coax          public verify path; secret fixed/variable-base paths
  schnorr.coax            general-message BIP-340, Nostr 32-byte wrapper
  ecdh.coax               validated x-only peer, raw shared x
coax/lib/nostr/
  hex.coax, base64.coax, bech32.coax
  json.coax, event.coax, filter.coax
  nip44.coax, nip59.coax, nip42.coax, nip46.coax, nip98.coax
  jcs.coax, envelope.coax, keys.coax
  relay.coax, store.coax  connection and persistent event lifecycle decisions
  adapters/              proto/domain mappings and source-evidence checks
  host/                  HOST/REACH and coordination policy
  block/                 identity, channels, artifacts and private-agent policy
coax/web/host.js          generic effects, never Nostr policy or UI
apps/openagents-ios/     Keychain/network/camera effects; Coax state and decisions
verse/                   the same verified projections and authority path
tools/coax-nostr/       native keygen, verify, fetch, publish, seal and gate
```

Names above describe planned modules; they are not installed APIs. A bounded
profile can split files further without changing task identity. Native CLI test
keys and fixtures must be separate from production identities.

## 4. Arithmetic and compiler prerequisites

Use 10x26 field limbs in `u64`: widening products fit without `u128`. Prove
bounds for each actual accumulator and carry/fold schedule, including admitted
non-normalized magnitudes and reduction by p = 2^256 - 2^32 - 977. Record
normalization/magnitude preconditions per operation; checks reject violations.
A negative bound test must exceed the actual proven bound, not assume magnitude
9 necessarily overflows. Fixed-chain inversion/square root precedes optional
public-data variable-time optimization.

Scalar 8x32 products individually fit `u64`; sums of several do not necessarily
fit. Specify split products/carries and prove bounds before implementation.
Group operations must cover infinity, P=Q, P=-Q, invalid lifts/peers, and secret
paths without exceptional secret branches. Verification may use explicitly named
`_vartime` operations on public data only.

Checked and optimized builds must agree on defined crypto arithmetic. The old
`0 - (bit & 1)` mask underflows checked unsigned arithmetic: use an explicitly
specified wrapping helper or an equivalent checked-safe construction. Widen-and-
truncate/mask helpers remain valid until intrinsics land. No secret index or
unchecked-access escape hatch is introduced.

| Prerequisite | Reconciled status | Gate |
| --- | --- | --- |
| Checked integers, fixed arrays/views, `#load`, `#run` | Implemented; actual table budgets still need measurement | Existing compiler plus emitted-target tests |
| Compound assignment through a view | Fixed in `e2f8432ea2` | Preserve focused regression |
| `value_barrier` | Shipped in `b9a5f216f0` | Scoped code-generation tests, not a constant-time guarantee |
| Secure erasure | Explicit identified-buffer/copy contract shipped in `487c01091f` | Optimized stores/copies and failure-path tests; hidden copies and consumer adoption remain separate |
| Wrapping ops/rotates | Implemented in Coax 0.15.0 | Matching u32/u64 operands; checked/optimized/compile-time/native/Wasm evidence |
| `mul_wide` | Shipped in Coax 0.16.0 as a named hi/lo record | Exact native/fallback/Wasm/compile-time tests; not required for 10x26 |
| Debug magnitude layout | Conditional fields planned | Explicit debug representation is allowed; no false `#if` claim |
| Secret/variable-time enforcement | Missing | `crypto-secret-lint`; `#secret` is not accepted syntax today |

The first secret-source checker is deliberately scalar/straight-line only.
Completing it does not admit the real array/aggregate/loop crypto paths.
`crypto-secret-aggregate-closure` separately gates secret multiplication and
generated-code audit: it must handle actual typed field/scalar/group closures,
reject secret controls/indices and unknown effects, and retain source identities.
Boundary classifications are inputs to check, never an authored substitute for
dataflow analysis. Compiler-inserted guards and optimized/JIT behavior still
require their separate target evidence. Authored secret-derived assertions are
also source-hashed obligations: a completed supported-flow analysis with such
obligations cannot report full secret-path admission. Their discharge belongs
to the generated-code/range/security review gates.

The presentation phrase-gate format review is in
[`docs/protocol/2026-10-28-phrase-gate-v2.md`](../docs/protocol/2026-10-28-phrase-gate-v2.md),
with its [non-author review and fixture evidence](../docs/verification/coax-phrase-gate-format-2026-10-28.md).
It leaves current AES-GCM behavior and entry bytes unchanged. Test-only v2
implementation is separate from production approval, dual-reader rollout and
event/recipient authority; the legacy cheap-check oracle and reused-phrase
limits are explicit.

## 5. Host effects and key custody

The socket pipe already exists. Extend its bounds, errors, queue overflow,
connection generations and cancellation only through explicit contracts. Browser
WebSockets do not expose arbitrary handshake headers; the FI/HOST transport
profile must select supported carriage rather than assume header injection.

Entropy adapters use OS CSPRNGs / `crypto.getRandomValues` (chunked at 65,536
bytes), fail closed when unavailable, and never substitute clocks or PRNGs.
Native/iOS adapters share the failure contract; pure algorithms receive explicit
entropy/auxiliary bytes. Tests inject deterministic bytes only through test APIs.

Storage adapters move opaque bytes with namespace/version, quota, failure,
crash/reload, generation and deletion semantics. Local IndexedDB failure is a
reported state. Default identity custody is the phone or an external signer;
plaintext `nsec` persistence in IndexedDB is not the default. Browser local keys
are explicit ephemeral slots; durable encrypted import/export waits for NIP-49.
The existing unlock-key store is a legacy migration concern, not an endorsement.

A signer abstraction supports phone/NIP-46, NIP-07 and a local test/ephemeral
slot. Verify and correlate returned events, enforce requested identity/kind/body,
show approvals, cancel requests and reject late/duplicate responses. Keychain
records are device-only with explicit user-presence policy. Software secp256k1
signing briefly exposes key bytes in app memory; Keychain protection is not a
promise of hardware-isolated signing. An iOS app may be
suspended: pending requests time out or await foreground approval, never assume
an always-running phone signer. Forget/logout revokes sessions and wipes owned
scratch; storage deletion and memory erasure are separate outcomes.

## 6. Secret handling and security evidence

No branch, loop bound, address/table index or early comparison exit depends on a
secret. Scalar multiplication scans tables with masks. Check intermediate bounds
and validate scalar 1..n-1 and peer points. BIP-340 includes parity normalization,
auxiliary randomness, zero-nonce refusal and self-verification before a signature
is released. Error paths do not release partial signatures/plaintext.

A value barrier prevents some optimizer reasoning; it does not prove constant
time, preserve all erasure stores, or erase hidden array/struct/ABI copies. A
foreign identity call is not a universal fallback under whole-program analysis.
`crypto-secure-zero` defines observable erasure at the host/compiler boundary,
allowed secret copies, caller scratch and failure cleanup. Inspect real optimized
objects, including builds where no test reads the wiped buffer afterward.

`crypto-secret-lint` must state its supported source/taint scope and fail closed
for unsupported constructs in marked secret entry points. Names ending
`_vartime` alone enforce nothing. Do not claim a language effect/taint system
exists before its implementation and negative tests.

`crypto-secret-codegen` records target, compiler/toolchain version, flags,
optimization/LTO mode and generated artifact hashes. Inspect arm64 and x86-64
branches/addresses/erasure; inspect and execute wasm32 across Chrome, Safari and
Firefox tiers where available. Browser/JIT timing has no general guarantee.
Script injection/extensions can read page memory; CSP, no third-party scripts
and external key custody remain separate protections. Physical/power/EM attacks
are outside this profile. Do not describe arbitrary future CPUs as constant time.

Dudect-style fixed/random secret classes, Welch t statistics and at least 10^6
samples are leakage probes: |t| < 4.5 means no leak detected in that run, not a
proof. Record methodology/noise and prove the harness detects seeded leaks.
Source checks, mutation scores, fixed-point bootstrap and timings do not prove
cryptographic security or compiler soundness.

`crypto-independent-review` requires independent arithmetic/protocol/compiler-
boundary review with findings resolved and evidence pinned. `nostr-production-
cutover` is the gate for production keys and retirement, not a prerequisite to
writing/testing algorithms with throwaway keys. Keep the working path until that
gate passes. No task may resolve it by recording its own unreviewed approval.

## 7. JSON, events, encodings and limits

`nostr-support-profiles` records per-consumer original-frame, decoded-content,
tag/cardinality, depth, output, pending verification and event-store budgets.
The old proposal used 1 MiB JSON, 64 KiB strings, 2,000 tags x16; current proto
reference limits include 524,288 original JSON bytes, 262,144 content bytes,
2,048 tags x64 and 1 MiB protobuf. These are distinct consumer limits, not
universal Nostr rules. Boundary adapters refuse cleanly rather than truncate or
advertise wider support. The existing socket inbox is 262,144 bytes.

The shipped bounded pull parser/writer covers UTF-8, escaping and surrogate
pairs. `nostr-json-profile-hardening` adds consumer rules and tests duplicate
object keys, nesting, integer overflow and output exhaustion. Choose an explicit
fail-closed duplicate-key policy. Safe integer projection is bounded by
2^53-1; retain numeric text/original bytes where another contract needs more.
The shipped `json_int` accepts +/-2^53; consumer safe-integer admission must
narrow this explicitly rather than assume the existing parser already does so.
Full event validation, including unknown-field policy, precedes durable use.

`nostr-event` handles parse, canonical NIP-01 serialization, ID recomputation
and BIP-340 verification. `nostr-event-sign` separately builds signed events.
That separation keeps read-only verification independent of signing/keys/AUTH.
Test every escape class and non-ASCII against independent implementations; do
not use JCS as the Nostr event preimage or silently rewrite signed originals.
Freshness/future limits belong to the consumer profile, not signature validity.

BIP-340 accepts byte-view messages, including lengths 0, 1, 17 and 100 in the
19 pinned official vectors; a Nostr wrapper requires exactly 32-byte event IDs.
Strict lowercase exact-size ID/pubkey/signature admission, invalid curve points,
r>=p, s>=n and altered event fields get independent negative fixtures.

NIP-19 uses BIP-173 bech32, not bech32m, with NIP-19 TLV length rules. Unknown
TLVs follow the pinned source policy. Hex/base64 decoders reject invalid padding,
noncanonical encodings where required, and insufficient buffers.

JCS contract digests sort UTF-16 code units. The initial envelope profile is an
explicit integer-only RFC 8785 subset, with safe-number bounds; reject floats
until a complete interoperable number renderer exists. Digest the specified
original/canonical representation, not a lossy proto reconstruction. The initial
artifact-envelope consumer caps raw/canonical bodies at512KiB, each string
at64KiB and depth16, while the published outer wire ceiling stays1MiB. These
axes remain distinct; unsupported larger bodies refuse before emission.

## 8. Relay and persistent state

Read-only live connectivity goes first. `project-page-live` parses/folds bounded
frames and may show explicitly relay-trusted provisional rows on the configured
project relay. Keep them distinct from verified snapshots; no authoritative
claim, write, grant or production-cutover decision consumes provisional data.
The production verified profile depends on `project-page-verify`: recompute ID,
verify signature, enforce author/project scope, then fold/store. Apply a bounded
verification queue and at most 4 ms/frame; forged events are refused and counted.

The generic relay machine covers OPEN/CONNECTING/CLOSED/BACKOFF, AUTH states,
EVENT/REQ/CLOSE/AUTH, EVENT/OK/EOSE/CLOSED/NOTICE/AUTH replies, acknowledgement
timeouts, cancellation, capped jittered backoff and connection generations. Each
subscription has bounded IDs/filters/events; frames from old/unknown connections
or subscriptions are ignored. NIP-42 proof is bound to the current challenge,
relay and connection; reauthenticate after reconnect. AUTH-required CLOSED
requeues once after successful AUTH. Do not rely on one relay's poison-before-
AUTH behavior as a universal protocol rule.

The store handles verified immutable originals plus indexes, duplicates,
replaceable/addressable-event ordering and same-time tie rules, authorized
deletions/tombstones, expiration and schema migration. Never resurrect deleted
records on reconnect. EOSE does not prove completeness; short pages, tied
timestamps, missing dependencies, paginated catch-up and relay disagreements are
explicit states. A count/connection chip must not claim a complete synchronized
project. Multi-relay merge/outbox rules distinguish published, acknowledged,
refused and unknown outcomes and survive offline restart without duplicate acts.

## 9. Correctness, oracles and target evidence

Pin source URL, revision/license, retrieval date and SHA-256 for every vector
file. Keep upstream bytes; label derived adapters and their input hashes.

| Module | Required evidence |
| --- | --- |
| SHA-256 | NIST short/long subset, padding boundaries and split streaming |
| HMAC/HKDF/PBKDF2 | RFC 4231, 5869, 7914 vectors and output bounds |
| ChaCha20 | RFC 8439 vectors, counters/exhaustion, alias/failure policy |
| Field/scalar/group | independent big-integer oracle or pinned reviewed white-box libsecp harness; edge/carry/magnitude/exception cases |
| BIP-340 | All 19 official rows, valid/invalid and variable message lengths; sign self-verification |
| ECDH | validated peers, raw-x oracle callback, NIP-44 conversation keys and labelled adapted Wycheproof inputs |
| NIP-44 | every valid/invalid source vector, MAC-before-plaintext, padding/version/alias/boundary cases |
| NIP-59 | independent implementations in both directions, wrap/seal verification and rumor identity |
| Events/JSON/NIP-19/JCS | independent preimages, malformed JSON/UTF-8, TLV/escaping/UTF-16 ordering and capacity cases |
| Domain protocols | revoked/expired/replayed/out-of-order evidence, missing authority, privacy and completeness refusals |

The public libsecp API does not expose arbitrary field/scalar internals: pick
and document the actual oracle interface. Its default ECDH API hashes the
compressed shared point; NIP-44 needs raw x through a reviewed callback, not the
default digest. No comparison against our own generated output is an independent
oracle. Downloaded source is inspected and built only in an explicitly reviewed,
pinned harness; never execute unreviewed install/build scripts on this Mac.

Small `#test`/`#forall` checks and zero-survivor mutation gates accompany modules;
authority/policy files use `#authority`. Native emitted C runs under ASan/UBSan;
independent differential fuzzing pins seeds and retains minimized regressions.
A seeded algorithm, serialization and oracle fault must be caught. Run actual
emitted vectors in checked/optimized C, wasm32 browser engines, iOS simulator
and physical devices when available. Device evidence is an owner step, not a
simulator claim. Hash inputs/artifacts and rerun only affected evidence; unrelated
remote commits or rebases do not justify rerunning the compiler/app suites.

Performance figures are provisional budgets, not facts: SHA-256 300 MB/s native
/150 MB/s Wasm, ChaCha20 500/250 MB/s, verify 100/600 us, sign 60/300 us,
ECDH 80/400 us and cached 1 KiB NIP-44 decrypt 10/40 us. Record hardware,
profiles, allocation/stack/table costs, distributions and complete consumer loop.
GLV, batch verification, 5x52 limbs and larger tables wait for measured need;
they never block first interoperable verification.

## 10. Encryption, migration and retirement

NIP-44 v2 uses raw-x ECDH, HKDF, ChaCha20 and HMAC over nonce||ciphertext.
Validate the outer event before decrypting and authenticate before releasing
plaintext. The pinned NIP supports the six-byte extended length prefix at
65,536 bytes and beyond. Test 65,535/65,536/65,537; a bounded profile may refuse
larger messages explicitly, not misdecode the extended form. Test unsupported
versions, zero/oversized lengths, malformed padding, wrong MAC and nonce reuse
policy. NIP-44 is not a promise of forward secrecy or post-compromise security.

NIP-59 checks the gift-wrap signature, decrypts, checks seal signature and rumor
identity, and generates fresh wrap keys/timestamps/entropy. Encrypted payloads
and private plaintext never enter public task snapshots, logs or observer stores.

The existing phrase gate uses PBKDF2 + AES-GCM. Keep that path until
`unlock-gate-format` specifies and independently reviews a versioned replacement:
KDF parameters/limits, salt, independent keys/domain separation, nonce generation,
exact MAC input/AAD/framing, authenticated version/length, error behavior and
rollback. Do not assert that a new ChaCha20/HMAC composition has "the same
security" or is "NIP-44 minus ECDH" without that definition and review.

Use immutable format/version fixtures, old/new readers for a documented window,
staged regeneration and rollback tests. `unlock-gate-coax` implements only the
reviewed format. PBKDF2 workload remains bounded and measured; it must not freeze
the render loop. No AES implementation is needed for core Nostr; keeping an
existing host adapter temporarily is not a protocol regression.

`nostr-production-cutover` pins independent review, target vectors, real-consumer
staging tests, rollout flags/compatibility and rollback. Only then retire the
working crypto path. `invariant-coax-only` changes INVARIANTS and removes browser
TypeScript dependencies after all consumers, including project tools, have actual
replacements. A check must detect hidden imports as well as extra script tags;
merely drawing everything in Coax is not proof that crypto is Coax.

## 11. Dependency order and exit consumers

Stable milestone slugs retain their identity; their old numeric prefixes are
historical identifiers. Read `blocked_by`, not a milestone number, for readiness.

| Delivery order | Work | Exit evidence |
| --- | --- | --- |
| Foundation (parallel) | Profiles/limits, pins/runners/oracles; existing socket plus bounded JSON and storage; compiler prerequisites only where required | Queue DAG/negative gates, real frame probe, independent vectors |
| Read-only verification | Hashes, field/scalar/group, BIP-340 verify, event parse/hash/verify | One real event and forged refusal, no signing dependency |
| Verified live project | Coax task fold then local verification, bounded store and replay | Live changes without release; forged/out-of-scope rows never enter verified state |
| Signing/encryption | Entropy, erasure, secret-path/codegen evidence, sign, ECDH, NIP-44/59 | Cross-implementation outputs and failure cleanup using throwaway keys |
| Durable clients | Event lifecycle, offline outbox, multi-relay catch-up; source-preserving proto/domain adapters | Restart/reorder/partition/deletion and privacy tests |
| Phone sign-in (M9) | iOS shared vectors, custody, NIP-46, QR, NIP-98 sessions and revocation | Local + staging browser/phone sign-in and explicit device step |
| Host and work | HOST grants, REACH, worker identities, atomic fenced claims and task journal | Real Linux Codex task, progress/result/PR, refused revoked/replayed grant |
| Identity/channels/private data | Block OA/AA/FI/FA, CL/DV/CW/AR/IA/WP, AE/AP/AO/AM and missing projections | Cross-client authority, CAS, privacy and pagination tests |
| Production migration | Independent review and cutover; native project/unlock tooling, format migration and rollback | Same real consumers, then removal of old paths |
| Optional breadth | NIP-49, consumer-backed additional NIPs; GLV/batch/native optimization | Separate advertised profiles and measured need |

M9 already exists in the queue and is not a newly discovered feature. The
previous M0-M8-only table and unlock-first prose are superseded by this order.
The early live fold may be developed before verification, but that does not
waive verification for authoritative use or require signing to read public data.

## 12. Phone signer and website sessions

Implement NIP-46 client and bunker sides with distinct client, remote-signer and
user keys; call `get_public_key` after connect. One-use connect secrets,
authenticated response correlation, granted kinds/methods, bounded requests,
replay/expiry, disconnect/logout, relay switching, approval and suspension are
part of the contract. Untrusted site metadata is display-only. Unsupported
legacy NIP-04 methods return an explicit error; this profile does not add NIP-04.

NIP-98 server validation binds exact URL, method and payload, time window and an
unused event ID. Replay reservation must be atomic across workers/restarts for
the acceptance window. Sessions need fixation protection, CSRF policy, scoped
roles, rotation, expiry, sign-out/revocation and bounded cleanup; a signed login
never implicitly grants project administration. Cookies contain no secrets.

Run simulator/browser fixtures with throwaway keys against a local relay, then
staging. Production keys remain behind the cutover gate. Device-only Keychain and
foreground approval behavior require owner evidence recorded in NEEDS_OWNER.md
when those tasks are implemented.

## 13. Runtime authority and OpenAgents integration

`nostr-proto-adapters` maps selected alpha profiles to real Coax domain values,
preserves original signed/framed inputs and rejects projection mismatches.
Expand missing official/Block definitions in root `proto/` only alongside their
consumers, pin/source review, compatibility and protoc gates. Codecs do not turn
an owner string, JWS claim or channel label into authorization.

HOST verifies original invitation/grant signatures, issuer/audience, host and
operation scope, expiry, redemption, epochs/revocation and request replay before
any effect. REACH validates route generations/epochs, identity, rollback
freshness and authenticated direct-channel binding. NIP-42 proves key possession
to a relay; FI community membership, tailnet routing and host execution are
separate decisions. Refuse closed when evidence/configuration is unavailable.

COORD claims use authenticated worker keys and owner admission, atomic revision
checks, all-or-none resources, fencing tokens, lease renewal/release and expiry
reconciliation. The current project-key `["claim", NAME]` comments remain
informational and readable as history; they are not migrated into forged worker
signatures or accepted as live fenced grants. WORK/CAP/POL/CTX bound what may be
run; RUN/SESS progress/result references keep evidence and errors stable.

`host-codex-e2e` exercises phone/app -> admitted HOST/REACH connection -> real
Linux Codex task -> progress/result/PR -> fenced completion. Duplicate delivery,
revocation during a run, disconnected/unknown result and refusal of unauthorized
commands are required failures, not just a happy-path connectivity probe.

## 14. Block identity, channels and private-agent state

OA attestations preserve exact signed conditions and check current owner roles;
AA key possession and membership follow the configured community policy. FI
requires bounded compact JWS parsing plus exact issuer/audience/community/token-
class binding, time/TTL checks, algorithm allowlist, key selection, authenticated
JWKS retrieval, rotation, stale-cache and revocation behavior. Select supported
asymmetric algorithms before implementing P-256/Ed25519/RSA; secp256k1/Schnorr
alone does not implement FI. Deny `none`, symmetric/confused algorithms and
untrusted network destinations. FA origin/session/code exchange is a distinct
credential-bearing flow, not public profile metadata.

The selected FI candidate profile is now
[`fi-rs256-policy-v1`](../docs/protocol/2026-10-28-fi-rs256-policy-v1.md):
RS256-only public key shape, exact community/issuer/class/time candidates,
complete pinned IANA address refusal, trusted fetch-observation policy,
bounded keys and issuer/source/policy/installed-generation-bound snapshots.
Its Coax authority closure passes 46 groups and 782/782 mutants. Native and
actual Wasm candidate fixtures pass; none of those predicates verifies a JWS,
authenticates a fetch or grants identity authority. Actual RSA/original-JWS/JWKS
parsing, authenticated provider effects, full AssertionPolicyId and fresh
NIP-42 pairing remain explicit `fi-jws-verifier` gates. No production FI support
is advertised from the current typed candidate implementation.


CL/DV/WP implement channel membership/visibility and owner controls. CW uses
composite timestamp+ID cursors, bounded rows/depth and explicit exhaustion; a
missing page never proves an empty channel. AR revisions implement CAS,
idempotence/history/move/deletion; IA archive consent is not revocation.
GS/MP/PL/RS projections/behaviors enter named profiles with actual consumers;
ER/PMA or other reserved proposals remain unadvertised until specified.

AE memory/tombstones, AP persona/config resolution, AO ephemeral observation and
AM exact usage/pricing identity each retain their private lifetime and logging
rules. AO is never persisted as ordinary history. A summary or converted proto
must not leak decrypted bodies/credentials into public snapshots or telemetry.

## 15. Optional key backup and broader compatibility

NIP-49 requires scrypt (including its Salsa20/8 machinery), HChaCha20/XChaCha20,
Poly1305, Unicode NFKC normalization, authenticated framing, bounded KDF memory
and cleanup. Untrusted KDF parameters cannot request arbitrary device memory.
Native/iOS/web adapters must agree on normalization and display owner approval
for export/import. This remains optional and is not replaced by a new homemade
key-wrap format. Browser plaintext-key persistence is not a shortcut around it.

`nostr-conformance-release` records profile/target support and negative evidence,
including supported optional official NIPs, signer interoperability, storage
migration and privacy limits. It rejects advertised support whose runtime gate
is missing. No promise to implement every NIP or every JWT algorithm is implied.

## 16. Ownership, queue maintenance and scoped checks

The [handoff](nostr-handoff.md) assigns this entire Nostr initiative, its plan,
work seed and signed queue order to this Codex chat. R0-R12 remains a separate
initiative with its existing worker. Use maximum safe parallelism on independent
tasks, signed claims/plans/notes and reviewed completion evidence. The live-page
worker finishes its in-flight task; preserve its work and reconcile the final
commit without claiming Coax cryptographic verification.

The handoff also records the E0409 `ct_view` regression masked by E0383. Track
it as `coax-ct-view-diagnostic`: preserve the intended diagnostic coverage rather
than changing its expected error. No compiler implementation is changed by this
plan update.

Preserve all published slugs, root issue IDs, comments, worker plans and resolved
statuses. Reconcile the shipped compound-assignment, socket, value-barrier and JSON
tasks with their commits; do not resolve broader live/crypto tasks from partial plumbing. Rank
changes use the publisher's signed order record, not manually assigned numbers.
New tasks require concrete files, acceptance evidence and an acyclic dependency
closure. Keep the page's bounded item capacity above the combined initiatives.

For plan/seed-only updates run `bun test coax/web/project/nostr-plan.test.ts`,
existing model/rank/publisher tests and a dry run against the real relay. Publish
idempotent revisions/new tasks, read back and verify, then regenerate the project
snapshot and release through staging/smoke if the snapshot page needs refreshing.
Do not rerun compiler/bootstrap/iOS suites for Markdown or unrelated rebases.
Changes to the page capacity require its focused Coax checks.
