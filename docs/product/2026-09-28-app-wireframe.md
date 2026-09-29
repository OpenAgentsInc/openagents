# OpenAgents phone app: wireframe specification

Written 2026-09-28. Status: design, revision 2. This page specifies every
screen of the OpenAgents phone app as a text wireframe, with the user flow,
for one closed loop. Each element is marked **EXISTS**, **PARTIAL**, or
**NEW** against the code on `main` today, so the gap is visible. We start
with the most basic elements and add to this wireframe later.

Revision 2 (2026-09-28, later the same day) makes **Chat** a first-class
part of the loop and the main menu. The first revision cut Chat because it
needed a computer; that stopped being true. Chat with OpenAgents now needs
no setup: every new chat goes to OpenAgents in the cloud, which speaks as
"we", answers common questions at once from a reviewed bank, and dispatches
Coder to a connected computer only when a message needs one
([what changed](#revision-2-what-changed)).

> **IDIOT PROOF.** Every screen must be usable by someone who has never
> heard of agents, Nostr, Bitcoin, benchmarks, or plugins. A first-time
> playtester completes the whole loop with zero explanation. Each screen has
> one obvious primary action, plain words only, a one-line answer to "what
> do I do next?", and no dead ends. No setup stands between a new player and
> their first win. This is the first principle of this spec and it overrides
> every other consideration here. The [IDIOT PROOF checklist](#idiot-proof-checklist)
> is run against every screen.

## Contents

- [Revision 2: what changed](#revision-2-what-changed)
- [Spec ID index](#spec-id-index)
- [Purpose and the loop](#purpose-and-the-loop)
- [Chat in the loop](#chat-in-the-loop)
- [Words on screen](#words-on-screen)
- [Status key and visual style](#status-key-and-visual-style)
- [Screen inventory](#screen-inventory)
- [Screens](#screens)
- [Chat screens](#chat-screens)
- [Intro cinematic](#cin-01-intro-cinematic)
- [User flows](#user-flows)
- [Navigation map](#nav-01-navigation-map)
- [IDIOT PROOF checklist](#idiot-proof-checklist)
- [Minimal v1 cut and later additions](#minimal-v1-cut-and-later-additions)
- [Appendix: what we cut and why](#appendix-what-we-cut-and-why)
- [Sources](#sources)

## Revision 2: what changed

What is on `main` now that the first revision didn't account for (verified
in code and history on 2026-09-28):

| Change | Commits |
| --- | --- |
| The phone's first tab is **Chat** (the message icon). It opens on a new chat, ready to type, with the composer **Message OpenAgents**. Chat needs no computer: messages go to the OpenAgents chat worker (NIP-CJ, Gemini 3.8 Flash through the AI Gateway, a Jev judgment first), which speaks as "we". | `6038a5ea48`, `820bc02ce4` |
| Every new chat goes to OpenAgents, even with a computer ready. It runs on a computer only when the person picks one in the target selector (**Cloud** or a computer) or taps one of its workspaces. | `25f8cb58c9` |
| Suggestion chips above the composer (continue a recent chat, another workspace, **Connect a computer**); previous chats behind the menu button (☰); a tap outside any text field puts the keyboard away. | `ed07491924`, `af22278ca5` |
| The chat router (`chat-router-v1`): Jev picks a route (`meta`, `smalltalk`, `general`, `product.kb`, `codebase.kb`, `work.dispatch`, `cli`, `wallet`, `account`, `clarify`, `end`, `refuse`), prepared answers come from a reviewed bank (`crates/coder/answers/chat-answers-v1.toml`), and product questions are answered from 52 sourced notes. | `81cee946af`, `dc8f9e2001`, `ea1946beda` |
| The phone shows the router's offers as its own controls: **Run Coder on** a computer or **Connect a computer**, screen chips (Open Wallet, Your computers, Identity keys, Playtest, Report a problem), read-only command cards with **Run**, follow-up chips, a quiet "Prepared answer" note, **Wrong answer**, and an opt-in **Share this chat** on Report a problem. | `8968ce8f8c`, `6c12b9e0e1` |
| Coder, dispatched to a computer, streams its reply into the chat; a session it delegated to OpenCode or Devin shows inside that Coder chat. Only OpenAgents and Coder chats are listed: the Claude Code, Codex, OpenCode, and Devin lists are gone. | `358975bdbd`, `951da41b10` |
| The Wallet's "whole numbers" notice is gone; the Grid's portal to Lagrange 1 is hidden; playtest logging is on for everyone with no switch. | `9240822d24`, `13997c305b`, `9f7ba429be` |
| TestFlight builds 18 ("Chat with OpenAgents") and 19 ("New chats go to OpenAgents") shipped. Build 20 carries the chat router's offers. | `e1d499def7`, `99e3094bff` |

What revision 2 changes in this spec:

- **Chat is in the loop and on the menu.** `SCR-01.E12` **CHAT WITH
  OPENAGENTS** is the second row on the main menu, under the one primary
  action. [Chat in the loop](#chat-in-the-loop) defines what chat does at
  each loop step (`CHAT-1` to `CHAT-8`).
- **Chat screens are specified**: `SCR-15` to `SCR-19`, with a new element
  `SCR-13.E05` (**Share this chat**).
- **New ways into chat** from the loop's screens: `SCR-02.E06`,
  `SCR-04.E09`, `SCR-05.E11`, `SCR-09.E06`.
- **New flows** `FLOW-03` to `FLOW-06`, and a new check `CHK-11`.
- **Stale claims removed**: "Chat needs a computer first", the Chat rows in
  the cut appendix, the playtest logging switch, and the "today's app fails"
  list, which is rewritten against what ships now.

ID changes: none. No existing ID was renumbered or retired; every change
above adds a new ID, so issues and commits that cite revision 1's IDs stay
correct.

## Spec ID index

Every screen, sequence, flow, check, and element has a stable ID. Use the ID
in issues and commits, for example `SCR-01.E03` for the **ENTER THE GYM**
button. IDs are stable, not positional: a new element gets the next free
number, and a removed element's ID is retired, never reused.

| Prefix | Meaning | Example |
| --- | --- | --- |
| `LOOP-n` | A step of the loop | `LOOP-3` Train |
| `CHAT-n` | A job chat does in the loop | `CHAT-2` "What should I try next?" |
| `SCR-nn` | A screen, sheet, or overlay | `SCR-01` Main menu |
| `SCR-nn.Enn` | An element on a screen | `SCR-01.E03` ENTER THE GYM button |
| `CIN-nn` | A cinematic sequence | `CIN-01` Intro cinematic |
| `CIN-nn.Snn` | A shot in a cinematic | `CIN-01.S07` Settle behind Coder |
| `FLOW-nn` | An end-to-end user flow | `FLOW-01` First-time playtester |
| `NAV-nn` | A navigation map | `NAV-01` Navigation map |
| `PAT-nn` | A shared pattern used by many screens | `PAT-01` Offline, error, and empty states |
| `CHK-nn` | An IDIOT PROOF checklist item | `CHK-01` One primary action |

| ID | Name | v1 |
| --- | --- | --- |
| `LOOP-1` … `LOOP-6` | Choose, Gym, Train, Measure, Share and level up, Return | Yes |
| `CHAT-1` … `CHAT-8` | Chat's jobs in the loop | Yes (`CHAT-1`, `CHAT-7`, `CHAT-8`); the rest later |
| `SCR-01` | Main menu | Yes |
| `SCR-02` | Choose your agent (first run, step 1 of 3) | Yes |
| `CIN-01` | Intro cinematic | Yes (short cut) |
| `SCR-03` | The Gym: pick a tool (first run, step 2 of 3) | Yes |
| `SCR-04` | Training (first run, step 3 of 3) | Yes |
| `SCR-05` | Result | Yes |
| `SCR-06` | Level up (overlay) | Yes |
| `SCR-07` | Coder: your agent | Later |
| `SCR-08` | All tools | Later |
| `SCR-09` | Tool detail | Later |
| `SCR-10` | Rankings | Later |
| `SCR-11` | Profile | Yes (minimal) |
| `SCR-12` | Updates | Later |
| `SCR-13` | Report a problem (sheet) | Yes |
| `SCR-14` | Just-in-time setup prompt (sheet) | Later |
| `SCR-15` | Chat: new chat, ready to type | Yes |
| `SCR-16` | Chat: previous chats | Yes |
| `SCR-17` | Chat: a conversation with OpenAgents | Yes |
| `SCR-18` | Chat: Wrong answer (inline) | Yes |
| `SCR-19` | Chat: Coder on a computer | Yes (as it exists) |
| `PAT-01` | Offline, error, and empty states | Yes |
| `FLOW-01` | First-time playtester | Yes |
| `FLOW-02` | Returning playtester, daily loop | Yes |
| `FLOW-03` | Kicking the tires in chat | Yes |
| `FLOW-04` | From chat to the Gym | Later |
| `FLOW-05` | Dispatching Coder from chat | Yes |
| `FLOW-06` | Wrong answer and Share this chat | Yes |
| `NAV-01` | Navigation map | Yes |
| `CHK-01` … `CHK-11` | IDIOT PROOF checklist | Yes |

## Purpose and the loop

We are building the best coding agent in the world by using network
effects: an agent collective. Coder is our first agent. The Verse is where
agents go to connect, communicate, and transact, and where people stay in the
loop while agents are built. The Gym is where people go to help agents
become better through our plugin system.

The near-term goal of this app is a simple experience that attracts
playtesters, shows them a measurable improvement in an agent's score, grows
the playtest cooperative, keeps playtesters coming back, and builds a
following. Every element on every screen serves one loop. Anything that
doesn't is cut ([appendix](#appendix-what-we-cut-and-why)).

```
            +--------------------------------------------+
            |                                            |
            v                                            |
   LOOP-1 CHOOSE ----> LOOP-2 GYM ----> LOOP-3 TRAIN     |
   your agent          pick a tool      Coder practices  |
   (Coder)             (default set)    with the tool    |
                                             |           |
                                             v           |
   LOOP-6 RETURN <---- LOOP-5 SHARE <---- LOOP-4 MEASURE |
   tomorrow's run,     add result to      score before   |
   a check waiting,    the collective,    -> after       |
   rank to defend      XP, level, rank                   |
            |                                            |
            +--------------------------------------------+

 Network effect: a result you add is checked by other trainers. When a
 tool is confirmed to help, Coder uses it for everyone, and everyone's
 Coder gets a higher score.

 CHAT WITH OPENAGENTS sits beside every step: ask what anything means,
 what to try next, or how your run did, and get the next step as a tap.
```

| Step | What the player does | What we do |
| --- | --- | --- |
| `LOOP-1` Choose | Chooses an agent. In v1 that is Coder, chosen for them. | Creates the player's identity silently. |
| `LOOP-2` Gym | Picks a tool to give Coder. A recommended tool is preselected. | Offers one default and a short list. |
| `LOOP-3` Train | Taps **Start training**. | Runs Coder on 10 practice tasks with the tool, on our computers. |
| `LOOP-4` Measure | Sees Coder's score before and after. | Compares against Coder's score today on the same 10 tasks, with repeats. |
| `LOOP-5` Share and level up | Adds the result to the Gym, gains XP, and levels up. | Publishes the result; other trainers check it; a confirmed tool ships to everyone. |
| `LOOP-6` Return | Comes back for today's run, a check that's waiting, or their rank. | Surfaces one reason to return on the main menu. |

## Chat in the loop

Chat with OpenAgents is the loop's companion, not a detour from it. It
needs no setup: no computer, sign-in, or wallet. A first-timer can kick the
tires before their first run, and every returning player can ask what to do
next. Chat serves the loop in three ways: it **answers** (a prepared answer
at once, or the model's reply streamed), it **offers** the next step as a
tap (open a screen, start something), and it **dispatches** Coder to a
connected computer for real work. Chat never acts on its own: every offer
does nothing until the player taps it, and the tap's meaning is the app's,
never the model's words.

| ID | The player asks | Chat answers with | Loop | Status |
| --- | --- | --- | --- | --- |
| `CHAT-1` | "Who are you?", "What model is this?", "What does it cost?", "What can you do?" (kicking the tires) | A prepared answer at once (under a second), marked "Prepared answer", with follow-up chips to the next likely question. | `LOOP-1` | EXISTS (`meta.*` and `smalltalk.*` in `chat-answers-v1`) |
| `CHAT-2` | "What should I try next?" | One recommendation in a sentence ("Give Coder Project map. 18 trainers tried it.") and an offer **Go to the Gym** that opens `SCR-03` with that tool preselected. | `LOOP-2` | NEW (no Gym screen offer in the router; `open_screen` names Wallet, Computers, Identity keys, Playtest, Report a problem only) |
| `CHAT-3` | "What does Project map do?", "What's a tool?" | A plain answer from the product notes, and **Train with this tool** (opens `SCR-03` with it selected). | `LOOP-2` | PARTIAL (`product.kb` answers from 52 sourced notes, including the Gym's RESULTS board; notes for each tool and the offer are NEW) |
| `CHAT-4` | "Train Coder with Code finder" | "We'll start a run with Code finder. It takes about 5 minutes." and **Start training** (opens `SCR-03` with the tool selected; the run starts only on that screen's button). | `LOOP-3` | NEW (needs the hosted runner) |
| `CHAT-5` | "How did my run do?", "What's Coder's score?" | The before and after in one line ("6 of 10 → 8 of 10 with Project map. Better.") and **See the result** (opens `SCR-05`). | `LOOP-4` | NEW (needs player results) |
| `CHAT-6` | "How do I earn XP?", "What level am I?" | A prepared or product answer; "Level 2, 93 XP to level 3" from the player's card, and **Enter the Gym**. | `LOOP-5` | PARTIAL (`openagents.earn-xp` and `openagents.playtest-xp` notes exist; reading the player's own level in chat is NEW) |
| `CHAT-7` | "Fix the failing test in my repo", "Look through this project" | "We'll dispatch Coder to …" and **Run Coder on** the computer, or **Connect a computer** when none is added. Coder's reply streams into its own chat (`SCR-19`). | Beyond the loop (your own code) | EXISTS (`work.dispatch`; `SCR-17.E05`) |
| `CHAT-8` | "Where's my wallet?", "How do I report a bug?", "Which computers are online?" | A short answer and a screen chip (**Open Wallet**, **Your computers**, **Identity keys**, **Playtest**, **Report a problem**) or a read-only command card. | Support | EXISTS on the phone (screen chips; command cards for `computer list`, `show`, `workspaces`); the worker's command proposals are PARTIAL (the CLI route landed in `coder::cli_route`, `528483364e`, but isn't wired into the chat worker yet) |

Rules chat keeps in the loop:

1. **Chat never starts a Gym run, spends, or publishes by itself.** It opens
   the screen where that happens, with the choice filled in, and the
   player taps that screen's one primary button.
2. **Chat is never in front of the first run.** The guided first run
   (`FLOW-01`) offers chat as a gray secondary link, and coming back from
   chat returns to the same step.
3. **Chat speaks as "we".** It is OpenAgents answering; Coder is the agent
   we dispatch. Refusals and waits speak in the same voice.
4. **Chat answers the question asked.** A prepared answer shows only when
   the router is sure; otherwise the model answers. A wrong prepared answer
   is one tap to report (`SCR-18`).

## Words on screen

Primary surfaces use plain words only. The internal term stays in code,
docs, and the Advanced section of Profile.

| On screen | Internal term | Why |
| --- | --- | --- |
| **Coder** ("an AI that writes code") | Coder, the agent | A name plus one plain line. |
| **Tool** | Plugin (a Wasm guest in `crates/plugin`) | Everyone knows what a tool is. "Plugin" is jargon. |
| **Project map**, **Code finder**, **Test reader** | `repo_map`, `code_search`, `test_report` evidence guests | Says what the tool does. |
| **Practice tasks** | A fixed subset of a Terminal-Bench 2.1 dev set | "Benchmark" is jargon. |
| **Coder's score: 6 of 10** | Passes out of the practice set | Whole numbers, whole denominator. |
| **Better**, **No clear change**, **Worse** | Keep, inconclusive, reject against the spread between repeats | Plain verdict. |
| **Check a result** | Reproduce a published pass (NIP-XP `reproduce`) | Says what you do. |
| **Add my result to the Gym** | Publish a signed result and claim (NIP-EVAL, NIP-XP) | Says where it goes. |
| **Trainer 7KQ** | The Verse world key's public key | A name, not a key. |
| **XP**, **Level 2** | NIP-XP awards on `trainer-curve-v1` | Game words most people know. |
| **Save your progress** | Back up the secret key | Says why, not how. |
| **OpenAgents** ("we") | The chat worker answering a NIP-CJ job | The app and the chat are one voice. |
| **Chat with OpenAgents**, **Message OpenAgents** | The Chat tab, the basic chat | Says who you're talking to. |
| **Run Coder on Studio Mac** | NIP-HOST `task.create` with the conversation | Names the agent and the computer. |
| **Connect a computer** | Enroll a host (Account > Computers) | Says what you do. |
| **Cloud** | No computer: the chat worker | The one place a message goes with no setup. |
| **Prepared answer** | A T0 bank answer (`bank:chat-answers-v1`) | Honest about where the words came from. |
| **Wrong answer** | A wrong-answer playtest report | Says what you're telling us. |

Banned on primary surfaces: npub, nsec, key, relay, Nostr, NIP, ATIF,
tailnet, Tailscale, Wasm, plugin, benchmark, Terminal-Bench, TB, Jev, Luna,
Microcoder, verifier, trace, recipe, grant, sats, BTC, ₿, Lightning,
invoice, host, workspace, pubkey, hex.

One exception, owner-approved on 2026-09-28: a chat **answer** may name
Gemini, the AI Gateway, Jev, or Nostr when the player asks what powers the
chat or how it works. The labels, buttons, chips, and notes around the
answer stay plain. "Workspace" appears only on a computer-backed chat, which
a player reaches only after connecting a computer.

## Status key and visual style

| Mark | Meaning |
| --- | --- |
| **EXISTS** | Built on `main` and reachable on the phone today, in some form. |
| **PARTIAL** | The data or logic exists (often desktop, CLI, or host only), or a similar screen exists that needs changes. |
| **NEW** | Not built. |

Visual style follows the owner's main-menu mockup: black background, white
and gray monochrome, a white power-symbol logo with the **OPENAGENTS**
wordmark, big stacked row buttons with an icon, a bold condensed uppercase
title, a one-line subtitle, and a chevron. The primary action on each screen
is the one white-filled row or button; everything else is outlined or gray.
Text is at least 17 pt; buttons are at least 56 pt tall and full width.

## Screen inventory

| ID | Screen | Serves | Reached from | Primary action |
| --- | --- | --- | --- | --- |
| `SCR-01` | Main menu | All steps | App open (returning), end of every flow | **ENTER THE GYM** |
| `SCR-02` | Choose your agent | `LOOP-1` | App open (first time) | **CHOOSE CODER** |
| `CIN-01` | Intro cinematic | `LOOP-1` to `LOOP-2` | `SCR-02` | **GO TO THE GYM** (at the end) |
| `SCR-03` | The Gym: pick a tool | `LOOP-2` | `CIN-01`, `SCR-01` | **START TRAINING** |
| `SCR-04` | Training | `LOOP-3` | `SCR-03` | **SEE THE RESULT** (when done) |
| `SCR-05` | Result | `LOOP-4`, `LOOP-5` | `SCR-04` | **ADD MY RESULT TO THE GYM** |
| `SCR-06` | Level up | `LOOP-5` | `SCR-05` | **NICE** (back to the main menu) |
| `SCR-07` | Coder: your agent | `LOOP-4` | `SCR-01` | **TRAIN CODER** |
| `SCR-08` | All tools | `LOOP-2` | `SCR-03`, `SCR-07` | Tap a tool |
| `SCR-09` | Tool detail | `LOOP-2` | `SCR-08` | **TRAIN WITH THIS TOOL** |
| `SCR-10` | Rankings | `LOOP-5`, `LOOP-6` | `SCR-01`, `SCR-05` | **CLIMB: ENTER THE GYM** |
| `SCR-11` | Profile | `LOOP-5` | `SCR-01` | **ENTER THE GYM** |
| `SCR-12` | Updates | `LOOP-6` | `SCR-01` bell | The top update's button |
| `SCR-13` | Report a problem | Playtest | `SCR-11`, long press anywhere | **SEND** |
| `SCR-14` | Just-in-time setup prompt | `LOOP-5`, `LOOP-6` | Triggered | The one setup step |
| `SCR-15` | Chat: new chat | `CHAT-1` to `CHAT-8` | `SCR-01.E12`, `SCR-02.E06`, `SCR-04.E09`, `SCR-05.E11`, `SCR-16` | Send a message (the composer) |
| `SCR-16` | Chat: previous chats | All chat | `SCR-15` ☰, `SCR-17` ☰ | Tap a chat |
| `SCR-17` | Chat: a conversation | `CHAT-1` to `CHAT-8` | `SCR-15` (after Send), `SCR-16` | The offer under the reply, else the composer |
| `SCR-18` | Chat: Wrong answer | `CHAT-1` (quality) | `SCR-17.E09` | **Send** |
| `SCR-19` | Chat: Coder on a computer | `CHAT-7` | `SCR-17.E05`, `SCR-16`, `SCR-15` with a computer chosen | Send a follow-up |
| `PAT-01` | Offline, error, and empty states | All | Any screen | **TRY AGAIN** or the named next step |

## Screens

Each screen has a wireframe (top to bottom, exact labels), its states, an
element table, transitions, and an IDIOT PROOF check line. The wireframe
shows the primary action as `[#### LABEL ####]` and secondary actions as
`[ label ]`. Examples such as `38` or `7KQ` are sample data.

### SCR-01 Main menu

Serves every loop step. It is the hub the player returns to.

```
+------------------------------------------+
| (O) OPENAGENTS                  [bell 2] |  E01, E02
|------------------------------------------|
| [avatar] Trainer 7KQ                     |  E04
|          Level 2  [######----] 140/283 XP|
|------------------------------------------|
|                                          |
|   [ hero: Coder, a matte-black agent     |  E05
|     with a glowing power emblem, looks   |
|     out over the white wireframe Grid    |
|     and the Gym building ]               |
|   ( GYM OPEN · 38 people training now )  |
|                                          |
|------------------------------------------|
| Next: give Coder a new tool.             |  E11
|##########################################|
|# [dumbbell] ENTER THE GYM             > #|  E03 (primary)
|#   Make Coder better · 3 runs left today #|
|##########################################|
| [message] CHAT WITH OPENAGENTS         > |  E12
|   Ask us anything. No setup needed.      |
| [spade] CODER                          > |  E06 (later)
|   Score 6 of 10 · up 2 this week         |
| [podium] RANKINGS                      > |  E07 (later)
|   You're #41 this week                   |
| [person] PROFILE                       > |  E08
|   Level, XP, help                        |
|------------------------------------------|
| PLAYTEST SEASON 1 · ENDS OCT 26          |  E09
| Help make Coder better. Every run counts.|
|------------------------------------------|
| ● Gym open            v1.0.0 · Playtest  |  E10
+------------------------------------------+
```

States:

- **Loading:** the layout draws at once from the phone's cache; numbers that
  are still loading show a gray bar, never `0`.
- **Empty (no runs yet):** cannot happen for a first-time player, because
  `FLOW-01` ends here only after the first run. If the first run was never
  finished, `SCR-01` is not shown; the app reopens the guided step instead.
- **A check is waiting:** `E11` reads "Next: check another trainer's result
  (+50 XP)." and `E03`'s subtitle reads "A check is waiting for you".
- **No runs left today:** `E03` stays primary and reads "ENTER THE GYM ·
  Check results while you wait"; `E11` reads "New runs at 9:00 tomorrow.
  You can still check results now."
- **Offline or error:** see `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-01.E01` | Logo and **OPENAGENTS** wordmark | Identity; no action. | — | PARTIAL (the Chat tab header says "OpenAgents") |
| `SCR-01.E02` | Updates bell with a count | Opens `SCR-12`. The count is unread updates, such as "Your result was confirmed." | `LOOP-6` | NEW |
| `SCR-01.E03` | **ENTER THE GYM** button | Opens `SCR-03`. The only primary action. Subtitle shows runs left today. | `LOOP-2` | NEW |
| `SCR-01.E04` | Player card: avatar, trainer name, level, XP bar | Shows progress at a glance. Tapping opens `SCR-11`. | `LOOP-5` | PARTIAL (level and XP exist in Account > Trainer; the name and the card on this screen are NEW) |
| `SCR-01.E05` | Hero image with Gym status pill | Shows the Gym is open and how many people are training now. | `LOOP-5` (network) | PARTIAL (the Grid and its presence exist; a "training now" count is NEW) |
| `SCR-01.E06` | **CODER** row | Opens `SCR-07` with Coder's score. | `LOOP-4` | NEW (later) |
| `SCR-01.E07` | **RANKINGS** row | Opens `SCR-10`. | `LOOP-5`, `LOOP-6` | NEW (later) |
| `SCR-01.E08` | **PROFILE** row | Opens `SCR-11`. | `LOOP-5` | PARTIAL (Account tab exists) |
| `SCR-01.E09` | Season card | States the season and its end date. Tapping opens `SCR-10` (or `SCR-11` in v1). | `LOOP-6` | PARTIAL (the season exists in the playtesting program; no card) |
| `SCR-01.E10` | Footer: Gym status, version | Status and build for reports. | — | PARTIAL (version exists in About this device) |
| `SCR-01.E11` | Next-step line | One line that says what to do next. | All | NEW |
| `SCR-01.E12` | **CHAT WITH OPENAGENTS** row | Opens `SCR-15`, a new chat ready to type. Subtitle: "Ask us anything. No setup needed." The owner's mockup row **CHAT WITH AN AGENT**, in our voice. Outlined, never the filled primary. With an unfinished reply or a Coder chat that needs an answer, the subtitle reads "Coder asked you a question" and opens that chat. | `CHAT-1` to `CHAT-8` | PARTIAL (the chat is the first tab today and opens ready to type; the row and the hub are NEW) |

Transitions: in from app open (returning player), `SCR-06`, `SCR-05`, and
every **Back to menu**. Out to `SCR-03`, `SCR-15`, `SCR-07`, `SCR-10`,
`SCR-11`, `SCR-12`.

IDIOT PROOF check: **pass.** One filled button, the next step in one line,
no jargon (no key, balance, or "sats"), progress visible on the card. Chat
is the second row: always there, never competing with the primary.

### SCR-02 Choose your agent

First run, step 1 of 3. Shown once, on the first app open.

```
+------------------------------------------+
| STEP 1 OF 3                    ● ○ ○     |  E01
|------------------------------------------|
| Choose your agent                        |  E02
| Your agent is an AI that writes code.    |
| You'll train it to get better.           |
|                                          |
| +--------------------------------------+ |
| | [Coder figure, glowing emblem]       | |  E03
| | CODER                     (selected) | |
| | Writes and fixes code.               | |
| | Score today: 6 of 10 practice tasks  | |
| +--------------------------------------+ |
|                                          |
| Next: choose Coder to begin.             |  E05
|##########################################|
|#           CHOOSE CODER                 #|  E04 (primary)
|##########################################|
| [ Ask OpenAgents a question first ]      |  E06 (gray)
+------------------------------------------+
```

States:

- **Loading:** the card shows the figure at once; the score line shows a gray
  bar until it loads. **CHOOSE CODER** works before the score loads.
- **Back from chat:** the screen is as it was; step 1 of 3 is still the
  step.
- **Offline:** the button still works. Choosing is local; `CIN-01` plays from
  the app bundle. The offline notice appears at `SCR-03`, where it matters.
- **Error:** none possible on this screen; identity is created on the phone.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-02.E01` | Step indicator | Shows step 1 of 3. | `LOOP-1` | NEW |
| `SCR-02.E02` | Title and two plain lines | Explains an agent in one sentence. | `LOOP-1` | NEW |
| `SCR-02.E03` | Coder card, preselected | The default and only agent in v1. Shows today's score so the player sees what they will improve. | `LOOP-1`, `LOOP-4` | NEW (Coder exists; the card and a public score are NEW) |
| `SCR-02.E04` | **CHOOSE CODER** | Creates the player's identity silently (a world key, no prompt) and plays `CIN-01`. Tap 1 of 3 to the first run. | `LOOP-1` | PARTIAL (the device and world keys are created silently today) |
| `SCR-02.E05` | Next-step line | "Next: choose Coder to begin." | `LOOP-1` | NEW |
| `SCR-02.E06` | **Ask OpenAgents a question first** | Gray link for the player who wants to kick the tires. Opens `SCR-15` with first-time suggestion chips ("Who are you?", "What is Coder?", "What's the Gym?"); its **< Back** returns here. | `CHAT-1` | NEW (the chat exists; the link and the guided step are NEW) |

Transitions: in from the first app open only. Out to `CIN-01`, or to
`SCR-15` and back. There is no back button; there is nothing before this
step.

IDIOT PROOF check: **pass.** One agent, already selected, one button. No
sign-in, name, key, or wallet. The chat link is gray and returns to the same
step.

### SCR-03 The Gym: pick a tool

First run, step 2 of 3. Later, the Gym's front door from `SCR-01.E03`.

```
+------------------------------------------+
| [< Menu]   THE GYM             ● ● ○     |  E01, E02
|------------------------------------------|
| Give Coder a new tool.                   |  E03
| We'll test it on 10 practice tasks and   |
| show you if Coder got better.            |
|                                          |
| RECOMMENDED                              |
| +======================================+ |
| | (•) [map] PROJECT MAP                | |  E04 (preselected)
| |     Shows Coder how the project is   | |
| |     laid out before it starts.       | |
| |     Tried by 18 trainers             | |
| +======================================+ |
| ( ) [search] CODE FINDER               | |  E05
| |     Finds the right lines of code.   | |
| ( ) [check] TEST READER                | |  E05
| |     Reads test failures for Coder.   | |
|                                          |
| [ See all tools ]  (later)               |  E06
|                                          |
| Takes about 5 minutes. Free.             |  E07
| Next: tap Start training.                |  E08
|##########################################|
|#          START TRAINING                #|  E09 (primary)
|##########################################|
| 3 runs left today                        |  E10
+------------------------------------------+
```

In the returning player's Gym, when a check is waiting, the recommended card
is a check instead, and the primary button reads **START THE CHECK**:

```
| RECOMMENDED                              |
| +======================================+ |
| | (•) [check] CHECK A RESULT  +50 XP   | |  E11
| |     Trainer 2PX says Code finder     | |
| |     made Coder better. Run it again  | |
| |     to confirm.                      | |
| +======================================+ |
```

States:

- **First run:** the back control is hidden; the step indicator shows 2 of 3.
  The player can't leave the guided path into a broken state.
- **Loading:** the three tool cards are bundled with the app and show at
  once. The "Tried by" counts show a gray bar until they load.
- **No runs left today:** **START TRAINING** is replaced by **START THE
  CHECK** if a check is waiting; otherwise the primary reads **BACK TO MENU**
  with "New runs at 9:00 tomorrow." A check never uses a daily run.
- **A run is already going:** the screen opens `SCR-04` instead.
- **Offline or error:** `PAT-01`, with "You're offline. Training needs the
  internet." and **TRY AGAIN**.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-03.E01` | **< Menu** | Back to `SCR-01`. Hidden on the first run. | — | NEW |
| `SCR-03.E02` | Title and step indicator | "THE GYM"; dots show step 2 of 3 on the first run. | `LOOP-2` | NEW |
| `SCR-03.E03` | Two-line explanation | Says what training does and what they will see. | `LOOP-2` | NEW |
| `SCR-03.E04` | Recommended tool, preselected | The default tool. v1 default: **Project map**. | `LOOP-2` | PARTIAL (the `repo_map` guest exists host side, off by default and unmeasured; a phone catalog is NEW) |
| `SCR-03.E05` | Two other tools | Alternates, one tap to select. | `LOOP-2` | PARTIAL (`code_search`, `test_report` guests exist host side) |
| `SCR-03.E06` | **See all tools** | Opens `SCR-08`. Hidden in v1. | `LOOP-2` | NEW (later) |
| `SCR-03.E07` | Time and cost line | "Takes about 5 minutes. Free." We pay for the run. | `LOOP-3` | NEW |
| `SCR-03.E08` | Next-step line | "Next: tap Start training." | `LOOP-2` | NEW |
| `SCR-03.E09` | **START TRAINING** | Starts a hosted run of Coder with the selected tool on the practice tasks. Tap 3 of 3 on the first run; tap 2 for a returning player. | `LOOP-3` | NEW (the Gym can start a reviewed recipe today, but only on a host paired with a `gym-connect:` code; a hosted runner with no setup is NEW) |
| `SCR-03.E10` | Runs left today | Daily limit, so progress and cost stay bounded. | `LOOP-6` | NEW |
| `SCR-03.E11` | Check a result card | Rerun another trainer's result to confirm it. | `LOOP-5` (network) | PARTIAL (the NIP-XP `reproduce` rule exists; doing it is desktop only) |

Transitions: in from `CIN-01` (first run) or `SCR-01.E03`. Out to `SCR-04`
(start), `SCR-08` (later), `SCR-01` (back).

IDIOT PROOF check: **pass.** A tool is already chosen, so the primary
button works on arrival. Time and cost are stated. No computer or wallet.

### SCR-04 Training

First run, step 3 of 3. Coder works on the practice tasks.

```
+------------------------------------------+
| [< Menu]   TRAINING            ● ● ●     |  E01
|------------------------------------------|
| Coder is practicing with Project map.    |  E02
|                                          |
|   [ Coder figure at a workbench; each    |  E03
|     finished task lights one of 10       |
|     blocks: ■ ■ ■ ■ □ □ □ □ □ □ ]        |
|                                          |
| Practice task 4 of 10                    |  E04
| Solved so far: 3                         |
| Coder's score before: 6 of 10            |  E05
| About 3 minutes left                     |
|                                          |
| You can leave. We'll keep going and      |  E06
| show you the result here.                |
|                                          |
| Next: wait for the result, or come back. |  E07
|[ ####  SEE THE RESULT (when done) #### ] |  E08 (primary, disabled until done)
| [ Ask OpenAgents while you wait ]        |  E09
+------------------------------------------+
```

States:

- **Running:** as drawn. **SEE THE RESULT** is visible but gray, with "Ready
  in about 3 minutes" on it.
- **Done:** the button turns white and reads **SEE THE RESULT**; the phone
  vibrates once if the screen is open.
- **Left and came back:** the app reopens here (first run) or shows a chip on
  `SCR-01` reading "Training · 4 of 10" that opens this screen.
- **Slow:** after twice the estimate, E06 reads "Taking longer than usual.
  We'll keep going." No action needed.
- **Failed (our side):** `PAT-01`: "Something went wrong on our side. This
  run didn't count against today's runs." and **TRY AGAIN**, which restarts
  the same run.
- **Offline:** the run continues on our computers; the screen shows "You're
  offline. Training continues. We'll show the result when you're back."

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-04.E01` | Title, step indicator, **< Menu** | Step 3 of 3. **< Menu** leaves; the run continues. | `LOOP-3` | NEW |
| `SCR-04.E02` | Status line | Names the agent and tool. | `LOOP-3` | NEW |
| `SCR-04.E03` | Progress picture, 10 blocks | Shows progress without numbers to read. | `LOOP-3` | NEW |
| `SCR-04.E04` | Task counter and solved count | Progress with the whole denominator. | `LOOP-3` | PARTIAL (the live Gym board reports `completed/total` per run) |
| `SCR-04.E05` | Score before and time left | Sets up the comparison. Unknown time says "Working" instead of a guess. | `LOOP-4` | NEW |
| `SCR-04.E06` | Reassurance line | Says it's safe to leave. | `LOOP-3` | NEW |
| `SCR-04.E07` | Next-step line | One line. | `LOOP-3` | NEW |
| `SCR-04.E08` | **SEE THE RESULT** | Opens `SCR-05` when done. | `LOOP-4` | NEW |
| `SCR-04.E09` | **Ask OpenAgents while you wait** | Opens `SCR-15` with chips about the run ("What is Coder doing?", "What does Project map do?"). The run continues; when it's done, a chip at the top of the chat reads **Your result is ready** and opens `SCR-05`. | `CHAT-3`, `CHAT-5` | NEW |

Transitions: in from `SCR-03`. Out to `SCR-05` (done), `SCR-15` (ask while
waiting), or `SCR-01` (leave).

IDIOT PROOF check: **pass.** The one button is visible from the start and
says when it will be ready. Leaving is safe and says so. Errors cost the
player nothing.

### SCR-05 Result

The payoff: the score before and after, the XP, and one action that adds the
result to the collective.

```
+------------------------------------------+
| [< Menu]   YOUR RESULT                   |  E01
|------------------------------------------|
|           CODER GOT BETTER               |  E02
|                                          |
|        6 of 10   -->   8 of 10           |  E03
|        before          with Project map  |
|                                          |
|  +50 XP  (pending until another trainer  |  E04
|           checks it)                     |
|  [######----]  Level 2 · 190/283 XP      |
|                                          |
| Your result helps every trainer. When    |  E05
| others confirm it, Coder uses Project    |
| map for everyone.                        |
|                                          |
| Next: add your result to the Gym.        |  E06
|##########################################|
|#      ADD MY RESULT TO THE GYM          #|  E07 (primary)
|##########################################|
| Your result and trainer name are public. |  E08
| [ Share outside the app ]                |  E09
| [ Try another tool ]                     |  E10
| [ Ask about this result ]                |  E11
+------------------------------------------+
```

States:

- **Better:** as drawn. The headline is **CODER GOT BETTER**.
- **No clear change:** headline **NO CLEAR CHANGE**, numbers such as "6 of 10
  --> 6 of 10", and E05 reads "That's useful too. Now everyone knows this
  tool doesn't help here." The primary stays **ADD MY RESULT TO THE GYM**.
- **Worse:** headline **CODER DID WORSE WITH THIS TOOL**, same primary, and
  E05 reads "That's useful too. We won't give Coder this tool."
- **After adding:** the primary becomes a check mark row, "Added to the Gym",
  and the next primary is **BACK TO MENU**. If the XP crossed a level, `SCR-06`
  opens first.
- **Adding failed:** `PAT-01`: "We couldn't add your result. It's saved on
  this phone." and **TRY AGAIN**.
- **A check result:** headline **YOU CONFIRMED IT** or **IT DIDN'T HOLD UP**,
  "+50 XP", and the same primary.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-05.E01` | Title and **< Menu** | Leaves; the result is kept on the phone. | — | NEW |
| `SCR-05.E02` | Verdict headline | **Better**, **No clear change**, or **Worse**, decided by the change against the spread between repeats. | `LOOP-4` | PARTIAL (the keep rule is written in the plugin measurement plan; never run) |
| `SCR-05.E03` | Score before and after | Whole numbers with whole denominators. | `LOOP-4` | NEW (the results board shows published passes, read only) |
| `SCR-05.E04` | XP gained and level bar | Shows the XP now, marked pending until confirmed. | `LOOP-5` | PARTIAL (NIP-XP awards and the level curve exist; a `gym-trial` rule is proposed only) |
| `SCR-05.E05` | Why it matters | One sentence on the network effect. | `LOOP-5` | NEW |
| `SCR-05.E06` | Next-step line | One line. | `LOOP-5` | NEW |
| `SCR-05.E07` | **ADD MY RESULT TO THE GYM** | Publishes the signed result and XP claim; queues it for checks by other trainers. | `LOOP-5` | PARTIAL (signed NIP-EVAL results publication exists for our boards; player publication is NEW) |
| `SCR-05.E08` | Public notice | One sentence at the moment it matters. | `LOOP-5` | NEW |
| `SCR-05.E09` | **Share outside the app** | Opens the system share sheet with an image card and a link. | `LOOP-5` (following) | PARTIAL (Export card exists for the trainer card) |
| `SCR-05.E10` | **Try another tool** | Opens `SCR-03`. | `LOOP-2` | NEW |
| `SCR-05.E11` | **Ask about this result** | Opens `SCR-17` with the result as context and a first question filled in ("Why did Coder do better with Project map?"). The answers suggest what to try next (`CHAT-2`). | `CHAT-2`, `CHAT-5` | NEW |

Transitions: in from `SCR-04`. Out to `SCR-06` (level up), `SCR-01`,
`SCR-03`, `SCR-17`, or the share sheet.

IDIOT PROOF check: **pass.** The before and after are the biggest thing on
the screen. Every verdict, including "worse", has the same next step. The
public notice is one sentence at the point it matters.

### SCR-06 Level up

An overlay over `SCR-05` when XP crosses a level.

```
+------------------------------------------+
|                                          |
|              LEVEL UP                    |  E01
|                 3                        |
|      [######################] 283 XP     |  E02
|                                          |
|   New: PLAYTESTER title on your name     |  E03
|                                          |
| Next: come back tomorrow for new runs.   |  E04
|##########################################|
|#               NICE                     #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

States: shown once per level. If a title isn't new, E03 is hidden. No error
state: it reads local numbers.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-06.E01` | Level number | The new level. | `LOOP-5` | PARTIAL (levels exist in Account > Trainer) |
| `SCR-06.E02` | Full XP bar | Progress made visible. | `LOOP-5` | PARTIAL |
| `SCR-06.E03` | New title or reward | Names the loot. | `LOOP-5` | PARTIAL (playtest titles exist) |
| `SCR-06.E04` | Next-step line | Gives the reason to return. | `LOOP-6` | NEW |
| `SCR-06.E05` | **NICE** | Closes and opens `SCR-01`. | `LOOP-6` | NEW |

Transitions: in from `SCR-05`. Out to `SCR-01`.

IDIOT PROOF check: **pass.** One button, no choices.

### SCR-07 Coder: your agent

Later. Shows the collective's agent and how it got here.

```
+------------------------------------------+
| [< Menu]   CODER                         |
|------------------------------------------|
| [Coder figure]                           |  E01
| Score today: 8 of 10 practice tasks      |  E02
| [chart: score by week, 6 → 7 → 8]        |  E03
|                                          |
| TOOLS CODER USES NOW                     |  E04
|  [map] Project map · confirmed by 6      |
|        trainers · added Oct 3            |
| BEING TESTED                             |  E05
|  [search] Code finder · 2 checks so far  |
|                                          |
| Next: train Coder to raise its score.    |  E06
|##########################################|
|#            TRAIN CODER                 #|  E07 (primary)
|##########################################|
+------------------------------------------+
```

States: loading shows gray bars; empty "Being tested" says "Nothing is being
tested. Be the first." Offline and error use `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-07.E01` | Coder figure | Identity. | — | NEW |
| `SCR-07.E02` | Score today | The collective's current score. | `LOOP-4` | NEW |
| `SCR-07.E03` | Score chart | Improvement over time. | `LOOP-4` | NEW |
| `SCR-07.E04` | Tools Coder uses now | Confirmed tools, with who confirmed them. | `LOOP-5` (network) | NEW |
| `SCR-07.E05` | Being tested | Tools with checks still open. Tapping one opens `SCR-09`. | `LOOP-2` | NEW |
| `SCR-07.E06` | Next-step line | One line. | — | NEW |
| `SCR-07.E07` | **TRAIN CODER** | Opens `SCR-03`. | `LOOP-2` | NEW |

Transitions: in from `SCR-01.E06`. Out to `SCR-03`, `SCR-09`, `SCR-01`.

IDIOT PROOF check: **pass.** Read-only screen with one action.

### SCR-08 All tools

Later. The full list of tools a player can give Coder.

```
+------------------------------------------+
| [< Gym]    ALL TOOLS                     |
|------------------------------------------|
| Pick a tool to see what it does.         |  E01
| [map]    PROJECT MAP        Helps ✓    > |  E02
| [search] CODE FINDER        Testing    > |
| [check]  TEST READER        Testing    > |
| [lock]   More tools soon                 |  E03
+------------------------------------------+
```

States: loading shows the bundled tools at once; statuses load after.
Offline shows the list with statuses marked "Can't check right now".

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-08.E01` | Instruction line | Says what to do. | `LOOP-2` | NEW |
| `SCR-08.E02` | Tool rows with a plain status | **Helps**, **Testing**, or **Doesn't help**. Tapping opens `SCR-09`. | `LOOP-2` | PARTIAL (install and catalog states are specified in `docs/extensions/packages.md` and NIP-EXT; not built) |
| `SCR-08.E03` | More tools soon | Honest placeholder. | — | NEW |

Transitions: in from `SCR-03.E06`. Out to `SCR-09`, `SCR-03`.

IDIOT PROOF check: **pass with a note.** A list has no filled primary; every
row is the same one action (tap to open). Allowed because the screen has one
kind of action.

### SCR-09 Tool detail

Later.

```
+------------------------------------------+
| [< Tools]  PROJECT MAP                   |
|------------------------------------------|
| Shows Coder how the project is laid out  |  E01
| before it starts.                        |
|                                          |
| Across all trainers:                     |  E02
|   6 of 10  -->  8 of 10                  |
|   18 trainers · 41 runs · confirmed      |
| It only reads the project. It can't      |  E03
| change files or go online.               |
|                                          |
| Next: train Coder with this tool.        |  E04
|##########################################|
|#       TRAIN WITH THIS TOOL             #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-09.E01` | What it does | One plain sentence. | `LOOP-2` | NEW |
| `SCR-09.E02` | Collective result | Pooled before and after, with counts. | `LOOP-4`, `LOOP-5` | NEW |
| `SCR-09.E03` | Safety line | Plain words for the `SnapshotRead` access mode. | `LOOP-2` | PARTIAL (access modes exist in `crates/plugin`) |
| `SCR-09.E04` | Next-step line | One line. | — | NEW |
| `SCR-09.E05` | **TRAIN WITH THIS TOOL** | Opens `SCR-03` with this tool selected. | `LOOP-3` | NEW |
| `SCR-09.E06` | **Ask about this tool** (gray link) | Opens `SCR-17` with "What does Project map do?" asked. | `CHAT-3` | NEW (later) |

IDIOT PROOF check: **pass.** One action; the only technical fact is
translated into a safety sentence.

### SCR-10 Rankings

Later. The collective, and a reason to come back.

```
+------------------------------------------+
| [< Menu]   RANKINGS                      |
|------------------------------------------|
| [ This week ] [ Season ]                 |  E01
|  1  Trainer 2PX    Level 9   1,240 XP    |  E02
|  2  Trainer QA4    Level 8   1,100 XP    |
|  …                                       |
| 41  YOU · Trainer 7KQ  Level 2  190 XP   |  E03 (pinned)
|      12 XP to pass #40                   |
|                                          |
| Next: one more run could pass #40.       |  E04
|##########################################|
|#         CLIMB: ENTER THE GYM           #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

States: empty week says "The week just started. Train first to top the
list." Offline and error use `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-10.E01` | Week and season toggle | Two plain choices. | `LOOP-6` | NEW |
| `SCR-10.E02` | Ranked trainers | Only trainers who chose to show their level. | `LOOP-5` | PARTIAL (NIP-XP ledgers and the show-my-level profile exist; no ranking) |
| `SCR-10.E03` | Your row, pinned | Shows the gap to the next rank. | `LOOP-6` | NEW |
| `SCR-10.E04` | Next-step line | One line. | — | NEW |
| `SCR-10.E05` | **CLIMB: ENTER THE GYM** | Opens `SCR-03`. | `LOOP-2` | NEW |

IDIOT PROOF check: **pass.**

### SCR-11 Profile

In v1, a minimal screen: level, XP, titles, and help.

```
+------------------------------------------+
| [< Menu]   PROFILE                       |
|------------------------------------------|
| [avatar] Trainer 7KQ                     |  E01
| Level 2 · 190 XP · 93 XP to level 3      |  E02
| [######----]                             |
| Titles: PLAYTESTER                       |  E03
|                                          |
| YOUR RUNS                                |  E04
|  Project map   6 → 8   Better   +50 XP   |
|  Code finder   6 → 6   No change +50 XP  |
|                                          |
| [ Show my level to others   (on) ]       |  E05
| [ Report a problem ]                     |  E06
| [ Advanced ]  (later)                    |  E07
|                                          |
| Next: train Coder to reach level 3.      |  E08
|##########################################|
|#           ENTER THE GYM                #|  E09 (primary)
|##########################################|
+------------------------------------------+
```

States: no runs yet (only if the first run failed) shows "No runs yet. Your
first one takes about 5 minutes." Offline shows cached numbers marked
"Last updated 10:42".

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-11.E01` | Avatar and trainer name | Identity without a key. | `LOOP-5` | PARTIAL (the key exists; the name is NEW) |
| `SCR-11.E02` | Level and XP | Progress. | `LOOP-5` | EXISTS (Account > Trainer) |
| `SCR-11.E03` | Titles | Loot earned. | `LOOP-5` | EXISTS (Trainer and Playtest cards) |
| `SCR-11.E04` | Your runs | History with before, after, verdict, and XP. | `LOOP-4` | NEW |
| `SCR-11.E05` | **Show my level to others** | A plain switch for the trainer profile. Confirms "Show your level to everyone?" once. | `LOOP-5` | EXISTS (Show my level / Hide my level) |
| `SCR-11.E06` | **Report a problem** | Opens `SCR-13`. | Playtest | EXISTS |
| `SCR-11.E07` | **Advanced** | Later: the player's key, linking keys, and export, in technical words, behind one screen; also **Your computers** and the **Wallet**, the screens a chat offer can open (`CHAT-8`). | — | EXISTS (Identity keys, Link a key, Export card, Computers, Wallet; today as Account rows and the Wallet tab) |
| `SCR-11.E08` | Next-step line | One line. | — | NEW |
| `SCR-11.E09` | **ENTER THE GYM** | Opens `SCR-03`. | `LOOP-2` | NEW |

IDIOT PROOF check: **pass.** Technical items sit behind **Advanced**, off the
primary surface.

### SCR-12 Updates

Later. Opened from the bell (`SCR-01.E02`). In-app only; push notifications
are later.

```
+------------------------------------------+
| [< Menu]   UPDATES                       |
|------------------------------------------|
| ● Trainer 2PX confirmed your result.     |  E01
|   +50 XP is yours.         [ SEE IT > ]  |
| ● Coder now uses Project map for         |
|   everyone. You helped.    [ SEE IT > ]  |
| ○ A check is waiting for you.            |
|                            [ CHECK > ]   |
+------------------------------------------+
```

States: empty says "Nothing new. Your next update comes when someone checks
your result." with **ENTER THE GYM**.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-12.E01` | Update rows, each with one button | Each update links to the screen it's about. The newest row's button is the primary. | `LOOP-6` | NEW (no notification system exists) |

IDIOT PROOF check: **pass with a note.** Each row has exactly one action.

### SCR-13 Report a problem

A sheet. Opened from Profile or a long press anywhere.

```
+------------------------------------------+
| REPORT A PROBLEM                  [ X ]  |
|------------------------------------------|
| What happened?                           |  E01
| [                                      ] |
| [ Add a picture of this screen  (on) ]   |  E02
| [ Share this chat (12 messages)  (off) ] |  E05 (from chat only)
|                                          |
| Next: tell us what went wrong.           |  E03
|##########################################|
|#               SEND                     #|  E04 (primary)
|##########################################|
+------------------------------------------+
```

States: sent shows "Thanks. We got it. Code: 4F2A." with **DONE**. Offline
shows "Saved. We'll send it when you're back online." with **DONE**.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-13.E01` | One text box | Replaces three boxes with one. | Playtest | PARTIAL (the sheet exists with three sections) |
| `SCR-13.E02` | Screenshot switch | Attaches the screen. | Playtest | EXISTS |
| `SCR-13.E03` | Next-step line | One line. | — | NEW |
| `SCR-13.E04` | **SEND** | Sends the report; accepted reports earn playtest XP. | `LOOP-5` | EXISTS |
| `SCR-13.E05` | **Share this chat** | Shown only when the report starts from a chat. Off by default; turning it on shows the whole chat first, and the report carries exactly the chat previewed, sealed to the triage team. | Playtest | EXISTS |

IDIOT PROOF check: **pass.** Sharing a chat is off unless the player turns
it on, and they see what they send.

### SCR-14 Just-in-time setup prompt

Later. A sheet shown only when a setup step becomes necessary, with one
sentence of why and one action. Never shown before the first win.

```
+------------------------------------------+
| SAVE YOUR PROGRESS                       |
|------------------------------------------|
| You're level 3. Save your progress so    |  E01
| you can get it back on a new phone.      |
|##########################################|
|#          SAVE MY PROGRESS              #|  E02 (primary)
|##########################################|
| [ Not now ]                              |  E03
+------------------------------------------+
```

`SCR-14.E01` is the one-sentence reason, `SCR-14.E02` the primary, and
`SCR-14.E03` **Not now**. Each trigger is its own variant:

| ID | Trigger | One sentence | Primary | Status |
| --- | --- | --- | --- | --- |
| `SCR-14.E04` | Reaching level 3 | "Save your progress so you can get it back on a new phone." | **SAVE MY PROGRESS** (backs up the key; words shown behind one warning) | PARTIAL (Reveal nsec exists; a guided backup is NEW) |
| `SCR-14.E05` | Asking chat for work on your own code, or choosing to train Coder on it (later) | "To work on your own code, connect your computer." | **CONNECT MY COMPUTER** | PARTIAL (chat already does this just in time: the **Connect a computer** chip shows only when the router says a message needs a computer, and opens Computers; the sheet form is NEW) |
| `SCR-14.E06` | First reward paid in bitcoin (later) | "You earned a reward. Open your wallet to keep it." | **OPEN MY WALLET** | PARTIAL (the Wallet exists) |

**Not now** always returns to where the player was, and the prompt returns
after the next level. Any step that spends money or reveals a secret key
confirms in plain words, such as "Pay 500 to Trainer 2PX? You can't undo
this." with **PAY** and **CANCEL**.

IDIOT PROOF check: **pass.** One reason, one action, and a safe way out.

### PAT-01 Offline, error, and empty states

Every screen uses this pattern for failures. It never shows an error code as
the main text and never leaves the player without a button.

```
+------------------------------------------+
| [icon]                                   |
| You're offline.                          |  E01 (what happened, plain)
| Training needs the internet.             |  E02 (what it means)
| Nothing is lost.                         |  E03 (reassurance)
|##########################################|
|#             TRY AGAIN                  #|  E04 (primary)
|##########################################|
| [ Back to menu ]                         |  E05
| Code: NET-01 (for reports)               |  E06 (small, gray)
+------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `PAT-01.E01` | What happened | One plain sentence. | NEW |
| `PAT-01.E02` | What it means | One sentence. | NEW |
| `PAT-01.E03` | Reassurance | "Nothing is lost", "This didn't use a run", or similar, only when true. | NEW |
| `PAT-01.E04` | One primary action | **TRY AGAIN**, or the specific next step. | NEW |
| `PAT-01.E05` | **Back to menu** | Always present except inside the first-run steps, where it reads **Back** to the current step. | NEW |
| `PAT-01.E06` | Code for reports | Small and gray, for `SCR-13`. | NEW |

Unknown values show "Not known yet", never `0`.

## Chat screens

Chat is built and on `main`; these screens describe it as it is, mark what
the loop still needs, and fix what fails IDIOT PROOF. Today the chat is the
app's first tab; in this spec's hub it opens from `SCR-01.E12`, and its
top-left control returns to the menu. The code is
`crates/openagents-mobile/src/coder_tab.rs` (the screens),
`basic_coder.rs` and `basic_chats.rs` (the OpenAgents chat),
`router.rs` (offers), and `playtest.rs` (Wrong answer, Share this chat),
drawn by the iOS and Android hosts.

### SCR-15 Chat: new chat

Where chat opens: a new chat, ready to type. The composer is the one primary
action; the keyboard is up and the cursor is in it.

```
+------------------------------------------+
| [<] [☰]   OpenAgents      [Cloud v]    |  E11, E01, E02, E03
|------------------------------------------|
|                                          |
|   Ask us anything. No setup needed.      |  E05
|   We answer here, and send Coder to your |
|   computer when a job needs one.         |
|                                          |
|                                          |
| (?) Who are you?  (?) What can you do?   |  E06 (suggestion chips)
| (?) What's the Gym?  (clock) Fix login…  |
| (+) Connect a computer                   |
|##########################################|
|# Message OpenAgents                 (^) #|  E07, E08 (primary)
|##########################################|
+------------------------------------------+
```

With the selector open, the chips give way to the targets:

```
| (check) Cloud   (computer) Studio Mac    |  E04
| (+) Connect a computer                   |
```

States:

- **First chat ever:** the chips are first-time questions from the answer
  bank ("Who are you?", "What can you do?", "What does it cost?", "What's
  the Gym?"), each answered at once.
- **Returning:** the chips continue the newest chats (clock glyph); with a
  computer chosen, they also offer its other workspaces (folder glyph).
- **No computer added:** the target is **Cloud**; **Connect a computer** is
  the last chip. Nothing about computers blocks sending.
- **A computer chosen and ready:** the selector reads **Studio Mac ·
  openagents**, the placeholder reads **Message OpenAgents on Studio Mac**,
  and Send starts Coder there (`SCR-19`). A new chat never picks a computer
  by itself.
- **A computer chosen but connecting or offline:** one line, "Connecting to
  Studio Mac…" or "Studio Mac is offline.", and Send waits; the selector
  switches back to **Cloud** in one tap.
- **Phone offline:** the message is kept; the reply area says "You're
  offline. We'll send it when you're back." with **Try again** (`PAT-01`).
- **Daily limit reached:** "We've answered all the messages we can for you
  today. Try again in 3 hours." (see `E09`).

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-15.E01` | **☰** Previous chats | Opens `SCR-16`. | — | EXISTS |
| `SCR-15.E02` | Title **OpenAgents** | Says who you're talking to. | `CHAT-1` | EXISTS |
| `SCR-15.E03` | Target selector | Where the first message goes: **Cloud** (the default, always) or a computer and workspace the player picked. | `CHAT-7` | EXISTS |
| `SCR-15.E04` | Target choices | Each computer this phone may use, **Cloud**, and **Connect a computer**, with a check on the current one. | `CHAT-7` | EXISTS |
| `SCR-15.E05` | Welcome lines | Two plain lines that say what chat is for, shown only while the chat is empty. The empty screen's "Next:" answer (`CHK-03`). | `CHAT-1` | NEW (the empty chat is blank today) |
| `SCR-15.E06` | Suggestion chips | Recent chats, workspaces, and **Connect a computer** today; the worker may rank them once each time the tab shows. First-time question chips from the bank, and a **Go to the Gym** chip for a player with runs left, are NEW. | `CHAT-1`, `CHAT-2` | PARTIAL |
| `SCR-15.E07` | Composer **Message OpenAgents** | Focused on open; grows to six lines. A tap outside any text field puts the keyboard away. | All chat | EXISTS |
| `SCR-15.E08` | Send | Sends a NIP-CJ job to the chat worker (or a Coder task to the chosen computer) and opens `SCR-17`. | All chat | EXISTS |
| `SCR-15.E09` | Wait and limit lines | Plain lines for a busy or used-up chat. Today they say "Coder has answered all the messages it can today…"; they should speak as "we" (rule 3 of [Chat in the loop](#chat-in-the-loop)). | — | PARTIAL (copy speaks as Coder) |
| `SCR-15.E10` | Welcome for first-timers from `SCR-02.E06` | A **< Back** that returns to step 1 of 3, and the first-time chips. | `CHAT-1` | NEW |
| `SCR-15.E11` | **< Menu** | Back to `SCR-01`. | — | NEW (Chat is a tab today) |

Transitions: in from `SCR-01.E12`, `SCR-02.E06`, `SCR-04.E09`, `SCR-16`
(New chat). Out to `SCR-17` (Send on Cloud), `SCR-19` (Send on a computer,
or a recent Coder chat chip), `SCR-16`, Computers (**Connect a computer**),
or back.

IDIOT PROOF check: **pass with a note.** One primary (the composer), no
setup, suggestions for someone with no idea what to type. The note: the
empty screen needs `E05` to answer "what do I do next?", and `E09` must
speak in our voice.

### SCR-16 Chat: previous chats

Behind **☰**. Every chat, newest message first.

```
+------------------------------------------+
| [< OpenAgents]    Chats          [new]   |  E01, E02, E03
|------------------------------------------|
| Studio Mac is offline.                   |  E04 (only when true)
| Why did Coder do better with Proje…  2m  |  E05
| (computer) Fix the login test · Stu…  1h |
| What does it cost?                  Mon  |
+------------------------------------------+
```

States: no chats yet shows "No chats yet." with **New chat** as the one
action. The list paints at once from what the phone kept, while computers
are read again; a computer that is connecting or offline gets one line.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-16.E01` | **< OpenAgents** | Back to the chat under the list. | — | EXISTS |
| `SCR-16.E02` | Title **Chats** | — | — | EXISTS |
| `SCR-16.E03` | **New chat** | Opens `SCR-15`. | — | EXISTS |
| `SCR-16.E04` | Computer status line | "Connecting to Studio Mac…" or "Studio Mac is offline." | `CHAT-7` | EXISTS |
| `SCR-16.E05` | Chat rows | OpenAgents chats and Coder chats on your computers, newest first; tapping opens `SCR-17` or `SCR-19`. No Claude Code, Codex, OpenCode, or Devin lists: a session Coder delegated shows inside its Coder chat. | All chat | EXISTS |

IDIOT PROOF check: **pass with a note** (a list; one action kind, like
`SCR-08`).

### SCR-17 Chat: a conversation

A chat with OpenAgents. The reply's first words show within about a second:
a prepared answer is the whole reply at once; otherwise a short opener
("Here's how that works.") shows while the model's answer streams in.

```
+------------------------------------------+
| [☰]   OpenAgents                  [new]  |  E01
|------------------------------------------|
|                  What can you do here? > |  E02 (you)
| We answer questions, explain things, and |  E03 (reply)
| help you plan and write. When something  |
| needs a computer, we dispatch Coder…     |
|   Prepared answer                        |  E04
|                                          |
| (computer) Run Coder on Studio Mac       |  E05 (offer)
| (wallet) Open Wallet                     |  E06 (screen chip)
| +--------------------------------------+ |
| | openagents computer list             | |  E07 (command card)
| | Reads only. Runs on this phone.      | |
| | [ (>_) Run ]                         | |
| +--------------------------------------+ |
| (?) What model is this?  (?) What does   |  E08 (follow-ups)
|     it cost?                             |
| [ (flag) Wrong answer ]                  |  E09
|##########################################|
|# Message OpenAgents                 (^) #|  E10
|##########################################|
+------------------------------------------+
```

A reply shows only the offers the router made for it; the wireframe stacks
them all to name each one. A reply with an offer makes that offer the
screen's primary; otherwise the composer is.

States:

- **Waiting for the first words:** "Thinking" in the reply's place (today
  it reads "Coder is thinking"; it should read "Thinking", since OpenAgents
  is answering).
- **Streaming:** the reply grows as it's written, drawn as Markdown.
- **Failed:** one plain line from the worker's refusal or the network, and
  **Try again**.
- **After Run Coder:** **Open Coder on Studio Mac** replaces the offer and
  opens `SCR-19`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-17.E01` | Header: **☰**, **OpenAgents**, **New chat** | Previous chats, who's answering, a new chat. | — | EXISTS |
| `SCR-17.E02` | Your message | — | — | EXISTS |
| `SCR-17.E03` | The reply | A prepared answer, an opener then the model's streamed reply, or a product answer from the sourced notes. Speaks as "we". | `CHAT-1`, `CHAT-3`, `CHAT-6` | EXISTS |
| `SCR-17.E04` | **Prepared answer** note | Quiet gray note under a bank answer, so the player knows it's reviewed text. | `CHAT-1` | EXISTS |
| `SCR-17.E05` | Coder offer | **Run Coder on Studio Mac** with a computer ready (a chip when the router says the message needs a computer, a plain button otherwise); **Connect a computer** with none, only when the message needs one; then **Open Coder on Studio Mac**. | `CHAT-7` | EXISTS |
| `SCR-17.E06` | Screen chips | **Open Wallet**, **Your computers** (or **Connect a computer**), **Identity keys**, **Playtest**, **Report a problem**; the label is the app's, never the model's. | `CHAT-8` | EXISTS |
| `SCR-17.E07` | Command card | A read-only `openagents` command, where it runs, and **Run**; then its output and **Run again**. The phone answers `computer list`, `show`, and `workspaces` itself. | `CHAT-8` | PARTIAL (the phone's card exists; the CLI route is built in `coder::cli_route` but not yet wired into the chat worker, so no live reply offers one yet) |
| `SCR-17.E08` | Follow-up chips | The next likely questions under a prepared answer; a tap sends it. | `CHAT-1` | EXISTS |
| `SCR-17.E09` | **Wrong answer** | Under a prepared answer only. Opens `SCR-18`. | `CHAT-1` | EXISTS |
| `SCR-17.E10` | Composer **Message OpenAgents** | The next message in the same chat. | All chat | EXISTS |
| `SCR-17.E11` | Loop offers: **Go to the Gym**, **Train with this tool**, **Start training**, **See the result**, **Enter the Gym** | The `CHAT-2` to `CHAT-6` taps, each opening its loop screen with the choice filled in. | `CHAT-2` to `CHAT-6` | NEW (needs a Gym screen in the router's `open_screen` set and the hosted runner) |
| `SCR-17.E12` | Result line | "6 of 10 → 8 of 10 with Project map. Better." from the player's own runs, in a reply about them. | `CHAT-5` | NEW |
| `SCR-17.E13` | **Try again** | After a failed reply; resends the same message. | — | EXISTS |

Transitions: in from `SCR-15`, `SCR-16`, `SCR-05.E11`, `SCR-09.E06`. Out to
`SCR-16`, `SCR-15`, `SCR-18`, `SCR-19`, the screens a chip names, and the
loop screens (`SCR-03`, `SCR-05`) through `E11`.

IDIOT PROOF check: **pass with a note.** Every reply that implies an action
carries it as a tap, and nothing happens without one (`CHK-11`). The note:
the command card shows a raw command; it is secondary, labeled "Reads only",
and belongs behind the player's question, not on a first screen.

### SCR-18 Chat: Wrong answer

Inline under a prepared answer, not a new screen.

```
| [ (flag) Wrong answer ]                  |  E01
          (tap)
| We'll send your question, our answer,    |  E02
| and how we chose it to the OpenAgents    |
| team, encrypted, to improve our answers. |
| Nothing else from this chat is sent.     |
| [#### Send ####]  [ Cancel ]             |  E03, E04
          (Send)
| Sending…  →  Sent to the OpenAgents team |  E05
| as 4F2A. Thank you.                      |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `SCR-18.E01` | **Wrong answer** | Asks first; sends nothing yet. | EXISTS |
| `SCR-18.E02` | What will be sent | The question, the prepared answer, and the router's judgment only. | EXISTS |
| `SCR-18.E03` | **Send** | Files a playtest report sealed to the triage team. | EXISTS |
| `SCR-18.E04` | **Cancel** | Back to the chat; nothing sent. | EXISTS |
| `SCR-18.E05` | Outcome line | "Sending…", then "Sent to the OpenAgents team as 4F2A. Thank you.", or "Saved on this phone…" when this build can't send yet, or why it failed. | EXISTS |

IDIOT PROOF check: **pass.** Says exactly what leaves the phone before
anything does.

### SCR-19 Chat: Coder on a computer

A Coder chat on the player's computer, reached by **Run Coder**, by sending
from `SCR-15` with a computer chosen, or from `SCR-16`. It exists for
players who connected a computer; it is outside the first loop.

```
+------------------------------------------+
| [☰]  Working · Studio Mac        [new]   |  E01
|------------------------------------------|
|                 Fix the login test.   >  |
| Found it: the test expects the old…      |  E02 (streams)
| $ cargo test login  ✓                    |  E03
| (→) Delegated to OpenCode            >   |  E04
|                                          |
| Coder asked: Keep the old redirect?      |  E05
| [ Approve ]  [ Deny ]                    |  E06 (only when asked)
|##########################################|
|# Answer Coder                       (^) #|  E07
|##########################################|
| [ Stop ]  [ Edit queue ]                 |  E08
+------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `SCR-19.E01` | Header: phase and computer | "Queued", "Working", "Done" and the computer's name. | EXISTS |
| `SCR-19.E02` | Coder's reply, streamed | Each paragraph appears as Coder writes it. | EXISTS |
| `SCR-19.E03` | Commands and their output | What Coder ran, and how the run ended. | EXISTS |
| `SCR-19.E04` | **Delegated to OpenCode** (or Devin) | Opens the delegated session's messages, inside this chat. | EXISTS |
| `SCR-19.E05` | Coder's question | The composer becomes the answer. | EXISTS |
| `SCR-19.E06` | **Approve** / **Deny** | Only for an approval request. | EXISTS |
| `SCR-19.E07` | Composer | Follow-up, queued for the next turn, or the answer to a question; a long press offers the other ways to send. Every message survives a relaunch and bad signal and never runs twice. | EXISTS |
| `SCR-19.E08` | **Stop**, **Edit queue** | Interrupt the turn; edit what waits. | EXISTS |

IDIOT PROOF check: **pass with a note.** A power surface ("queue", long
press options) reached only after a player chose to connect a computer; the
first loop never passes through it.

## CIN-01 Intro cinematic

Plays once, in `FLOW-01`, right after the player chooses their agent on
`SCR-02`, like the cinematic after character creation in a role-playing
game. The camera sweeps the Grid, the Gym, and the wider world, then settles
behind the player's agent. A narrator explains why we train agents and what
the player gets. It ends by handing off to the single next action: the first
Gym run.

IDIOT PROOF rules for the cinematic:

- Subtitles are always on, large, white on a dark band at the bottom.
- The first play is short (about 60 seconds) and plays through. After the
  first play (on a replay from Profile), one clear **Skip** sits top right.
- If the app is closed during the cinematic, the next open resumes at the
  end card, not the start, so the player is never stuck.
- The end card has one button, **GO TO THE GYM**, which opens `SCR-03`
  at step 2 of 3.
- Sound is optional: the subtitles carry the whole script with the phone on
  silent.

### Shot list

| Shot | Camera move | On screen | Subtitle (narration) | Duration |
| --- | --- | --- | --- | --- |
| `CIN-01.S01` | Fade in from black; slow push toward a single point of light. | Black, then the power emblem glowing. | "Every day, people ask AI agents to write their code." | 5 s |
| `CIN-01.S02` | High crane shot descending over the white wireframe Grid. | The Grid's plaza, the ball, blocks, and dominoes. | "One agent, working alone, only gets so good." | 7 s |
| `CIN-01.S03` | Low tracking shot past other players' avatars and their name tags. | Other trainers in the plaza, walking and playing. | "So we're doing it together. This is the Verse, where agents and people meet." | 8 s |
| `CIN-01.S04` | Dolly through the Gym doorway to the board inside. | The Gym building, its board lit, ghost figures replaying a run between stations. | "This is the Gym. Here, you train your agent. You give it a new tool, and we measure if it helps." | 10 s |
| `CIN-01.S05` | Wide pull-back and rise; many small lights stream into one large emblem above the world. | The whole Grid from above; the Lagrange 1 station in the sky. | "When a tool helps, every agent gets it. Many agents, trained by many people, combined into one: OpenAgents." | 10 s |
| `CIN-01.S06` | Quick cuts: a title appearing over a name tag, a level number rising, the ball glowing. | Titles, a level-up, the glowing ball. | "Train well, and you earn XP, loot, rewards, and glory." (See the note on bitcoin below.) | 7 s |
| `CIN-01.S07` | Swoop down and settle behind the player's agent, into the follow camera. | Coder, the matte-black agent with the glowing emblem, facing the Gym. | "This is Coder. It's yours to train." | 6 s |
| `CIN-01.S08` | Hold; the end card fades in over the live scene. | End card: **GO TO THE GYM** and "Step 2 of 3". | "Let's go to the Gym." | 4 s, then waits |

Total: about 57 seconds before the end card. A v1 short cut keeps `S02`,
`S04`, `S05`, `S07`, and `S08` (about 37 seconds).

### Narration script

Narrated in our plural "we" voice. The subtitles match word for word.

> Every day, people ask AI agents to write their code.
>
> One agent, working alone, only gets so good.
>
> So we're doing it together. This is the Verse, where agents and people
> meet.
>
> This is the Gym. Here, you train your agent. You give it a new tool, and we
> measure if it helps.
>
> When a tool helps, every agent gets it. Many agents, trained by many
> people, combined into one: OpenAgents.
>
> Train well, and you earn XP, loot, rewards, and glory.
>
> This is Coder. It's yours to train.
>
> Let's go to the Gym.

Note on bitcoin: the owner's brief lists bitcoin among the rewards. Nothing
pays bitcoin for training today, and awards are XP and titles only. Until a
paid reward path ships, `S06` uses the line above. When it ships, `S06`
becomes "Train well, and you earn XP, loot, bitcoin, and glory." We don't
narrate a reward the app can't give.

### What exists to build it

| Need | Status | Where |
| --- | --- | --- |
| The Grid, plaza, ball, blocks, dominoes | EXISTS | `crates/verse` (`WorldRuntime::bare`), phone Verse tab |
| Other players' avatars and name tags | EXISTS | Grid presence; `crates/verse/src/avatar.rs` |
| The Gym building and board | EXISTS | `crates/verse/src/gym.rs`, `docs/verse/gym.md` |
| Ghost figures replaying a run | EXISTS | `crates/verse/src/gym_replay.rs`, `replay.rs` |
| A timed clock with play, seek, and speed | EXISTS | `replay::Clock`, `replay::Track` |
| Third-person follow camera that swings back behind the player | EXISTS | `crates/verse/src/camera.rs` (`FollowCamera::settle`) |
| The player's agent as a companion figure | PARTIAL | `crates/verse/src/agent.rs` (a floating spade on desktop; the Grid has no companion); the Coder figure is NEW |
| The Lagrange 1 station in the sky | PARTIAL | `crates/verse-lagrange`; the portal is hidden on the phone |
| Scripted camera path (keyframes, easing, crane, dolly) | NEW | Nothing in `crates/verse`. `three-effect` has no camera-path primitive, and the phone renderer is Rust, not `three-effect`. |
| Subtitle band, end card, skip control | NEW | Native overlay over the Verse view |
| Voice-over audio | NEW | No audio pipeline in the Verse today |
| The "lounge" | NEW | `lounge` exists only as a desktop chat room; there is no lounge place. `S03` uses the plaza instead. |
| The stream of lights and the big emblem (`S05`) | NEW | Effect in the Verse renderer |

## User flows

### FLOW-01 First-time playtester

A guided path of three steps that can't be skipped into a broken state. At
most 3 taps from app open to the first Gym run starting.

```
 App open (first time)
   |
   v
 SCR-02 Choose your agent   STEP 1 OF 3
   |  tap 1: CHOOSE CODER   (identity created silently)
   v
 CIN-01 Intro cinematic     (~60 s, plays through the first time)
   |  tap 2: GO TO THE GYM
   v
 SCR-03 The Gym             STEP 2 OF 3   (Project map preselected)
   |  tap 3: START TRAINING  <-- the first run starts here
   v
 SCR-04 Training            STEP 3 OF 3   (~5 min; safe to leave)
   |  SEE THE RESULT
   v
 SCR-05 Result              "6 of 10 --> 8 of 10", +50 XP
   |  ADD MY RESULT TO THE GYM
   v
 SCR-06 Level up (if a level was crossed)
   |  NICE
   v
 SCR-01 Main menu           guided path complete
```

Rules that keep the path unbreakable:

1. The app records the furthest step reached. On any reopen, it resumes at
   that step: `SCR-02`, the `CIN-01` end card, `SCR-03`, `SCR-04`, or
   `SCR-05`. `SCR-01` appears only after `SCR-05`.
2. Steps 1 to 3 hide **< Menu**. The only way forward is the primary button.
   The one way sideways is chat: **Ask OpenAgents a question first**
   (`SCR-02.E06`) and **Ask OpenAgents while you wait** (`SCR-04.E09`) open
   chat, and **< Back** from it returns to the same step. Chat never skips
   or finishes a step.
3. No step asks for a sign-in, name, email, computer, wallet funding, or key
   backup. Identity is created silently at step 1.
4. If training fails on our side, **TRY AGAIN** restarts the same run and
   doesn't use a daily run.
5. If the player is offline at `SCR-03`, the screen says so and waits with
   **TRY AGAIN**. The step doesn't change.

### FLOW-02 Returning playtester, daily loop

Two taps from app open to a run.

```
 App open
   |
   v
 SCR-01 Main menu   "Next: a check is waiting for you (+50 XP)."
   |  tap 1: ENTER THE GYM
   v
 SCR-03 The Gym     recommended: CHECK A RESULT, or a new tool
   |  tap 2: START THE CHECK / START TRAINING
   v
 SCR-04 Training --> SCR-05 Result --> ADD MY RESULT TO THE GYM
   |
   v
 SCR-06 Level up (if crossed) --> SCR-01
   |
   +--> later: SCR-10 Rankings ("12 XP to pass #40") --> ENTER THE GYM
   +--> later: SCR-12 Updates ("Your result was confirmed.") --> SEE IT
   +--> any time: SCR-01.E12 CHAT WITH OPENAGENTS --> SCR-15
          "What should I try next?" --> Go to the Gym --> SCR-03 (FLOW-04)
```

Reasons to return, each shown as the `SCR-01.E11` next-step line:

1. New daily runs ("3 runs left today").
2. A check is waiting (another trainer's result to confirm).
3. Your result was confirmed, or a tool you tested now ships to everyone.
4. Your rank is within reach of the next place (later).
5. The season ends on a date (Season 1 ends 2026-10-26).

### FLOW-03 Kicking the tires in chat

A first-timer, or anyone curious, asks what this is before doing anything.
Zero setup, answers in under a second.

```
 SCR-01.E12 CHAT WITH OPENAGENTS   (or SCR-02.E06 on the first open)
   |
   v
 SCR-15 New chat      chips: "Who are you?"  "What can you do?"
   |  tap a chip, or type                   "What does it cost?"
   v
 SCR-17 Conversation  prepared answer at once · "Prepared answer"
   |                  follow-up chips: "What model is this?" ...
   |  tap a follow-up (repeat)
   v
 A question the bank doesn't cover --> opener at once, then the model's
   answer streams in; a product question is answered from our notes
   |
   +--> "What's the Gym?" --> answer + Go to the Gym (later) --> SCR-03
   +--> < Back / < Menu --> where the player was
```

Status: EXISTS up to the Gym offer (`SCR-17.E11` is NEW) and the first-time
chips (`SCR-15.E06` is PARTIAL).

### FLOW-04 From chat to the Gym

Later. Chat hands the player to the loop with the choice filled in; the
loop screen's own button does the work.

```
 SCR-17  "What should I try next?"
   |     "Give Coder Project map. 18 trainers tried it; most saw Coder
   |      do better."   [Go to the Gym]
   v  tap 1
 SCR-03 The Gym       Project map preselected
   |  tap 2: START TRAINING
   v
 SCR-04 --> SCR-05 --> [Ask about this result] --> SCR-17 (CHAT-5)
```

Two taps from a chat answer to a run, the same as `FLOW-02`. Status: NEW
(`CHAT-2`, `SCR-17.E11`, the hosted runner).

### FLOW-05 Dispatching Coder from chat

Work on the player's own code. Needs a computer only at the moment it's
needed.

```
 SCR-17  "Fix the failing login test in my repo"
   |     "We'll dispatch Coder to fix the failing login test."
   |
   +-- computer ready -->  [Run Coder on Studio Mac]
   |                          | tap
   |                          v
   |                       SCR-19 Coder on Studio Mac, reply streams in;
   |                       SCR-17 now shows [Open Coder on Studio Mac]
   |
   +-- no computer ----->  [Connect a computer]
                              | tap
                              v
                           Computers (Add a computer); the conversation is
                           kept, and Run Coder appears once it's ready
```

Status: EXISTS (`CHAT-7`).

### FLOW-06 Wrong answer and Share this chat

How a tester tells us chat got it wrong. Nothing leaves the phone without
a confirming tap.

```
 SCR-17 prepared answer --> [Wrong answer] --> SCR-18: what will be sent
   |                                             |  Send
   |                                             v
   |                                   "Sent to the OpenAgents team as
   |                                    4F2A. Thank you."
   |
 Any chat --> long press the tab bar (later: anywhere) --> SCR-13
   Report a problem --> [Share this chat (12 messages)] off by default;
   turning it on shows the chat first --> SEND
```

Status: EXISTS.

## NAV-01 Navigation map

Hub and spoke. The main menu is the hub; there is no tab bar in this
experience.

```
                        [first open only]
                  SCR-02 --> CIN-01 --> SCR-03 (step 2)
                                            |
                                            v
  SCR-12 Updates <--bell--+            SCR-04 --> SCR-05 --> SCR-06
        (later)           |                           |          |
                          |                           v          v
  SCR-07 Coder <----------+------- SCR-01 MAIN MENU <-------------+
    (later)  \            |          |      ^
              \           |          |      |
               v          v          v      |
  SCR-09 <-- SCR-08 <-- SCR-03 The Gym  ----+
  (later)    (later)      ^
                          |
  SCR-10 Rankings --------+  (later; CLIMB: ENTER THE GYM)
  SCR-11 Profile ---------+  (ENTER THE GYM)
     |
     +--> SCR-13 Report a problem (sheet; also a long press anywhere)
     +--> Advanced (later): keys, Your computers, Wallet

  CHAT (from SCR-01.E12; also SCR-02.E06, SCR-04.E09, SCR-05.E11)
     SCR-15 New chat <--> SCR-16 Previous chats (☰)
        |  Send                     |  tap a chat
        v                           v
     SCR-17 Conversation ------> SCR-19 Coder on a computer
        |   Run Coder / Open Coder
        +--> SCR-18 Wrong answer (inline)
        +--> screen chips: Wallet, Your computers, Identity keys,
        |    Playtest, Report a problem (Profile > Advanced, SCR-13)
        +--> loop offers (later): SCR-03 The Gym, SCR-05 Result

  SCR-14 setup prompts open over any screen when triggered (later).
  PAT-01 replaces a screen's body on failure.
```

Every screen reaches `SCR-03` in at most two taps, and every screen reaches
`SCR-01` in one. `SCR-01` reaches chat in one tap. Today the app is still
four tabs (Chat, Verse, Wallet, Account) with Chat first; the hub replaces
the tab bar, and Chat keeps its place as the first thing below the one
primary button.

## IDIOT PROOF checklist

| ID | Check |
| --- | --- |
| `CHK-01` | One primary action: a single big white-filled button; everything else is outlined or gray. |
| `CHK-02` | No jargon in any label (see the banned list in [Words on screen](#words-on-screen)). |
| `CHK-03` | A one-line "Next:" answer to "what do I do next?" |
| `CHK-04` | No dead ends: every state, including empty, error, and offline, has one clear next step. |
| `CHK-05` | No required setup before the first win: no computer, wallet funding, or key backup. Setup is deferred until needed and explained in one sentence then (`SCR-14`). |
| `CHK-06` | At most 3 taps from app open to starting a Gym run (first time: 3; returning: 2). |
| `CHK-07` | Defaults chosen for the player: Coder as the agent, Project map as the tool. |
| `CHK-08` | Progress always visible: score before and after, XP gained, level bar. |
| `CHK-09` | Destructive or spending actions confirmed in plain words. |
| `CHK-10` | The first-time flow is guided (step 1 of 3 style) and can't be skipped into a broken state. |
| `CHK-11` | Talk leads to a tap: every chat reply that implies an action carries that action as the app's own control, nothing acts without a tap, and chat never claims to have done what it hasn't. |

Results for every screen. "n/a" means the check doesn't apply to that
screen.

| Screen | 01 | 02 | 03 | 04 | 05 | 06 | 07 | 08 | 09 | 10 | 11 | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SCR-01` Main menu | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (1 tap to Gym) | ✓ | ✓ | n/a | n/a | ✓ (chat is a row, not the primary) | Pass |
| `SCR-02` Choose your agent | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (tap 1) | ✓ | ✓ (score shown) | n/a | ✓ | ✓ (chat link returns to step 1) | Pass |
| `CIN-01` Intro cinematic | ✓ | ✓ | ✓ (end card) | ✓ (resumes at end card) | ✓ | ✓ (tap 2) | n/a | n/a | n/a | ✓ | n/a | Pass |
| `SCR-03` The Gym | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (tap 3) | ✓ | ✓ (runs left) | n/a | ✓ | n/a | Pass |
| `SCR-04` Training | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | n/a | ✓ | ✓ (ask while you wait) | Pass |
| `SCR-05` Result | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | ✓ (public notice) | ✓ | ✓ (ask about this result) | Pass |
| `SCR-06` Level up | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | n/a | n/a | n/a | Pass |
| `SCR-07` Coder | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | Pass |
| `SCR-08` All tools | ~ (list; one action kind) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | Pass with note |
| `SCR-09` Tool detail | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | Pass |
| `SCR-10` Rankings | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | Pass |
| `SCR-11` Profile | ✓ | ✓ (technical items under Advanced) | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | ✓ (show level) | n/a | n/a | Pass |
| `SCR-12` Updates | ~ (one action per row) | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | Pass with note |
| `SCR-13` Report a problem | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | ✓ (Share this chat off by default) | Pass |
| `SCR-14` Setup prompt | ✓ | ✓ | ✓ | ✓ (Not now) | ✓ (triggered after) | n/a | n/a | n/a | ✓ | n/a | n/a | Pass |
| `PAT-01` States | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Pass |
| `SCR-15` New chat | ✓ (the composer) | ✓ | ~ (`E05` NEW) | ✓ | ✓ | n/a | ✓ (Cloud by default) | n/a | n/a | n/a | ✓ | Pass with note |
| `SCR-16` Previous chats | ~ (list; one action kind) | ✓ | ✓ | ✓ (No chats yet + New chat) | ✓ | n/a | n/a | n/a | n/a | n/a | n/a | Pass with note |
| `SCR-17` Conversation | ✓ (the offer, else the composer) | ✓ (answers may name vendors when asked) | ✓ (offers and follow-ups) | ✓ (Try again) | ✓ | n/a | n/a | n/a | ✓ (no offer spends or reveals a key) | n/a | ✓ | Pass with note |
| `SCR-18` Wrong answer | ✓ | ✓ | ✓ | ✓ (Cancel) | ✓ | n/a | n/a | n/a | ✓ (says what is sent) | n/a | ✓ | Pass |
| `SCR-19` Coder on a computer | ✓ (the composer) | ~ (queue, long press) | ✓ (phase line) | ✓ | n/a (only after connecting) | n/a | n/a | ✓ | ✓ (Approve, Deny) | n/a | ✓ | Pass with note |

### What today's app still fails

Checked against `main` and TestFlight build 19 on 2026-09-28. Chat now
passes: it opens ready to type, needs no computer, sign-in, or wallet, and
answers common questions in under a second. What still fails:

| Check | Where | What fails today |
| --- | --- | --- |
| `CHK-05`, `CHK-06` | Verse > the Gym board (`VerseGym.swift`) | Starting a Gym run needs a Gym host: the player copies their public key to a host, creates a connection grant there, and pastes a `gym-connect:` code. No hosted run exists. |
| `CHK-05` | Tutorial quests (`docs/verse/tutorial-quests.md`) | Earning XP by reproducing a result needs a desktop with Docker and the command line. |
| `CHK-02` | Verse > the Gym's RESULTS board (`VerseResults.swift`, `VerseTrace.swift`) | Readable with no setup, but in expert words: Terminal-Bench, traces, verifiers, Jev. |
| `CHK-02` | Account > Trainer | The level card is there, but the same screen shows npub, nsec, relay, and `microcoder xp` commands. |
| `CHK-02` | Account > Identity keys, Tailnet | npub, nsec, and hex keys, and a Tailnet screen, one tap from the Account list. |
| `CHK-02` | Wallet tab | The ₿ balance is on the tab's first screen. (The "whole numbers" notice is gone.) |
| `CHK-01`, `CHK-03`, `CHK-10` | The whole app | Four equal tabs, no main menu, no guided first run, and no "Next:" line on any screen. |
| `CHK-03` | Chat, `SCR-15` | An empty new chat has no line that says what to do; the suggestion chips are recent chats and workspaces, not first-time questions. |
| Voice | Chat, `SCR-15.E09`, `SCR-17` | Wait and limit lines say "Coder has answered all the messages it can today"; the waiting label says "Coder is thinking". Both should speak as OpenAgents. |

Revision 1 was wrong or is now out of date on these: chat has needed no
computer since build 17, and since build 18 OpenAgents answers it; since
build 19 new chats go to OpenAgents even with a computer ready; the Wallet's whole-numbers notice is gone; playtest logging is on
with no switch to find; the Lagrange 1 portal is hidden.

This spec fixes the rest by moving the first run to our computers, putting
chat on the main menu as the one secondary row, and moving every technical
term behind **Advanced**.

## Minimal v1 cut and later additions

### v1: the most basic elements to build first

| ID | What v1 includes |
| --- | --- |
| `SCR-02` | The Coder card (preselected) and **CHOOSE CODER**. |
| `CIN-01` | The short cut: `S02`, `S04`, `S05`, `S07`, `S08`, subtitles, the end card. |
| `SCR-03` | Three bundled tools, Project map preselected, **START TRAINING**, runs left. The check card once a first result exists to check. |
| `SCR-04` | Progress blocks, task counter, safe to leave. |
| `SCR-05` | Verdict, before and after, XP, **ADD MY RESULT TO THE GYM**, **Share outside the app**. |
| `SCR-06` | Level up. |
| `SCR-01` | Logo, player card, hero with Gym status, **ENTER THE GYM**, **CHAT WITH OPENAGENTS**, **PROFILE**, season card, footer, next-step line. No bell, Coder row, or Rankings row. |
| `SCR-11` | Name, level, XP, titles, your runs, show my level, **Report a problem**. |
| `SCR-13` | One text box, screenshot, **Share this chat**, **SEND**. |
| `SCR-15` to `SCR-19` | Chat as it exists (composer ready to type, **Cloud** by default, previous chats, prepared answers, offers, Wrong answer, Coder on a computer), plus three fixes: the welcome lines (`SCR-15.E05`), first-time question chips (`SCR-15.E06`), and wait and limit lines in our voice (`SCR-15.E09`). |
| `SCR-02.E06`, `SCR-04.E09`, `SCR-05.E11` | The ways into chat from the loop. |
| `PAT-01` | The shared failure pattern. |

New work v1 depends on, outside the screens:

1. **A hosted practice runner.** We run Coder with the chosen tool on 10
   fixed practice tasks, on our computers, with repeats, with no player
   setup. Today a Gym run can only start on a host the player pairs with a
   `gym-connect:` code.
2. **A measured baseline per Coder version** on the same 10 tasks, so every
   run compares against the same "before".
3. **Tool measurement.** Turn on the three evidence guests (`repo_map`,
   `code_search`, `test_report`) as selectable tools and apply the keep rule
   from the plugin measurement plan to decide **Better**, **No clear
   change**, or **Worse**.
4. **An XP rule for a Gym run and for a check.** The `reproduce` rule exists;
   a `gym-trial` rule is proposed only.
5. **Player-published results** that other trainers can check, signed with
   the player's world key.
6. **Trainer names** derived from the world key, and a "training now" count.
7. **The guided first-run state machine** and the cinematic's camera path.
8. **Chat in the hub**: the chat screens behind `SCR-01.E12` with a **< Menu**
   control instead of a tab, keeping everything chat does today.

### Later additions

| ID | Addition |
| --- | --- |
| `SCR-07` | Coder: score history and the tools Coder uses now. |
| `SCR-08`, `SCR-09` | All tools and tool detail; later, tools written by trainers. |
| `SCR-10` | Rankings, weekly and season. |
| `SCR-12` | Updates, then push notifications. |
| `SCR-14` | Just-in-time setup: save your progress, connect your computer, open your wallet. |
| `CHAT-2` to `CHAT-6`, `SCR-17.E11`, `SCR-17.E12`, `FLOW-04` | Chat's loop offers (**Go to the Gym**, **Train with this tool**, **Start training**, **See the result**) and the player's own results in chat, once the hosted runner and player results exist. This needs a Gym screen in the router's `open_screen` set. |
| `SCR-09.E06` | **Ask about this tool**. |
| `CIN-01` | The full cut with voice-over, and bitcoin in the narration once rewards pay. |
| — | More agents on `SCR-02` once a second agent exists. |
| — | Walking into the Gym in the Verse as a second way into `SCR-03`. |
| — | Bitcoin rewards for tools that ship to everyone, through the existing Wallet, introduced by `SCR-14`. |
| — | Training Coder on your own code, from chat (`CHAT-7` already dispatches Coder to a connected computer). |
| — | Group training ("raids") and parties from the trainer leveling design. |

## Appendix: what we cut and why

Cut from this experience, not deleted from the code. Each can come back when
it serves the loop.

| Cut | Where it lives today | Why |
| --- | --- | --- |
| The four-tab bar (Chat, Verse, Wallet, Account) | `AppTabs.swift` | A hub with one primary action is simpler than four equal tabs. Chat is not cut: it moves to the menu's second row (`SCR-01.E12`). Verse, Wallet, and Account don't serve the first loop as tabs. |
| **BITCOIN WALLET** row and the ₿ balance pill | Wallet tab | No step of the loop pays or spends yet, and money on the main menu invites confusion (sats versus ₿). It returns when rewards pay. |
| Walking the Grid, the sticks, and the physics toys | Verse tab | Steering a 3D world to reach a board isn't IDIOT PROOF and costs taps. The Grid stays as the hero image and the cinematic's set. |
| The results board, trace viewer, caveats, and Jev tabs | `VerseResults.swift`, `VerseTrace.swift` | Built for experts. The player sees one before-and-after instead. |
| The live Gym board with `gym-connect:` pairing and recipes | `VerseGym.swift` | Needs a host and a pasted code. Replaced by the hosted runner. |
| Computers, Tailnet, Terminal, and Agent payments | Account, Wallet | Setup for your own computer; not part of the first loop. |
| Identity keys, Reveal nsec, Link a key, Export card JSON | Account | Technical; moved behind **Advanced** or `SCR-14`. |
| Changelog and "What to test" | Account | Useful to testers, but not to the loop. The version stays in the footer for reports. |
| **My reports** | Account > Playtest | The list of past reports isn't needed to play. (The playtest logging switch is already gone: logging is on for everyone.) |
| **Source code** and **Follow us on X** links | Account | Building a following happens through **Share outside the app** on a result. |
| **ENTER THE VERSE** as the main button in the mockup | Mockup | The main button is now **ENTER THE GYM**, the step that makes Coder better. |
| Tutorial quests run from a desktop | `docs/verse/tutorial-quests.md` | Replaced on the phone by **Check a result**, the same `reproduce` idea with no setup. |

Back from the cut in revision 2:

| Was cut | Now | Why it came back |
| --- | --- | --- |
| Chat with Coder on your computers | `SCR-15` to `SCR-19`, `SCR-01.E12` | Chat needs no computer: OpenAgents answers every new chat, and Coder is dispatched only when a message needs a computer. It serves every loop step (`CHAT-1` to `CHAT-8`). |
| **CHAT WITH AN AGENT** row from the mockup | **CHAT WITH OPENAGENTS** (`SCR-01.E12`) | The same row, in our voice: you chat with OpenAgents, which dispatches agents. |

## Sources

- [Launch roadmap, 2026-09-29](../roadmap/2026-09-29-launch-roadmap.md): the launch plan.
- [Playtesting program](../game/playtesting.md): Season 1, sessions, and playtest XP.
- [Agent trainer leveling](../verse/agent-trainer-leveling.md) and [tutorial quests](../verse/tutorial-quests.md): XP, levels, titles, and quest rules.
- [The Gym building](../verse/gym.md) and [Gym leaderboard](../verse/gym-leaderboard.md): run observation, recipes, and published results.
- [Wasm plugins](../extensions/plugins.md) and [packages](../extensions/packages.md): the plugin host, evidence guests, and the measurement plan.
- [Chat router design](../coder/design/2026-09-28-chat-router.md): routes, the answer bank, offers, and what is implemented.
- [Chat worker deployment](../deployment/chat-worker.md): the service that answers chat.
- Answer bank: `crates/coder/answers/chat-answers-v1.toml`; product notes: `knowledge/openagents/`.
- [Verse on the phone](../verse/mobile.md): the Grid on iOS and Android.
- [NIP-XP](../../nips/openagents/NIP-XP.md), [NIP-EVAL](../../nips/openagents/NIP-EVAL.md), [NIP-EXT](../../nips/openagents/NIP-EXT.md).
- App code: `bins/openagents-ios/host/App/`, `crates/openagents-mobile/src/`, `bins/openagents-android/README.md`, `crates/verse/src/`.
