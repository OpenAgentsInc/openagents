# 143. Elephant and Mammoth see

> **Status: unimplemented — design draft.** Nothing here runs yet.

Elephant and Mammoth are Qwen3.5 models, and their published weights accept
images. The text profile of chapters 14 and 15 admits the 333 vision tensors
in each checkpoint as inventory only. It loads no vision encoder or projector
and refuses any request with an image. This chapter adds the image path, so a
decision request can carry an image.

## What this chapter covers

- The pinned Qwen3.5 vision encoder: its patch embedding, transformer blocks
  and output merge, read from the same authenticated files as the text
  weights.
- The projector that maps vision features into the language model's hidden
  size.
- Exact image preprocessing for this model family, stated as a chapter 142
  profile: resize rule, value range, normalization and patch size.
- Image token layout: where image tokens sit in the rendered request, how
  many tokens an image of a given size produces, and how chapter 9's
  rendering marks them.
- Multimodal rotary positions: the position indices that image tokens and the
  text around them receive.
- Image-bearing decision requests with their own reference captures, kept
  separate from the text-only evidence.

## Dependencies

- [Chapter 142](ch142-pixels-convolutions-and-patches.md): preprocessing,
  convolution and patches.
- [Chapters 14](ch014-execute-elephant.md) and
  [15](ch015-execute-mammoth.md): the text decision path that the image
  tokens join.
- [Chapter 9](ch009-text-becomes-a-model-request.md): request rendering and
  special tokens.
- [Chapter 16](ch016-are-the-probabilities-any-good.md) and
  [chapter 17](ch017-qualify-honestly.md): quality measurement and
  qualification rules.

## Done means

- The vision encoder's output and the projector's output match recorded
  reference tensors for fixed images within a tolerance frozen before
  comparison.
- A complete image-bearing decision on Elephant, then Mammoth, matches its
  reference capture under chapter 17's rules.
- Text-only requests produce the same results as before; the image path does
  not change the text profile's evidence.
- Text-only qualification is never reported as image qualification, and the
  image profile has its own model identity in every answer.
- Requests with an image outside the admitted sizes are refused with a typed
  reason.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [Model targets](../docs/targets/model-targets.md).
