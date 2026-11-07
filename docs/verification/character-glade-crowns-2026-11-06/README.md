# Crown, grass and slash artifact correction

The three cyan lines were authored wind-ribbon geometry. All 294 vertices and
864 indices were removed, and the material branch was removed. The binary
oracle requires zero former ribbon markers. `scene.png` is the checked final
1920×1080 view; `before.png` is the previous pushed pass at 14d7a9f947.

The crown normal bake fits eight local volumes to each source tree; each whole
four-corner leaf card uses one volume. It changes shading rather than replacing
source geometry or enlarging leaf cards. Three missing trees restore the right
orange layer and left woodland. Grass gains fuller height and a dark-root,
transmitted-tip response; nearby fog's blue veil is removed. Sky tint and rock
placement colors were compared against the supplied reference, with a flatter
path stone and a darker/wider central rock group. Random background scatter
was reduced to retain the 32 MiB pack bound and all original wardrobe bytes.

`colour-comparison.json` records masked channel distributions and sky samples.
These guided color correction; they do not establish registered image parity.
`pack-oracle.json` checks source grass topology, finite packing, zero ribbon
markers, normalized crown normals and original wardrobe preservation.
Desktop/phone GPU, numeric light readback, controls/persistence/pan, generated
shader and offline staging observations are retained with the final hashes.
RAF timings are headless pacing observations, not a GPU benchmark. Tree shape,
terrain composition and the reconstructed material response still differ from
the reference; no exact rendering equality is claimed.
