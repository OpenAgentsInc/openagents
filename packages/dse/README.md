# @openagentsinc/dse

`dse` is an Effect-first library for declarative, optimizable LM “programs”:

- **Signatures** are typed contracts (Effect `Schema` IO + Prompt IR).
- **Modules** are Effect programs (`I -> Effect<R, E, O>`).
- **Predict** is the minimal runtime bridge: Signature + Policy -> prompt -> model -> decoded output.

This package is intentionally small at first. The corresponding spec lives at `docs/autopilot/dse.md`.

