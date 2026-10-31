# 34. Importing robot models

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R17 (spec stage 10).

A robot description file says how a robot's links and joints fit together.
This chapter plans how Coax will import a published URDF subset into checked
data with frame and unit types, validate it, and refuse anything it doesn't
understand by name.

## What this chapter will cover

- The URDF subset: links, joints, limits, inertias, geometry references and
  frames.
- Generated declarations through R5 derivation.
- Validation rules and their refusals.
- Provenance of the source file and its meshes.
- Reproducible import.

## Outline

1. **The subset.** Links, joints, limits, inertias, geometry references and
   frames, imported as R12 frame and unit types
   ([chapter 29](ch29-units-and-frames.md)).
2. **Generated code.** R5 derivation produces the declarations, so the model
   is ordinary checked Coax data.
3. **Validation.**
   - The frames form a tree with no cycle and no missing frame.
   - Inertias are positive and physically consistent.
   - Joint limits are ordered, and joint axes are valid.
4. **Refusals.** An unsupported element is refused by name and never
   dropped.
5. **Provenance.** The import records the identity of the source file and
   its meshes.
6. **Reproducibility.** The same input bytes give the same output.
7. **Other formats.** MJCF and others are qualified separately; Aethyr
   [chapter 35](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch035-bring-in-robots-and-meshes.md)
   plans that library work.

## The consumer and the gate

- Import one pinned arm description used by the simulation (the Panda arm of
  [Aethyr chapter 19](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch019-joints-in-generalized-coordinates.md)),
  and compare every value with an independent parser.
- Malformed fixtures: a cycle in the joint tree, a missing frame, a
  non-positive or inconsistent inertia, an inverted limit and an unknown
  element.
- Import is reproducible from the same input bytes.

## Done when

- R17 ships with those fixtures.
- The pinned arm imports and matches the independent parser value for
  value.
- This chapter's examples, including each malformed fixture with its
  `.status` file, run under `check-examples.sh`.

## Dependencies

- R2 errors ([chapter 7](ch07-errors.md)), R5 derivation
  ([chapter 13](ch13-data-layout.md)) and R12 frames and units
  ([chapter 29](ch29-units-and-frames.md)).
- Used by [chapter 35](ch35-sensors-and-actuators.md)'s limits and by
  [Psionic chapters 167–170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch167-command-a-robot.md).
