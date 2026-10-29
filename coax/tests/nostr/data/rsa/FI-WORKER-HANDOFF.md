# FI verifier work in progress

Task #bd06 remains claimed by `codex-nostr-codecs` and open. The partial local
verification slice is being committed; the disabled transport prototype and
its fixtures remain uncommitted in `/Users/christopherdavid/.codex/worktrees/coax-nostr-fi-jws/openagents`.
The base was `d5fc7e58328986e6e16a5b62609fc181d5c439b7`. Preserve all
untracked files, including inherited NIST and JOSE corpora. No authority has
been wired and no task acceptance or release success is implied.

- `rsa_verify.coax`: bounded public e=65537 arithmetic and exact SHA-256
  PKCS#1 v1.5 verification. `CURRENT-EVIDENCE.md` pins source, compiler,
  vectors, generated artifacts, default-budget tests and zero-survivor gate.
  Independent security review is still required.
- `fi-base64url.coax`: canonical unpadded base64url, with a 512-byte
  base64urlUInt helper. The ordinary decoder requires disjoint source and
  output storage from its caller. It does not establish trusted origin.
- `fi-compact.coax`: strict compact framing and original protected.payload
  span, without JSON claim admission or signature verification.
- `fi-protected.coax`: closed `alg`/`typ`/`kid` protected-header projection;
  unknown, duplicate and header-selected key sources refuse. It does not
  authenticate a header or select a JWKS snapshot. Its `typ` values 1, 2, 3
  mean `nip-fi+jwt`, `nip-fi-agent+jwt`, `at+jwt` respectively; bridge them
  explicitly to the existing `FiHeader` strings, never to caller DTO flags.
- `fi-jwks-parse.coax`: strict JWKS projection with public RSA integer checks,
  unique kid, optional exactly `["verify"]`, and whole-snapshot refusal.
  It passes 29 checks, 1395/1395 mutants and 392 native/Wasm oracle cases in
  each mode; it is not connected to a trusted fetch/provider. The parsed key
  stores no exponent bytes: successful parsing establishes exponent 65537; a
  bridge to `FiRsaKey` must supply local `[1,0,1]` bytes, preserve the
  `has_key_ops` flag, and provide `["verify"]` only when present.
- `fi-jws-signature.coax`: composes compact framing, protected header and RSA
  against the exact original signing span. It is signature evidence only, not
  claims, possession or identity authority. It caps decoded protected headers
  at 2048 bytes and signed input at the SHA-256 limit; bind these behavioral
  bounds into the eventual policy identity. It passed 85/85 checks and the
  full 2364/2364 default-budget mutation gate with no exemptions.

The semantic claim parser and admission chain are not implemented. In
particular, no transported DTO, `FiFetch` boolean, parsed key, or match on
`kid` is an authority. Do not accept NIP-42 events without actual public event
signature verification, exact connection challenge and relay binding, a
matching key, and freshness; wait for the public NIP-01 core and independently
test this possession gate.

A separate transport worker should implement and test actual bounded async
resolution, classify **every** DNS answer using the existing FI address policy,
connect only the pinned address, inspect and compare the actual peer, verify
TLS name and certificate for HTTPS, disable proxy, redirect and fallback, and
apply one whole-operation deadline and a streamed 524288-byte cap. Preserve
issuer, URI and locally compiled policy identity in authenticated provider
state; atomic install and generation changes must drive real dependent-session
revalidation, including add/remove, stale, refresh failure and error paths.
Browser contexts lacking an authenticated broker deny unavailable. Synthetic
`FiFetch` facts are fixture evidence only and must never be promoted into an
actual adapter claim.

A separate policy worker should build a versioned canonical identity covering
all selected configuration and the actual trusted verifier/adapter behavior;
key rotation changes snapshot generation rather than the policy digest. The
selected NIP-FI contract and accepted limits are in
`docs/protocol/2026-10-28-fi-rs256-policy-v1.md`. Implement real original-byte
JWS, claims, JWKS and NIP-42 integration before any FI authorization, and
cover class/audience/time/key-confusion and all unavailable/rejected paths.
Never mutate waiver files or mark the queue item done without every acceptance
check, target evidence, independent review and verified pushed commit.

## Successor progress: signature-only composition

`fi-jws-jwks.coax` locally parses the complete JWKS, extracts the compact
protected header and exact kid, then checks the original signing bytes against
the selected modulus. It never authenticates or installs the supplied JWKS and
returns signature evidence only. `coax test` now passes 92/92 checks. The new
`fi_jws_jwks.py` harness passed 13 fixture cases per checked/release native and
Wasm mode and native ASan/UBSan, with OpenSSL verifying the original signature.
`fi-jws-jwks.coax` SHA-256 is
`c24889248b2f81dcf4090939015b16baf2718789d4d7169478001bdfae5cee16`;
the Python harness is
`208a171f96ff60801d45701f0f5f5900a2a21671ec3d4e04e4660ce744d54cd5`.
The first full mutation killed 2607/2616; nine survivors were not waived. Tests were added; the default-budget full rerun killed 2616/2616 with no waivers. `../fi-jws/COMPOSITION-REVIEW.md` records the
independent partial source review, exact hashes and limits.

