# Everglade Link input v1

The opt-in `J1 SEQ KX KY PX PY THETA B HEARD` record is a separate command
within session v5. It leaves published `I` semantics and version identity
unchanged. The machine-readable extension is
[`everglade-link-input-v1.json`](../../proto/verse/everglade-link-input-v1.json).
Field order, quantization, sequencing and acknowledgement units match `I`;
THETA is an integer in 0.1 milliradians (10,000 units per radian).

Bits 1–16 retain run, jump, dive, rise and sink. Bit 32 selects the shared Lab
profile and is required; 64 selects roll, 128 selects backflip, and 256 retains
active source jump gravity. Only bits under mask 511 are admitted. Roll and
backflip cannot coexist. Negative/unknown flags refuse. Ordinary I input still
clamps to 31 and cannot opt into the profile. THETA is the captured action
heading (roll sends facing + pi; backflip sends facing); normal movement sends
camera heading. Action movement axes are serialized too.

J1 uses the same travel/checkpoint/can-play guards and sequence queue as I.
Inputs retain their flags and quantized heading in client history. Both the
client and room call `eg_step_world`, `eg_step_begin` and `eg_step_advance` with
the same input, profile and existing pose/carry snapshot. There is no additional
unreplicated action impulse or hidden server motor state. Busy/still masks
remove action flags; aboard uses the existing boat controller. Spell conditions,
jump permissions, world collision and external forces retain their authority.

This experiment requires its matching local room worker (`offline=worker`).
There is no capability negotiation with older public workers and this change
does not deploy a public room. Toggle Link off to resume ordinary I inputs.
Admission has a bounded property check and 25 killed mutations, zero survivors;
the independent room check exercises J1 dispatch, conflicting flags and I's
unchanged clamp.
