# Float drawing and one final UNORM conversion: diagnostic only

This is an optional presentation experiment. Production formats, shaders,
comparison bounds, the desktop default and realm opt-in remain unchanged.
It does not turn the ordinary 8-bit Vulkan failure into a passing result.

The [source-qualified complete captures](qualified-shared-frame.md) already
retain matched browser, Metal and Vulkan RGBA16Float frames. Each complete
frame was normalized to RGBA f32 and passed through the existing Coax
`rr_unorm8` once: clamp to [0,1], multiply by 255, round ties to even and
divide by 255. No pixel is masked, shifted, aligned or discarded. The exact
quantizer source is retained with the raw outputs in
`~/.openagents/aethyrforge/host-contract-12869/qualified-frame/presentation-unorm/`.
The admitted historical `598ddc…` CPU compiler executes these Coax tools;
the enclosing captures retain their original provenance joins.

| Converted complete frames | Channels past unchanged tolerance | Nonfinite / masked | Existing live allowance |
| --- | ---: | ---: | ---: |
| Browser–Metal | 5 / 230,400 | 0 / 0 | 2,000 |
| Browser–Vulkan | 1,603 / 230,400 | 0 / 0 | 2,000 |
| Metal–Vulkan | 1,604 / 230,400 | 0 / 0 | 2,000 |

All strict comparison receipts retain `status: fail`; their full channel
counts are within the inherited live allowance. Browser–Metal maximum
absolute error is one 8-bit step. Both Vulkan comparisons retain maximum
error 0.1294117793, including the earlier float-frame raster differences.
This predicts a possible portable display path, rather than observing an
actual GPU presentation conversion. Hardware sampling and conversion still
need measurement on all three hosts.

There is also an explicit visual regression cost relative to the historical
per-blend UNORM path. The Metal owner's separate diagnostic measures 5,811
channels past tolerance between a once-quantized float frame and the ordinary
browser BGRA8 frame, each differing by one 8-bit step. A future shared float
policy must describe that changed accumulation precision and qualify a fresh
reference. It cannot reinterpret the old 7,020-channel failure or claim that
the old web result stayed identical.

`aethyr/render/presentation.coax` supplies the optional typed descriptor:
the complete ordinary graph draws to RGBA16Float, then existing authenticated
media entry 13 samples that same-size screen into the actual RGBA8/BGRA8
display. The descriptor owns full-screen coordinates, brightness one,
opaque output, replace blending, no depth, nearest/clamp sampling, reflected
binding slots/stages and six vertices. Unsupported profiles/formats and
invalid or unaffordable device extents publish no partial descriptor.
Neither adding the descriptor nor projecting it through the scalar face
selects it for a host.

Checks were written before implementation. Its final root gate kills all
19 mutants, with no equivalents. The common viewport admission was factored
from `resources.coax`; its 182-mutant unit gate also has zero survivors and
no equivalents. These logs and all three complete comparison receipts are
retained in `presentation-experiment/`. `artifacts.sha256` binds the exact
normalizer, quantizer, complete converted frames and receipts.
Native facade C/header emission with the admitted CPU image and C scalar
face syntax compilation pass. The repository tools check was run; it reports
the same seven unrelated pre-existing Python files without `Firm reason`
headers. This checkpoint adds no non-Coax tool.

Next exits are actual browser, Vulkan and Metal display-target readbacks
through their normal interpreters, source/binary joins, recovery/ledger
qualification and an explicit old-to-new visual comparison. No presentation
policy is enabled by this checkpoint.

## Actual display-target experiment

The optional descriptor now has explicit diagnostic consumers in the normal
browser and Vulkan SDK interpreters. Browser capture must request
`coaxialRenderPresentation="float"`; the native frozen-frame tool must
request `rgba8-present`. Neither ordinary path selects it. Metal consumes
the same descriptor through its existing media entry, with no handwritten
conversion kernel. Each executes 136 ordinary graph draws and one
presentation draw. The following complete readbacks supersede the earlier
CPU conversion as evidence about the actual display path:

| Complete actual comparison | Channels beyond 0.002 | Nonfinite / masked | Existing live allowance |
| --- | ---: | ---: | ---: |
| Browser BGRA8–Metal BGRA8 | 5 / 230,400 | 0 / 0 | 2,000 |
| Browser BGRA8–Vulkan RGBA8 | **2,611 / 230,400** | 0 / 0 | **FAIL: 2,000** |
| Metal FP16 graph–Vulkan FP16 graph | 53 / 230,400 | 0 / 0 | 2,000 |

The failed actual Vulkan display comparison is retained. It prevents
promotion of this experiment. The exact same Vulkan FP16 graph, converted
once by the existing CPU `rr_unorm8`, differs from the browser in 1,603
channels; that CPU result differs from actual Vulkan presentation in 1,573
one-step channels. Consequently the CPU conversion receipt cannot stand
in for an actual GPU display receipt. Vulkan's float-to-UNORM rule permits
either neighboring integer and recommends nearest rounding; it does not
require round-to-nearest-even for every noninteger input. See the
[Vulkan numeric conversion contract](https://docs.vulkan.org/spec/latest/chapters/fundamentals.html#fundamentals-fpfixedconv).
No shader, format default, frame tolerance or live allowance was changed
to make these results pass.

The browser CPU/Wasm and native facade use the original admitted historical
598 compiler, with original receipt verification before and after closed
product overlays. The existing shader products keep their separate frozen
qualification identity. Exact C/header/profile files, SDK copies, image,
raw frames, payloads and original failed preparation attempts remain under
`~/.openagents/aethyrforge/host-contract-12869/presentation-experiment/`.
The actual Vulkan measurement retains complete downloaded SDK logs,
validation enabled with zero errors/warnings, unchanged local/remote/SDK
inputs and successful owned remote cleanup. Metal reports completed one,
pending zero and closed ledger zero with API/GPU validation enabled.

The first Vulkan execution refused its physical maximum extent of 32,768
because the descriptor expected the portable 16,384 cap. The shared Coax
device-limit intersection now handles that physical observation before
viewport admission; it preserves refusal for nonpositive capacity. Its
resource, host-policy and presentation root gates kill 194, 235 and 19
mutants respectively, with zero survivors or equivalent annotations.
The native measurement selector's 101-mutant policy gate also passes.
This correction adds no compiler policy or display default.
