# DeadGears precision allowance: research handoff

Work is stopped. No production oracle, mask admission, comparator exception or
new precision tolerance has been implemented. The ordinary comparisons remain
failed. All research below is under this `/tmp` directory; the old book worktree
was frozen before this research began.

## Established results

The original four contests are not duplicated coplanar faces. Exact rational
geometry from the current vertex words shows one pair of perpendicular faces
sharing a box edge and three pairs of intersecting capsule surfaces. The capsule
plane angles are about 10.73 and 17.73 degrees. Geometry nudges would not be a
defensible numerical fix.

Two independent geometry improvements are complete:

- Canonical eight-direction capsule rings: exact closing seam/cardinals/poles;
  8/8 authoritative mutants killed; 728 independent primitive cases pass on
  native C, Vulkan and WebGPU. It did not resolve the original four contests.
- Correct box winding: the four X/Y faces previously opposed their authored
  normals. Source checks fail before the fix and all 27 pass after it. Three
  native modes verify 108 packets and 36 triangles each, with five loss controls.
  Separate execution of the retained old source proves all 18 per-face packet
  multisets are byte-identical. Culling remains disabled.

The fresh default producer's winding-repaired frame still fails strict comparison
on both GPUs: three RGB channels at `(42,76)`, maximum error
`0.005706787109375`; zero nonfinite values or excluded pixels. Both proper CPU
raster profiles were measured. The native image is byte-identical before/after
winding. WebGPU changes six half-float channels at five other pixels, maximum
`0.000244140625`; no visible layout/geometry/HUD regression was seen.

The original unobscured input still fails twelve channels at four pixels. The
fresh default's extra UI plates explain its different visible errors. A prior
progress message incorrectly suggested a fade change: wordwise comparison proves
fade/camera/object data unchanged, plate count 11→14 and plate/HUD data changed.

## Normative boundary

Primary sources:

