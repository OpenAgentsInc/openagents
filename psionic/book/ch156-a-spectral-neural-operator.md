# 156. A spectral neural operator

> **Status: unimplemented — design draft.** Nothing here runs yet.

A Fourier neural operator (FNO) learns a map between fields, such as from a
material's permeability to the pressure it produces. It does this with
layers that multiply the field's low Fourier modes by learned weights. This
chapter builds the spectral layer and checks it on a small field.

## What this chapter covers

- Tiny DFT and FFT references in two dimensions, reused from chapter 151's
  spectral core.
- Complex arithmetic and complex-valued weights.
- Mode truncation: keep a fixed number of low-frequency modes and set the
  rest to zero.
- The spectral layer: FFT, multiply the kept modes, inverse FFT, plus a
  pointwise linear path and an activation.
- Gradients through the spectral layer, checked with finite differences.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): the FFT and
  complex arithmetic.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse-mode
  differentiation.
- [Chapter 109](ch109-learn-a-physical-system.md): the first physics
  chapter, with its held-out evaluation pattern.

## Done means

- The 2D FFT matches a direct DFT within a stated tolerance.
- The spectral layer's output matches a direct computation on a small field.
- Its gradients match central finite differences.
- A field smaller than the kept modes is refused.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), physics section.
