# Source-preserving proto adapters (#2ff9)

#2ff9 asks for two things:

1. Real domains use the generated root proto codecs. Original Nostr and JWS
   bytes are kept, projections are compared, and bounds, presence and
   unknown-field policy are enforced.
2. Decoded owner flags, "verified" flags and other unknown flags cannot
   authorize an effect.

The two domain adapters landed with #161a (`d42b3c1c2f`):

- `coax/lib/nostr/adapters/event-projection.coax`, for NIP-01 events;
- `coax/lib/nostr/adapters/fi-projection.coax`, for Block FI JWS assertions.

This record maps them to the #2ff9 acceptance criteria. It also reruns their
gates on the Linux host, where the sources are unchanged since #161a. HOST
grant projections are not covered here; they are #3d81.

## Acceptance mapping

- **Actual source fixtures round-trip original bytes through generated
  codecs, and mismatched projections refuse.** This is true for both domains.
  - The event fixture `proto/nostr/fixtures/owner-attested.event.json` and its
    `.pb` round-trip through the generated `SignedEvent` codec and through
    protoc. The adapter compares every signed field and every ordered tag with
    the untouched JSON. Changing any one of `id`, `pubkey`, `sig`,
    `created_at`, `kind`, `content` or `tags` refuses. So do a duplicate kind,
    a missing original, an invalid original, truncated wire, a duplicate JSON
    key, and content over the profile limit.
  - The FI adapter does the same for the compact JWS header, payload, claims,
    numeric presence and audience shape, against a real RS256 fixture.
- **Decoded authority flags cannot authorize an effect.** This is tested in
  `event_projection_adapter.coax` and `event_projection.py`. The only effect is
  test-only, and the test counts it.
  - Unknown protobuf fields set to an authority value (fields 90–93), and JSON
    fields `verified`, `authorized` and `owner`, leave a projection consistent.
    They grant nothing.
  - A forged signature carrying those flags stays structurally consistent, but
    real BIP-340 verification fails and the effect count stays zero.
  - A genuinely signed event whose signer is not the independently configured
    owner is also refused.
  - In the FI domain, a forged signature that matches structurally fails
    OpenSSL and the Coax RSA verifier.

## Rerun on this host

The compiler was `coax0 0.22.0`, binary SHA-256
`6e385ae9e0d92cca16a8818ca11a9f5c594b19299958963c1812b272f1cc68b8`,
rebuilt from current main. The adapter sources were event
`1946d02e0cd1cbf2e816ad3b2e7e0e773a3118c1a93e7686da6bec9ce4ce8c29` and FI
`fa887723059edbb2d4632f0b14aecbefab83a5a276b3638ba04d7fee71bce0fd`.

| Check | Result | Record |
| --- | --- | --- |
| `coax test …/event-projection.coax --run-steps 250000000` | 195/195 | [event-checks.log](data/proto-adapters-2026-10-29/event-checks.log) |
| `coax test …/fi-projection.coax --run-steps 250000000` | 210/210 | [fi-checks.log](data/proto-adapters-2026-10-29/fi-checks.log) |
| `event_projection.py` | protoc 34.1 exact envelope round-trip; 18 cases on checked and release native and Wasm, plus ASan/UBSan | [event-fixture.log](data/proto-adapters-2026-10-29/event-fixture.log) |
| `fi_projection.py` | protoc 34.1 and OpenSSL signed and forged controls; 19 cases on checked and release native and Wasm, plus ASan/UBSan | [fi-fixture.log](data/proto-adapters-2026-10-29/fi-fixture.log) |
| FI `coax mutate --mutation-reference` | 3,162/3,162 killed, full schedule, 236,387 checks, 0 skipped, no waivers | [fi-reference-trace.log](data/proto-adapters-2026-10-29/fi-reference-trace.log) |
| Event `coax mutate --mutation-reference` | Stopped at 546/4,306 mutants. No survivors had appeared, but this is **not a pass**. At this host's rate a full run would take about a day | — |

For the event adapter, the authority gate remains the complete #161a run on
the same source hash: 4,306/4,306, trace SHA-256
`c695531970a3cbe94606682b7aaea05ba4c2d88a2c4ecdb6164c75252105a21b`
([proto-consumers](2026-10-29-proto-consumers.md)). No iOS Simulator run was
possible on this host. #161a's Mac run covers that.

The two fixture scripts now read `COAXIAL_CLANG`, `COAXIAL_SYSROOT` and
`COAXIAL_RESOURCES` for Wasm, as `coax/web/wasm.sh` does. The defaults are
still the Mac Homebrew paths. Wasm here used clang 21, `wasm-ld` and
wasi-libc 27 from nix, under Node WASI.

## Limits

- A consistent projection is not signature verification, identity,
  membership, freshness or authorization.
- Borrowed input must be live, stable and physically valid for the whole
  call.
- The kind range (#ef04) and generated error precedence (#5869) remain open.
