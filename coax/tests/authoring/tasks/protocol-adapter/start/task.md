# Update a protocol adapter: the relay's first frame says who is in

The relay now sends `y SEAT N` as a player's first frame: N is how many others
were already in the room (one digit). Give `Frame` an `others` field and read
N into it. The old `y SEAT` (from a relay not yet redeployed) must still read
as F_IN, with `others` -1 (not known), as on every other frame. Nothing else in
the protocol changes.

Edit only `frame.coax`. Check with `python3 coax/tests/authoring.py check DIR`.
