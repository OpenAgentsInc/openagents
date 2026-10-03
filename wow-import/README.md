# Private WoW asset import

This Rust 1.98.1 tool reads the owner's 1.12.1 install through Benilla's pinned,
Bevy-free format readers. It writes a private Verse data pack with indexed
textured surfaces, skeletons, animation keys, and dungeon prop placements.

```sh
cd wow-import
CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1" cargo run --locked -- \
  "$HOME/work/Stonetavern-Classic-1.12.1-v1.8/Data" \
  "$HOME/wow-gym/verse-assets"
```

The import resolves map 289 through its WDT and ADT placement, rather than
assuming the dungeon is a global WMO. It includes Scholomance, nearby props,
displays 10691 and 11157, a composited human adventurer, and bow display 20723.
Model positions and animation tracks retain source yards and Z-up axes;
placements convert them into Verse meters. Texture files carry SHA-256 digests.
The pack carries no shaders, scripts, network endpoints, or credentials.

The reviewed import contains 117 models, 76 textures, and 113 placed props.
This importer is scoped to the ritual chamber. Multiplicative legacy sheen
passes are omitted; the owned renderer supplies illumination instead of baked
vertex lighting. Texture animation, liquid rendering, arbitrary character
outfits, and a complete terrain-streaming importer remain future extensions.
Keep generated packs and client assets outside the repository.

The importer also extracts `Fonts\FRIZQT__.TTF`, the Classic nameplate border,
and the status-bar texture into the private output directory. The Verse capture
loads these assets for proportional outlined text and textured health bars.
To refresh only these UI assets without importing the chamber again, append
`--ui-only` to the importer command. The font and textures remain outside Git.
