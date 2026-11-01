# 63. Git on Nostr

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

OpenAgents publishes its projects and work as NIP-34 events. This chapter
builds the typed readers and writers for them.

Part X, Every NIP. Result: NIP-34 repository announcements (30617) and state
(30618), patches (1617), pull requests (1618, 1619), issues (1621) and
statuses (1630–1633), and NIP-C0 code snippets (1337).

## What this chapter teaches

- Kind 30617: `d`, `name`, `clone`, `web`, `relays`, `maintainers`, `r` with
  `euc` for the earliest unique commit.
- Kind 30618: `refs/heads/*` and `HEAD`.
- Kinds 1617, 1618, 1619 and 1621 tied to the repository by `a`.
- Statuses 1630 open, 1631 applied or merged, 1632 closed, 1633 draft; the
  newest from an authorized author wins.
- NIP-C0: kind 1337 with `l` language, `name`, `extension`.

## What exists today

- `coax/lib/nostr/nip34.coax` and `nip34-event.coax` read repositories, issues
  and statuses, plus OpenAgents project and ordering records. NIP-C0 is not
  implemented.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [52. Reposts, reactions and
  comments](ch052-reposts-reactions-and-comments.md)

## Done means

- A status from someone who is neither the author nor a maintainer is ignored.
- The project fold over a captured relay matches its recorded tables.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-34](../nips/official/34.md)
- [NIP-C0](../nips/official/C0.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
