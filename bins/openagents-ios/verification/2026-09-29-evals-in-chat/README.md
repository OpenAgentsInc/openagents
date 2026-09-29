# The Gym in chat: menu, first run, cards, and sheets

Simulator record for [#9939](https://github.com/OpenAgentsInc/openagents/issues/9939)
(wireframe revision 3), on a scratch iPhone 17 Pro simulator (iOS 26.5),
simulator builds of this change.

## Against the live chat worker

The real app against `relay.openagents.com` and the deployed chat worker
(release `17c7484f9f`, `chat-router-v2`), from a fresh install:

- [`SCR-02-choose-your-agent.png`](SCR-02-choose-your-agent.png): step 1 of
  3, with no tab bar.
- [`CIN-01-end-card.png`](CIN-01-end-card.png): the end card, **LET'S GO**.
- [`SCR-15-E12-first-run-chat.png`](SCR-15-E12-first-run-chat.png): the
  first-run chat asked for Project map's test. The worker's tool card is
  drawn in the phone's words (**STEP 2 OF 3**, "Not tested yet. Be the
  first."). No test set is published yet, so there is no **START THE
  TEST**; the card offers **Write tests for Project map**, the other tools,
  and **Skip for now**, never a dead end.
- [`SCR-01-main-menu.png`](SCR-01-main-menu.png): the main menu after the
  first run: the trainer name from the world key, level and XP read from
  the relay's ledger (level 1, 0 of 100 XP), the next step, **CHAT WITH
  OPENAGENTS**, the starter chips, **PROFILE**, and **THE GYM IN THE VERSE**.
- [`CARD-05-news.png`](CARD-05-news.png): **What's new** sent from the menu,
  and the worker's news card, each item with its source line.
- [`CARD-07-credit.png`](CARD-07-credit.png): "What have I earned?": the
  worker's prepared answer, and the phone's own credit card from its
  ledger (nothing yet).
- [`SCR-11-profile.png`](SCR-11-profile.png): Profile, from the ledger.
- [`FLOW-07-make-a-tool-1.png`](FLOW-07-make-a-tool-1.png): "Help me make a
  tool that tells Coder how we write commit messages": the deployed
  interview hands the tool to Coder on a computer, so no draft card yet; the
  phone shows **Connect a computer**.

At that release, the live worker had no published eval result or test set yet (the next section is the real run once they were), so it offered
no `start_eval`, and a run, a result, and a check couldn't be reached from a
live chat ([#9943](https://github.com/OpenAgentsInc/openagents/issues/9943));
the interview handed chat-made tools to Coder, so the draft couldn't either
([#9945](https://github.com/OpenAgentsInc/openagents/issues/9945)). The
phone's hosted path itself is live: `live_the_runner_answers_the_phone` in
`crates/openagents-mobile/src/hosted.rs` sent the phone's signed request
from a fresh key to the deployed runner, which answered in 0.7 s
(`not_admitted` for a test set it doesn't run, before anything ran), and
the phone bound the answer and showed "Our test computers don't run this
tool or test set."

## A real hosted test, end to end

After the hosted runner ([#9935](https://github.com/OpenAgentsInc/openagents/issues/9935),
`0b39640d66`) and the chat worker release that offers the starter test
sets (`0546032e17`, [#9943](https://github.com/OpenAgentsInc/openagents/issues/9943))
were deployed, the same simulator app (Trainer 7GW) ran a real test.
Every number below comes from the runner's records:

- [`live-CARD-01-tool.png`](live-CARD-01-tool.png): "Test Project map on
  Coder": the worker's tool card with the published latest result and
  **START THE TEST** (release `8e4ae48b9d14…`, 6 tests × 3 runs × 2 sides).
- [`live-CARD-03-run.png`](live-CARD-03-run.png): after the tap, the
  phone's signed `25920` reached the runner; its progress, 35 of 36 runs.
- [`live-CARD-04-result.png`](live-CARD-04-result.png): the result about
  a minute after the tap, from the sealed report (`3188`) opened on the
  phone: 2 of 6 without the tool, 5 of 6 with it, **Better** (the
  report's own `ext-eval-v2` verdict).
- [`live-SCR-05-result.png`](live-SCR-05-result.png): each test on each
  side, by the majority of its runs.
- [`live-SCR-20-added.png`](live-SCR-20-added.png): **Add to the Gym**
  published it through the runner: `3189`
  `c4a9a2b54eea6698fb9978e79ce3d8d031ac83f6850f6961e1ff1944002c36c6`,
  naming the trainer and carrying their signed request.
- A second trainer checked it through the runner (5 of 6 against 2 of 6,
  **Better**; check `8f28f3a6e5bf…`), and the XP referee awarded 25 XP to
  Trainer 7GW (`3193` `b066590dca81…`).
- [`live-SCR-01-menu-credited.png`](live-SCR-01-menu-credited.png): the
  menu read the award from the phone's own ledger: 25 of 100 XP, "Your
  work was checked. +25 XP."
- [`live-SCR-11-profile-credited.png`](live-SCR-11-profile-credited.png):
  Profile's **What you made**: the result, checked by 1 trainer, +25 XP.

The chat's own **CARD-07** ("What have I earned?") couldn't be captured
live the same day: this trainer had used the chat worker's daily message
quota, and the phone said so ("Try again in 16 hours, or run Coder on
your computer"). The card draws from the same ledger as the menu and
Profile (`credit_comes_from_the_phones_own_ledger`); `CARD-07-credit.png`
above shows it live with nothing earned yet.

Found and fixed on the way: after a relaunch, **CHAT WITH OPENAGENTS**
now reopens the chat holding a result not yet added to the Gym, the one
the menu's next step names. The recorded Gym (`--gym-fixture`) now keeps
its chats and runs in their own stores, because its fixture results had
shown up in this simulator's Profile; those stores were cleared before
the Profile capture. Profile also leaves out an empty **Your results**
list when the ledger shows results (as after a reinstall).

## From the recorded Gym (`fixture-…`)

`--gym-fixture 1` (debug builds only) answers with the chat router's
recorded NIP-CJ card and offer bodies (`crates/coder/fixtures/nip-cj/`) and
runs tests on an offline runner that returns a recorded report
(`crates/openagents-mobile/fixtures/gym-report.json`). The numbers in these
files are fixture data:

- [`fixture-CARD-01-tool.png`](fixture-CARD-01-tool.png): the tool card with
  its latest result and **START THE TEST**.
- [`fixture-CARD-03-run.png`](fixture-CARD-03-run.png): the run card with
  one row of blocks (the runner counts runs across both sides).
- [`fixture-CARD-04-result.png`](fixture-CARD-04-result.png),
  [`fixture-SCR-05-result.png`](fixture-SCR-05-result.png): the result and
  its detail, each test on each side.
- [`fixture-SCR-20-add-to-the-gym.png`](fixture-SCR-20-add-to-the-gym.png),
  [`fixture-SCR-20-added.png`](fixture-SCR-20-added.png),
  [`fixture-CARD-04-added.png`](fixture-CARD-04-added.png): Add to the Gym
  lists what becomes public, publishes on its button, and the card says so.
- [`fixture-SCR-21-test-set.png`](fixture-SCR-21-test-set.png): the tests a
  run ran.
- [`fixture-CARD-06-check.png`](fixture-CARD-06-check.png): a result to
  check, **RUN THE CHECK**.
- [`fixture-CARD-02-draft.png`](fixture-CARD-02-draft.png),
  [`fixture-SCR-21-draft.png`](fixture-SCR-21-draft.png),
  [`fixture-CARD-02-try-it-once.png`](fixture-CARD-02-try-it-once.png),
  [`fixture-CARD-03-trying.png`](fixture-CARD-03-trying.png),
  [`fixture-CARD-04-first-try.png`](fixture-CARD-04-first-try.png): the
  interview's draft at a gate (**LOOKS GOOD**, **Change it**, **See every
  test**), the pilot step (**TRY IT ONCE**), the try running, and **FIRST
  TRY** with **RUN THE FULL TEST SET**.

The Android record of the same flows is in
[`bins/openagents-android/verification/2026-09-29-evals-in-chat/`](../../../openagents-android/verification/2026-09-29-evals-in-chat/).

## IDIOT PROOF, re-checked against this build

The wireframe's checklist (`CHK-01` to `CHK-13`) against what these builds
do, live and recorded:

| Check | Result |
| --- | --- |
| `CHK-01` One primary | Pass: each card, sheet, the menu, and each first-run step has one white button; a run card has none while it runs (as the table notes). |
| `CHK-02` No jargon | Pass on every label the phone draws (`no_label_uses_a_banned_word`). Fails in one worker news line, "Terminal-Bench" ([#9944](https://github.com/OpenAgentsInc/openagents/issues/9944)). |
| `CHK-03` Next line | Pass: the menu, SCR-02, the end card, SCR-05, SCR-06, and SCR-11. |
| `CHK-04` No dead ends | Pass: a card that can't run says why and offers Try again, Connect a computer, or, on the first run, Skip for now. |
| `CHK-05` No setup first | Pass: no sign-in, computer, wallet, or key before a first test. |
| `CHK-06` Three taps to a run | Pass on the phone (`the_first_run_starts_a_test_in_three_taps_and_resumes`; the recorded Gym on iOS and Android). Live, the third tap waits on the router offering the starter test set ([#9943](https://github.com/OpenAgentsInc/openagents/issues/9943)). |
| `CHK-07` Defaults chosen | Pass: Coder preselected, Project map asked for first. |
| `CHK-08` Progress visible | Pass: the run card's blocks, the result's counts, the menu's level bar. |
| `CHK-09` Confirm before publishing | Pass: SCR-20 before any publish; Stop asks first. |
| `CHK-10` Guided first run | Pass: resumes at the furthest step; no menu until the first result or Skip for now. |
| `CHK-11` Talk leads to a tap | Pass: nothing runs or publishes without its button (`only_a_cards_button_sends_a_request`). |
| `CHK-12` Chat drafts, you decide | Pass on the phone (recorded Gym and tests). Live, the interview hands chat-made tools to Coder ([#9945](https://github.com/OpenAgentsInc/openagents/issues/9945)). |
| `CHK-13` Numbers are real | Pass: every number is a worker card's, a run's report, or the ledger's; the menu shows a gray bar until the ledger is read, the runs-left count and "people testing now" are left out (no record says them), and a check card says "a trainer" rather than the runner's name. |

Not built in this change, as the wireframe's v1 cut allows or for want of a
record: the updates bell (`SCR-01.E02`), the season card (`E09`), the
"testing now" count, and the cinematic's shots before its end card. The
tab bar stays (Chat, Verse, Wallet, Account) with the menu as the Chat
tab's first screen; it hides during the first run.
