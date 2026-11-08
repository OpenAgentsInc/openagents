# Range-addressable packs (VPK3) — #13001

Recorded 2026-11-07 in worktree `oa-stream`, based on `e25db6962f`.

## What landed

- `coax/lib/image/deflate.coax`: a DEFLATE encoder in Coax (LZ77 over a
  32 KiB window with hash chains, dynamic Huffman or stored blocks,
  length-limited codes). Every check round-trips through the existing
  `inflate.coax`. On Everglade's pack in 256 KiB pages it writes 4,389,719
  bytes, against 4,372,855 for Python's zlib at level 6 (0.4% larger).
- `aethyr/forge/page-pack.coax` (`#authority`): the VPK3 format. A 96-byte
  header, a manifest of 64-byte page records (stored offset and length,
  image offset, decoded length, codec, root flag, SHA-256 of the decoded
  bytes), then pages deflated one at a time. The header carries the
  manifest's SHA-256 and the image's. The image is the VPK1 pack byte for
  byte, so VPK1 readers and existing pins stay valid, and VPK1 is unchanged.
  `ppk_admit` takes a page only when its size and SHA-256 are its record's
  (the `aethyr/assets.coax` identities); `ppk_roots_ready` says when the
  roots are held.
- `aethyr/host/page-stream.coax` and `coax/web/pack-face.c`: the browser pack
  reactor admits each page in place in its image arena. With the roots held,
  the image becomes a finished stream for the existing complete-pack
  admission.
- `coax/web/pack-host.js`: `loadPaged` fetches the header and manifest in one
  range, then runs of pages (roots first, up to 2 MiB each), 4 ranges at once
  by default (`?ranges=N` in development). Each range is retried up to three
  times with back-off. A server that answers 200 sends the file once and
  ranges are cut from it. A changed `ETag` stops the load. Pages are inflated
  with `DecompressionStream("deflate-raw")`, and a refused page is fetched
  again. The result is published when the roots are held; other pages follow
  (`result.complete`). `coax/web/host.js` loads any `pack:` URL ending in
  `.vpk3` this way.
- `aethyr/tools/page-pack.coax` writes and checks VPK3 files.
  `apps/openagents-web/build.sh` writes `everglade2.vpk3` from the tracked
  `everglade2.pack` (the `.vpk3` is a build product, ignored by Git).
  Everglade II and Character Studio now name the `.vpk3`.

## Measured: Everglade II, first image at 50 Mbit/s and 30 ms

Apple M5 Max, Chrome 157 headless, `hardware apple metal-3`. Server:
`coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:9371
--throttle-mbps 50 --latency-ms 30` (from #13000). Measured with
`coax/web/tool.sh first-image URL 120000`. One observation each.

| Pack | Ranges at once | First image | Bytes before it | Network quiet |
| --- | ---: | ---: | ---: | ---: |
| [`everglade2.pack`, whole](everglade2-pack.json) | 1 | 22,313 ms | 43,634,844 | 19,178 ms |
| [`everglade2.vpk3`](everglade2-vpk3-1-range.json) | 1 | 14,019 ms | 22,834,767 | 11,691 ms |
| [`everglade2.vpk3`](everglade2-vpk3-4-ranges.json) | 4 | 8,012 ms | 22,834,767 | 4,817 ms |

Deflating pages halves the download (42,581,440 bytes to 21,683,844, 217
pages, 186 deflated). The one-range row is the fair comparison with a single
50 Mbit/s link. The four-range row gains more because the #13000 server
shapes each connection on its own, so four connections get four times the
bandwidth. On a real link that is shared, expect a result between the two rows.

## What this does not yet do

- **Everglade II's first image still needs the whole image.** Its pack holds
  one vertex buffer, one index buffer and texture layers that the realm
  uploads together, so every page is a root. Deferring pages works:
  `ppk_ready` and `result.complete` publish the roots first, and sounds
  follow when they are written as non-roots (`--defer sounds`, the default).
  Geometry that streams on demand comes with the cluster DAG pages (#13015,
  #13008), which use this format.
- **The 69-entry pack takes the whole-image path.** Everglade II's pack has 69 entries
  under VPK1 magic, so the Coax VPK1 reader (48 entries) refuses it, as on
  `main`. This realm takes host.js's whole-image path: every page is still
  admitted by its SHA-256 in Coax, then the browser's `unpack` reads the image
  as before.

## Checks

- `coax/bin/coax test coax/lib/image/deflate.coax --library`: 11 pass. These
  cover empty, one-byte and run inputs, text, more than 64 KiB of
  incompressible bytes (stored blocks), distance 32,768, every length and
  distance code across many blocks, and code-length limits.
- `coax/bin/coax test aethyr/forge/page-pack.coax --library`: 51 pass (with
  the loaded pack, deflate and inflate checks). These cover the round trip,
  roots-first order, and refusal of edited, short, repeated and out-of-range
  pages. Every header and manifest byte the format fixes is checked by
  flipping it.
- `coax/bin/coax test aethyr/host/page-stream.coax --library`: 170 pass.
- `coax/web/tool.sh aethyr/tools/page-pack.coax`: Everglade's and Everglade
  II's packs write and read back byte for byte.
- `coax/web/tool.sh tools-check`: 0 new files without a reason.
- Mutation was not run (lean verification, owner direction, 2026-11-07).

## Identities

| File | SHA-256 |
| --- | --- |
| `everglade2.pack` | `726ad0be7661b314488e71245483d060931ce6aae9877b49772ba0d377f607a7` |
| `everglade2.vpk3` | `8242ceb45e095f09b1cae906a5b3d098e9ad3c7d85cef01c765d8c6301f5ba0d` |
| `build/everglade2.wasm` | `8f118fa4a23db314bd0fcce0dcd3d88a8751f9c984fe8d23db38e07ad383014b` |
| `build/host-packs.wasm` | `b8404d1515ed635add5b6e15a0091989759984f9d06154d1e08f91a440632837` |
