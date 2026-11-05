# Room latency components and browser candidate

Issue #12840 remains open. This checkpoint qualifies the independent opt-in
latency codec, bounded server observation kernel and clock interval arithmetic.
It does not yet qualify the SDK timing adapters or a complete presentation
measurement. Ordinary sessions do not send this profile.

The server kernel has 346/346 mutation kills; the clock component has 105/105,
both root-only with dependency checks retained and zero survivors. The separate
JSON fixture profile passes. Counters distinguish refused, expired and encoded
observations; encoding a report does not prove its socket delivery. The cache
observation helper passes 100 checks. Raw results are in
`room-latency-components-2026-11-05/`.

`tools-check` was run and reports one newly landed upstream file without a Firm
reason, `coax/tests/capacity_refusals.py` (fbedfd141a). This checkpoint adds only
Coax tools and does not alter that compiler agent's file.

## Pinned browser candidate

Product inputs are exactly commit
`17a742c08c8d4ca3833348d09765178e97f4a55d`. A separate historical compiler root
`bfe8dfefb1` matches the **original**, unchanged c64 release receipt. The helper
`aethyr/host/prepare-historical-root.coax` verifies the original receipt before
and after overlaying separately pinned product inputs while protecting compiler
inputs. Original verification also passes after the page build. This is an
immutable consistency receipt, not a signature or qualification of today's
compiler sources.

The exact compiler cache is
`~/.cache/coax-release/c64ee8a16c2cfa20b729f7fb5b057e072990765a01372e9ff6f1d534c5a621c5/`.
The release page is **2,078,813 bytes**, SHA-256
`1e0abdfa8953cdd888f8646e14302e5213284f231339882f911cd0e652d4416d`.
Its six exact C/H cache keys and byte hashes are retained in the manifest. All
source-table joins against the frozen product inputs pass: carrier 188, step 30,
Grid 7, river 6, guard policy 19, rules 10; zero differing sources. Cache-key
observations and source-table joins are not original emission receipts.

Reproduce the preparation with the existing Coax helper, supplying an empty
output directory and the original cache above. In the prepared root run:

```
COAX=ORIGINAL_CACHE/coax COAX_JOBS=1 COAX_CHECK_JOBS=1 COAXIAL_RENDER=1 \
  coax/web/wasm.sh apps/openagents-web/everglade5e.coax \
  apps/openagents-web/everglade5e-face.c OUTPUT/everglade5e.wasm \
  apps/openagents-web/everglade-step.coax verse/grid.coax \
  verse/everglade/river.coax verse/everglade/guard-guide-policy.coax \
  apps/openagents-web/everglade5e-rules.coax
```

This used six exact cache hits and completed in 11.83 seconds. Generated C/H
bytes are frozen locally in `/tmp/room-latency-12840/page-c-units-exact/`; the
record retains their hashes rather than committing generated megabytes.

## Actual browser session

Headless muted Chrome 156 on this M5 Max joined native room artifact SHA-256
`11338370af66950d8b70b35e17bd9144572629d90ef5dc8758babe4f54bcf5e5`
from the capacity record. It received actual input ACK 1 and complete state
tick 3851, reported live network state and a ready scene, with no browser errors.
The Coax driver is `coax/web/room/latency-browser-check.coax`; all its browsers
and the two owned local servers were closed after the run.

```
coax/web/tool.sh coax/web/room/latency-browser-check.coax \
  'http://127.0.0.1:61242/realms/everglade/?relay=ws://localhost:61241&room=pair-latency-1' \
  OUTPUT/browser-session.json
```

The numbered local room matches the development ticket stand-in. The normal
page used its legacy WGSL browser rendering path. An independent screenshot
smoke reported the Apple Metal WebGPU adapter and a visible Everglade scene;
the screenshot path deliberately disables live networking. These are distinct
checks. Neither admits the shared render host, native Metal/Vulkan, default
host switching, GPU completion or physical display scanout. Live input-to-
application/render timing, throttling/fault checks and operator thresholds
remain for #12840.
