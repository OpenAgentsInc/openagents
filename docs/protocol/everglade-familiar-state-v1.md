# Everglade II familiar state v1

`m2 TICK COUNT [OWNER FORM X Z HP MAX]...` publishes an Everglade II
room's visible familiars alongside the Everglade session v5 transport.
Each row gives the owner seat, the Wild Companion v1 form index, world X
and Z in integer millimetres, and current and maximum Hit Points.

Rows are sorted by owner seat. The room omits unplaced, retired and
mutually blocked owners. This is a complete snapshot, so omitted rows
disappear and a count of zero clears the snapshot. The room sends it on
changes and refreshes it periodically. The client applies only a fully
valid snapshot with a newer room tick.
