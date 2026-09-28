# Verse tab: the Grid Gym's RESULTS board

Simulator record (iPhone 17, iOS 26.5, simulator build of `main` at the
commit that added this file), reading the live publication on `main`
(four boards, digest `9ccdd810`), launched with:

```sh
xcrun simctl launch <udid> com.openagents.app --tab verse --gym-preview \
  --verse-script walk,walk,walk,right,wait,results,wait[,r=board:<id>[,r=attempt:<id>]]
```

- [`boards.png`](boards.png): the boards list in publication order, each
  headline verbatim with its labels; the TB2.1 headline's dollar figure is
  followed by the list-price note.
- [`board.png`](board.png): `tb4-fable-delegate-repro-9776`: the headline,
  labels, the caveat count with the first caveat, every split with whole
  denominators, spend with the bound labeled, and the bar's reference.
- [`attempt.png`](attempt.png): `coq-block-bound.p2`, a beat: header with
  labels, cost and time against the bar with ratios, the in-sample caveat,
  the reference rule with its conditions one tap away, and phases.

The load, entry, exit, and tap rules are covered by
`bare_results_tests` in `crates/coder-mobile`, the loader by
`gym_results::tests` in `crates/verse`, and the presentation rules by
`view::tests` in `crates/gym-leaderboard`.
