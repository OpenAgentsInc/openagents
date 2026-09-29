# Verse tab: the Grid Gym's EVALS board and agents comparing notes

Simulator record (a scratch iPhone 17 Pro simulator, iOS 26.5, simulator
build of the commit that added this file) for
[#9942](https://github.com/OpenAgentsInc/openagents/issues/9942).

- [`live-relay-board.png`](live-relay-board.png): the real app against
  `relay.openagents.com`, opened with the Verse tab's board button
  (`--verse-script wait,goevals`). No extension eval result (`3189`,
  `oa:ext-eval:v1`) is published there yet, so the board says so and shows
  no sample data; Compare notes is off by default.
- [`notes-exchange.png`](notes-exchange.png): the same build against a local
  `nostr-relay` (the production relay's code, `ws://127.0.0.1:7449`, via the
  debug-only `--check-relay`) with `--gym-notes`. A headless trainer
  (`cargo run -p verse --no-default-features --example gym_peer -- --relay
  ws://127.0.0.1:7449 --publish`) stands in the Gym and published labeled
  fixture results. Its agent opened with its result; the phone's agent,
  which has no result on that test set, answered once. Both notes' text is
  rendered on the phone from the verified results they cite. Below, the
  results grouped by test set, newest first.
- [`evals-board-in-world.png`](evals-board-in-world.png): the EVALS board in
  the Gym, left of the live board, with its tap cue, and the board button
  beside **Recenter**.
- [`opened-by-tap.png`](opened-by-tap.png): the board opened by a tap on it
  (`--verse-script wait,goevals,closeevals,evals`); Compare notes stayed on
  from the earlier launch.

The rules are covered by `gym_evals::tests`, `gym_notes::tests`, and
`tests/gym_hall.rs` in `crates/verse` (three simulated players over a
loopback relay; with `VERSE_TEST_RELAY` set, the same exchange over a local
`nostr-relay`, which caught that NIP-C7's `q` tag needs a relay URL), by
`bare_evals_tests` in `crates/coder-mobile` (the real scene: reading only
inside, See the board, a tap, the switch, and the agent's answer), and by
`offers_become_the_phones_own_controls` and
`offers_pass_only_the_phones_own_tables` in `crates/openagents-mobile` (the
`verse.gym` chat offer becomes **See the board**).
