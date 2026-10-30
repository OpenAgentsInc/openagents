# NIP-98 public request observation (#db7e)

`coax/lib/nostr/nip98-observe.coax` compares caller-supplied request fields.
It is not an authentication or authorization entry point: it does not decode
an HTTP header, parse or verify an event, bind the fields to original bytes,
hash a body, validate a URL, reserve replay IDs, or issue a session. An actual
API adapter needs all of those independently reviewed steps. No production
API or phone endpoint uses this module.

The observer compares kind, a nonnegative inclusive ±60-second window, exact
URL, ASCII-case-insensitive method, and an optional lowercase SHA-256 digest
against the supplied digest. It requires exactly one `u` and one `method`,
at most one `payload`, no malformed selected tag, and a nonempty URL and
method. If no payload tag is present it does not bind the body. The caller
must supply truthful tag counts and digest derived from the same request and
original event; accepting arbitrary caller assertions as authority is unsafe.

The existing API is more permissive for some cases: it accepts repeated `u`
and `method` tags using the last value, a present but empty `payload` as if
absent, and a selected tag with extra elements. The observer rejects those
cases. The existing Coax event verifier rejects duplicate JSON members while
the legacy API accepts them in some cases; conversely, the Coax event
verifier accepts valid unknown event fields that the legacy API rejects.
This is a record of differences, not a decision to change the production API.

## Tests

At source revision `215139c62d46a40112e97616c42f6fbe7d482ee9` with the new
files in this change, on macOS arm64:

- `coax/bin/coax test coax/lib/nostr/nip98-observe.coax --library` passed
  6/6 checks. After the requested no-payload/nonempty-body control was added,
  `coax/bin/coax mutate coax/lib/nostr/nip98-observe.coax --library
  --mutation-reference --mutation-stats --jobs 2` killed all 92/92 mutants;
  216 reference checks ran, zero were skipped and no waiver was used.
- `python3 coax/tests/nostr/api_request_observation.py
  PublicRequestObservation.test_source_bound_public_request_fixtures` passed
  21 public cases in each of checked and release native builds, comparing the
  current legacy `nip98_check` with the observer. The test passes the exact
  same immutable original JSON and request body to both paths. The Coax
  event verifier must accept the original first; its returned original bytes
  and fields are checked against that same input before **test-only Python**
  extracts tags and hashes the supplied body. It checks invalid signatures,
  changed content, method, URL and body, both time boundaries, and expected
  duplicate/unknown-field differences. It independently verifies the three
  additional fixed fixture signatures against the bigint BIP-340 oracle and
  their NIP-01 SHA-256 preimages. Those fixtures use throwaway test scalar 3.
  This demonstrates a test-harness composition, not a source-bound Coax API
  adapter, production admission, or replay protection.
- `python3 coax/tests/nostr/api_request_observation.py SanitizedObserver`
  passed checked and release native observer drivers under ASan/UBSan,
  including extreme signed timestamps and malformed cardinality.
- `python3 coax/tests/nostr/api_request_observation.py WasmObserver` passed
  14 observer-only cases each on checked and release WASI WebAssembly, with
  a 1 MiB stack. It does not run the event verifier or a live browser.

The provenance-verified Coax 0.24.0 compiler SHA-256 was
`641429d9a4be0d018e15525375bad1ec44a15a0ab5d6301120060d69b689ac1a`;
compiler input fingerprint
`dac4ca73305c69923a2adb9aea2df7193d9c6bc529994dd3a0ba38ecf020996c`.
The observer source SHA-256 was
`55f4e366a11d162b443dec74bc1c961960f775170f646be5e2f98f7df999758f`.
The native harness SHA-256 was
`205aba57dcf52a82c8babc1f8025ab27bf20333f847c86d38f6c0a7032f4ceb6`;
the three added fixture file SHA-256 was
`e4fb8e78e1c566725cf5c6a2367e0e49ce85a56f268bc192cc59195719baeb28`.
The legacy API verifier and Coax event source SHA-256 values were
`e6a67b2dcfacbd3b288cfa31376688c2969d15b3fa8a487cc8c8ecec498cb5a6`
and `3b0c0feca25959ecd4a4138a3e74a795c65258198fcb1510c0f9e5b2d3e5419f`.
Native `cc` was Apple clang 21.0.0; Wasm used Homebrew clang 23.1.4,
WASI libc and Node 26.12.1; the legacy oracle linked libsecp256k1 0.8.0
and OpenSSL 4.1.0. No timing, security review, signing, production-secret,
physical-iOS or deployed-request claim follows from these tests.
