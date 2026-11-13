# Audio session ownership

`audio-session.coax` owns the v1 audio clock's gesture, coverage and terminal
engine lifetime. Construct with `has_open()`. `has_offer(state[])` returns
`HostAudioSessionOffer {active, started, retired}`; `active` requires a
started, uncovered, unretired owner. `has_event(state[], kind)` accepts:

| Kind | Observed platform fact |
| --- | --- |
| 1 | Actual input gesture |
| 2 | World covered or hidden |
| 3 | World uncovered or shown |
| 4 | Engine owner retired |

Coverage preserves the gesture fact and the exact PCM/block cursor. A
gesture behind coverage records `started` while leaving the device clock
inactive. Unknown events preserve state. Retirement is terminal: a later
gesture, uncover or old callback cannot restart that owner.

The adapter captures `offer.started` when each command is offered, so an
earlier refused one-shot cannot inherit a later gesture. Controls and beds
retain their existing shared PCM semantics. The SDK executes `offer.active`
on the concrete device and checks it before rendering a callback. It must
quiesce and destroy that device before freeing PCM or the Coax state.

The root authority gate kills 33/33 mutants, with no equivalent annotations.
