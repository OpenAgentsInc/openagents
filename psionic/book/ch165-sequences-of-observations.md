# 165. Sequences of observations

> **Status: unimplemented — design draft.** Nothing here runs yet.

A world model predicts what a scene looks like next from what it looked like
before and from an action or condition. This chapter builds the
representation such models need: time-ordered, multimodal observations, a
tiny conditioned generation task and the codec and scheduler contracts that
chapter 166 uses.

## What this chapter covers

- Temporal representations: frames with timestamps, fixed frame rates and
  explicit frame counts.
- Multimodal observations: images, state vectors and text conditions in one
  record.
- A tiny conditioned generation task: predict the next frame of a simple
  synthetic scene from earlier frames and an action.
- Codec contracts: what a latent encoder and decoder must accept and return.
- Scheduler contracts: the noise schedule and sampling steps from chapter 146.
- Worlds come from the Aethyr book: batched environments
  ([Aethyr chapter 31](../../aethyr/book/ch031-many-worlds-in-one-step.md)),
  sensors ([33](../../aethyr/book/ch033-sensors-read-the-simulation.md)),
  observation pictures
  ([34](../../aethyr/book/ch034-pictures-for-policies.md)) and replay
  ([36](../../aethyr/book/ch036-record-and-replay-a-world.md)).

## Dependencies

- [Chapter 146](ch146-noise-diffusion-and-flow.md): diffusion and flow.
- [Chapter 145](ch145-compress-and-reconstruct-an-image.md): a learned codec.
- [Chapter 142](ch142-pixels-convolutions-and-patches.md): image operations.

## Done means

- The observation record round-trips without loss and refuses frames with
  missing or out-of-order timestamps.
- The tiny conditioned model predicts held-out next frames better than
  copying the last frame, by a reported margin.
- Codec and scheduler contracts have tests that the chapter 166 components
  must pass.

## Sources

- [Chapter plan](PLAN.md), Part XVIII.
- [Aethyr book](../../aethyr/book/README.md).
