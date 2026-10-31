# 159. Learn a scene from its views

> **Status: unimplemented — design draft.** Nothing here runs yet.

A coordinate network maps a position to a value, such as a color or a
distance. If you can render it and differentiate the render, you can fit it
to pictures. This chapter builds that step by step and ends with a small
radiance field trained on rendered views of a synthetic Verse scene.

## What this chapter covers

1. A coordinate network that fits a single image.
2. A coordinate network that fits a signed-distance field.
3. Differentiable ray compositing on tiny fixtures: samples along a ray,
   densities, transmittance and the composited color.
4. A small radiance field trained on rendered views of a synthetic Verse
   scene and rendered from held-out cameras.

Measurements:

- Compositing derivatives compared with finite differences.
- Held-out image quality from cameras not used in training.
- Depth error where the synthetic scene gives ground truth.
- Memory and render time.

## Dependencies

- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse-mode
  differentiation.
- [Chapter 158](ch158-a-field-around-a-shape.md): signed distances.
- The Aethyr renderer
  ([Aethyr chapter 11](../../aethyr/book/ch011-the-wgsl-renderer.md)) for
  the training views.

## Done means

- Compositing gradients match finite differences on tiny fixtures.
- Held-out image quality, depth error, memory and render time are reported.
- The chapter states that good novel views are not evidence of an accurate
  collision surface or a usable game asset.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
