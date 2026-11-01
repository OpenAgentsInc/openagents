# 48. Signers outside the app

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Many people keep their key in another program. This chapter builds the client
side of the three signer interfaces and the bunker side of NIP-46.

Part X, Every NIP. Result: NIP-07's `window.nostr`, NIP-46 remote signing over
kind 24133 with `bunker://` and `nostrconnect://` URIs, and NIP-55's Android
signer intents.

## What this chapter teaches

- NIP-07: `getPublicKey()`, `signEvent()`, and `nip44.encrypt`/`decrypt`; the
  page never sees the key.
- NIP-46: requests and responses as NIP-44-encrypted kind 24133 events;
  methods `connect`, `sign_event`, `get_public_key`, `nip44_encrypt`,
  `nip44_decrypt`, `ping`.
- Distinct client, remote-signer and user keys; call `get_public_key` after
  `connect`.
- One-use connect secrets, response correlation, permissions by kind and
  method.
- NIP-55: Android intents and content resolver; only described here.
- Legacy NIP-04 methods return an explicit error.

## What exists today

- `coax/lib/nostr/keys.coax` defines the signer-request contract for NIP-07
  and NIP-46. The iOS app signs with a vendored libsecp256k1
  (`apps/openagents-ios/OpenAgents/Platform/NostrSign.c`), which is not Coax.

## Dependencies

- [12. Keys and randomness](ch012-keys-and-randomness.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Done means

- A Coax client signs through a Coax bunker and an independent bunker.
- A reused connect secret refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-07](../nips/official/07.md)
- [NIP-46](../nips/official/46.md)
- [NIP-55](../nips/official/55.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
