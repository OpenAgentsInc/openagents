# Everglade shadow receiver cost (#13019)

The extra cost is in shadow receiving, outside the caster pass. The controlled
receiver comparison changes only the colour pass's uniform binding: the caster
draws, tile scheduling, atlas, scene and shader stay the same. Selecting the old
receiver removes most of the observed whole-frame difference. Clearing instead
of loading the atlas does not remove a comparable cost.

This resolves an investigation, not a performance improvement. The production
shadow shader and its image quality are unchanged. A cheaper receiver that
preserves the tile coverage and filtering still needs separate implementation
and validation. These experiments do not identify a particular Metal instruction
as the cause, and do not establish that uniform reads are free.

## Reproduce

Build and serve Everglade normally, then run from the repository root:

```sh
coax/web/tool.sh shadow-pair http://127.0.0.1:9365 map.json 3240 map
coax/web/tool.sh shadow-pair-report map.json
coax/web/tool.sh shadow-pair http://127.0.0.1:9365 receiver.json 3240 receiver
coax/web/tool.sh shadow-pair http://127.0.0.1:9365 receiver-one.json 3240 receiver-one
coax/web/tool.sh shadow-pair http://127.0.0.1:9365 load.json 3240 load
```

Run these sequentially. The tool launches two fresh muted headless hardware
Chrome profiles at 1440×900, DPR 1, high quality, clear weather, clock 0.45 and
epoch 1794009600000. It freezes the 54-second Everglade flyover path in the tool
as a versioned experiment, advances each simulation at 60 Hz, and waits for GPU
completion before advancing the other. Pair order alternates AB/BA. This is an
offline GPU cost comparison, **not a live 60 FPS qualification**. Both camera
clocks must match within 1 ms, both rasters must be full size, and every admitted
sample must contain a submitted nonempty world draw and one fresh timestamp
readback with a progressing GPU end timestamp.

The compiled shader is hashed at `createShaderModule`, rather than inferred
from a filename. Everglade currently serves **everglade5e.wgsl**; editing only
everglade.wgsl does not test this page. Both source hashes must agree except in
explicit `shader` mode. That mode accepts an optional local `/build/*.wgsl`
replacement on the left for controlled shader comparisons.

| Kind | Left | Right | Interpretation |
| --- | --- | --- | --- |
| map | Real `?shadowmap` producer and receiver | Real tiles | Actual complete implementations; each retains its own correct atlas |
| receiver | Tile producer, old receiver selected only for colour draws | Ordinary tiles | Receiver work isolated from caster scheduling and atlas handling |
| receiver-one | Same as receiver, both filters forced to one tap | Tiles, one tap | Projection/selection and one compare remain; normal filter cost removed |
| load | Tiles, depth-only pass clears whole atlas | Tiles, ordinary depth load | Test preservation overhead independently of receiver code |

The last three are deliberately invalid **image** controls. An old receiver
cannot interpret a tile atlas, and clearing the whole atlas destroys retained
tiles. Their timings isolate work; their pictures must never be treated as
visual parity, a quality-preserving optimization, or a shipping option. Uniform
offsets are pinned to the measured Everglade profile (2532 floats; cascade count
2432; tap count 1169), not a generic host ABI.

Pass timestamps can overlap on this GPU. Whole-frame GPU time is the first
pass's start to the last pass's end. Do not sum pass durations or subtract them
as if passes execute serially.

## Evidence and limits

Measured source: `d61f754a28`, Apple M5 Max, macOS 26.4, Chrome
157.0.8103.39, WebGPU `hardware apple metal-3`. Other work shared the GPU. The
owner explicitly requested continuing diagnostics under that load. Absolute
times and outliers are therefore workload-dependent; alternating order reduces
order bias but cannot eliminate interference. Signed per-camera differences
are reported alongside both order medians. No quiet-machine or FPS claim is
made.

Measured artifacts (SHA-256):

```text
everglade5e.wgsl  1bb26c1974fb5daea0a9385dbd72db56b842c25856489578cd0e0fc3d24a0620
everglade5e.wasm  24b41b4c1394de82e934a39822ef14a0bcaaf48186c6e1f932d78858c45f234d
coax compiler    52402aa1520ff7f267ce7c44b3d3dfa8e9dacc868bf7d84d95cdf95a0586d7f1
```

The raw `shadow-pair-v2` JSON receipts and text reports accompany this note.
Each row is `[frame, ...left, ...right]`, with eleven values per side:
camera milliseconds, width, height, whole GPU microseconds, shadow
microseconds, pass count, summed pass microseconds, nonempty world draw count,
submitted flag, readbacks during the step, GPU end-timestamp delta nanoseconds.
Only `complete:true` receipts with all rows admitted can be reported.