- [Vulkan floating-point computation and conversions](https://docs.vulkan.org/spec/latest/chapters/fundamentals.html#fundamentals-fp-ops): nonshader representation and arithmetic are not fixed; the general accuracy requirement is approximate. Defined-format conversion permits either adjacent representable value.
- [Vulkan polygon rasterization](https://docs.vulkan.org/spec/latest/chapters/primsrast.html#primsrast-polygons-basic): depth is linearly interpolated; shared-edge coverage is constrained, but no precise CPU-centred depth-error constant is supplied.
- [Vulkan limits](https://docs.vulkan.org/spec/latest/chapters/limits.html): `subPixelPrecisionBits` describes framebuffer X/Y precision, not D32 interpolation accuracy.
- [Vulkan shader arithmetic](https://docs.vulkan.org/spec/latest/appendices/spirvenv.html#spirvenv-precision-operation): operation accuracy, implementation-defined rounding and allowed contraction/reassociation must be respected.
- [Vulkan depth comparison and writes](https://docs.vulkan.org/spec/latest/chapters/fragops.html#fragops-depth-comparison): comparison uses incoming depth and stored depth; the text does not establish incoming D32 quantization before LESS.
- [Pinned WGSL accuracy](https://www.w3.org/TR/2026/CRD-WGSL-20261026/#floating-point-accuracy): basic operations, inherited builtins, rounding freedom and finite/domain assumptions constrain a shader model.
- [WebGPU rasterization](https://www.w3.org/TR/2026/CRD-webgpu-20260915/#rasterization): gives the abstract coverage/interpolation/depth sequence; it does not turn the measured device profile into a universal numeric bound.

There is no established normative universal four-ULP or `1e-5` depth band around
our CPU plane evaluation. Treating the approximate minimum as such a theorem
would be unjustified. A complete interval-enclosure oracle needs an additional
explicit fixed-function arithmetic contract; a measured device assumption must
be labelled as conditional, not as a Vulkan guarantee. A broad semantic
vertex-depth hull is possible but may admit too many occluded candidates to be
useful or to detect missing geometry.

## Input-only preflight

`input-preflight.json` covers all 20,628 actual indexed VS invocations / 6,876
triangles. No GPU colour/depth file was read. Nominal native outputs are finite;
there are 736 exactly degenerate world triangles and eight triangles crossing
both W=0 and the near plane. An initial no-clipping profile cannot qualify this
whole frame by ignoring those triangles.

For a four-term linear form, each term encounters at most four rounding factors.
With directed binary32 unit roundoff `u=2^-23`, one evaluation has error
`gamma4*S`; two evaluations differ by at most `2*gamma4*S <= gamma8*S`.
For composed box world→clip transforms, propagate world error:

`delta_clip_j <= gamma8*S_clip_j + (1+gamma4)*sum_i(abs(VP_ji)*delta_world_i)`.

The magnitude census gives maximum box clip envelopes approximately
`[3.17e-5, 2.90e-5, 5.93e-5, 5.93e-5]`. These are shader-linear-form estimates,
not depth tolerances. Production bounds would require outward exact arithmetic,
normal/FTZ/overflow guards, and source-graph validation. Capsule normalize/cross
eligibility remains a separate obligation. The authored epsilon avoids nominal
zero axes in this capture; the minimum nominal axis magnitude is `1e-5`.

## Promising positive-certificate formulation

Instead of bounding every possible actual rasterizer, prove that at least two
permitted **complete global arithmetic models** can choose different frontmost
primitives from the same inputs. This is an incomplete positive certificate of
legitimate ambiguity, not a universal hardware-error envelope.

The research family is fixed to exact linear interpolation followed by one
GLOBAL internal sample-depth representation choice: binary32 nearest-even,
downward or upward rounding. LESS and D32 storage then consume that typed value.
This is a proposed implementation choice, **not** an inference that attachment
conversion is mandated before comparison. Model conformance still needs review.
Keeping the same typed value for comparison/storage also avoids an invariance
problem from mixing high-precision incoming depth with differently rounded
stored depth.

`depth_witness_preflight.py` uses exact integer homogeneous edge functions and
rational depth, including full prior occluder history. Five independent exact
fixtures pass. Its full-frame exploratory result takes 4.34 seconds: 615,400
pixel visits, 124,420 covered fragments, maximum 18 fragments/pixel. The three
models disagree at only two pixels: `(42,76)` and `(262,99)`. The provisional
mask SHA is `0f0558832f01640528dfba5a6b6591ac4a74e984c4ed108e439584621447533e`.
It would identify eight colour channels as unchecked and leave 230,392 strict.
It does not certify the old three capsule contests. **This mask is not admitted
and has never been applied to a GPU comparison.**

The exact-coverage preflight is not the measured eight-bit device coverage
profile. A refined witness family could use declared eight-bit snapped XY and
canonical clipping. Under bounded coordinates, edge products fit exact f64
integer significance. Nonnegative barycentric weights and vertex depths in
`[0,1]` give a noncancelling f64 error bound (`gamma4`, or `gamma5` including
vertex-depth conversion); final directed binary32 rounding is far more accurate
than the generic minimum. This is a useful proof route, but snapping scope,
clipping, model conformance and normal/zero handling must be fixed explicitly.
The two-pixel result above is not a result for this unimplemented refined family.

## Candidate images versus a certified allowance

**Candidate images:** render each globally fixed model through the actual FS and
subsequent HUD. Prefer requiring one whole candidate image to match all RGBA,
rather than mixing models between pixels. Each full vector must come from one
candidate; never choose channels independently. This also needs concrete FS
evaluation legality/range certification, or interval branch-stability/fork
certificates. DG lighting contains normalization, ray-box shadow branches,
floor/hash boundaries, pow/exp, fog and UI branches. Nominal shading alone is not
a proof for uncertain inputs. Textures, discard/custom depth, feedback, post/MRT
or unsupported blending should initially refuse.

**Certified allowance:** produce an input-only exception certificate from
different complete winner maps, then preserve the ordinary comparator everywhere
else. Report the exception's entire RGBA packet as unchecked, even if some
channels happen to agree. Still reject nonfinite values across the entire image,
including exception pixels. This avoids claiming FS colour correctness where
only visibility ambiguity has been proved, and matches the draft's explicit
allowance more directly. It does not claim all possible ambiguities are found;
uncertified differences remain ordinary failures.

Before either form is product code, require: pinned package/tree/VS semantics;
single-sample opaque REPLACE, exact LESS order, fixed viewport/depth range,
known depth format; no bias/clamp/custom depth/discard/feedback; complete clipping
and occluder history; finite normal-or-zero arithmetic and bounded area/coordinate
domains. Unknown formats/shaders/topologies and any exhausted resource/work or
exception budget must refuse the entire qualification, never silently broaden
the exception. Budgets must be authored/frozen before target colour ingestion.

## Required independent controls and next steps

1. Prove both complete arithmetic models conform for each supported backend;
   settle internal-depth representation and clipping/coverage premises first.
2. Implement the expected mathematical models/fixtures independently before any
   authoritative Coax admission code. Preserve the exact ordinary failures.
3. Test separated-depth triangles (no allowance), exact ties (earlier wins), the
   analytic adjacent-float crossing fixture, clipped/W-crossing geometry,
   normal/zero/subnormal limits, malformed topology and unknown format/shader.
4. Plant interior colour corruption, a depth/winner change outside the
   certificate, per-channel candidate mixing, certificate translation/swap,
   incomplete prior occluder history, cap+1, forged source hash and nonfinite
   output even inside an exception. Each must fail/refuse. Do not claim colour
   corruption inside an explicitly unchecked packet is detected.
5. Split certificate creation and comparison into separate phases. The former
   accepts no GPU colour/depth argument and freezes all input/model/budget hashes
   plus complete affected-pixel/channel counts. The latter verifies those hashes
   before reading target pixels. No GPU-colour-derived candidates or exceptions.

No next step above has been implemented as production policy. No further command
or writer remains active.

## Exact artifacts to retain

- This directory: all source-only preflight packets, tools, input hashes,
  provisional maps and unresolved-premise reports.
- Frozen old-tree `docs/verification/data/aethyr-ch11-2026-11-01/canonical-box36/`
  and its `runtime/`: checks-first proof, native primitive/preservation artifacts,
  full runtime failures and exact frame/executor archive.
- Frozen old-tree `reference-depth-contests/deadgears/topology-audit/` and
  `current-default-pre-winding/`: source geometry proof and causal correction.
- `/tmp/aethyr-ch11-native-icecream-doorway-main-320/retained-native.tar.gz`, SHA
  `6e5d9bca40d672534e6edbb32565d97664597eddb2b505b0fff50fb3c046f4a4`.
  Actual new Ice Vulkan readback SHA
  `4643f31b844d377102b9a52716b9fb7a22b1778d19aee85015c2e3416c3a80dd`;
  CPU owner reports strict native and browser pass, 230,400 channels each,
  no exclusions, max error `0.00146484375`. CPU owner imports this evidence
  into the new main worktree; no old-tree edits are needed.

Minor research-tool failures are preserved in `input-preflight.json`: an initial
nonindexed-draw assumption was refused and corrected using actual index bytes;
a case-insensitive temporary C filename collision required separating generated
kernel and driver. Neither changed product code or repeated a passing GPU gate.
