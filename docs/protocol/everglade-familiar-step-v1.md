# Everglade II familiar Step v1

`M5 SEQ` requests one grid step in the direction the owner faces over
Everglade session v5. The positive sequence increases across familiar
requests. The room uses its admitted facing and the familiar's current
square. It checks world collision along the route and resolves the step
using the familiar's saved turn, movement and telepathic link.

Success replies to the owner with `m5 SEQ`. Refusal replies
`r SEQ familiar`. The resulting position comes from the separate
familiar state snapshot.
