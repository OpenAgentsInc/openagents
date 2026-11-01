# 49. HTTP auth

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-98 signs an HTTP request with a Nostr key. OpenAgents uses it for API
calls and sign-in. This chapter builds both sides.

Part X, Every NIP. Result: NIP-98 kind 27235 events in `Authorization: Nostr
<base64>`, binding the exact URL, method and payload hash within 60 seconds,
with single-use event IDs.

## What this chapter teaches

- Tags: `u` (absolute URL), `method`, optional `payload` (SHA-256 of the
  body).
- Server checks: kind 27235, time within the window, exact URL and method,
  payload hash, signature.
- Replay: each event ID is used once within the window, across workers and
  restarts.
- A valid NIP-98 event proves key possession for one request; it grants
  nothing else.

## What exists today

- `coax/lib/nostr/nip98-tags.coax` and `nip98-observe.coax` extract tags and
  observe request fields; `weblogin.coax` signs a login. The production
  verifier is still C (`coax/web/api/nostr.c`).
- Records:
  [2026-10-29-nip98-tags.md](../../docs/verification/2026-10-29-nip98-tags.md),
  [2026-10-29-nip98-observation.md](../../docs/verification/2026-10-29-nip98-observation.md).

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [4. SHA-256](ch004-sha-256.md)

## Done means

- Each server check has a refusing test.
- The same event ID used twice refuses the second time.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-98](../nips/official/98.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
