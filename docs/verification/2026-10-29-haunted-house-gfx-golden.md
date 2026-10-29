# Haunted House graphics-control golden

Commit `86e0f4f69e3ba5b5973d55b998d2b4f0df8bb6d9` added a GFX button to
the Haunted House hotbar, shifting its neighboring controls. The normal
staging release walk at `d5fc7e5832` correctly refused the previous golden:
92 changed blocks, with differences confined to x 744–1168 and y 736–784.

The replacement is the unmodified 1280×800 screenshot captured by that walk.
An independent pixel comparison with the previous reference found 3,635 changed
pixels inside x 720–1279, y 736–783, and **zero changed pixels outside that
area**. The visible change is the intended additional control and its adjacent
button positions. No tolerance, mask, walk step or smoke check is relaxed. The
normal full staging and production walks remain the release gate.
