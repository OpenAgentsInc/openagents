# Everglade graphics through Coax's Metal writer — #12867

Every Everglade render entry pair emits Metal Shading Language directly from
the portable checked Coax tree used by WGSL and SPIR-V. This qualifies the
shader backend and an offscreen SDK executor. The interactive Metal realm host
is #12868; making it the desktop default belongs to #12869.

## Reproduce

On an Apple Silicon Mac with Xcode's Metal compiler, and SSH access to the
configured Vulkan machine (`coderos-4080` here), from an isolated worktree:

```sh
COAX_JOBS=2 COAX_CHECK_JOBS=2 coax/web/tool.sh coax/tests/build-graphics.coax \
  "$PWD/coax/bin/coax" "$HOME/.openagents/aethyrforge/metal-compiler"
COAX_JOBS=2 COAX_CHECK_JOBS=2 coax/web/tool.sh coax/tests/metal-parity.coax \
  "$HOME/.openagents/aethyrforge/metal-compiler/coax" \
  "$PWD/apps/openagents-web/build/everglade.wasm" \
  "$HOME/.openagents/aethyrforge/metal-qualification" coderos-4080
```

The second command is the complete shader qualification: all eleven entry
pairs through Apple's offline compiler, seven existing G5 fixtures through
muted headless Chrome and native Vulkan, then Metal, followed by both pixel
comparisons. Build the Wasm first with `verse/gpu/everglade-pass.coax`'s default
build path, or pass an already built image as above. The explicit image must
come from the same source revision. The iteration compiler is separate from
the published compiler cache; this command does not publish a compiler.

`--from N`, `--reuse-shaders` and `--reuse-reference` support bounded slices
and debugging against retained references. Only use reuse flags with unchanged
qualified inputs. Each subprocess has a deadline and its own log. Source,
GPU and compiler identities and retained artifacts are recorded below.

## Graphics and host boundary

`coax metal FILE --vertex V --fragment F [-o shader.metal]` runs the ordinary
graphics checker and shared tree serializer, then `metal-shader.coax` writes
MSL. No translated WGSL, external shader translator or hand-written realm
shader participates. Existing scalar `metal` and `metal-compute` commands keep
their profiles. `--tree` exposes the same serialized words; graphics failures
have E0707 diagnostics.

Compile MSL 2.3 or later with fast math disabled. Tests use MSL 3.2 with safe,
precise math. Resource-only argument structs are passed by value, per Apple's
[MSL specification](https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf),
section 2.12.2. V3 fields use `alignas(16) packed_float3` to preserve portable
storage size and alignment; admitted storage structs have compile-time stride
assertions. Helpers specialize opaque resources, retain local-array thread
pointers, and carry their resource context explicitly. Generic and fine
derivatives use Metal's high-precision derivatives. `discard_fragment()` on
MSL 2.3+ preserves helper execution for subsequent derivatives.

Reflection is `metal-reflection 1` on stderr. Its resource records contain
class, slot, shape, stride, stage mask and name; entry and vertex attribute
records contain the actual interface and stream offsets/stride. Resource
names are shared across entries; inconsistent types refuse. Independent slot
spaces admit buffers 0–28, textures 0–127 and samplers 0–15. Buffer 29 carries
storage element counts; vertex stream 30 is reserved. Entry/procedure/local
limits use the portable tree's existing capacities. Unsupported layouts,
types, stages and missing names refuse rather than guessing a binding.

`metal-pass.coax` resolves the existing G5 spec, reflection, buffer lengths,
draw records, texture shapes, blend/depth state and pass order into a bounded
plan. Its authority policy is `metal-plan.coax`. `metal-sdk.swift` performs
Metal SDK calls and readback only. Texture arrays use the same sRGB format and
level-major/slice-major data as the existing hosts. Buffer allocation padding
and length counts match their physical binding ranges. No gameplay or shader
arithmetic lives in Swift.

## GPU results

Apple M5 Max, macOS 26.4; reference WGSL on muted headless Chrome on the Mac,
SPIR-V on the RTX 4080 at `coderos-4080`. The seven G5 captures are 320×180,
RGBA16F read back to f32. Existing tolerance is
`abs(reference - actual) <= 0.002 * max(1, abs(reference))` per value, with
the existing allowance for rasterization/foliage edges. No tolerance changed.