The configured access-token marker can equal `iss`: the future claim parser
must project the same decoded member into both roles and select the exact
issuer's policy before dependencies. NIP-01 public event verification is not
available here, so fresh NIP-42 possession cannot be integrated safely yet.
Real authenticated DNS/TLS transport, provider installation and session
revalidation, full policy identity and semantic claim parsing remain open.

### Integration constraints for the next worker

- Do not adopt `getaddrinfo` or a curl completion flag as proof of all-answer,
  peer or certificate checks. Specify what DNS results are returned (including
  unsupported families and over-limit sets), bound asynchronous resolver
  ownership after deadline/cancellation, and classify each returned address
  through the Coax policy. Connect to only the selected address, compare the
  live socket peer before request data, and authenticate the original DNS name
  and certificate during TLS. Disable proxy, redirect, connection reuse and
  alternate route behavior. Public HTTPS and mesh are different trust models;
  mesh HTTP is not authenticated merely because its IP is private.
- Exercise real effects with controlled DNS and TLS endpoints, including mixed
  answer sets, rebinding, peer mismatch, bad names/certificates, redirect,
  proxy environment, stalled resolver/connect/body, exact cap and cap+1.
  Fake fixture observations alone cannot establish these behaviors. A broker
  must itself be authenticated and bound to issuer, URI and local policy ID.
- Claim projection must accept the valid `marker_name="iss"` overlap when the
  issuer URI is in one marker set, and use the same decoded value for both
  fields. Include escaped duplicate names, wrong type, absent and distinct
  issuer/marker cases. The issuer is untrusted until signature verification;
  selection and community allowlisting must occur before fetching a key.
- The pending public `nostr-event` task #7cdf is independently claimed. Do not
  copy its uncommitted source into this worktree or treat an event DTO's flags
  as proof: integrate its landed event signature verifier, then bind kind,
  challenge, relay, Nostr key and freshness to the current connection.

## Successor transport experiment: disabled by default

`coax/lib/nostr/platform/fi_fetch.c/h` and `fi_dns_worker.c` are a partial
public-HTTPS experiment, not an authenticated provider. An independent review
found that the parent `waitpid` can race an uncontrolled host-wide child reaper
and reap an unrelated reused PID. The adapter therefore refuses all starts in
normal builds; `FI_FETCH_TEST_UNSAFE_REAPER=1` is for isolated testing only and
must not be used as an integration or production escape hatch. No transport
acceptance is claimed. `../fi-jws/TRANSPORT-EVIDENCE.md` records test outcomes,
source hashes and scope, the host-reaper blocker, and the unimplemented XPC
service option. Provider state, issuer/source/policy binding, rotation and
session revalidation remain open. Both checked/release Coax-policy test builds
passed the native prototype suite; an external certificate connectivity check
timed out on one run and passed on retry. Real local controlled endpoint,
app/device and target coverage is still required.

Two temporary, ad-hoc-signed macOS XPC probes now pass locally. `fi_xpc_probe.py`
checks the service CDHash, wrong-hash refusal, host SIGCHLD auto-reap, and
request timeout. It also observed that cancelling a request did not terminate
a stalled service while the client stayed alive. `fi_xpc_fetch_probe.py` wraps
the test-only transport in a temporary service, confirms an actual stalled
resolver child starts and is reaped at the operation deadline, and fetches
real HTTPS again. Neither probe supplies a shipped broker, client authorization,
production signing identity, or provider binding, and neither claims iOS
support. See `../fi-jws/TRANSPORT-EVIDENCE.md` for exact hashes and scope.

## Successor claims candidates

`fi-claims-parse.coax` projects bounded untrusted claims with decoded
top-level duplicate detection, exact field presence and marker overlap.
`fi-claims-select.coax` resolves Host/community, performs issuer-only
projection, then checks the exact issuer allowlist before parsing the selected
class and marker. Dedicated assertions ignore unverified `client_id`; access
assertions validate it. `fi-claims-candidate.coax` adapts the owned result to temporary typed
policy views and checks class, audience and time. None verifies signature,
provider, possession or grants identity. Caller inputs/configuration must stay
immutable and valid through both parses and any later use of selected indices.
Ignored unknown nested values are syntax-skipped and must never later become
verified claims. `../fi-jws/CLAIMS-EVIDENCE.md` pins all test/review hashes,
new parser size limits and the completed claims mutation gates.

