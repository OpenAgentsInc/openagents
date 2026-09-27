# Wizard Woods asset pack

`7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp` contains the original Ruins of Atlantis wizard, zombie, and
CommonTree_3 geometry in Verse's bounded geometry format. The plaza does not
embed, download, decode, or upload this pack. Explicit forest entry starts its
verified download; subsequent entries can use the content-addressed disk cache.

The source checkout is pinned to commit
`daeb5d0895270159ec8c18341b4adb84bf7a4346`. Each source input and the result has a
SHA-256 digest in [provenance.json](provenance.json). Regenerate it from a hydrated
source checkout with Python 3, NumPy, and Pillow:

```sh
python3 scripts/bake-verse-forest.py --source /path/to/ruinsofatlantis
```

The baker reads that checkout without modifying it. It refuses changed input
bytes and writes only the chosen output directory. The result is 6,629,578 bytes
and expands to about 12 MiB of face vertices. No original model is embedded in
the app, and no program or script is executed from a zone pack.

Each pack and checksum use the pack digest as their filename. Retain old pack
files when publishing a new revision, and append previous digests to the loader's
`FOREST_PACK_HISTORY` list. The published files keep older app versions working;
the cache list permits bounded cleanup of those recognized old local revisions.
Unknown cache files and symlinks are never cleanup candidates. Temporary files
in the reserved `.forest-<pid>-<counter>.part` namespace are eligible only after
24 hours; a current transfer has a 60-second deadline.

## Fidelity and format

The tree is normalized to 8 meters. The wizard and zombie are normalized to
1.8 meters from their original standing poses. The pack retains the wizard's
Still pose and 12 frames each from its Waiting animation, the zombie's Idle
animation, and the zombie's Walk animation. These poses come from the original
joint tracks and inverse bind matrices.

The baker samples the original diffuse textures into linear vertex colors and
applies fixed lighting. It subdivides transparent leaf cards and removes small
triangles below the source alpha threshold. This preserves a sampled silhouette
instead of rendering opaque rectangles. It is an approximation of the original
texture edges. Normal maps, live skeletal interpolation, and per-pixel textures
are not part of this first pack.

Version 1 starts with eight magic bytes, `VZP1\r\n\x1a\n`. Two meshes follow:
tree, then wizard Still. Three animations follow: wizard Waiting, zombie Idle,
and zombie Walk. Each mesh has a little-endian `u32` vertex count, then triangle
vertices containing three little-endian `f32` positions and three `u8` linear
color channels. Each animation has a little-endian `u16` frame count, an `f32`
frame duration, and its meshes. The decoder rejects extra data, invalid floats,
unsupported timing, oversized allocations, and any content identity mismatch.

## Source notices and provenance limits

[SOURCE_LICENSE](SOURCE_LICENSE) and [SOURCE_NOTICE](SOURCE_NOTICE) preserve the
source repository's Apache-2.0 license and notices. The source notice also names
SRD 5.2.1 and Noto Sans; neither rules text nor that font is in this geometry pack.

The tree appears in the source's Quaternius collection with an identical content
digest. The inspected wizard and zombie files, their initial import commits, and
the source notice do not identify their upstream authors or an asset-specific
license. Animation names that contain `mixamo.com` do not establish one. This
pack records the repository's provenance without inventing separate asset
permissions or claiming those attribution gaps are resolved. Retain this record
when reviewing or redistributing the pack.
