# NIP-01 in Coax

[NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) is Nostr's
basic protocol: the event format, how an event is identified and signed, the
filters clients ask with, and the messages clients and relays send each other.
OpenAgents runs it in [Coax](https://openagents.com/projects/coax), our own
language, compiled to C and to WebAssembly. The pinned copy of the
specification is `nips/official/01.md` in the repository.

This page, [openagents.com/nip/01](https://openagents.com/nip/01), runs
Coax's event checks on a real relay event in your browser.

## What Coax checks in an event

Given an event's original bytes, Coax does this before anything trusts it:

1. **Parse.** The event must have exactly the NIP-01 fields: `id`, `pubkey`,
   `created_at`, `kind`, `tags`, `content` and `sig`. The id, pubkey and
   signature must be lowercase hex of the right length, and the kind must be
   between 0 and 65535.
2. **Preimage.** Coax writes `[0,<pubkey>,<created_at>,<kind>,<tags>,<content>]`
   from the event's original bytes, with no whitespace and with NIP-01's
   escapes (`\n`, `\"`, `\\`, `\r`, `\t`, `\b`, `\f`) and every other
   character verbatim.
3. **SHA-256.** Coax hashes the preimage.
4. **Id.** The hash must equal the event's `id`. If one character of the
   content changed, the hash changes and the event is refused.
5. **BIP-340.** Coax verifies the Schnorr signature over the id with the
   event's pubkey, on the secp256k1 curve.

The project pages, such as [/projects/coax](https://openagents.com/projects/coax),
already read their relay in Coax: the `REQ`, and the `EVENT`, `EOSE` and
`CLOSED` frames. They still rely on relay.openagents.com's signature check.
Running these checks on every event they keep, within a frame's time budget,
is the next step ([`project-page-verify`](https://openagents.com/projects/coax)).

## Try it

The page shows one captured event (a repository announcement, kind 30617,
captured from relay.openagents.com on 2026-10-28) and four copies of it, each
changed in one place. Pick one with the buttons at the bottom or the keys 1 to 5:

| Button | What changed | Where it stops |
| --- | --- | --- |
| REAL | Nothing | Verified |
| ID | One digit of the id | Id: the hash is not the id |
| TEXT | One letter of the content | Id: the content no longer hashes to the id |
| SIG | One bit of the signature | BIP-340: the signature does not verify |
| KIND | Kind set to 99999 | Parse: not a NIP-01 kind |

The page shows the id the event claims and the hash Coax computed, digit by
digit, with the digits that differ in red. PARTS (key 6) lists every part of
NIP-01 and the Coax that runs it.

## Every part of NIP-01, and where it runs

| Part of NIP-01 | Runs in | Source |
| --- | --- | --- |
| Event fields and limits | Coax | `coax/lib/nostr/event.coax` |
| Id preimage and SHA-256 | Coax | `coax/lib/nostr/event.coax`, `coax/lib/crypto/sha256.coax` |
| BIP-340 verification | Coax | `coax/lib/crypto/schnorr.coax`, `schnorr_fast.coax` |
| BIP-340 signing | Next | `coax/lib/crypto/schnorr_sign.coax`, `coax/lib/nostr/event_sign.coax` (written, in its mutation gate) |
| Standard tags `e`, `p`, `a`, and `d` | Next | `coax/lib/nostr/tags.coax` (written, in its mutation gate) |
| Kind 0 user metadata | Next | `coax/lib/nostr/metadata.coax` (written, in its mutation gate) |
| Kind ranges: regular, replaceable, ephemeral, addressable | Coax | `coax/lib/nostr/store_policy.coax` |
| Replaceable events: keep the newest, lowest id on a tie | Coax | `coax/lib/nostr/store_policy.coax` |
| Filters: `ids`, `authors`, `kinds`, `#x`, `since`, `until`, `limit` | Coax | `coax/lib/nostr/filter.coax` |
| Client messages `EVENT`, `REQ`, `CLOSE` | Coax for `REQ`; next for all | `REQ` in `apps/openagents-web/project-live.coax`; next: `coax/lib/nostr/message.coax` |
| Relay messages `EVENT`, `OK`, `EOSE`, `CLOSED`, `NOTICE` | Coax for `EVENT`, `EOSE`, `CLOSED`; next for all | `apps/openagents-web/project-fold.coax`; next: `coax/lib/nostr/message.coax` |
| The WebSocket | The browser | `coax/web/host.js` opens it; `apps/openagents-web/socket.coax` drives it |

The WebSocket itself stays with the browser and the operating system, as
TLS does: Coax decides what to send and what each message means.

## What this does not change

- **Signing keys.** The Coax signer matches the official BIP-340 vectors and
  checks every signature it makes before returning it; it lands on main when
  its mutation gate passes, and its self-check becomes fixed-work under
  `schnorr-self-verification-boundary`. Publishing still signs with the
  existing TypeScript and C paths; moving real keys to the Coax signer is a
  separate, owner-approved step.
- **Authority.** A verified signature says who signed an event. It does not
  say the signer may do what the event asks. The project pages still accept
  only the project's own key.
- **Relay completeness.** A relay can still leave events out. Verification
  stops it from inventing or altering them.

## How it is checked

- The page's verifier is the same library code the project pages use, and its
  checks run the captured event and every changed copy
  (`apps/openagents-web/nip01-verify-test.coax`).
- The built page module runs in WebAssembly with every case, the keys and the
  buttons on desktop, phone and 2x screens (`coax/web/tests/nip01_wasm.cjs`).
- The event reader and verifier have their own checks and mutation records
  (`coax/lib/nostr/event.coax`, `coax/lib/crypto/schnorr_fast.coax`).
