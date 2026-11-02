# WebGPU diagnostic probes

The six raster/depth/filter calibration tools share one Coax runner:

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/webgpu-probe.coax \
  depth-plane /path/to/emitted-probe.wgsl /tmp/depth-plane.json
```

| Mode | Authored Coax shader | Observations |
| --- | --- | ---: |
| `raster` | `render-raster-probe.coax` | 6 |
| `raster-guard` | `render-raster-guard-probe.coax` | 6 |
| `depth-plane` | `render-depth-plane-probe.coax` | 12 |
| `depth-left` | `render-depth-left-envelope-probe.coax` | 30 |
| `depth-guard` | `render-depth-guard-envelope-probe.coax` | 30 |
| `filter-precision` | `render-filter-precision-probe.coax` | 14 |

`webgpu-probe-policy.coax` owns every case, uniform, sample position, expected
count, finite-value check and alpha check. `webgpu-probe.coax` owns CDP, the
temporary loopback HTTP origin, fingerprints, the bounded completion poll and
cleanup. Chrome stays headless and muted. Both the origin child and the owned
Chrome group are reaped when the command finishes.

`webgpu-probe-effects.js` runs only inside that page. It creates the supplied
WebGPU descriptors, uploads the supplied values and returns shader messages,
errors, numeric RGBA observations and raw binary32 words. It decides no verdict
and carries no authored numerical fixture. The original result fields remain;
`raw_words` adds the physical readback words, including signed-zero bits.

Run the pure checks and changed policy's focused mutation gate with:

```sh
"$COAX" test aethyr/tests/webgpu-probe-policy.coax --library
"$COAX" test aethyr/tests/webgpu-probe-policy.coax --library --release
"$COAX" mutate aethyr/tests/webgpu-probe-policy.coax --library --root-only --jobs 4
```

These tools record backend observations. They neither select a universal
rasterization rule nor change a renderer tolerance or exclusion mask. The
[port qualification](../../docs/verification/aethyr-webgpu-probe-port-2026-11-02.json)
retains the original drivers, observer-only raw-word variants, exact six-profile
parity, negative controls and the earlier bounded-transport failure.

`webgpu-session.coax` supplies the common bounded CDP/origin transport.
The four texture/ring/sampling successors and their same-input evidence are
described in `PRIMITIVE_WEBGPU.md`.
