# Fixed CPU scalar math (#12917)

Work started in a fresh worktree at remote main `dbd3934983`. The change
replaces host sin/cos/exp/log calls in the evaluator and native-C emitter with
one authored Coax module, `coax/lib/compiler/strict-math.coax`. Generated C
and its embedded text are checked bootstrap seeds. Native verification C uses
the same private runtime. Generated output retains the fdlibm permission
notice. Metal/CUDA, host tan/pow and the engine's existing qualified math
profiles remain separate.

The result is a fixed approximation, not correctly rounded transcendental
math. It assumes nearest IEEE binary64 arithmetic with gradual underflow and
no contraction/reassociation; f32 widens exactly and rounds once. The large
angle reducer uses integer limbs and 1584 bits of 2/pi. Runtime IEEE special
values and compile-time E0412 behavior are checked separately. The numerical
contract and seed-refresh procedure are in `coax/strict-math.md`.

## Checks

| Check | Result |
| --- | --- |
| Normal compiler bootstrap, `COAX_MUTATION=0 coax/build.sh` | Passed authored compiler checks, C/header seed comparisons and final compiler fixed point; unrelated compiler mutation gates were not rerun |
| `coax/tests/run.sh --only native python` | Both units passed |
| `coax/tests/run.sh --only strict_math` | 2,104 independent Decimal references within two ulps; debug/release exact bits; f32 rounding; IEEE edges; E0412; evaluator/native parity; actual verification C; ASan/UBSan passed |
| `coax/seed-portability.sh --linux` | Seeds match on macOS Apple clang and Debian bookworm GCC; independent oracle, native/evaluator parity and ASan/UBSan passed on both |
| `coax test psionic/lib/transformer/train.coax` | 110 checks passed |
| `coax test psionic/lib/transformer/train-kernels.coax` | 10 checks passed |
| `coax test psionic/lib/radiance.coax` | 58 checks passed |
| `coax test psionic/lib/strict-math.coax --library` | All six checks passed; the older bounded/saturating profile is retained |
| `coax mutate psionic/lib/distill-mammoth.coax --root-only` | All 234 mutants killed; dependency checks retained |
| `coax mutate psionic/lib/distance.coax --root-only` | All 399 mutants killed; dependency checks retained |
| Psionic book, macOS arm64 | All 714 debug/release runs match one set of transcripts |
| Psionic book, Linux x86-64 | All 714 debug/release runs match that same set of transcripts; retained successful source/mode pairs cover the complete manifest exactly |
| Original 14 examples in #12917, Linux x86-64 | All 28 debug/release cases match those same transcripts |
| Aethyr book, macOS arm64, after rebasing | All 96 debug/release runs match: the 92-run replay, then the new power-boundary and convex-contact examples in both modes; passing programs were not repeated |
| `coax test aethyr/math.coax --library`, after rebasing | All seven checks pass with the fixed CPU intrinsics; its own arithmetic profile is retained |
| Psionic/Aethyr book structure | Source-linked listings, inventory and local links pass |
| `coax/web/tool.sh tools-check` | Passed; no new tool lacks its reason |

The macOS book replay retained each run's exit status, stdout, stderr and CPU
time. After updating the fixtures, the checker validated those retained
results without rerunning passing programs. Failed cases were rerun after
their pin or stack correction. The independent math qualification is sampled
accuracy evidence, not a universal error theorem. An early exploratory
mutation run, before tightening the large-angle assertions to ulps, had 62
survivors among 376 sites in the non-authority math module; no
equivalence exemptions were added and no zero-survivor claim is made for it.

The separately built Linux GCC test compiler is optimized at `-O2` with
contraction disabled. Its SHA-256 is
`93d56871f286ec2f94e5b62ebc36a031d7fa19ca10ca077fb7dfdeae644b86e5`.
It passed the same independent math and sanitizer qualification before the
book replay, and its bytes were checked on reuse. A combined build/replay
was stopped at its time bound; the successful per-case results from later
slices are retained rather than repeating passing cases.

Cloud Build `65b648ca-832f-4876-81fa-81c02b68023f` built and qualified that
compiler. The complete Linux book result is the union of validated cases in
`cdda0db1-e0d6-4370-aa12-8846871d1e38` (650),
`00320112-d8fd-4cea-a94a-5eaa7032bb94` (one additional),
`3c0ff707-ed64-4cdf-9ca9-0870518f14a2` (44 additional),
`69eeac21-c4ca-4db1-ae05-70d44cc2bc64` (four budget retries) and
`f211edd5-da7c-46e5-b3b1-931c10b86ec9` (15 remaining). The last two builds
succeeded. Earlier slices hit their build duration bound; their per-case
successes are retained, and only failed/unreached cases were retried. Five
missing expected-output files in an initial source archive were supplied
before retrying those cases. No numerical tolerance or transcript exception
was added. A final set comparison found exactly all 714 inventory pairs.

## Re-recordings

The book has no per-platform transcript selections. The short trainer and
kernel gradient digests, Ranger palette digest and continued-training base
checkpoint have one recording each. The additional distance/radiance pins
changed by a few final bits. Distill Mammoth's hand check now uses exactly
representable 0.75, whose odds are exactly 3, rather than assuming binary64
0.8 has odds exactly 4; the check retains exact equality.

The deliberately unstable training examples produce new trajectories. In the
fixed-math active-learning example, uncertainty narrowly passes the existing
gate (24/1,000 resamples at or below the floor, with 25 allowed); the chapter's
explanation now matches that result. No quality threshold or promotion rule
was changed. The chapter-250 workflow evidence was regenerated from the new
transcripts without expanding any capability claim.

`compiled-backward` now declares a 64 MiB child stack, consistent with the
other large examples. Its debug main frame is about 4.4 MB and a nested
compiler frame about 3.7 MB, before the intervening planner/scheduler frames;
the default macOS stack was insufficient.

The run budgets also name measured Linux GCC costs. `diagnose-faults` gets
120 seconds after its unoptimized fixed-math run used 54 of 60 CPU seconds
in isolation. The complete serving-rollout journeys get 180 seconds after
140 CPU/78 wall seconds; the MNIST and CNN classifier journeys get 120 after
86/68 and 70/63 seconds respectively. These change duration bounds only;
every transcript and exit status comparison remains exact.
