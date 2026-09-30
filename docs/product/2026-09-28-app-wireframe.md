# OpenAgents phone app: wireframe specification

Written 2026-09-28. Status: revision 3; its v1 cut shipped in build 21 ([what shipped](#build-21-what-shipped)). This page specifies every
screen of the OpenAgents phone app as a text wireframe, with the user flow,
for one closed loop. Each element is marked **EXISTS**, **PARTIAL**, or
**NEW** against the code on `main` (checked 2026-09-29, build 21), so the gap is visible. We start
with the most basic elements and add to this wireframe later.

Revision 3 (2026-09-28, the same night) refocuses the loop from benchmark
scores to **evals**, and puts the whole loop inside **Chat with
OpenAgents**. A person chats with us about what's new in the Gym, picks
or makes a tool and a test set for it, runs the tests with and without the
tool, sees the measured change, adds the result to the Gym, and earns XP
when other trainers check it or Coder adopts the tool. Chat is the entry
point; the Gym's separate pages become cards inside chat, with a sheet
only where chat can't do the job well. The Gym in the Verse stays the
place to review results, and becomes a social place later
([what changed](#revision-3-what-changed)).

Revision 2 made **Chat** a first-class part of the loop and the main menu
([what changed](#revision-2-what-changed)).

Added 2026-09-29: pairing a computer by scanning a QR code
([#9965](https://github.com/OpenAgentsInc/openagents/issues/9965),
[design](../coder/design/2026-09-29-auto-pairing.md)). The phone gains
**Connect a computer** (`SCR-22`) and **Connected** (`SCR-23`); the companion
desktop app's screens are `DSK-01` to `DSK-05`
([Desktop app screens](#desktop-app-screens)). The desktop app installs,
shows a code, and pairs with no terminal step. Coder's work on a project
there uses Codex or Claude Code, which the milestone assumes the person
already signed in to on that Mac (owner decision, 2026-09-29).

> **IDIOT PROOF.** Every screen must be usable by someone who has never
> heard of agents, Nostr, Bitcoin, benchmarks, evals, or plugins. A
> first-time playtester completes the whole loop with zero explanation.
> Each screen has one obvious primary action, plain words only, a one-line
> answer to "what do I do next?", and no dead ends. No setup stands between
> a new player and their first win. This is the first principle of this
> spec and it overrides every other consideration here. The
> [IDIOT PROOF checklist](#idiot-proof-checklist) is run against every
> screen and every chat card.

## Contents

- [Build 21: what shipped](#build-21-what-shipped)
- [Build 25: chat first](#build-25-chat-first)
- [Revision 3: what changed](#revision-3-what-changed)
- [Revision 2: what changed](#revision-2-what-changed)
- [Spec ID index](#spec-id-index)
- [Purpose and the loop](#purpose-and-the-loop)
- [Chat in the loop](#chat-in-the-loop)
- [Words on screen](#words-on-screen)
- [Status key and visual style](#status-key-and-visual-style)
- [Screen inventory](#screen-inventory)
- [Screens](#screens)
- [Chat screens](#chat-screens)
- [Chat cards and sheets](#chat-cards-and-sheets)
- [Connect a computer](#connect-a-computer)
- [Desktop app screens](#desktop-app-screens)
- [Intro cinematic](#cin-01-intro-cinematic)
- [User flows](#user-flows)
- [Navigation map](#nav-01-navigation-map)
- [IDIOT PROOF checklist](#idiot-proof-checklist)
- [Minimal v1 cut and later additions](#minimal-v1-cut-and-later-additions)
- [Later: the Gym in the Verse](#later-the-gym-in-the-verse)
- [Appendix: what we cut and why](#appendix-what-we-cut-and-why)
- [Sources](#sources)

## Build 21: what shipped

Build 21 ("Test tools in chat") implements revision 3's v1 cut in the real
app, [#9939](https://github.com/OpenAgentsInc/openagents/issues/9939)
(`59b6908044` to `caf14e1a3b`); OpenAgents Mockup draws all of revision 3
([#9940](https://github.com/OpenAgentsInc/openagents/issues/9940),
`b4c6e5f6ab`). The code is `crates/openagents-mobile/src/first_run.rs`
(the menu and first run), `eval_cards.rs` and `gym.rs` (cards and sheets),
and `hosted.rs` (the hosted runner client). A real test ran end to end from
the phone on 2026-09-29: "Test Project map on Coder", one tap, 2 of 6 → 5 of
6 **Better**, **Add to the Gym**, a second trainer's check, and +25 XP on
the menu and Profile ([simulator record](../../bins/openagents-ios/verification/2026-09-29-evals-in-chat/README.md)).

| ID | Build 21 |
| --- | --- |
| `SCR-01` | Shipped: player card, **GYM OPEN** pill, next-step line, **CHAT WITH OPENAGENTS**, starter chips, **PROFILE**, **THE GYM IN THE VERSE**, footer. Not built: the updates bell (`E02`), the season card (`E09`), and the "testing now" count (`E05`). The tab bar stays. Build 21 opened the Chat tab on the menu; build 25 opens it on the chat ([chat first](#build-25-chat-first)). |
| `SCR-02`, `CIN-01`, `SCR-15.E12` | Shipped: three taps to a test (`CHOOSE CODER`, `LET'S GO`, `START THE TEST`), resuming at the furthest step. `CIN-01` is only its end card; the shots before it (`S01` to `S07`) are not built. Build 25 moves the three taps behind **Train Coder**. |
| `CARD-01` to `CARD-07` | Shipped, with the gaps marked PARTIAL or NEW in each card's table. |
| `SCR-05`, `SCR-06`, `SCR-11`, `SCR-20`, `SCR-21` | Shipped; `SCR-11` is the minimal Profile. |
| `FLOW-01` | Shipped, and passes `CHK-06` live. |
| `FLOW-04` | Shipped: live on the hosted runner. |
| `FLOW-07` | Shipped: the live chat worker drafts a skill-shaped tool to a test set ([#9945](https://github.com/OpenAgentsInc/openagents/issues/9945)); a tool that needs new code goes to Coder. |
| `FLOW-08` | Shipped: **RUN THE CHECK** on `CARD-06`; the live checks so far ran through the hosted runner from a second trainer key. |
| `FLOW-09` | Shipped: news from the Gym's records and our changelog, without ids or banned words ([#9944](https://github.com/OpenAgentsInc/openagents/issues/9944)). Tapping a news item opens nothing yet. |
| `FLOW-10` | Shipped for `eval-check`: the referee's award reached the menu and Profile live. `eval-adopt` works in tests; no adoption has been made. |
| `CHK-01` to `CHK-13` | Re-checked against build 21 in the [simulator record](../../bins/openagents-ios/verification/2026-09-29-evals-in-chat/README.md#idiot-proof-re-checked-against-this-build). |

## Build 25: chat first

The owner's direction on 2026-09-29
([#9958](https://github.com/OpenAgentsInc/openagents/issues/9958)): drop
people immediately into talking with OpenAgents, the auto-upgradable agent.
Chat is pure chat plus the capabilities that are present. The Gym's loop is
a separate flow a person opts into.

- **A new install lands in chat**, with the tab bar. No `SCR-02`, no step
  counter, no "Test Project map on Coder" sent for the player, no Gym
  cards or menu until the person asks. A new chat's suggested questions
  include a few about the Gym (#9963); the `eval.*` routes answer them.
- **The Gym is opt-in: Train Coder**, on the Verse's Gym board (where
  **See the board** and **THE GYM IN THE VERSE** lead) and under Account.
  It opens the intro (`FLOW-01`'s three taps) on the Chat tab; **Not now**
  on step 1 returns to the chat, opted out. The opt-in and the intro's
  furthest step persist with the phone's Gym state.
- **`SCR-01` is the Gym menu**, behind the chat header's **Menu** after the
  intro's first result. **PROFILE** is a button in every chat's header;
  previous chats stay where they were.
- **The word is capability** ([#9957](https://github.com/OpenAgentsInc/openagents/issues/9957)):
  Test a capability, with and without the capability, Coder has this
  capability now. "Tool" joins the banned list as an umbrella word; Project
  map, Code finder, and Test reader keep their names.

## Revision 3: what changed

The owner's direction on 2026-09-28: our plans were too focused on
benchmarks and need to focus on evals; people should use the Gym and evals
through our one routed chat; and the Gym in the Verse is for reviewing
results and, later, for socializing. The engine is specified in
[extension evaluation](../extensions/evaluation.md) (revision 2).

What revision 3 changes:

- **The loop is an eval loop.** "Coder practices 10 tasks and scores 6 of
  10 → 8 of 10" becomes "we run a **test set** for a **tool** with and
  without it: Coder passes 5 of 8 tests without it, 7 of 8 with it".
  `LOOP-1` to `LOOP-6` keep their IDs and positions; their names and
  meanings change as the [loop table](#purpose-and-the-loop) shows.
- **Chat is the entry point.** The main menu's one primary action is
  **CHAT WITH OPENAGENTS** (`SCR-01.E12`). The first run is a guided chat.
  The Gym's pick-a-tool page and training page are retired and replaced by
  chat cards.
- **New chat jobs** `CHAT-9` to `CHAT-14`: what's new in the Gym, make a
  tool and its tests in chat, run a pilot, add to the Gym, check someone's
  result, and see your credit. `CHAT-2` to `CHAT-6` are reworded for
  tests.
- **Chat cards**, a new ID prefix `CARD`: typed cards the app draws inside
  a reply, each with the app's own button (`CARD-01` to `CARD-07`), and two
  sheets (`SCR-20` Add to the Gym, `SCR-21` Test set).
- **Credit** is explicit: XP when another trainer checks your result or
  test set, and when Coder adopts your tool. No money.
- **New flows** `FLOW-07` to `FLOW-10` and checks `CHK-12` and `CHK-13`.
- **The Gym in the Verse** is specified as [later](#later-the-gym-in-the-verse):
  reviewing results, and trainers' agents comparing notes over chat.

ID changes (IDs are never renumbered or reused):

| ID | Change in revision 3 |
| --- | --- |
| `LOOP-1` … `LOOP-6` | Kept; renamed Ask, Pick or make, Run, See the change, Add to the Gym, Earn and return. |
| `CHAT-2` … `CHAT-6` | Kept; reworded from practice tasks to tests and test sets. |
| `CHAT-9` … `CHAT-14` | New. |
| `SCR-01.E03` **ENTER THE GYM** | Retired. The primary is now `SCR-01.E12` **CHAT WITH OPENAGENTS**. |
| `SCR-01.E13`, `SCR-01.E14` | New: starter chips, and **THE GYM IN THE VERSE** row (later). |
| `SCR-03` The Gym: pick a tool | Retired, with all its elements. Replaced by `CARD-01` Tool card in chat. |
| `SCR-04` Training | Retired, with all its elements. Replaced by `CARD-03` Run card. |
| `SCR-05` Result | Kept as the result's detail view, opened from `CARD-04`; its numbers are tests; `SCR-05.E07` now opens `SCR-20`. |
| `SCR-07.E07`, `SCR-09.E05`, `SCR-10.E05`, `SCR-11.E09` | Kept; each now opens chat with the matching request instead of `SCR-03`. |
| `SCR-11.E10` | EXISTS (`xp_ledger::eval::made`) |
| `SCR-15.E12` | EXISTS |
| `SCR-17.E11`, `SCR-17.E12` | `E11` now names the eval offers; `E12` (result line) is retired in favor of `CARD-04`. |
| `SCR-20`, `SCR-21` | New sheets. |
| `CARD-01` … `CARD-07` | New. |
| `CIN-01.S08` | Kept; the end card's button reads **LET'S GO** and opens the first-run chat. |
| `FLOW-01`, `FLOW-02`, `FLOW-04` | Kept; rewritten for the chat-first loop. |
| `FLOW-07` … `FLOW-10` | New. |
| `CHK-12`, `CHK-13` | New. |

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

Every screen, card, sequence, flow, check, and element has a stable ID. Use
the ID in issues and commits, for example `SCR-01.E12` for the **CHAT WITH
OPENAGENTS** button. IDs are stable, not positional: a new element gets the
next free number, and a removed element's ID is retired, never reused.

| Prefix | Meaning | Example |
| --- | --- | --- |
| `LOOP-n` | A step of the loop | `LOOP-3` Run |
| `CHAT-n` | A job chat does in the loop | `CHAT-2` "Which tool should I try?" |
| `SCR-nn` | A screen, sheet, or overlay | `SCR-01` Main menu |
| `SCR-nn.Enn` | An element on a screen | `SCR-01.E12` CHAT WITH OPENAGENTS button |
| `CARD-nn` | A card the app draws inside a chat reply | `CARD-04` Result card |
| `CARD-nn.Enn` | An element on a card | `CARD-04.E05` **Add to the Gym** |
| `CIN-nn` | A cinematic sequence | `CIN-01` Intro cinematic |
| `CIN-nn.Snn` | A shot in a cinematic | `CIN-01.S07` Settle behind Coder |
| `FLOW-nn` | An end-to-end user flow | `FLOW-01` First-time playtester |
| `NAV-nn` | A navigation map | `NAV-01` Navigation map |
| `PAT-nn` | A shared pattern used by many screens | `PAT-01` Offline, error, and empty states |
| `DSK-nn` | A screen of the companion desktop app | `DSK-01` Connect a phone |
| `DSK-nn.Enn` | An element on a desktop app screen | `DSK-01.E01` The QR code |
| `CHK-nn` | An IDIOT PROOF checklist item | `CHK-01` One primary action |

| ID | Name | v1 |
| --- | --- | --- |
| `LOOP-1` … `LOOP-6` | Ask, Pick or make, Run, See the change, Add to the Gym, Earn and return | Yes |
| `CHAT-1` … `CHAT-14` | Chat's jobs in the loop | Yes (`CHAT-1` to `CHAT-5`, `CHAT-7` to `CHAT-12`, `CHAT-14`); `CHAT-6` and `CHAT-13` in the second wave |
| `SCR-01` | Main menu | Yes |
| `SCR-02` | Choose your agent (first run, step 1 of 3) | Yes |
| `CIN-01` | Intro cinematic | Yes (short cut) |
| `SCR-03` | The Gym: pick a tool | Retired in revision 3 (`CARD-01`) |
| `SCR-04` | Training | Retired in revision 3 (`CARD-03`) |
| `SCR-05` | Result (detail) | Yes |
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
| `SCR-20` | Add to the Gym (sheet) | Yes |
| `SCR-21` | Test set (sheet) | Yes |
| `SCR-22` | Connect a computer (the scanner) | Milestone of #9965 |
| `SCR-23` | Connected | Milestone of #9965 |
| `DSK-01` | Desktop: Connect a phone | Milestone of #9965 |
| `DSK-02` | Desktop: Connected | Milestone of #9965 |
| `DSK-03` | Desktop: Home | Milestone of #9965 |
| `DSK-04` | Desktop: A phone nearby wants to connect | After the milestone |
| `DSK-05` | Desktop: Menu bar | After the milestone |
| `CARD-01` | Tool card | Yes |
| `CARD-02` | Test set draft card | Yes |
| `CARD-03` | Run card | Yes |
| `CARD-04` | Result card | Yes |
| `CARD-05` | Gym news card | Yes |
| `CARD-06` | Check card | Yes (once a first result exists to check) |
| `CARD-07` | Credit card | Yes |
| `PAT-01` | Offline, error, and empty states | Yes |
| `FLOW-01` | First-time playtester | Yes |
| `FLOW-02` | Returning playtester, daily loop | Yes |
| `FLOW-03` | Kicking the tires in chat | Yes |
| `FLOW-04` | Test a tool from chat | Yes |
| `FLOW-05` | Dispatching Coder from chat | Yes |
| `FLOW-06` | Wrong answer and Share this chat | Yes |
| `FLOW-07` | Make a tool and its tests in chat | Yes |
| `FLOW-08` | Check another trainer's result | Yes |
| `FLOW-09` | What's new in the Gym | Yes |
| `FLOW-10` | Credit: when your work is used | Yes |
| `NAV-01` | Navigation map | Yes |
| `CHK-01` … `CHK-13` | IDIOT PROOF checklist | Yes |

## Purpose and the loop

We are building the best coding agent in the world by using network
effects: an agent collective. Coder is our first agent. The Verse is where
agents go to connect, communicate, and transact, and where people stay in the
loop while agents are built. The Gym is where people help agents become
better through our plugin system, and **evals** are how the Gym knows a
tool helps: a test set runs with and without the tool, and the change is
the measurement.

The near-term goal of this app is a simple experience that attracts
playtesters, shows them a measured improvement they caused, grows the
playtest cooperative, keeps playtesters coming back, and builds a
following. Every element on every screen serves one loop, and the loop
happens in one place: **Chat with OpenAgents**. Anything that doesn't serve
it is cut ([appendix](#appendix-what-we-cut-and-why)).

```
             +---------------------------------------------------+
             |                                                   |
             v                                                   |
   LOOP-1 ASK ---------> LOOP-2 PICK OR MAKE ---> LOOP-3 RUN     |
   chat with us:          a tool and its          tests with and |
   what's new, what       test set (ours, or      without the    |
   to try, or "help me    one we draft with you)  tool, on our   |
   make a tool"                                   computers      |
                                                       |         |
                                                       v         |
   LOOP-6 EARN <-------- LOOP-5 ADD TO THE GYM <- LOOP-4 SEE     |
   and return: XP when    signed, public,          THE CHANGE    |
   others check it or     checkable                5 of 8 -> 7   |
   Coder adopts the tool                           of 8 tests    |
             |                                                   |
             +---------------------------------------------------+

 Network effect: other trainers check a result by running the same tests.
 When a tool is confirmed to help, Coder adopts it for everyone, and the
 people who made the tool and the tests earn XP with their names on it.
```

| Step | What the player does | What we do |
| --- | --- | --- |
| `LOOP-1` Ask | Chats with OpenAgents: what's new in the Gym, which tool to try, or "help me make a tool". Coder is their agent, chosen for them. | Answers from verified Gym records and our notes, and offers the next step as a tap. Creates the player's identity silently. |
| `LOOP-2` Pick or make | Picks a tool we recommend, or answers a few questions so we draft a tool and its test set with them. | Shows a tool card, or runs the interview and shows the draft as a card. |
| `LOOP-3` Run | Taps **Start the test**. | Runs the test set with the tool and without it, three times each, on our computers. |
| `LOOP-4` See the change | Reads the result card: tests passed with and without the tool, and the verdict. | Decides **Better**, **No clear change**, or **Worse** by the Gym's rule, against the spread between repeats. |
| `LOOP-5` Add to the Gym | Taps **Add to the Gym** and confirms. | Publishes the tests and the result, signed; other trainers can check it. |
| `LOOP-6` Earn and return | Comes back for a check that's waiting, news, or their credit. | Pays XP when another trainer's check confirms their result, and when Coder adopts their tool; says so in chat and on the menu. |

## Chat in the loop

Chat with OpenAgents is where the loop happens. It needs no setup: no
computer, sign-in, or wallet. Every Gym and eval job goes through the one
routed chat: the router (Jev's typed route question, never word matching)
picks the route, reviewed answers and verified records supply the facts,
and the app draws each next step as its own control. Chat serves the loop
in four ways: it **answers** (a prepared answer at once, a grounded answer
from our records, or the model's reply streamed), it **shows** a card (a
tool, a draft, a run, a result, news, a check, or credit), it **offers**
the next step as a tap, and it **dispatches** Coder to a connected
computer for real work. Chat never acts on its own: every offer does
nothing until the player taps it, and the tap's meaning is the app's,
never the model's words.

| ID | The player asks | Chat answers with | Loop | Status |
| --- | --- | --- | --- | --- |
| `CHAT-1` | "Who are you?", "What model is this?", "What does it cost?", "What can you do?" (kicking the tires) | A prepared answer at once (under a second), marked "Prepared answer", with follow-up chips to the next likely question. | `LOOP-1` | EXISTS (`meta.*` and `smalltalk.*` in `chat-answers-v1`) |
| `CHAT-2` | "Which tool should I try?", "What should I do next?" | One recommendation in a sentence ("Try Project map. 18 trainers tested it; most saw Coder pass more tests.") and `CARD-01` for that tool with **Start the test**. | `LOOP-2` | EXISTS (`eval.run`, with `CARD-01` and **START THE TEST** for the starter test sets) |
| `CHAT-3` | "What does Project map do?", "What's a tool?", "What's a test?" | A plain answer from our notes, and `CARD-01` when a tool is named. | `LOOP-2` | PARTIAL (`product.kb` answers from 60 sourced notes, including one for each tool and for tests; `CARD-01` comes with `CHAT-4`, not with a note's answer) |
| `CHAT-4` | "Test Project map on Coder" | "We'll run 8 tests with Project map and without it. It takes about 5 minutes." and `CARD-01` with **Start the test**; the run starts only on that tap. | `LOOP-3` | EXISTS (the hosted runner; run live on 2026-09-29) |
| `CHAT-5` | "How did my test do?", "Did Coder get better?" | `CARD-04` from the player's own result ("5 of 8 → 7 of 8 tests with Project map. Better.") with **See details** and **Add to the Gym**. | `LOOP-4` | EXISTS (`eval.result`; the phone draws its own latest result) |
| `CHAT-6` | "How do I earn XP?", "What level am I?" | A prepared or product answer; "Level 2, 93 XP to level 3" from the player's card, and `CARD-07` when they have credit. | `LOOP-6` | EXISTS (`eval.credit` and the XP notes; `CARD-07` from the phone's ledger) |
| `CHAT-7` | "Fix the failing test in my repo", "Look through this project" | "We'll dispatch Coder to …" and **Run Coder on** the computer, or **Connect a computer** when none is added. Coder's reply streams into its own chat (`SCR-19`). | Beyond the loop (your own code) | EXISTS (`work.dispatch`; `SCR-17.E05`) |
| `CHAT-8` | "Where's my wallet?", "How do I report a bug?", "Which computers are online?" | A short answer and a screen chip (**Open Wallet**, **Your computers**, **Identity keys**, **Playtest**, **Report a problem**) or a read-only command card. | Support | EXISTS on the phone; the worker's command proposals are PARTIAL |
| `CHAT-9` | "What's new in the Gym?", "What are people working on?", "What's the latest?" | A grounded answer from the Gym's records (new results, test sets, checks waiting, tools Coder adopted) and our changelog, as `CARD-05`, every item with its source; at most one offer. | `LOOP-1` | EXISTS (`gym.news` from the Gym's records) |
| `CHAT-10` | "Help me make a tool that …", "Write tests for my tool" | The interview, one question per turn: what the tool is for, what a good run looks like, then a draft as `CARD-02` with **Looks good** and **Change it**. | `LOOP-2` | EXISTS (`eval.author` and the authoring interview) |
| `CHAT-11` | "Try it once", "Run my tests" | `CARD-03` for a one-run pilot, then `CARD-04`; the full run needs its own tap. | `LOOP-3`, `LOOP-4` | EXISTS |
| `CHAT-12` | "Add it to the Gym" | **Add to the Gym**, which opens `SCR-20` to confirm what becomes public. | `LOOP-5` | EXISTS |
| `CHAT-13` | "Find me a result to check" (the **Check a result** chip), "Is there a result I can check?" | `CARD-06` with **Run the check**. | `LOOP-5`, `LOOP-6` | EXISTS (`eval.check` with `CARD-06`) |
| `CHAT-14` | "What have I earned?", "Did anyone check my tests?" | `CARD-07`: XP pending and confirmed, who checked what, and whether Coder adopted the tool. | `LOOP-6` | EXISTS (`CARD-07` from the phone's ledger) |

Rules chat keeps in the loop:

1. **Chat never runs, spends, or publishes by itself.** It shows the card
   with the choice filled in, and the player taps its one button. A run and
   a publish each take their own tap; publishing also confirms on `SCR-20`.
2. **Numbers come from records.** Every score, count, and news item comes
   from a verified record the router read, and cites it. The model never
   states a result it wasn't given; with no record, chat says so.
3. **The draft is the player's.** A test set made in chat stays on the
   phone as a draft until **Add to the Gym**. Every interview step is a
   tap.
4. **The first run is a guided chat.** `FLOW-01` puts the player in a chat
   with the first tool card ready; there is no separate Gym page to find.
5. **Chat speaks as "we".** It is OpenAgents answering; Coder is the agent
   we dispatch and the agent we test.
6. **Chat answers the question asked.** A prepared answer shows only when
   the router is sure; otherwise the model answers. A wrong prepared answer
   is one tap to report (`SCR-18`).

## Words on screen

Primary surfaces use plain words only. The internal term stays in code,
docs, and the Advanced section of Profile.

| On screen | Internal term | Why |
| --- | --- | --- |
| **Coder** ("an AI that writes code") | Coder, the agent | A name plus one plain line. |
| **Capability** | What a person adds to Coder: a program, a plugin (a Wasm guest), a skill, or a knowledge entry, shipped in an extension package ([#9957](https://github.com/OpenAgentsInc/openagents/issues/9957), decided 2026-09-29) | The video's word and the app's word are the same word: a capability is what a claim establishes Coder can do. "Tool" is retired as the umbrella (it survives only for a model's own tool call in a transcript); "plugin" and "extension" are jargon. |
| **Project map**, **Code finder**, **Test reader** | `repo_map`, `code_search`, `test_report` evidence guests | Says what the capability does. Names stay names: Project map is a capability, never "a tool". |
| **Test** | A case in an extension eval suite | Everyone knows what a test is. "Eval" and "case" are jargon. |
| **Test set** | A suite | A set of tests. |
| **With the capability**, **without it** | The subject and baseline arms | Says what's being compared. |
| **Passes 7 of 8 tests** | Cases passed in the subject arm | Whole numbers, whole denominator. |
| **Better**, **No clear change**, **Worse** | The `ext-eval-v2` gate: keep (more tests passed, beyond the spread between repeats, at a cost and time not materially worse), inconclusive, reject. Faster or cheaper is a note beside the verdict, never Better on its own | Plain verdict. |
| **Try it once** | A one-run pilot | Says it's a quick try. |
| **Check a result** | A rerun of a published test set by another trainer | Says what you do. |
| **Add to the Gym** | Publish the suite and the signed result (NIP-EXT, NIP-EVAL) | Says where it goes. |
| **Coder has this capability now** | Adoption into Coder defaults | Says what happened. |
| **Train Coder** | Opt into the Gym: the intro (`FLOW-01`), then the Gym's starters, cards, and menu | Says what you're signing up for. Until it's tapped, the chat volunteers nothing of the Gym. |
| **Trainer 7KQ** | The Verse world key's public key | A name, not a key. |
| **XP**, **Level 2** | NIP-XP awards on `trainer-curve-v1` | Game words most people know. |
| **Save your progress** | Back up the secret key | Says why, not how. |
| **OpenAgents** ("we") | The chat worker answering a NIP-CJ job | The app and the chat are one voice. |
| **Chat with OpenAgents**, **Message OpenAgents** | The chat | Says who you're talking to. |
| **Run Coder on Studio Mac** | NIP-HOST `task.create` with the conversation | Names the agent and the computer. |
| **Connect a computer** | Enroll a host by scanning its NIP-HOST connect code (`SCR-22`, from a chat chip or Account > Computers) | Says what you do. |
| **Scan with the OpenAgents app on your phone** | The desktop app's QR code, an `openagents-connect:` host invitation | Names the one app and the one action. |
| **Copy a code**, **Paste a code** | The connect code's text form, a bearer secret for up to two minutes | A code is something you copy; nothing more is said. |
| **Phone**, **Kai's iPhone** | A device key holding a host grant, with its label | Says what it is. |
| **Project** | A workspace label on a computer | "Workspace" is jargon on the desktop app. |
| **Remove** | `device.revoke` | Says what happens to the phone's access. |
| **OpenAgents** (the Mac app) | The companion desktop app, which runs the resident host | The same name as the phone app, so "the OpenAgents app" names one thing on each device. |
| **Cloud** | No computer: the chat worker and the hosted runner | The one place a message goes with no setup. |
| **Prepared answer** | A T0 bank answer (`bank:chat-answers-v1`) | Honest about where the words came from. |
| **Wrong answer** | A wrong-answer playtest report | Says what you're telling us. |

Banned on primary surfaces: npub, nsec, key, relay, Nostr, NIP, ATIF,
tailnet, Tailscale, Wasm, plugin, extension, tool (as the umbrella word;
a model's tool call in a transcript keeps its name), benchmark,
Terminal-Bench, TB,
eval, evaluation, suite, case, grader, rubric, judge, baseline, arm,
harness, stand-in, mock, pilot, Jev, Luna, Microcoder, verifier, trace,
recipe, grant, sats, BTC, ₿, Lightning, invoice, host, workspace, pubkey,
hex.

One exception, owner-approved on 2026-09-28: a chat **answer** may name
Gemini, the AI Gateway, Jev, or Nostr when the player asks what powers the
chat or how it works, and may say "eval" when the player uses the word
first. The labels, buttons, chips, cards, and notes around the answer stay
plain. "Workspace" appears only on a computer-backed chat, which a player
reaches only after connecting a computer.

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
| `SCR-01` | Main menu | All steps | App open (returning), end of every flow | **CHAT WITH OPENAGENTS** |
| `SCR-02` | Choose your agent | `LOOP-1` | App open (first time) | **CHOOSE CODER** |
| `CIN-01` | Intro cinematic | `LOOP-1` | `SCR-02` | **LET'S GO** (at the end) |
| `SCR-05` | Result (detail) | `LOOP-4`, `LOOP-5` | `CARD-04.E04` | **ADD TO THE GYM** |
| `SCR-06` | Level up | `LOOP-6` | `SCR-20` | **NICE** (back to the chat or menu) |
| `SCR-07` | Coder: your agent | `LOOP-4` | `SCR-01` | **TEST A TOOL** |
| `SCR-08` | All tools | `LOOP-2` | `SCR-07` | Tap a tool |
| `SCR-09` | Tool detail | `LOOP-2` | `SCR-08`, `CARD-01.E05` | **TEST THIS TOOL** |
| `SCR-10` | Rankings | `LOOP-6` | `SCR-01` | **CLIMB: CHAT WITH OPENAGENTS** |
| `SCR-11` | Profile | `LOOP-6` | `SCR-01` | **CHAT WITH OPENAGENTS** |
| `SCR-12` | Updates | `LOOP-6` | `SCR-01` bell | The top update's button |
| `SCR-13` | Report a problem | Playtest | `SCR-11`, long press anywhere | **SEND** |
| `SCR-14` | Just-in-time setup prompt | `LOOP-5`, `LOOP-6` | Triggered | The one setup step |
| `SCR-15` | Chat: new chat | `CHAT-1` to `CHAT-14` | `SCR-01.E12`, `SCR-01.E13`, `SCR-02.E06`, `CIN-01`, `SCR-16` | Send a message (the composer), or the first-run card |
| `SCR-16` | Chat: previous chats | All chat | `SCR-15` ☰, `SCR-17` ☰ | Tap a chat |
| `SCR-17` | Chat: a conversation | `CHAT-1` to `CHAT-14` | `SCR-15` (after Send), `SCR-16` | The card's or offer's button, else the composer |
| `SCR-18` | Chat: Wrong answer | `CHAT-1` (quality) | `SCR-17.E09` | **Send** |
| `SCR-19` | Chat: Coder on a computer | `CHAT-7` | `SCR-17.E05`, `SCR-16`, `SCR-15` with a computer chosen | Send a follow-up |
| `SCR-20` | Add to the Gym | `LOOP-5` | `CARD-04.E05`, `SCR-05.E07` | **ADD TO THE GYM** |
| `SCR-21` | Test set | `LOOP-2` | `CARD-02.E04`, `CARD-01.E06`, `CARD-04.E06` | **DONE** (read) or **LOOKS GOOD** (a draft) |
| `SCR-22` | Connect a computer | `CHAT-7` | `SCR-17.E05`, `SCR-17.E06`, `SCR-15.E04`, `SCR-14.E05`, Account > Computers | Point the camera at the code |
| `SCR-23` | Connected | `CHAT-7` | `SCR-22` after a scan | **DONE** |
| `CARD-01` … `CARD-07` | Chat cards | `LOOP-1` to `LOOP-6` | A reply in `SCR-17` | The card's one button |
| `PAT-01` | Offline, error, and empty states | All | Any screen | **TRY AGAIN** or the named next step |

## Screens

Each screen has a wireframe (top to bottom, exact labels), its states, an
element table, transitions, and an IDIOT PROOF check line. The wireframe
shows the primary action as `[#### LABEL ####]` and secondary actions as
`[ label ]`. Examples such as `38` or `7KQ` are sample data.

### SCR-01 The Gym menu

The Gym's hub, for a player who opted in with **Train Coder**. Its one
primary action is chat, because the loop happens in chat. Since build 25
(chat first, [#9958](https://github.com/OpenAgentsInc/openagents/issues/9958))
it is not the Chat tab's first screen: the tab opens on the chat, and the
menu is behind the chat header's **Menu** once the intro's first result is
in. **PROFILE** is also a button in every chat's header.

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
|   ( GYM OPEN · 38 people testing now )   |
|                                          |
|------------------------------------------|
| Next: a check is waiting for you (+50 XP)|  E11
|##########################################|
|# [message] CHAT WITH OPENAGENTS       > #|  E12 (primary)
|#   Test a tool, see what's new, earn XP #|
|##########################################|
| (dumbbell) Test a tool  (news) What's new|  E13 (starter chips)
| (check) Check a result                   |
| [spade] CODER                          > |  E06 (later)
|   Passes 7 of 8 starter tests            |
| [podium] RANKINGS                      > |  E07 (later)
|   You're #41 this week                   |
| [person] PROFILE                       > |  E08
|   Level, XP, what you made               |
| [globe] THE GYM IN THE VERSE           > |  E14 (later)
|   See every result on the boards         |
|------------------------------------------|
| PLAYTEST SEASON 1 · ENDS OCT 26          |  E09
| Help make Coder better. Every test counts|
|------------------------------------------|
| ● Gym open            v1.0.0 · Playtest  |  E10
+------------------------------------------+
```

States:

- **Loading:** the layout draws at once from the phone's cache; numbers that
  are still loading show a gray bar, never `0`.
- **Empty (no results yet):** cannot happen, because `FLOW-01` ends here
  only after the intro's first result. Before **Train Coder**, and while the
  intro is unfinished, `SCR-01` is not shown; the Chat tab is the chat (or
  the intro's step).
- **A check is waiting:** `E11` reads "Next: a check is waiting for you
  (+50 XP)." and the **Check a result** chip is first.
- **Your result was checked, or Coder adopted your tool:** `E11` says so
  ("Trainer 2PX confirmed your result. +25 XP."), and the primary opens a
  chat with `CARD-07` ready.
- **No runs left today:** `E11` reads "New runs at 9:00 tomorrow. You can
  still ask what's new or check results."
- **Offline or error:** see `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-01.E01` | Logo and **OPENAGENTS** wordmark | Identity; no action. | — | PARTIAL (the Chat tab header says "OpenAgents") |
| `SCR-01.E02` | Updates bell with a count | Opens `SCR-12`. The count is unread updates, such as "Your result was confirmed." | `LOOP-6` | NEW (not built in build 21) |
| `SCR-01.E03` | ~~**ENTER THE GYM**~~ | Retired in revision 3. `E12` is the primary. | — | Retired |
| `SCR-01.E04` | Player card: avatar, trainer name, level, XP bar | Shows progress at a glance. Tapping opens `SCR-11`. | `LOOP-6` | EXISTS (name, level, and XP bar from the phone's XP ledger; a gray bar until it is read) |
| `SCR-01.E05` | Hero image with Gym status pill | Shows the Gym is open and how many people are testing now. | `LOOP-6` (network) | PARTIAL (the **GYM OPEN** pill; the "testing now" count is not built, because no record says it) |
| `SCR-01.E06` | **CODER** row | Opens `SCR-07` with Coder's starter test result. | `LOOP-4` | NEW (later) |
| `SCR-01.E07` | **RANKINGS** row | Opens `SCR-10`. | `LOOP-6` | NEW (later) |
| `SCR-01.E08` | **PROFILE** row | Opens `SCR-11`. | `LOOP-6` | EXISTS (opens a minimal `SCR-11`) |
| `SCR-01.E09` | Season card | States the season and its end date. Tapping opens `SCR-10` (or `SCR-11` in v1). | `LOOP-6` | NEW (not built in build 21) |
| `SCR-01.E10` | Footer: Gym status, version | Status and build for reports. | — | EXISTS ("Gym open · v1.0.0 (21) · Playtest") |
| `SCR-01.E11` | Next-step line | One line that says what to do next, from the player's state (a check waiting, a result confirmed, runs left). | All | EXISTS (a run in progress, a result not yet added, your work checked, or the first step; no runs-left line) |
| `SCR-01.E12` | **CHAT WITH OPENAGENTS** | The primary. Opens `SCR-15`, a new chat ready to type. Subtitle: "Test a tool, see what's new, earn XP". With a pending card (a check waiting, credit to see, a run in progress), it opens that chat with the card on top. | `CHAT-1` to `CHAT-14` | EXISTS (with a result not yet added, it reopens that chat) |
| `SCR-01.E13` | Starter chips: **Test a tool**, **What's new**, **Check a result** | Each opens a new chat and sends that message, so the answer and its card are the first thing the player sees. Outlined, never filled. | `CHAT-2`, `CHAT-9`, `CHAT-13` | EXISTS |
| `SCR-01.E14` | **THE GYM IN THE VERSE** row | Opens the Verse at the Gym building to review results on its boards. Later: the social Gym ([later](#later-the-gym-in-the-verse)). | `LOOP-6` | EXISTS (opens the Verse's Gym and its EVALS board, [#9942](https://github.com/OpenAgentsInc/openagents/issues/9942); the social Gym is later) |

Transitions: in from app open (returning player), `SCR-06`, and every
**Back to menu**. Out to `SCR-15` (or `SCR-17` with a card), `SCR-07`,
`SCR-10`, `SCR-11`, `SCR-12`, and the Verse.

IDIOT PROOF check: **pass.** One filled button, the next step in one line,
starter chips for someone with no idea what to type, no jargon, progress
visible on the card.

### SCR-02 Choose your agent

The Gym intro, step 1 of 3. Shown after **Train Coder** (build 25); before
that, a new install opens on the chat.

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
| | Today: passes 5 of 8 starter tests   | |
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
  the app bundle. The offline notice appears on the first-run card, where
  it matters.
- **Error:** none possible on this screen; identity is created on the phone.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-02.E01` | Step indicator | Shows step 1 of 3. | `LOOP-1` | EXISTS |
| `SCR-02.E02` | Title and two plain lines | Explains an agent in one sentence. | `LOOP-1` | EXISTS |
| `SCR-02.E03` | Coder card, preselected | The default and only agent in v1. Shows how many of our starter tests Coder passes today, so the player sees what a tool could improve. | `LOOP-1`, `LOOP-4` | PARTIAL (the Coder card, preselected; Coder's starter test result is not shown) |
| `SCR-02.E04` | **CHOOSE CODER** | Creates the player's identity silently (a world key, no prompt) and plays `CIN-01`. Tap 1 of 3 to the first run. | `LOOP-1` | EXISTS |
| `SCR-02.E05` | Next-step line | "Next: choose Coder to begin." | `LOOP-1` | EXISTS |
| `SCR-02.E06` | **Ask OpenAgents a question first** | Gray link for the player who wants to kick the tires. Opens `SCR-15` with first-time suggestion chips ("Who are you?", "What is Coder?", "What's the Gym?"); its **< Back** returns here. | `CHAT-1` | EXISTS |

Transitions: in from the first app open only. Out to `CIN-01`, or to
`SCR-15` and back. There is no back button; there is nothing before this
step.

IDIOT PROOF check: **pass.** One agent, already selected, one button. No
sign-in, name, key, or wallet. The chat link is gray and returns to the same
step.

### SCR-03 The Gym: pick a tool (retired)

Retired in revision 3. Picking a tool happens in chat: `CARD-01` Tool card
shows the recommended tool with **Start the test**, and a chip under it
offers the others. The IDs `SCR-03` and `SCR-03.E01` to `SCR-03.E11` are
retired and never reused. What they did moved here:

| Retired | Now |
| --- | --- |
| `SCR-03.E03` explanation, `E07` time and cost | `CARD-01.E03`, `CARD-01.E04` |
| `SCR-03.E04` recommended tool, `E05` alternates | `CARD-01` for the recommended tool; `CARD-01.E07` chips for the others |
| `SCR-03.E06` See all tools | `SCR-08` from `SCR-07` (later) |
| `SCR-03.E09` START TRAINING | `CARD-01.E05` **Start the test** |
| `SCR-03.E10` runs left today | `CARD-01.E04` |
| `SCR-03.E11` check a result card | `CARD-06` |

### SCR-04 Training (retired)

Retired in revision 3. A run shows as `CARD-03` Run card in the chat that
started it, and as the `SCR-01.E12` subtitle while it runs. The IDs
`SCR-04` and `SCR-04.E01` to `SCR-04.E09` are retired: the progress blocks
and counter are `CARD-03.E02` and `E03`, the reassurance line is
`CARD-03.E04`, and **SEE THE RESULT** is the `CARD-04` that replaces the run
card when it finishes. "Ask while you wait" needs no link: the player is
already in the chat.

### SCR-05 Result (detail)

The result's full view, opened from **See details** on `CARD-04`. The card
in chat already shows the headline; this screen shows each test.

```
+------------------------------------------+
| [< Chat]   YOUR RESULT                   |  E01
|------------------------------------------|
|           CODER GOT BETTER               |  E02
|                                          |
|   without the tool     with Project map  |  E03
|      5 of 8     -->      7 of 8 tests    |
|                                          |
|  TESTS                                   |  E12
|  ✓ ✓  Find where login is handled        |
|  ✗ ✓  Add a test for the date parser     |
|  ✗ ✓  Explain the build setup            |
|  ✓ ✓  Leave a one-line fix alone  (tool  |
|        should stay out of the way)       |
|  [ See the whole test set ]              |  E13
|                                          |
|  +50 XP when another trainer checks it   |  E04
|  [######----]  Level 2 · 140/283 XP      |
|                                          |
| When other trainers confirm it, Coder    |  E05
| can use Project map for everyone.        |
|                                          |
| Next: add your result to the Gym.        |  E06
|##########################################|
|#           ADD TO THE GYM               #|  E07 (primary)
|##########################################|
| [ Share outside the app ]                |  E09
| [ Test another tool ]                    |  E10
| [ Ask about this result ]                |  E11
+------------------------------------------+
```

States:

- **Better:** as drawn. The headline is **CODER GOT BETTER**.
- **No clear change:** headline **NO CLEAR CHANGE**, and E05 reads "That's
  useful too. Now everyone knows this tool doesn't help on these tests."
  The primary stays **ADD TO THE GYM**.
- **Worse:** headline **CODER DID WORSE WITH THIS TOOL**, same primary, and
  E05 reads "That's useful too. We won't give Coder this tool."
- **A one-run try:** headline **FIRST TRY**, the counts, and E05 reads
  "One run is a first look, not a result. Run the full test set to add it
  to the Gym." The primary is **RUN THE FULL TEST SET**, back in chat.
- **After adding:** the primary becomes a check-mark row, "Added to the
  Gym", and the next primary is **BACK TO CHAT**.
- **A check result:** headline **YOU CONFIRMED IT** or **IT DIDN'T HOLD
  UP**, and the same primary.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-05.E01` | Title and **< Chat** | Back to the chat; the result is kept on the phone. | — | EXISTS (**BACK TO CHAT**) |
| `SCR-05.E02` | Verdict headline | **Better**, **No clear change**, or **Worse**, from the Gym's rule against the spread between repeats. | `LOOP-4` | EXISTS (the report's own `ext-eval-v2` verdict) |
| `SCR-05.E03` | Tests passed without and with the tool | Whole numbers with whole denominators. | `LOOP-4` | EXISTS |
| `SCR-05.E04` | XP line and level bar | Says when XP comes: when another trainer checks it. | `LOOP-6` | PARTIAL (the XP line; no level bar) |
| `SCR-05.E05` | Why it matters | One sentence on the network effect. | `LOOP-5` | EXISTS |
| `SCR-05.E06` | Next-step line | One line. | `LOOP-5` | EXISTS |
| `SCR-05.E07` | **ADD TO THE GYM** | Opens `SCR-20` to confirm. | `LOOP-5` | EXISTS |
| `SCR-05.E08` | ~~Public notice~~ | Retired in revision 3: `SCR-20` says what becomes public before the tap. | — | Retired |
| `SCR-05.E09` | **Share outside the app** | Opens the system share sheet with an image card and a link. | `LOOP-5` (following) | EXISTS (share text with the numbers) |
| `SCR-05.E10` | **Test another tool** | Back to chat with "Which tool should I try?" sent. | `LOOP-2` | EXISTS |
| `SCR-05.E11` | **Ask about this result** | Back to chat with "Why did Coder do better with Project map?" sent, the result as context. | `CHAT-5` | EXISTS |
| `SCR-05.E12` | Test list | Each test with a mark per arm (without, with), and a note on tests where the tool should stay out of the way. | `LOOP-4` | EXISTS |
| `SCR-05.E13` | **See the whole test set** | Opens `SCR-21`. | `LOOP-2` | EXISTS |

Transitions: in from `CARD-04.E04`. Out to `SCR-20`, the chat, or the share
sheet.

IDIOT PROOF check: **pass.** The before and after are the biggest thing on
the screen. Every verdict, including "worse", has the same next step.

### SCR-06 Level up

An overlay when XP crosses a level: after **Add to the Gym** (`SCR-20`), or when chat shows a new award (`CARD-07`).

```
+------------------------------------------+
|                                          |
|              LEVEL UP                    |  E01
|                 3                        |
|      [######################] 283 XP     |  E02
|                                          |
|   New: PLAYTESTER title on your name     |  E03
|                                          |
| Next: check someone's result for more XP.|  E04
|##########################################|
|#               NICE                     #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

States: shown once per level. If a title isn't new, E03 is hidden. No error
state: it reads local numbers.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-06.E01` | Level number | The new level. | `LOOP-5` | EXISTS |
| `SCR-06.E02` | Full XP bar | Progress made visible. | `LOOP-5` | EXISTS |
| `SCR-06.E03` | New title or reward | Names the loot. | `LOOP-5` | EXISTS |
| `SCR-06.E04` | Next-step line | Gives the reason to return. | `LOOP-6` | EXISTS |
| `SCR-06.E05` | **NICE** | Closes, back to the chat (or `SCR-01` at the end of the first run). | `LOOP-6` | EXISTS |

Transitions: in from `SCR-20` or a chat card. Out to the chat or `SCR-01`.

IDIOT PROOF check: **pass.** One button, no choices.

### SCR-07 Coder: your agent

Later. Shows the collective's agent and how it got here.

```
+------------------------------------------+
| [< Menu]   CODER                         |
|------------------------------------------|
| [Coder figure]                           |  E01
| Starter tests: passes 7 of 8             |  E02
| [chart: tests passed by week, 5 → 6 → 7] |  E03
|                                          |
| TOOLS CODER USES NOW                     |  E04
|  [map] Project map · confirmed by 6      |
|        trainers · added Oct 3            |
| BEING TESTED                             |  E05
|  [search] Code finder · 2 checks so far  |
|                                          |
| Next: test a tool to help Coder pass more|  E06
|##########################################|
|#            TEST A TOOL                 #|  E07 (primary)
|##########################################|
+------------------------------------------+
```

States: loading shows gray bars; empty "Being tested" says "Nothing is being
tested. Be the first." Offline and error use `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-07.E01` | Coder figure | Identity. | — | NEW |
| `SCR-07.E02` | Starter tests passed | How many of our starter tests Coder passes with the tools it uses now. | `LOOP-4` | NEW |
| `SCR-07.E03` | Chart | Starter tests passed over time. | `LOOP-4` | NEW |
| `SCR-07.E04` | Tools Coder uses now | Tools adopted into Coder's defaults, with who made and checked them. | `LOOP-6` (network) | NEW |
| `SCR-07.E05` | Being tested | Tools with checks still open. Tapping one opens `SCR-09`. | `LOOP-2` | NEW |
| `SCR-07.E06` | Next-step line | One line. | — | NEW |
| `SCR-07.E07` | **TEST A TOOL** | Opens chat with "Which tool should I try?" sent (`CHAT-2`). Before revision 3, it opened `SCR-03`. | `LOOP-2` | NEW |

Transitions: in from `SCR-01.E06`. Out to chat, `SCR-09`, `SCR-01`.

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

Transitions: in from `SCR-07`. Out to `SCR-09`, `SCR-07`.

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
|   5 of 8  -->  7 of 8 tests              |
|   18 trainers · 41 runs · 6 checks       |
| It only reads the project. It can't      |  E03
| change files or go online.               |
|                                          |
| Next: test this tool on Coder.           |  E04
|##########################################|
|#         TEST THIS TOOL                 #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-09.E01` | What it does | One plain sentence. | `LOOP-2` | NEW |
| `SCR-09.E02` | Results so far | The newest checked result on its test set, with how many trainers ran and checked it. Results from different test sets are never pooled. | `LOOP-4`, `LOOP-5` | NEW |
| `SCR-09.E03` | Safety line | Plain words for the `SnapshotRead` access mode. | `LOOP-2` | PARTIAL (access modes exist in `crates/plugin`) |
| `SCR-09.E04` | Next-step line | One line. | — | NEW |
| `SCR-09.E05` | **TEST THIS TOOL** | Opens chat with `CARD-01` for this tool. Before revision 3, it opened `SCR-03`. | `LOOP-3` | NEW |
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
| Next: one check could pass #40.          |  E04
|##########################################|
|#     CLIMB: CHAT WITH OPENAGENTS        #|  E05 (primary)
|##########################################|
+------------------------------------------+
```

States: empty week says "The week just started. Test a tool first to top
the list." Offline and error use `PAT-01`.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-10.E01` | Week and season toggle | Two plain choices. | `LOOP-6` | NEW |
| `SCR-10.E02` | Ranked trainers | Only trainers who chose to show their level. | `LOOP-5` | PARTIAL (NIP-XP ledgers and the show-my-level profile exist; no ranking) |
| `SCR-10.E03` | Your row, pinned | Shows the gap to the next rank. | `LOOP-6` | NEW |
| `SCR-10.E04` | Next-step line | One line. | — | NEW |
| `SCR-10.E05` | **CLIMB: CHAT WITH OPENAGENTS** | Opens chat with "Is there a result I can check?" sent. Before revision 3, it opened `SCR-03`. | `LOOP-6` | NEW |

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
| YOUR RESULTS                             |  E04
|  Project map   5 → 7 of 8  Better        |
|  Code finder   5 → 5 of 8  No change     |
| WHAT YOU MADE                            |  E10
|  Changelog helper · Coder uses it +200 XP|
|  Test reader tests · 2 checks   +50 XP   |
|                                          |
| [ Show my level to others   (on) ]       |  E05
| [ Report a problem ]                     |  E06
| [ Advanced ]  (later)                    |  E07
|                                          |
| Next: check a result to reach level 3.   |  E08
|##########################################|
|#        CHAT WITH OPENAGENTS            #|  E09 (primary)
|##########################################|
+------------------------------------------+
```

States: no results yet (only if the first run failed) shows "No results
yet. Your first test takes about 5 minutes." Offline shows cached numbers marked
"Last updated 10:42".

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-11.E01` | Avatar and trainer name | Identity without a key. | `LOOP-5` | EXISTS |
| `SCR-11.E02` | Level and XP | Progress. | `LOOP-5` | EXISTS (Profile and Account > Trainer) |
| `SCR-11.E03` | Titles | Loot earned. | `LOOP-5` | EXISTS (Trainer and Playtest cards) |
| `SCR-11.E04` | Your results | Each tool you tested: tests passed without and with it, and the verdict. Tapping one opens its `SCR-05`. | `LOOP-4` | EXISTS |
| `SCR-11.E05` | **Show my level to others** | A plain switch for the trainer profile. Confirms "Show your level to everyone?" once. | `LOOP-5` | EXISTS (Account > Trainer; not on the minimal Profile) |
| `SCR-11.E06` | **Report a problem** | Opens `SCR-13`. | Playtest | EXISTS (Account; not on the minimal Profile) |
| `SCR-11.E07` | **Advanced** | Later: the player's key, linking keys, and export, in technical words, behind one screen; also **Your computers** and the **Wallet**, the screens a chat offer can open (`CHAT-8`). Computers keeps listing and removing computers, and its **Connect a computer** row opens `SCR-22`. | — | EXISTS (Identity keys, Link a key, Export card, Computers, Wallet; today as Account rows and the Wallet tab); the Computers row to `SCR-22` is NEW |
| `SCR-11.E08` | Next-step line | One line. | — | EXISTS |
| `SCR-11.E09` | **CHAT WITH OPENAGENTS** | Opens chat. Before revision 3, it opened `SCR-03`. | `LOOP-1` | EXISTS |
| `SCR-11.E10` | **What you made** | Your tools and test sets, with the checks and adoptions that earned XP; the same as `CARD-07`. | `LOOP-6` | NEW |

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
| ● Coder now uses your tool for everyone. |
|   +200 XP.                 [ SEE IT > ]  |
| ○ A check is waiting for you.            |
|                            [ CHECK > ]   |
+------------------------------------------+
```

States: empty says "Nothing new. Your next update comes when someone checks
your result." with **CHAT WITH OPENAGENTS**.

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-12.E01` | Update rows, each with one button | Each update opens the chat card it's about (`CARD-06`, `CARD-07`). The newest row's button is the primary. | `LOOP-6` | NEW (no notification system exists) |

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
| `SCR-14.E05` | Asking chat for work on your own code, or choosing to train Coder on it (later) | "To work on your own code, connect your computer." | **CONNECT MY COMPUTER** | PARTIAL (chat already does this just in time: the **Connect a computer** chip shows only when the router says a message needs a computer, and opens Computers today and `SCR-22` once #9971 lands; the sheet form is NEW) |
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
| Tests need the internet.                 |  E02 (what it means)
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

- **Every new chat:** up to four suggested questions, the first of an
  ordered list of ten not yet used on this phone: "Who are you?", "What
  can you do?", "What's new in the Gym?", "Test a capability", "What model
  is this?", "How do I earn XP?", "Check a result", "How do I connect a
  computer?", "Are you open source?", "What does it cost?". Each is
  answered at once. A suggestion tapped, or the same words typed, never
  shows again, nor does a follow-up chip once used; with all ten used,
  no chips show (owner direction, 2026-09-29,
  [#9963](https://github.com/OpenAgentsInc/openagents/issues/9963)).
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
| `SCR-15.E04` | Target choices | Each computer this phone may use, **Cloud**, and **Connect a computer** (opens `SCR-22`), with a check on the current one. | `CHAT-7` | EXISTS (opens Computers today; `SCR-22` is NEW) |
| `SCR-15.E05` | Welcome lines | Two plain lines that say what chat is for, shown only while the chat is empty. The empty screen's "Next:" answer (`CHK-03`). | `CHAT-1` | EXISTS (one line) |
| `SCR-15.E06` | Suggestion chips | Up to four questions to send, on every new chat, from the ordered list above, never one already used on this phone; the worker may rank them once each time the tab shows. | `CHAT-1`, `CHAT-2`, `CHAT-9`, `CHAT-13` | EXISTS |
| `SCR-15.E07` | Composer **Message OpenAgents** | Focused on open; grows to six lines. A tap outside any text field puts the keyboard away. | All chat | EXISTS |
| `SCR-15.E08` | Send | Sends a NIP-CJ job to the chat worker (or a Coder task to the chosen computer) and opens `SCR-17`. | All chat | EXISTS |
| `SCR-15.E09` | Wait and limit lines | Plain lines for a busy or used-up chat, spoken as "we" (rule 3 of [Chat in the loop](#chat-in-the-loop)): "We've answered all the messages we can for you today…" | — | EXISTS |
| `SCR-15.E10` | Welcome for first-timers from `SCR-02.E06` | A **< Back** that returns to step 1 of 3, and the first-time chips. | `CHAT-1` | EXISTS |
| `SCR-15.E11` | **< Menu** | Back to `SCR-01`. Hidden in the first-run chat. | — | EXISTS |
| `SCR-15.E12` | First-run chat | Step 2 of 3 of `FLOW-01`, opened by `CIN-01`'s **LET'S GO**: a step indicator, our greeting ("Hi, we're OpenAgents. Let's see if a tool makes Coder better."), and `CARD-01` for Project map, whose **START THE TEST** is tap 3. The composer works; answers appear under the card. | `CHAT-4`, `LOOP-3` | NEW |

Transitions: in from `SCR-01.E12`, `SCR-01.E13`, `SCR-02.E06`, `CIN-01`
(first run), `SCR-16` (New chat). Out to `SCR-17` (Send on Cloud), `SCR-19` (Send on a computer,
or a recent Coder chat chip), `SCR-16`, `SCR-22` (**Connect a computer**),
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
| `SCR-17.E05` | Coder offer | **Run Coder on Studio Mac** with a computer ready (a chip when the router says the message needs a computer, a plain button otherwise); **Connect a computer** with none, only when the message needs one, which opens `SCR-22` and returns here through `SCR-23`; then **Open Coder on Studio Mac**. | `CHAT-7` | EXISTS (**Connect a computer** opens Computers today; the `SCR-22` target is NEW) |
| `SCR-17.E06` | Screen chips | **Open Wallet**, **Your computers** (or **Connect a computer**, which opens `SCR-22`), **Identity keys**, **Playtest**, **Report a problem**; the label is the app's, never the model's. | `CHAT-8` | EXISTS (the `SCR-22` target is NEW) |
| `SCR-17.E07` | Command card | A read-only `openagents` command, where it runs, and **Run**; then its output and **Run again**. The phone answers `computer list`, `show`, and `workspaces` itself. | `CHAT-8` | PARTIAL (the phone's card exists; the CLI route is built in `coder::cli_route` but not yet wired into the chat worker, so no live reply offers one yet) |
| `SCR-17.E08` | Follow-up chips | The next likely questions under a prepared answer; a tap sends it. | `CHAT-1` | EXISTS |
| `SCR-17.E09` | **Wrong answer** | Under a prepared answer only. Opens `SCR-18`. | `CHAT-1` | EXISTS |
| `SCR-17.E10` | Composer **Message OpenAgents** | The next message in the same chat. | All chat | EXISTS |
| `SCR-17.E11` | Loop cards and offers: `CARD-01` to `CARD-07`, **Start the test**, **Try it once**, **Run the check**, **Add to the Gym**, **See details** | The `CHAT-2` to `CHAT-14` taps, each a card's button or an offer; nothing runs or publishes without the tap. | `CHAT-2` to `CHAT-14` | EXISTS |
| `SCR-17.E12` | ~~Result line~~ | Retired in revision 3; `CARD-04` shows the player's result. | — | Retired |
| `SCR-17.E13` | **Try again** | After a failed reply; resends the same message. | — | EXISTS |

Transitions: in from `SCR-15`, `SCR-16`, `SCR-05.E11`, `SCR-09.E06`, `SCR-23`. Out to
`SCR-16`, `SCR-15`, `SCR-18`, `SCR-19`, `SCR-22`, the screens a chip names, and the
cards and sheets (`CARD-01` to `CARD-07`, `SCR-05`, `SCR-20`, `SCR-21`) through `E11`.

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

## Chat cards and sheets

A card is a typed record the router sends with a reply (NIP-CJ `card`
feedback) and the app draws with its own labels and one button. The
model's words never become a card's numbers or its button: every number
comes from a record the router verified, and the card names its source in
small gray text. A card sits under the reply that introduced it; a newer
card of the same thing (a run finishing, a draft revised) replaces the
older one in place. Cards follow `PAT-01` for failures.

Specified in [extension evaluation](../extensions/evaluation.md#chat-the-product-path);
the wire is in [NIP-CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs).

### CARD-01 Tool card

A tool, what it does, its latest result, and the one button that tests it.

```
| We'd try Project map. 18 trainers tested   |  (reply)
| it; most saw Coder pass more tests.        |
| +--------------------------------------+   |
| | [map] PROJECT MAP                    |   |  E01
| | Shows Coder how the project is laid  |   |  E02
| | out before it starts.                |   |
| | Latest: 5 of 8 → 7 of 8 tests ·      |   |  E08
| | Better · checked by 3 trainers       |   |
| | 8 tests, with and without the tool.  |   |  E03
| | About 5 minutes. Free. 3 runs left   |   |  E04
| | today.                               |   |
| |######################################|   |
| |#        START THE TEST              #|   |  E05 (primary)
| |######################################|   |
| | [ See the tests ]  [ More about it ] |   |  E06, E09
| +--------------------------------------+   |
| (search) Code finder  (check) Test reader  |  E07
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-01.E01` | Icon and name | The tool. | EXISTS |
| `CARD-01.E02` | One plain line | What it does. | EXISTS |
| `CARD-01.E03` | What will run | "8 tests, with and without the tool." | EXISTS |
| `CARD-01.E04` | Time, cost, and runs left | "About 5 minutes. Free. 3 runs left today." We pay for hosted runs. | PARTIAL ("Free. We run it on our computers."; no time or runs-left count, which no record says) |
| `CARD-01.E05` | **START THE TEST** | Sends a signed request to the hosted runner for this tool and its test set; the card becomes `CARD-03`. On the first run it's step 2 of 3's tap. | EXISTS (the hosted runner) |
| `CARD-01.E06` | **See the tests** | Opens `SCR-21` read-only. | NEW (not on the tool card in build 21) |
| `CARD-01.E07` | Other tools, as chips | Each sends "Test Code finder on Coder". | EXISTS |
| `CARD-01.E08` | Latest result | The newest verified result and how many trainers checked it, or "Not tested yet. Be the first." | PARTIAL (the newest result, or "Not tested yet. Be the first."; how many trainers checked it isn't shown) |
| `CARD-01.E09` | **More about it** | Opens `SCR-09` (later; hidden in v1). | NEW (later) |

States: no runs left today (E05 reads **START TOMORROW AT 9:00**, gray, and
a **Check a result** chip appears, since a check doesn't use a run); offline
(`PAT-01`); a run of this tool already going (the card is `CARD-03`).

### CARD-02 Test set draft card

The draft the interview builds with the player (`CHAT-10`). It changes as
they answer; nothing runs until they tap.

```
| +--------------------------------------+   |
| | YOUR TEST SET · DRAFT                |   |  E01
| | Tool: Changelog helper (yours)       |   |  E02
| |  1 Summarize a merged fix            |   |  E03
| |  2 Write the entry for a new flag    |   |
| |  3 Group three small changes         |   |
| |  4 Note a breaking change            |   |
| |  5 Leave an unrelated question alone |   |
| |    (tool should stay out of the way) |   |
| | Each test is checked on Coder's last |   |  E06
| | message and the files it made.       |   |
| |######################################|   |
| |#          LOOKS GOOD               #|   |  E05 (primary)
| |######################################|   |
| | [ Change it ]  [ See every test ]    |   |  E07, E04
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-02.E01` | Title and **DRAFT** | Says it's not public and hasn't run. | EXISTS |
| `CARD-02.E02` | The tool | An existing tool, or the one being made ("yours"). | EXISTS |
| `CARD-02.E03` | Tests, numbered | Names only; tests where the tool should stay out of the way are marked. | EXISTS |
| `CARD-02.E04` | **See every test** | Opens `SCR-21` with the draft. | EXISTS |
| `CARD-02.E05` | **LOOKS GOOD** | Approves this interview step; the next step (checks, then **Try it once**) follows in chat. At the last step it reads **TRY IT ONCE** and sends a one-run request. | EXISTS |
| `CARD-02.E06` | How tests are checked | One plain line per kind of check. | EXISTS |
| `CARD-02.E07` | **Change it** | Puts the cursor in the composer with "Change: ". | EXISTS |

### CARD-03 Run card

A run in progress. It replaces the card that started it.

```
| +--------------------------------------+   |
| | TESTING PROJECT MAP        ● ● ●     |   |  E01 (step 3 of 3 on the first run)
| | ■ ■ ■ ■ ■ □ □ □  with the tool       |   |  E02
| | ■ ■ ■ ■ □ □ □ □  without it          |   |
| | Test 5 of 8 · about 3 minutes left   |   |  E03
| | You can leave. We'll post the result |   |  E04
| | here and on the menu.                |   |
| | [ Stop ]                             |   |  E05
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-03.E01` | Title | The tool; step dots on the first run. | EXISTS |
| `CARD-03.E02` | Two rows of blocks | Progress for each side, without numbers to read. | PARTIAL (one row of blocks: the runner counts runs across both sides) |
| `CARD-03.E03` | Counter and time left | "Working" when time is unknown, never a guess. | PARTIAL ("35 of 36 runs done" or "Working"; no time left) |
| `CARD-03.E04` | Reassurance | Safe to leave. | EXISTS |
| `CARD-03.E05` | **Stop** | Cancels; confirms "Stop the test? It won't use a run." | EXISTS |

States: slow ("Taking longer than usual. We'll keep going."); failed on our
side (`PAT-01`: "Something went wrong on our side. This didn't use a run."
and **TRY AGAIN**); phone offline (the run continues; the card updates when
the phone is back).

### CARD-04 Result card

The payoff, in the chat.

```
| +--------------------------------------+   |
| | CODER GOT BETTER                     |   |  E01
| | without: 5 of 8  →  with: 7 of 8     |   |  E02
| | tests · Project map                  |   |
| | +50 XP when another trainer checks   |   |  E03
| | it                                   |   |
| |######################################|   |
| |#         ADD TO THE GYM            #|   |  E05 (primary)
| |######################################|   |
| | [ See details ]  [ See the tests ]   |   |  E04, E06
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-04.E01` | Verdict | **CODER GOT BETTER**, **NO CLEAR CHANGE**, **CODER DID WORSE WITH THIS TOOL**, or **FIRST TRY** for a one-run try. | EXISTS |
| `CARD-04.E02` | Tests passed without and with | The biggest text on the card. | EXISTS |
| `CARD-04.E03` | XP line | When XP comes, in one line. | EXISTS |
| `CARD-04.E04` | **See details** | Opens `SCR-05`. | EXISTS |
| `CARD-04.E05` | **ADD TO THE GYM** | Opens `SCR-20`. For a one-run try it reads **RUN THE FULL TEST SET** and sends that request instead. | EXISTS |
| `CARD-04.E06` | **See the tests** | Opens `SCR-21`. | EXISTS |

### CARD-05 Gym news card

What's new and what's in progress (`CHAT-9`), from the Gym's records.

```
| Here's what's new in the Gym this week.    |  (reply)
| +--------------------------------------+   |
| | ● Coder now uses Project map for     |   |  E01 (item)
| |   everyone. Oct 3 · 6 checks         |   |
| | ● Trainer 2PX made a test set for    |   |
| |   Test reader. 1 check so far.       |   |
| | ● Code finder: no clear change on 8  |   |
| |   tests. 3 checks.                   |   |
| | ● Build 21: tests in chat.           |   |
| |######################################|   |
| |#      CHECK TRAINER 2PX'S RESULT    #|   |  E02 (primary: the one offer)
| |######################################|   |
| | From the Gym's records and our       |   |  E03
| | changelog.                           |   |
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-05.E01` | Up to 5 items | Each from one record: a result, a test set, a check, an adoption, or a changelog entry. Tapping an item opens its card (`CARD-01`, `CARD-04`, or `CARD-06`). | PARTIAL (the items, each from a record; tapping an item opens nothing yet) |
| `CARD-05.E02` | One offer | The single most useful next step, or none. | EXISTS |
| `CARD-05.E03` | Source line | Where the items came from. | EXISTS |

Empty: "Nothing new since you last asked." and a **Test a tool** chip.

### CARD-06 Check card

Another trainer's result, waiting for someone to run the same tests
(`CHAT-13`).

```
| +--------------------------------------+   |
| | CHECK A RESULT              +50 XP   |   |  E01
| | Trainer 2PX says Test reader made    |   |  E02
| | Coder pass 6 of 8 tests instead of 4.|   |
| | Run the same tests to check it.      |   |
| | About 5 minutes. Doesn't use a run.  |   |  E03
| |######################################|   |
| |#          RUN THE CHECK             #|   |  E04 (primary)
| |######################################|   |
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-06.E01` | Title and XP | The XP the checker earns if it holds up. | EXISTS |
| `CARD-06.E02` | The claim | Whose result, which tool, the numbers. Never your own. | EXISTS |
| `CARD-06.E03` | Time and cost | A check never uses a daily run. | PARTIAL (says a check doesn't use a daily run; no time) |
| `CARD-06.E04` | **RUN THE CHECK** | Sends the check request; the card becomes `CARD-03`, then `CARD-04` with **YOU CONFIRMED IT** or **IT DIDN'T HOLD UP**. | EXISTS |

### CARD-07 Credit card

What your work has earned (`CHAT-14`), from the XP ledger.

```
| +--------------------------------------+   |
| | YOUR CREDIT                          |   |  E01
| | Project map test set                 |   |  E02
| |  ✓ Checked by Trainer 2PX   +25 XP   |   |
| |  ✓ Checked by Trainer QA4   +25 XP   |   |
| |  … Waiting for a check               |   |
| | Changelog helper (your tool)         |   |
| |  Coder uses it now          +200 XP  |   |
| | Level 3 · 40 XP to level 4           |   |  E03
| |######################################|   |
| |#       SHARE WHAT YOU MADE          #|   |  E04 (primary)
| |######################################|   |
| | XP can't be spent. It shows what you |   |  E05
| | did, with your name on it.           |   |
| +--------------------------------------+   |
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `CARD-07.E01` | Title | — | EXISTS |
| `CARD-07.E02` | Your tools and test sets | Each with the checks and adoptions that earned XP, and what's still pending. | EXISTS (`eval-check` and `eval-adopt` awards, from the phone's ledger) |
| `CARD-07.E03` | Level line | From the player's ledger. | EXISTS |
| `CARD-07.E04` | **SHARE WHAT YOU MADE** | The system share sheet with an image card and a link. | EXISTS |
| `CARD-07.E05` | Honesty line | XP is not money. | EXISTS |

Empty: "Nothing yet. When another trainer checks a result you added, you
earn XP here." and a **Test a tool** chip.

### SCR-20 Add to the Gym

A sheet that confirms what becomes public, opened from `CARD-04.E05` and
`SCR-05.E07`.

```
+------------------------------------------+
| ADD TO THE GYM                    [ X ]  |
|------------------------------------------|
| Everyone will see:                       |  E01
|  · your 8 tests and how they're checked  |
|  · the result: 5 of 8 → 7 of 8           |
|  · your trainer name, Trainer 7KQ        |
| Coder's full work on each test stays     |  E02
| private.                                 |
|                                          |
| Other trainers can run your tests to     |  E03
| check the result. You earn XP when they  |
| confirm it.                              |
|##########################################|
|#            ADD TO THE GYM              #|  E04 (primary)
|##########################################|
| [ Not now ]                              |  E05
+------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `SCR-20.E01` | What becomes public | The tests, the result, and the trainer name. | EXISTS |
| `SCR-20.E02` | What stays private | Coder's full work (the trajectories). | EXISTS |
| `SCR-20.E03` | What happens next | Checks and XP, in one sentence. | EXISTS |
| `SCR-20.E04` | **ADD TO THE GYM** | Publishes the test set and the signed result; the chat shows "Added to the Gym" and, if a level was crossed, `SCR-06`. | EXISTS |
| `SCR-20.E05` | **Not now** | Closes; the result stays on the phone. | EXISTS |

States: adding failed (`PAT-01`: "We couldn't add your result. It's saved
on this phone." and **TRY AGAIN**).

IDIOT PROOF check: **pass.** Says exactly what becomes public before
anything does.

### SCR-21 Test set

A sheet listing every test in a test set, read-only for a published one and
editable in words for a draft.

```
+------------------------------------------+
| PROJECT MAP · 8 TESTS             [ X ]  |  E01
|------------------------------------------|
| 1 Find where login is handled            |  E02
|   Checked: Coder names the right file,   |  E03
|   and looked at the project map.         |
| 2 Add a test for the date parser         |
|   Checked: a new test file exists and    |
|   passes.                                |
| …                                        |
| 8 Leave a one-line fix alone             |
|   The tool should stay out of the way.   |
|   Checked: Coder didn't use the tool.    |
| Made by Trainer 7KQ · checked by 3       |  E04
|##########################################|
|#                DONE                    #|  E05 (primary; LOOKS GOOD on a draft)
|##########################################|
+------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `SCR-21.E01` | Title | The tool and the count. | EXISTS |
| `SCR-21.E02` | Each test's task | In the words Coder gets. | EXISTS |
| `SCR-21.E03` | How it's checked | Each grader in one plain line. | EXISTS |
| `SCR-21.E04` | Author and checks | Who made it and how often it was checked. | PARTIAL (where the test set comes from; no check count) |
| `SCR-21.E05` | **DONE** / **LOOKS GOOD** | Closes; on a draft, approves it (the same as `CARD-02.E05`). | EXISTS |

IDIOT PROOF check: **pass.** A read-only list with one action.

## Connect a computer

Pairing a computer by scanning the code on its screen. Specified in the
[QR pairing design](../coder/design/2026-09-29-auto-pairing.md); the wire is
NIP-HOST's [connect codes](../../nips/openagents/NIP-HOST.md#connect-codes)
and [enrollment over iroh](../../nips/openagents/NIP-HOST.md#enrollment-over-iroh).
Both screens are NEW
([#9971](https://github.com/OpenAgentsInc/openagents/issues/9971)); the
scanner itself exists (`QRScanner.swift`, `QRScanner.kt`).

### SCR-22 Connect a computer

Opened by **Connect a computer** in chat (`SCR-17.E05`, `SCR-17.E06`,
`SCR-15.E04`, `SCR-14.E05`) and by the row in Account > Computers. The camera
is the screen; pointing it at the code is the one action.

```
+------------------------------------------+
| [<]   CONNECT A COMPUTER                 |  E01
|------------------------------------------|
| NEARBY  (after the milestone)            |  E02
|  (computer) Studio Mac              >    |
|+----------------------------------------+|
||                                        ||
||             [ camera view ]            ||  E03 (primary)
||                                        ||
|+----------------------------------------+|
| Point at the code on your computer.      |  E04
| [ Paste a code ]                         |  E05
| No code on your computer? Get OpenAgents |  E06
| for Mac at openagents.com/desktop.       |
+------------------------------------------+
```

States:

- **Connecting:** after a scan, the camera freezes and one line reads
  "Connecting to Studio Mac…" (the label from the code). Nothing else to
  tap.
- **Camera not allowed:** "Allow the camera for OpenAgents in Settings, or
  paste a code." with **Open Settings** and **Paste a code**.
- **Not a computer's code:** "That isn't a code from OpenAgents for Mac.",
  and the camera keeps looking. An older `coder-host:` code from a computer
  set up the old way is accepted and pairs as before.
- **Code expired or already used:** "This code has expired. Your computer
  shows a new one; scan that." (`expired`, `revoked`), or "Another phone
  already used this code. Scan the new one on your computer." (`forbidden`).
- **Phone clock off:** "Your phone's clock is off by 4 minutes. Set it to
  automatic in Settings, then scan again." Shown when the computer's time
  differs from the phone's by more than a minute.
- **Can't reach the computer:** "We couldn't reach Studio Mac. Check that
  OpenAgents is open on it, then scan again." with **Try again**
  (`PAT-01`).
- **Local network not allowed (iOS):** pairing still completes, more slowly;
  one quiet line reads "Allow Local Network for OpenAgents in Settings to
  connect faster on Wi-Fi."

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-22.E01` | **<** and title **CONNECT A COMPUTER** | Back to where the player came from; nothing changes. | — | NEW |
| `SCR-22.E02` | **Nearby** list | After the milestone: computers on the same Wi-Fi. A tap asks the computer to connect and shows the six-digit code the computer also shows (`DSK-04`). Hidden until nearby pairing ships. | `CHAT-7` | NEW (later) |
| `SCR-22.E03` | Camera view | Reads an `openagents-connect:` (or older `coder-host:`) code, then connects and pairs. The primary action. | `CHAT-7` | PARTIAL (the scanner exists behind Add a computer > Scan invitation) |
| `SCR-22.E04` | **Point at the code on your computer.** | The one-line "what do I do next?". | — | NEW |
| `SCR-22.E05` | **Paste a code** | Reads a code the person copied on the computer with **Can't scan? Copy a code instead** (`DSK-01.E04`). | `CHAT-7` | PARTIAL (Paste invitation exists) |
| `SCR-22.E06` | Get the Mac app line | For a player with nothing to scan: where to get OpenAgents for Mac. | — | NEW |

Transitions: in from `SCR-17.E05`, `SCR-17.E06`, `SCR-15.E04`, `SCR-14.E05`,
and Account > Computers. Out to `SCR-23` after a pairing, or back.

IDIOT PROOF check: **pass.** One action (point the camera), a fallback for a
phone that can't scan, a line for a player with no computer set up, and
every failure names its own next step.

### SCR-23 Connected

Shown once a scan pairs. It confirms and sends the player back.

```
+------------------------------------------+
|                                          |
|              (check)                     |  E01
|         Studio Mac is connected.         |  E02
|  You can send Coder work on this Mac     |  E03
|  from any chat.                          |
|##########################################|
|#                DONE                    #|  E04 (primary)
|##########################################|
+------------------------------------------+
```

| ID | Element | What it does | Loop | Status |
| --- | --- | --- | --- | --- |
| `SCR-23.E01` | Check mark | Success at a glance. | — | NEW |
| `SCR-23.E02` | **Studio Mac is connected.** | The computer's name from the code's label, else from the computer. | `CHAT-7` | NEW |
| `SCR-23.E03` | What's next | One line: Coder can work on this Mac from chat. When the computer allowed a terminal, a second line: "This phone can also open a terminal on it." | — | NEW |
| `SCR-23.E04` | **DONE** | Returns to the chat the chip came from, where the reply's offer now reads **Run Coder on Studio Mac** and dispatches through `task.create`; from Account > Computers, returns to Computers with the new row. | `CHAT-7` | NEW |

Transitions: in from `SCR-22`. Out to `SCR-17` (the same conversation, with
the offer updated) or Computers.

IDIOT PROOF check: **pass.** One sentence, one button, back to where the
player was.

## Desktop app screens

The companion desktop app, **OpenAgents** for Mac (`crates/openagents-desktop`,
[#9970](https://github.com/OpenAgentsInc/openagents/issues/9970)). Installing
it and opening it runs the computer's side of Coder; nothing on these
screens needs a terminal. They follow the same [words on
screen](#words-on-screen): no key, host, relay, grant, workspace, tailnet,
Tailscale, npub, or nsec. "Project" names a workspace and "phone" names a
device. Every element is NEW. Coder's work on a project needs Codex or
Claude Code signed in on the Mac; the milestone assumes the person already
did that, and pairing never asks them to.

The screens below show a Mac. On Linux and Windows every "this Mac" on them
reads "this computer" (for example **Starting Coder on this computer…**),
and "This Mac" reads "This computer".

There is no screen for a computer that already ran Coder the old way. The
app upgrades that setup silently on first launch and opens on `DSK-01` or
`DSK-03` as usual; phones paired before stay paired
([#9965](https://github.com/OpenAgentsInc/openagents/issues/9965)). If the
upgrade has to wait, Coder keeps running as it was and `DSK-01` shows one
quiet line where the code goes: "Coder keeps running here as it did before
this app."

### DSK-01 Connect a phone

The first-run window, and the window **Connect another phone** opens.

```
+----------------------------------------------+
|  OpenAgents                                  |
|----------------------------------------------|
|                                              |
|               +----------------+             |
|               |                |             |
|               |   [ QR code ]  |             |  E01
|               |                |             |
|               +----------------+             |
|   Scan with the OpenAgents app on your       |  E02
|   phone.                                     |
|                                              |
|   Can't scan? Copy a code instead            |  E04
+----------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `DSK-01.E01` | The QR code | A connect code for one phone, drawn locally. It changes quietly every minute; a replaced code stops working a minute later, and every code stops when the window hides, the screen locks, after ten idle minutes, or once a phone connects. | NEW |
| `DSK-01.E02` | **Scan with the OpenAgents app on your phone.** | The one instruction. | NEW |
| `DSK-01.E03` | (Removed.) There is no rights checkbox. Every phone that scans gets full permission, a terminal included; to narrow a phone, **Remove** it. Owner-directed on 2026-09-29 ([#9965](https://github.com/OpenAgentsInc/openagents/issues/9965)). | REMOVED |
| `DSK-01.E04` | **Can't scan? Copy a code instead** | Copies the same code's text once, for **Paste a code** (`SCR-22.E05`); says "Copied. It works for two minutes." | NEW |

States: while the phone connects, the code gives way to "Connecting to
Kai's iPhone…". If the computer can't reach our connection service, one line
reads "Phones on this Wi-Fi can still connect." and the code stays.

The window opens nearly full screen (90% of the display, centered) and the
code and words scale up with it.

IDIOT PROOF check: **pass.** One code, one sentence, no choice to make.

### DSK-02 Connected

Shown when a phone pairs.

```
+----------------------------------------------+
|  OpenAgents                                  |
|----------------------------------------------|
|  (check) Kai's iPhone is connected.          |  E01
|                                              |
|  Pick a project for Coder                    |  E02
|  ~/code/website              [ Choose folder… ]
|                                              |
|  Coder uses Codex or Claude Code on this Mac |  E03
|   (check) Codex      signed in               |
|   ( )     Claude Code  not signed in         |
|                                              |
|  [ on ] Let my phone start Coder here        |  E04
|                                              |
|##############################################|
|#                  DONE                      #|  E05 (primary)
|##############################################|
+----------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `DSK-02.E01` | **Kai's iPhone is connected.** | The phone's label. | NEW |
| `DSK-02.E02` | **Pick a project for Coder** | **Choose folder…** picks a Git checkout; it becomes a project the phone can send work to. | NEW |
| `DSK-02.E03` | **Coder uses Codex or Claude Code on this Mac** | A check beside each one that is signed in. With neither, one line: "Sign in to Codex or Claude Code on this Mac so Coder can work here." It never offers a command. | NEW |
| `DSK-02.E04` | **Let my phone start Coder here** | Turns on once a project is picked; lets a phone's **Run Coder** start work in that project. | NEW |
| `DSK-02.E05` | **DONE** | Opens `DSK-03`. | NEW |

IDIOT PROOF check: **pass with a note.** Two setup choices, both preset
where they can be. The note: an agent sign-in is the person's to do, before
or after pairing; the screen says what is missing in one line.

### DSK-03 Home

```
+----------------------------------------------+
|  OpenAgents                                  |
|----------------------------------------------|
|  Online. Your phone can reach this Mac.      |  E01
|                                              |
|  PHONES                                      |  E02
|   Kai's iPhone · seen 2 min ago · terminal   |
|                                   [ Remove ] |
|                                              |
|  CODER                                       |  E03
|   Fix the login test · Working               |
|   Update the README · Done                   |
|                                              |
|  [ Connect another phone ]                   |  E04
+----------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `DSK-03.E01` | Status line | **Online. Your phone can reach this Mac.** or **Offline.** | NEW |
| `DSK-03.E02` | **Phones** | Each phone: name, last seen, and whether it may open a terminal. **Remove** confirms "Remove Kai's iPhone? It can't reach this Mac until it connects again." and cuts it off at once, open terminals included. | NEW |
| `DSK-03.E03` | **Coder** | Running and recent tasks, by title and phase. | NEW |
| `DSK-03.E04` | **Connect another phone** | Opens `DSK-01`. | NEW |

IDIOT PROOF check: **pass.** Status first; the one destructive action
confirms in plain words (`CHK-09`).

### DSK-04 A phone nearby wants to connect

After the milestone
([#9975](https://github.com/OpenAgentsInc/openagents/issues/9975)).

```
+----------------------------------------------+
|  Kai's iPhone wants to connect.              |  E01
|  Check that your phone shows 482 913.        |  E02
|##############################################|
|#                 CONNECT                    #|  E04 (primary)
|##############################################|
|  [ Don't connect ]                           |  E05
+----------------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `DSK-04.E01` | **Kai's iPhone wants to connect.** | The phone's label; display only. | NEW (later) |
| `DSK-04.E02` | The six-digit code | The same code the phone shows; the person compares them. | NEW (later) |
| `DSK-04.E03` | (Removed.) No rights checkbox: **Connect** grants full permission, as a scan does. | REMOVED |
| `DSK-04.E04` | **Connect** | The only way a nearby phone is admitted. | NEW (later) |
| `DSK-04.E05` | **Don't connect** | Closes; nothing is granted. The request also ends by itself after two minutes. | NEW (later) |

IDIOT PROOF check: **pass.** One comparison, one click.

### DSK-05 Menu bar

After the milestone
([#9976](https://github.com/OpenAgentsInc/openagents/issues/9976)).

```
+------------------------------------+
| Online. Your phone can reach this  |  E01
| Mac.                               |
|------------------------------------|
| Open OpenAgents                    |  E02
| Connect a phone…                   |  E03
| Pause Coder                        |  E04
|------------------------------------|
| Quit OpenAgents                    |  E05
| Stop Coder on this Mac             |  E06
+------------------------------------+
```

| ID | Element | What it does | Status |
| --- | --- | --- | --- |
| `DSK-05.E01` | Status line | As `DSK-03.E01`. | NEW (later) |
| `DSK-05.E02` | **Open OpenAgents** | Opens `DSK-03`. | NEW (later) |
| `DSK-05.E03` | **Connect a phone…** | Opens `DSK-01`. | NEW (later) |
| `DSK-05.E04` | **Pause Coder** | No new tasks start; running ones finish. | NEW (later) |
| `DSK-05.E05` | **Quit OpenAgents** | Closes the window; Coder keeps running for the phones. | NEW (later) |
| `DSK-05.E06` | **Stop Coder on this Mac** | Confirms, then stops Coder from starting at login; phones show this Mac as offline. | NEW (later) |

IDIOT PROOF check: **pass.** Quitting the window and stopping Coder are two
named items, so neither surprises.

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
- The end card has one button, **LET'S GO**, which opens the first-run
  chat (`SCR-15.E12`) at step 2 of 3.
- Sound is optional: the subtitles carry the whole script with the phone on
  silent.

### Shot list

| Shot | Camera move | On screen | Subtitle (narration) | Duration |
| --- | --- | --- | --- | --- |
| `CIN-01.S01` | Fade in from black; slow push toward a single point of light. | Black, then the power emblem glowing. | "Every day, people ask AI agents to write their code." | 5 s |
| `CIN-01.S02` | High crane shot descending over the white wireframe Grid. | The Grid's plaza, the ball, blocks, and dominoes. | "One agent, working alone, only gets so good." | 7 s |
| `CIN-01.S03` | Low tracking shot past other players' avatars and their name tags. | Other trainers in the plaza, walking and playing. | "So we're doing it together. This is the Verse, where agents and people meet." | 8 s |
| `CIN-01.S04` | Dolly through the Gym doorway to the board inside. | The Gym building, its board lit, ghost figures replaying a run between stations. | "This is the Gym. Here, you train your agent. You give it a new tool, and we test if it helps." | 10 s |
| `CIN-01.S05` | Wide pull-back and rise; many small lights stream into one large emblem above the world. | The whole Grid from above; the Lagrange 1 station in the sky. | "When a tool helps, every agent gets it. Many agents, trained by many people, combined into one: OpenAgents." | 10 s |
| `CIN-01.S06` | Quick cuts: a title appearing over a name tag, a level number rising, the ball glowing. | Titles, a level-up, the glowing ball. | "Train well, and you earn XP, loot, rewards, and glory." (See the note on bitcoin below.) | 7 s |
| `CIN-01.S07` | Swoop down and settle behind the player's agent, into the follow camera. | Coder, the matte-black agent with the glowing emblem, facing the Gym. | "This is Coder. It's yours to train." | 6 s |
| `CIN-01.S08` | Hold; the end card fades in over the live scene. | End card: **LET'S GO** and "Step 2 of 3". | "Let's see if a tool makes Coder better." | 4 s, then waits |

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
> test if it helps.
>
> When a tool helps, every agent gets it. Many agents, trained by many
> people, combined into one: OpenAgents.
>
> Train well, and you earn XP, loot, rewards, and glory.
>
> This is Coder. It's yours to train.
>
> Let's see if a tool makes Coder better.

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
| Subtitle band, end card, skip control | PARTIAL | The end card with **LET'S GO** exists (`crates/openagents-mobile/src/first_run.rs`); the subtitle band and skip control are NEW |
| Voice-over audio | NEW | No audio pipeline in the Verse today |
| The "lounge" | NEW | `lounge` exists only as a desktop chat room; there is no lounge place. `S03` uses the plaza instead. |
| The stream of lights and the big emblem (`S05`) | NEW | Effect in the Verse renderer |

## User flows

### FLOW-01 First-time playtester, and the Gym intro

Chat first (the owner's direction, 2026-09-29,
[#9958](https://github.com/OpenAgentsInc/openagents/issues/9958)): a new
install lands in a chat with OpenAgents, the auto-upgradable agent, with the
tab bar. Chat is pure chat plus the capabilities that are present. There is
no Choose Coder screen, no step counter, and no message sent for the
player; the Gym's starters, cards, and menu are not volunteered. The Gym's
loop (test, check, add to the Gym, XP) is a separate flow the player opts
into with **Train Coder**, on the Verse's Gym board (`SCR-01.E14`, **See
the board**) or under Account.

```
 App open (first time)
   |
   v
 SCR-15 Chat with OpenAgents   the tab bar stays; Profile and previous
   |                           chats in the header; first-time chips
   |  (any message; eval.* routes answer if asked)
   |
   |  Train Coder  (the Verse's Gym board, or Account)
   v
 the Gym intro, three taps to a test starting:
```

The intro is the guided path below. It can't be skipped into a broken
state, it's at most 3 taps from **Train Coder** to the first test run
starting, and steps 2 and 3 happen in chat. **Not now** on step 1 returns
to the chat, opted out, until the next **Train Coder**.

```
 Train Coder
   |
   v
 SCR-02 Choose your agent   STEP 1 OF 3     (Not now: back to chat)
   |  tap 1: CHOOSE CODER   (identity created silently)
   v
 CIN-01 Intro cinematic     (~60 s, plays through the first time)
   |  tap 2: LET'S GO
   v
 SCR-15.E12 Intro chat      STEP 2 OF 3
   |  "Hi, we're OpenAgents. Let's see if a capability makes Coder better."
   |  CARD-01 Project map, preselected
   |  tap 3: START THE TEST  <-- the first run starts here
   v
 CARD-03 Run card           STEP 3 OF 3   (~5 min; safe to leave; the
   |                        player can keep chatting)
   v
 CARD-04 Result card        "without: 5 of 8 -> with: 7 of 8 tests"
   |  ADD TO THE GYM
   v
 SCR-20 Add to the Gym      what becomes public
   |  ADD TO THE GYM
   v
 SCR-06 Level up (if a level was crossed)
   |  NICE
   v
 SCR-01 The Gym menu        intro complete
```

Rules that keep the path unbreakable:

1. The app records the furthest step reached. On any reopen after **Train
   Coder**, the Chat tab resumes at that step: `SCR-02`, the `CIN-01` end
   card, or the intro chat with its newest card on top. `SCR-01` appears
   only after the result, and after that the app reopens on the chat, with
   the header's **Menu** leading to `SCR-01`.
2. In the first-run chat, the composer works: the player can ask anything,
   and the answers come under the first-run card without replacing it. The
   card's button stays the primary until the run starts.
3. No step asks for a sign-in, name, email, computer, wallet funding, or key
   backup. Identity is created silently at step 1.
4. If the run fails on our side, **TRY AGAIN** restarts it and doesn't use
   a daily run.
5. If the player is offline at step 2, the card says so and waits with
   **TRY AGAIN**. The step doesn't change.
6. The first result needs no publishing: **Not now** on `SCR-20` still ends
   the guided path at `SCR-01`.

### FLOW-02 Returning playtester, daily loop

Two taps from app open to a run.

```
 App open
   |
   v
 SCR-01 Main menu   "Next: a check is waiting for you (+50 XP)."
   |  tap 1: CHAT WITH OPENAGENTS  (or a starter chip)
   v
 SCR-17 with CARD-06 Check card (or CARD-01 for a tool to test)
   |  tap 2: RUN THE CHECK / START THE TEST
   v
 CARD-03 --> CARD-04 --> ADD TO THE GYM --> SCR-20
   |
   v
 SCR-06 Level up (if crossed) --> SCR-01
   |
   +--> "What's new?" --> CARD-05 (FLOW-09)
   +--> "What have I earned?" --> CARD-07 (FLOW-10)
   +--> later: SCR-10 Rankings, SCR-12 Updates
```

Reasons to return, each shown as the `SCR-01.E11` next-step line:

1. A check is waiting (another trainer's result to confirm).
2. Your result was checked, or Coder adopted your tool (XP).
3. New daily runs ("3 runs left today").
4. Something new in the Gym that fits what you tested.
5. The season ends on a date (Season 1 ends 2026-10-26).

### FLOW-03 Kicking the tires in chat

A first-timer, or anyone curious, asks what this is before doing anything.
Zero setup, answers in under a second.

```
 SCR-01.E12 CHAT WITH OPENAGENTS   (or SCR-02.E06 on the first open)
   |
   v
 SCR-15 New chat      chips: "Who are you?"  "What can you do?"
   |  tap a chip, or type                   "What's the Gym?"
   v
 SCR-17 Conversation  prepared answer at once · "Prepared answer"
   |                  follow-up chips: "What model is this?" ...
   v
 A question the bank doesn't cover --> opener at once, then the model's
   answer streams in; a product question is answered from our notes
   |
   +--> "What's the Gym?" --> answer + CARD-01 --> START THE TEST (FLOW-04)
   +--> < Back / < Menu --> where the player was
```

Status: EXISTS up to the Gym answer; `CARD-01` is NEW.

### FLOW-04 Test a tool from chat

Chat hands the player a tool with the test ready; the card's button does
the work.

```
 SCR-17  "Which tool should I try?"
   |     "Try Project map. 18 trainers tested it; most saw Coder pass
   |      more tests."   CARD-01 Project map
   v  tap 1: START THE TEST
 CARD-03 Run card --> CARD-04 Result card
   |  ADD TO THE GYM --> SCR-20 --> ADD TO THE GYM
   v
 "Added to the Gym. You'll earn XP when another trainer checks it."
```

One tap from a chat answer to a run. Status: NEW (`eval.run`, the hosted
runner).

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
                           SCR-22 Connect a computer: point the camera at
                              |   the code in OpenAgents for Mac (DSK-01)
                              v
                           SCR-23 Studio Mac is connected.  [DONE]
                              |
                              v
                           SCR-17, the same conversation, now offering
                           [Run Coder on Studio Mac]
```

Status: EXISTS (`CHAT-7`) with **Connect a computer** opening Computers
(Add a computer); the scan path through `SCR-22` and `SCR-23` is NEW.

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


### FLOW-07 Make a tool and its tests in chat

The interview (`CHAT-10`), driven from chat. OpenAgents asks; the player
answers in their own words; every step is a tap.

```
 SCR-17  "Help me make a tool that writes changelog entries"
   |     "Happy to. First: when Coder writes a changelog entry, what does
   |      a good one look like?"
   |  (player answers in a sentence or two)
   v
 "Here's what we'd make: a tool that tells Coder how you write entries
  (a short guide Coder follows), plus Project map to find the change."
   |  [ LOOKS GOOD ]  [ Change it ]
   v
 CARD-02 draft: 4 tests where the tool should help, 1 where it shouldn't
   |  tap: LOOKS GOOD            (or Change it --> revised CARD-02)
   v
 "Here's how we'd check each test." (checks in plain words, on CARD-02)
   |  tap: LOOKS GOOD
   v
 CARD-02 --> TRY IT ONCE --> CARD-03 --> CARD-04 "FIRST TRY"
   |  "Test 3 looks too easy: Coder passed it without the tool. Change it?"
   |  (fix and repeat until the player is happy)
   v
 CARD-04 --> RUN THE FULL TEST SET --> CARD-03 --> CARD-04
   |  ADD TO THE GYM --> SCR-20 (the tool and its tests become public)
   v
 "Added to the Gym."
```

Rules: the tool made in chat is a short guide Coder follows, optionally
with catalog tools turned on; a tool with new code is made with Coder on a
connected computer ("We'll need your computer to build that. Run Coder on
Studio Mac?", `CHAT-7`), then tested the same way. The draft stays on the
phone. At least one test must be one where the tool should stay out of the
way; chat adds it if the player doesn't. Status: EXISTS (build 21;
`coder::eval_author` on the chat worker).

### FLOW-08 Check another trainer's result

```
 SCR-01 "Next: a check is waiting (+50 XP)" --> CHAT WITH OPENAGENTS
   v
 CARD-06 "Trainer 2PX says Test reader made Coder pass 6 of 8 instead of 4."
   |  tap: RUN THE CHECK
   v
 CARD-03 --> CARD-04 "YOU CONFIRMED IT" (or "IT DIDN'T HOLD UP")
   |  ADD TO THE GYM --> SCR-20
   v
 "+50 XP once our referee confirms it. Trainer 2PX earns XP too."
```

A check never uses a daily run, and a player never gets their own result
to check. Status: EXISTS (build 21; the live checks so far ran through
the hosted runner).

### FLOW-09 What's new in the Gym

```
 SCR-01.E13 "What's new" chip, or "What's new in the Gym?" typed
   v
 SCR-17  a grounded reply and CARD-05 (up to 5 items, each with its source)
   |  tap an item --> its CARD-01, CARD-04, or CARD-06
   |  or the card's one offer
```

Every item comes from a verified record: a published result, a test set,
a check, an adoption, or our changelog. With no records, chat says
"Nothing new since you last asked." Status: EXISTS (build 21, `gym.news`;
tapping an item opens nothing yet).

### FLOW-10 Credit: when your work is used

```
 Another trainer's check confirms your result
   --> our referee signs the XP award
   --> SCR-01.E11 "Trainer 2PX confirmed your result. +25 XP."
   --> CHAT WITH OPENAGENTS opens CARD-07
 Coder adopts your tool
   --> "Coder now uses your tool for everyone. +200 XP."
```

"Used" means exactly two things: another trainer checked your result or
test set and got the same answer, or Coder adopted your tool. XP can't be
spent; nothing pays money. Status: EXISTS for `eval-check` (live on
2026-09-29); `eval-adopt` is tested, and no adoption has been made.

## NAV-01 Navigation map

Chat first: the Chat tab opens on `SCR-15`, with the tab bar. The Gym menu
is the Gym's hub, reached after **Train Coder** and the intro's first
result, with chat as its main spoke.

```
                  [after Train Coder, from the Verse's Gym board or Account]
            SCR-02 --> CIN-01 --> intro chat (SCR-15.E12, step 2)
                                      |  CARD-01 -> CARD-03 -> CARD-04
                                      v
  SCR-12 Updates <--bell--+       SCR-20 --> SCR-06 --> SCR-01
        (later)           |
                          |
  SCR-07 Coder <----------+------- SCR-01 MAIN MENU
    (later)               |          |   primary: CHAT WITH OPENAGENTS
  SCR-10 Rankings <-------+          |   chips: Test a tool, What's new,
    (later)               |          |          Check a result
  SCR-11 Profile <--------+          v
     |                           CHAT: SCR-15 New chat <--> SCR-16 (☰)
     +--> SCR-13 Report a problem       |  Send
     +--> Advanced (later)              v
                                  SCR-17 Conversation
  The Gym in the Verse <--E14--     |  cards: CARD-01 tool, CARD-02 draft,
    (results boards; social         |         CARD-03 run, CARD-04 result,
     later)                         |         CARD-05 news, CARD-06 check,
                                    |         CARD-07 credit
                                    +--> SCR-05 Result detail
                                    +--> SCR-20 Add to the Gym
                                    +--> SCR-21 Test set
                                    +--> SCR-19 Coder on a computer
                                    +--> SCR-22 Connect a computer
                                    |      --> SCR-23 Connected --> back
                                    +--> SCR-18 Wrong answer (inline)
                                    +--> screen chips: Wallet, Your
                                         computers, Identity keys,
                                         Playtest, Report a problem

  SCR-14 setup prompts open over any screen when triggered (later).
  Account > Computers also opens SCR-22.
  On the Mac: DSK-01 Connect a phone --> DSK-02 Connected --> DSK-03 Home;
  DSK-04 (nearby) and DSK-05 (menu bar) come after the milestone.
  PAT-01 replaces a screen's or card's body on failure.
```

Every screen reaches chat in at most one tap, and `SCR-01` in one for a
player who opted in. From `SCR-01`, a run starts in two taps (`FLOW-02`).
The app is four tabs (Chat, Verse, Wallet, Account) with Chat first, and
the tab bar stays: chat is the app's first screen, and the Gym menu is the
Gym's hub inside it.

## IDIOT PROOF checklist

| ID | Check |
| --- | --- |
| `CHK-01` | One primary action: a single big white-filled button; everything else is outlined or gray. In chat, the newest card's button or offer is the primary; otherwise the composer is. |
| `CHK-02` | No jargon in any label (see the banned list in [Words on screen](#words-on-screen)). |
| `CHK-03` | A one-line "Next:" answer to "what do I do next?" |
| `CHK-04` | No dead ends: every state, including empty, error, and offline, has one clear next step. |
| `CHK-05` | No required setup before the first win: no computer, wallet funding, or key backup. Setup is deferred until needed and explained in one sentence then (`SCR-14`). |
| `CHK-06` | At most 3 taps from app open to starting a test run (first time: 3; returning: 2). |
| `CHK-07` | Defaults chosen for the player: Coder as the agent, Project map as the first tool, a ready test set. |
| `CHK-08` | Progress always visible: tests passed without and with the tool, XP, level bar. |
| `CHK-09` | Destructive, spending, or publishing actions confirmed in plain words. |
| `CHK-10` | The first-time flow is guided (step 1 of 3 style) and can't be skipped into a broken state. |
| `CHK-11` | Talk leads to a tap: every chat reply that implies an action carries that action as the app's own control, nothing acts without a tap, and chat never claims to have done what it hasn't. |
| `CHK-12` | Chat drafts, you decide: in making a tool or a test set, every step is a tap, the draft is visible before anything runs, and nothing is public until **Add to the Gym**. |
| `CHK-13` | Numbers are real: every score, count, and news item comes from a verified record and names its source; with no record, the screen or card says "Not known yet" or "Nothing yet", never a made-up number. |

Results for every screen and card. "n/a" means the check doesn't apply.

| Screen | 01 | 02 | 03 | 04 | 05 | 06 | 07 | 08 | 09 | 10 | 11 | 12 | 13 | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SCR-01` Main menu | ✓ (chat) | ✓ | ✓ | ✓ | ✓ | ✓ (2 taps to a run) | ✓ | ✓ | n/a | n/a | ✓ | n/a | ✓ | Pass |
| `SCR-02` Choose your agent | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (tap 1) | ✓ | ✓ | n/a | ✓ | ✓ | n/a | ✓ | Pass |
| `CIN-01` Intro cinematic | ✓ | ✓ | ✓ (end card) | ✓ (resumes at end card) | ✓ | ✓ (tap 2) | n/a | n/a | n/a | ✓ | n/a | n/a | n/a | Pass |
| `SCR-05` Result detail | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | ✓ (via `SCR-20`) | n/a | ✓ | n/a | ✓ | Pass |
| `SCR-06` Level up | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass |
| `SCR-07` Coder | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass |
| `SCR-08` All tools | ~ (list; one action kind) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass with note |
| `SCR-09` Tool detail | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass |
| `SCR-10` Rankings | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass |
| `SCR-11` Profile | ✓ | ✓ (technical items under Advanced) | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | ✓ (show level) | n/a | n/a | n/a | ✓ | Pass |
| `SCR-12` Updates | ~ (one action per row) | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | n/a | n/a | ✓ | Pass with note |
| `SCR-13` Report a problem | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | ✓ | n/a | n/a | Pass |
| `SCR-14` Setup prompt | ✓ | ✓ | ✓ | ✓ (Not now) | ✓ (triggered after) | n/a | n/a | n/a | ✓ | n/a | n/a | n/a | n/a | Pass |
| `PAT-01` States | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | ✓ | Pass |
| `SCR-15` New chat | ✓ (the composer, or the first-run card) | ✓ | ~ (`E05` NEW) | ✓ | ✓ | ✓ (first run) | ✓ (Cloud by default) | n/a | n/a | ✓ (first run) | ✓ | n/a | n/a | Pass with note |
| `SCR-16` Previous chats | ~ (list; one action kind) | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Pass with note |
| `SCR-17` Conversation | ✓ (the card or offer, else the composer) | ✓ (answers may name vendors when asked) | ✓ | ✓ (Try again) | ✓ | ✓ | n/a | ✓ (cards) | ✓ | n/a | ✓ | ✓ | ✓ | Pass with note |
| `SCR-18` Wrong answer | ✓ | ✓ | ✓ | ✓ (Cancel) | ✓ | n/a | n/a | n/a | ✓ (says what is sent) | n/a | ✓ | n/a | n/a | Pass |
| `SCR-19` Coder on a computer | ✓ (the composer) | ~ (queue, long press) | ✓ (phase line) | ✓ | n/a (only after connecting) | n/a | n/a | ✓ | ✓ (Approve, Deny) | n/a | ✓ | n/a | n/a | Pass with note |
| `SCR-20` Add to the Gym | ✓ | ✓ | ✓ | ✓ (Not now) | ✓ | n/a | n/a | ✓ | ✓ (says what becomes public) | n/a | ✓ | ✓ | ✓ | Pass |
| `SCR-21` Test set | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | n/a | n/a | n/a | ✓ | ✓ | Pass |
| `SCR-22` Connect a computer | ✓ (the camera) | ✓ | ✓ (`E04`) | ✓ (each failure names its step; `E06` with no computer) | ✓ (only when a job needs a computer) | n/a | ✓ (full permission, only from the owner's unlocked screen) | n/a | n/a | n/a | ✓ | n/a | n/a | Pass |
| `SCR-23` Connected | ✓ | ✓ | ✓ | ✓ | n/a | n/a | n/a | ✓ | n/a | n/a | ✓ (Run Coder in the chat) | n/a | n/a | Pass |
| `DSK-01` Connect a phone | ✓ (scan) | ✓ | ✓ | ✓ | ✓ (no terminal step) | n/a | ✓ (full permission; Remove narrows) | n/a | n/a | ✓ | n/a | n/a | n/a | Pass |
| `DSK-02` Connected | ✓ | ✓ | ✓ | ✓ (says what is missing) | ~ (an agent sign-in, assumed done) | n/a | ✓ | ✓ | n/a | n/a | n/a | n/a | n/a | Pass with note |
| `DSK-03` Home | ~ (status page) | ✓ | ✓ | ✓ | n/a | n/a | n/a | ✓ | ✓ (Remove confirms) | n/a | n/a | n/a | n/a | Pass |
| `DSK-04` Nearby | ✓ | ✓ | ✓ | ✓ (Don't connect) | ✓ | n/a | ✓ (full permission; Remove narrows) | n/a | ✓ (compare the code) | n/a | n/a | n/a | n/a | Pass |
| `DSK-05` Menu bar | ~ (a menu) | ✓ | ✓ | ✓ | n/a | n/a | n/a | ✓ (status line) | ✓ (Stop confirms) | n/a | n/a | n/a | n/a | Pass with note |
| `CARD-01` Tool | ✓ | ✓ | ✓ (time, cost, runs left) | ✓ (no runs left: tomorrow, or a check) | ✓ | ✓ | ✓ (preselected) | ✓ | n/a | n/a | ✓ | n/a | ✓ (latest result sourced) | Pass |
| `CARD-02` Draft | ✓ | ✓ | ✓ | ✓ (Change it) | ✓ | n/a | ✓ (drafted for you) | ✓ | n/a | n/a | ✓ | ✓ | n/a | Pass |
| `CARD-03` Run | ~ (no primary while running) | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | ✓ (Stop confirms) | ✓ | ✓ | n/a | ✓ | Pass with note |
| `CARD-04` Result | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | ✓ (via `SCR-20`) | ✓ | ✓ | ✓ | ✓ | Pass |
| `CARD-05` News | ✓ (one offer) | ✓ | ✓ | ✓ (empty says so) | ✓ | n/a | n/a | n/a | n/a | n/a | ✓ | n/a | ✓ (sources) | Pass |
| `CARD-06` Check | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | n/a | ✓ | n/a | n/a | ✓ | n/a | ✓ | Pass |
| `CARD-07` Credit | ✓ | ✓ | ✓ | ✓ (empty says how to earn) | ✓ | n/a | n/a | ✓ | n/a | n/a | ✓ | n/a | ✓ (from the ledger) | Pass |

### What today's app still fails

Checked against `main` and TestFlight build 20 on 2026-09-28. Build 21
fixes the rows marked *Fixed in build 21*.

| Check | Where | What fails today |
| --- | --- | --- |
| `CHK-05`, `CHK-06` | Verse > the Gym board (`VerseGym.swift`) | Starting a Gym run needs a Gym host: the player copies their public key to a host, creates a connection grant there, and pastes a `gym-connect:` code. *Fixed in build 21* for tests: they run from chat on the hosted runner. The board's own runs still need a host. |
| `CHK-05` | Tutorial quests (`docs/verse/tutorial-quests.md`) | Earning XP by reproducing a result needs a desktop with Docker and the command line. |
| `CHK-02` | Verse > the Gym's RESULTS board (`VerseResults.swift`, `VerseTrace.swift`) | Readable with no setup, but in expert words: Terminal-Bench, traces, verifiers, Jev. |
| `CHK-02` | Account > Trainer | The level card is there, but the same screen shows npub, nsec, relay, and `microcoder xp` commands. |
| `CHK-02` | Account > Identity keys, Tailnet | npub, nsec, and hex keys, and a Tailnet screen, one tap from the Account list. |
| `CHK-02` | Wallet tab | The ₿ balance is on the tab's first screen. |
| `CHK-01`, `CHK-03`, `CHK-10` | The whole app | Four equal tabs, no main menu, no guided first run, and no "Next:" line on any screen. *Fixed in build 21*: the menu, the first run, and next-step lines. |
| `CHK-03` | Chat, `SCR-15` | An empty new chat has no line that says what to do; the suggestion chips are recent chats and workspaces, not first-time questions. *Fixed in build 21.* |
| `CHK-11`, `CHK-13` | Chat | The router has no Gym or eval routes, no cards, and no Gym screen to open: asking "What's new in the Gym?" gets a general answer, not records. *Fixed in build 21* (`chat-router-v2`). |
| Voice | Chat, `SCR-15.E09`, `SCR-17` | Wait and limit lines say "Coder has answered all the messages it can today"; the waiting label says "Coder is thinking". Both should speak as OpenAgents. *Fixed on `main`*: the lines now speak as "we". |

This spec fixes the rest by putting the loop in chat, running tests on our
computers, and moving every technical term behind **Advanced**.

## Minimal v1 cut and later additions

### v1: the most basic elements to build first

| ID | What v1 includes |
| --- | --- |
| `SCR-02` | The Coder card (preselected) and **CHOOSE CODER**. |
| `CIN-01` | The short cut: `S02`, `S04`, `S05`, `S07`, `S08`, subtitles, the end card with **LET'S GO**. |
| `SCR-15.E12` | The first-run chat: the greeting and `CARD-01` for Project map. |
| `CARD-01` to `CARD-07` | All seven cards; `CARD-06` once a first result exists to check. |
| `SCR-05` | The result detail with the test list. |
| `SCR-20`, `SCR-21` | Add to the Gym, and the test set. |
| `SCR-06` | Level up. |
| `SCR-01` | Logo, player card, hero with Gym status, **CHAT WITH OPENAGENTS**, starter chips, **PROFILE**, season card, footer, next-step line. No bell, Coder row, Rankings row, or Verse row. |
| `SCR-11` | Name, level, XP, titles, what you made, show my level, **Report a problem**. |
| `SCR-13` | One text box, screenshot, **Share this chat**, **SEND**. |
| `SCR-15` to `SCR-19` | Chat as it exists, plus the welcome lines (`SCR-15.E05`), first-time chips (`SCR-15.E06`), and wait and limit lines in our voice (`SCR-15.E09`). |
| `CHAT-2` to `CHAT-5`, `CHAT-9` to `CHAT-12`, `CHAT-14` | The Gym and eval routes and their cards. |
| `FLOW-01` to `FLOW-05`, `FLOW-07`, `FLOW-09`, `FLOW-10` | The flows these support. |
| `PAT-01` | The shared failure pattern. |

New work v1 depends on, outside the screens (the
[evals epic](../extensions/evaluation.md#delivery) tracks each):

1. **The eval engine** (`openagents ext eval`): cases, graders, both arms,
   the sandbox, the report, and the `ext-eval-v2` gate.
2. **A hosted runner** that runs a test set for a tool on our computers,
   with no player setup, within a daily quota.
3. **A starter test set** for each catalog tool (Project map, Code finder,
   Test reader), written with the interview and checked by us.
4. **Chat routes and cards**: `gym.news`, `eval.run`, `eval.author`,
   `eval.check`, `eval.result`, `eval.credit`, the Gym knowledge source, and
   the NIP-CJ `card` feedback.
5. **Publishing**: suites as NIP-EXT releases and results as NIP-EVAL
   `3189` publications, signed.
6. **Credit**: the `eval-check` and `eval-adopt` XP rules, the referee that
   signs them, and the ledger reading them.
7. **Trainer names** derived from the world key, and a "testing now" count.
8. **The guided first-run chat** and the cinematic's camera path.

### Later additions

| ID | Addition |
| --- | --- |
| `SCR-07` | Coder: its starter test results over time and the tools it uses now. |
| `SCR-08`, `SCR-09` | All tools and tool detail; tools and test sets made by trainers. |
| `SCR-10` | Rankings, weekly and season. |
| `SCR-12` | Updates, then push notifications. |
| `SCR-14` | Just-in-time setup: save your progress, connect your computer, open your wallet. |
| `SCR-01.E14` | The Gym in the Verse as a social place ([below](#later-the-gym-in-the-verse)). |
| `CIN-01` | The full cut with voice-over, and bitcoin in the narration once rewards pay. |
| — | More agents on `SCR-02` once a second agent exists. |
| — | Tools with new code made from chat end to end (today chat hands that to Coder on a connected computer). |
| — | Paid rewards for tools Coder adopts, through funded quest purses and the existing Wallet, introduced by `SCR-14`. Never XP converted into money. |
| — | Group checks ("raids") and parties from the trainer leveling design. |

## Later: the Gym in the Verse

Not in this implementation wave. The owner's direction: chat is where
people work with the Gym and evals; the Gym building in the Verse is for
reviewing results and for being together.

- **Reviewing results.** The Gym's boards show published results, grouped
  by tool and test set, with checks and adoptions, from the same verified
  records chat reads. It exists today for our benchmark results (the
  RESULTS board); it gains eval results.
- **Agents comparing notes.** A trainer's agent in the Gym can talk with
  other trainers' agents over chat: it reads their published test sets and
  results, asks why a tool helped one project and not another, proposes a
  joint check, and reports back to its trainer in their own chat, with
  every claim sourced to published records and every action still a tap
  by a person.
- **Being there.** Trainers see who is testing what, gather at a board
  when a result lands, and run group checks together.

Implemented in v1 by [#9942](https://github.com/OpenAgentsInc/openagents/issues/9942)
([the Gym building](../verse/gym.md#the-evals-board-and-agents-comparing-notes)):
the Grid Gym's **EVALS** board shows verified eval results by test set and
tool with checks and credit, and with **Compare notes** on, agents trade one
opener and one answer about their published results, rendered by each reader
from the cited records. Still later: proposing a joint check, reporting back
in the trainer's own chat, and group checks.

## Appendix: what we cut and why

Cut from this experience, not deleted from the code. Each can come back when
it serves the loop.

| Cut | Where it lives today | Why |
| --- | --- | --- |
| The four-tab bar (Chat, Verse, Wallet, Account) | `AppTabs.swift` | A hub with one primary action is simpler than four equal tabs. Chat is not cut: it is the menu's primary (`SCR-01.E12`). Verse, Wallet, and Account don't serve the first loop as tabs. |
| **BITCOIN WALLET** row and the ₿ balance pill | Wallet tab | No step of the loop pays or spends, and money on the main menu invites confusion (sats versus ₿). It returns when rewards pay. |
| Walking the Grid, the sticks, and the physics toys | Verse tab | Steering a 3D world to reach a board isn't IDIOT PROOF and costs taps. The Grid stays as the hero image, the cinematic's set, and the later social Gym. |
| The results board, trace viewer, caveats, and Jev tabs | `VerseResults.swift`, `VerseTrace.swift` | Built for experts. The player sees one with-and-without result instead; the boards stay in the Verse for review. |
| The live Gym board with `gym-connect:` pairing and recipes | `VerseGym.swift` | Needs a host and a pasted code. Replaced by the hosted runner. |
| The Gym as separate pages (`SCR-03`, `SCR-04`) | Revision 2 of this spec; the mockup | Revision 3: the loop happens in chat, so the pages became cards. |
| **ENTER THE GYM** as the main button | Revision 2; the mockup | Revision 3: the main button is **CHAT WITH OPENAGENTS**, where every Gym step starts. |
| Computers, Tailnet, Terminal, and Agent payments | Account, Wallet | Setup for your own computer; not part of the first loop. |
| Identity keys, Reveal nsec, Link a key, Export card JSON | Account | Technical; moved behind **Advanced** or `SCR-14`. |
| Changelog and "What to test" | Account | Useful to testers, but not to the loop. The changelog feeds `CARD-05` instead. The version stays in the footer for reports. |
| **My reports** | Account > Playtest | The list of past reports isn't needed to play. |
| **Source code** and **Follow us on X** links | Account | Building a following happens through **Share outside the app** on a result and **Share what you made** on the credit card. |
| Tutorial quests run from a desktop | `docs/verse/tutorial-quests.md` | Replaced on the phone by **Check a result**, the same idea with no setup. |

Back from the cut in revision 2:

| Was cut | Now | Why it came back |
| --- | --- | --- |
| Chat with Coder on your computers | `SCR-15` to `SCR-19`, `SCR-01.E12` | Chat needs no computer: OpenAgents answers every new chat, and Coder is dispatched only when a message needs a computer. |
| **CHAT WITH AN AGENT** row from the mockup | **CHAT WITH OPENAGENTS** (`SCR-01.E12`) | The same row, in our voice; in revision 3 it is the primary. |

## Sources

- [Extension evaluation](../extensions/evaluation.md): the eval engine, the chat product path, publishing, checks, and credit.
- [Launch roadmap, 2026-09-29](../roadmap/2026-09-29-launch-roadmap.md): the launch plan and the evals milestone.
- [Playtesting program](../game/playtesting.md): Season 1, sessions, and playtest XP.
- [Agent trainer leveling](../verse/agent-trainer-leveling.md) and [tutorial quests](../verse/tutorial-quests.md): XP, levels, titles, and quest rules.
- [The Gym building](../verse/gym.md) and [Gym leaderboard](../verse/gym-leaderboard.md): run observation, recipes, and published results.
- [Wasm plugins](../extensions/plugins.md) and [packages](../extensions/packages.md): the plugin host, evidence guests, and packages.
- [Chat router design](../coder/design/2026-09-28-chat-router.md): routes, the answer bank, offers, and what is implemented.
- [Chat worker deployment](../deployment/chat-worker.md): the service that answers chat.
- Answer bank: `crates/coder/answers/chat-answers-v1.toml`; product notes: `knowledge/openagents/`.
- [Verse on the phone](../verse/mobile.md): the Grid on iOS and Android.
- [NIP-EVAL](../../nips/openagents/NIP-EVAL.md), [NIP-XP](../../nips/openagents/NIP-XP.md), [NIP-EXT](../../nips/openagents/NIP-EXT.md), [NIP-CJ](../../nips/openagents/NIP-CJ.md).
- App code: `bins/openagents-ios/host/App/`, `crates/openagents-mobile/src/`, `bins/openagents-android/README.md`, `crates/verse/src/`.
