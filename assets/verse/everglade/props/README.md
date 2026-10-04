# Fantasy Props MegaKit subset

This subset comes from Quaternius's Fantasy Props MegaKit (Standard),
downloaded by the owner from <https://quaternius.com>. The kit's
`License_Standard.txt` is kept here as `license.txt` and declares CC0 1.0.
`manifest.json` pins the creator, license, package, and the SHA-256 of every
admitted file, both as it shipped in the kit (`originals`) and as committed
(`files`), with the transform for each changed file.

The Everglade spec proposes this kit as an addition to the two the owner
named, because neither of them has desks, shelves, or lecterns. Eight of these
models are also admitted, at full resolution with their normal and ORM maps,
under `assets/verse/props/quaternius/` for the summoning lair; this set is
Everglade's own, so the zone pack does not depend on the lair's sources.

The Everglade pack compiler (`verse::zones::everglade_pack::compile`) reads
these files, refuses any byte that differs from the manifest, and builds the
pinned zone pack. Only base-color textures are admitted, because the zone
renderer samples base color only.

## Admitted models

| Models | Studio station |
| --- | --- |
| `Workbench`, `Table_Large` | Desks |
| `Bookcase_2`, `BookStand`, `Scroll_1`, `Book_Stack_1` | Library and podium |
| `Cauldron`, `CandleStick_Triple` | Oracle, at the hearth |
| `Dummy`, `Anvil` | Proving ground |
| `Crate_Metal` | Merge station (the rigged chest is not static, so the strongroom uses a metal crate) |
| `Bench`, `Stool` | Lounge |
| `Banner_2`, `Lantern_Wall` | Podium and walls |

## Admitted textures

The base-color images those models use: `T_Trim_Furniture_BaseColor`,
`T_Trim_Props_BaseColor`, `T_Trim_Metal_BaseColor`, `T_Trim_Cloth_BaseColor`,
and `T_Page_Noise`.

The furniture and props trim sheets keep 1,024 pixels on their long edge; the
rest are at most 512 pixels, as the compiler's texture policy requires. Images
larger than their edge were downscaled with macOS `sips -Z`, as `transforms`
records, to keep the committed sources and pack within 30 MB.
