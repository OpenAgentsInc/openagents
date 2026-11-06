# Exact Valley source assets and the first private scan baseline

The clean local `ValleyoftheAncient2` Unreal project contains enough embedded
AssetImportData to recover original Quixel IDs without launching Unreal or
executing project code. This is source lineage, not proof that a current Fab
tier is byte-identical to the artist's imported or edited mesh.

`unreal-inventory.json` records 317 Megascans mesh packages, 299 with import
metadata found in their first 262,144 bytes, and 124 unique ID candidates.
281 mesh packages have positive exact package references in the first
262,144 bytes of Ancient World's 13,801 external actor packages. 247 actor
packages exceed that prefix. Counts are lower bounds on actor-package
references, not scene instance totals or a complete Unreal package parse.
This does not inventory every texture, material, skeletal mesh or custom asset.
The report stores source basenames and MD5s, never artists' original full paths.

`fab-matches.json` records eleven exact delimited Quixel IDs found in current
Fab glTF archive filenames, their listings and observed source licence prices.
Examples include CanyonSandstone `uk4sehs` -> Desert Western Rock Small 11,
RockSandstone_049 `umcjabuva` -> Desert Western Patch Ground Gravel Coarse 01,
NatureRock2_03 `uk5qbct` -> Desert Western Rock Medium 10, and QuarryCliff_00
`ubhvccfda` -> Quarry Cliff. Matching names alone are insufficient.

Two matching sources had free Standard licence tiers and working anonymous
GET download endpoints on 2026-11-05:

