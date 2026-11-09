# Everglade II Find Familiar v1

`M3 SEQ FORM SLOT SPIRIT` requests Wizard Find Familiar over Everglade
session v5. `SEQ` is positive and increases across familiar requests.
`FORM` uses the Wild Companion v1 index. `SLOT` is zero for the
seventy-minute ritual or a spell slot level for the one-hour prepared cast.
`SPIRIT` is 1 for Fey, 2 for Celestial, or 3 for Fiend.

The room replies to the owner with `r SEQ familiar` on refusal to start,
or `m3 SEQ STATUS` as the cast progresses. Status 1 starts the cast,
2 completes it, and 3 ends it without payment. The room holds the
Concentration and spends the saved Magic action each turn. On completion
it derives a destination from the owner's current position and facing,
checks world clearance, and spends incense and any slot through the
shared engine. Disconnecting interrupts an unfinished cast. A successful
cast appears in the separate familiar state snapshot.
