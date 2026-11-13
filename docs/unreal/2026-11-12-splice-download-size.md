# Splice's download size: Aethyr and the Unreal export

Measured 2026-11-12. One game, Splice, in two builds: the Aethyr page, and
the same game exported to Unreal Engine 5.9 and packaged for the web
([the exporter](2026-11-12-aethyr-to-unreal-exporter-spec.md)).

## Result

| Build | First visit downloads | Requests |
|---|---|---|
| Aethyr page | 443 KB | 9 |
| Unreal web build (Development) | 72.7 MB | 24 |

The Unreal build is about 164 times larger to download. Both are the whole
first visit: everything the browser fetches before the game can be played.

## The Aethyr page

Measured on the live page, `https://openagents.com/worlds/splice`, in a
fresh headless Chrome, from the browser's own resource timings.

| File | Over the network | Unpacked |
|---|---|---|
| The game (`splice.wasm`) | 78 KB | 177 KB |
| The page host script (`host.js`) | 119 KB | 410 KB |
| IBM Plex Mono font | 137 KB | 137 KB |
| Michroma font | 65 KB | 64 KB |
| Shared host code (`host-layers.wasm`) | 24 KB | 57 KB |
| The shader (`splice.wgsl`) | 13 KB | 61 KB |
| The page, icon and a sign-in check | 7 KB | 7 KB |
| **Total** | **443 KB** | **912 KB** |

- The two fonts are 202 KB, nearly half the download. The game, host and
  shader together are 234 KB.
- The game file holds only Splice: its world, rules and interface. The
  browser supplies drawing (WebGPU), input and text.
- The live page was the release from earlier that day. The build on `main`
  has later art changes and its game file is a few KB larger.

## The Unreal web build

Measured by the engine's web harness (`webrun.py`, its `load.json`) on the
packaged build of the exported project, served locally with brotli: 72.7 MB
in 24 requests before the game is ready.

The main files in the package:

| File | Size |
|---|---|
| The engine and project (`AethyrHost.wasm`), compressed | 42.5 MB |
| The same, unpacked | 208.7 MB |
| Cooked content (`AethyrHost-Wasm.ucas`) | 17.8 MB |
| Cooked content (`AethyrHost-Wasm.pak`) | 9.3 MB |
| Engine content (`global.ucas`) | 3.8 MB |
| The loader script (`AethyrHost.js`), compressed | 0.1 MB |

- The binary is the whole engine compiled to WebAssembly: renderer, physics,
  audio, animation, UI and the rest. Unreal compiled 765 units to make it.
  Splice's rules are about 1 MB of C source inside it.
- The package folder is 290 MB on disk, because it holds both compressed and
  unpacked copies.
- This is a Development build, with logging and debug support. A Shipping
  build is smaller. It has not been measured.
- The cooked content includes the test overrides (a cone and a beep) from
  the exporter's checks. They are small.

## What it means

- **Load time.** From this Mac's disk the Unreal build is ready in 2 to 5
  seconds. Over a real connection, 72.7 MB is a long wait, and a problem on
  phones. The Aethyr page is under half a megabyte.
- **Hosting cost.** [web-games-in-the-verse.md](web-games-in-the-verse.md)
  estimated about half a cent per first visit for a 70.5 MB Unreal build.
  This build is the same size class.
- **The size is the cost of the export, not of the game.** The rules are
  the same code in both builds. Exporting to Unreal buys Unreal's editor,
  asset pipeline and a build the jam accepts. The engine's size is what
  that costs.

## How to measure again

Aethyr, from the repository root:

```sh
coax/web/tool.sh shot https://openagents.com/worlds/splice /tmp/s.png 1280x800 6000 \
  --eval 'JSON.stringify(performance.getEntriesByType("navigation").concat(performance.getEntriesByType("resource")).map(e=>[e.name,e.transferSize,e.decodedBodySize]))'
```

Unreal: run any fixture in `metaverse-engine/export/unreal/web-fixtures.json` with the
engine's `webrun.py` (see `metaverse-engine/export/unreal/README.md`) and read `bytes`
and `requests` in the run's `load.json`.
