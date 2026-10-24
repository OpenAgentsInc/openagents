# Cooked entry and town packages

Implemented for issue #2. Medieval Town can open a bounded entry area, then load
the complete town while the entry keeps running. **Complete town** loads the
second cell; **Return to entry** releases its GPU resources. The same path runs
on the owned courtyard fixture.

## Cook and run

Extend the existing compiled-asset closure, without importing vendor source again:

```sh
python3 tools/assets/cook-cells.py \
  --source "$AETHYRFORGE_PRIVATE_ASSETS" \
  --out "$AETHYRFORGE_COOKED_ROOT" --entry-radius 10
AETHYRFORGE_STREAM_ROOT="$AETHYRFORGE_COOKED_ROOT" \
VITE_STREAM_PACKAGE_URL=/private-assets/stream-package.json npm run dev
```

Choose an output outside the repository and source assets. The dev server overlays
its URL directories over the original assets. Leave `VITE_STREAM_PACKAGE_URL`
unset to run the complete baseline. For the owned fixture, cook `public/starter`
with `--url-prefix /starter/ --entry-radius 4`, and run with
`VITE_WORLD_PROFILE=starter` and `/starter/stream-package.json`.

The descriptor binds ABI, cell bounds, scene mappings, inventories, hashes, and
sizes. Shared bytes are stored once through hash-addressed blobs and hardlinked
URL aliases. Files and descriptors publish only after validation; a changed
package needs a fresh output. The loader bounds concurrent work and memory,
checks descriptor and file hashes, refuses unlisted candidate reads, and rejects
missing, truncated, oversized, redirected, or incompatible resources.

## Activation and ownership

The entry has source geometry and collision ready before control is enabled.
Walking uses a capsule-margin boundary until the next cell is active. Preparing
an area cannot widen that boundary. Returning to entry requires the player to
be inside its bounds. Fly cameras can inspect unloaded space.

A candidate prepares on a separate offscreen canvas and includes current authored
objects and the last working shader. It binds to the live canvas only after it
is ready. The existing Rust kernel, player pose, gameplay progress, undo history,
and clock remain live. Druid destruction, collision ownership, spell selection,
and explicit lighting overrides also survive cell changes. Both lighting
scenarios must keep compatible gameplay and collision identities. Failure or cancellation discards the candidate; a late
candidate cannot replace a newer request. Retiring a renderer cannot unconfigure
the newly active display. The preview frame loop remains independent of UI state.

Verified CPU bytes stay cached for warm revisits. The typed `stream` action
`unload` calls `unloadInactive()` to release unused cache entries. Closing the
world cancels pending work and releases cache, renderers, and display resources.

This is an entry plus one aggregate full-town cell. Geometry and collision are
still fully resident. Entry batches exclude distant objects, unused material
maps are pruned, and optional source scenery/lighting effects are deferred.
The initial quality is explicitly `staged-flat-outside-entry`; the full cell
uses the original source scenes and restores final quality. This does not claim
arbitrary spatial geometry streaming or final-quality entry rendering.

## Evidence and measurement

Run `openagents browser run -- node tools/verify-stream.mjs` with
`AF_CAPTURE_DIR` outside the checkout, the streaming environment above, and
`AF_VERIFY_WORLD=medieval-town` for the private town. Druid-enabled runs also
require `AF_DRUID_FIXTURES` pointing to the matching private source-bound probes. It runs baseline and streamed
sessions sequentially, with fresh contexts and same-context reloads. The owned fixture passes 18 browser checks; the Druid-enabled town passes 20, including exact frozen final pixels,
real movement, progress, shader failure, unload, and revisit. The
[scrubbed results](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-2.json) retain measurements;
licensed captures and detailed traces stay private.

| Town measurement | Complete baseline | Entry |
| --- | ---: | ---: |
| Response-body bytes before first control | 876,231,081 | 344,485,200 |
| Runtime start to first reliable control, final cold run | 3,219 ms | 2,057 ms |
| Renderer-accounted GPU bytes at first control | 1,101,416,842 | 569,006,318 |

These are local functional runs on one shared Mac, not population estimates or
FPS evidence. Reload does not guarantee an HTTP cache hit. The cooked package's
gzip sizes are estimates; the table measures actual localhost response bodies.
The full-stage preparation peak accounts for about 1.67 GB across the old and new
renderers; verified CPU cache peaks near 636 MB. Device/driver/process memory,
image decoder peaks, and unrelated applications are outside those counters.

Runtime summaries separately expose scene-document decode time, aggregate
preparation, hash time, request bytes, cache/reservation high water, and core
buffer/material upload byte counters. Upload counts cover helper-created buffers,
ordinary material base levels, and planned layered-map uploads. They exclude
other passes, GPU mip generation, and physical driver transfers. The common
benchmark work in issue #7 can reuse these measurements without relabeling
unavailable values as measured zeroes.
