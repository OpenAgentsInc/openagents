# Explicit display quantization: disabled iteration

This component adds an explicitly requested presentation mode. Ordinary
web/Vulkan/Metal precision selection, the desktop default and realm opt-in
remain unchanged. The graphics products are private iteration artifacts;
these measurements do not publish a compiler or establish complete host parity.
The previous [2,611-channel display failure](presentation-experiment.md),
historical 7,020-channel failure and asset-generation regressions are retained.

The new `render_presentation_quantized_v1` descriptor draws the complete graph
into RGBA16Float and presents through existing media entry 13. It retains
the checked source target, binding slots, nearest/clamp sampling, replace
blending, no depth, full-screen coordinates and opaque alpha. Its explicit
uniform lane 5 selects clamp to [0,1], multiply by 255, round to the nearest
integer with even ties, then divide by 255. Media lane 5 equal to zero keeps
the previous shader behavior; other selectors do not enable quantization.
The final attachment conversion still needs actual GPU evidence because
most fractions with denominator 255 are not exactly representable.

## Complete conversion probe

The independent Coax oracle enumerates all 65,536 half-float bit patterns
in each RGB channel, in different permutations, over a complete 256×256
RGBA16Float image. Nonfinite encodings are replaced by zero before upload;
negative values, subnormals, values above one, boundaries and all finite
positive encodings are retained. Alpha is deliberately nonopaque at the
source; the descriptor must produce opaque output. Separate f32 checks
exercise even and odd exact ties and both sides of each tie. The expected
bytes use integer arithmetic independent of the shader quantizer.

| Actual complete GPU output | Compared channels | Different bytes |
| --- | ---: | ---: |
| WebGPU BGRA8 | 262,144 | 0 |
| Vulkan RGBA8 | 262,144 | 0 |
| Metal BGRA8 | 262,144 | 0 |
| Metal RGBA8 | 262,144 | 0 |

Each executes the normal interpreter's typed presentation draw, with zero
realm producer frames. The browser and Metal BGRA8 files are byte-identical
(`e68116e4f887fe03cb84684231a4dd4843049b8c539dbf0a2ce7fb37d43c5855`).
Vulkan, Metal and the independent oracle's RGBA8 files are byte-identical
(`0db1566d2c3ceb9141f9c0d4ad9b1453367ff56d51b3516e29ed8e19833cc15a`).
Metal's old-descriptor control also matches this oracle on Apple; this
probe establishes the new output without claiming to distinguish the old
Apple conversion. Vulkan validation and Metal API/GPU validation were
enabled with no errors. Submitted work completed and closed ledgers are empty.

## Frozen complete Everglade frame

The browser uses the original admitted 598 CPU compiler with separately
pinned product inputs and a private presentation overlay. Its complete
producer buffers, eight textures, seven media images and actual BGRA8
readback are retained. The canonical layer bridge remains enabled for this
capture. Vulkan and Metal replay exactly those resource bytes through their
ordinary draw interpreters plus the explicit presentation descriptor.

| Complete actual display comparison | Channels beyond unchanged 0.002 rule | Nonfinite / masked | Inherited live allowance |
| --- | ---: | ---: | ---: |
| Web BGRA8–Metal BGRA8 | 4 / 230,400 | 0 / 0 | 2,000 |
| Web BGRA8–Vulkan RGBA8 | 1,920 / 230,400 | 0 / 0 | 2,000 |
| Metal BGRA8–Vulkan RGBA8 | 1,920 / 230,400 | 0 / 0 | 2,000 |

All strict receipts retain their failures. The complete counts pass the
existing live allowance without a mask, alignment correction or changed
tolerance. The actual Vulkan measurement has 137 draws, validation errors
and warnings zero, unchanged local/remote/SDK input fences and successful
owned cleanup. Metal's FP16 graph and actual presented BGRA8 bytes are each
identical before and after GPU recovery; completed two, pending zero, old
and closed ledgers empty, API/GPU validation enabled and clean.

The real browser held-completion/recovery workload also passes its existing
13 observations. While completion is held, the producer digest, epoch and
submission count do not advance. After device replacement, an old completion
does not release the new pending frame or change its upload prefixes; the
old ledger retires and the resumed producer progresses. Its unchanged
source fence passes. The first attempt requested the driver's old half-float
display and was correctly refused before drawing. That attempt is retained;
the passing attempt explicitly requests the actual BGRA8 display.

This frozen-resource comparison does not cover native decoding or the native
producer together. A separate matched a0f native-producer/native-assets
composition through this mode has **2,213** channels beyond the unchanged
rule, exceeding the existing 2,000 allowance, with no nonfinite values or
masked pixels. That failure remains a promotion blocker. Canonical authored
media decoding, complete services, ordinary Linux integration and the final
current-source matrix remain required before any default switch.

## Source and artifact boundary

The original historical CPU compiler receipt verifies before preparation
and after the new Wasm build. The compiler image is
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`;
the historical compiler source root is c224561457. Public product inputs
are pinned separately at 6f910979e9, followed by the retained private source
overlay. Generated C/header source tables, profiles, native objects and
Wasm are retained. No mixed-source compiler receipt is fabricated.

Only media WGSL, SPIR-V entry 13 and its checked tree change in the private
shader package. Every other WGSL/SPIR-V/tree product is byte-identical to
the frozen package. The new media MSL compiles under the Metal SDK without
fast math. The graphics emitter remains the explicitly qualified iteration
image `c0561aaa7b630175f7e8efc1718c76397ea4584dac04acfacaa6d9cc6ab36b7a`;
no published compiler admission or broader compiler release is claimed.

Checks preceded implementation. Final root mutation gates kill all 15
quantizer, 19 media, 23 presentation, 82 independent probe, 30 exact receipt
and 102 native measurement-policy mutants, with no survivors or equivalent
annotations. Existing unchanged passing gates were not repeated.
JavaScript syntax checks pass. The required repository tools check reports
1,783 tracked tool files, 35 vendored vectors and zero new files missing a
reason header.

Complete sources, SDKs, C/header/profile files, raw bytes, logs, source fences,
failed attempts and payloads are retained under
`~/.openagents/aethyrforge/host-contract-12869/display-quantized-iteration/`.
The new Metal receipts and frozen SDK are under
`~/.openagents/aethyrforge/metal-host-12868/presentation-v1-full/` and
`presentation-probe/`. The committed `display-quantized-iteration/` receipt
directory records these scoped results; it contains no published shader
package or ordinary host selector.
