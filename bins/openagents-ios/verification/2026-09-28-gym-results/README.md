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

## Trace viewer

Opened from an attempt with `r=trace`, then `r=tab:<tab>` (and `r=seek`,
`r=expand` on the Agent tab):

- [`trace-jev.png`](trace-jev.png): `coq-block-bound.p2` (a beat), Jev:
  every candidate's probability as a bar against the 0.5 keep line, kept
  rows bold, **own** marked; requirements follow with the 0.7 flag line.
- [`trace-briefing.png`](trace-briefing.png): the briefing the delegate
  received.
- [`trace-agent.png`](trace-agent.png): seeked to 60% of the clock; the
  running token counter at the playhead, reached rows bright and later ones
  dim, the current row outlined, and one command's output expanded.
- [`trace-verifier.png`](trace-verifier.png): each test's status and the
  output tail.
- [`trace-deadline-agent.png`](trace-deadline-agent.png):
  `batched-eval-parity.p1`, stopped at its deadline: the header says the
  cost is unknown with its bound labeled, and a cut command says
  "cut from 4546 bytes".

`gym_results::tests::every_published_trace_opens_on_demand` opens every
published bundle through the panel, verified against its `TraceRef` and
cached, and renders each tab under 256 KiB. A frame packet carries only the
panel's revision, and playback bumps it only when the current row changes.
