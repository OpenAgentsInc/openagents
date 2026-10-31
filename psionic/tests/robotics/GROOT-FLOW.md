# GR00T flow and time operators

`psionic/lib/robotics/groot-flow.coax` qualifies a CPU F32 reference slice of
the GR00T N1.7 inference formulas. It does not run learned velocities, a DiT,
the backbone, a complete VLA or a robot controller. It accepts caller-provided
initial state and velocities; it neither generates noise nor owns a request.

The reference source is NVIDIA's Apache-2.0
[Isaac-GR00T revision c56f54128d92f743889b7e4cecfc1ceafd88739e](https://github.com/NVIDIA/Isaac-GR00T/tree/c56f54128d92f743889b7e4cecfc1ceafd88739e):

- `gr00t/model/gr00t_n1d7/gr00t_n1d7.py`, `get_action_with_features`;
- `gr00t/model/modules/embodiment_conditioned_mlp.py`,
  `SinusoidalPositionalEncoding`;
- `gr00t/model/modules/dit.py`, `TimestepEncoder`, whose Diffusers call uses
  256 channels, cosine-first ordering and `downscale_freq_shift=1`.

Source was read as reference. No downloaded repository script was executed.
The Coax implementation and C oracle are independently authored.

## Contracts

| API | Contract |
| --- | --- |
| `gf_schedule(step)` | Steps 0–3 give buckets 0, 250, 500, 750 and `dt=0.25`; other steps refuse. |
| `gf_advance_admit(...)` | Metadata-only time, extent, capacity and overlap admission. |
| `gf_advance(step,x,velocity,out,work,overlap)` | One checked `x + 0.25 * velocity` update over 1–5280 F32 values. |
| `gf_time_admit(...)` | Kind, even embedding width, bucket, capacity and overlap admission. |
| `gf_time(kind,width,bucket,out,work,overlap)` | Action or DiT sinusoidal features with the distinct source conventions. |

The 5280-value bound is the released 40×132 action inventory. The schedule is
stateless: accepting step 2 does not prove the caller already performed steps
0 and 1. Callers own sequencing, model identity, the supplied noise and
velocities, typed allocation extents, exclusive ownership and truthful overlap
flags. This slice covers the ordinary unit-strength Euler update. Action
masks, RTC overlap/inpainting, frozen-step strengths and randomness are separate
work.

The Euler product is rounded to F32 before addition. The sum is checked in F64
and then narrowed to F32, matching the existing checked-storage convention:
nonfinite inputs or an unrepresentable checked sum refuse. This is not a
BF16 activation execution profile. Every input is validated before arithmetic;
the public destination is written only after all staged results succeed.
Metadata/number failures leave scratch alone. Arithmetic refusal may change
private scratch and preserves the public destination.

Both embeddings accept buckets 0–999. Action widths are even 2–4096; DiT
widths are even 4–4096. Their full released widths, 1536 and 256, are tested.
The formulas intentionally have different expression order as well as layout:

- Action: F32 `log(10000)/half`, then F32 multiplication by `-i`;
  concatenate sine followed by cosine.
- DiT: F32 `-log(10000)*i`, then F32 division by `half-1`;
  concatenate cosine followed by sine.

Frequency, phase and each output have explicit F32 stages. CPU transcendental
functions evaluate in F64 and narrow to F32. This establishes the declared CPU
reference formula, rather than bit parity with PyTorch vector math, CUDA,
Metal or BF16 inference. The learned DiT timestep MLP is not included.

## Qualification

Checks were written before implementation. The final changed source passed
**35 Coax checks including dependencies**, killed **92/92 focused root mutants**
with no waivers, and passed **273,040 independent native cases in each of
debug, release and ASan/UBSan**.

The C oracle independently checks the metadata domain, every time-feature
position at multiple widths and buckets, full 40×132 Euler updates, signed
zero/subnormal stages, NaN/infinity at first and last positions of both input
spans, arithmetic refusal, output atomicity and native ABI bounds. Its source
formulas and expected statuses are separate from the Coax implementation.
Native compilation disables fast math and contraction.

```shell
coax/bin/coax test psionic/lib/robotics/groot-flow.coax
coax/bin/coax mutate psionic/lib/robotics/groot-flow.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-flow.sh /tmp/groot-flow-native
```

The native driver also writes F32 dumps of the four full-width action and DiT
time embeddings in each mode. They are diagnostic reference outputs, not model
actions. Existing numerical dependencies were not independently remutated.
No checkpoint payload, model run, external service or physical device is part
of this gate.
