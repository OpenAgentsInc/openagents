# Aethyr engine core

The primitives every realm shares, in [`aethyr/`](../../aethyr/README.md)
(GitHub #12609, the verse project's milestone "Engine core"). Each has its own
checks; where it decides behaviour, `coax mutate` leaves no survivors. The
plan they follow is [docs/research/game-engine-systems.md](../research/game-engine-systems.md),
sections A and D, and coax/ROADMAP.md R12, R13 and R16.

## Loop (`aethyr/loop.coax`)

One clock owns time. A realm hands it each frame's time once; it says how
many fixed steps to run and how far the frame lies between the last two.

- Time is whole nanoseconds, so the steps a run takes depend only on the
  frame times fed in, not on float rounding in a running total.
- A frame counts for at most `max_frame_ns` (0.25 s by default), at most
  `max_steps` (8) run in one frame, and a backlog past that is dropped and
  counted in `dropped_ns`: a stall never makes the next frame slower.
  Non-number and infinite frame times count as none.
- `loop_alpha` is what drawing mixes the last two steps by.

The player controller (`verse/player.coax`, every walking realm) steps on it
at 1/64 s, and Orbit League at 1/120 s. Checks: a run and jump passes through
the same states, bit for bit, at 30, 60, 120 and 144 FPS; drawn at 60 FPS, a
jump's feet change their per-frame move by at most 0.008 m (gravity alone is
0.0067 m; drawing the last step instead lurches by 0.077 m) and its camera by
at most 0.014 m (0.062 m).
