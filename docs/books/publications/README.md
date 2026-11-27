# Publication receipts

The publisher writes `BOOK.json` here by default after a successful real merge
and live inventory check. Keep each committed version in Git history. A failed
publication cannot replace a successful receipt.

| Book | Source/revision receipt introduced by this work |
| --- | --- |
| Coax | Not recorded; no real publication performed for the runner change. |
| Psionic | Not recorded; no real publication performed for the runner change. |
| OpenAgents Metaverse Engine | [`aethyr.json`](aethyr.json) (prior GitBook publication); live GitBook republish awaits owner credentials. |
| Nostr | Not recorded for live GitBook (requires owner GitBook credentials); offline route `nostr-event-client` qualified on `coax0 0.52.0`. |
| Coder | [`coder.json`](coder.json): first publication, from commit `7ba66df968`; offline route `coder-harness-memory` qualified on `coax0 0.52.0`. |
| Bitcoin | [`bitcoin.json`](bitcoin.json): latest publication, from commit `c466f49635`; offline route `bitcoin-crypto-settlement` qualified on `coax0 0.52.0`; live GitBook republish awaits owner credentials. |
| Everglade | Published natively via Coax Web SSR (`/books/everglade`); offline route `everglade-world-srd-dragon` qualified on `coax0 0.52.0`; GitBook space not yet provisioned. |
| OpenAgents | Published natively via Coax Web SSR (`/books/openagents`); offline route `openagents-app-economy` qualified on `coax0 0.52.0`; GitBook space not yet provisioned. |
| Reverse Engineering Claude Code (`claude-code`) | Published natively via Coax Web SSR (`/books/claude-code`); 25 `written` external-survey chapters checked by `psionic/tests/book.py --book claude-code`; GitBook space not yet provisioned. |

This table says nothing about whether an older live book exists. Existing
publication inventory checks and retained historical records keep their stated
scope. See [delivery](../DELIVERY.md#keep-publication-identity-separate) for
publishing and strict local/live receipt checks. No token belongs in a receipt.
