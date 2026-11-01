# 52. Reposts, reactions and comments

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

People respond to events in three ways. This chapter reads and writes each
one.

Part X, Every NIP. Result: Kind 6 and 16 reposts, kind 7 reactions (`+`, `-`,
emoji) and kind 17 website reactions, and NIP-22 kind 1111 comments with
uppercase root and lowercase parent scope tags.

## What this chapter teaches

- NIP-18: kind 6 for kind 1, kind 16 with a `k` tag for other kinds; quote
  reposts with a `q` tag.
- NIP-25: kind 7 content `+`, `-` or an emoji; `e`, `p` and `k` tags; kind 17
  for websites.
- NIP-22: `E`/`A`/`I`/`K`/`P` for the root scope, `e`/`a`/`i`/`k`/`p` for the
  parent; never for kind 1 replies.

## What exists today

- `coax/lib/nostr/nip22.coax` reads kind 1111 comments and the task-plan,
  claim and revision tags OpenAgents uses with them. NIP-18 and NIP-25 are not
  implemented.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md)

## Done means

- A comment whose root and parent scopes disagree in kind refuses.
- Reactions aggregate per target with one per author.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-18](../nips/official/18.md)
- [NIP-25](../nips/official/25.md)
- [NIP-22](../nips/official/22.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
