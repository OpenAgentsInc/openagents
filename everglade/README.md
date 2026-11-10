# Everglade

Everglade is an MMORPG built on the D&D 5E System Reference Document (SRD
5.2.1). It follows the SRD's rules faithfully and adapts them for action
combat: you aim, move and act in real time, and the SRD's rules decide the
outcome. It runs on the Aethyr engine, is written in Coax and is a realm in
the Verse.

- To play, go to [everglade.world/play](https://everglade.world/play).
- To build it from scratch, read [Building Everglade](book/README.md).
- For system specs that aren't book chapters yet, see [Everglade design docs](docs/README.md),
  such as [Golemancy](docs/golemancy-profession-spec.md), the profession for commanding golems.

Everglade's code is moving into this folder from `verse/everglade2/` and
`apps/openagents-web/`. For what moves and when, see
[Where Everglade's code lives](book/README.md#where-everglades-code-lives).

Zones only wire shared features. [features.coax](features.coax) lists each
feature, the shared modules that own it and the features each zone uses;
`coax/web/tool.sh everglade-parity` fails on a new zone-local copy. See
[Everglade feature parity](../docs/verse/everglade-feature-parity.md).
