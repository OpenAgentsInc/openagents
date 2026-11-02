# 88. Federated identity

A community relay often already has an identity provider: a company login,
a school account, a hosted directory. NIP-FI lets the relay accept those
identities without keeping its own account table. The provider signs a
short-lived token, an *assertion*, that names a Nostr key. The relay checks
the token offline against the provider's published public keys, and NIP-42
(chapter 38) proves that the client holds the named key. This chapter builds
the offline check: RSA signature verification with SHA-256, the compact JWS
and JWKS formats, and the community, issuer, audience and clock rules, with
one algorithm allowed and every other refused.

Part VI, Every NIP. Result: a local NIP-FI verifier for the selected profile
`fi-rs256-policy-v1`. It frames a compact JWS from its original bytes,
projects a closed protected header (`RS256` only), selects the community from
`Host` and the issuer from the community's allowlist, checks the token class,
audience and time bounds, finds the key by exact `kid` in a strictly parsed
JWKS, and verifies the RSA PKCS#1 v1.5 signature over the original signing
input. Fetching the JWKS, pairing with NIP-42, sessions and NIP-FA's browser
login are not built; see [What is not built](#what-is-not-built).

## What this chapter teaches

- What a NIP-FI assertion is: a compact JWS whose claims bind an identity
  `(iss, sub)` to one Nostr key, for one community's audience, for a short
  time.
- RSA signature verification: `s^e mod n` with `e = 65537`, worked by hand
  with small numbers, and the Montgomery form the code uses for 2048-bit
  numbers.
- The PKCS#1 v1.5 encoding that RS256 compares against: `00 01`, a run of
  `ff`, `00`, a fixed SHA-256 DigestInfo prefix and the 32-byte digest.
- Why the verifier fixes the algorithm in configuration and never reads it
  from the token or the key: `none`, `HS256` and every other value refuse.
- How the relay picks the community from `Host`, the issuer from that
  community's allowlist, and only then reads issuer-specific keys.
- The time rules `now < exp`, `iat <= now + skew`, `now < iat + maximum_age`
  and their edges.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [rsa_verify.coax](../../coax/lib/crypto/rsa_verify.coax) | `rsa_verify_sha256(modulus, signature, message)` | RSASSA-PKCS1-v1_5 with SHA-256 and `e = 65537`, for 2048- to 4096-bit moduli |
| [fi-base64url.coax](../../coax/lib/nostr/block/fi-base64url.coax) | `fi_b64url_decode`, `fi_b64url_uint` | Canonical unpadded base64url; minimal big-endian integers for JWK `n` and `e` |
| [fi-compact.coax](../../coax/lib/nostr/block/fi-compact.coax) | `fi_compact_parse(token)` | Splits `header.payload.signature` and keeps the original signing span |
| [fi-protected.coax](../../coax/lib/nostr/block/fi-protected.coax) | `fi_protected_parse(text)` | The closed protected header: exactly `alg`, `typ`, `kid` |
| [fi-jwks-parse.coax](../../coax/lib/nostr/block/fi-jwks-parse.coax) | `fi_jwks_parse(text)` | The closed public RSA key set, 1 to 16 keys, unique `kid` |
| [fi-jws-signature.coax](../../coax/lib/nostr/block/fi-jws-signature.coax) | `fi_jws_signature(token, modulus)` | One token against one modulus |
| [fi-jws-jwks.coax](../../coax/lib/nostr/block/fi-jws-jwks.coax) | `fi_jws_document_signature(token, jwks)` | Exact `kid` lookup, then `fi_jws_signature` |
| [fi-policy.coax](../../coax/lib/nostr/block/fi-policy.coax) | `fi_select`, `fi_token_class`, `fi_time_bounds`, `fi_claims_candidate` | Community and issuer selection, token class, claims and clock |
| [fi-claims-parse.coax](../../coax/lib/nostr/block/fi-claims-parse.coax) | `fi_claims_profile(text, marker, mode)` | The strict claims projection: duplicates, types, sizes |
| [fi-claims-select.coax](../../coax/lib/nostr/block/fi-claims-select.coax) | `fi_claims_select` | Reads `iss` first, selects the policy, then projects the rest |
| [fi-claims-candidate.coax](../../coax/lib/nostr/block/fi-claims-candidate.coax) | `fi_claims_semantic_candidate` | Every claim rule, as `candidate`, `unavailable` or `rejected` |
| [fi-local-jws-candidate.coax](../../coax/lib/nostr/block/fi-local-jws-candidate.coax) | `fi_local_jws_candidate(token, jwks, communities, issuers, host, now)` | The whole local chain |
| [fi-jwks.coax](../../coax/lib/nostr/block/fi-jwks.coax) | `fi_fetch_candidate`, `fi_snapshot_key` | Fetch-observation and snapshot rules for a future key provider |
| [fi-projection.coax](../../coax/lib/nostr/adapters/fi-projection.coax) | `fpa_read(wire)` | Checks that a protobuf projection of an assertion agrees with its original compact bytes |

Every file is an `#authority` file: its checks came before its code, and a
mutation run kills every mutant (see [How the code is
checked](#how-the-code-is-checked)). This chapter changes no library code.

## The assertion

NIP-FI's assertion is a compact JWS (RFC 7515): three base64url segments
joined by dots.

```text
BASE64URL(protected header) . BASE64URL(claims) . BASE64URL(signature)
```

The signature covers the ASCII bytes of the first two segments and the dot
between them, the *signing input*. The verifier checks those original bytes.
It never re-serializes the JSON, because two JSON texts with the same meaning
have different bytes and different signatures.

The claims that NIP-FI requires:

| Claim | Meaning | Rule in this profile |
| --- | --- | --- |
| `iss` | The issuer URI | Exact match against the community's allowlist; no normalization |
| `sub` | The subject, opaque | 1 to 256 visible ASCII bytes; never an email address |
| `aud` | The audience | A string or 1 to 16 strings; one must equal the community's audience exactly |
| `nostr_pubkey` | The Nostr key | Exactly 64 lowercase hex digits |
| `iat`, `exp` | Issued at, expires | Integers in `0..2^53 - 1` |
| `nbf` | Not before (optional) | Same domain; when present, `nbf <= now + skew` |

The identity is the pair `(iss, sub)`. The same `sub` under two issuers is two
identities. The protected `typ` chooses the token class before any claim is
read: `nip-fi+jwt` is a person, `nip-fi-agent+jwt` is an agent (and must name
a distinct `nostr_owner_pubkey`), and `at+jwt` is an access token that needs a
`client_id` and a configured marker claim. Failure under one class never
retries under another.

The client sends the token on the WebSocket upgrade in one header,
`Nostr-Federated-Identity: Bearer <compact-JWS>`. `Authorization` stays
reserved for NIP-98. This chapter starts from the token's bytes; reading the
header from an HTTP request is chapter 49's job.

## RSA signatures

An RSA public key is a modulus `n`, the product of two secret primes, and an
exponent `e`. The signer computes `s = m^d mod n` with a secret `d`; anyone
checks `s^e mod n = m`. Here `m` is not the message: it is the message's
SHA-256 digest inside a fixed encoding of the same length as `n` (the next
section). The profile fixes `e = 65537`, the only value JWKS publishers use
in practice, as the three bytes `01 00 01`.

Because `65537 = 2^16 + 1`, the code computes `s^65537` as 16 squarings and
one multiplication:

```text
s^65537 = (s^(2^16)) · s
```

### Worked example: 65^65537 mod 3233, by hand

Take the textbook key `n = 61 · 53 = 3233` and the signature `s = 65`.
Square 16 times, reducing mod 3233 each time:

| k | s^(2^k) mod 3233 | computed as |
| ---: | ---: | --- |
| 1 | 992 | 65² = 4225 = 3233 + 992 |
| 2 | 1232 | 992² = 984064 = 304·3233 + 1232 |
| 3 | 1547 | 1232² = 1517824 = 469·3233 + 1547 |
| 4 | 789 | 1547² = 2393209 = 740·3233 + 789 |
| 5 | 1785 | |
| 6 | 1720 | |
| 7 | 205 | |
| 8 | 3229 | 205² = 42025 = 12·3233 + 3229 |
| 9 | 16 | 3229 = −4, and (−4)² = 16 |
| 10 | 256 | |
| 11 | 876 | 256² = 65536 = 20·3233 + 876 |
| 12 | 1155 | |
| 13 | 2029 | |
| 14 | 1232 | |
| 15 | 1547 | |
| 16 | 789 | |

Then multiply once: `789 · 65 = 51285 = 15·3233 + 2790`. So
`65^65537 mod 3233 = 2790`.

There is a shortcut that checks the answer. `λ(3233) = lcm(60, 52) = 780` and
`65537 = 84·780 + 17`, so `65^65537 ≡ 65^17 (mod 3233)`, and `65^17 mod 3233
= 2790` is the textbook RSA example. With `d = 2753`, `2790^2753 mod 3233 =
65`: the signer's side.

A 12-bit modulus is factored by hand, so `rsa_verify_sha256` refuses it:
`rsa_modulus_ok` admits only 256 to 512 bytes (2048 to 4096 bits), with the
top bit of a 256-byte modulus set and `n` odd, and the signature exactly as
long as the modulus. It also requires `s < n`.

### Montgomery form

For 2048-bit numbers, each `mod n` would be a long division. The code works
in Montgomery form instead. With `R = 2^(32·count)` for `count` 32-bit limbs,
a number `a` is held as `a·R mod n`, and `rsa_mont_mul(a, b)` returns
`a·b·R⁻¹ mod n` using only multiplications, additions and shifts by whole
limbs (CIOS, one limb of `n` per step). `rsa_verify_sha256`:

1. moves `s` into Montgomery form by doubling it `32·count` times mod `n`
   (`rsa_add_mod`), which multiplies it by `R`;
2. squares 16 times with `rsa_mont_mul`;
3. multiplies by the Montgomery `s` once;
4. multiplies by plain `1`, which removes the factor `R`.

The example runs exactly these steps on `n = 3233` with one limb and prints
the 16 squarings of the table above.

## PKCS#1 v1.5 with SHA-256

RS256 (RFC 7518 §3.3) is RSASSA-PKCS1-v1_5 with SHA-256 (RFC 8017 §8.2).
For a `k`-byte modulus, the value `s^e mod n` must be exactly:

```text
00 01 | ff × (k − 54) | 00 | 30 31 30 0d 06 09 60 86 48 01 65 03 04 02 01 05 00 04 20 | SHA-256(signing input)
```

The 19-byte middle part is the DER DigestInfo that names SHA-256. For
`k = 256`, the `ff` run is 202 bytes, the separator is byte 204, the prefix is
bytes 205 to 223 and the digest is bytes 224 to 255. `rsa_pkcs1_sha256`
compares every byte, so a short `ff` run, a different hash name or trailing
bytes refuse. It does not parse the DER: it compares bytes, which removes the
parsing mistakes that led to forged PKCS#1 signatures in the past.

### Worked example: the test token's encoding

The example token's signature raised to 65537 with its 2048-bit key gives
`00 01`, 202 bytes of `ff`, a `00`, the prefix above, and
`703a1014...4567448e`. SHA-256 of the token's 322-byte signing input is the
same `703a1014...4567448e`. The example prints both.

## The header and the key set

### The closed protected header

`fi_protected_parse` accepts a JSON object with exactly three members:

- `alg`, which must be `"RS256"`;
- `typ`, which must be `nip-fi+jwt`, `nip-fi-agent+jwt` or `at+jwt`;
- `kid`, 1 to 128 visible ASCII bytes.

Any other member refuses, including `jku`, `jwk`, `x5u` and `crit`, so a
token can't point the verifier at a key of its choice. A duplicate member
refuses, also when it is spelled with a JSON escape (`"alg"`). Text
after the object refuses.

The value of `alg` is never used to choose code. The profile has one
algorithm; the header must agree with it or the token refuses. This is RFC
8725's advice and closes the two classic JWT attacks: `"alg":"none"` (no
signature at all) and `"alg":"HS256"` (an HMAC keyed with the public RSA key,
which anyone can compute).

### The key set

A JWKS is `{"keys":[...]}`. `fi_jwks_parse` accepts each key only with
exactly these members: `kid`, `kty` = `RSA`, `alg` = `RS256`, `use` = `sig`,
`n`, `e` = `AQAB` (65537), and optionally `key_ops` = `["verify"]`. `n` must
be a minimal unsigned big-endian integer of 2048 to 4096 bits and odd. A
private member (`d`, `p`, `q`, ...), an unknown member, a different `alg` or
`kty`, a duplicate `kid`, or more than 16 keys refuses the whole set: the
verifier never picks the one good key out of a set it doesn't understand.

`fi_jws_find_key` then compares the header's `kid` byte for byte with each
key's. A missing `kid` refuses; the verifier doesn't fall back to trying
every key.

## Community, issuer and claims

NIP-FI's verification resolves the community first. The relay's
configuration maps each `Host` to one community: its canonical audience
(`wss://relay.test` here) and its issuer allowlist. The community is
configuration, not a claim, so the token can't choose it.

`fi_claims_select` reads the claims twice:

1. The first pass reads only `iss` (`mode 0`), skipping every other value.
2. `fi_select` resolves `Host` to the community, requires `iss` to be on its
   allowlist and finds the one issuer policy with that exact `iss`. An
   unknown Host, an invalid community table or an invalid issuer policy is
   `unavailable`; an issuer that isn't on the allowlist is `rejected`.
3. The second pass reads every claim under the selected issuer's class and
   marker, and its `iss` must equal the first pass's.

`fi_claims_semantic_candidate` then applies `fi_token_class` (the header's
`typ` against the issuer's configured class) and `fi_claims_candidate` (exact
`iss`, `sub` size, audience membership, key shape and time). NIP-FI's two
denial codes map to the results: `unavailable` is `authorization_unavailable`
and `rejected` is `evidence_rejected`.

The example's issuer is configured for the `at+jwt` class. Its marker claim
is `iss` itself, with the resource-owner set `{"https://issuer.test"}` and the
client set `{"service"}`, so the token's marker value classifies it as a
resource owner's token. A dedicated issuer has no marker.

### Worked example: the clock

The token has `iat = 100` and `exp = 200`. The issuer has `maximum_age = 10`
and `skew = 0`. The rules are `now < exp`, `iat <= now + skew` and
`now < iat + maximum_age`:

| now | `now < 200` | `100 <= now` | `now < 110` | Result |
| ---: | --- | --- | --- | --- |
| 99 | yes | no | yes | rejected: issued in the future |
| 100 | yes | yes | yes | candidate |
| 101 | yes | yes | yes | candidate |
| 109 | yes | yes | yes | candidate |
| 110 | yes | yes | no | rejected: older than `maximum_age` |
| 200 | no | yes | no | rejected: expired |

The token is accepted for 10 seconds, though its `exp` is 100 seconds after
`iat`: the relay's own age limit is the tighter bound. Equality at `exp` and at
`iat + maximum_age` is expired.

### The order of the checks

NIP-FI's procedure verifies the signature before the remaining claims.
`fi_local_jws_candidate` checks the claims first and reads the JWKS only for a
token that passes them. Every check must pass either way, so the result is the
same; the difference is that an unknown Host, an issuer that isn't allowed or
an expired token never touches the key set. The sanitizer runs place the JWKS
in unreadable memory to show that.

## What is not built

- **Fetching the JWKS.** `fi-jwks.coax` decides whether a fetch observation
  is acceptable (HTTPS, every DNS answer outside the IANA special-purpose
  ranges, one pinned address, no redirect, at most 524,288 bytes) and whether
  a snapshot is current (bound to its issuer, source, policy digest and
  installed generation, refreshed before its hard deadline). No Coax code
  performs the fetch, and the caller of `fi_local_jws_candidate` supplies the
  JWKS text. A local result proves a key/signature/claims match, not that the
  key came from the issuer.
- **Pairing with NIP-42.** NIP-FI accepts a key only when `nostr_pubkey`
  equals the key that signed the NIP-42 AUTH event (chapter 38). Nothing
  compares them yet. The test token's `nostr_pubkey` is 64 zeros, which has
  the right shape and is not a valid key.
- **Sessions.** The connection deadline (`fi_session_deadline` computes the
  minimum of the connection lifetime, `exp`, `iat + maximum_age` and the
  snapshot deadline), re-authentication, the admin disconnect API and HTTP
  ingress have no runtime.
- **The full `AssertionPolicyId`.** The hash of the whole policy that binds a
  snapshot is supplied by the caller; nothing builds it.
- **NIP-FA.** Browser login with a loopback callback and an S256 handoff
  challenge, the code exchange, the session check and the relay-assertion
  request have no Coax code. NIP-FA issues `nip-fi+jwt` tokens with
  `aud = https://` plus the relay's authority; this chapter's verifier would
  check them once an issuer policy for them is configured.
- **Other algorithms.** No ES256, EdDSA or PS256; they refuse.

## The example

[nostr/examples/federated-identity.coax](../examples/federated-identity.coax)
verifies the test token from
[coax/tests/nostr/data/fi-jws](../../coax/tests/nostr/data/fi-jws/). Its key
is a throwaway 2048-bit key whose private half was deleted after signing; the
example has only the public JWKS. It runs the worked RSA example, opens the
token, raises its signature to 65537 and shows the PKCS#1 encoding, then
runs the whole chain at a fixed clock with one accepted case and 15 refusals.
The `stages` procedure prints each stage separately: the header projection,
the claims result, the signature result and the final answer of
`fi_local_jws_candidate`.

<!-- source: nostr/examples/federated-identity.coax example -->
```jai
#load "../../coax/lib/nostr/block/fi-local-jws-candidate.coax";
#load "../../coax/lib/text/base64.coax";
#import "Basic";

// A test token signed with a throwaway 2048-bit key whose private half was
// deleted (coax/tests/nostr/data/fi-jws/claims-token.txt and claims-jwks.json).
TOKEN :: "eyJhbGciOiJSUzI1NiIsInR5cCI6ImF0K2p3dCIsImtpZCI6InRlc3Qta2V5In0.eyJpc3MiOiJodHRwczovL2lzc3Vlci50ZXN0Iiwic3ViIjoic3ViamVjdCIsImF1ZCI6IndzczovL3JlbGF5LnRlc3QiLCJub3N0cl9wdWJrZXkiOiIwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwIiwiaWF0IjoxMDAsImV4cCI6MjAwLCJjbGllbnRfaWQiOiJtb2JpbGUifQ.XSeCVpjfcpU3ghfQ8PZIMCCZDeke4nk87Fjsag7mGboO2qpbc-j7xHvu1utTsQXaud1jjqpV5v9FQc4yApNLpxa01gkDs8sNR_dGPd67LUjYhpAdJSWbIJbbBNf-uXzzA35fRFThFGcbZ6gCyxq7KRmb5n35g9ExqunVPUmw-4FQB1veh_C1DZC_8-oqQnIolHvqPqKA_6Qvo8EC6J6KYL7QMPOm0UuywCew9F1LQDMFowFOOCMVaxS7UHXcxL_IN0VjNaVAstpn67oLCnaTSVRyeRf6RTQmgbBlUeP3KmjoOzSnYKPG6Zs6e39dgN3k7BfqyPLqM6Ay5cUyvb4_8w";
KEY_N :: "mSSukJYIEze9DyYlGyp2FJ3o6HTXmrQ1YRceZ_AbJuraYooxZ3qs46aZsRZeuNmb3i4pBZTNJkzhgE2ewEvQ11JQuyMWjlYoCTgvVpaazIEvGnXFRLL7AR7J_PlEZPwxFVcQZP7O-pLIp65V0Y9t6WmSEK1WZ3XG6FLo834haItBHj1deE-_lu_DC0ku6LqU8Uf4s79PBf7K5YOz2eh_7m81DKbuPSuXSqMD0XQcu0npAdW8-wuYNqaQvzFPn7lE7wJKHGkuLCJ4qkVxG3UmkTWCUPdY5Borg7nVNK9owvm3E6TySCL9DYx4439UXfcvhwEtdZaZhxhBxu9ZVm5wWQ";
ISSUER :: "https://issuer.test";

// Build tokens and JWKS documents in a caller's buffer: each helper writes
// at position at and returns the new end.
put :: (out: [] u8, at: int, text: string) -> int {
  for i: 0..<text.count { out[at + i] = text[i]; }
  return at + text.count;
}
// Unpadded base64url (RFC 7515 §2): standard base64 with - and _, no '='.
put_b64url :: (out: [] u8, at: int, text: string) -> int {
  std: [4096] u8; n := b64_encode(text, std[..]);
  w := at;
  for i: 0..<n {
    c := std[i];
    if c == '=' { break; }
    if c == '+' { c = '-'; } else if c == '/' { c = '_'; }
    out[w] = c; w += 1;
  }
  return w;
}
// A one-key JWKS with the test modulus and the given kid, kty and alg.
jwks_with :: (out: [] u8, kid: string, kty: string, alg: string) -> string {
  at := put(out, 0, "{\"keys\":[{\"kid\":\""); at = put(out, at, kid);
  at = put(out, at, "\",\"kty\":\""); at = put(out, at, kty);
  at = put(out, at, "\",\"alg\":\""); at = put(out, at, alg);
  at = put(out, at, "\",\"use\":\"sig\",\"n\":\""); at = put(out, at, KEY_N);
  at = put(out, at, "\",\"e\":\"AQAB\"}]}");
  return cast(string) out[0..<at];
}
// TOKEN with its header replaced by the base64url of header_json.
with_header :: (out: [] u8, header_json: string) -> string {
  parts := fi_compact_parse(TOKEN);
  at := put_b64url(out, 0, header_json);
  at = put(out, at, TOKEN[parts.protected_end..<TOKEN.count]);
  return cast(string) out[0..<at];
}
// TOKEN with its payload replaced by the base64url of claims_json.
with_payload :: (out: [] u8, claims_json: string) -> string {
  parts := fi_compact_parse(TOKEN);
  at := put(out, 0, TOKEN[0..<parts.payload_start]);
  at = put_b64url(out, at, claims_json);
  at = put(out, at, TOKEN[parts.signing_end..<TOKEN.count]);
  return cast(string) out[0..<at];
}
// Print a label and bytes as lowercase hex.
show_hex :: (label: string, bytes: [] u8) {
  digits := "0123456789abcdef";
  out: [1024] u8;
  for i: 0..<bytes.count { out[2 * i] = digits[bytes[i] >> 4]; out[2 * i + 1] = digits[bytes[i] & 15]; }
  print("%: %\n", label, cast(string) out[0..<2 * bytes.count]);
}
status_name :: (s: int) -> string {
  if s == FI_CANDIDATE { return "candidate"; }
  if s == FI_UNAVAILABLE { return "unavailable"; }
  return "rejected";
}

// s^65537 mod n the way rsa_verify_sha256 computes it: move s into
// Montgomery form by 32·count doublings, square 16 times, multiply by s once,
// and multiply by 1 to leave Montgomery form. squares[k] keeps s^(2^(k+1))
// mod n (its low limb), taken out of Montgomery form to print.
Power :: struct { value: RsaWords; squares: [16] u32; }
pow65537 :: (s: RsaWords, n: RsaWords, count: int) -> Power {
  result: Power;
  inverse := rsa_mont_inverse(n.words[0]);
  one: RsaWords; one.words[0] = 1;
  base := s; for i: 0..<count * 32 { base = rsa_add_mod(base, base, n, count); }
  value := base;
  for i: 0..<16 {
    value = rsa_mont_mul(value, value, n, count, inverse);
    result.squares[i] = rsa_mont_mul(value, one, n, count, inverse).words[0];
  }
  value = rsa_mont_mul(value, base, n, count, inverse);
  result.value = rsa_mont_mul(value, one, n, count, inverse);
  return result;
}

// Run every stage of the local verifier on one token and print each result.
stages :: (label: string, token: string, jwks: string, cs: [] FiCommunity, ps: [] FiIssuer, host: string, now: int) {
  bytes: [4096] u8; for i: 0..<token.count { bytes[i] = token[i]; }
  t := bytes[0..<token.count];
  accepted := fi_local_jws_candidate(t, jwks, cs, ps, host, now);
  parts := fi_compact_parse(token);
  if !parts.ok { print("%: framing refused; accepted %\n", label, accepted); return; }
  hb: [2048] u8; hd := fi_b64url_decode(token[0..<parts.protected_end], hb[..]);
  h := fi_protected_parse(cast(string) hb[0..<hd.written]);
  if !h.ok { print("%: header refused; accepted %\n", label, accepted); return; }
  pb: [4096] u8; pd := fi_b64url_decode(token[parts.payload_start..<parts.signing_end], pb[..]);
  header := FiHeader.{algorithm = "RS256", typ = fi_local_typ(h.typ), kid = cast(string) h.kid[0..<h.kid_count]};
  claims := fi_claims_semantic_candidate(cs, ps, host, header, cast(string) pb[0..<pd.written], now);
  signature := fi_jws_document_signature(t, jwks);
  print("%: claims %, signature %, accepted %\n", label, status_name(claims), signature, accepted);
}

main :: () {
  // 1. RSA by hand: n = 61·53 = 3233, signature s = 65, e = 65537.
  tiny_n: RsaWords; tiny_n.words[0] = 3233;
  tiny_s: RsaWords; tiny_s.words[0] = 65;
  tiny := pow65537(tiny_s, tiny_n, 1);
  print("65^(2^k) mod 3233, k = 1..16:");
  for i: 0..<16 { print(" %", tiny.squares[i]); }
  print("\n65^65537 mod 3233 = %\n", tiny.value.words[0]);
  tiny_bytes := [2] u8.[12, 161];
  print("rsa_modulus_ok accepts the 2-byte modulus 3233: %\n", rsa_modulus_ok(tiny_bytes[..], 2));

  // 2. The token's three parts.
  parts := fi_compact_parse(TOKEN);
  hb: [2048] u8; hd := fi_b64url_decode(TOKEN[0..<parts.protected_end], hb[..]);
  pb: [4096] u8; pd := fi_b64url_decode(TOKEN[parts.payload_start..<parts.signing_end], pb[..]);
  print("header: %\n", cast(string) hb[0..<hd.written]);
  print("claims: %\n", cast(string) pb[0..<pd.written]);
  print("signing input: % bytes, signature: % bytes\n", parts.signing_end, parts.signature_size);

  // 3. The key, and the signature raised to e = 65537 with it.
  jwks_buffer: [1024] u8; good := jwks_with(jwks_buffer[..], "test-key", "RSA", "RS256");
  keys := fi_jwks_parse(good);
  key := keys.keys[0];
  print("JWKS: ok %, % key, kid %, modulus % bits\n", keys.ok, keys.count,
        cast(string) key.kid[0..<key.kid_count], key.modulus_count * 8);
  sig: [512] u8; sd := fi_b64url_decode(TOKEN[parts.signature_start..<TOKEN.count], sig[..]);
  n := rsa_load(key.modulus[0..<key.modulus_count]); s := rsa_load(sig[0..<sd.written]);
  m := pow65537(s, n, rsa_word_count(key.modulus_count)).value;
  em: [256] u8;
  for i: 0..<256 { em[255 - i] = cast(u8) ((m.words[i / 4] >> ((i % 4) * 8)) & 255); }
  run := 0; while em[2 + run] == 255 { run += 1; }
  show_hex("s^e mod n starts", em[0..<2]);
  print("then % bytes of ff and a % byte\n", run, em[2 + run]);
  show_hex("DigestInfo", em[3 + run..<22 + run]);
  show_hex("digest in the signature", em[22 + run..<256]);
  signing: [512] u8; for i: 0..<parts.signing_end { signing[i] = TOKEN[i]; }
  digest: [32] u8;
  #assert sha256_hash_checked(signing[0..<parts.signing_end], digest[..], false) == SHA_OK;
  show_hex("SHA-256 of the signing input", digest[..]);
  print("rsa_pkcs1_sha256: %\n", rsa_pkcs1_sha256(em[..], digest[..]));

  // 4. The relay's policy: one community and the issuer it trusts.
  names := [1] string.[ISSUER]; owners := [1] string.[ISSUER]; clients := [1] string.["service"];
  cs: [1] FiCommunity; cs[0].host = "relay.test"; cs[0].audience = "wss://relay.test"; cs[0].issuers = names[..];
  ps: [1] FiIssuer; ps[0].issuer = ISSUER; ps[0].jwks_uri = "https://keys.test"; ps[0].transport = 1;
  ps[0].token_class = FI_ACCESS_TOKEN; ps[0].maximum_age = 10; ps[0].connection_lifetime = 10;
  ps[0].refresh = 1; ps[0].hard_deadline = 30; ps[0].marker_name = "iss";
  ps[0].owner_values = owners[..]; ps[0].client_values = clients[..];
  stages("now 101", TOKEN, good, cs[..], ps[..], "relay.test", 101);

  // 5. Refusals: the clock, the community and the issuer.
  stages("now 99, issued in the future", TOKEN, good, cs[..], ps[..], "relay.test", 99);
  stages("now 110, older than maximum_age", TOKEN, good, cs[..], ps[..], "relay.test", 110);
  stages("now 200, at exp", TOKEN, good, cs[..], ps[..], "relay.test", 200);
  stages("unknown Host other.test", TOKEN, good, cs[..], ps[..], "other.test", 101);
  cs[0].audience = "wss://other.test";
  stages("community audience wss://other.test", TOKEN, good, cs[..], ps[..], "relay.test", 101);
  cs[0].audience = "wss://relay.test";
  names[0] = "https://other.test";
  stages("issuer not on the community's allowlist", TOKEN, good, cs[..], ps[..], "relay.test", 101);
  names[0] = ISSUER;

  // 6. Refusals: the protected header.
  forged: [2048] u8;
  stages("header alg none", with_header(forged[..], "{\"alg\":\"none\",\"typ\":\"at+jwt\",\"kid\":\"test-key\"}"),
         good, cs[..], ps[..], "relay.test", 101);
  stages("header alg HS256", with_header(forged[..], "{\"alg\":\"HS256\",\"typ\":\"at+jwt\",\"kid\":\"test-key\"}"),
         good, cs[..], ps[..], "relay.test", 101);
  stages("header typ JWT", with_header(forged[..], "{\"alg\":\"RS256\",\"typ\":\"JWT\",\"kid\":\"test-key\"}"),
         good, cs[..], ps[..], "relay.test", 101);
  stages("header with jku", with_header(forged[..], "{\"alg\":\"RS256\",\"typ\":\"at+jwt\",\"kid\":\"test-key\",\"jku\":\"https://evil.test\"}"),
         good, cs[..], ps[..], "relay.test", 101);

  // 7. Refusals: the key set.
  other: [1024] u8;
  wrong_kid := fi_jwks_parse(jwks_with(other[..], "old-key", "RSA", "RS256"));
  print("old-key JWKS: parses %, index of test-key %\n", wrong_kid.ok, fi_jws_find_key(wrong_kid, "test-key"));
  print("PS256 JWKS parses: %\n", fi_jwks_parse(jwks_with(other[..], "test-key", "RSA", "PS256")).ok);
  print("oct/HS256 JWKS parses: %\n", fi_jwks_parse(jwks_with(other[..], "test-key", "oct", "HS256")).ok);
  stages("JWKS without kid test-key", TOKEN, jwks_with(other[..], "old-key", "RSA", "RS256"),
         cs[..], ps[..], "relay.test", 101);
  stages("JWKS key with alg PS256", TOKEN, jwks_with(other[..], "test-key", "RSA", "PS256"),
         cs[..], ps[..], "relay.test", 101);
  stages("JWKS key with kty oct, alg HS256", TOKEN, jwks_with(other[..], "test-key", "oct", "HS256"),
         cs[..], ps[..], "relay.test", 101);

  // 8. Refusals: the signed bytes.
  stages("payload sub changed to attacker",
         with_payload(forged[..], "{\"iss\":\"https://issuer.test\",\"sub\":\"attacker\",\"aud\":\"wss://relay.test\",\"nostr_pubkey\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"iat\":100,\"exp\":200,\"client_id\":\"mobile\"}"),
         good, cs[..], ps[..], "relay.test", 101);
  at := put(forged[..], 0, TOKEN);
  forged[at - 2] = ifx forged[at - 2] == 'A' then 'B' else 'A';
  stages("one signature character changed", cast(string) forged[0..<at], good, cs[..], ps[..], "relay.test", 101);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/federated-identity.coax
```

The squarings and `2790` are the worked example's. The digest inside the
signature equals SHA-256 of the signing input, and the clock rows match the
table above:

```text
65^(2^k) mod 3233, k = 1..16: 992 1232 1547 789 1785 1720 205 3229 16 256 876 1155 2029 1232 1547 789
65^65537 mod 3233 = 2790
rsa_modulus_ok accepts the 2-byte modulus 3233: false
header: {"alg":"RS256","typ":"at+jwt","kid":"test-key"}
claims: {"iss":"https://issuer.test","sub":"subject","aud":"wss://relay.test","nostr_pubkey":"0000000000000000000000000000000000000000000000000000000000000000","iat":100,"exp":200,"client_id":"mobile"}
signing input: 322 bytes, signature: 256 bytes
JWKS: ok true, 1 key, kid test-key, modulus 2048 bits
s^e mod n starts: 0001
then 202 bytes of ff and a 0 byte
DigestInfo: 3031300d060960864801650304020105000420
digest in the signature: 703a10140badeb1399751fdefb2779ee26f52224e07ce257c22b01e14567448e
SHA-256 of the signing input: 703a10140badeb1399751fdefb2779ee26f52224e07ce257c22b01e14567448e
rsa_pkcs1_sha256: true
now 101: claims candidate, signature true, accepted true
now 99, issued in the future: claims rejected, signature true, accepted false
now 110, older than maximum_age: claims rejected, signature true, accepted false
now 200, at exp: claims rejected, signature true, accepted false
unknown Host other.test: claims unavailable, signature true, accepted false
community audience wss://other.test: claims rejected, signature true, accepted false
issuer not on the community's allowlist: claims rejected, signature true, accepted false
header alg none: header refused; accepted false
header alg HS256: header refused; accepted false
header typ JWT: header refused; accepted false
header with jku: header refused; accepted false
old-key JWKS: parses true, index of test-key -1
PS256 JWKS parses: false
oct/HS256 JWKS parses: false
JWKS without kid test-key: claims candidate, signature false, accepted false
JWKS key with alg PS256: claims candidate, signature false, accepted false
JWKS key with kty oct, alg HS256: claims candidate, signature false, accepted false
payload sub changed to attacker: claims candidate, signature false, accepted false
one signature character changed: claims candidate, signature false, accepted false
```

The refusals:

- **The clock.** At `now = 99`, `110` and `200` the claims are `rejected`;
  the signature still verifies, and the token is refused.
- **The community.** An unknown Host (`other.test`) is `unavailable`. A
  community whose audience is `wss://other.test`, or whose allowlist doesn't
  name `https://issuer.test`, is `rejected`. Done means: a JWS with a wrong
  audience or community refuses.
- **The header.** `alg` `none` or `HS256`, `typ` `JWT`, and an extra `jku`
  refuse in `fi_protected_parse`, before any claim or key is read. Done means:
  `none` and symmetric algorithms refuse.
- **The key set.** A JWKS that has only `old-key` parses but has no
  `test-key` (index `-1`). A key that says `alg` `PS256`, or `kty` `oct` with
  `alg` `HS256`, refuses the whole set. Done means: confused algorithms
  refuse, in the key set as in the header.
- **The signed bytes.** The claims with `sub` changed to `attacker` pass
  every claim rule, and the signature refuses them. One changed character of
  the signature refuses too.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/federated-identity.out](../fixtures/federated-identity.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/federated-identity.coax](../examples/checks/federated-identity.coax),
which loads the example. They compare with values that share no code with the
verifier:

- RFC 7515 Appendix A.2, the published RS256 example: its signature verifies
  over its signing input with its modulus, and one flipped bit in the input or
  the signature refuses;
- the 16 squarings and `2790`, computed with Python's `pow()`, against the
  Montgomery chain;
- `s^65537` by plain `u64` square-and-multiply against the Montgomery chain,
  for 200 values mod 3233 and mod the prime `4294967291`;
- the test token's digest, from Python's `hashlib`;
- nine algorithm names (`none`, `HS256`, `HS384`, `HS512`, `RS384`, `RS512`,
  `PS256`, `ES256`, `EdDSA`) refused in the header and in the key set;
- a wrong Host, an `https://` audience for a `wss://` community, a community
  under another Host, and the dedicated class for an `at+jwt` token, each
  refused by the whole chain.

<!-- source: nostr/examples/checks/federated-identity.coax check -->
```jai
// RFC 7515 Appendix A.2: the published RS256 example, its modulus and its
// signature (coax/tests/nostr/data/fi-jws/rfc7515-a2.json). Its header
// {"alg":"RS256"} has no typ or kid, so only the RSA layer applies.
A2_N :: "ofgWCuLjybRlzo0tZWJjNiuSfb4p4fAkd_wWJcyQoTbji9k0l8W26mPddxHmfHQp-Vaw-4qPCJrcS2mJPMEzP1Pt0Bm4d4QlL-yRT-SFd2lZS-pCgNMsD1W_YpRPEwOWvG6b32690r2jZ47soMZo9wGzjb_7OMg0LOL-bSf63kpaSHSXndS5z5rexMdbBYUsLA9e-KXBdQOS-UTo7WTBEMa2R2CapHg665xsmtdVMTBQY4uDZlxvb3qCo5ZwKh9kG4LT6_I5IhlJH7aGhyxXFvUK-DWNmoudF8NAco9_h9iaGNj8q2ethFkMLs91kzk2PAcDTW9gb54h4FRWyuXpoQ";
A2_TOKEN :: "eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ.cC4hiUPoj9Eetdgtv3hF80EGrhuB__dzERat0XF9g2VtQgr9PJbu3XOiZj5RZmh7AAuHIm4Bh-0Qc_lF5YKt_O8W2Fp5jujGbds9uJdbF9CUAr7t1dnZcAcQjbKBYNX4BAynRFdiuB--f_nZLgrnbyTyWzO75vRK5h6xBArLIARNPvkSjtQBMHlb1L07Qe7K0GarZRmB_eSN9383LcOLn6_dO--xi12jzDwusC-eOkHWEsqtFZESc6BfI7noOPqvhJ1phCnvWh6IeYI2w9QOYEUipUTI8np6LbgGY9Fs98rqVt5AXLIhWkWywlVmtVrBp0igcN_IoypGlUPQGe77Rw";
#test "RFC 7515 A.2 verifies, and a changed byte does not" #steps 1000000000 {
  n: [512] u8; nd := fi_b64url_uint(A2_N, n[..]);
  #assert nd.error == NC_OK && nd.written == 256;
  parts := fi_compact_parse(A2_TOKEN);
  #assert parts.ok && parts.signature_size == 256;
  sig: [512] u8; sd := fi_b64url_decode(A2_TOKEN[parts.signature_start..<A2_TOKEN.count], sig[..]);
  #assert sd.error == NC_OK && sd.written == 256;
  input: [512] u8; for i: 0..<parts.signing_end { input[i] = A2_TOKEN[i]; }
  #assert rsa_verify_sha256(n[0..<256], sig[0..<256], input[0..<parts.signing_end]);
  input[30] = input[30] ^ 1;
  #assert !rsa_verify_sha256(n[0..<256], sig[0..<256], input[0..<parts.signing_end]);
  input[30] = input[30] ^ 1; sig[255] = sig[255] ^ 1;
  #assert !rsa_verify_sha256(n[0..<256], sig[0..<256], input[0..<parts.signing_end]);
}
// The hand values of the chapter, computed with Python's pow(): the 16
// squarings of 65 mod 3233 and 65^65537 mod 3233 = 2790.
SQUARES :: [16] u32.[992, 1232, 1547, 789, 1785, 1720, 205, 3229, 16, 256, 876, 1155, 2029, 1232, 1547, 789];
#test "the Montgomery chain gives the hand squarings of 65 mod 3233" {
  n: RsaWords; n.words[0] = 3233; s: RsaWords; s.words[0] = 65;
  p := pow65537(s, n, 1);
  for i: 0..<16 { #assert p.squares[i] == SQUARES[i]; }
  #assert p.value.words[0] == 2790;
}
// Plain square-and-multiply in u64 shares no code with the Montgomery path.
plain_pow65537 :: (s: u64, n: u64) -> u64 {
  v := s % n;
  for i: 0..<16 { v = v * v % n; }
  return v * (s % n) % n;
}
#forall "s^65537 mod 3233 and mod 4294967291 match plain arithmetic" s: 0..<200 {
  a: RsaWords; a.words[0] = cast(u32) s;
  n: RsaWords; n.words[0] = 3233;
  if s < 3233 { #assert pow65537(a, n, 1).value.words[0] == cast(u32) plain_pow65537(cast(u64) s, 3233); }
  big: RsaWords; big.words[0] = 4294967291;
  b: RsaWords; b.words[0] = cast(u32) (s * 21474836 + 7);
  #assert pow65537(b, big, 1).value.words[0] == cast(u32) plain_pow65537(cast(u64) b.words[0], 4294967291);
}
// The test token's digest, from Python's hashlib over the signing input, and
// the whole chain accepting it with its JWKS at the fixed clock.
DIGEST :: "703a10140badeb1399751fdefb2779ee26f52224e07ce257c22b01e14567448e";
#test "the test token's digest and the whole local chain" #steps 1000000000 {
  parts := fi_compact_parse(TOKEN);
  input: [512] u8; for i: 0..<parts.signing_end { input[i] = TOKEN[i]; }
  digest: [32] u8;
  #assert sha256_hash_checked(input[0..<parts.signing_end], digest[..], false) == SHA_OK;
  want: [32] u8;
  #assert hex_decode_checked(DIGEST, want[..], false).error == 0;
  for i: 0..<32 { #assert digest[i] == want[i]; }
  buffer: [1024] u8; jwks := jwks_with(buffer[..], "test-key", "RSA", "RS256");
  token: [1024] u8; at := put(token[..], 0, TOKEN);
  #assert fi_jws_document_signature(token[0..<at], jwks);
}
// Done means: none, symmetric and other algorithms refuse in the header and
// in the key set; only RS256 projects.
ALGS :: [9] string.["none", "HS256", "HS384", "HS512", "RS384", "RS512", "PS256", "ES256", "EdDSA"];
#forall "every algorithm but RS256 refuses in the header" i: 0..<9 {
  out: [256] u8; at := put(out[..], 0, "{\"alg\":\""); at = put(out[..], at, ALGS[i]);
  at = put(out[..], at, "\",\"typ\":\"at+jwt\",\"kid\":\"test-key\"}");
  #assert !fi_protected_parse(cast(string) out[0..<at]).ok;
  #assert fi_protected_parse("{\"alg\":\"RS256\",\"typ\":\"at+jwt\",\"kid\":\"test-key\"}").ok;
}
#forall "every algorithm but RS256 refuses in the key set" i: 0..<9 {
  buffer: [1024] u8;
  #assert !fi_jwks_parse(jwks_with(buffer[..], "test-key", "RSA", ALGS[i])).ok;
  #assert fi_jwks_parse(jwks_with(buffer[..], "test-key", "RSA", "RS256")).ok;
}
// Done means: a wrong audience or community refuses. The policy is the
// example's; each change alone turns the candidate into a refusal.
#test "wrong audience and wrong community refuse" #steps 1000000000 {
  names := [1] string.[ISSUER]; owners := [1] string.[ISSUER]; clients := [1] string.["service"];
  cs: [1] FiCommunity; cs[0].host = "relay.test"; cs[0].audience = "wss://relay.test"; cs[0].issuers = names[..];
  ps: [1] FiIssuer; ps[0].issuer = ISSUER; ps[0].jwks_uri = "https://keys.test"; ps[0].transport = 1;
  ps[0].token_class = FI_ACCESS_TOKEN; ps[0].maximum_age = 10; ps[0].connection_lifetime = 10;
  ps[0].refresh = 1; ps[0].hard_deadline = 30; ps[0].marker_name = "iss";
  ps[0].owner_values = owners[..]; ps[0].client_values = clients[..];
  token: [1024] u8; at := put(token[..], 0, TOKEN);
  buffer: [1024] u8; jwks := jwks_with(buffer[..], "test-key", "RSA", "RS256");
  #assert fi_local_jws_candidate(token[0..<at], jwks, cs[..], ps[..], "relay.test", 101);
  #assert !fi_local_jws_candidate(token[0..<at], jwks, cs[..], ps[..], "other.test", 101);
  cs[0].audience = "https://relay.test";
  #assert !fi_local_jws_candidate(token[0..<at], jwks, cs[..], ps[..], "relay.test", 101);
  cs[0].audience = "wss://relay.test"; cs[0].host = "other.test";
  #assert !fi_local_jws_candidate(token[0..<at], jwks, cs[..], ps[..], "relay.test", 101);
  cs[0].host = "relay.test"; ps[0].token_class = FI_DEDICATED;
  #assert !fi_local_jws_candidate(token[0..<at], jwks, cs[..], ps[..], "relay.test", 101);
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/federated-identity.coax
coax/bin/coax mutate nostr/examples/checks/federated-identity.coax --root-only
```

On 2026-11-01, `coax test` passed 268 checks, including the loaded files' own
checks, in about 4 seconds, and the root-only mutation run killed 9 of 9
mutants.

### The library's records

These ran when the library files were written and were not repeated for this
chapter, because the library files did not change:

- [rsa_verify.py](../../coax/tests/nostr/rsa_verify.py) compares
  `rsa_verify_sha256` in checked and release native and wasm builds with
  Python integer exponentiation and an exact PKCS#1 encoder on the C2SP
  Wycheproof RSA PKCS#1 SHA-256 vectors (2048, 3072, 4096 and 8192 bits;
  1031 comparisons per mode) and 300 Montgomery products per mode from 1 to
  128 limbs. `coax mutate` killed 1018 of 1018 mutants. The record is
  [CURRENT-EVIDENCE.md](../../coax/tests/nostr/data/rsa/CURRENT-EVIDENCE.md)
  (2026-10-28).