`fi-local-jws-candidate.coax` now composes the semantic candidate with the
original compact protected header/payload and locally supplied JWKS signature
check. Its earlier reviewed revision had 254 Coax checks and 33 independently OpenSSL-signed/tampered
native/Wasm/ASan cases per checked/release mode. Guard pages show semantic
refusals for unknown Host, unauthorized issuer and expired claims do not read
the supplied JWKS. It is not authenticated key-source evidence or FI
identity/possession authority. `../fi-jws/LOCAL-CANDIDATE-EVIDENCE.md` records
exact hashes, fixture provenance, immutable input requirement and independent
source review. The current class-aware parser mutation gate passed 1683/1683.
The first local signature closure gate failed; the rerun passed 3397/3397.

Current class-aware selector mutation passed 1698/1698 with no waivers.
The semantic candidate gate also passed 1729/1729 without waivers.

`fi-policy-encoding.coax` now emits canonical candidate bytes for selected
community/issuer configuration, including bytewise-sorted sets. It is not a
full AssertionPolicyId: compiled verifier/adapter behavior and provider/build
binding are not supplied. The independent max configuration needed 41143 bytes
and prompted a 65536-byte capacity; 22 checked/release native/Wasm/sanitizer
cases pass and the full mutation rerun killed 604/604 with no waivers.
`../fi-jws/POLICY-ENCODING-EVIDENCE.md` records hashes, initial mutation
failures and the independent source review.

The first complete local-JWS closure mutation run killed 3393/3397; four
mutations survived and the gate failed. Test-only additions for a signed
one-byte kid and malformed/capacity boundaries bring Coax checks to 256/256;
the full default-budget mutation rerun killed 3397/3397 without waivers. The source hash, public fixture hashes,
and limits are recorded in `../fi-jws/LOCAL-CANDIDATE-EVIDENCE.md`.

After the partial local-verification commit, an unconnected DNS observation
prerequisite and macOS DNS-SD test fixture were added. The Coax state has no
complete/success state and cannot authorize; it rejects unsafe or overflow
callbacks, errors/removals and events after cancellation. The macOS probe
used real localhost DNS-SD callbacks and same-queue deallocation under
SIGCHLD auto-reaping, without a child process. This is finite local evidence,
not an exhaustive answer-set or bounded platform lifecycle guarantee.
Apple's MoreComing flag does not mean the answer set is complete. The source,
independent trace oracle, review, exact hashes and open requirements are in
`../fi-jws/TRANSPORT-EVIDENCE.md`. The previously disabled unsafe prototype
remains uncommitted, and all provider and FI authority remain closed.

A separate test-only macOS scratch XPC reaper probe exercises a host-wide
`waitpid(-1)` reaper during a request for a stalled resolver; a resolver
PID marker precedes the overlapping host reaps and the PID is absent after
cancellation. Four marker-only runs passed; a newer revision also checks a
test-held resolver lock immediately after a counted reap, passed once and
failed an unlocked negative control. Marker ordering alone does not prove the
resolver was still alive at each counted reap. The probe uses the explicit unsafe transport
flag and does not establish exclusive service ownership, bound OS teardown or
authenticate a provider. See `../fi-jws/TRANSPORT-EVIDENCE.md` for source hash,
initial cold-start failures and the narrow independent review.

A separate test-only reaperless pipe experiment avoids parent-side
supervisor PID waits/signals. Its single-purpose supervisor waits for its own
resolver child and internal EOF before emitting a complete response or cleanup
terminal. The default-disabled client retains a separate response pipe through
cancellation and keeps one slot busy until terminal plus EOF; missing terminal
quarantines the slot. Native and sanitizer probes exercised host-wide reaping,
SIGCHLD auto-reap, busy/reuse and a deliberate supervisor-crash/live-resolver
counterexample. This is not an installed or authenticated service, provider or
FI authority, and no universal cleanup deadline is claimed. Exact hashes,
remaining assumptions and independent review are in
`../fi-jws/TRANSPORT-EVIDENCE.md`.

A macOS-only mutual XPC peer scratch test checks service-side
client requirements and client-side service CDHash. A different ad-hoc binary
with the same allowed client identifier was accepted, demonstrating that this
identifier-only test requirement is spoofable and is not deployable identity.
It makes no iOS or authority claim; exact hash and controls are recorded in
`../fi-jws/TRANSPORT-EVIDENCE.md`.

The test-only reaperless client experiment passes an advisory lock through
supervisor to resolver. In the crash fixture, a fresh process cannot acquire
the same inode until the orphan exits. This relies on a stable trusted inode
and a cooperative helper. A negative control with an untrusted helper that
fakes `FDE1` and drops the lock starts a second helper while both descendants
remain alive; therefore the default-disabled client is not a provider and its
terminal/lock cannot establish cleanup without authenticated implementation and
channel binding. Exact test hashes and review limitations are in the transport
evidence; the pipe CLOEXEC race and host reaping policy remain unresolved.

The scratch experiments and byte-identical disabled prototype copies are
under `coax/tests/nostr/`; the original untracked platform prototypes remain in
the existing worktree. They are not product adapters or enabled providers.
