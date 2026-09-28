# Playtesting program

> **Status: Active from 2026-09-29.** Written 2026-09-28 and revised the same
> day with the owner's decisions: the program is **open** (a public
> TestFlight link and a public Android APK; anyone can join), and **joining
> earns nothing by itself**: the **PLAYTESTER** title, other titles, and
> playtest XP come only from contributions. Coder and Verse launch to
> playtesters on **Tuesday 2026-09-29**; see the
> [day-0 launch checklist](#day-0-launch-checklist-2026-09-29) and the
> [launch roadmap](../roadmap/2026-09-29-launch-roadmap.md), which ties this
> program to the MVP and the milestones after it. What exists today is listed
> in [What exists today](#what-exists-today); everything else is planned.
> Nothing in this document pays testers, and no payout is promised.
> Implementation is tracked in [Implementation tasks](#implementation-tasks)
> and epic [#9888](https://github.com/OpenAgentsInc/openagents/issues/9888).

The OpenAgents app went from build 1 to build 15 on TestFlight in two days.
Each build so far was tested by the same person who asked for it: the owner
installs it, writes notes such as "1.0.0 Build 13 Feedback", and agents turn
the notes into commits and the next build. That loop is fast and it works,
but it has one player. This document turns it into an open program with many
players, a method, and a reward that fits the rest of the game: XP for
evidence-backed, accepted contributions, titles and cosmetics in the Grid, and
no money.

## Contents

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
| Coder chat | Does commanding your own computer from your phone feel trustworthy and worth coming back to? | It is the product's reason to exist; pairing a computer is the steepest step. |
| The Grid (Verse tab) | Is moving around, pushing the ball, and meeting people fun for five minutes with nothing to win? | The book's warning: if the core isn't fun by beta, "you are stuck with it." |
| The Gym and Lagrange 1 | Does a player understand what the RESULTS board and a trace replay show, and want to see more? | The Gym is where agent training will happen. |
| Wallet | Can a person receive and send a small amount on mainnet correctly, and do they trust it? | Real money: errors cost testers real bitcoin. Correctness beats fun here. |
| Agent training | Would a player want to level up as an agent trainer, and do the rules feel fair? | The loop is specified, not built: the cheapest time to change it is now. |

Non-goals: load testing the relay, security review of the wallet, and
marketing research (the book's "focus group testing"). Those have their own
owners.

## What exists today

Status words follow the [glossary](../glossary.md).

| Piece | Status |
| --- | --- |
| The OpenAgents app on iOS (`com.openagents.app`) with four tabs: Coder, Verse (the Grid), Wallet, and Account ([README](../../bins/openagents-ios/README.md)) | Implemented. Version 1.0.0 builds 1 to 14 went to TestFlight on 2026-09-28; build 15 (`be94321643`) carries that day's fixes and follows build 14 to testers. |
| OpenAgents for Android ([`bins/openagents-android`](../../bins/openagents-android/README.md)), the same Rust library | Partial ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)). Coder, Verse (with the Gym and RESULTS panels, `82663b935d`), Wallet (`e56d173480`, `e1aeec7413`; [#9861](https://github.com/OpenAgentsInc/openagents/issues/9861) closed), and Account work. Verified on the emulator only; a live tailnet chat, QR scanning, the terminal, and motion look haven't been checked on a device. Distributed as a signed release APK (1.0.0, version code 16, the iPhone build number) from a GitHub release that testers install by hand. |
| Public distribution | Planned for 2026-09-29: a public TestFlight link for iOS and a public APK download for Android (owner steps in the workspace's `NEEDS_OWNER.md`). |
| TestFlight's own feedback: a tester takes a screenshot or uses **Send Beta Feedback** in the TestFlight app, and it reaches App Store Connect with the build number, device, and OS. Crash reports reach it too. | Exists, from Apple. Nothing in this repository reads it. |
| The owner's build notes ("1.0.0 Build 13 Feedback") turned into commits by agents, followed by a build bump (for example `e84de16fd5`, `06d033d663`) | The current loop. It isn't written down anywhere except in commit history. |
| **Changelog** in Account (`crates/openagents-mobile/src/account.rs`) | Implemented. One entry per TestFlight build with a **What to test** line, from build 16 (`a4aa3013de`); a test ties the newest entry to the build number in `project.yml`. |
| **Report a problem**, **My reports**, and the opt-in **Playtest session** log in Account, and a long press on the tab bar (`crates/playtest`, `crates/openagents-mobile/src/playtest.rs`; iOS and Android) | Implemented in build 16 (`74f2f90be0`) on iOS, and on Android with the Account playtest card ([#9903](https://github.com/OpenAgentsInc/openagents/issues/9903)); Android reports name the `android` platform. Reports are sealed to the triage key, which the owner hasn't created yet, so until a build carries it reports wait on the phone. No telemetry: the app sends nothing but a report the tester files. |
| Triage inbox and triage log: `openagents playtest` reads the triage key's reports, drafts `playtest` issues for a person to approve, and records every acceptance ([playtest-triage.md](playtest-triage.md)) | Implemented ([#9884](https://github.com/OpenAgentsInc/openagents/issues/9884)). It reads reports once the owner creates the triage key and a build carries it. |
| NIP-XP quests, awards, revocations, and achievement labels; the ledger; the referee tool ([NIP-XP](../../nips/openagents/NIP-XP.md)) | Implemented, with three rules: `kb-transfer`, `reproduce`, and `playtest` ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)). The `playtest` rule, its report (`3197`) and session record (`3196`), and `microcoder xp playtest-keygen`/`playtest-session` exist; the playtest referee key doesn't yet (an owner step), so no playtest award counts. |
| Levels, titles, and `lv n` name tags | Implemented on desktop Verse only. The Grid's name tags show a pubkey prefix and no level. |
| A read-only XP reader and trainer card in the app | Specified; phase 1 of epic [#9847](https://github.com/OpenAgentsInc/openagents/issues/9847), in progress. |
| NIP-17 private messages and NIP-44 encryption | Implemented in `crates/nostr` (`nip17.rs`, `nip44.rs`). |
| Chat in the Grid | None. The Grid has presence, a shared ball and blocks, the Gym, and the Lagrange 1 portal; talking happens outside the app. |

## What we test, by stage

The book's Chapter 10 splits testing by stage: foundation, structure, formal
details, and refinement. Each OpenAgents surface is at a different stage, and
the stage decides the kind of test and the kind of report we want.

| Surface | Stage | Test for | Wanted | Not yet wanted |
| --- | --- | --- | --- | --- |
| Agent training loop | Foundation | Is it fun and fair at all? | Reactions to a paper prototype | Bug reports: there's no code |
| The Grid | Foundation to structure | Fun, and what players do unprompted | Play-matrix notes, "what did you try" | Pixel polish |
| Gym and Lagrange 1 | Structure | Internally complete: dead ends, loopholes | Where players got stuck or lost | Balance |
| Coder chat | Formal details | Functional: every state has a way out | Reproducible bugs, confusing states | New features |
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
- **Coder needs a computer.** The Coder tab needs a Mac or Linux computer
  running the Coder host on the same tailnet. Testers without one test the
  Grid, the Gym, the Wallet (iOS), and Account, and that is a full program.
- **Adults only for the Wallet.** The Wallet runs on Bitcoin mainnet. The
  brief says so, and the scripted wallet session is only for testers who
  confirm they are 18 or older.
- **Say plainly what testers get:** nothing for joining, and XP and titles
  for accepted contributions; never money. See [Rewards](#rewards).
- **TestFlight limits apply.** The public link is an external group, so the
  first build of a version goes through Apple's Beta App Review, which can
  take a day, and the public link has a tester cap that the owner sets (up
  to Apple's 10,000). Not every internal build goes to the public link.
- **Android is behind iOS.** Android testers get Coder, Verse, Wallet, and
  Account, with **Report a problem**, **My reports**, **Playtest session**,
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
- **An opt-in, local session log** is the one exception we propose (see
  [task 2](#implementation-tasks)): the app records a short list of
  structural events (which tab, which screen, which error code, and when)
  only while the tester has turned on **Playtest session**, keeps it on the
  device, and sends it only attached to a report the tester previews. It
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
| **Moderated think-aloud** | One tester, one moderator, a call with screen sharing, the five-part session from the book | First run, Coder pairing, Wallet | 1, 2, 3 (invited) | 45 to 50 minutes of moderator time |
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

- **Where:** in Account, as **Report a problem**, and as a long press on the
  tab bar from any tab, so the report knows where the tester was.
- **What it fills in for the tester:** app version and build number, the tab
  and screen (route), device model, iOS version, the time, and, from the
  Coder tab, the chat's task ID only if the tester ticks it.
- **What the tester writes:** what happened, what they expected, and the
  steps, with a **Kind** choice: *bug*, *confusing*, *idea*, or *felt good*.
  "Felt good" is on purpose: the book asks what is fun, not only what is
  broken.
- **Screenshot:** off by default. When on, the app shows the exact image
  before sending, and lets the tester crop it. Never offered on the Wallet
  tab or on **Identity keys**.
- **Session log:** attached only when **Playtest session** is on, and shown
  in full before sending.
- **Transport:** a NIP-17 private message to the OpenAgents triage key,
  sealed with NIP-44, signed by the tester's Verse world key (so the XP it
  can earn lands on the key whose name tag shows in the Grid). Screenshots
  above the relay's size limit are compressed or dropped, with a note.
- **Receipt:** the report's event ID shows as a short code the tester can
  quote. **My reports** in Account lists what was sent and, later, what was
  accepted.

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

- **iOS:** the OpenAgents app (`com.openagents.app`), version 1.0.0 build 14
  or later, from the **public TestFlight link**. Build 15 (`be94321643`),
  with the fixes from 2026-09-28, follows as soon as it clears processing
  and Beta App Review; testers update from TestFlight.
- **Android:** the signed OpenAgents APK from
  [`bins/openagents-android`](../../bins/openagents-android/README.md),
  linked publicly next to the TestFlight link. Android has Coder, Verse (the
  Grid, the Gym and RESULTS panels, Lagrange 1), Wallet, and Account, and
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
   play a trace; go through the **LAGRANGE 1** arch and come back. With
   another tester online, check that you see each other and share the ball.
3. **Coder** (testers with a Mac or Linux computer on a tailnet): run
   `coder host serve --tailnet-admission standard`, sign in on the phone's
   **Tailnet** screen, start a new chat, send a follow-up while it works,
   steer or queue from the long press on send, answer a question, stop a
   run, and open an older chat from **Chats on your computers**.
4. **Wallet** (iOS, adults, tiny amounts only): read the trust note, back up
   the recovery words, receive a small Lightning payment, send part of it
   back, and find both in history.

Session scripts [1](#session-1-first-run-and-coder-chat),
[2](#session-2-the-grid-the-gym-and-lagrange-1), and
[3](#session-3-wallet-receive-and-send-tiny-amounts) are the long form; the
unmoderated task list is the four items above.

### Known issues on day 0

Published with the link so testers don't spend reports on them:

- **No in-app report on builds 14 and 15.** Use TestFlight feedback, the
  GitHub template, or email ([Stage 0](#stage-0-what-works-today-launch-day)).
  Build 16 adds **Report a problem**; it sends once a build carries the
  triage key.
- **Chat loading over the tailnet can be slow** on long chats; faster reads
  are in progress (host side landed in `4ef967aa40`; the Rust-laid-out
  transcript is [#9833](https://github.com/OpenAgentsInc/openagents/issues/9833)).
  A chat shows at most its newest 240 rows; replies arrive by polling every
  few seconds, not token streaming.
- **Coder needs your own computer** on the same tailnet, running the Coder
  host; there's no hosted computer. No photo attachments, voice dictation,
  model picker, or push notifications yet
  ([what comes later](../../bins/openagents-ios/docs/chat-later.md)).
- **The Grid has no chat** and no name-tag levels; presence shows a pubkey
  prefix. Shared ball and block state can lag between players.
- **Wallet:** real mainnet bitcoin; use amounts you can lose. Receiving to a
  Lightning address, paying other users by npub, and unclaimed on-chain
  deposits aren't built yet
  ([#9859](https://github.com/OpenAgentsInc/openagents/issues/9859),
  [#9860](https://github.com/OpenAgentsInc/openagents/issues/9860),
  [#9862](https://github.com/OpenAgentsInc/openagents/issues/9862)). Amounts
  show in sats and BTC; the BIP 177 display is
  [#9881](https://github.com/OpenAgentsInc/openagents/issues/9881).
- **Android:** the APK (1.0.0, version code 16, the iPhone build number) has the Wallet since
  `e56d173480`, but not **Report a problem** or **Playtest session**, which
  its Changelog lists for build 16
  ([#9838](https://github.com/OpenAgentsInc/openagents/issues/9838)); it has
  run on the emulator only, not yet on physical devices for Vulkan, QR
  scanning, the terminal, or motion look.
- **Rewards aren't visible in the app yet.** Accepted contributions are
  recorded in the triage log and signed later
  ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885),
  [#9886](https://github.com/OpenAgentsInc/openagents/issues/9886),
  [#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)).

### Launch-day steps

1. Owner: turn on the public link for the external group with build 14,
   submit build 15, and publish the APK (workspace `NEEDS_OWNER.md`).
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
- Ship build 15 to the public link, then a build with the week's P0 and P1
  fixes, each with a Changelog line saying what to test.
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
  on a voice call: stack the blocks together, push the ball through the
  Lagrange 1 arch, reset with the pillar, and meet in the Gym at the
  RESULTS board.
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

### Session 1: first run and Coder chat

For testers with a Mac or Linux computer they are willing to pair. Testers
without one skip parts marked *(computer)*.

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
  3. *(computer)* **"Connect your computer to the app."** Let them find
     **Add a computer**. Note every step where they leave the app to read
     instructions, and each intervention.
  4. *(computer)* **"Ask Coder, from your phone, to list the files in a
     folder on your computer and tell you what the project is."** Watch
     them find the Coder tab and send.
  5. *(computer)* **"While it works, ask it to do one more thing after
     this."** (Queue, from the long press on send.)
  6. *(computer)* **"Stop it."**
  7. **"Find what changed in this build."** (Account, **Changelog**.)
- **Discussion (15 to 20 minutes).** **"What was that like? What surprised
  you? Where did you hesitate? Did you trust it to run things on your
  computer? What would make you open it tomorrow?"**
- **Wrap-up.** **"Anything else? Here's how to send reports from here
  on."** Explain stage 0 or stage 1 reporting, and thank them.

### Session 2: the Grid, the Gym, and Lagrange 1

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
     how it went."** (Open a trace and play it.) Ask what they think the
     ghost under the board is doing.
  6. **"Find the arch that goes somewhere else. Go through it, look around,
     and come back."** (**LAGRANGE 1**, then **THE GRID** arch or the
     **The Grid** button.)
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
  1. **"Open the wallet and tell me what it's telling you."** Then: **"Find
     out who holds your money in this wallet."** (The **i** button, the
     trust note.) Note whether the ₿ amounts and the note explaining them
     make sense, and whether the tester switches to BTC.
  2. **"Back up the wallet the way you'd back up any wallet."** (Recovery,
     **Show recovery words**.) Camera off. Note hesitation and whether
     they write the words down.
  3. **"I want to send you ₿100. Give me something to pay."**
     (Receive, Lightning, an amount, **New invoice**.) The moderator pays;
     note how long the tester takes to believe it arrived.
  4. **"Send ₿90 back to this Lightning address."** Read the address
     aloud or paste it in the call chat. Note whether they check the fee
     and the range before confirming.
  5. **"Find both payments."** (History.)
  6. **"Find how you'd buy bitcoin with dollars, but stop before paying."**
     (Buy; stop at the provider page.)
- **Discussion (15 to 20 minutes).** **"Would you keep money here? How much?
  What would change that number? What worried you?"**
- **Wrap-up.** As in session 1. Remind them to keep the recovery words.

### Group session outline ("raid")

Five to eight testers, one moderator on the call and one in the Grid.

1. **Assemble at the spawn** (5 minutes). Everyone reads out their name tag
   prefix so players can find each other.
2. **Build** (10 minutes): stack the blocks into one tower together.
3. **Push** (10 minutes): push the ball through the **LAGRANGE 1** arch as
   a team, then come back.
4. **Reset** (2 minutes): one player walks into the pillar; everyone
   confirms it reset for them.
5. **Gym** (10 minutes): meet at the RESULTS board, and each open the same
   attempt's trace.
6. **Debrief** (15 minutes): what was fun, what got in the way, what they
   needed to say to each other that the Grid couldn't carry.

## Questionnaire

Sent at the end of week 4, and after any tester's third session. Scales run
from 1 (strongly disagree) to 5 (strongly agree).

1. Which build did you use most? (Account, **About this device**.)
2. Which tabs did you use? Coder, Verse, Wallet, Account.
3. I understood what the app is for within the first minute. (1 to 5)
4. Connecting a computer was easy. (1 to 5, or "didn't try")
5. I would trust Coder to run commands on my computer from my phone. (1 to 5)
6. Moving around the Grid felt good. (1 to 5)
7. I understood what the Gym's RESULTS board and replays show. (1 to 5)
8. I trust the Wallet with a small amount. (1 to 5) What amount would you
   keep in it?
9. The Changelog told me what to test in a new build. (1 to 5)
10. Sending a report was easy. (1 to 5)
11. After reading about agent trainer XP and titles, I'd want to level up.
    (1 to 5) What would you do first?
12. How often did you open the app in the last week without being asked?
13. What is the one thing you'd change first?
14. What is the one thing we must not change?
15. Would you recommend the app to a friend who uses a coding agent? Why or
    why not?

## Success metrics and the first milestone

For season 1 (2026-09-29 to 2026-10-26):

| Metric | Target |
| --- | --- |
| Outside testers who complete at least one moderated session | 12 |
| Outside testers who install at least two builds (iOS or Android) | 25 |
| Unaided completion on session 1's tasks without a computer | 80 percent by week 4 |
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
2. **Opt-in local playtest session log** ([#9883](https://github.com/OpenAgentsInc/openagents/issues/9883)), with new `INVARIANTS.md` rows and
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

## Open questions

1. **Which key does a tester use?** We say the Verse world key, so XP shows
   over their head. A tester who already has a trainer key needs the
   leveling spec's two-sided key link, which is phase 2.
2. **Should playtest XP ever count toward the trainer level?** We say no.
   Revisit if players read the two numbers as one anyway.
3. **Does App Store Connect's TestFlight feedback reach a tool we can
   run?** Apple exposes screenshot and crash feedback in App Store Connect;
   check the current App Store Connect API before building the triage tool
   on it rather than on stage 1 reports.
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
- [OpenAgents for iOS](../../bins/openagents-ios/README.md)
- [Verse in the OpenAgents app](../verse/mobile.md), the
  [Gym building](../verse/gym.md), the
  [Gym leaderboard](../verse/gym-leaderboard.md), and
  [Lagrange 1](../verse/lagrange-1.md)
- [Verse game design document](../verse/gdd.md)
- [Games, MMORPGs, and 3D worlds in OpenAgents](README.md)
- [`INVARIANTS.md`](../../INVARIANTS.md), Phone wallet
