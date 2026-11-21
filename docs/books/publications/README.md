# Publication receipts

The publisher writes `BOOK.json` here by default after a successful real merge
and live inventory check. Keep each committed version in Git history. A failed
publication cannot replace a successful receipt.

| Book | Source/revision receipt introduced by this work |
| --- | --- |
| Coax | Not recorded; no real publication performed for the runner change. |
| Psionic | Not recorded; no real publication performed for the runner change. |
| OpenAgents Metaverse Engine | Not recorded; no real publication performed for the runner change. |
| Nostr | Not recorded for live GitBook (requires owner GitBook credentials); offline route `nostr-event-client` qualified 2026-11-20 (4/4 debug+release runs passed on `coax0 0.52.0`). |
| Coder | [`coder.json`](coder.json): first publication, from commit `7ba66df968`. |
| Bitcoin | [`bitcoin.json`](bitcoin.json): latest publication, from commit `c466f49635` (chapters 5–7); first from `329da2c61e`; full book structure (105 programs, 865 listings) verified 2026-11-20; live GitBook republish awaits owner credentials. |

This table says nothing about whether an older live book exists. Existing
publication inventory checks and retained historical records keep their stated
scope. See [delivery](../DELIVERY.md#keep-publication-identity-separate) for
publishing and strict local/live receipt checks. No token belongs in a receipt.
