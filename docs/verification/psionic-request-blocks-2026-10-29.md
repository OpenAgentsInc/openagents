# Psionic request block verification, 2026-10-29

`qwen-request.coax` starts a fresh hybrid state, creates text rotary
coefficients and executes a prefix followed by a suffix. It publishes live
state and option probabilities only after successful readout. The caller is
responsible for the authenticated model, exact token stream, split, option
slots, storage and output association.

On the development Mac, 152 Coax checks passed in debug and release and all
3,663 mutation candidates were killed. Native debug and release checks passed
normally and with ASan/UBSan; native release checks also passed on CoderOS.
The tests exercise nonzero attention layers, distinct cache segments, positions,
stale workspace, bounds and failure isolation. See the chapter 10 verification
record for the separately executed original-weight request.
