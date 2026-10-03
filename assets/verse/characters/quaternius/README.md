# Universal characters

Creator: Quaternius. These retained assets come from the downloaded Standard
editions of Universal Base Characters, Modular Character Outfits – Fantasy,
and Universal Animation Library 2. Each pack grants CC0 1.0; see the three
license files in this directory. No Blizzard assets are included.

The portable glTF exports retain the male and female Superhero base bodies,
all four Ranger and Peasant outfits, the modular clothing parts, rigged hair,
and the non-root-motion animation library. The Standard base pack supplies
Superhero bodies rather than separate regular heads. The Rust importer extracts
head and neck triangles and retargets them onto the outfit's named skeleton.
Covered body vertices are removed, as the outfit README requires.

License and README text uses normalized line endings. Image references are normalized to content-addressed files in `textures/`.
Duplicate PNGs are stored once. `source-repairs.json` records three broken image
filenames repaired against the supplied PNGs. Geometry, skin weights, inverse
binds, and animation keyframes retain their supplied data. `manifest.json`
records SHA-256 digests of the retained source files. GPU compilation uses
bounded 1024-pixel base-color textures; normal and roughness/metallic source
images remain available for a future material pipeline.

Run the native scene from the repository root:

```sh
CARGO_TARGET_DIR=~/work/openagents-target-agent1 cargo run -p verse \
  --features imported-desktop --example verse_play -- --appearance female-ranger
```

Available player appearances: `male-ranger` (default), `female-ranger`,
`male-peasant`, `female-peasant`, `superhero-male`, and `superhero-female`.
NPCs alternate the four outfitted appearances. Claude uses an enlarged, red-tinted male Ranger outfit. `--greybox` selects the earlier procedural character fixture.
The chamber, spell effects, UI graphics, bow, and arrows remain original.

`verse::imported::characters::import` imports another retained modular part or
hair mesh. `compose` binds its named joints onto an existing rig. `retarget_clip`
adds any named animation from `animations.glb` under a caller-selected state ID;
`animations.json` lists all 43 available clips. The default scene maps locomotion,
combat idle, blocking, casting, and gesture states to that library and blends
transitions in local skeletal space. The Standard library has no dedicated
run, bow, death, or magic-casting clips. Walk/cast/block mappings use the supplied
carry, throw, and shield clips. Archery uses authored two-bone arm poses, and
death uses an authored root fall that holds its final pose. No chat input drives
these animations or the cinematic dialogue.

The engine uses rest transforms and inverse-bind matrices, following the
[Khronos glTF skinning contract](https://github.com/KhronosGroup/glTF-Tutorials/blob/main/gltfTutorial/gltfTutorial_020_Skins.md).
Animation retargeting matches joint names and carries local motion relative to
the source and target rest poses. This is a shared-rig integration, not a general
retargeter for unrelated humanoid skeletons.
