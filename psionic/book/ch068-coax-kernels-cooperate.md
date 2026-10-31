# 68. Coax kernels cooperate

> **Status: unimplemented — design draft.** Nothing here runs yet.

gpt-oss decode on the RTX 4080 spends most of its time in the selected expert
products. Faster products need threads that share work: tiles in shared
memory, barriers and lane exchange. This chapter extends Coax's compute
contract with those forms. Coax owns their meaning and admission, and the
native backend lowers them.

## What this chapter covers

- Explicit cooperative tiles held in group-shared memory.
- Uniform participation and barriers: every lane in a group reaches each
  barrier, and the compiler refuses a kernel where that cannot be shown.
- Lane exchange (shuffles) within a warp or SIMD group.
- Device capability admission: a kernel that needs a feature runs only on a
  device that reports it.
- Packed BF16 and FP16 matrix primitives.
- No hand-written model kernels. Every model kernel stays Coax source.

## What it builds

- Planned extensions to the `coax cuda` and `coax metal-compute` subsets, with
  CPU scalar oracles for each new form.

## Dependencies

- [Chapter 37](ch037-a-coax-compute-subset.md): the existing compute subset,
  including the Metal group kernels.
- [Chapter 66](ch066-a-complete-cuda-decoder.md): the decoder whose products
  this chapter speeds up.
- [Chapter 67](ch067-prefill-together.md): batched prefill.

## Done means

- Each new form has a CPU scalar oracle and small actual-device checks that
  match it.
- Negative cases are refused: a barrier that some lanes skip, and a kernel
  run on a device without the capability.
- Only after those checks pass does any full-model measurement use the new
  forms.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 4.
- [Making Psionic serving faster](../docs/serving/performance.md): cooperative
  BF16/FP16 matrix operations (P2).
- [gpt-oss device record](../../docs/verification/psionic-gptoss-device-2026-10-29.md).
