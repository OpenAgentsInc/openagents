# 144. See with C-RADIO

> **Status: unimplemented — design draft.** Nothing here runs yet.

C-RADIOv3-B is an NVIDIA image encoder that produces general visual features.
You can use those features to search for images and to train a small
classifier without training the encoder. This chapter executes the pinned
checkpoint in Psionic, retrieves visual assets with it, and trains a small
head over frozen features.

## What this chapter covers

- The pinned C-RADIOv3-B revision: exact preprocessing, patch size, position
  handling and the feature-output conventions (summary and spatial features).
- Feature extraction compared with recorded reference tensors for fixed
  images.
- Visual-asset retrieval: embed a fixed collection of images, query by image,
  and compare results with exhaustive search.
- A small Psionic-trained head over frozen features, evaluated on held-out
  images and compared with a constant predictor.
- License handling: the weights use NVIDIA model terms, and the RADIO source
  license limits that code and derivatives to noncommercial research and
  evaluation. You read the source as a reference only and write the Coax
  implementation separately.

## Dependencies

- [Chapter 142](ch142-pixels-convolutions-and-patches.md): preprocessing and
  patch projection.
- [Chapter 10](ch010-attention-without-training.md): the shared attention
  core.
- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): training the head.
- [Chapter 54](ch054-search-the-coax-book.md): exhaustive vector search as
  the retrieval baseline.

## Done means

- Features for fixed images match the recorded reference within a stated
  tolerance.
- Retrieval reports recall at fixed cutoffs on a held-out query set.
- The trained head beats a constant predictor on held-out images, with the
  split and seeds recorded.
- The chapter states that features alone do not qualify any downstream task.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), C-RADIO entry and
  license notes.
