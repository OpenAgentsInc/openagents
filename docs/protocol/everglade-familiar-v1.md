# Everglade II Wild Companion v1

`M1 SEQ FORM SLOT` requests Wild Companion in an Everglade II room over
the Everglade session v5 transport. `SEQ` is positive and increasing.
`FORM` is a zero-based index in this order. Cat, Rat, Spider, Bat, Frog,
Hawk, Lizard, Octopus, Owl, Raven, Weasel. `SLOT` is zero for a Wild
Shape use or the level of a spell slot.

The room derives a destination from the owner's admitted position and
facing, then asks the world for line of sight. The shared rules admit the
form, destination, Magic action and resource. Success responds to the
owner with `m1 SEQ FORM`. Refusal responds with `r SEQ familiar`.
The familiar owns a room actor and an independent turn. A completed
Long Rest, seat retirement or preset replacement removes it.
