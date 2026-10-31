# 162. A recurrence you can inspect

> **Status: unimplemented — design draft.** Nothing here runs yet.

State-space models such as Mamba-2 carry a fixed-size state from token to
token instead of attending to every earlier token. This chapter builds a tiny
state-space layer and proves that its three execution forms give the same
results. Chapter 11's Gated DeltaNet support does not establish Mamba-2
support, so this chapter starts from a small example.

## What this chapter covers

- A tiny state-space transition: state update, input projection and output
  readout, with values you can compute by hand.
- Causal convolution in front of the recurrence, with its own carried state.
- Three execution forms: token by token, chunked and whole sequence.
- Independent state reset between requests, so one request never reads
  another's state.

## Dependencies

- [Chapter 11](ch011-a-hybrid-sequence-model.md): causal convolution and a
  recurrent state.
- [Chapter 42](ch042-deltanet-in-chunks.md): chunked execution of a
  recurrence.

## Done means

- Token-step, chunked and whole-sequence outputs agree within a named profile
  on fixed inputs.
- State snapshots at chunk boundaries resume to the same output.
- Two requests run in sequence give the same results as each run alone.

## Sources

- [Chapter plan](PLAN.md), Part XVII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Nano entry.