| Quixel ID | Fab source | Lab vertices | Lab triangles |
| --- | --- | ---: | ---: |
| `uk4cdch` | [Desert Western Rock Medium 08](https://www.fab.com/listings/11700737-077c-416b-a005-53d0b5e1e285) | 6,780 | 2,994 |
| `ubhvccfda` | [Quarry Cliff](https://www.fab.com/listings/c47f6376-20c4-4631-a6d6-82f626652691) | 1,949 | 3,099 |

The CanyonSandstone download-info endpoint returned HTTP 403, requiring login
for the paid listing. No purchase, login, entitlement change or payment was
performed. Some other matches offer a free **UEFN reference** licence while
charging for source files; that is not counted as a free source download.
Pricing and availability are observations on this date, not permanent promises.
The [Quixel announcement](https://quixel.com/news/update-megascans-are-going-free-again)
describes a staged free rollout. Recheck the remaining IDs as it progresses.

## Pipeline and scope

All shipped behavior, inventory/catalog/download tools, glTF decoding,
quaternion/unit conversion, JPEG decoding and vertex-color sampling are Coax.
C changes transport the existing bounded same-origin HTTP effect only.
The independent Python/Pillow oracle recomputes transformations and color
samples; it is tooling, never shipped.

The downloader performs GETs only for the two pinned free file IDs, fetches
fresh expiring links without logging them, downloads bounded 512 KiB ranges,
checks the complete archive SHA-256 and atomically renames its `.part` file.
It refuses changed source bytes and paid/failed responses. The catalogue
matcher is separately bounded to three public candidates per ID.

The native importer accepts a deliberately narrow glTF profile: one buffer,
node and triangle primitive; float positions, normals and UVs; uint16 indices;
positive uniform scale and quaternion rotation; baseline JPEG. No extension,
sparse, interleaved, matrix, translation, hierarchy or compressed geometry
support is claimed. It applies source centimetres-to-metres conversion,
recentres X/Z and puts the source's minimum Y on the floor. Normals are rotated
and normalized. Base color is sampled nearest-neighbor at each vertex's
repeating UV; this is an explicit vertex-color approximation, not textured PBR.
ORM, tangent-space normals, texture streaming and general scan LOD are future
work. The source tier has fewer triangles than the original high-detail FBX.

`0 SCANS` / `#scans` is an isolated baseline. `L` switches the two sources;
`V` cycles lit/albedo/normals/edges/depth; `R` retries. Each download must match
its compiled SHA-256, structural checks, finite colors, indexed mesh checks
and the shared asset registry before the candidate replaces the retained scan.
A bad or missing replacement retains the previous scan. A first missing asset
shows an explicit local-file message. Stale replies after leaving the station
are ignored. Cameras retain metric dimensions and fit the 14.24 m cliff.
The original rock/reducer/field stations continue to use their original asset;
no arbitrary imported mesh is passed off as a supported structured grid parent.

The declared arena budget increases together from 7 MB to 9.5 MB for bounded
raw/hex transport and live/candidate scan storage. Actual arena is 8,604,520
bytes. GPU mesh/index buffers, submitted-vertex cap and 160 KB Wasm cap remain
within their previous limits. The private scan station is not scene placement,
triangle collision, Nanite parity, streaming world cells or full valley completion.

## Local bytes and reproduction

Licensed source archives, decoded source and derived mesh bytes stay under
`~/.openagents/aethyrforge/valley-lab/source/{uk4cdch,ubhvccfda}/`.
`download-manifest.json` commits only identities, provenance and hashes.
The ignored `apps/openagents-web/assets/valley/scans/` directory exposes the
private hex transport files to the local preview. No raw/derived licensed
asset bytes or signed download links are committed. A clean checkout without
this private cache still runs every original station and shows the scan
station's missing-file state.

Run from an implementation worktree, with explicit private output paths:

```sh
coax/web/tool.sh aethyr/tools/valley-source-inventory.coax CONTENT_ROOT ANCIENT_WORLD_ACTOR_ROOT OUTPUT_JSON
coax/web/tool.sh aethyr/tools/valley-fab-match.coax OUTPUT_JSON uk4cdch ubhvccfda
coax/web/tool.sh aethyr/tools/valley-fab-pull.coax uk4cdch PRIVATE_DIR/low.zip
coax/web/tool.sh aethyr/tools/valley-fab-pull.coax ubhvccfda PRIVATE_CLIFF_DIR/low.zip
```

Extract only the archive's named glTF, bin and `Textures/T_ID_1K_B.jpg` data
members (not scripts). Then, for each source:

```sh
coax/web/tool.sh aethyr/tools/valley-scan-import.coax SOURCE.gltf SOURCE.bin BASECOLOR.jpg PRIVATE_DIR/mesh.vsm apps/openagents-web/assets/valley/scans/ID.vsm.hex
python3 aethyr/tools/valley_scan_oracle.py PRIVATE_SOURCE_ROOT
```

The last optional importer output is text hex because the existing browser
HTTP inbox contract transports UTF-8. The parser checks canonical decoded
binary bytes, not the hex file's hash. It accepts at most 8,192 vertices and
10,922 triangles. New source versions require an explicit manifest/hash update.

## Verification

- Existing application suite: 491/491 passed before the final parser helper
  expansion; the changed scan checks were then rerun separately.
- Scan parser: root-only mutation, 171 mutants killed, zero survivors, no
  equivalent annotations. Its dependency checks run in the baseline.
- Independent oracle: every original index exact; every vertex position and
  normal agrees within 0.00001 m / 0.000001; every sampled vertex color agrees
  with independent Pillow decoding within float rounding. See `oracle.txt`.
- Actual desktop/phone muted headless WebGPU: both source counts, five distinct
  GPU diagnostic crops, corrupt/missing replacement retention, retry, stale
  response after leaving, identical state and scan pixels after GPU loss.
- Existing walk/controller, indexed geometry, parents/LOD/pages, adjacent groups
  and deterministic field browser journeys pass. The larger arena is included
  in their declared budget check.
- Reusable Coax downloader fetched and SHA-verified both archives. Public
  catalogue and inventory helper checks pass. Tools policy check passes.

See the adjacent text logs, manifest and screenshots for the actual observations.
