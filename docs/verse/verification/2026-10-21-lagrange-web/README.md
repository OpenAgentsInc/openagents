# Lagrange 2 on the Grid homepage (#12438)

Headless Chrome under `openagents browser run`, against a local website
built from the fix, ran `scripts/dev/check-grid-home-lagrange.py`:

1. `1-grid-before`: the Grid on `/` with the Pylon labels and walking hint.
2. `2-load-screen-in`: walking into the Lagrange 2 arch puts up the load
   screen; its bar moves with each step (5, 40, 60, then 100 percent).
3. `3-lagrange2`: the zone with its panel (Slower, Faster, Pause, New ship,
   Forces, Art, The Grid) and no Pylon overlay. `A` turned the visitor
   without moving them, and `Space` climbed 2.1 m.
4. `4-lagrange2-forces`: the panel's **Forces** button turned the overlay
   on; the button now reads **Hide forces**.
5. `5-load-screen-back`: **The Grid** puts the load screen up again while
   the Grid's engine opens; this frame is its fade.
6. `6-grid-after`: back on the Grid, with the Pylon overlay restored.

`capture.json` holds each check, the `verse-zone` and `verse-zone-load`
events, and the load screen's samples. The software renderer spent about 17
seconds uploading the factory, so the screen stood at "Setting up the scene"
for most of the way in.
