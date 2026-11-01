# 67. Relay discovery, access and management

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Relays describe themselves, control who may join, and accept management calls.
This chapter builds all three for the Coax relay and its operators.

Part X, Every NIP. Result: NIP-66 relay discovery (30166) and monitor
announcements (10166), NIP-43 membership lists (13534) and join and leave
requests (28934–28936), and the NIP-86 JSON-RPC management API over HTTP with
NIP-98 auth.

## What this chapter teaches

- NIP-66: kind 30166 with `d` the relay URL and tags `rtt-open`, `rtt-read`,
  `n`, `N`, `R`, `T`; monitors publish kind 10166 with their frequency.
- NIP-43: kind 13534 member list signed by the relay; 8000 add and 8001
  remove; 28934 join with a claim, 28935 invite, 28936 leave.
- NIP-86: `POST` with `Content-Type: application/nostr+json+rpc`, methods such
  as `banpubkey`, `allowpubkey`, `listbannedpubkeys`, `banevent`,
  `changerelayname`; NIP-98 `Authorization` with the payload hash.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [37. NIP-11 relay information](ch037-nip-11-relay-information.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [49. HTTP auth](ch049-http-auth.md)

## Done means

- Management calls without a valid NIP-98 event from an admin key refuse.
- A member list not signed by the relay's NIP-11 `self` key is ignored.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-66](../nips/official/66.md)
- [NIP-43](../nips/official/43.md)
- [NIP-86](../nips/official/86.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
