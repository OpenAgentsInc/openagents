# Texture GPU memory, block compression and phone tiers

GitHub #13014, part of #12998 (learning from Solstice's streaming renderer).
Data, screenshots and quality tables: `docs/verification/textures-2026-11-07/`.

## The audit

`coax/web/tool.sh texture-audit BASE_URL --world SLUG [--query Q]` opens a realm in
headless Chrome twice: as a desktop (1920x1080, dpr 1, fine pointer) and as a phone
(390x844, dpr 3, mobile, touch, so `(pointer: coarse)` holds). A script that runs
before the page wraps `GPUDevice.createTexture` and `GPUTexture.destroy`, so the
ledger holds every live texture with its format, size, layers, mips and usage. Coax
turns the rows into bytes. A material is an 8-bit or block texture that is uploaded to
(pack layers, atlases); a target is everything else (depth, light buffers, shadow map).
The tool also writes the page as it was measured (`TIER.png`).

Measured on 2026-11-07 on the Mac's Apple GPU (the adapter offers BC, ASTC and ETC2):

| Realm | Tier | Materials before | Materials after | Targets |
| --- | --- | ---: | ---: | ---: |
| Everglade (`/worlds/everglade`) | desktop | 70.7 MiB | 70.7 MiB | 81.8 MiB |
| Everglade | phone | 70.7 MiB | 70.7 MiB | 14.4 MiB |
| Everglade, Link materials (`?pack=everglade-link`) | desktop | 636.0 MiB | 636.0 MiB | 81.8 MiB |
| Everglade, Link materials | phone | **636.0 MiB** | **70.7 MiB** | 14.4 MiB |
| Character Create (`/worlds/character-creator`) | desktop | 346.6 MiB | 346.6 MiB | 58.0 MiB |
| Character Create | phone | 90.6 MiB | 90.6 MiB | 10.7 MiB |

Almost all of it is one array: Everglade's 53 layers at 512 (70.7 MiB with mips), at
1536 with Link's materials (636 MiB), and Character Create's 64 layers at 1024 on a
desktop (341 MiB) and 512 on a narrow screen (85 MiB). Before this change a phone got
Link's materials at 1536, the same 636 MiB as a desktop.

