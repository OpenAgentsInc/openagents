# 146. Noise, diffusion and flow

> **Status: unimplemented — design draft.** Nothing here runs yet.

Diffusion and flow-matching models generate data by learning to undo noise.
World models (chapter 166) and robot policies (chapter 169) use these methods.
This chapter trains a tiny generative model so that you can see how the
objectives, schedules and samplers differ.

## What this chapter covers

- The forward noising process and its schedule (for example linear and
  cosine variance schedules).
- Training objectives: noise prediction, data prediction and velocity
  prediction for diffusion, and the velocity field of flow matching.
- Conditioning: a class or vector input that steers generation.
- Sampling: a deterministic sampler and a stochastic one, with a fixed number
  of steps, and a named counter-based random generator so every sample is
  reproducible.
- A tiny dataset that you can check directly, such as a 2D mixture of
  Gaussians, where the true distribution is known.

## Dependencies

- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): training with gradients.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): differentiation
  of a small network.
- [Chapter 53](ch053-choosing-the-next-token.md): the named RNG with explicit
  seed and counter.

## Done means

- Each objective's loss matches an independent derivation on fixed inputs.
- Samples from the trained model match the known target distribution within
  a stated distance (for example a two-sample test statistic) on a fixed seed
  set.
- The same seed reproduces the same samples bit for bit under the named
  profile.
- The chapter states that it teaches the methods and does not qualify any
  later world-model or robotics architecture.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
