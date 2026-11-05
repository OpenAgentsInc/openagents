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
