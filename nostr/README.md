# Nostr

This folder holds the Nostr specifications OpenAgents implements and the book
that builds them in Coax.

| Path | What |
| --- | --- |
| [`nips/`](nips/README.md) | Official, external and OpenAgents NIPs, pinned by [`nips/manifest.json`](nips/manifest.json) |
| [`book/`](book/README.md) | *Nostr From Scratch*: every primitive, the client, the relay and every NIP, built in Coax |

The book is published at https://openagents.gitbook.io/nostr-from-scratch.

The code the book teaches lives in the Coax library, not here:
[`coax/lib/crypto/`](../coax/lib/crypto/), [`coax/lib/nostr/`](../coax/lib/nostr/),
[`coax/lib/net/`](../coax/lib/net/) and the relay in [`coax/relay/`](../coax/relay/).
The plan for that code is [coax/nostr.md](../coax/nostr.md). A chapter is
filled in as its code and checks land.

```sh
scripts/check-nips-pin.sh --offline                          # the pinned NIP files
python3 psionic/tests/book.py --book nostr                   # the book's structure and registry
python3 psionic/tests/book.py --book nostr --coax coax/bin/coax   # also run its examples
```
