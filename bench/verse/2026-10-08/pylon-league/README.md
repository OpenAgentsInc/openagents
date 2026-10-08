# Pylon league on the Gym's EVALS board (#10922)

The EVALS board in the Grid's Gym shows the pylon league, rendered by the
desktop app's Grid over the real GPU layer. It has one block for each
hardware class, with the class's pinned suite and its digest. Each pylon's
row has its pass rate on that suite (pass/fail/inconclusive), accepted jobs,
median job time, cost per accepted job, standing, and sigil.

- `pylon-league.png`: 1280 × 800 at 2×.
- `pylon-league-1920x1080-1x.png`: 1920 × 1080 at 1×.

The data comes from `pylon::fixture::League`, an in-process loopback relay
with five real providers on fake engines:

- Four of them answer the pinned canaries correctly.
- One echoes the prompt, so it fails.

A checker key runs the canaries and one redundant job through the normal job
path, and a buyer sends two jobs to the CPU box, which no checker ever
checks. The Gym reads this data with `pylon::league::fetch`, the same call
that `openagents pylon league` makes, through `verse::gym_league`. No live
relay is involved. Median times are 0 s and every job is free because the
engines are fakes.

To reproduce, run on a machine with any GPU adapter (Metal is enough):

```sh
OPENAGENTS_GRID_EVIDENCE=<dir> cargo test -p openagents-desktop \
  pylon_league_on_the_gym_evals_board --bin openagents-desktop -- --ignored
```

Before the test pins the fixture relay, it checks the offline state: the
fixture Grid has no relay, and the board says so. The empty, unreachable,
and offline states are covered by `gym_league::tests` in `crates/verse-gym`.