- [fi_policy.py](../../coax/tests/nostr/fi_policy.py) covers community,
  class, time, fetch observations and snapshots (add, remove, duplicate,
  revoked, stale, relabeled) in native, ASan/UBSan and wasm builds; the
  `fi-jwks.coax` closure killed 782 of 782 mutants. The record is
  [2026-10-28-fi-rs256-policy-v1.md](../../docs/protocol/2026-10-28-fi-rs256-policy-v1.md).
- [fi_local_jws_candidate.py](../../coax/tests/nostr/fi_local_jws_candidate.py)
  signs fresh tokens with a temporary key and OpenSSL, deletes the key, and
  runs 33 signed, malformed and tampered cases through the whole chain in
  native, wasm and ASan/UBSan builds. The closure mutation run killed 3397 of
  3397 mutants. The record is
  [LOCAL-CANDIDATE-EVIDENCE.md](../../coax/tests/nostr/data/fi-jws/LOCAL-CANDIDATE-EVIDENCE.md).

These are correctness and refusal checks on fixtures. They are not a
security audit, a constant-time measurement (RSA verification handles only
public values) or a trace of a real provider.

## Summary

- A NIP-FI assertion is a compact JWS binding `(iss, sub)` to one Nostr key
  for one community's audience and a short time.
- `rsa_verify_sha256` computes `s^65537 mod n` as 16 Montgomery squarings
  and one multiplication, and compares every byte of the PKCS#1 v1.5 SHA-256
  encoding.
