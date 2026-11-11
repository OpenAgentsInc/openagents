# Fast loads

Issue: [#13141](https://github.com/OpenAgentsInc/openagents/issues/13141).
Code: `coax/web/pack-fetch.js` and `coax/web/pack-worker.js` (a pack's bytes,
from a worker), `coax/web/layer-pool.js` (texture layers, from workers),
`coax/web/host.js` (`loadPack`, `layerTexture`, `loadMarks`),
`coax/web/serve.c` (large files from a child process).

A fresh load of a realm with a pack now takes about a third to a half of the
time it took. The pictures are the same. Nothing changes per realm: every page
that names a `pack:` gets it.

| Realm | Before | After |
| --- | --- | --- |
| `/worlds/everglade-experimental` (a 90 MB `glade.pack.gz`, 300 MB unzipped, 93 layers) | 4.5 to 5.6 s | 1.5 to 1.6 s |
| `/worlds/everglade2` (a 31 MB `everglade2.vpk3`, 62 MB image, 81 layers) | 5.3 to 6.0 s | 2.5 to 2.6 s |

The numbers are `load until ready` from `coax/web/tool.sh shot URL OUT.png
1280x720 5000 --times`: from the navigation to the first scene drawn with the
loading card gone. Measured 2026-11-10 on the Mac (18 cores, load average 18 to
27), Chrome headless, `coax/web/serve.sh --static`, five loads each, old and
new alternating.

## See where a load's time goes

The host records a mark at each step of a load. Read them from any page:

```sh
coax/web/tool.sh shot URL OUT.png 1280x720 --eval "JSON.stringify(coaxial.loadMarks())"
```

Each mark is a name and the milliseconds since the page began.

| Mark | When |
| --- | --- |
| `main` | The host script starts |
| `engine-installed` | The realm's Wasm module is compiled and started |
| `pack-asked` | The pack's download starts, in a worker |
| `shaders-start`, `shaders-done` | The page's shaders are fetched and its pipelines compiled |
| `gpu-started` | The GPU device, shaders and pipelines are ready |
| `pack-start` | The page is ready for the pack |
| `pack-bytes` | The worker hands over the whole unzipped pack |
| `pack-admitted` | The pack's SHA-256 is checked, for a realm that asks |
| `pack-buffers` | The vertex and index buffers are written to the GPU |
| `pack-layers` | Every colour layer is decoded, resized and on the GPU |
| `pack-delivered` | The realm has its catalog and the scene is bound |
| `scene-first-draw` | The first frame with the world in it is submitted |
| `scene-ready` | The GPU finished that frame |

## What took the time

Everglade Experimental before, in milliseconds (one load; the others are within
20%):

| Step | Before | After |
| --- | --- | --- |
| Engine, GPU device, shaders and pipelines | 330 | 330, with the pack coming in beside it |
| Download (local) | 100 to 160 | with the unzip |
| Unzip 92 MB to 300 MB | 780 to 880 | 770 to 880 from `pack-asked`, in a worker |
| Write 288 MB of vertex and index buffers | 45 to 60 | 45 to 60 |
| Decode, resize and upload 93 layers | 2,900 to 3,900 | 400 to 480 |
| First frame, and the GPU finishing it | 120 to 200 | 120 to 140 |

Shader and pipeline compile was never the cost: about 200 ms, already in
parallel.

**Layers were two thirds of the load.** Each layer is decoded, resized to the
texture's size, and resized again for each of its 10 or 11 mip levels. Chrome
does that work on the thread that calls `createImageBitmap`, about 30 ms a
layer. The page made two layers at a time, but both on its own thread, so the
93 layers ran one after another and held the thread for 3 s. More makers on the
same thread changed nothing: 2, 4, 8 and 16 all took 2.2 to 2.4 s.

**Everglade II waited behind its own first frame.** Its first frame plants the
woods (`verse/everglade2/plant.coax`: 194,000 samples of the land) and takes
1.8 to 1.9 s on the page's thread. The pack's 297 pages were fetched and
inflated on the same thread, so they waited for it, and then took 1 s more.

**The dev server sent one file at a time.** While `coax serve` sent a pack, the
page's shaders, fonts and sounds waited for the browser to read all of it.

## What changed

- **Layers are made in workers** (`layer-pool.js`). A worker makes the same
  calls with the same options as the page did, so a layer's pixels are the
  same: decoding a JPEG and two PNGs both ways gives no differing byte in 5.5
  million. Up to 12 layers are in the making at once, fewer on a machine with
  fewer cores, and never more than 192 MiB of bitmaps. On 18 cores, 93 layers
  at 1024 take 950 ms with 4 makers, 570 ms with 8, 460 ms with 12 and 440 ms
  with 16. Without workers, or when one fails, the page makes the layer itself.
- **A pack's bytes come from a worker** (`pack-fetch.js`, `pack-worker.js`).
  The download is unzipped as it arrives, into one buffer of the size the pack
  states, and the worker hands the buffer over without a copy. The page used
  to download the pack, join its chunks, unzip the copy and copy the answer. A
  `.vpk3` pack's pages are fetched, inflated and admitted by Coax in the same
  worker, with the existing reader (`pack-host.js` `loadPaged`). The page's own
  checks of the bytes are unchanged. Without module workers, the page loads
  the pack on its own thread.
- **The pack starts as soon as the engine is in**, about 100 ms into the page,
  beside the font, the GPU device and the shaders. It used to start after all
  of them, at about 330 ms.
- **`coax serve` sends a file over 4 MiB from a child process**, so the page's
  other files are answered at once.

Private packs (a broker session, or a local `/private/` pack) load as before.
So do pages on the host transport (`loadPackV1`) and pages with a render
package (`render.js` makes their layers).

## Do the pictures match?

Yes. The layers' pixels and the pack's bytes are the same (the SHA-256 of the
unzipped pack from `fetchWholePack` is the file's). The scenes move, so two
shots differ by where the grass is in its sway, as two old shots do:

| Page, with `?golden` | Old against old | New against new | Old against new |
| --- | --- | --- | --- |
| `/worlds/everglade-experimental` | 258 in 1,000 | 181 in 1,000 | 309 in 1,000 |
| `/worlds/everglade2` | 4 in 1,000 | 6 in 1,000 | 148 in 1,000 |

Everglade II's old and new shots differ by one thing besides the grass: the new
shot is taken 3 s sooner after the world starts, so the place's name ("THE
GLADE") is still on screen. Pass `--exact-wait` with a longer wait for a shot
without it.

## What is left

- **Everglade II's planting.** Its first frame still takes 1.8 to 1.9 s of the
  2.5 s, and the pack is in long before it ends. Planting in a worker, or
  baking the planting into the pack, would take most of that off.
- **Layers last in the pack.** A pack holds its vertices, then its indices,
  then its layers, so a gzipped pack's layers can't be decoded until all of it
  is unzipped. A pack written with its layers first would have them decoded by
  the time the vertices arrive: up to 0.4 s less for Everglade Experimental.
- **Everglade Experimental as a `.vpk3`.** Its pages would download and inflate
  several at once. The pack reader holds at most 64 MiB and this pack is
  300 MiB, so it stays a `.pack.gz` for now.
- **Experimental's layers are 512 a side and go up at 1024,** because three of
  its 93 are 1024. That is four times the pixels to resize and upload.
- **A second visit.** The browser's HTTP cache holds the gzipped pack; it is
  unzipped again on each visit (0.8 s, in the worker). A decoded copy in the
  Cache API would skip that for players. It would not help a shot, which
  starts with an empty browser context.
