# Everglade II Healer’s Kit v1

`U1 SEQ` asks to stabilize a dying ally with a Healer’s Kit. This extension
uses the Everglade session v5 transport. The sequence is positive and
increases for each request on a connection. The page supplies no target,
position, kit count or condition.

The room selects the nearest live, placed ally eligible for stabilization
within 5 feet in three dimensions. Ties use the lower seat index. The
room refuses blocked players and checks world line of sight to its chosen
patient. The shared engine spends an Action and one use, and
the patient becomes Stable at 0 HP. Each new Everglade II room seat has
one kit with ten uses. Class changes preserve its remaining uses.

The sender receives `u1 SEQ LEFT` after success or `r SEQ kit` on
refusal. Other pages receive the usual authoritative state updates.