- The algorithm is configuration: `fi_protected_parse` and `fi_jwks_parse`
  accept `RS256` and nothing else, and refuse unknown header and key members.
- The community comes from `Host`, the issuer from the community's allowlist,
  and the audience must equal the community's; the clock enforces `exp`,
  `iat` and the relay's own maximum age.
- `fi_local_jws_candidate` runs the chain on the original bytes. Fetching
  keys, NIP-42 pairing, sessions and NIP-FA are not built.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [1. Bytes, hex and base64](../../bitcoin/book/ch001-bytes-hex-and-base64.md)

## Sources

- [Block NIP-FI](../nips/block/NIP-FI.md)
- [Block NIP-FA](../nips/block/NIP-FA.md)
- The selected profile:
  [2026-10-28-fi-rs256-policy-v1.md](../../docs/protocol/2026-10-28-fi-rs256-policy-v1.md).
- [RFC 7515, JSON Web Signature](https://www.rfc-editor.org/rfc/rfc7515),
  Appendix A.2, pinned in
  [rfc7515-a2.json](../../coax/tests/nostr/data/fi-jws/rfc7515-a2.json).
- [RFC 7517, JSON Web Key](https://www.rfc-editor.org/rfc/rfc7517) and
  [RFC 7518 §3.3, RS256](https://www.rfc-editor.org/rfc/rfc7518#section-3.3).
- [RFC 8017 §8.2 and §9.2, RSASSA-PKCS1-v1_5](https://www.rfc-editor.org/rfc/rfc8017).
- [RFC 8725, JSON Web Token Best Current Practices](https://www.rfc-editor.org/rfc/rfc8725).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
