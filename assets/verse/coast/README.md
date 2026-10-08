# Original coast pack

The 487,425-byte VTP is pinned by SHA-256 and retained under that digest.
It contains 36 static models at three detail levels and four animated
wildlife rigs. The client embeds the small public file and decodes it on
coast entry. Replacing the world releases its decoded static resources.
All content is original Reference-mode work, dedicated under CC0-1.0;
no downloaded geometry, Fab content, or licensed textures are included.

Rebuild original sources with `scripts/blender/build_coast.py`, reduce them
with `kit_lod.py --generated`, and review `coast_preview.py --lod` raster
previews. Admit them with `coast_admit.py`. The source manifests bind each
admitted file to its generated GLB. Per-model records and the full model
inventory are in `../generated/coast/` and `../generated/PROVENANCE.md`.

Land changes through `openagents artifact submit coast-pack`. The queue
compiles the admitted sources independently of the generated file, writes
the digest file, updates the pin, and checks the zone. It never starts a
lighting bake. Use a build lease and an external target directory.

The lighthouse lens has an explicit runtime emissive material; admission
removes only its unsupported glTF emissive-strength extension. C3 owns
moving boats; C4 owns wildlife behavior and reef animation; C6 owns climate
and live surf-spray/sea-mist emitters. Their original sprite sheets and
encoding records are retained beside the generators' other outputs.
