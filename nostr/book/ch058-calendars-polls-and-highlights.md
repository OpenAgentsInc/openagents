# 58. Calendars, polls and highlights

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

This chapter covers three structured content types that share tag patterns.

Part X, Every NIP. Result: NIP-52 date (31922) and time (31923) events,
calendars (31924) and RSVPs (31925); NIP-88 polls (1068) and responses (1018);
NIP-84 highlights (9802).

## What this chapter teaches

- NIP-52: `start`/`end` as ISO dates or Unix times with `start_tzid`; RSVP
  status `accepted`, `declined`, `tentative`.
- NIP-88: `option` tags, `polltype` `singlechoice` or `multiplechoice`,
  `endsAt`; one counted response per pubkey, the latest.
- NIP-84: kind 9802 with the highlighted text, source `e`/`a`/`r` tags and
  optional `context`.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Poll tallies on a fixture with repeated votes match the NIP's
  latest-response rule.
- A date event with `end` before `start` refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-52](../nips/official/52.md)
- [NIP-88](../nips/official/88.md)
- [NIP-84](../nips/official/84.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
