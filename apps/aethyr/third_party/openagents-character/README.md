# OpenAgents character port

The Everglade/Grove character data reader and movement animation semantics are
adapted from public OpenAgents Verse at commit
[`83fe8b7f6b3cd437ae9b7af213cf9f04875dd793`](https://github.com/OpenAgentsInc/openagents/tree/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793):

- `crates/verse-zone-everglade/src/zones/everglade_pack/format.rs`
- `crates/verse-zone-everglade/src/zones/everglade/player.rs`
- `crates/verse-content/src/compiler/characters.rs`

OpenAgents is copyright OpenAgents contributors, Apache License 2.0. The full
license is retained in [the shared OpenAgents notice](../openagents-camera/LICENSE).
The Python importer and TypeScript animation/WebGPU pass are an Aethyrforge
reimplementation of these public contracts. No private sibling implementation
is included.

The selected character is Quaternius's Universal male Ranger composition, the
implemented player in both Everglade and the Grove druid demo. Its component
meshes, textures, and animation sources use CC0 1.0. Source license text and
file hashes remain in OpenAgents's
[`assets/verse/characters/quaternius`](https://github.com/OpenAgentsInc/openagents/tree/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/assets/verse/characters/quaternius).
Prepared art and source packs are local private inputs, excluded from this
repository. The source-pack SHA-256 and source commit accompany the prepared
manifest.

The retained source license notices are [Universal Base Characters](base-license.txt),
[Modular Character Outfits – Fantasy](outfits-license.txt), and
[Universal Animation Library](animations-license.txt). These texts come from the
public source revision above. Browser releases include them alongside the
OpenAgents port notices in `THIRD_PARTY_LICENSES.txt`.
