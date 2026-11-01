# 59. Groups and communities

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Groups need a place that enforces membership. NIP-29 makes a relay that place.
This chapter builds both the relay and the client side.

Part X, Every NIP. Result: NIP-29 relay-based groups with `h` tags, moderation
kinds 9000–9022 and relay-signed metadata 39000–39005; NIP-7D forum threads
(kind 11); NIP-C7 chats (kind 9); and the unrecommended NIP-72 and NIP-28.

## What this chapter teaches

- NIP-29: group ID `<host>'<id>`, `h` tag on every event, `previous` tags for
  timeline references.
- Moderation: 9000 put-user, 9001 remove-user, 9002 edit-metadata, 9005
  delete-event, 9007 create-group, 9008 delete-group, 9009 create-invite; join
  9021 and leave 9022.
- Relay-signed 39000 metadata, 39001 admins, 39002 members, 39003 roles.
- NIP-7D: kind 11 threads with kind 1111 replies; NIP-C7: kind 9 chat messages
  with `q` quotes.
- NIP-72 (kind 34550 communities) and NIP-28 (kinds 40–44 public chat) are
  `unrecommended`; read only.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [34. Admit an event](ch034-admit-an-event.md)
- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- A non-member's event to a closed group refuses.
- Group metadata is accepted only when signed by the relay's key.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-29](../nips/official/29.md)
- [NIP-7D](../nips/official/7D.md)
- [NIP-C7](../nips/official/C7.md)
- [NIP-72](../nips/official/72.md)
- [NIP-28](../nips/official/28.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
