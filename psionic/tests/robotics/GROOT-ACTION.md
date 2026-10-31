# The original GR00T learned action head on Metal

`groot-action-metal-plan.coax` owns the complete selected category-2 action-head
program: 537 original named tensors, bounded original-header binding, exact
category slicing/transposition, active position prefix, shapes, 2,877 ordered
micro-operations, disjoint numeric flags, device receipts and final publication.
The C bridge maps/reads files, allocates buffers, compiles Coax-generated Metal,
dispatches Coax descriptors, waits and records named private captures. It runs
no learned arithmetic and has no CPU model fallback.

This first integration profile is one observation, **140 unpadded tokens**,
132 stored state channels, 40×132 actions and explicit caller-supplied noise.
It executes VLLN, all four VL self-attention blocks without a padding mask,
the category state encoder, both distinct timestep conventions, action encoder
and learned position prefix, all 32 alternating DiT blocks at each of four
Euler steps, final AdaLN and the category action decoder. Text/self/image/self
routes intersect the appropriate valid backbone tokens. The first state token
participates in DiT; only the last forty rows enter the action readout.

The source is the original
[Isaac-GR00T revision c56f54128d92f743889b7e4cecfc1ceafd88739e](https://github.com/NVIDIA/Isaac-GR00T/tree/c56f54128d92f743889b7e4cecfc1ceafd88739e),
model `nvidia/GR00T-N1.7-LIBERO`, revision
`2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`, `libero_10`. Source was read;
no downloaded checkout script or module was executed. Actual complete cached
shards supply the coefficients. Header fingerprints are checked again; the
prior full-cache hash verification is reused, not represented as a fresh hash.
Only public metadata and qualification metrics are committed.

## Execution and ownership

The declared profile uses original BF16 coefficient bytes, F32 activation
storage, matrix-unit accumulation, products, bias, affine, activation, softmax
and Euler stages. **Normalization statistics use Coax F64**: the existing
float-float profile on Metal, with F32 normalization/affine storage stages.
This mixed reduction profile is explicit. It is not NVIDIA's default BF16
activation execution or a claim of device bit identity.

`gac_spec`/`gac_bind` own each tensor's original name/rank/extent, category-2
range and conversion. `gac_stage`/`gac_node`/`gac_kernel` own the complete
micro-op program. `gac_sent` and `gac_done` require each actual launch receipt,
successful wait, exact flag extent and zero in every poisoned flag before
advancing. `gac_publish` validates complete state and finite output bits before
its one final copy, then consumes the cursor. This is single-owner value state;
effect receipts and typed allocation extents must be truthful.

All working buffers and parameter-loading destinations are private. Position
decode uses the existing checked BF16 row decoder; failure can change earlier
private parameter rows and reports zero published values. The caller discards
the failed load. No graph launches until every original parameter prepares
successfully. Intermediate inference stages can change private buffers on
failure; final public action storage remains untouched.

## What was measured and improved

The independent reference uses pinned Torch 2.9.0, Diffusers 0.35.1 and
Transformers 4.57.3 on MPS, original BF16 coefficients decoded to F32,
owned patterned images/state and explicit PCG64 noise. Its recorded capsule
contains every selected DiT/velocity/action boundary. The capsule is reused;
no whole-model CPU inference or reference rerun occurs here.

Before observation we froze `abs(error) <= 0.002 + 0.002*abs(reference)` at
all 31 action-head captures. The initial F32-statistics head passed the final
actions but missed `flow3-block31`: maximum absolute error 0.00423622, worst
budget fraction 1.06961. Switching the already qualified 16×64 projection
geometry to 32×32 gave bit-identical captures and was discarded. A separate
analytic device probe found all **7,168 timestep words bit-exact** with the
original MPS formulas, ruling out the distinct sinusoid conventions.

Wide normalization statistics fixed the accumulated denominator rounding.
With 16×64 projections retained, **all 31 captures / 1,595,296 values passed**
without changing the tolerance. `flow3-block31` fell to 0.000762939 absolute
error, 0.196917 of budget. Final normalized actions fell to **0.0000230074**,
0.00826588 of budget. The Apple M5 Max spent **0.339526 seconds** summed across
142 GPU command batches. This excludes parameter loading, compilation, host
receipts, transfers/captures and comparison; it is not request latency.

The fully composed wide-vision→wide-text→head path passes all nine original
velocity/action endpoint comparisons. Final normalized actions have maximum
absolute error **0.000558734**, 0.200956 of the frozen budget. The original
global `flow3-block31` hidden trace still fails, at 4.88684 of its budget, and
remains recorded as failed. It is not relabelled an all-boundaries pass.

That global hidden comparison starts the two heads from different backbone
values. To isolate the head implementation, an independent original head also
ran on the **exact actual Coax backbone**, original weights, state and noise,
entirely on MPS. All **31 same-input captures / 1,595,296 values passed** the
unchanged tolerance; the largest budget fraction was 0.877073. Conditional
normalized-action error was 0.0000998676. This establishes component fidelity
on the same input alongside end-to-end public-output agreement. It does not
erase the separate global hidden-trace diagnostic. The conditional receipt
names its input SHA-256 and explicitly says the full-network oracle was not
replaced. No Coax inference or original full-network reference was rerun for
that conditional comparison; existing Coax outputs were reused.

## Checks and replay

Checks were written first, then sharpened with original metadata, edge cases
and the exact GPU-qualified descriptor transcript. The latter contains no
learned outputs; it locks the already independently validated micro-op graph.

| Unit | Authored checks | Focused mutants killed |
| --- | --- | --- |
| Action plan | 11 | 1,231/1,231 |
| Action glue kernels | 4 | 199/199 |
| Wide statistics kernel | 1 | 55/55 |
| Descriptor encoding oracle | 1 | 8/8 |

Independent native component equations/refusals passed **417,134 cases in each
of debug, release and ASan/UBSan**. Native metadata/ABI passed **3,570 cases in
each mode**, with every final descriptor identical to the GPU-executed program.
The canonical transcript SHA-256 is
`14aca175010e4b4d5f75140ac010060b5f70eab019698ffaf1a18efaf8aed6c2`.
Existing passing primitive/reference gates were not repeated unchanged.

```shell
coax/bin/coax mutate psionic/lib/robotics/groot-action-metal-plan.coax --root-only --jobs 8
coax/bin/coax mutate psionic/lib/robotics/groot-action-metal-kernels.coax --root-only --jobs 4
coax/bin/coax mutate psionic/lib/robotics/groot-action-wide-kernels.coax --root-only --jobs 4
coax/bin/coax mutate psionic/lib/robotics/groot-action-transcript.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-action-native.sh /tmp/groot-action-native
psionic/tests/robotics/check-groot-action-metal.sh FULL_SHARDS REFERENCE_CAPSULE NEW_OUTPUT
```

An optional fourth argument supplies an actual already-computed Coax backbone
F32 file for composition. It must contain 140×2048 finite values. The oracle
continues comparing every boundary with the unchanged independent capsule and
records the actual input hash. The strict Metal gate fails without a device;
Python is independent development-only replay tooling.

Receipts: [action-head-qualification.json](../../fixtures/robotics/groot-libero/action-head-qualification.json),
[conditional-head-qualification.json](../../fixtures/robotics/groot-libero/conditional-head-qualification.json),
[composed-head-diagnostic.json](../../fixtures/robotics/groot-libero/composed-head-diagnostic.json),
[time-qualification.json](../../fixtures/robotics/groot-libero/time-qualification.json),
and [device-primitives-qualification.json](../../fixtures/robotics/groot-libero/device-primitives-qualification.json).
These qualify owned numerical cases. They do not measure LIBERO task success,
establish another embodiment's semantics, provide a controller, or authorize
physical robot actions.
