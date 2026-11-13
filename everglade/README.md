# Everglade

Everglade is an MMORPG built on the D&D 5E System Reference Document (SRD
5.2.1). It follows the SRD's rules faithfully and adapts them for action
combat: you aim, move and act in real time, and the SRD's rules decide the
outcome. It runs on the OpenAgents Metaverse Engine, is written in Coax and is a realm in
the OpenAgents Metaverse.

Up to 16 players share a room. When a room is full, the next player joins
another room. The number is `WIRE_SEATS` in
[`everglade-wire.coax`](../everglade/shared/everglade-wire.coax), which the
room server's `EG_SEATS` matches.

- To play, go to [everglade.world/play](https://everglade.world/play).
- For the system map, architecture and decision log, read [ARCHITECTURE.md](ARCHITECTURE.md) and [docs/decisions/](../docs/decisions/).
- To build it from scratch, read [Building Everglade](book/README.md).
- For system specs that aren't book chapters yet, see [Everglade design docs](docs/README.md),
  such as [Golemancy](docs/golemancy-profession-spec.md), the profession for commanding golems.

Everglade's code is moving into this folder from `apps/openagents-web/`. The
land, planting, ground and light are in [world/](world/). For what moves and when, see
[Where Everglade's code lives](book/README.md#where-everglades-code-lives).

## State and authority

Everglade's row in `metaverse-engine/editor/authority.coax` (#13431,
[what the kinds are](../docs/architecture/state-and-authority.md)):

| Kind | Who decides |
| --- | --- |
| Content (the land, content tables, edits) | The site's admins |
| Simulation (combat, movement, loot, travel) | The room, on the server; a page predicts and is corrected |
| Durability (the character record, items, coin, quests) | The item ledger and the character record |

An undo, a content rollback or a world load never writes a character's record
or the ledger (INVARIANTS.md, "Authoring never writes durable state").

Zones only wire shared features. [features.coax](features.coax) lists each
feature, the shared modules that own it and the features each zone uses;
`coax/web/tool.sh everglade-parity` fails on a new zone-local copy. See
[Everglade feature parity](../docs/metaverse/everglade-feature-parity.md).
