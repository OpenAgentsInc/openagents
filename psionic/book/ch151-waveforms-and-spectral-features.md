# 151. Waveforms and spectral features

> **Status: unimplemented — design draft.** Nothing here runs yet.

Speech models do not read raw audio samples directly. They read spectral
features computed with exact conventions for sample rate, window, hop and mel
scale. This chapter builds those features from a tiny discrete Fourier
transform up to log-mel spectrograms. The same spectral core serves the
physics chapters in Part XVI.

## What this chapter covers

- Audio admission: sample format, sample rate, channel count and maximum
  length, with typed refusals.
- Windowing: Hann and other windows, frame length, hop length and padding
  at the edges.
- A direct DFT as the reference, then a fast Fourier transform (FFT) and its
  inverse, checked against the DFT.
- Complex arithmetic with a stated precision profile.
- Spectrogram, mel filter bank and log-mel features.
- Feature normalization exactly as a pinned model expects it (per-feature or
  per-utterance mean and variance).

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): numerical contracts and stable
  reductions.
- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): the operator
  registry.

## Done means

- The FFT matches the direct DFT within a stated tolerance on fixed signals,
  including sizes that are not powers of two if admitted.
- FFT followed by inverse FFT returns the input within a stated tolerance.
- Mel features for fixed audio match recorded reference features for the
  pinned model in chapter 153.
- Audio outside the admitted format is refused.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry.
