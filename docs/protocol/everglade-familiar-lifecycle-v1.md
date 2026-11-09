# Everglade II familiar lifecycle v1

`M4 SEQ ORDER` requests an action for the sending seat's familiar over
Everglade session v5. The positive sequence increases across familiar
requests. The order is 1 to pocket, 2 to recall, or 3 to release.

The room resolves the request on the owner's saved turn through the
shared Magic action rules. Pocketing removes the familiar from the map
and preserves its Hit Points. Recall uses a clear space selected by the
room near the owner. Release ends the familiar, including a pocketed one.

Success replies to the owner with `m4 SEQ ORDER`. Refusal replies
`r SEQ familiar` and leaves the action unspent. Visible position and
Hit Points continue to come from the separate familiar state snapshot.
