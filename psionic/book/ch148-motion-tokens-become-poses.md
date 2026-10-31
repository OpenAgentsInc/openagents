# 148. Motion tokens become poses

> **Status: unimplemented — design draft.** Nothing here runs yet.

MotionBricks generates motion as discrete tokens and then decodes them into
poses. This chapter builds a tiny version of both halves: a codebook decoder
that turns tokens into poses, and masked-token generation that fills in
missing tokens. You check it on cases where the right answer is known.

## What this chapter covers

- Vector quantization: a codebook of pose-segment vectors and a nearest-entry
  encoder.
- A small VQ decoder that maps a token sequence to a pose sequence.
- Masked-token generation: start from masked tokens, predict them, and
  unmask over a fixed schedule.
- Known token-to-pose fixtures: tokens whose decoded poses are written by
  hand.
- Pose continuity checks across token boundaries: no jumps in joint angles
  or root position beyond a stated bound.

## Dependencies

- [Chapter 147](ch147-poses-have-conventions.md): pose and skeleton
  conventions.
- [Chapter 10](ch010-attention-without-training.md): attention for the
  masked-token model.
- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): training the tiny models.

## Done means

- Known tokens decode to their hand-written poses exactly or within a stated
  tolerance.
- Masked generation on a tiny trained model reproduces held-out sequences at
  a reported rate.
- Every decoded sequence passes the continuity checks.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
