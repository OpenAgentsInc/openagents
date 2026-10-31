# 161. Forecast on a sphere

> **Status: unimplemented — design draft.** Nothing here runs yet.

FourCastNet3 forecasts global weather on a sphere. This is an optional
branch. It needs spherical operators that flat-grid models do not, and it
does not wait for DoMINO.

## What this chapter covers

- The pinned `fourcastnet3` revision.
- Spherical operators: spherical harmonic transforms and spherical
  convolutions, with references on small grids.
- Variable and grid conventions: the model reads 72 variables on a
  721×1440 latitude-longitude grid.
- Geographic weighting: latitude-weighted error metrics.
- Multi-step forecasts: the model's output becomes its next input, and error
  is reported for each lead time.
- Activation and memory budgets for full-size fields, stated before running.
- ERA5 data acquisition and terms need their own qualification before any
  data is used.

## Dependencies

- [Chapter 156](ch156-a-spectral-neural-operator.md): spectral layers.
- [Chapter 151](ch151-waveforms-and-spectral-features.md): the FFT core.

## Done means

- Spherical transforms round-trip within a stated tolerance on small grids.
- A single forecast step matches a recorded reference output.
- Latitude-weighted error is reported for each lead time on a fixed
  evaluation set whose terms are recorded.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), FourCastNet3 entry.
