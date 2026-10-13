# Draconia parity log

## Peacock scene

The peacock guardian (reference 113.0–118.0 s, [shots.md](shots.md)),
rendered by `crates/verse/examples/film_shot.rs` scene kind `peacock` from
`docs/verse/film/verse-shots/draconia-peacock-feathers.json` (P1+P2) and
`draconia-peacock-tail.json` (P3). Internal study only. Review files:
`~/Downloads/draconia-peacock-vN.mp4`, `-vN-side-by-side.mp4` (reference on
top), and `~/Downloads/draconia-peacock-progress.png` (one row per version).

### v1, 2026-10-13

- **Built:** the `feather` sprite sheet (a feather of light and a soft dot,
  `scripts/blender/fx/feather.py`), `verse::fx::feathers` (one feather, a dot,
  and a fanned tail), the effects `guardian_flash`, `ground_mist`,
  `night_embers` and `ruin_fire`, and the `peacock` scene: a starry void, a
  neon stage with cobbles, ruined walls, fires, and Alice recolored as
  Draconia (platinum hair band, the green coat to black).
- **Matches:** the cut timing (P1 1.3 s, the flash, P3 from 1.5 s), 21:9 at
  1920×822, the push in to one eyespot and the white-out, the wide 28 mm hold
  with a slight ease back, the fan's half-ellipse filling the frame, her
  centred backlit figure, fires and embers at the edges.
- **Gaps:** feathers are straight sprites, so P1 lacks the reference's curved
  shafts, fine dense barbs and rainbow fringe; P3's fan washes toward one
  white mass instead of crisp rows of eyespots on dark gaps; she is Alice's
  explorer outfit recolored (a staff on her back, no skirt or tights), standing
  from the start rather than rising from a crouch, with no hair lift; the face
  close-up at 1.5 s is our overexposed Alice, not the reference's eyes.

### v2, 2026-10-13

- **Gap fixed:** the tail washed into one white sheet. `feathers::Fan` now
  caps outer feathers at half the fan's radius, so eyespots stay apart along
  each arc, and takes a `vane` share: vanes and shafts burn at 0.42 of the
  eyespots' light, with a bright point on every eyespot. The fan has 175
  feathers in 9 rows over a faint haze.
- **Now matches:** crisp rows of white eyespots on darker gaps, with shafts
  converging on a white-hot core behind her silhouette, as in the reference.
- **Still open:** curved P1 feathers and the rainbow fringe; the haze between
  the reference's eyespots is brighter and bluer; her outfit, the rise from a
  crouch, and the lifting hair.

### v3, 2026-10-13

- **Gaps fixed:**
  - P1's feathers were straight sticks. `feathers::curved` now draws the head
    as a sprite, with the shaft and lower vane as ribbons along a curve, so
    the feathers sweep in arcs. `feathers::rainbow` adds the soft rainbow
    fringe low across the frame.
  - P3: she was front-lit. The tail now backlights her into a near-black
    silhouette: no key light, no lamps near her, the cobbles lit by low lamps
    at the fan's sides, and a pool of light under the core.
  - Alice's staff no longer shows (its vertices collapse every frame).
  - The P3 framing is closer: she's larger, and the fan (radius 7.6 m) reaches
    near the top of the frame.
- **Still open:**
  - the P3 open: the reference rises from a crouch with her hair flying up;
  - her outline is Alice's coat, not the skirt, tights and boots (waiting for
    the face agent's Draconia asset);
  - P1's barbs are sparser and less fine than the reference's;
  - the reference's walls and fires read more clearly at the frame edges.
