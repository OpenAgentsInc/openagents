# 149. Execute MotionBricks

> **Status: unimplemented — design draft.** Nothing here runs yet.

MotionBricks is an NVIDIA motion-generation package. Its SOMA path takes a
destination and timing, predicts a root trajectory, generates motion tokens
with a masked transformer, and decodes them to poses with a VQ-VAE. This
chapter executes one pinned SOMA package in Psionic and compares full
trajectories with the reference.

## What this chapter covers

- The pinned MotionBricks revision and its SOMA components.
- Root timing and trajectory prediction from a destination input.
- The masked motion-token transformer.
- The VQ-VAE pose decoder.
- Rotation, skeleton and time conventions stated as chapter 147 profiles.
- License handling: the package card separates NVIDIA model terms from the
  Apache-2.0 software and from the restricted BONES motion samples. You check
  the specific asset terms before you choose any fixture.

## Dependencies

- [Chapter 147](ch147-poses-have-conventions.md): pose conventions.
- [Chapter 148](ch148-motion-tokens-become-poses.md): codebooks and masked
  generation.
- [Chapter 10](ch010-attention-without-training.md): the shared attention
  core.

## Done means

- For fixed inputs, the root trajectory, tokens and decoded poses match
  recorded reference outputs within a tolerance frozen before comparison.
- Full fixed-input trajectories are compared, not single frames.
- The chapter states that inference parity does not establish reproduction of
  the original training.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), MotionBricks entry and
  license notes.
