# Lagrange 1 sky and body data provenance

This directory holds the baked Earth, Moon, Milky Way, and bright-star data for
the Lagrange 1 zone of the Verse renderer (GitHub issue #9799). Every file is
derived from public-domain NASA imagery or a freely redistributable
astronomical catalog.

`scripts/bake-lagrange-sky.py` regenerates every file in this directory. It
downloads each source to `~/.cache/lagrange-sky`, checks the source against the
SHA-256 pinned in the script, and writes the outputs. With the same sources and
library versions, the outputs match the hashes in this file byte for byte.

- **Download date:** 2026-09-27 (all sources).
- **Bake environment:** Python 3.9.6, NumPy 2.0.2, Pillow 11.3.0, and zlib
  1.2.12 on macOS.

## Conventions

### Equirectangular projection (Earth and Moon)

`earth_day.png`, `earth_clouds.png`, `earth_water.png`, `moon_albedo.png`, and
`moon_height.png` use the equirectangular (plate carree) projection with north
up. For an image `W` pixels wide and `H` pixels tall:

- Column `x` covers longitude `[-180 + 360 * x / W, -180 + 360 * (x + 1) / W]`
  degrees. The left edge of the image is -180 degrees (the antimeridian), the
  horizontal center is 0 degrees (the prime meridian), and longitude increases
  eastward to the right.
- Row `y` covers latitude `[90 - 180 * (y + 1) / H, 90 - 180 * y / H]` degrees.
  The top edge is the north pole and the bottom edge is the south pole.
- Pixel centers sit at half-pixel offsets, so texture coordinates
  `u = (lon + 180) / 360` and `v = (90 - lat) / 180` sample the map directly.

Longitude is planetocentric east longitude for both bodies. The Moon's 0 degree
meridian is the mean Earth-facing meridian, so the Earth-facing near side is
at the image center.

### Sky projection (Milky Way)

`milky_way.png` keeps the NASA SVS Deep Star Maps convention: equirectangular in
J2000 equatorial coordinates, viewed from inside the celestial sphere.

- Declination `+90` degrees is the top edge and `-90` degrees is the bottom edge.
  Row `y` covers declination `[90 - 180 * (y + 1) / H, 90 - 180 * y / H]`.
- Right ascension 0h is at the horizontal center, and right ascension
  *increases to the left*. The left and right edges are both 12h.
- For right ascension `ra` in radians, `u = 0.5 - ra / (2 * pi)`, wrapped into
  `[0, 1)`. Equivalently, `ra = (0.5 - u) * 2 * pi` modulo `2 * pi`.
- Check: the galactic center (RA 17h45.6m, Dec -28.9 degrees) falls near pixel
  `(779, 338)` in the 1024 x 512 image, which is the brightest part of the band.

### Color encoding

Every 8-bit color or gray texture stores sRGB-encoded values. Sample it as an
`*Srgb` texture format, or decode it with the sRGB transfer function, to get
linear values. `earth_clouds.png` and `earth_water.png` are exceptions: they
store linear coverage fractions, so sample them as `Unorm`.

All downsampling averages pixel areas with Pillow's `BOX` filter. Color data is
averaged in linear light (sRGB decoded, averaged, and re-encoded). Rounding is
`floor(value * 255 + 0.5)`.

## Files

### `earth_day.png`

- **Content:** Earth surface color without clouds, including shaded
  bathymetry in the oceans.
- **Format:** 2048 x 1024, RGB8, sRGB-encoded, equirectangular.
- **Source product:** NASA Blue Marble Next Generation, July 2004, with
  topography and bathymetry (`world.topo.bathy.200407`), 5400 x 2700 JPEG.
- **Source URL:**
  <https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73751/world.topo.bathy.200407.3x5400x2700.jpg>
  (Visible Earth record 73751).
- **Source SHA-256:**
  `4f4240673a3a1b173d61b92ca4b07bac5fd17059ea5f725ba6da5a9c5386b7ba`
- **Processing:** Decode the sRGB values to linear light, area-average each
  channel to 2048 x 1024, and re-encode to sRGB.
- **License and credit:** Public domain (NASA imagery). Credit requested:
  "NASA Earth Observatory (Reto Stockli, NASA GSFC)".
- **Output SHA-256:**
  `d1f34286fe88d9bda8ec5746925415546403a4d0be0afbd11a4b466696b8a304`

### `earth_clouds.png`

- **Content:** Global cloud cover, 0 for clear sky and 255 for opaque cloud.
- **Format:** 2048 x 1024, L8, linear coverage, equirectangular.
- **Source product:** NASA Blue Marble clouds, combined cloud image
  (`cloud_combined_8192.tif`), 8192 x 4096 TIFF with three equal channels.
- **Source URL:**
  <https://eoimages.gsfc.nasa.gov/images/imagerecords/57000/57747/cloud_combined_8192.tif>
  (Visible Earth record 57747).
- **Source SHA-256:**
  `d137775d8966ab8d443fd5126dc6e7ad72072bc1ed50555c5818d221735daf0f`
- **Processing:** Take the red channel (all three channels are equal),
  normalize it to `[0, 1]`, and area-average it by 4 x 4 to 2048 x 1024.
- **License and credit:** Public domain (NASA imagery). Credit requested:
  "NASA Earth Observatory / NASA GSFC".
- **Output SHA-256:**
  `d1cf2be571099cd09f384c84e535a1915a815c441f0c041d29e9466c746a658e`

### `earth_water.png`

- **Content:** Water mask for oceans, seas, and large inland lakes. 255 is
  water and 0 is land. Values in between are coastline pixels that are
  partly water, stored as the covered fraction.
- **Format:** 1024 x 512, L8, linear coverage, equirectangular.
- **Source products:**
  - GEBCO bathymetry from the Blue Marble Next Generation collection
    (`gebco_08_rev_bath_21600x10800.png`), 21600 x 10800 PNG. Land is 255 and
    sea-floor depth maps to 0 to 254.
    <https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73963/gebco_08_rev_bath_21600x10800.png>
    (Visible Earth record 73963). SHA-256
    `b3a67076fccfbee73ebe437fd66a8ed4969fbc5ce949033b50675d43f379e403`.
  - Blue Marble land, ocean, and ice (`land_ocean_ice_8192.png`), 8192 x 4096
    PNG, where oceans and inland lakes are a saturated dark blue.
    <https://eoimages.gsfc.nasa.gov/images/imagerecords/57000/57730/land_ocean_ice_8192.png>
    (Visible Earth record 57730). SHA-256
    `aaddcd967a9f09fb2d7ef50ff452bebfcf10192c520465d5eb1ad8446c716e98`.
- **Processing:**
  1. Mark every GEBCO pixel below 255 as ocean. This includes ice-covered
     polar ocean and below-sea-level water bodies such as the Caspian Sea.
     GEBCO treats inland lakes above sea level as land.
  2. Mark every `land_ocean_ice` pixel with red at most 12, blue at least 45,
     and blue at least 30 above green as water. This adds lakes such as the
     Great Lakes, Lake Victoria, Lake Baikal, and Lake Titicaca.
  3. Area-average each mask to 1024 x 512 and keep the larger of the two
     fractions per pixel.
- **License and credit:** Public domain (NASA imagery). Credit requested:
  "NASA Earth Observatory / NASA GSFC; bathymetry from GEBCO".
- **Output SHA-256:**
  `454b38145ee147621e28f1ed82aed21189e9bb82570edca839d692726265d71e`

### `moon_albedo.png`

- **Content:** Lunar surface brightness from the LRO Wide Angle Camera natural
  color mosaic, reduced to luminance.
- **Format:** 2048 x 1024, L8, sRGB-encoded, equirectangular.
- **Source product:** NASA SVS CGI Moon Kit, 2025 color map
  (`lroc_color_16bit_srgb_4k.tif`), 4096 x 2048, 16-bit sRGB TIFF. It is
  adapted from the LROC WAC Hapke-normalized mosaic (643, 566, and 415 nm bands
  as red, green, and blue), white-balanced and range-adjusted for human vision,
  so its values are relative brightness, not physical normal albedo.
- **Source URL:**
  <https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_16bit_srgb_4k.tif>
  (<https://svs.gsfc.nasa.gov/4720>).
- **Source SHA-256:**
  `9731fa8af425b6c2f88f277ecca82bf8c603f3743894f64ed7b25c5bfefa22ff`
- **Processing:** Load with Pillow, which reduces the 16-bit samples to 8 bits
  per channel. Decode sRGB to linear light, area-average by 2 x 2 to
  2048 x 1024, compute Rec. 709 luminance
  (`0.2126 R + 0.7152 G + 0.0722 B`), and re-encode to sRGB.
- **Source tint:** The source is near-neutral and slightly warm. Over the whole
  2048 x 1024 map, the mean color is:
  - Linear RGB: `(0.531535, 0.504630, 0.485463)`.
  - sRGB8: `(193, 188, 185)`.
  - Luminance-normalized linear tint: `(1.044343, 0.991480, 0.953823)`.

  To restore the average color, decode the texel to linear and multiply it by
  the normalized tint.
- **License and credit:** Public domain (NASA data). Credit requested:
  "NASA's Scientific Visualization Studio; LRO LROC WAC (NASA/GSFC/Arizona
  State University)".
- **Output SHA-256:**
  `d290e3659a78817f79c3294d633dc194130ee0a4a55f1af87867e060df6482bc`

### `moon_height.png`

- **Content:** Lunar surface elevation relative to the LOLA reference sphere
  (radius 1,737,400 m).
- **Format:** 1024 x 512, L16 (16-bit grayscale PNG), equirectangular.
- **Scale:** `height_m = value * 0.5 - 10000`. One unit is 0.5 m and the value
  20000 is the reference radius. Values in this file span about -8,518 m to
  +10,007 m (area-averaged; the source spans -8,879 m to +10,504 m).
- **Source product:** NASA SVS CGI Moon Kit displacement map, LOLA gridded
  elevation at 4 pixels per degree (`ldem_4.tif`), 1440 x 720, 32-bit float
  kilometers relative to 1737.4 km.
- **Source URL:**
  <https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/ldem_4.tif>
  (<https://svs.gsfc.nasa.gov/4720>).
- **Source SHA-256:**
  `330afa2556a86fd05ac6ba2f912f246600fdade35de2a0d90593d50d07b01b65`
- **Processing:** Convert kilometers to meters, area-average to 1024 x 512,
  and store `round((height_m + 10000) / 0.5)` as an unsigned 16-bit integer.
  This matches the SVS `ldem_*_uint.tif` encoding.
- **License and credit:** Public domain (NASA data). Credit requested:
  "NASA's Scientific Visualization Studio; LRO LOLA (NASA/GSFC)".
- **Output SHA-256:**
  `46f33456b437783ecafe0b49b72b01ef0c9e504f198cd7ad1e922aaf52b736e3`

### `milky_way.png`

- **Content:** Diffuse Milky Way and faint-star background with the bright
  stars removed.
- **Format:** 1024 x 512, RGB8, sRGB-encoded, J2000 equatorial sky projection
  (see [Sky projection](#sky-projection-milky-way)).
- **Source product:** NASA SVS Deep Star Maps 2020, Milky Way background in
  celestial coordinates (`milkyway_2020_4k.exr`), 4096 x 2048 OpenEXR, half
  float, linear color, ZIP compression. This SVS product omits the Hipparcos
  and Tycho stars, so no median filter is needed. Draw bright stars from
  `stars.bin` on top.
- **Source URL:**
  <https://svs.gsfc.nasa.gov/vis/a000000/a004800/a004851/milkyway_2020_4k.exr>
  (<https://svs.gsfc.nasa.gov/4851>).
- **Source SHA-256:**
  `2eb802d6e68d170b410f766c7fec07f7518619f6b6708fdc81e9302d93e74fdb`
- **Processing:** Decode the EXR with the script's built-in ZIP scanline
  reader, area-average each linear channel by 4 x 4 to 1024 x 512, multiply by
  an exposure scale of 1.0 (the source peaks at 1.0, so nothing clips), and
  encode to sRGB. To recover source-linear values, decode the texel from sRGB
  and divide by 1.0.
- **License and credit:** NASA imagery, public domain. Credit requested:
  "NASA/Goddard Space Flight Center Scientific Visualization Studio. Gaia DR2:
  ESA/Gaia/DPAC."
- **Output SHA-256:**
  `dd37bf780fe2beedfb2d4614e899d9077ea5f26142180cf7267a02be37a94268`

### `stars.bin`

- **Content:** Every star in the Yale Bright Star Catalogue, 5th revised
  edition, with a valid J2000 position and visual magnitude: 9,096 of the
  9,110 entries. The 14 entries without a position are novae, clusters, and
  other non-stellar objects that were removed from later catalogs.
- **Source product:** Bright Star Catalogue, 5th Revised Ed. (Hoffleit and
  Warren, 1991), CDS catalog V/50, `catalog.gz`.
- **Source URL:** <https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz>
  (<https://cdsarc.cds.unistra.fr/viz-bin/cat/V/50>).
- **Source SHA-256:**
  `3dc44b1e90be8fbe5bcc7656032560f51275f985c7e3f783c9028e1838ec7bed`
- **Format:** Little-endian binary.

  | Offset | Type | Field |
  | --- | --- | --- |
  | 0 | 8 bytes | Magic `LGSTARS1` (ASCII) |
  | 8 | `u32` | Star count `N` (9096) |
  | 12 + 16 * i | `f32` | Right ascension, J2000, radians, `[0, 2 * pi)` |
  | 16 + 16 * i | `f32` | Declination, J2000, radians, `[-pi / 2, pi / 2]` |
  | 20 + 16 * i | `f32` | Visual magnitude V |
  | 24 + 16 * i | `f32` | B-V color index; 0.65 when the catalog has none |

  The file size is `12 + 16 * N` bytes (145,548). Records are sorted by
  magnitude, brightest first, with ties broken by HR number. The first
  records are Sirius (HR 2491, V -1.46), Canopus (HR 2326, V -0.72), Arcturus
  (HR 5340, V -0.04), Rigil Kentaurus (HR 5459, V -0.01), and Vega
  (HR 7001, V 0.03). 310 records use the 0.65 B-V fallback.
- **Processing:** Parse the fixed-width fields RAh, RAm, RAs (bytes 76-83),
  DE-, DEd, DEm, DEs (bytes 84-90), Vmag (bytes 103-107), and B-V
  (bytes 110-114). Positions are catalog J2000 values at epoch 2000.0; the
  script applies no proper motion.
- **License and credit:** The catalog is freely redistributable through the
  NASA Astronomical Data Center and CDS. Credit: "Hoffleit, D. and Warren,
  W. H. Jr., The Bright Star Catalogue, 5th Revised Ed., 1991; CDS/ADC catalog
  V/50".
- **Output SHA-256:**
  `82a2d10f4f58ff1e21ee9e8a3e6d0e0ab33faa582fcac65fd038d35d3014c5ee`
