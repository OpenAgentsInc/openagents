# Direct entry after Character Create

A completed shared CC2 appearance was loaded and drawn by Everglade, but the
legacy arrival picker still checked only whether the ticket contained an old
Ranger look. That opened the extra name/class/skin/cloak/trim dialog and held
the socket for Enter. The arrival-preview route had a second automatic opener.
The previous browser journey pressed Enter whenever this dialog appeared,
which hid the regression.

A validated shared profile now suppresses both automatic openers. If its bytes
arrive after the automatic dialog opens, successful restoration releases that
hold and clears its pending pointer state. The profile does not need its meshes
to finish loading to count as completed creation. Invalid records do not release
the fallback; an intentionally opened manual picker remains open. Room choice,
server ticket/name and room-seat requirements remain unchanged.

Regression coverage exercises profile before/after ticket, before network
attachment, before/after arrival facts, invalid saves and manual editing. The
browser test now waits for the saved avatar AND an actual room seat without
sending Enter, then verifies reload, movement and animation. A duplicate picker
is a test failure rather than something the harness dismisses.

The local preview update contains generated Wasm artifacts only; implementation,
builds and tests were performed in the asset-library worktree. The owner's
existing tab needs a reload to replace its already-instantiated old module.