The same materials as block formats, at the same sizes (the audit's projection):

| Realm | Tier | RGBA8 | BC7 or ASTC 4x4 | ASTC 6x6 |
| --- | --- | ---: | ---: | ---: |
| Everglade | both | 70.7 MiB | 17.7 MiB | 8.0 MiB |
| Everglade, Link materials | desktop | 636.0 MiB | 159.0 MiB | 70.7 MiB |
| Character Create | desktop | 346.6 MiB | 86.6 MiB | 38.8 MiB |
| Character Create | phone | 90.6 MiB | 22.6 MiB | 10.3 MiB |

## What changed

- **Phone size tier.** `aethyr/render/texture-format.coax` caps a phone's RGBA8 layer
  array at 512 a side (Solstice's phone rule) and a block-compressed one at 1024. The
  browser host asks the shared image reactor (`host-layers.wasm`) for the side before it
  makes the array. Link's materials on a phone drop from 636 MiB to 70.7 MiB. A desktop
  keeps the side its realm declares. Before and after shots of the phone:
  `shots/everglade-link-phone-{before,after}.jpg` and a full-resolution crop of the
  character, `shots/everglade-link-phone-crop-before-after.jpg`. At a phone's render
  scale the two read the same.
- **Format choice per device.** The host now asks for every block-compression feature the
  adapter has (`texture-compression-bc`, `-astc`, `-etc2`), and Coax picks the format from
  those and from the encodings the pack carries: a phone prefers ASTC (6x6 for colour,
  4x4 for normals), a desktop BC (BC7 colour, BC5 normals). No pack carries encoded
  layers yet, so every device still gets RGBA8; that is the fallback, and it is also what
  happens when the reactor can't load.
- **The encoder.** `aethyr/render/texture-encode.coax` writes BC7 (mode 6), BC4/BC5 and
  ASTC 4x4 and 6x6 (one partition, LDR, binary ranges; colour, colour with alpha, and
  dual-plane normals with X in `.r` and Y in `.a`). `aethyr/tools/texture-encode.coax`
  runs it offline (`encode`) and checks it (`check`) against the independent decoders
  `aethyr/tools/ocarina-texture-core.coax` already uses: the pinned bcdec runner and ARM
  astcenc 5.7.1.

## Encoder quality

`check` decodes every block file with the reference decoder (astcenc `-ds` for colour,
`-dl` for normals) and compares it with the source. Colour is PSNR over RGB. Normals are
the angle between the normal the source's X and Y give and the one the decoded X and Y
give, both with Z rebuilt as the shader rebuilds it. It also decodes every block with the
encoder's own decoder: in every run below, no texel differs by more than one level, so
the reference decoders read every bitstream as written.

Colour layers (PSNR over RGB, mean over every texel of the set; `quality/*-summary.tsv`):

| Set | Layers | Texels | BC7 | ASTC 4x4 | ASTC 6x6 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Everglade pack | 38 | 7,405,568 | 41.64 dB | 41.18 dB | 35.70 dB |
| Character Create pack (`glade.pack`) | 64 | 15,794,176 | 39.33 dB | 38.91 dB | 33.37 dB |

Normal maps (17 Ocarina `Nrm` maps and a 512 synthetic sphere field, 9,109,504 texels):

| Format | PSNR | Mean angle error | Worst texel |
| --- | ---: | ---: | ---: |
| BC5 | 45.06 dB | 0.55° | 29.8° |
| ASTC 4x4 normal (dual plane) | 38.82 dB | 1.19° | 50.8° |
| ASTC 6x6 normal (dual plane) | 31.07 dB | 2.90° | 105.3° |

On the smooth synthetic map alone the mean errors are 0.06° (BC5), 0.13° (ASTC 4x4) and
0.21° (ASTC 6x6). The worst texels sit where a map's normal turns sharply inside one block;
a 6x6 block with one colour line can't follow it, so ASTC 6x6 normals are for distant or
low-detail surfaces only, and the phone pick uses ASTC 4x4 for normals.

Our BC7 uses mode 6 only and our ASTC one partition with no trit or quint ranges, so
blocks with two colour lines lose more than a full encoder would (astcenc and the good BC7
encoders search partitions); ASTC 6x6 colour shows it most.

## Frame time

`coax/web/tool.sh flyover` on the Mac (1440x900, dpr 1, the Everglade path), the commit's
parent against this change (`flyover/`):

| Run | Frames over 33 ms | GPU p50 | GPU p95 |
| --- | ---: | ---: | ---: |
| Everglade, before | 3 | 26.7 ms | 38.6 ms |
| Everglade, after | 2 | 27.3 ms | 37.6 ms |
| Everglade, Link materials, before | 15 | 28.6 ms | 40.4 ms |
| Everglade, Link materials, after | 0 | 20.9 ms | 32.0 ms |

The flyover runs as a desktop, so the phone tier does not apply and every layer is still
RGBA8 at the declared side: the change adds nothing to a desktop frame. Other agents' builds
shared the machine, so the gap in the Link run is load, not this change.

## Not done yet

- **Tiered texture arrays.** Solstice keeps three tiers (128 for every layer in the first
  download, 512 on demand, a few full-size slots under LRU) fed by the GPU request array.
  That array is Valley Lab step 6 (#13008), which is not on `main`, so the tiers wait for
  it. `tf_side` is the hook they extend.
- **Encoded layers in packs.** Packs still carry JPEG and PNG layers. Shipping BC7 or ASTC
  needs a pack entry kind for block layers (the range-addressable packs, #12990), an
  upload path that writes blocks with `writeTexture`, and the normal shaders reading
  ASTC normals from `.ra`. The format pick is ready for it: set the pack's carried bits.
- **The renderer's layer paths.** Realms drawn through `coax/web/render.js`
  (`makeLayersEffect`, canonical layers) do not take the phone tier yet; Everglade and
  Character Create use `host.js`'s path, which does.
- **The encoder's search under mutation.** Root-only mutation of
  `aethyr/render/texture-encode.coax` kills 1513 of 1681 mutants; 167 survive. Most sit in
  the search (the principal axis, the least-squares refit, which grid and range to try),
  where a changed search still finds blocks of the same error on the pinned fixtures. The
  bitstream half (packing, decoding, the blue-contract decode) is pinned by the golden
  blocks and the reference decoders. The tool (`aethyr/tools/texture-encode.coax`) and the
  format pick have no survivors.
