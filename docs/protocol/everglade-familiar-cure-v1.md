# Everglade II familiar Cure Wounds v1

`M6 SEQ SEAT` requests delivery of the owner's level-one Cure Wounds
through its familiar over Everglade session v5. The positive sequence
increases across familiar requests. SEAT identifies a living wounded
player in the same room, including the owner.

The room checks visibility permissions and collision clearance from the
familiar to the target. Shared rules check the spell, saved Magic action,
slot, link, reach and familiar Reaction. Success replies `m6 SEQ` to
the owner. Refusal replies `r SEQ familiar` and spends no resources.
Player health continues to come from the room's player snapshot.
