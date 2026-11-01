# 61. Live activities

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Live streams and meeting spaces announce themselves as addressable events.
This chapter reads and writes them.

Part X, Every NIP. Result: NIP-53 kind 30311 live events with `streaming`,
`status` and participant roles, kind 1311 live chat, and kind 30312/30313
meeting spaces and rooms with kind 10312 presence.

## What this chapter teaches

- Kind 30311: `d`, `title`, `streaming`, `recording`, `starts`, `ends`,
  `status` `planned`/`live`/`ended`, `p` with role.
- Kind 1311: chat with an `a` tag to the activity.
- Kinds 30312 and 30313 for spaces and their meetings; kind 10312 presence.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- An activity with `status` `live` and no update for an hour is shown as
  ended, as the NIP recommends.
- Chat events reference the activity by its `a` address.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-53](../nips/official/53.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
