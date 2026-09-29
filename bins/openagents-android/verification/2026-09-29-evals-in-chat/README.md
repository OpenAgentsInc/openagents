# The Gym in chat on Android

Emulator record for [#9939](https://github.com/OpenAgentsInc/openagents/issues/9939)
(`oa_chat1st`, arm64, a debug build of this change). See the
[iOS record](../../../openagents-ios/verification/2026-09-29-evals-in-chat/README.md)
for what each screen is.

- Live chat worker: [`SCR-02-choose-your-agent.png`](SCR-02-choose-your-agent.png),
  [`CIN-01-end-card.png`](CIN-01-end-card.png), and
  [`SCR-15-E12-first-run-chat.png`](SCR-15-E12-first-run-chat.png) (the
  worker's Project map card; no published test set yet, so **Skip for
  now**).
- `--ez gym_fixture true` (debug builds only; the recorded Gym and report,
  fixture numbers): the first run's three taps to a test
  ([`fixture-CARD-01-first-run-tool.png`](fixture-CARD-01-first-run-tool.png),
  [`fixture-CARD-03-run.png`](fixture-CARD-03-run.png)), the result and its
  detail ([`fixture-CARD-04-result.png`](fixture-CARD-04-result.png),
  [`fixture-SCR-05-result.png`](fixture-SCR-05-result.png)), Add to the Gym
  ([`fixture-SCR-20-add-to-the-gym.png`](fixture-SCR-20-add-to-the-gym.png),
  [`fixture-SCR-20-added.png`](fixture-SCR-20-added.png)), and the main menu
  the guided path ends on
  ([`fixture-SCR-01-after-first-run.png`](fixture-SCR-01-after-first-run.png)).
