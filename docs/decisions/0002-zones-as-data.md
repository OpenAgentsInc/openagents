# 0002. Zones as data

Date: 2026-11-11
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), sections G1 and G7; [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md)

## Context

Each Everglade v1 zone was wired as a bespoke Coax client module (`everglade-*.coax`) with per-zone glue (`everglade/zone-*.coax`), procedural terrain seeds, and separate room wiring. Adding or editing a zone required duplicating client/server plumbing and maintaining feature-parity ratchets across zones, while unpaged multi-hundred-megabyte packs blocked streaming.

## Decision

Represent every zone as authored data loaded by one shared zone runtime on both client and room server:

- A zone is a manifest and paged `.pack` bundle (`metaverse-engine/world/`, `coax/web/api/verse_packs.c`) describing terrain tiles, static collision hulls, spawn tables, props, portals, and lighting.
- Both the client realm (`metaverse-engine/world/cells.coax`) and the room shard load the same collision and navigation data for a zone.
- Adding a zone adds data and content records, not a new client page or room binary.

## Consequences

- New zones, dungeons, and towns ship as pack revisions without forking client or room code.
- Client prediction and server movement/line-of-sight step against identical authored geometry.
- Packs must stay budgeted and cell-paged (`metaverse-engine/world/cells.coax`) so browser and native hosts stream within memory budgets.