The complete implementation comparison (`map.json`, 3,240 pairs) measured:

| Whole GPU time | Old map | Tiles |
| --- | ---: | ---: |
| p50 | 14.626 ms | 16.039 ms |
| p95 | 31.310 ms | 34.377 ms |
| Shadow pass p50 | 0.727 ms | 0.515 ms |

The signed per-camera tile-minus-map median was **1.011 ms**, mean 1.608 ms,
p95 10.951 ms. AB and BA medians were 1.004 and 1.026 ms. Both camera clocks
advanced from 17 to 54,000 ms. This reproduces a receiver-side regression but
does not reproduce a universal 2 ms constant; the issue's original back-to-back
runs shared a fluctuating GPU too.

The receiver-only comparison (`receiver.json`, 3,240 pairs) reproduced that
difference with the tile caster and atlas unchanged: tile-minus-old-receiver
median **1.035 ms**, mean 1.512 ms, AB 1.014 ms and BA 1.075 ms. Whole GPU
p50 was 14.461 versus 15.655 ms; shadow p50 was effectively unchanged at
0.467 versus 0.465 ms. The unchanged shader hash and equal caster work isolate
the cost to the selected receiver path, rather than depth-atlas preservation.

The source explains the additional work being tested: `eg_map_lit`'s old path
projects into a 60 m map and returns fully lit outside it before filtering.
`eg_casc_lit` tests successive tile projections and their filter margins, then
filters a valid tile over coverage extending to 360 m. The same tap count does
not imply the same number of filtered fragments or the same projection work.
Both projections are camera-centred; this is not a player-centred versus
camera-centred comparison. Reducing coverage or disabling retained tiles would
change the picture, so neither is presented as a fix.

With both receivers forced to one tap (`receiver-one.json`, 3,240 pairs), the
paired median fell to **0.678 ms**, mean 0.846 ms; AB 0.752 ms and BA 0.607 ms.
Whole GPU p50 was 14.473 versus 15.362 ms, shadow p50 0.484 versus 0.491 ms.
Thus extra filtering contributes, while projection/selection and the coverage
of the remaining compare still have a cost. These are separate runs under
changing load: subtracting their medians is not an exact additive breakdown.

The atlas control (`load.json`, 3,240 pairs) measured only **0.059 ms** median
load-minus-clear, mean 0.053 ms; AB 0.017 ms and BA 0.103 ms. Whole GPU p50
was 13.109 versus 13.149 ms, shadow p50 0.436 versus 0.440 ms. Keeping the
atlas's depth contents therefore does not account for the approximately 1 ms
receiver difference on this profile. This does not assert zero depth-memory
cost, or generalize the measurement to another GPU.

Earlier exploratory captures localized the same receiver cost, but lacked the
final fresh-draw/readback admission; they are not the receipts used here.
Initial experiments against the unused shader filename were discarded.

After rebasing onto `9d01dbe34e`, a separate 30-pair compatibility smoke
(`rebased-smoke.json`) passed with the current shader hash
`2a0b7867df47bf683930283ceed0b161001b02e9461d4f7a27a2bf728241606b` on both
pages. It is not part of the full-flight measurements above and its short,
loaded timings are not a new performance conclusion.

## Verification

The observation policy's checks were written before its implementation:
101 mutants killed, zero survivors. The report's 87 checks passed, including
signed quantiles and the observation boundary. JavaScript syntax checks passed.
The frozen-flight check passed. `tools-check` admitted all new tool files with
zero missing reasons. Commands and logs:

```sh
coax/bin/coax test coax/web/shadow-pair-policy.coax --library
coax/bin/coax mutate coax/web/shadow-pair-policy.coax --library
coax/bin/coax test coax/web/shadow-pair-report.coax --library
coax/bin/coax test coax/web/shadow-pair.coax --library --only 'paired shadow experiment'
node --check coax/web/shadow-pair-host.js
node --check coax/web/host.js
coax/web/tool.sh tools-check
```

Before clip tracing was added, a real capture correctly refused its first pair
because it had no observed world-draw/submission facts. Clip tracing now records
the actual draw facts and wall-clock intervals; it never substitutes the fixed
simulation step for an observed frame interval. Normal animation-frame tracing
uses the same finishing function.

The timestamp observer reads the host's mapped timestamp bytes without changing
them. The host requests that readback before the clip step awaits queue
completion; see the [WebGPU promise ordering contract](https://www.w3.org/TR/webgpu/#promise-ordering).
The per-step read count and increasing timestamp independently reject stale
results.