| G5 case | Values compared | Existing allowance | Metal vs WGSL past tolerance | Metal vs SPIR-V past tolerance |
| --- | ---: | ---: | ---: | ---: |
| bg | 460,800 | 40 | 0 | 6 |
| scene + shadow | 460,800 | 400 | 0 | 7 |
| grass | 460,800 | 400 | 0 | 4 |
| water transmit + emit | 460,800 | 2,000 | 14 | 390 |
| fx + both particle layers | 460,800 | 10,000 | 0 | 14 |
| bloom + post | 230,400 | 2,000 | 0 | 28 |
| live | 230,400 | 2,000 | 0 | 28 |

There are zero nonfinite values. Post/live execute 15 passes, 129 nonempty draw calls
and all eleven shader libraries, including HUD, both particle stages, both
water stages, shadow, scene, grass, background, bloom and post. The current G5
spell fixture contains 16 ambient particle sprites; the old generator comment
claiming 429 Meteor Swarm quads is not evidence for this run. The complete
pass fixtures and actual draw records are retained. Metal omits records with
zero vertices or instances; Vulkan's log counts those attempted records too.
Shader qualification does
not measure interactive world performance or claim the Mac realm host exists.

Post also ran with `MTL_DEBUG_LAYER=1 MTL_SHADER_VALIDATION=1
MTL_SHADER_VALIDATION_REPORT_TO_STDERR=1`: both validation layers enabled,
zero reported errors; its separately instrumented readback has zero values
past tolerance against WGSL. Vulkan reports zero validation warnings/errors.
See Apple's [API validation](https://developer.apple.com/documentation/xcode/validating-your-apps-metal-api-usage)
and [shader validation](https://developer.apple.com/documentation/xcode/validating-your-apps-metal-shader-usage)
documentation for the debug switches.

## Compiler checks and provenance

Qualification logs, source/output identities and final mutation totals are
listed in this directory's gate record. Thirteen additional small checked
trees exercise scalar operations, control flow, local-array specialization,
storage layout, all texture operations, stage interfaces, derivatives, helper
demotion and depth. Their exact MSL contracts are accepted by Apple's SDK;
separate GPU comparisons above validate the realm arithmetic. Portable
malformed-tree refusals also run with dirty reusable work memory. These checks
caught and fixed an out-of-bounds constructor type lookup.

The benchmark compiler is generated from the complete authored Coax compiler
module, then linked to the C++ platform driver. Scalar/compute regressions,
CLI graphics admission, authority mutation, fixed-point regeneration and
tool classification are recorded alongside the GPU results. No existing
imported qualification is claimed to have been rerun merely by root mutation.

Final authority mutation killed all **1,421** entry-source mutants; the
bounded SDK plan policy killed all **169**, with no survivor marked equivalent.
The final compiler regenerated the complete compiler module: the header is
byte-identical and C differs only by one terminal LF from direct stdout versus
the process tool's line capture. All eleven emitted MSL files and their
reflection are byte-identical to the SDK/GPU-qualified files. The final CLI
check also compiles an explicit array initializer with Apple's SDK, compares
the numeric Metal/WGSL/SPIR-V trees, rejects a missing entry, rejects an
unsupported vertex format before writing the output file, and checks the
unchanged scalar mode and its loop refusal. The existing Metal compute suite
passed all 13 tests on the initial iteration image; compute source is unchanged.

The renderer/shader inputs are pinned at `ff4bcffbda`, plus the new Metal writer
and verification tooling. Later room/network changes on main do not alter this
frozen qualification. See `artifacts.sha256` for exact source and output
identities. Raw CPU buffers, textures, Wasm, WGSL/SPIR-V/Metal readbacks and
plans, both compiler images and the SDK executable remain under
`~/.openagents/aethyrforge/shader-qualification-12867/`. Recreate a relocated
plan with `metal-pass.coax`; saved plans retain their original absolute paths.
The committed logs and shaders are sufficient to audit the reported comparisons;
the one-command harness regenerates the complete fixtures.

`tools-check` reports five pre-existing unclassified Python files under
`verse/tools/humans/`; this change introduces no unclassified tool. The normal
compiler bootstrap now gates the new authority unit. This record qualifies an
iteration image for shader work; it does not publish a fully gated compiler
release or replace the existing compiler cache.
