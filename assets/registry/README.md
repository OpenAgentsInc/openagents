# Shared Aethyr authoring registry

This is the portable, reviewable catalog for the complete Quaternius Fantasy
Outfits, Medieval Village and Stylized Nature Source kits, plus the Fantasy
Props Standard kit. Every original file
is inventoried. Blender artwork, license evidence and alternate exports remain
available; a world's runtime pack includes only its chosen collection.

The `dungeon` collection inventories the complete 48-model Modular Dungeon
archive published by Quaternius on [itch.io](https://quaternius.itch.io/lowpoly-modular-dungeon-pack).
The source snapshot came from the [creator's OpenGameArt upload](https://opengameart.org/content/lowpoly-modular-dungeon-pack);
the downloaded archive's SHA-256 is `bb9959ffd2c80b28bcd4ea574eaf4c9dd62a70fda4d61dbc5d6f5193a3f94630`.
The `ultimate-animals` collection contains the 12 animated glTF models from
the [Ultimate Animated Animals pack](https://quaternius.com/packs/ultimateanimatedanimals.html),
mirrored from [FreeModels at db3df04](https://github.com/agentkaer/FreeModels/tree/db3df04d1e4714298a09510b26fb6de6645138a2).
That snapshot contains glTF and license files, not the full original package's
other authoring formats. The owner's dungeon download matches the existing
complete `dungeon` snapshot file for file. The full original animal package is
also preserved as `ultimate-animals-source`, indexing its twelve animated glTF
models. `ultimate-rpg` and
`ultimate-food` inventory and index all 106 and 103 original Blender models,
respectively, alongside the alternative formats. These collections are
unqualified for runtime until a world explicitly packs and tests them.

`bestiary-dungeon` inventories the owner's Standard download and indexes its
two supplied monsters, Imp and Puglin. Its evidence is the Quaternius Asset
License v1.0, not CC0. Original files and GLB objects stay in the local shared
store; the portable registry contains hashes and metadata. The Source edition
advertises seven monsters; those extra models are not in the Standard download.

## Identity and storage

An authoring ID such as `quaternius/nature/birch-1` is a stable lookup key, not
Aethyr's `AssetId`. Runtime asset identity remains SHA-256 of actual bytes,
with the existing bounded, whole-pack admission in `aethyr/assets.coax`.

* `packages/NAME.json` pins the exact source snapshot.
* `packages/NAME.files.json` inventories every source file by relative path,
  byte size and SHA-256. The snapshot revision is the hash of this exact file.
* `packages/NAME.representations.jsonl` links alternate exports by name. These
  are candidates, explicitly **not** assertions of equivalent geometry.
  Unmatched files stay inventoried and are listed rather than dropped.
* `assets/NAME.jsonl` records logical assets and immutable revision definitions.
  A revision is SHA-256 of the exact compact `definition` JSON bytes, without
  the enclosing `id`/`revision` wrapper or a trailing newline.
* `collections/*.lock.json` pins each asset's exact revision. A changed revision
  requires regenerating and reviewing its consumer's lock.
* `recipes/` selects the source Blender files, grouping and export origin.
* `previews/` contains paged geometry contact sheets, not material/shader parity
  evidence. Previews deliberately show shape with neutral studio lighting.

The default local library is `~/.openagents/assets/`:

```
originals/<package revision>/inventory.json
originals/<package revision>/source/       read-only vendor snapshot
objects/<sha256>                         GLBs, revision JSON, exact SDK bridge
work/<openagents ID>/                    writable, packed Blender forks
cache/<package>/                         rebuildable observations and previews
catalog/                                 published copy of this portable catalog
catalog.previous/                        preceding published copy
```

Source snapshots retain alternate formats and engine-project archives. Their
embedded scripts are never run. `.DS_Store` is omitted from identity. Imports
reject symlinks and special files. Snapshot files are made read-only after
publication; this is protection against accidental edits, not a security sandbox.
Import is serialized by `import.lock`; an interrupted `.staging` directory is
left for inspection rather than silently reused. Byte and path limits fail
closed. Each external command has a deadline below 15 minutes.

## Using and rebuilding the library

Run the tool from an OpenAgents checkout. `STORE` below means an absolute path;
the examples use the shared default. No Desktop/Downloads path is committed.

```sh
coax/web/tool.sh verse/tools/assets/registry.coax import \
  "$HOME/Desktop/Quaternius/Modular Character Outfits - Fantasy[Source]" \
  fantasy "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax observe fantasy "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax index fantasy "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax representations fantasy
```

Use `village` and `nature` for the other two source packages and their checked-in
recipes. For a Standard glTF kit, `registry.coax gltfs PACKAGE Exports/glTF STORE`
indexes each model and its external dependencies without running package code. `observe` snapshots the SDK bridge into its build directory, runs
Blender in background/factory-startup mode with autoexec disabled and Python
errors fatal, then registers the observations and self-contained GLBs. The
revision pins the exact bridge bytes; the observation records Blender's version.
`index` retains that bridge in the content-addressed store. The checked Blender
executable path is macOS-specific; the registry formats are platform independent.
For Bestiary Standard, `registry.coax qal-glbs bestiary-dungeon
'Exports/GLB (Godot-Unreal)' STORE` records the QAL GLBs. CC0-only indexers
refuse this package rather than mislabel its contents.

GLBs are full-resolution **interchange artifacts**. They are not newly qualified
Aethyr runtime assets. They can repeat textures across different GLBs. Identical
whole artifacts deduplicate by hash; image-level runtime packing remains the
realm compiler's responsibility. Original textures, normal/ORM maps and Blender
material graphs are preserved in their source snapshots, independent of a
world's lighting profile.

Fantasy Props Standard registers all 94 supplied glTF models with their pinned
external buffers and texture dependencies. It contains no Blender source; the
registry preserves every supplied FBX, OBJ, glTF, texture and license file.

Nature's per-model Blender files define 116 multi-part assets; its combined
scene is retained as another source representation. Fantasy selects the 76
individual clothing meshes, excluding fitting bodies and rig widgets. Village
registers all 304 visual meshes and 1,327 collision parts. Each `UCX_` part links
to its owning visual asset. The village exports remove their display-scene grid
offset using the visual object's origin; collision parts use the same origin.
Original Blender coordinates and export origins remain in the observations.
Collision hull validity, building snap sockets and foliage wind behavior remain
unreviewed; the catalog does not invent physical or gameplay properties.

## Editing and remixing

```sh
coax/web/tool.sh verse/tools/assets/registry.coax fork \
  quaternius/nature/birch-1 openagents/examples/birch-study "$HOME/.openagents/assets"
# Edit ~/.openagents/assets/work/openagents/examples/birch-study/birch-study.blend
coax/web/tool.sh verse/tools/assets/registry.coax publish \
  openagents/examples/birch-study "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax index \
  openagents-examples-birch-study "$HOME/.openagents/assets"
```

`fork` never overwrites an existing workspace. It keeps the selected meshes and
required armature/parents, packs image dependencies and records the exact parent
revision. `publish` snapshots the editable project, exports it and produces a
new immutable revision under the OpenAgents ID. The included birch study is an
unchanged fork used to verify this flow, not new artwork. Original revisions
remain in `objects/` and package snapshots after a new revision is published.
The v1 CLI forks one parent; definitions carry an array for future explicit
multi-parent assemblies. Do not change parent history to point at "latest".

## Character Create

`collections/character-create.json` is the authored wardrobe definition: stable
asset IDs, labels, availability and preset membership. `registry.coax wardrobe`
resolves it against the imported catalog, writes the collection lock and
regenerates `everglade/scene/charactercreator-parts.coax`.

Its `legacy_slot` values are frozen CC2 compatibility keys, never source-array
positions. Do not reorder or reuse them. Retired slots stay reserved; a future
expanded save format needs a new version. Existing CC1/CC2 records and the
current runtime pack remain compatible. Runtime catalog readiness checks every
referenced mesh instead of a hardcoded asset count. Appearance choices remain
Coax; the Blender bridge performs only SDK inspection/export/rendering.

## Checks and local publication

```sh
coax/bin/coax test verse/tools/assets/registry.coax --only registry
coax/bin/coax test apps/openagents-web/everglade.coax --only create:
coax/web/tool.sh verse/tools/assets/registry.coax wardrobe --check
coax/web/tool.sh verse/tools/assets/registry.coax check "$HOME/.openagents/assets"
python3 verse/tools/assets/registry_oracle.py "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax sync "$HOME/.openagents/assets"
```

`verify SNAPSHOT` independently rehashes a source snapshot through the Coax
inventory reader. The Python oracle separately checks source/artifact hashes,
source dependency closure, GLB structure, normalized village origins, collision
owners, lineage and collection pins. `sync` publishes the portable local catalog;
it retains the previous catalog and never modifies originals or editable work.
No server, database, cloud account or runtime format change is required.

## Installed command

`~/.openagents/bin/asset-registry` is installed for this library. Outside an
OpenAgents checkout it runs against `~/.openagents/assets/catalog/`, with the
SDK bridge in `~/.openagents/assets/authoring/`. From a checkout it uses that
checkout's reviewable catalog. `show ID` prints a registered revision;
`source PACKAGE STORE` prints the pinned editable-source root. For example:

```sh
~/.openagents/bin/asset-registry show quaternius/nature/birch-1
~/.openagents/bin/asset-registry source village "$HOME/.openagents/assets"
```

Local publications update the shared catalog when run outside a checkout.
Bring reviewed catalog changes into a checkout before changing a shipped
world. `wardrobe` outside a checkout writes a generated table into the local
authoring directory, not into a shipped application. The self-contained
command remains usable after its build worktree is removed.

`selfcheck` exercises object publication with a temporary fixture: complete
bytes, an idempotent existing object, a mismatched incoming hash, and refusal
to silently repair a corrupted object. New objects are hashed in staging,
made read-only, then renamed into the store. Current object bytes are never
edited in place. GLB export files in `cache/` can be discarded after indexing;
the immutable copies remain in `objects/`.

## Reusable scene declarations

`scenes/character-glade.json` pins the reference image and registered Nature
revisions used by Character Create's 3D meadow. Scene placements and terrain
are Coax source, with separate runtime-pack and shader source hashes. This
scene declaration is `oa-scene-v1`; it does not change the mesh-definition or
runtime pack contracts. See `docs/verse/character-glade.md` for regeneration.

## Universal animation libraries

`ual1` and `ual2` pin the complete CC0 standard distributions, including
the Unity, root-motion, in-place and supporting files. Their 86 animation
records use stable `quaternius/ual1/...` and `quaternius/ual2/...` IDs.
Each record identifies its named animation and index in the immutable in-place
GLB; the full library object is shared by all its records. These records are
authoring sources, not a claim of runtime rig compatibility. Root-motion
exports remain in each complete source snapshot.

Rebuild the clip index from a verified snapshot with:

```sh
coax/web/tool.sh verse/tools/assets/registry.coax animations ual1 Unreal-Godot/UAL1_Standard.glb "$HOME/.openagents/assets"
coax/web/tool.sh verse/tools/assets/registry.coax animations ual2 Unreal-Godot/UAL2_Standard.glb "$HOME/.openagents/assets"
```

## Two-handed weapon clips

`openagents-two-hand-clips` is an OpenAgents original (CC0): six animation records under
`openagents/two-hand/...` for holding a greataxe or greatsword in both hands. For each weapon
there is an idle, a run and a cleave. The package snapshot holds the Blender source
(`two-hand-clips.blend`), the exported GLB, the authoring script, `report.json` and
`provenance.json`. Each record names its parent Universal Animation Library 1 clips by
revision: Idle_Loop or Jog_Fwd_Loop for the body, and Driving_Loop for the closed fingers.
`recipe_sha256` pins the script. The script keys the clips at 60 frames a second and fails if
the weapon's mesh comes within 8 cm of the head or neck, enters the torso, or leaves either
fist; `report.json` records the closest approach for each clip. To rebuild the source, run:

```sh
Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
  --python verse/tools/assets/two_hand_clips.py -- \
  ~/.openagents/assets/originals/e2adadc59702bc88770e299949d1f4c6790db1852133fd1ce374d2a0815537cd/source/Unreal-Godot/UAL1_Standard.glb OUT_DIR
```

Then import `OUT_DIR` with `License_Source.txt` and `provenance.json` beside it
(`registry.coax import OUT_DIR openagents-two-hand-clips "$HOME/.openagents/assets"`).

## Everglade's cat

`openagents-cat` records `openagents/beasts/cat`, Everglade's cat, an OpenAgents original (CC0).
No CC0 animated cat with a sit clip was found on 2026-11-09, so
`verse/tools/assets/cat_bake.py` builds one from primitives: a black and white tuxedo cat with a
round, wide head, a short white muzzle, green eyes, upright triangular ears, a white bib and paws
and a long black tail. The in-house package `openagents-cat` holds the script, `Cat.glb` (the
sit's first frame, the interchange artifact), `license.txt` and `PROVENANCE.md`; `recipe_sha256`
pins the script. The bake also writes the frames a zone's pack holds (`glade/cat0..63`: Idle,
Walk and Sit, `verse/tools/assets/cat_pack.coax`):

```sh
Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
  --python verse/tools/assets/cat_bake.py -- ~/.openagents/assets/cache/cat-e2
```

## External asset discovery

The [CodePawl asset index](https://huggingface.co/datasets/codepawl/asset-index)
is a separate, searchable discovery catalog. Its pinned source revision,
metadata SHA-256, expected byte count and row count are recorded in
[`external/codepawl-asset-index.json`](external/codepawl-asset-index.json).
The full upstream metadata is kept locally under
`~/.openagents/assets/discovery/codepawl/`, not committed or copied into the
registry's asset-definition or runtime-object namespaces. The dataset author
states that the index is theirs, but this snapshot does not include an explicit
reuse license for the index itself. Each result retains its original license,
credit, acquisition, NoAI, size and source fields as unverified upstream claims.
The snapshot has 37,668 rows from six sources: 22,696 Megascans, 4,434 Kenney,
3,451 Smithsonian, 2,917 ambientCG, 2,387 Poly Haven and 1,783 Quaternius.
24,185 rows have a non-null `scale_m`; the source of each measurement is also
retained. An index row is not an imported asset, a qualified GLB, or evidence
that the user has obtained a license to an asset.

```sh
# Fetch the exact pinned revision, validate its size, SHA-256 and every row,
# and atomically publish it for discovery. No upstream code is run.
~/.openagents/bin/asset-registry discover-sync
# Or use an already-downloaded metadata.jsonl (the same checks apply):
~/.openagents/bin/asset-registry discover-sync /path/to/metadata.jsonl

# JSON Lines: local registry matches followed by external matches.
~/.openagents/bin/asset-registry search 'wooden crate'
# Limit to a source; get the next page by skipping 20 matches.
~/.openagents/bin/asset-registry search 'rock' megascans 20
# Limit to records with an indexed scale between 0.5 and 1 metre, inclusive.
~/.openagents/bin/asset-registry search 'rock' megascans 0 0.5 1
```

Search is case-insensitive ASCII substring matching of all query words, against
local ID/name or the external metadata row. It emits at most 10 local and 20
external results per page, in catalog order. `local`, `megascans`, `kenney`,
`smithsonian`, `ambientcg`, `polyhaven` and `quaternius` are source filters;
an empty source (`''`) searches both. The offset applies independently to local
and external matches. A size range excludes rows with unknown `scale_m` and
local entries without comparable measurements. Every line identifies its
`origin`, and external rows with a known matching Quaternius source pack include
`related_local_collection`; this is a pack-level hint, not a claim that the
listed and local editions or mesh bytes are identical.

To update the snapshot, review the upstream dataset's new commit and metadata
file, update the lock's revision, immutable URL, SHA-256, byte count and row
count together, then run `discover-sync`. A failed validation leaves the current
snapshot intact. Ordinary registry `sync` publishes the lock and metadata about
the adapter, while leaving discovery data in its separate local directory.
Acquiring or converting a candidate remains a separate import workflow: obtain
it from its original provider, verify the actual terms and provenance, then
register the locally held source and derived artifacts with immutable identities.
For example, the index says Megascans are claimed through Fab, and does not
contain those assets. The upstream optimizer is untrusted downloaded code; it is
not run or installed by this adapter, and its output is not qualified for Aethyr.

### Selected 0 A.D. art

`0ad-selected` pins nine models from the 0 A.D. GLB dataset, revision
`051e4649defa445799f4cb89a8681e4e7908e61d`. The imported source snapshot
includes the dataset metadata, its README and Wildfire Games' license. The art
is attributed to Wildfire Games and licensed CC BY-SA 3.0; it is not CC0.
The collection contains Gaul and Briton buildings, rocks and mushrooms used in
Everglade Experimental. Its immutable source is available with:

```sh
coax/web/tool.sh verse/tools/assets/registry.coax source 0ad-selected ~/.openagents/assets
```

`verse/tools/assets/zero_ad_gltf.py` uses Blender's Python API to convert the
pinned engine GLBs to separate glTF files for the experimental scene packer.
These are derived files: retain the original snapshot and attribution when
rebuilding or sharing them.
