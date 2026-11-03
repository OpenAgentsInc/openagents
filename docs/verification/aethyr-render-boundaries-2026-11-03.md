# Aethyr: the two unqualified render boundaries (#12628)

Date: 2026-11-03. Both boundaries stay outside the declared profile. This
record states exactly why, from the retained evidence. It runs no producer,
GPU frame, CPU image or comparison again: every input it reads is a retained
record, and no passing check was rerun.

## Dead Gears: the 159 edge pixels

**Outcome: outside the profile.** The difference is a depth tie between two
faces of one box, decided by fixed-function depth interpolation on the GPU.
Neither Vulkan nor WebGPU specifies that interpolation to the last bit, so no
CPU raster profile can predict which face wins.

The facts, from the retained records:

- **Which pixels fail.** The retained complete images
  ([box36 runtime](data/aethyr-ch11-2026-11-01/canonical-box36/runtime/qualification.json))
  fail in one pixel only: three RGB channels at (42, 76), the same on Vulkan
  and WebGPU, with a maximum error of 0.0057. The other 158 pixels of the
  159-pixel [geometric envelope](aethyr-shared-edge-envelope-2026-11-02.json)
  pass the strict bound of 0.002. The envelope is a conservative exclusion
  around shared fold edges. It isn't the set of differing pixels.
- **Why (42, 76) differs.** The
  [depth-contest diagnosis](data/aethyr-ch11-2026-11-01/reference-depth-contests/deadgears/analysis.json)
  traced every fragment at (42, 76) on the CPU reference
  ([cpu-fragments.json](data/aethyr-ch11-2026-11-01/reference-depth-contests/deadgears/cpu-fragments.json)).
  Two faces of instance 129 meet at an edge through the pixel: triangle 9–11
  at depth 0.9982853531837463 and triangle 27–29 at 0.9982852935791016. The
  two depths are one float32 step apart. On the CPU, 27–29 is nearer, passes
  `less` and writes its colour. The diagnosis then fed the CPU's exact clip
  positions to the GPUs' fixed-function rasterizers. On the RTX 4080
  (Vulkan), 9–11's depth comes out as 0.9982852935791016, equal to 27–29's.
  On the WebGPU probe, the two depths come out swapped: 9–11 at
  0.9982852935791016 and 27–29 at 0.9982853531837463. Either way, 27–29 is no
  longer strictly nearer, fails `less`, and 9–11's colour stays.
- **The colours confirm it.** In that epoch's
  [failed comparison](data/aethyr-ch11-2026-11-01/reference-depth-contests/deadgears/failed-comparison.json),
  the CPU pixel (0.0681, 0.0818, 0.1061) is 27–29's shaded colour
  (0.0681, 0.0818, 0.1061), and the GPU pixel (0.1394, 0.1528, 0.1736) is
  9–11's (0.1394, 0.1529, 0.1736), each to within half-float rounding. The
  GPU drew the other face of the tie. It didn't shade a face wrongly.
- **The current epoch.** The winding repair (box36) changed the faces' colours
  at (42, 76). The error there fell from 0.0713 to 0.0057, at the same pixel
  on both GPU backends. The current epoch has no fragment trace, so this
  record doesn't say which faces contest that pixel now.

**What would qualify it.** A *depth-tie* profile: at a pixel where two
fragments' depths are within one float32 step, the GPU colour must equal the
shaded colour of one of the tied fragments, under the strict bound. Every
other pixel stays strict. The profile needs a fragment trace of the current
epoch at (42, 76). The retained profiler
(`aethyr/render/tests/profile-reference-frame.coax --fragments --pixels 42,76`)
produces one, but it admits only a receipt whose generated C, header and
executable it can hash. The box36 receipt was produced on the Mac with the
older 308 compiler and records no such paths, so producing the trace means
running the CPU producer again under a current receipt. That run, and a
comparator that admits the tie profile (an `#authority` change with its own
mutation gate), weren't done here. Until then the 636 channels stay
unqualified as recorded.

## Everglade: the complete CPU image against WebGPU and Vulkan

**Outcome: outside the profile.** No complete CPU image exists, so there's
nothing to compare the GPU frames with. The CPU reference interpreter refuses
the frame at its budget.

The facts, from the
[CPU reference record](aethyr-cpu-reference-final-2026-11-02.json):

- After the boolean short-circuit repair, the `complex-world` profile stops
  with status 4 (budget refusal) in the `shadow` operation at 3,000,000,000
  shader steps, with zero image writes. For scale, the whole Grid frame at
  the same 320×180 takes about 1.55 billion steps.
- A 20-billion-step experimental profile also didn't finish. It is retained
  and withdrawn, and its API isn't shipped.
- The GPU side passes on its own terms: the 1280×800 WebGPU ready frame and
  stock golden, and the native Vulkan frame with all 190 rows, eight textures
  and zero validation errors or warnings
  ([native measurement](aethyr-native-frame-measurement-2026-11-02.json)).
  That shows the GPU executes the frame. It doesn't show its colours agree
  with a CPU image.

**Why it can't be done here.** One command may take at most 15 minutes, and
a larger single-command budget isn't assumed. A complete Everglade CPU image
needs the interpreter to *resume*: render the frame in bounded pass or draw
slices, each under the deadline, carrying depth, colour and the shadow target
from slice to slice, with the joined image hashed as one. The interpreter
(`aethyr/render/reference-ir.coax`, `#authority`) can't resume today. Adding
that is new interpreter work with its own zero-survivor mutation gate. It is
the follow-on the CPU reference record names ("bounded resumed pass/draw
slices or a separately measured faster path").

## What stays true

- The Grid's 320×180 frame agrees across CPU, WebGPU and Vulkan on all
  230,400 channels, with no mask
  ([CPU reference](aethyr-cpu-reference-final-2026-11-02.json)).
- Dead Gears' strict interior (229,764 channels) passes on both GPU backends.
- No tolerance changed, and no masked image is accepted as a pass.
