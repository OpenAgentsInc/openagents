# Public API event-verifier comparison

This is a partial prerequisite for `web-nostr-api-verifier-parity`. It compares
`coax/web/api/nostr.c`'s `nostr_verify_event` with checked and release native
builds of `coax/lib/nostr/event.coax`'s `nostr_event_verify_vartime`. Both
consume the same original public JSON. Neither result grants authority here.
A separate small check calls the current `nip98_check` directly with
fixed fixtures; this is **not** a comparison with a Coax request adapter. It
does not cover replay protection, deployed routes, or an actual Coax API
adapter. No production code or authorization behavior changed.

Run `python3 coax/tests/nostr/api_event_parity.py` with a built Coax compiler
and `pkg-config` packages `libsecp256k1` and `openssl`. Eight fixtures in
`coax/tests/nostr/data/api-public-events.json` use an explicitly throwaway test
key. An independent bigint BIP-340 verifier and Python SHA-256 validate their
signatures and NIP-01 preimages. The fixture-only independent check is
`python3 coax/tests/nostr/api_event_parity.py IndependentFixtureCheck`.

On 2026-10-29, the differential test passed for both checked and release
Coax output on macOS arm64 at source revision `cbc956f245`, using
provenance-verified Coax 0.24.0 compiler SHA-256 `58d4b5585f3e1a7f819c7972d87828317723d5d4bdbec6133e002c2b093c5cf8`.
The fixture-only independent check also passed. For ordinary signed events,
both implementations accepted and agreed on id, author, timestamp and kind;
both refused modified signatures, modified content, missing signatures and
trailing bytes. The following are observed event-verifier differences, not a
judgment that either acceptance policy is suitable for HTTP authentication:

| Fixture | Current API event verifier | Coax event verifier |
| --- | --- | --- |
| Unknown JSON field | Refuses | Accepts |
| Duplicate identical `kind` field | Accepts | Refuses |
| Embedded NUL in content | Refuses on ID mismatch | Accepts |
| 17 tags | Refuses at its 16-tag bound | Accepts |
| Negative timestamp | Accepts at the event layer | Refuses |

The direct `nip98_check` fixtures accept both identical and conflicting
repeated `kind` members if the final kind is 27235 and the signed event uses
that value. Repeated `u` tags also take the final value. The test checks a
matching payload hash, refuses mismatched and empty bodies, accepts timestamps
at both 60-second boundaries, and refuses timestamps 61 seconds away. It also
checks the existing method case-insensitivity and wrong URL/method rejection.
These are local function observations, not deployed requests, a claim of
protocol conformance, or a demonstrated exploit. The request path applies
additional rules; the negative timestamp observation by itself says nothing
about whether an HTTP request is accepted. These fixtures prevent silent
policy drift while the
full request-level comparison and an explicit compatibility/security review
are designed. They do not authorize migration or replace independent review.
