# Playtesting program

> **Status: Active from 2026-09-29.** Written 2026-09-28 and revised the same
> day with the owner's decisions: the program is **open** (a public
> TestFlight link and a public Android APK; anyone can join), and **joining
> earns nothing by itself**: the **PLAYTESTER** title, other titles, and
> playtest XP come only from contributions. Coder and Verse launch to
> playtesters on **Tuesday 2026-09-29**; see the
> [day-0 launch checklist](#day-0-launch-checklist-2026-09-29) and the
> [launch roadmap](../roadmap/2026-09-29-launch-roadmap.md), which ties this
> program to the MVP and the milestones after it. What to test right now,
> in the mockup and in build 21, is the [lo-fi playtest](#lo-fi-playtest).
> What exists today is listed in [What exists today](#what-exists-today);
> everything else is planned.
> Nothing in this document pays testers, and no payout is promised.
> Implementation is tracked in [Implementation tasks](#implementation-tasks)
> and epic [#9888](https://github.com/OpenAgentsInc/openagents/issues/9888).

The OpenAgents app went from build 1 to build 15 on TestFlight in two days,
and builds 16 to 21 followed; build 20 brought the chat router's prepared
answers and offers, and a simpler Wallet, and build 21, the current build,
puts the Gym in chat: test a tool, make your own, add results to the Gym,
check other trainers' results, and earn XP.
Each build so far was tested by the same person who asked for it: the owner
installs it, writes notes such as "1.0.0 Build 13 Feedback", and agents turn
the notes into commits and the next build. That loop is fast and it works,
but it has one player. This document turns it into an open program with many
players, a method, and a reward that fits the rest of the game: XP for
evidence-backed, accepted contributions, titles and cosmetics in the Grid, and
no money.

## Contents

- [Lo-fi playtest](#lo-fi-playtest)
- [The book behind the method](#the-book-behind-the-method)
- [Goals](#goals)
- [What exists today](#what-exists-today)
- [What we test, by stage](#what-we-test-by-stage)
- [Testers and cohorts](#testers-and-cohorts)
- [Onboarding, consent, and privacy](#onboarding-consent-and-privacy)
- [Session formats](#session-formats)
- [What to observe and measure](#what-to-observe-and-measure)
- [Feedback capture in the app](#feedback-capture-in-the-app)
- [Triage: from report to the next build](#triage-from-report-to-the-next-build)
- [Rewards](#rewards)
- [Day-0 launch checklist (2026-09-29)](#day-0-launch-checklist-2026-09-29)
- [Season 1, week by week](#season-1-week-by-week)
- [Session scripts](#session-scripts)
- [Questionnaire](#questionnaire)
- [Success metrics and the first milestone](#success-metrics-and-the-first-milestone)
- [Implementation tasks](#implementation-tasks)
- [Open questions](#open-questions)
- [Related documents](#related-documents)

## Lo-fi playtest

The lo-fi playtest tests the
[minimal v1 cut](../product/2026-09-28-app-wireframe.md#minimal-v1-cut-and-later-additions)
of the [phone app wireframe](../product/2026-09-28-app-wireframe.md)
(revision 3: the eval loop, inside chat) with what we can put in a
tester's hands today. It has two parts:

- **Part A, the mockup.** **OpenAgents Mockup**
  ([`bins/openagents-mockup-ios`](../../bins/openagents-mockup-ios/README.md)),
  a screens-only app with fake data, draws the whole v1 loop: the main
  menu, the guided first run in chat, a tool card, a test run, the result
  with and without the tool, **Add to the Gym**, level up, making a test
  set in chat, what's new, a check, and credit. It tests flow,
  comprehension, and the look. It draws revision 3
  ([#9940](https://github.com/OpenAgentsInc/openagents/issues/9940), `b4c6e5f6ab`).
- **Part B, the real app.** The OpenAgents app, build 21, tests the
  real eval loop in chat: the guided first run, a real test on our
  computers, **Add to the Gym**, making a tool and its tests, what's new, a
  check, and credit. It also tests Chat with OpenAgents with prepared
  answers and dispatch offers, the simple Wallet, the Grid, playtest
  logging, **Report a problem**, **Wrong answer**, and **Share this chat**.

The rule for both parts is the spec's first principle,
[IDIOT PROOF](../product/2026-09-28-app-wireframe.md#idiot-proof-checklist):
someone who has never heard of agents, Bitcoin, benchmarks, or evals
finishes the loop with zero explanation. Every hint a facilitator gives is a finding.

### Lo-fi goals

| ID | Question | Part | Spec IDs |
| --- | --- | --- | --- |
| `LF-G1` | Can a first-timer complete `FLOW-01` (Choose Coder, the cinematic, the first-run chat, the test, the result, **Add to the Gym**, Level up, the main menu) with zero explanation? | A and B | `FLOW-01`, `CHK-10` |
| `LF-G2` | How long and how many taps from app open to the first test run starting? The spec's limit is 3 taps. | A and B | `CHK-06`, `CARD-01.E05` |
| `LF-G3` | Do they understand the result: tests passed without and with the tool ("5 of 8 → 7 of 8"), the verdict, and when XP comes? | A and B | `CARD-04`, `SCR-05.E02` to `E04`, `SCR-06`, `CHK-08` |
| `LF-G4` | Does chat answer the tire-kicking questions ("Who are you?", "What does it cost?") at once and correctly? | A (finding chat), B (speed and correctness) | `CHAT-1`, `FLOW-03`, `SCR-17.E04`, `E08` |
| `LF-G5` | Do the dispatch offers (**Run Coder on** a computer, **Connect a computer**) make sense before the tap, and does the tester know nothing happens until they tap? | A and B | `CHAT-7`, `FLOW-05`, `SCR-17.E05`, `CHK-11` |
| `LF-G6` | Can they receive and send a small amount in the Wallet without help? | B | Wallet (outside the v1 menu) |
| `LF-G7` | Can they tell us something went wrong: **Report a problem**, **Wrong answer**, and **Share this chat**? | A (finding it), B (sending it) | `FLOW-06`, `SCR-13`, `SCR-18` |
| `LF-G8` | Which words didn't they understand? Any word from the spec's banned list on a v1 screen is a bug. | A and B | `CHK-02` |
| `LF-G9` | Can they make a tool and its test set by chatting, approve each step, and say what becomes public before adding it? | A and B | `FLOW-07`, `CARD-02`, `SCR-20`, `CHK-12` |
| `LF-G10` | Can they find what's new in the Gym, run a check of someone else's result, and say what XP they (and the other trainer) earn and why? | A and B | `FLOW-08` to `FLOW-10`, `CARD-05` to `CARD-07`, `CHK-13` |

Not tested in the lo-fi round: XP from another trainer's check of the
tester's own result (it needs a second trainer and the referee, so it
rarely arrives within a session), an adoption into Coder's defaults,
Rankings, Updates, the Coder screen, setup prompts (`SCR-07`, `SCR-10`,
`SCR-12`, `SCR-14`), and the cinematic's shots before its end card, which
build 21 doesn't have. The mockup draws some of these; skip them in
scoring. In build 21, a real test runs on our computers and its result is
real ([the live run](../../bins/openagents-ios/verification/2026-09-29-evals-in-chat/README.md#a-real-hosted-test-end-to-end)).

### Which build to use

| Part | App | How to get it | Use it for | It can't tell us |
| --- | --- | --- | --- | --- |
| A | **OpenAgents Mockup** 0.1.0 (`com.openagents.mockup`), installed next to the real app | Its own TestFlight app once the App Store Connect record exists (an owner step in the workspace's `NEEDS_OWNER.md`). Until then, the facilitator's phone or simulator: `bins/openagents-mockup-ios/build.sh sim`. | `FLOW-01` to `FLOW-10` as click-throughs; comprehension; the look | Speed or correctness of chat. The mockup chat doesn't read what you type: a typed message gets one generic reply, and only the suggestion chips and cards lead anywhere. A test run is a 12-second timer. Its **Report a problem** sends nothing. |
| B | **OpenAgents** 1.0.0 build 21 (`com.openagents.app`), installed fresh for `LF-B11`. On build 20, skip `LF-B11` to `LF-B16`; on build 19, also skip `LF-B3`, `LF-B4`, and `LF-B10`. | The public TestFlight link on iOS; the APK on Android | The real first run, tests, **Add to the Gym**, making a tool, what's new, checks, credit, Chat with OpenAgents, offers, the Wallet, the Grid, and real reports | The full cinematic (build 21 shows only its end card), XP that needs another trainer's check, and more than 3 tests a day: a trainer gets 3 runs per day (checks don't count), so the tester may see "no runs left" by `LF-B13`. Record those words; they are a finding too. |

Mockup settings for a playtest copy, in `App/MockData.swift`: turn
`showLaterFeatures` off (the v1 menu has no bell, **CODER**, or
**RANKINGS** row), turn `cinematicSkipAlways` off (the first play plays
through), and set `cinematicTimeScale` to `1.0` (about 57 seconds). The
design defaults are the opposite; if the copy you use has them, write that
in the results. Leave `fakeRunSeconds` short; a real run takes about a
minute on build 21.

Before each tester: long press the **OPENAGENTS** logo (on the first screen,
**STEP 1 OF 3**) to open the Screen index, tap **FLOW-01 · start over**, and
lock the phone. Never let a tester see the Screen index. Run part A before
part B, so the real app doesn't teach the tester the words first.

### Who takes part

- **First-timers only for part A.** People who have never seen either app
  and haven't read about it. A tester who has seen it runs part B only.
- **Five testers per round.** At least two who have never used a coding
  agent, and at least two who have.
- **Wallet tasks: adults who confirm they are 18 or older**, with amounts
  of about ₿100 that the facilitator sends and the tester returns, as in
  [session 3](#session-3-wallet-receive-and-send-tiny-amounts).
- **A computer is optional.** Testers with a Mac or Linux computer running
  the Coder host can finish `LF-B3` on it; everyone else stops at the offer.

### Facilitator script

A lo-fi session takes about 45 minutes: introduction (3), part A (15), part
B (15), and discussion (10). Say only the **bold** lines.

1. **Set up.** Reset the mockup, delete the real app and install build 21
   fresh (so `LF-B11` starts at step 1 of 3), open the
   [results template](#lo-fi-results-template), start a timer app, and note
   the tester's code (for example `LF-03`), device, and app builds.
2. **Introduction.** **"This is an early app. We're testing the app, not
   you; there are no wrong answers. Please think out loud the whole time:
   what you see, what you expect, what confuses you. I can't answer
   questions while you use it, but I'll answer all of them at the end. Is
   it OK if I record the screen and audio?"** Note the answer.
3. **Hand over the phone** with the mockup's icon showing, and read the
   first task. Read each task exactly as written, one at a time.
4. **During each task, you may say only these:**
   - **"What are you thinking?"**
   - **"What do you expect that to do?"**
   - **"What are you looking for?"**
   - **"What does that word mean to you?"**
   - **"What would you do next?"**
   - **"Please keep talking."** (when they go quiet for 10 seconds)
5. **Never:** name a button or screen, point, nod, say "right" or "almost",
   explain a word, or answer "is this right?". Answer questions with
   **"What do you think?"**
6. **When they're stuck.** At 60 seconds without progress, ask **"What are
   you looking for?"** At 2 minutes, mark the task *failed*, give the
   smallest hint that gets them moving (for example "It's on this screen"),
   log it as an intervention, and go on. A hint never names the answer if a
   smaller one might do.
7. **Stop the clock** while you ask a comprehension question, and restart
   it when they touch the phone again.
8. **Discussion.** **"What was that app for? What would you do with it
   tomorrow? What was confusing? Which words didn't make sense? What did you
   expect to happen that didn't?"** Then answer their questions.
9. **Wrap up.** Show them how to send reports from the real app (below),
   thank them, and fill in the template the same day.

### Task list

Part A tasks use the mockup; part B tasks use the real app. "Success" means
unaided: no intervention.

**Part A: the mockup**

| Task | Read aloud | Spec IDs | Success when | Then ask |
| --- | --- | --- | --- | --- |
| `LF-A1` | **"Here's an app. Before you tap anything, what do you think it's for?"** | `SCR-02.E02`, `E03` | They mention an AI, coding, or helping something get better. | — |
| `LF-A2` | **"Do whatever the app asks you to do, until you think you're done."** | `FLOW-01`: `SCR-02`, `CIN-01`, `SCR-15.E12`, `CARD-01`, `CARD-03`, `CARD-04`, `SCR-20`, `SCR-06`, `SCR-01` | They reach the main menu with no intervention. | On the result card, before they add it: **"What just happened? What do the two numbers mean? When do you get XP?"** On **Add to the Gym**: **"What will other people see?"** |
| `LF-A3` | (Same run; don't read anything.) Count taps from app open to **START THE TEST**, and time it. | `CHK-06`, `SCR-02.E04`, `CIN-01` end card, `CARD-01.E05` | 3 taps. | — |
| `LF-A4` | On the main menu: **"What do you think this screen wants you to do next?"** | `SCR-01.E12`, `E11`, `E13`, `CHK-03` | They point at **CHAT WITH OPENAGENTS**, a starter chip, or read the next-step line. | **"What would you use the chat for?"** |
| `LF-A5` | **"Test a different tool on Coder."** | `FLOW-02`, `FLOW-04`, `CARD-01.E07` | 2 taps from the main menu to the run starting, with a tool that isn't Project map. | — |
| `LF-A6` | **"You want to know who makes this and what it costs. Find out."** | `FLOW-03`, `SCR-01.E12`, `SCR-15`, `SCR-17` | They open chat and get both answers. Write down anything they type: typed questions show which first questions the answer bank must cover. | **"Where did that answer come from?"** (Did they notice **Prepared answer**?) |
| `LF-A7` | **"Ask what it can do for you."** Then, without pointing: **"What would happen if you tapped the buttons under the answer?"** | `FLOW-05`, `SCR-17.E05`, `E06`, `CHK-11` | They say **Run Coder on Studio Mac** would start work on that computer, and that nothing has happened yet. | **"What's Coder?"** |
| `LF-A8` | **"Suppose that answer was wrong. Tell the app."** | `FLOW-06`, `SCR-17.E09`, `SCR-18` | They find **Wrong answer** and see what it sends. | — |
| `LF-A9` | **"Something about this chat confused you. Report it, and include the chat."** | `SCR-13.E05`, `SCR-11.E06` | They open **Report a problem** and turn on **Share this chat**. | **"What will we see when you send that?"** |
| `LF-A10` | **"Find out what level you are and how much XP you have."** | `SCR-01.E04`, `SCR-11.E02` | They find it on the player card or Profile. | — |
| `LF-A11` | **"You have an idea for a tool that helps Coder write changelog entries. Make it with the app, and test it."** (Use the mockup's scripted answers.) | `FLOW-07`, `CARD-02`, `SCR-21`, `CARD-04` "first try", `SCR-20` | They approve the draft, try it once, run the full test set, and stop at **Add to the Gym** knowing what becomes public. | **"What's a test here? Why is there a test where the tool should stay out of the way?"** |
| `LF-A12` | **"Find out what's new in the Gym, and help check someone else's result."** | `FLOW-09`, `FLOW-08`, `CARD-05`, `CARD-06` | They open **What's new**, tap the check, and finish it. | **"What does checking do? Who gets XP?"** |
| `LF-A13` | **"Find out whether anything you made has earned you anything."** | `FLOW-10`, `CARD-07`, `SCR-11.E10` | They reach the credit card or **What you made**. | **"Can you spend XP?"** (The answer should be no.) |

**Part B: the real app, build 21**

Run `LF-B11` first, on the fresh install, then `LF-B1` to `LF-B10`, then
`LF-B12` to `LF-B16`.

| Task | Read aloud | Spec IDs | Success when | Then ask |
| --- | --- | --- | --- | --- |
| `LF-B1` | **"This is the real app. Ask it anything you'd want to know before trusting it."** After their own questions, ask them to type these if they didn't: "Who are you?", "What model is this?", "What does it cost?", "What can you do?", "How do I earn XP?" | `CHAT-1`, `FLOW-03`, `SCR-15`, `SCR-17.E03`, `E04`, `E08` | Every prepared answer shows in under a second, and every answer is correct (check it against the answer bank, `crates/coder/answers/chat-answers-v1.toml`, and the product notes in `knowledge/openagents/`). | **"Did any answer seem wrong or off?"** |
| `LF-B2` | **"Ask a follow-up about one of those answers."** | `SCR-17.E08`, `E10` | They use a follow-up chip or type one, and get an answer. | — |
| `LF-B3` | **"Ask it to fix a bug in a project on your computer."** | `CHAT-7`, `FLOW-05`, `SCR-17.E05`, `SCR-19` | Before tapping, they say correctly what **Run Coder on …** or **Connect a computer** will do. With a computer ready, Coder's reply streams into its own chat. Without one, stop at the explanation; don't pair. | **"Has anything happened on your computer yet?"** (The answer should be no until they tap.) |
| `LF-B4` | **"Find your wallet, by asking the chat."** | `CHAT-8`, `SCR-17.E06` | They get an **Open Wallet** chip and it opens the Wallet. | — |
| `LF-B5` | **"I want to send you ₿100. Show me what I should pay."** (Adults only.) | Wallet: **Receive** | They show a QR code or copy a request without help; the facilitator pays and they see it arrive. | **"How do you know it arrived?"** |
| `LF-B6` | **"Send ₿90 back to me."** Paste the facilitator's Lightning address in the call chat or show it as a QR code. | Wallet: **Send**, confirm screen | They send the right amount without help and can say what the fee was. | **"Was anything on the confirm screen unclear?"** |
| `LF-B7` | **"Find both payments."** Then: **"Is there anything the wallet wants you to do?"** | Wallet: **Recent activity**, **Back up your wallet** | They find both, and notice the backup card. Don't let them show the recovery words. | — |
| `LF-B8` | **"Open the tab with the globe. Tell me what you think you can do here."** Then say nothing for 2 minutes. | The Grid (outside the v1 menu) | Note every unprompted action: walk, push the ball, knock over blocks, jump. | **"What do you think this is for?"** |
| `LF-B9` | **"Something went wrong in a chat. Tell us, and include the chat."** | `FLOW-06`, `SCR-13`, `SCR-13.E05` | From a chat, they open **Report a problem** (a long press on the tab bar, or Account > Playtest), turn on **Share this chat**, read the preview, and tap **Send**. | — |
| `LF-B10` | Under a prepared answer: **"Tell us this answer was wrong."** | `SCR-17.E09`, `SCR-18` | They tap **Wrong answer**, read what it sends, and tap **Send**. | — |
| `LF-B11` | **"This is the real app. Do whatever it asks you to do, until you think you're done."** (Fresh install.) | `FLOW-01`: `SCR-02`, `CIN-01` end card, `SCR-15.E12`, `CARD-01`, `CARD-03`, `CARD-04`, `SCR-20`, `SCR-01`; `CHK-06` | 3 taps from app open to **START THE TEST** (**CHOOSE CODER**, **LET'S GO**, **START THE TEST**); they wait for the result, decide on **Add to the Gym** or **Not now**, and reach the main menu. Record the taps and the seconds from the third tap to the result. | On the result card: **"What do the two numbers mean? Did the tool help?"** On **Add to the Gym**: **"What will other people see?"** |
| `LF-B12` | From the main menu: **"Test a different tool on Coder."** | `FLOW-02`, `FLOW-04`, `SCR-01.E13`, `CARD-01.E07` | A run starts with Code finder or Test reader. Record the taps from the menu (the spec's limit is 2). | **"What happens while it runs? Can you leave?"** |
| `LF-B13` | **"You have an idea for a tool that helps Coder write changelog entries. Make it with the app, and try it."** | `FLOW-07`, `CHAT-10`, `CARD-02`, `SCR-21`, `CARD-04` **FIRST TRY**, `CHK-12` | They reach the draft, approve or change each step with **Looks good** or **Change it**, and tap **TRY IT ONCE**. If runs remain, **RUN THE FULL TEST SET** is optional. | **"What's a test here? Why is there a test where the tool should stay out of the way? Is any of this public yet?"** |
| `LF-B14` | **"Find out what's new in the Gym, and help check someone else's result."** | `FLOW-09`, `FLOW-08`, `SCR-01.E13`, `CARD-05`, `CARD-06` | They open **What's new** and read at least one item, then start a check with **RUN THE CHECK** from **Check a result**. | **"What does checking do? Does it use one of your tests for today? Who gets XP?"** |
| `LF-B15` | **"Find out whether anything you made has earned you anything."** | `FLOW-10`, `CARD-07`, `SCR-11.E10` | They reach **What you made** on Profile, or ask the chat what they've earned, and can say when XP comes. | **"Can you spend XP?"** (The answer should be no.) |
| `LF-B16` | **"Find where everyone's results are shown in the world."** (iPhone only; skip on Android.) | `SCR-01.E14`, the Verse's EVALS board | They open **THE GYM IN THE VERSE** or **See the board** and find the board. | **"What does that board tell you?"** |

### What to observe and record, per task

| What | How |
| --- | --- |
| **Result** | *Unaided*, *with hint* (count the hints), or *failed*. |
| **Time** | Seconds from the end of the instruction to success or the 2-minute stop, from the facilitator's clock. |
| **Taps** | Every tap, including wrong ones. For `LF-A3` and `LF-B11`, the count from app open to **START THE TEST**. |
| **Confusion points** | Where they paused more than 5 seconds, tapped the wrong thing, went back, or said "hmm". Name the spec ID (for example `SCR-03.E04`). |
| **Words they didn't understand** | Every word or label they misread, asked about, or skipped, verbatim, with the screen ID. Check each against the banned list below. |
| **Expectation gaps** | What they said they expected versus what happened. |
| **Quotes** | Short, verbatim, and marked with the task. |
| **Chat log** (`LF-A6`, `LF-A7`, `LF-B1` to `LF-B4`, `LF-B11` to `LF-B15`) | Each question verbatim; prepared or streamed; seconds to the first words; *correct*, *partly correct*, or *wrong*; any offer shown, and whether they understood it before tapping. |

The banned list from the spec's
[Words on screen](../product/2026-09-28-app-wireframe.md#words-on-screen):
npub, nsec, key, relay, Nostr, NIP, ATIF, tailnet, Tailscale, Wasm, plugin,
benchmark, Terminal-Bench, TB, Jev, Luna, Microcoder, verifier, trace,
recipe, grant, sats, BTC, ₿, Lightning, invoice, host, workspace, pubkey,
hex, and eval, evaluation, suite, case, grader, rubric, judge, baseline,
arm, harness, stand-in, mock, pilot, and extension. A banned word on a v1
screen or card (`SCR-01`, `SCR-02`, `SCR-05`, `SCR-06`, `SCR-11`,
`SCR-13`, `SCR-15` to `SCR-18`, `SCR-20`, `SCR-21`, `CARD-01` to
`CARD-07`) is a `CHK-02` bug. The exceptions: a chat
answer may name Gemini, the AI Gateway, Jev, or Nostr when the tester asks
what powers the chat, and "workspace" may appear in a chat on a connected
computer. The Wallet and the Grid are outside the v1 menu and use ₿ and
Lightning today; record the words testers didn't understand there too,
but file them as Wallet findings, not `CHK-02` bugs.

### Lo-fi success thresholds

A round of five testers passes when it meets every row. A missed row
becomes a `playtest` issue with the spec IDs, and the next round retests
it.

| Goal | Threshold |
| --- | --- |
| `LF-G1` `FLOW-01` unaided | 4 of 5 reach the main menu with no intervention. |
| `LF-G2` Taps to the first run | 3 taps for every tester (`CHK-06`). |
| `LF-G2` Time to the first run | Median under 2 minutes from app open to **START THE TEST**, with the mockup's cinematic at full length. |
| `LF-G2` Real app | 3 taps for every tester on build 21 (`LF-B11`), and the result arrives on the card without the tester doing anything else. |
| `LF-G3` Without and with | 4 of 5 explain the two numbers in their own words ("Coder passed 7 tests with the tool instead of 5"). |
| `LF-G3` The Gym | 4 of 5 say what the Gym does and why they'd come back. |
| `LF-G3` XP | 3 of 5 say what XP is for and that it comes when someone checks their result. |
| `FLOW-02` Returning run | 2 taps from the main menu, 5 of 5. |
| `LF-G4` Finding chat | 4 of 5 open chat without help (`LF-A6`). |
| `LF-G4` Speed | Every prepared answer shows in under 1 second; the first words of any other answer in under 2 seconds. |
| `LF-G4` Correctness | No wrong prepared answer on the `LF-B1` questions. |
| `LF-G5` Offers | 4 of 5 say what an offer will do before tapping; no tester thinks something ran without a tap. |
| `LF-G6` Wallet | 4 of 5 receive and send unaided; no wrong amount; no bitcoin lost to an app error. |
| `LF-G7` Reporting | 4 of 5 send a **Wrong answer** and a report with **Share this chat** unaided. |
| `LF-G8` Words | No banned word on a v1 screen or card; every word two or more testers didn't understand gets an issue. |
| `LF-G9` Making a test set | 3 of 5 finish `LF-A11` unaided; 4 of 5 say what becomes public before tapping **Add to the Gym**; 3 of 5 reach **FIRST TRY** in `LF-B13` unaided. |
| `LF-G10` News, checks, credit | 4 of 5 finish `LF-A12` and `LF-B14`; 4 of 5 say XP can't be spent. |
| Dead ends | None: every screen a tester reached had a clear next step (`CHK-04`). |

### How to report

- **In the real app, report from where it happened.** A long press on the
  tab bar opens **Report a problem** for the screen on view (also Account >
  Playtest). Write what happened and what you expected, and name the task
  and the spec ID if you know it (for example "`LF-A2`, `SCR-05.E03`:
  didn't understand the arrow").
- **Wrong answer.** Under a prepared answer, tap **Wrong answer**, read what
  it sends (the question, the answer's ID, and the router's judgment), and
  tap **Send**.
- **Share this chat.** When you report from a chat, **Share this chat** is
  off by default. Turn it on to include the conversation; the app shows the
  whole chat before you send.
- **Playtest logging is always on.** The app keeps a short list of which
  screens you opened and when, on the phone, with no message text. Tick it
  in a report to attach it; it gives the facilitator exact timings.
- **Until a build carries the triage key, reports wait on the phone**
  (**My reports** shows them), so also send TestFlight feedback or a GitHub
  issue from the **Playtest report** template
  ([Stage 0](#stage-0-what-works-today-launch-day)).
- **The mockup sends nothing.** Its **Report a problem** and **Wrong
  answer** are drawings. Mockup findings go in the results template, and
  the facilitator files them.
- **Facilitators file each finding** as a GitHub issue with the `playtest`
  label, the app and build (`OpenAgents Mockup 0.1.0 (1)` or
  `OpenAgents 1.0.0 (20)`), the task ID, and the spec IDs in the title or
  first line, so a fix can cite the same ID.

### Lo-fi results template

Copy one per tester. Keep it free of names, keys, recovery words, and
payment requests.

```markdown
# Lo-fi playtest: LF-03

- Date: 2026-09-30
- Facilitator:
- Tester code and circle: LF-03, confidant | public | target player
- Uses a coding agent: yes | no
- Device and OS:
- Apps: OpenAgents Mockup 0.1.0 (1), settings: playtest | design defaults
        OpenAgents 1.0.0 (20)
- Recording consent: yes | no

## Tasks

| Task | Result (unaided / hints: n / failed) | Time (s) | Taps | Confusion points (spec ID) | Notes |
| --- | --- | --- | --- | --- | --- |
| LF-A1 | | | | | Said the app is for: |
| LF-A2 | | | | | |
| LF-A3 | | | taps to START THE TEST: | | |
| LF-A4 | | | | | "Make Coder better" means: |
| LF-A5 | | | | | |
| LF-A6 | | | | | |
| LF-A7 | | | | | |
| LF-A8 | | | | | |
| LF-A9 | | | | | |
| LF-A10 | | | | | |
| LF-A11 | | | | | What a test is: |
| LF-A12 | | | | | What checking does: |
| LF-A13 | | | | | Can XP be spent: |
| LF-B1 | | | | | |
| LF-B2 | | | | | |
| LF-B3 | | | | | computer: yes / no |
| LF-B4 | | | | | |
| LF-B5 | | | | | |
| LF-B6 | | | | | |
| LF-B7 | | | | | |
| LF-B8 | | | | | unprompted actions: |
| LF-B9 | | | | | |
| LF-B10 | | | | | |
| LF-B11 | | | taps to START THE TEST: | | seconds to the result: |
| LF-B12 | | | taps from the menu: | | |
| LF-B13 | | | | | What a test is: |
| LF-B14 | | | | | What checking does: |
| LF-B15 | | | | | Can XP be spent: |
| LF-B16 | | | | | |

## Comprehension answers (verbatim)

- Result card (the two numbers, when XP comes):
- What becomes public on Add to the Gym:
- What Run Coder would do:

## Chat log

| Question (verbatim) | Prepared or streamed | Seconds to first words | Correct / partly / wrong | Offer shown | Understood before tapping |
| --- | --- | --- | --- | --- | --- |

## Words they didn't understand

| Word | Screen (spec ID) | On the banned list |
| --- | --- | --- |

## Top three problems

1.
2.
3.

## Filed

- Issues: #
- In-app report codes: PT-
```

After a round, add up the five templates against the
[thresholds](#lo-fi-success-thresholds) and post the totals, the missed
rows, and the issue numbers in the weekly note.

## The book behind the method

The method comes from Chapter 9, "Playtesting", of the book photographed for
this program. The title and author aren't visible in the nine photos. The
contents, the chapter structure, the "playcentric design process", the
designer perspectives, the Halo 3 figures, and the photo of a *Flower*
playtest at thatgamecompany match Tracy Fullerton's *Game Design Workshop: A
Playcentric Approach to Creating Innovative Games*; the page numbers (Chapter
9 starts on page 277) suggest the third edition. Treat that attribution as an
identification from the contents, not something read off a cover.

Two of the photos show chapter text (pages 277 to 279). The rest are table of
contents pages, so for the later sections of Chapter 9 and for Chapter 10 this
document uses only the section headings and says so. It doesn't invent what
the book says under them.

### Lessons from the pages we have

1. **Playtesting is the most important and least understood activity.**
   "Playtesting is the single most important activity a designer engages in,
   and ironically, it is often the one that designers typically understand
   the least about." It isn't "just play the game and gather feedback";
   playing is "only one part of a process that involves selection,
   recruiting, preparation, controls, and analysis." Our program has a
   section for each of those five.
2. **Playtesting isn't four other things.** The book separates it from an
   *internal design review* (the team plays and talks features), *quality
   assurance testing* (rigorously testing each element for flaws), *focus
   group testing* (asking a sample how much they would pay), and *usability
   testing* (recording mouse movements, eye movements, and navigation).
   Playtesting is what the designer does "throughout the entire design
   process to gain an insight into how players experience the game." We keep
   the distinction: a crash in the Wallet is a QA finding, a confusing send
   screen is a usability finding, and "I didn't want to go back into the
   Grid" is a playtest finding. All three are welcome; they are triaged
   differently.
3. **Scale to the facility you have.** Microsoft Game Studios ran "over 3000
   hours of playtesting with more than 600 players" for Halo 3; "your game
   might have 10 or 20 playtesters, possibly playing in your garage." Both
   are valuable. The shared end goal is "gaining useful feedback from players
   to improve the overall experience of the game." We start with the garage.
4. **The designer's goal is four questions.** The designer's "foremost goal"
   is to make sure the game "is functioning the way you intended, that it is
   internally complete, balanced, and fun to play." Chapter 10's headings
   turn these into tests: *Is your game functional?*, *Is your game
   internally complete?* (loopholes, dead ends), and *Is your game
   balanced?* (dominant strategies, balancing for skill, balancing for the
   median skill level). Our [stage table](#what-we-test-by-stage) uses them.
5. **Be the advocate for the player, all the way through.** The designer's
   primary role is "an advocate for the player," and teams working "long
   days and nights for months at a time" forget "the player in their own
   quest to make the game live up to their vision." Fourteen builds in a day
   is exactly that risk.
6. **Test from the first moment, and loop: playtest, evaluate, revise.**
   Figure 9.1 shows the cycle getting "tighter and tighter as production
   moves forward," with smaller changes near launch. The book argues
   against waiting for a beta "strongly": by then "it is really too late to
   make any fundamental changes," and "if the core gameplay is not fun or
   interesting at this point, you are stuck with it." It advises testing
   "from the very moment you begin," cheaply, with "your own time and some
   volunteers." For us, that means testing the agent trainer loop now, on
   paper, before its code exists (see [week 3](#week-3-2026-10-13-to-10-19-raids-and-the-paper-trainer-loop)).
7. **Start with yourself, with a fresh mind, and keep a notebook.**
   Self-testing is "most valuable in the foundation stage" and is "where you
   create solutions to glaring problems"; the goal is "to make the game work,
   even if it is only a rough approximation." Learn to play with a "fresh"
   mind: clear what you know of the game and play "naively." Exercise 9.1
   asks you to "describe in detail what goes through your head" and to
   "start a playtesting notebook in which you record all of the feedback
   you get from yourself and other testers." As the game evolves, "you will
   have to rely more and more on outside testers."
8. **Then confidants, briefly.** Friends and colleagues outside the design
   team "bring truly fresh eyes." Early on you may need to be there to
   explain. The goal is "a version that people can play without much
   intervention from you": for software, "the user interface will need to be
   in place, or you might need to provide some written rules." Figure 9.2
   shows Jenova Chen explaining "minimal information to get the game
   started."
9. **Then wean yourself off confidants.** Friends and family "have a personal
   relationship with you, and this obscures their objectivity": most are
   "either too harsh or too forgiving." "It is best not to rely too heavily
   on a small group of individuals. They will never give you the objective,
   broad criticism that you require to take your design to the next level."

### Structure we adopt from the headings

The rest of Chapter 9 is visible only as headings. We follow its order:

- **Recruiting, in widening circles:** self-testing, confidants, people you
  don't know, the ideal playtesters, and your target audience.
- **A session in five timed parts:** introduction (2 to 3 minutes), warm-up
  discussion (5 minutes), play session (15 to 20 minutes), discussion of the
  game experience (15 to 20 minutes), and wrap-up. Our
  [scripts](#session-scripts) use these times.
- **Methods of playtesting**, **the play matrix**, and **taking notes**.
- **Basic usability techniques:** *do not lead*, and *remind testers to think
  out loud*.
- **Quantitative data**, **metrics in game design**, **data gathering**, and
  **test control situations**.
- Sidebars titled "How Feedback from Typical Gamers Can Help Avoid
  Disappointing Outcomes", "A Primer for Playtesting: Don't Follow These
  Rules!", and "Why We Play Games".

Chapter 10's headings name what a test is for, by stage: *foundation*,
*structure*, *formal details*, and *refinement*.

## Goals

Playtesting answers different questions for each part of OpenAgents. We
take a position on each.

| Surface | The question | Why it matters now |
| --- | --- | --- |
| First run and Account | Can a new person get from install to one useful thing without help? | Every other goal depends on it, and nobody outside OpenAgents has tried. |
| Chat with OpenAgents | Does chat answer a first-timer's questions at once and correctly, and do its offers (**Run Coder on** a computer, a screen chip) make sense before the tap? | Chat needs no setup, so it is the first thing every tester tries. |
| Coder on a computer | Does sending Coder to your own computer from chat feel trustworthy and worth coming back to? | Work on your own code is why Coder exists; connecting a computer is the steepest step. |
| The Grid (the Verse tab, the globe) | Is moving around, pushing the ball, and meeting people fun for five minutes with nothing to win? | The book's warning: if the core isn't fun by beta, "you are stuck with it." |
| The Gym | Does a player understand what the RESULTS board and a replay show, and want to see more? | The Gym is where agent training will happen. The Lagrange 1 portal is hidden for now. |
| Wallet | Can a person receive and send a small amount on mainnet correctly with the simpler screen (balance, **Receive**, **Send**, recent activity), and do they trust it? | Real money: errors cost testers real bitcoin. Correctness beats fun here. |
| Agent training | Would a player want to level up as an agent trainer, and do the rules feel fair? | The loop is specified, not built: the cheapest time to change it is now. |

Non-goals: load testing the relay, security review of the wallet, and
marketing research (the book's "focus group testing"). Those have their own
owners.

## What exists today

Status words follow the [glossary](../glossary.md).

| Piece | Status |
| --- | --- |
| The OpenAgents app on iOS (`com.openagents.app`) with four tabs: Chat, Verse (the Grid), Wallet, and Account ([README](../../bins/openagents-ios/README.md)) | Implemented. Version 1.0.0 builds 1 to 20: 16 added Report a problem, 17 chat with no computer, 18 Chat with OpenAgents (`e1d499def7`), 19 sends every new chat to OpenAgents (`99e3094bff`), 20 brings the chat router's prepared answers and offers, **Wrong answer**, and a simpler Wallet (`85b37f75c7`), and 21 puts the Gym in chat ([#9939](https://github.com/OpenAgentsInc/openagents/issues/9939), `59b6908044` to `caf14e1a3b`; the upload is the release owner's step). |
| **OpenAgents Mockup** (`com.openagents.mockup`, [README](../../bins/openagents-mockup-ios/README.md)): every screen of the [wireframe spec](../product/2026-09-28-app-wireframe.md) with fake data, for the [lo-fi playtest](#lo-fi-playtest) | Implemented (`1f73e0c4bd`), version 0.1.0; draws wireframe revision 3 since `b4c6e5f6ab`. Runs from Xcode or the simulator; its own TestFlight app waits on an App Store Connect record (an owner step). |
| OpenAgents for Android ([`bins/openagents-android`](../../bins/openagents-android/README.md)), the same Rust library | Partial ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)). Chat, Verse (with the Gym and RESULTS panels, `82663b935d`), Wallet (`e56d173480`, `e1aeec7413`; [#9861](https://github.com/OpenAgentsInc/openagents/issues/9861) closed), and Account work. Verified on the emulator only; a live tailnet chat, QR scanning, the terminal, and motion look haven't been checked on a device. Distributed as a signed release APK (1.0.0, version code 16, the iPhone build number) from a GitHub release that testers install by hand. |
| Public distribution | Planned for 2026-09-29: a public TestFlight link for iOS and a public APK download for Android (owner steps in the workspace's `NEEDS_OWNER.md`). |
| TestFlight's own feedback: a tester takes a screenshot or uses **Send Beta Feedback** in the TestFlight app, and it reaches App Store Connect with the build number, device, and OS. Crash reports reach it too. | Exists, from Apple. `openagents playtest testflight` reads it into the triage inbox as drafts and log entries, without the tester's Apple identity ([#9905](https://github.com/OpenAgentsInc/openagents/issues/9905), [triage](playtest-triage.md#testflight-feedback)). |
| The owner's build notes ("1.0.0 Build 13 Feedback") turned into commits by agents, followed by a build bump (for example `e84de16fd5`, `06d033d663`) | The current loop. It isn't written down anywhere except in commit history. |
| **Changelog** in Account (`crates/openagents-mobile/src/account.rs`) | Implemented. One entry per TestFlight build with a **What to test** line, from build 16 (`a4aa3013de`); a test ties the newest entry to the build number in `project.yml`. |
| **Report a problem**, **My reports**, and **Playtest logging** (on for everyone; the old opt-in **Playtest session** switch is gone) in Account, and a long press on the tab bar (`crates/playtest`, `crates/openagents-mobile/src/playtest.rs`; iOS and Android) | Implemented in build 16 (`74f2f90be0`) on iOS, and on Android with the Account playtest card ([#9903](https://github.com/OpenAgentsInc/openagents/issues/9903)); Android reports name the `android` platform. Reports are sealed to the triage key, which the owner hasn't created yet, so until a build carries it reports wait on the phone. No telemetry: the app sends nothing but a report the tester files. |
| Triage inbox and triage log: `openagents playtest` reads the triage key's reports, drafts `playtest` issues for a person to approve, and records every acceptance ([playtest-triage.md](playtest-triage.md)) | Implemented ([#9884](https://github.com/OpenAgentsInc/openagents/issues/9884)). It reads reports once the owner creates the triage key and a build carries it. |
| NIP-XP quests, awards, revocations, and achievement labels; the ledger; the referee tool ([NIP-XP](../../nips/openagents/NIP-XP.md)) | Implemented, with three rules: `kb-transfer`, `reproduce`, and `playtest` ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)). The `playtest` rule, its report (`3197`) and session record (`3196`), and `microcoder xp playtest-keygen`/`playtest-session` exist; the playtest referee key doesn't yet (an owner step), so no playtest award counts. |
| Levels, titles, and `lv n` name tags | Implemented on desktop Verse; the phone shows the level in Account > Trainer. The Grid's name tags show a pubkey prefix and no level. |
| A read-only XP reader and trainer card in the app | Specified; phase 1 of epic [#9847](https://github.com/OpenAgentsInc/openagents/issues/9847), in progress. |
| The Gym in chat: tests of a tool with and without it on our computers ([hosted eval runner](../deployment/eval-runner.md)), making a tool and its tests in chat, **Add to the Gym**, checks, and XP from checks ([evaluation](../extensions/evaluation.md)) | Implemented and live on 2026-09-29: the starter test sets for Project map, Code finder, and Test reader each helped Coder (2 of 6 without the tool; 5, 4, and 5 of 6 with it), and every check confirmed ([record](../extensions/measurements/2026-09-29-hosted-runner-live.md)). XP is never money. |
| NIP-17 private messages and NIP-44 encryption | Implemented in `crates/nostr` (`nip17.rs`, `nip44.rs`). |
| Chat in the Grid | None. The Grid has presence, a shared ball and blocks, and the Gym (the Lagrange 1 portal is hidden); talking happens outside the app. |

## What we test, by stage

The book's Chapter 10 splits testing by stage: foundation, structure, formal
details, and refinement. Each OpenAgents surface is at a different stage, and
the stage decides the kind of test and the kind of report we want.

| Surface | Stage | Test for | Wanted | Not yet wanted |
| --- | --- | --- | --- | --- |
| Agent training loop | Foundation | Is it fun and fair at all? | Reactions to a paper prototype | Bug reports: there's no code |
| The Grid | Foundation to structure | Fun, and what players do unprompted | Play-matrix notes, "what did you try" | Pixel polish |
| The v1 loop (mockup) | Foundation | Can a first-timer follow it, and do they understand it? | Where they got stuck, words they didn't know ([lo-fi playtest](#lo-fi-playtest)) | Bug reports about fake data |
| The Gym board | Structure | Internally complete: dead ends, loopholes | Where players got stuck or lost | Balance |
| Chat with OpenAgents | Formal details | Correct, instant answers; offers that make sense | Wrong answers, confusing offers, reproducible bugs | New features |
| Coder on a computer | Formal details | Functional: every state has a way out | Reproducible bugs, confusing states | New features |
| Wallet | Refinement | Functional and correct, then trust | Any wrong amount, fee, or state; any fear | Feature requests beyond the open wallet issues |
| First run and Account | Formal details | Can a stranger finish unaided? | Every intervention a moderator had to make | |

## Testers and cohorts

The program is **open**. Anyone can join: on iOS through the public
TestFlight link, on Android by installing the public APK. There's no
application, no invite list, and no cohort that gates access. Every tester
gets every build that goes to the public link.

The book's widening circles still shape *who we talk to and when*, not who
may install:

| Circle | Who | How they arrive | When |
| --- | --- | --- | --- |
| 0. Self | The owner and anyone building the app, playing "with a fresh mind" | Internal TestFlight group | Every build |
| 1. Confidants (informal) | Friends and colleagues who don't work on the app | The same public link, plus a personal ask for a moderated session | Day 0 to week 1 |
| 2. Public testers | Anyone who installs from the public link or APK: people who follow OpenAgents on Nostr and X, past contributors, strangers | The public TestFlight link and APK, posted publicly on 2026-09-29 | From day 0 |
| 3. Target players | People matching the thesis in [docs/game](README.md): gamers who raid or grind and who use a coding agent; people who already run a Lightning wallet | The same link; we invite them to moderated and group sessions | Week 2 onward |

Positions:

- **Open by default, moderated by invitation.** Installing is open.
  Moderated think-aloud sessions and group sessions ("raids") take a
  moderator's time, so we invite testers into them from whoever has joined,
  favoring circles 1 and 3. Anyone can run the unmoderated task lists.
- **Wean off confidants fast.** Confidants are an early, informal round, not
  a gate. The book is blunt that friends are "too harsh or too forgiving."
- **Only Coder on your own code needs a computer.** Chat with OpenAgents
  needs no computer, sign-in, or wallet. Sending Coder to work on your own
  code needs a Mac or Linux computer running the Coder host. Testers
  without one test chat, the Grid, the Gym, the Wallet, and Account, and
  that is a full program.
- **Adults only for the Wallet.** The Wallet runs on Bitcoin mainnet. The
  brief says so, and the scripted wallet session is only for testers who
  confirm they are 18 or older.
- **Say plainly what testers get:** nothing for joining, and XP and titles
  for accepted contributions; never money. See [Rewards](#rewards).
- **TestFlight limits apply.** The public link is an external group, so the
  first build of a version goes through Apple's Beta App Review, which can
  take a day, and the public link has a tester cap that the owner sets (up
  to Apple's 10,000). Not every internal build goes to the public link.
- **Android is behind iOS.** Android testers get Chat, Verse, Wallet, and
  Account, with **Report a problem**, **My reports**, playtest logging,
  and the playtest card
  ([#9903](https://github.com/OpenAgentsInc/openagents/issues/9903)), but the
  build has run on the emulator only
  ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)). Reports
  name the platform.

## Onboarding, consent, and privacy

Every tester gets the same one-page brief. It's published next to the public
TestFlight link and APK, so joining means reading it; testers in a moderated
session also agree to it aloud at the start (or, later, by tapping **I
agree** in the app). It says, in plain words:

1. **What this is.** You are testing an early app. Things will break. We are
   testing the app, not you; there are no wrong answers.
2. **What we record.**
   - In a moderated session: the moderator's notes, and a screen or audio
     recording only if you say yes at the start of that session. You can
     say no and still take part.
   - In a report you send: exactly what the report preview shows you, and
     nothing else.
   - In the app, by default: nothing. The app sends no analytics.
3. **What becomes public.** Accepted reports become GitHub issues in a
   public repository, written by us from your report, with no screenshots
   of the Wallet or of your keys. XP awards are signed Nostr events: anyone
   can see which key earned what. You choose which key, and it doesn't have
   to be tied to your name.
4. **Money.** The Wallet is real Bitcoin. Use amounts you can afford to lose.
   We never ask for your recovery words, your nsec, or a screenshot of
   either. Nobody from OpenAgents will ever ask you to send them bitcoin, except
   in the scripted wallet session, where you return the test bitcoin that the
   moderator sent you.
5. **Leaving.** You can stop at any time. We delete your session notes and
   recordings on request. Signed events on relays can't be deleted, but an
   award can be revoked, and a revocation is public.
6. **Confidentiality.** None. The repository is open source, so you can
   talk about what you see.

Privacy positions for the program:

- **No background telemetry.** We don't add an analytics SDK, a crash SDK
  beyond Apple's, or a remote logging endpoint. Measurement comes from
  sessions, reports, and questionnaires.
- **Playtest logging, a local log,** is the one exception (see
  [task 2](#implementation-tasks)): the app records a short list of
  structural events (which tab, which screen, which error code, and when),
  keeps it on the device, and sends it only attached to a report the
  tester previews and chooses to attach it to. It was first built as an
  opt-in **Playtest session** switch; the owner made it on for everyone
  for the playtest, so the app has no switch, and the Playtest screen says
  in one line whether it is on in this build (see
  [Playtest logging in a release](#playtest-logging-in-a-release)). It
  never records message text, prompts, transcripts, keys, recovery words,
  invoices, addresses, amounts, balances, or other players' keys. The change
  that builds it adds these rules to [`INVARIANTS.md`](../../INVARIANTS.md)
  with tests, as the repository requires.
- **Wallet and key screens are never captured by the app.** A report sent
  from the Wallet tab or from **Identity keys** attaches no screenshot, even
  if the tester asks. A tester who wants to show a Wallet problem describes
  it in words.
- **Reports are private until triaged.** They travel encrypted to the triage
  key and become public only as an issue we write, with the tester's text
  quoted only if they allowed it.

## Session formats

| Format | What it is | Best for | Cohorts | Cost per tester |
| --- | --- | --- | --- | --- |
| **Moderated think-aloud** | One tester, one moderator, a call with screen sharing, the five-part session from the book | First run, chat, Wallet, connecting a computer | 1, 2, 3 (invited) | 45 to 50 minutes of moderator time |
| **Lo-fi session** | One first-timer, one facilitator: the mockup's v1 loop, then build 21 ([lo-fi playtest](#lo-fi-playtest)) | `FLOW-01`, chat, offers, the Wallet | 1, 2, 3 (first-timers) | 45 minutes |
| **Unmoderated tasks** | A published task list for the current build; any tester plays alone and sends a report | Grid, Gym, regressions after a fix | Anyone | 20 minutes of triage time |
| **Async diary** | Five short entries over a week: what you opened the app for, what you did, what annoyed you | Whether anyone comes back; the Grid's pull | Volunteers | One read-through a week |
| **Group session ("raid")** | Five to eight testers in the Grid at the same time, on a voice call, with a script | Presence, the shared ball and blocks, the reset pillar, social fun | 2, 3 | One hour, two moderators |
| **Paper prototype** | Printed or on-screen mockups of the trainer card, quest board, and titles; testers "play" a week of agent training on paper | The agent training loop before it's built | 1, 2, 3 | 30 minutes |

Rules for every moderated format, from the book's usability techniques:

- **Do not lead.** Ask "what do you expect this to do?", never "tap the
  globe." When a tester is stuck for 60 seconds, ask "what are you looking
  for?" Only after a second minute, help, and log it as an intervention.
- **Remind testers to think out loud** whenever they go quiet.
- **Give minimal information to start**, as in the book's *Flower* photo:
  the install link and one sentence.
- **Take notes in the play matrix**: one row per task, with columns for
  *did*, *said*, *expected*, *intervention*, and *severity*.

## What to observe and measure

### Observed in sessions

- **Unaided completion**: did the tester finish the task without an
  intervention?
- **Interventions**: each time the moderator had to help, and why.
- **Time on task**, from the moderator's clock, not the app.
- **Expectation gaps**: what the tester said they expected versus what
  happened.
- **Unprompted play**: in the Grid, what the tester tried with no task
  (pushed the ball, stacked blocks, jumped off something, walked out of
  bounds).
- **Trust moments**: any hesitation before a send, a pairing, or a reveal,
  and what they said.
- **Dead ends and loopholes**, in Chapter 10's sense: a screen with no way
  out, or a way to do something the design didn't intend.

### Counted in the program

| Metric | Source |
| --- | --- |
| Testers active per week, per platform | TestFlight installs, APK downloads, and sessions held |
| Unaided completion rate per scripted task | Play-matrix notes |
| Interventions per session | Play-matrix notes |
| Reports received, accepted, duplicate, and declined | Triage log |
| Median time from an accepted report to a build that fixes it | Triage log and build numbers |
| Fixes verified by the reporter on the fixing build | Triage log |
| Crashes per build | App Store Connect |
| Day-7 return: did the tester open the app again a week later, by their own account | Diary and questionnaire |

These come from people, not from telemetry, on purpose: at 10 to 40
testers, a conversation tells us more than a dashboard, and it costs nothing
in privacy.

## Feedback capture in the app

We stage it, so testing starts today and the app catches up.

### Stage 0: what works today (launch day)

The in-app **Report a problem** action
([#9882](https://github.com/OpenAgentsInc/openagents/issues/9882)) arrives in
build 16 and sends only once a build carries the triage key, so on
2026-09-29 testers give feedback three ways:

- **TestFlight feedback (iOS).** Take a screenshot in the app and tap
  **Share Beta Feedback**, or use **Send Beta Feedback** in the TestFlight
  app. It arrives in App Store Connect with the build number, device, and
  OS. Don't use it on the Wallet tab or **Identity keys**: describe those
  problems in words instead.
- **A GitHub issue** from the **Playtest report** template
  (`.github/ISSUE_TEMPLATE/playtest-report.yml` in `OpenAgentsInc/openagents`,
  label `playtest`). It asks for the platform, the build (Account, **About
  this device**), the tab, what happened, what you expected, and the steps.
  This is the only channel that works for Android testers who have a GitHub
  account, and it's public: never paste keys, recovery words, invoices, or
  addresses. A tester who wants XP later adds the npub they want credited.
- **Email** to the playtest address published with the public link, for
  anything that shouldn't be public or for testers without GitHub.

The owner's own build notes continue as before.

### Stage 1: a Report action in the app

Status: implemented on iOS in build 16 (`74f2f90be0`), as described below,
with the triage key still to be created (workspace `NEEDS_OWNER.md`); until
then reports wait on the phone. Screenshots are cropped top and bottom and
sent as a small JPEG; reports read on the triage side with
`openagents playtest inbox` ([triage](playtest-triage.md)). When a report is
sent, the app also publishes its public, content-free NIP-XP playtest record
(kind `3197`: the build, platform, kind, and the private report's digest,
signed by the same world key), which a playtest award cites
([#9904](https://github.com/OpenAgentsInc/openagents/issues/9904)); the form
says so, and **My reports** marks it once a relay accepts it.

A **Report** action that files a structured report, signed by the tester's
key and sent privately:

- **Where:** in Account > Playtest, as **Report a problem**, and as a long
  press on the tab bar from any tab, so the report knows where the tester was.
- **What it fills in for the tester:** app version and build number, the tab
  and screen (route), device model, iOS version, the time, and, from a
  Coder chat, the chat's task ID only if the tester ticks it.
- **What the tester writes:** what happened, what they expected, and the
  steps, with a **Kind** choice: *bug*, *confusing*, *idea*, or *felt good*.
  "Felt good" is on purpose: the book asks what is fun, not only what is
  broken.
- **Screenshot:** off by default. When on, the app shows the exact image
  before sending, and lets the tester crop it. Never offered on the Wallet
  tab or on **Identity keys**.
- **Playtest log:** offered only while playtest logging is on in the
  build, attached only when the tester ticks it, and shown in full before
  sending.
- **Share this chat:** from an open chat with OpenAgents, off by default,
  shown in full before sending, and attached only when the tester ticks it.
  It is evaluation data for the
  [chat router](../coder/design/2026-09-28-chat-router.md), and it stays in
  the private report. Under a prepared answer, **Wrong answer** sends only
  that question, the answer's ID, and the worker's judgment, after the
  tester reads what it sends and taps **Send**.
- **Transport:** a NIP-17 private message to the OpenAgents triage key,
  sealed with NIP-44, signed by the tester's Verse world key (so the XP it
  can earn lands on the key whose name tag shows in the Grid). Screenshots
  above the relay's size limit are compressed or dropped, with a note.
- **Receipt:** the report's event ID shows as a short code the tester can
  quote. **My reports** in Account lists what was sent and, later, what was
  accepted.

### Give feedback on selected text

Selected text in a chat has a **Give feedback** item
([#10127](https://github.com/OpenAgentsInc/openagents/issues/10127)): on
the phone in the long-press menu and in the selection toolbar beside
**Copy** and **Select All** (iOS and Android), and on the desktop first in
the right-click menu while text is selected. Its dialog quotes the
selection and has one field, "What's wrong or what should change?";
**Send** files a report of kind `comment` and says "Sent". The report
(`playtest::feedback`) carries the selected text, where it came from (the
conversation, the message's index, and a reply's route, tier, prepared
answer, and model), the build, and the device, and no other chat content.
It is sealed to the triage key like any report; the phone keeps it in **My
reports**, and until a build knows the triage key it waits there (the
desktop keeps it in `~/.openagents/desktop/feedback/`). `openagents
playtest inbox` lists each one with its quote and comment. A desktop build
seals to the key in `OPENAGENTS_PLAYTEST_TRIAGE_KEY` when that is set, so
an operator can prove the path with a key of their own before the owner's
key exists. When the keychain doesn't give the desktop its world key (a dev
build's keychain prompt was denied), a one-time key signs the comment: it
still reaches the triage key, without a public record or playtest XP.

### Playtest logging in a release

Playtest logging follows the app's release gate
([mobile 1.0 audit](../mobile/1.0-audit.md)), changed on 2026-10-09: it is
on only in a preview build (made with `OPENAGENTS_MOBILE_PREVIEW=on`) or one
made with `OPENAGENTS_PLAYTEST_LOGGING=on`, and off in release archives,
Android release and bundle builds, and normal debug builds.
`OPENAGENTS_PLAYTEST_LOGGING=off` forces it off in any build; any value but
`on`, `off`, or unset is refused by the build scripts. The app has no
switch; **Account > Playtest** (itself a preview screen) shows one line
saying whether it is on. The log's rules don't change: closed structural
values only, kept on the phone in the app's encrypted store (at most 200
events, oldest dropped first), deletable with **Delete the log**, and sent
only inside a report whose preview showed it and whose tester ticked it.

```sh
OPENAGENTS_MOBILE_PREVIEW=on bins/openagents-ios/build.sh sim          # preview: logging on
OPENAGENTS_PLAYTEST_LOGGING=on bins/openagents-android/build.sh run    # logging on, no preview
```

The Rust library reads both when it compiles
(`crates/openagents-mobile/src/playtest.rs`, `LOGGING`), so Cargo rebuilds
it when a value changes. A build with logging off records nothing, offers
no log in **Report a problem**, deletes any log a playtest build left on
the phone, and shows "Playtest logging is off in this build." The tests
`a_build_with_playtest_logging_off_records_nothing_and_deletes_the_log` and
`playtest_logging_is_off_in_a_release_build_unless_switched_on` check it.

### Stage 2: public, re-checkable reports for XP

For an accepted report to earn XP that anyone can re-check, the tester
publishes a small signed **playtest report** event that holds no content: the
build number, the report's kind, a digest of the private report, and the
time. The award cites that event and the GitHub issue. See
[Rewards](#rewards).

## Triage: from report to the next build

Today's loop, written down, then widened to many testers.

1. **Collect.** Once a day, the triage owner (an agent with the triage key,
   supervised by the owner) reads new reports: TestFlight feedback, GitHub
   issues from the **Playtest report** template, the playtest email, the
   owner's notes, and, from stage 1, the triage inbox.
   The inbox is `openagents playtest inbox`; the
   [triage inbox guide](playtest-triage.md) covers the commands and the
   triage log.
2. **Deduplicate and classify.** Each report is one of: *new bug*,
   *duplicate* (linked to the first), *not reproducible yet*, *design
   feedback*, *idea*, or *declined*, with a reason.
3. **File.** A new bug or design finding becomes a GitHub issue in
   `OpenAgentsInc/openagents`, written in the repository's style, with the
   label `playtest`, the build it was seen on (`1.0.0 (14)`), the surface
   (`area:coder`, `area:grid`, `area:wallet`, and so on), and the report
   code. No Wallet or key screenshots, no private text unless the tester
   allowed it. The issue is the acceptance record.
4. **Severity.** *P0*: loses money, leaks a key, or bricks the app; fixed
   before the next external build. *P1*: blocks a scripted task. *P2*: hurts
   but has a way around. *P3*: polish.
5. **Fix.** An agent or contributor fixes it with a commit that names the
   issue, as the owner's notes are handled today.
6. **Build.** The build number is bumped, the Changelog in Account gets a
   line for what changed in plain words, and the build goes to TestFlight.
   Internal first; external groups when a batch of fixes is worth their
   time.
7. **Verify.** The issue gets a comment with the fixing build number, and
   the reporter is asked to check it on that build. When they confirm, the
   issue closes as *fix verified*. When they can't reproduce the original
   either way, it closes as *fixed, unverified*.
8. **Evaluate.** Each week, read the play-matrix notes and the issues
   together and decide what to revise: the book's "playtest, evaluate, and
   revise." Record the decision in the weekly note.

## Rewards

### Positions

- **Joining earns nothing.** Installing from the public link or the APK,
  opening the app, reading the brief, or being in the TestFlight group
  earns no XP, no title, and no **PLAYTESTER** tag. There's no automatic
  tag for testers.
- **XP only for evidence-backed, accepted contributions**, as
  [agent trainer leveling](../verse/agent-trainer-leveling.md#principles)
  requires. Never for time in the app, sessions started, reports filed,
  words written, or builds installed. A contribution counts only after a
  person on the OpenAgents side (a moderator or triager who isn't the
  tester) accepts it and records the acceptance.
- **A separate playtest referee key.** Playtest awards are signed by an
  OpenAgents *playtest* referee, not the agent-trainer referee. A reader
  that trusts only the trainer referee sees a trainer level unaffected by
  playtesting; a reader that trusts both sees both. The Account tab shows
  **Playtest XP** next to trainer XP, never summed into one level under the
  `trainer-curve-v1` name. Testing an app is valuable, but it isn't training
  agents, and the trainer level has to keep meaning what it says.
- **No money.** No bitcoin, no gift cards, and no promise of either. If quest
  purses ever arrive, they follow the leveling spec's
  [prerequisites](../verse/agent-trainer-leveling.md#rewards), and
  playtesting would be a later candidate, not a first one.
- **Cosmetics are readings of titles, not items.** There's no inventory or
  item ledger, and we don't add one: an item ledger is a new economy with
  its own farming and trading problems. A cosmetic is something the client
  draws because a key holds a title (a NIP-32 label that points at a counted
  award). It can't be sold, transferred, or bought.

### What earns playtest XP

Every row is a quest version published by the playtest referee in season
`playtest-s1` (2026-09-29 to 2026-10-26). The amounts sit in the leveling
spec's tutorial and daily tiers. Each row names the contribution, not the
activity: a session that produces no report, or a report that isn't
accepted, earns nothing.

| Contribution | Counts when | Evidence the award cites | XP | Uniqueness |
| --- | --- | --- | --- | --- |
| Accepted feedback | Submitted feedback (bug, confusing state, or design point) is accepted by a triager as a new `playtest` issue | The tester's report (TestFlight feedback, GitHub issue, email, or later the in-app report) and the `playtest` issue filed or kept from it | 10 | First accepted reporter per issue |
| Reproducible bug report, P2 or P3 | The triager reproduces the bug from the tester's steps and labels its severity | Same, with the `reproduced` note and the severity label | 20 | First accepted reporter per issue |
| Reproducible bug report, P0 or P1 | Same | Same | 50 | First accepted reporter per issue |
| Design finding that shipped | An accepted design finding led to a change in a shipped build | The issue and the commit that closes it | 30 | First accepted reporter per issue |
| Verified fix | The reporter confirms the fix on the fixing build and the triager records it | The confirmation comment on the issue naming the build | 10 | Once per issue |
| Completed session script with a report | A moderated session, an unmoderated task list, or a group session is completed **and** the tester's written report on it is accepted | The moderator's signed session record (moderated and group) or the accepted report naming the script and build (unmoderated) | 15 (unmoderated), 25 (moderated or group) | Once per tester per script per season |
| Completed diary week with a report | Five entries on five different days plus a short summary, accepted by the facilitator | The accepted diary summary | 20 | Once per tester per season |

These earn nothing: joining, installing a build, opening the app, a session
with no accepted report, a duplicate, a declined report, a report that can't
be reproduced (it can earn later if it's reproduced), and a verification of
someone else's issue.

A full, active season (three scripts with reports, a diary, a few
reproducible bugs, and verifications) comes to roughly 250 to 400 playtest
XP.

### How it fits NIP-XP

NIP-XP's only rule today is `kb-transfer`, and readers refuse unknown rules.
Playtest awards need a new rule, proposed here as `playtest`, with its own
fixtures, as NIP-XP requires of "a future rule":

- **Roles:** `tester` (the awardee) and `moderator` or `triager` (the key
  that recorded the session or accepted the report). The tester is never
  the moderator or the triager.
- **What a reader re-checks from signed events alone:** the tester signed the
  report or appears in the signed session record; the event is inside the
  season; it names a build in the season's build list; the award binds the
  exact quest version and amount; one award per uniqueness key (issue number
  or tester and script), using the proposed `per-awardee` policy with a
  stated maximum.
- **What a reader can't re-check:** whether the bug was real and severe.
  That's the referee's judgment, recorded on a public issue. We say so
  plainly: `playtest` is a weaker rule than `kb-transfer`, which is exactly
  why it has its own referee key and never feeds the trainer level.

The rule is specified in
[NIP-XP, `playtest`](../../nips/openagents/NIP-XP.md#playtest) and
implemented in `crates/nostr` (`xp::playtest`), `knowledge::xp::derive`, and
`microcoder xp` (2026-09-28). Its uniqueness keys are rule-derived (one
report-class award per issue, one session award per tester and script)
with a per-quest `max_awards`; the general `per-awardee` policy (#9894) may
later subsume them. The app reads it through `verse::xp::playtest_card`
and `playtest_titles` once `PLAYTEST_REFEREE` holds the owner's key.

Until the referee key and the app's playtest reader exist, the triage
log records every acceptance with the tester's key, the issue, and the date.
Awards are signed from that log when the rule lands; evidence dated inside
the season stays valid, as the leveling spec says of closed seasons.

### Titles and cosmetics

Titles are NIP-32 labels (`L=openagents.xp`) signed by the playtest referee
and pointing at one counted award, as NIP-XP specifies.

| Title | Earned by | Cosmetic in the Grid |
| --- | --- | --- |
| `playtester` | The tester's first counted playtest award, which means a first accepted contribution; never joining | The word **PLAYTESTER** under the name tag |
| `founding-playtester` | A counted contribution award in `playtest-s1` | A thin white ring on the ground under the avatar, forever |
| `bug-hunter` | Three counted accepted-bug awards in one season | A small crosshair mark beside the name tag |
| `fix-verifier` | Five counted fix-verified awards | A check mark beside the name tag |
| `raider` | A counted group-session award (the session plus an accepted report) | The ball glows briefly when this player pushes it |

All of these read labels through the mobile XP reader that phase 1 of
[#9847](https://github.com/OpenAgentsInc/openagents/issues/9847) adds, and
render only for keys that published a trainer profile, following the
leveling spec's [privacy](../verse/agent-trainer-leveling.md#privacy) rule
that boards and tags are opt-in. The Grid stays white and gray: every
cosmetic is a shape, not a color.

Status (2026-09-28, [#9886](https://github.com/OpenAgentsInc/openagents/issues/9886)):
the Grid draws these in `crates/coder-mobile` (`verse_app.rs`,
`playtest_marks` and `raider_glow`) from `verse::xp::playtest_titles`,
through a second read-only reader that trusts the playtest referee alone.
It reads nothing until `PLAYTEST_REFEREE` holds the owner's key; the
`--xp-preview` launch argument shows a labeled fixture with every title.
The opt-in gate (a published trainer profile) isn't built yet: name tags
show levels and titles for every key today, and #9895 adds the gate.

### Anti-farming

The leveling spec's defenses apply. In addition, for playtesting:

- **Duplicates earn nothing.** Only the first accepted reporter of an issue
  gets the bug award. A later report that adds a missing reproduction is
  thanked on the issue, not awarded.
- **Declined reports earn nothing**, and there's no negative XP for them.
- **Split reports are merged.** One problem filed as five reports is one
  issue and one award.
- **No self-dealing.** Keys that belong to OpenAgents staff, or to anyone
  with commit access, get titles for the record but no playtest XP, and the
  referee never awards a key it controls.
- **Sessions are capped** by uniqueness: one award per tester per script,
  per season, and only with an accepted report.
- **An open program means more keys, not more XP.** Anyone can join, so
  every award still needs a person to accept a contribution; installing
  from many keys earns nothing.
- **Sybil testers are a recruiting decision.** Moderated and group awards
  need a human on a call. Report awards need a human triager to accept an
  issue. That caps what a farm of keys can earn to what it can get past a
  person.
- **Revocation is public.** A fabricated report or session is revoked with a
  reason, and its titles and cosmetics go with it.

## Day-0 launch checklist (2026-09-29)

Coder and Verse launch to playtesters on Tuesday 2026-09-29. Season 1
(`playtest-s1`) starts the same day and runs four weeks, to 2026-10-26.

### What testers get

- **iOS:** the OpenAgents app (`com.openagents.app`), version 1.0.0, from
  the **public TestFlight link**. Build 21 is the newest: the Gym in chat,
  where you test a tool, make your own, add results to the Gym, check
  other trainers' results, and earn XP. Build 20 brought the chat router's
  prepared answers and offers, and a simpler Wallet. Testers update from
  TestFlight.
- **The mockup** (OpenAgents Mockup) for [lo-fi sessions](#lo-fi-playtest),
  from a facilitator's phone until it has its own TestFlight app.
- **Android:** the signed OpenAgents APK from
  [`bins/openagents-android`](../../bins/openagents-android/README.md),
  linked publicly next to the TestFlight link. Android has Chat, Verse (the
  Grid and the Gym and RESULTS panels; the Lagrange 1 portal is hidden),
  Wallet, and Account, and
  the build has run on the emulator but not yet on a range of devices
  ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)).
- **The brief** from [Onboarding, consent, and privacy](#onboarding-consent-and-privacy),
  the known issues below, and the three ways to give feedback.
- **Nothing for joining.** XP and titles come only from accepted
  contributions ([Rewards](#rewards)).

### What to test first

1. **First run, no help** (everyone): install, open, and say what the app is
   for; find the build in Account, **About this device**; read the
   **Changelog**.
2. **The Grid** (everyone): walk, look, jump, zoom; push the ball; knock the
   blocks over and use the reset pillar; open the Gym's **RESULTS** board and
   play a replay. With another tester online, check that you see each other
   and share the ball.
3. **Chat with OpenAgents** (everyone, no computer needed): ask who you're
   talking to, what it costs, and what it can do; check each prepared answer
   shows at once and is right; tap a follow-up chip; tap **Wrong answer**
   under any answer that's wrong.
4. **Coder on your computer** (testers with a Mac or Linux computer): run
   `coder host serve --tailnet-admission standard`, connect it from **Connect
   a computer**, ask chat to work on a project, tap **Run Coder on** it, send
   a follow-up while it works, stop a run, and open an older chat from the
   menu at the top left.
5. **Wallet** (adults, tiny amounts only): back up the recovery words from
   the **Back up your wallet** card, tap **Receive** and get paid a small
   amount, **Send** part of it back, and find both in **Recent activity**.

Session scripts [1](#session-1-first-run-chat-and-coder),
[2](#session-2-the-grid-and-the-gym), and
[3](#session-3-wallet-receive-and-send-tiny-amounts) are the long form; the
unmoderated task list is the five items above, and the
[lo-fi playtest](#lo-fi-playtest) is the first-timer version.

### Known issues on day 0

Published with the link so testers don't spend reports on them:

- **In-app reports wait on the phone.** Build 16 and later have **Report a
  problem**; it sends once a build carries the triage key. Until then, also
  use TestFlight feedback, the GitHub template, or email
  ([Stage 0](#stage-0-what-works-today-launch-day)).
- **Chat with OpenAgents needs no computer**; its reply streams in, and
  common questions get a prepared answer at once, with the router's offers
  and **Wrong answer** from build 20. The chat's daily limit is 40
  messages per user.
- **Coder on your own code needs your own computer** on the same tailnet,
  running the Coder host; there's no hosted computer. Coder's reply streams
  in paragraph by paragraph, and chats open in one or two reads
  (`358975bdbd`, `fe6cf24aa2`). No photo attachments, voice dictation,
  model picker, or push notifications yet
  ([what comes later](../../bins/openagents-ios/docs/chat-later.md)).
- **The Grid has no chat** and no name-tag levels; presence shows a pubkey
  prefix. The Lagrange 1 portal is hidden. Shared ball and block state can
  lag between players.
- **Wallet:** real mainnet bitcoin; use amounts you can lose. Receiving to a
  Lightning address of your own isn't built yet
  ([#9859](https://github.com/OpenAgentsInc/openagents/issues/9859)); paying
  other users by npub and unclaimed deposits landed (`f0466f1b15`,
  `d43b304e87`). Amounts show in BIP 177 form (`₿12,345`), with legacy BTC
  as a choice (`dfe066f267`).
- **Android:** the APK has the Wallet since `e56d173480`, and **Report a
  problem** and the playtest log since
  [#9903](https://github.com/OpenAgentsInc/openagents/issues/9903); it has
  run on the emulator only
  ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)), not
  yet on physical devices for Vulkan, QR scanning, the terminal, or motion
  look.
- **Rewards aren't visible in the app yet.** Accepted contributions are
  recorded in the triage log and signed later
  ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885),
  [#9886](https://github.com/OpenAgentsInc/openagents/issues/9886),
  [#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)).

### Launch-day steps

1. Owner: turn on the public link for the external group with the newest
   build (build 21 once it is uploaded; build 20 until then), and publish
   the APK (workspace `NEEDS_OWNER.md`).
2. Post the TestFlight link, the APK link, the brief, and the known issues
   publicly (Nostr and X).
3. Open the triage log; read TestFlight feedback, new `playtest` issues, and
   the playtest inbox at least once on day 0.
4. Owner self-test with a fresh mind (sessions 1 and 2), notebook open.
5. Ask two or three confidants for a moderated session this week.

## Season 1, week by week

Each week has a goal, the book's circle it leans on, and an exit check.

### Week 1 (2026-09-29 to 10-05): open launch, self, and confidants

- **Goal:** strangers can install and play without us; every P0 and P1
  from launch is fixed in a new build.
- Day-0 checklist above. Daily triage of all three channels.
- Three to five moderated sessions with informal confidants (session 1 and
  session 2); the owner's own fresh-mind notebook.
- Run five [lo-fi sessions](#lo-fi-playtest) with first-timers, and file
  every missed threshold.
- Ship each new build to the public link (build 20, then build 21), then a build
  with the week's P0 and P1 fixes, each with a
  Changelog line saying what to test.
- Draft the paper prototype for week 3.
- **Exit:** no open P0; every P1 has a fix in a build or a stated reason;
  at least ten outside testers have installed a build.

### Week 2 (2026-10-06 to 10-12): people we don't know

- **Goal:** strangers finish the first-run and Grid tasks unaided, and the
  wallet session is safe.
- Invite public testers who already use a Lightning wallet to three
  moderated [wallet sessions](#session-3-wallet-receive-and-send-tiny-amounts).
- Publish the unmoderated task list for the current build.
- Start diaries with five volunteers.
- Land the **Report a problem** action
  ([#9882](https://github.com/OpenAgentsInc/openagents/issues/9882)) if it's
  ready, and the triage inbox
  ([#9884](https://github.com/OpenAgentsInc/openagents/issues/9884)).
- **Exit:** unaided completion of session 1's pairing-free tasks is at least
  60 percent, and no wallet session lost bitcoin to an app error.

### Week 3 (2026-10-13 to 10-19): raids and the paper trainer loop

- **Goal:** find out whether the Grid is fun with others, and whether the
  agent training loop is worth building as specified.
- Run two group sessions ("raids") in the Grid, five to eight testers each,
  on a voice call: stack the blocks together, push the ball to the Gym and
  back, reset with the pillar, and meet in the Gym at the RESULTS board.
- Run six paper-prototype sessions of agent training: show a trainer card,
  the quest board, a reproduce quest, a raid charter, and the titles; ask
  testers to plan a week and explain what they'd chase and why.
- Invite target players (gamers who use a coding agent) into sessions.
- **Exit:** a written decision on what to change in
  [agent trainer leveling](../verse/agent-trainer-leveling.md) before its
  phase 1 ships, and a list of Grid changes ranked by how often testers
  asked for them.

### Week 4 (2026-10-20 to 10-26): verify, measure, reward

- **Goal:** close the loop: fixes verified by the people who found them,
  the questionnaire in, and the first acceptances recorded as awards.
- Ask every reporter to verify their fixed issues on the latest build.
- Send the [questionnaire](#questionnaire) to every tester who played.
- Run a retrospective: what the numbers and the notes say, and what we
  change for season 2 (including Android Wallet testers once
  [#9861](https://github.com/OpenAgentsInc/openagents/issues/9861) ships).
- If the `playtest` rule and referee key are ready
  ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)), sign
  the first awards from the triage log.
- **Exit:** the [first milestone](#success-metrics-and-the-first-milestone).

## Session scripts

Each moderated script follows the book's five parts. The moderator reads the
**bold** lines; everything else is notes.

### Session 1: first run, chat, and Coder

Chat needs no computer. Testers with a Mac or Linux computer they are
willing to connect also do the parts marked *(computer)*; testers without
one skip them.

- **Introduction (2 to 3 minutes).** **"Thanks for helping. We're testing
  the app, not you. Please think out loud the whole time: what you see, what
  you expect, what confuses you. I'll mostly stay quiet. Is it OK to record
  the screen and audio?"** Note the answer.
- **Warm-up (5 minutes).** **"What do you use on your phone to do work on
  your computer, if anything? Have you used a coding agent? What for?"**
- **Play (15 to 20 minutes).** Give one task at a time; don't lead.
  1. **"Install the build from TestFlight and open it. Tell me what you think
     this app is for."** Note their answer before they tap anything.
  2. **"Find out which version and build you have."** (Account, **About
     this device**.)
  3. **"Ask the app who you're talking to, and what it costs."** (The Chat
     tab opens ready to type.) Note whether the prepared answers show at
     once and whether the tester trusts them.
  4. *(computer)* **"Connect your computer to the app."** Let them find
     **Connect a computer**. Note every step where they leave the app to
     read instructions, and each intervention.
  5. *(computer)* **"From your phone, have Coder list the files in a
     folder on your computer and tell you what the project is."** Watch
     them ask in chat and find **Run Coder on** or the computer in the
     selector.
  6. *(computer)* **"While it works, ask it to do one more thing after
     this."** (Queue, from the long press on send.)
  7. *(computer)* **"Stop it."**
  8. **"Find what changed in this build."** (Account, **Changelog**.)
- **Discussion (15 to 20 minutes).** **"What was that like? What surprised
  you? Where did you hesitate? Did you trust it to run things on your
  computer? What would make you open it tomorrow?"**
- **Wrap-up.** **"Anything else? Here's how to send reports from here
  on."** Explain stage 0 or stage 1 reporting, and thank them.

### Session 2: the Grid and the Gym

No computer needed.

- **Introduction (2 to 3 minutes).** As in session 1.
- **Warm-up (5 minutes).** **"What games do you play? Have you played an
  MMO? What makes you keep going back to one?"**
- **Play (15 to 20 minutes).**
  1. **"Open the tab with the globe. Tell me what you see and what you think
     you can do here."** Then say nothing for two minutes, and note every
     unprompted action.
  2. **"Walk to the ball and push it."** (Left stick.)
  3. **"Look around without walking."** (Right stick or drag.) **"Now jump,
     and zoom all the way in."**
  4. **"Knock something over."** (The stack or the dominoes.) **"Now put it
     all back."** (The reset pillar to the right of the spawn.)
  5. **"Go into the big building ahead and find out what it's showing."**
     (The Gym; tap the **RESULTS** board.) **"Pick one attempt and watch
     how it went."** (Open a replay and play it.) Ask what they think the
     ghost under the board is doing.
- **Discussion (15 to 20 minutes).** **"Was any of that fun? Which part?
  What did you want to do that you couldn't? Would you come back here with
  a friend? What do you think the Gym is for?"**
- **Wrap-up.** As in session 1.

### Session 3: Wallet, receive and send tiny amounts

For adults who already use a Lightning wallet. The moderator has a funded
Lightning wallet and a Lightning address. The amounts are test fixtures, not
a reward: the tester returns them, and can skip any step.

- **Introduction (2 to 3 minutes).** As in session 1, plus: **"This wallet
  is real Bitcoin. We'll move about ₿100, a hundred of bitcoin's smallest
  units and less than a few cents. I'll
  never ask for your recovery words. Please don't show them on screen."**
  Turn off recording before the recovery-words step, or ask the tester to
  turn the phone away.
- **Warm-up (5 minutes).** **"Which wallets do you use? What makes you trust
  a new one?"**
- **Play (15 to 20 minutes).**
  1. **"Open the wallet and tell me what it's telling you."** (The balance,
     **Receive**, **Send**, and **Recent activity**.) Then: **"Find out who
     holds your money in this wallet."** (The **i** button, the trust
     note.) Note whether the ₿ amount makes sense, and whether the tester
     finds **Show amounts as** under **Advanced**.
  2. **"Back up the wallet the way you'd back up any wallet."** (The **Back
     up your wallet** card.) Camera off. Note hesitation and whether they
     write the words down.
  3. **"I want to send you ₿100. Give me something to pay."**
     (**Receive** shows a QR code at once; an amount is optional.) The
     moderator pays; note how long the tester takes to believe it arrived.
  4. **"Send ₿90 back to this address."** Paste the moderator's Lightning
     address in the call chat. (**Send**, **Paste or scan**, the amount,
     the confirm screen.) Note whether they check the fee before
     confirming.
  5. **"Find both payments."** (**Recent activity**.)
  6. **"Find how you'd buy bitcoin with dollars, but stop before paying."**
     (**Advanced**, **Buy bitcoin**; stop at the provider page.)
- **Discussion (15 to 20 minutes).** **"Would you keep money here? How much?
  What would change that number? What worried you?"**
- **Wrap-up.** As in session 1. Remind them to keep the recovery words.

### Group session outline ("raid")

Five to eight testers, one moderator on the call and one in the Grid.

1. **Assemble at the spawn** (5 minutes). Everyone reads out their name tag
   prefix so players can find each other.
2. **Build** (10 minutes): stack the blocks into one tower together.
3. **Push** (10 minutes): push the ball together to the Gym's doors and
   back to the spawn.
4. **Reset** (2 minutes): one player walks into the pillar; everyone
   confirms it reset for them.
5. **Gym** (10 minutes): meet at the RESULTS board, and each open the same
   attempt's replay.
6. **Debrief** (15 minutes): what was fun, what got in the way, what they
   needed to say to each other that the Grid couldn't carry.

## Questionnaire

Sent at the end of week 4, and after any tester's third session. Scales run
from 1 (strongly disagree) to 5 (strongly agree).

1. Which build did you use most? (Account, **About this device**.)
2. Which tabs did you use? Chat, Verse, Wallet, Account.
3. I understood what the app is for within the first minute. (1 to 5)
4. Chat answered my questions quickly and correctly. (1 to 5)
5. Connecting a computer was easy. (1 to 5, or "didn't try")
6. I would trust Coder to run commands on my computer from my phone. (1 to 5)
7. Moving around the Grid felt good. (1 to 5)
8. I understood what the Gym's RESULTS board and replays show. (1 to 5)
9. I trust the Wallet with a small amount. (1 to 5) What amount would you
   keep in it?
10. The Changelog told me what to test in a new build. (1 to 5)
11. Sending a report was easy. (1 to 5)
12. After reading about agent trainer XP and titles, I'd want to level up.
    (1 to 5) What would you do first?
13. How often did you open the app in the last week without being asked?
14. What is the one thing you'd change first?
15. What is the one thing we must not change?
16. Would you recommend the app to a friend who uses a coding agent? Why or
    why not?

## Success metrics and the first milestone

For season 1 (2026-09-29 to 2026-10-26):

| Metric | Target |
| --- | --- |
| Outside testers who complete at least one moderated session | 12 |
| Outside testers who install at least two builds (iOS or Android) | 25 |
| Unaided completion on session 1's tasks without a computer | 80 percent by week 4 |
| Lo-fi rounds that meet every [lo-fi threshold](#lo-fi-success-thresholds) | One by week 4 |
| Unaided pairing of a computer (session 1, task 3) | 50 percent by week 4, up from whatever week 1 shows |
| Accepted `playtest` issues | 30 |
| Median time from an accepted P0 or P1 to a TestFlight build with the fix | 2 days |
| Fixes verified by the reporter | Half of all fixed issues |
| Wallet sessions that lost bitcoin to an app error | 0 |
| Privacy incidents (a key, recovery word, or private text made public) | 0 |
| Day-7 return reported by diarists | 3 of 5 |
| A written decision on agent trainer leveling from the paper sessions | Yes |

**First milestone:** by the end of week 4, a person outside OpenAgents has a
reproducible bug report accepted from a public TestFlight or APK build, verifies the fix on a later
build, and holds a recorded playtest acceptance for it. When the `playtest`
rule and the mobile XP reader from
[#9847](https://github.com/OpenAgentsInc/openagents/issues/9847) land, that
acceptance becomes a signed award, and **PLAYTESTER** shows under their name
tag in the Grid. The milestone doesn't wait for the tag: the acceptance is
the evidence, and the award can be signed later.

## Implementation tasks

Tracked in epic [#9888](https://github.com/OpenAgentsInc/openagents/issues/9888). In order:

1. **Report action in the OpenAgents app.** ([#9882](https://github.com/OpenAgentsInc/openagents/issues/9882)) Account **Report a problem** and
   a long press on the tab bar; the fields in
   [stage 1](#stage-1-a-report-action-in-the-app); screenshot off by default
   with a preview, never on the Wallet or **Identity keys**; sent as a
   NIP-17 message to the triage key, signed by the world key; **My
   reports** in Account. Rust in `crates/openagents-mobile`, thin SwiftUI
   in `bins/openagents-ios/host`; thin Kotlin in `bins/openagents-android/host`
   ([#9903](https://github.com/OpenAgentsInc/openagents/issues/9903)).
2. **Local playtest log** (built opt-in, now on by default) ([#9883](https://github.com/OpenAgentsInc/openagents/issues/9883)), with new `INVARIANTS.md` rows and
   tests for what it never records and that it leaves the device only in a
   previewed report.
3. **Triage inbox tool.** ([#9884](https://github.com/OpenAgentsInc/openagents/issues/9884)) A command that reads the triage key's reports,
   deduplicates them, and drafts GitHub issues with the `playtest`, build,
   and area labels for a person to approve; it records every acceptance in
   the triage log with the tester's key.
4. **NIP-XP `playtest` rule and the playtest referee key.** ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)) Specify the rule,
   the roles, the playtest report event, and the session record; depends on
   the leveling spec's proposed `per-awardee` policy. Fixtures in
   `crates/nostr` (`xp`) and `crates/knowledge` (`xp::derive`), and support
   in `microcoder xp`.
5. **Playtest titles and cosmetics in the Grid** ([#9886](https://github.com/OpenAgentsInc/openagents/issues/9886)), reading labels through the
   mobile XP reader from #9847's phase 1: the **PLAYTESTER** line, the
   founding ring, and the marks, for opted-in keys only.
6. **Playtest card in Account** ([#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)): sessions, accepted reports, fixes verified,
   playtest XP (not summed into the trainer level), and titles.
7. **Changelog per build.** ([#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)) Give each TestFlight build a Changelog entry with
   a "What to test" line, so testers know where to look.
8. **Program operations (not code).** The public TestFlight link, the
   public APK, the brief, the known issues list, the `playtest` label and
   GitHub issue template, the playtest email, the scripts, the triage log,
   and the weekly note. Owner steps are in the workspace's `NEEDS_OWNER.md`.

The eval loop that the lo-fi round's revision 3 tasks test is its own epic,
[#9931](https://github.com/OpenAgentsInc/openagents/issues/9931): the
mockup's update is [#9940](https://github.com/OpenAgentsInc/openagents/issues/9940),
and the lo-fi round on the mockup and on build 21 is part of
[#9941](https://github.com/OpenAgentsInc/openagents/issues/9941).

## Open questions

1. **Which key does a tester use?** We say the Verse world key, so XP shows
   over their head. A tester who already has a trainer key needs the
   leveling spec's two-sided key link, which is phase 2.
2. **Should playtest XP ever count toward the trainer level?** We say no.
   Revisit if players read the two numbers as one anyway.
3. **Does App Store Connect's TestFlight feedback reach a tool we can
   run?** Yes: the App Store Connect API lists beta feedback screenshot and
   crash submissions, and `openagents playtest testflight` reads them into
   the triage inbox ([#9905](https://github.com/OpenAgentsInc/openagents/issues/9905)).
   They carry no Nostr key, so they don't back playtest awards.
4. **Android.** Android testers join from day 0 through the public APK.
   When does it move to a Play Store testing track, and when does its Wallet
   ([#9861](https://github.com/OpenAgentsInc/openagents/issues/9861)) reach
   parity?
5. **Who moderates?** The owner can't run 20 sessions a week. Can an agent
   run unmoderated task lists and triage, and a small group of trusted
   players moderate sessions, earning `moderator` titles?

## Related documents

- [Launch roadmap, 2026-09-29](../roadmap/2026-09-29-launch-roadmap.md): the
  MVP that ships to playtesters and the milestones after it
- [Agent trainer leveling](../verse/agent-trainer-leveling.md) and epic
  [#9847](https://github.com/OpenAgentsInc/openagents/issues/9847)
- [NIP-XP](../../nips/openagents/NIP-XP.md)
- [Phone app wireframe](../product/2026-09-28-app-wireframe.md): spec IDs,
  flows, the IDIOT PROOF checklist, and the v1 cut
- [OpenAgents Mockup](../../bins/openagents-mockup-ios/README.md)
- [OpenAgents for iOS](../../bins/openagents-ios/README.md)
- [Verse in the OpenAgents app](../verse/mobile.md), the
  [Gym building](../verse/gym.md), the
  [Gym leaderboard](../verse/gym-leaderboard.md), and
  [Lagrange 1](../verse/lagrange-1.md)
- [Verse game design document](../verse/gdd.md)
- [Games, MMORPGs, and 3D worlds in OpenAgents](README.md)
- [`INVARIANTS.md`](../../INVARIANTS.md), Phone wallet
