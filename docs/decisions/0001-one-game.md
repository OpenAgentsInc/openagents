# 0001. One Everglade game

Date: 2026-11-11
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), section G1; [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md)

## Context

Four Everglade lines grew side by side: Everglade v1 (`everglade/`, `apps/openagents-web/everglade-*.coax`), Everglade II (`verse/everglade2/`, `apps/openagents-web/everglade2-*.coax`), Everglade Nights Experimental (`docs/metaverse/everglade-nights-experimental.md`), and the native desktop client (`apps/everglade-desktop/`). The room server (`coax/web/room/`) carried two parallel Everglade paths inside `room-everglade.coax` and `room-e2-*.coax` with differing cast, line-of-sight, and persistence rules, splitting player state and doubling parity bugs.

## Decision

Ship one Everglade game, built on the Everglade II foundation and one authoritative room path:

- Browser and native desktop clients run the same Coax realm through `aethyr-host-v1`.
- One authoritative room loop serves all Everglade zones; parallel `everglade` and `everglade2` room code paths merge rather than grow apart.
- Everglade v1 zones are ported as zone data into the single runtime and retired once migrated; Experimental stays frozen as an archive.

## Consequences

- Gameplay rules, line of sight, cooldowns, checkpoints, and replication are implemented and audited once.
- `everglade-parity` shrinks from guarding duplicated zone glue to checking zone manifests until the v1 split is removed.
- Client, server, and host changes target one shared architecture documented in [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md).
