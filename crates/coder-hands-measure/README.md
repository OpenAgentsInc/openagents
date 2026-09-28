# coder-hands-measure

Record a labelled run of hand gestures, then score the desk's gesture
rules and the seam beside them against it. The rules are
`crates/coder-hands/src/gestures.rs`, the seam is
`crates/coder-hands/src/judge.rs`, and this package reads both rather
than a copy of them, so a constant or a floor that moves is scored by
replaying runs already recorded.

It builds on macOS and on Linux. A CoderOS host builds it from a checkout
with `cargo run -p coder-hands-measure`.

## The three commands

```sh
coder-hands-measure record [--out <file>] [--passes <n>]
coder-hands-measure score <run> [--answers <file>] [--windows]
coder-hands-measure ask <run> [--answers <file>]
```

`record` prompts for one gesture at a time, counts you in, and writes
every landmark line the camera published with the prompt as its label.
`score` replays the run through the rules and prints what they decided.
`ask` sends the windows the rules could not settle to the seam once and
writes the answers beside the run, so a later score reads the file rather
than the network. `docs/os/camera-and-hands.md` says what to do at the camera and for
how long.

## The camera

`src/camera.rs` reads the CoderOS camera daemon's hands socket. In the
private Coder repository a Mac also recorded from its own camera through a
macOS tracker crate, which did not move here, so this package records on a
CoderOS host alone. A run recorded on a Mac still scores, because scoring
reads the file.

The run's header names the camera it came from. A run recorded from the
daemon and one recorded from a Mac are two measurements, not one: the
cameras run at different rates and frame a person differently, so the
report prints the source rather than letting the two be read as the same
thing.

Everything after the recording reads the file. Scoring is a function of
the run alone, and `bench/golden/hands/runs/synthetic-run.report.txt` is
what scoring the synthetic run prints; a test fails when a score stops
producing it byte for byte, on either platform.

## Tests

```sh
cargo test -p coder-hands-measure
```

They need no camera and no key: the recorder's tests feed it frames over
the channel its reader would send them on, and the scorer's tests read
the runs and the answers under `bench/golden/hands/runs/`.
