# 160. Execute DoMINO

> **Status: unimplemented — design draft.** Nothing here runs yet.

DoMINO predicts the flow field around a vehicle from its geometry. NVIDIA
publishes `domino_drivaerml`, trained on the DrivAerML car dataset. This
chapter executes that checkpoint on a bounded subset of the data and measures
physical error against the dataset's simulations.

## What this chapter covers

- The pinned `domino_drivaerml` revision.
- Exact preprocessing: geometry normalization, signed-distance grids,
  surface and volume sample points.
- The geometry operators from chapter 158: point neighborhoods, point
  convolutions and local aggregation.
- Surface outputs (for example pressure and wall shear stress) and volume
  outputs (for example velocity and pressure).
- A bounded DrivAerML subset. The full dataset is about 31 TB across 500
  variants under CC BY-SA 4.0. You select a small, hashed subset and respect
  the published splits.

## Dependencies

- [Chapter 158](ch158-a-field-around-a-shape.md): geometry operators.
- [Chapter 156](ch156-a-spectral-neural-operator.md): field-learning
  background.

## Done means

- Intermediate tensors and outputs for fixed geometries match recorded
  reference outputs within a tolerance frozen before comparison.
- Physical error measures (for example relative L2 error of surface pressure
  and integrated drag) are reported on held-out vehicles from the published
  test split.
- The chapter states that the model does not replace a solver or Verse's
  authoritative simulation without a separate consumer contract.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), DoMINO and DrivAerML
  entries.
