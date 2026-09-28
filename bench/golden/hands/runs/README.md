# Recorded hand runs

A run is one scripted minute at a camera: `coder-hands-measure record`
prompts for a gesture, counts you in, and writes every landmark line the
camera published with the prompt as its label. The first line of the file
is the header, and every line after it is one frame. The header names the
camera: `daemon` for the CoderOS camera daemon and `vision` for a Mac's
own, because the two are two measurements rather than one.

`coder-hands-measure score <run>` replays a run through the rules in
`crates/coder-hands/src/gestures.rs` and prints what they decided. The
numbers are a function of the run file alone, so a run scores the same on
either platform; `synthetic-run.report.txt` beside the runs is that
report, and a test fails when scoring the run stops producing it byte for
byte. `coder-hands-measure ask <run>` sends the windows the trigger asks about
to the seam in `crates/coder-hands/src/judge.rs` once and writes the
answers to `<run>.answers.jsonl`, which the next score reads instead of
the network. It asks about the same windows the desk would, so the corpus
and the desk see one trigger rather than two. `docs/os/camera-and-hands.md` says what the owner runs and for
how long.

The runs here are what a change to a constant or a floor is scored
against, so nobody has to wave at the camera again:

| File | What it holds |
| --- | --- |
| `empty-room-recorded.jsonl` | 425 frames from the camera daemon on the owner's CoderOS host with nobody in the room, at 14.7 frames a second. The rules act on none of it and the seam is asked nothing. |
| `synthetic-run.jsonl` | The four cases beside this directory, relabelled as one run of four cues with a count-in before each. The rules act on every gesture, three windows come out ambiguous, and the trigger asks about two of them and holds one back. |
| `synthetic-run.jsonl.answers.jsonl` | Answers for the two windows the trigger asks about, written by hand so the scorer's own tests need no key. |
| `synthetic-run.report.txt` | What scoring `synthetic-run.jsonl` prints. Regenerate it with `coder-hands-measure score bench/golden/hands/runs/synthetic-run.jsonl` in the commit that moves a constant, a floor, or the trigger. |

A run you record lands in `~/.openagents/hands/runs/`. Copy it here to
keep it.
