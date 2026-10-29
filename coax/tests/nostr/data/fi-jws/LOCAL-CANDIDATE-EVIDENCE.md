# Local original-JWS semantic/signature candidate

`fi-local-jws-candidate.coax` is local fixture evidence only. It
reads protected header and payload from the original compact token, maps the
three supported types explicitly, applies untrusted semantic candidate policy,
then verifies the original signing span against a caller-supplied JWKS. That
JWKS has no authenticated provider provenance. A `true` result is only a
local key/signature/semantic match; it does not prove key-source trust, NIP-42
possession, session or FI identity authority. The caller must keep token,
JWKS and all configuration/slice storage valid and immutable for the call.
Semantic failure happens before the supplied JWKS is examined.

The earlier reviewed source passed 254 Coax check groups. `fi_local_jws_candidate.py` generates
a fresh temporary test key, signs and independently verifies fixture tokens
with OpenSSL, and discards the private key. Thirty-three signed, malformed and
tampered cases passed each checked/release native and wasm32-wasip1 mode and
native ASan/UBSan. The Wasm harness explicitly provisions a 4 MiB stack;
this is not evidence for an unconfigured browser host. The tests include semantic/class/issuer/audience/time,
class-aware dedicated versus access client_id handling and agent-owner
positive, missing, equal-key and human-owner refusals; original-byte
tampering including semantically equivalent whitespace changes,
unknown/duplicate/private keys and unsupported
headers. ASan/UBSan guard-page tests also place the JWKS in an unreadable mapped
region and confirm unknown-Host, unauthorized-issuer and expired-claim refusals do not read it. The
public-only Coax fixture was generated separately; its private key was deleted.
The closure mutation outcome and rerun are described below.

Independent read-only source review (`fixedbase_rescue`) found no concrete
blocker in the local caller-supplied-key fixture scope at the source hash below.
It checked exact compact spans, three-type mapping, semantic ordering and the
original-token verification call. It does not approve transport, provider,
possession, session, identity or production use.

SHA-256:

- `coax/lib/nostr/block/fi-local-jws-candidate.coax`: `324c7ef738b59fe5290c1738a1fc99331869634c4aa845fb7233f1c9a3fe1e7b`
- `coax/tests/nostr/fi_local_jws_candidate.py`: `d50fbcd54613bb509941a0e9cb12a4c11ec907893251d5a51ea413b0dd08f90a`
- `coax/tests/nostr/fi_local_jws_fixture.c`: `33e3703e2aa955822304f0568023053be1bb4c7a64a1c22855bfd4946e84ed4f`
- `coax/tests/nostr/fi_local_jws_wasm.cjs`: `0a89b9430a41333983ec182b05cd5270609c67c35a58bcada38bc9e12facbaa0`
- `claims-token.txt`: `595bea8fc4cc9d53af14c863f200dba4b57039437752d855cd5f900e4cf46fc3`
- `claims-jwks.json`: `5a46ec8992440d64e1f26c056a05d8628fa266ee4dc1b12159ff1aefe939b805`
- `claims-public.pem`: `b8ee2cfd7bd3728fb20c58959ee907c258fdd8ef90e6452d22c37c6cda3bcd27`

The 33-case local chain was also compiled and exercised on checked/release
native, Wasm and ASan/UBSan with the separately verified current-main Coax
0.21.0 binary SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`.
This is target/test evidence, not an authenticated provider or deployment.
The same 0.21.0 binary passed all 254 Coax check groups on the earlier reviewed source in the local chain.
The mutation gates use the previously pinned 0.18.2 compiler (SHA-256
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`); no
current-compiler mutation result is implied by this test run.

The first complete local closure mutation run killed 3393 of 3397 mutations;
four survived and the gate did not pass. Subsequent test-only additions exercise
a signed one-byte `kid` and invalid header/decode and payload-capacity paths.
The resulting source SHA-256 is
`756b48d6ab09a0e461c5ae5ee6c582b5dbfe8ce82ee1b88999efe0fb5a9b1d48` and
passes 256 Coax checks on both 0.18.2 and the verified 0.21.0 binary. The full
default-budget mutation rerun with pinned 0.18.2 killed 3397/3397, with no
survivors or waivers. The additional
public-only fixture was generated with an ephemeral key, then its private key
was deleted. OpenSSL independently verified the original compact signing span
against the public PEM, and the protected and JWK `kid` values both equal `k`.
Independent read-only delta review (`fixedbase_rescue`) confirmed that the
change from the previously reviewed hash to the current source adds only
boundary tests, with no production procedure changes; its earlier limitations
continue to apply.

As a separate diagnostic, each of the four original surviving mutations was
applied individually to a temporary copy and tested. Every probe failed one
of the 256 checks. This targeted result is separate from the full mutation gate.


- `shortkid-token.txt`: `09f4b1337d46e7cbabeca19e673e3ff89f71ee0eebab70ad5f7e60a42d059b6e`
- `shortkid-jwks.json`: `bcfa30893eaec012ae6224f72710d42456058f22f6ec41a3a0f958ae894dca0a`
- `shortkid-public.pem`: `f48273bcb9d19eddc1af0c9b317040181adcea1b493b2df7a9ce6b715661ad2c`
