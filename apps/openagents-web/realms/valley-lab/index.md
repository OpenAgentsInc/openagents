# Valley Lab

An unlisted Aethyr workbench at `/worlds/valley-lab`, written in Coax.
The first scene is an original sandstone graybox: a basin, an arch, stone
steps and a visible block traveller. It uses Aethyr's shared fixed-step
character controller, capsule collision, follow camera and touch sticks.

- WASD moves; Shift runs; Space jumps.
- Arrow keys turn and tilt the camera; the wheel changes camera distance.
- On touch screens, drag on the left to move, drag on the right to look,
  and tap on the right to jump. Push the movement stick fully to run.
- Press R or tap RESET to return to the starting point. Escape releases input.

The boxes drawn in the scene are the same boxes used by collision. The
traveller is a placeholder, and this first scene has no imported assets,
virtualized geometry, streaming or encounter yet. The implementation plan
is `docs/verse/2026-11-05-valley-lab.md` in the repository.
