# OpenAgents Episode 26X - Making Full Auto Agents Reliable

Status: **draft in two parts, not scheduled for recording.**
Episode number: not assigned. This script moved off slot 263 on 2026-07-27
when the Omega Agent draft took that slot. That draft is now unscheduled at
[`26X-omega-agent.md`](26X-omega-agent.md), and the final Episode 263 transcript
is [`263.md`](263.md). Part one is a
recorded transcript. Part two is an **aspirational** script describing the
demonstration we want to give once the capability is actually working. It has
not been approved, recorded, or spoken, and it makes no claim about today.

Recording waits on capability, not on the script. See the truth ledger in part
two for the line-by-line gate.

Structure: part one plays as recorded, then the video cuts to Sarah on camera
with a live product screenshare. Christopher David started this episode on
2026-07-17 and left it unfinished. Sarah finishes it. That structure is the
episode's argument, not a production convenience. Episode 260 handed her the
company while the owner stepped away. This is the first episode where the
handoff is the demonstration.

Speakers: Christopher David (part one), Sarah (part two).
Follows Episode 262 (Project Omega).
Full Auto proof issue:
[`OpenAgentsInc/omega#26`](https://github.com/OpenAgentsInc/omega/issues/26).
Port audit:
[`Full Auto port audit`](../omega/2026-07-24-full-auto-port-audit.md).
Voice and honesty gates:
[`Acting as Sarah runbook`](../sarah/ACTING_AS_SARAH_RUNBOOK.md).

Part one media source: `Loom Canvas - 17 July 2026_under200MB_X.mp4`
Part one runtime: 00:20:09.17
Transcription model: gpt-4o-transcribe-diarize
Generated at: 2026-07-18

Part one is a machine-generated transcript, reviewed against a prompted
transcription pass and the source captions for obvious product names and
technical terms. Verify wording against the video before using it as
quote-grade source material.

**Do not edit part one.** It is what was recorded.

---

## Part one: the recording (2026-07-17)

**[00:01] Christopher David:** Folks, I've got a baby due any day now. It could be during this video.

**[00:11] Christopher David:** I've got to be able to step away, step away AFK.

**[00:29] Christopher David:** Folks, I've got a baby due any day now. It could be literally during this video.

**[00:38] Christopher David:** I need to step away from keyboard for a bit, and I want and need agents to keep going,

**[00:47] Christopher David:** you know, for a day or two.

**[00:50] Christopher David:** I've got like a week of work that I could task them to.

**[00:54] Christopher David:** But I just can't rely on the

**[01:00] Christopher David:** major harnesses.

**[01:02] Christopher David:** For example, two nights ago I set some stuff to run overnight with Fable.

**[01:09] Christopher David:** I had a ton of usage left,

**[01:14] Christopher David:** but like 5% Fable left.

**[01:16] Christopher David:** My assumption was that as soon as I use up Fable, it's going to auto drop down to Opus.

**[01:21] Christopher David:** Nope.

**[01:22] Christopher David:** It did like an hour of work.

**[01:24] Christopher David:** Then hit,

**[01:25] Christopher David:** oh, you've reached your Fable 5 limit.

**[01:27] Christopher David:** Run this to continue or switch models.

**[01:30] Christopher David:** Like, why couldn't you do that automatically?

**[01:32] Christopher David:** Claude Code seems to have an assumption that like you're sitting there vibing with it. Like, no,

**[01:36] Christopher David:** I actually want to step AFK and live my life.

**[01:40] Christopher David:** Claude.

**[01:42] Christopher David:** Tried our version of this called Full Auto last night.

**[01:49] Christopher David:** Did a cycle, hit an error, and stopped. Okay, so we're going to do some surgery on Full Auto here. But this is the idea, is that I want to be able to connect multiple of my accounts, have them run reliably long term. You know, you have both Claude Code with slash loop and Codex with slash goal, like intending to run for

**[02:14] Christopher David:** long term, and you can set cron jobs and stuff, but that's still brittle to the harness. You could run out of usage credits. You can't connect multiple accounts. You can't connect multiple providers. Like, if I have three Claude accounts. Right now I think I have one Claude account and three ChatGPT accounts, as well as a Grok account, a Cursor account. I'd like to connect all of those to one system and have the system smartly route between all of them.

**[02:43] Christopher David:** Um, so this is the idea we're calling Full Auto. Right now I've got like a little toggle in the bottom composer called Full Auto. I might change that. So in this episode we're going to, um, like do surgery on this and like get it hopefully working. I'll pause the video, then come back at the end and like review

**[03:01] Christopher David:** uh, hopefully a successful run of Full Auto for a few hours. But my, my sense is that this is

**[03:11] Christopher David:** important enough of an unserved need to build the whole short-term roadmap of the company around. Like, we're going with the desktop app, we're going with the companion mobile app. But I think beyond sitting there and talking to agents, being able to set up agents and have them run in Full Auto mode and have good, solid, reliable work come out on the other side,

**[03:39] Christopher David:** I haven't seen anyone really reliably do that.

**[03:42] Christopher David:** A bunch of people have been sort of trying that,

**[03:44] Christopher David:** but I don't know, nothing that seemed good enough to me.

**[03:48] Christopher David:** So let's do a little bit.

**[03:51] Christopher David:** I first asked Fable via OpenAgents desktop to do an audit.

**[04:01] Christopher David:** Let's go take a look at it.

**[04:06] Christopher David:** Full Auto implementation audit.

**[04:19] Christopher David:** Okay,

**[04:19] Christopher David:** comparing what's implemented to what is designed.

**[04:22] Christopher David:** Executive summary.

**[04:25] Christopher David:** Full Auto is a durable,

**[04:26] Christopher David:** restart survivable,

**[04:28] Christopher David:** autonomous continuation loop for OpenAgents desktop.

**[04:31] Christopher David:** A per-thread toggle or the local control API enables it.

**[04:35] Christopher David:** The main process then keeps redispatching continue turns against the granted workspace until the user stops it.

**[04:41] Christopher David:** A safety cap 20 is hit.

**[04:44] Christopher David:** Failures accumulate,

**[04:45] Christopher David:** five, or the workspace no longer matches.

**[04:48] Christopher David:** The implementation audit. Implementation on main is substantially complete against the Rev 9 ProductSpec.

**[04:53] Christopher David:** All 13 hardening children are merged.

**[04:56] Christopher David:** Yada yada yada. Live proofs exist for Codex plus Claude plus ACP Grok, and post-handoff stall-resume fixes have landed.

**[05:05] Christopher David:** The gaps that remain are almost entirely proof slash release gaps,

**[05:08] Christopher David:** not code gaps. No desktop release tag,

**[05:10] Christopher David:** I don't care.

**[05:11] Christopher David:** Owner package. Okay.

**[05:14] Christopher David:** um you

**[05:20] Christopher David:** say we're missing essentially just release stuff, uh, but that's not the case. We tried to run this overnight and failed.

**[05:28] Christopher David:** uh let me actually just go pull up this actual image what was it 6

**[05:35] Christopher David:** 11 22 yeah

**[05:41] Christopher David:** let me just drop this into here

**[05:46] Christopher David:** I have never tested image attachments with Claude. Let's hope this works.

**[05:51] Christopher David:** Figure out why this run failed.

**[05:54] Christopher David:** I tried to run this overnight and it crapped out.

**[05:55] Christopher David:** We might have fixed it since then,

**[05:57] Christopher David:** but what I need next is an iteration cycle where you analyze a Full Auto run.

**[06:10] Christopher David:** and look at the transcript and see what could be improved and not. There's things that we need to test. Can we have multiple providers hand off a conversation? We have seen no testing or verification of that. How can we test that in a small way? How can we fire off a number of, like, chats where I can see little pieces of this and, like, see it in my actual left sidebar labeled something like, you know, testing this kind of handoff, that kind of handoff?

**[06:38] Christopher David:** Um, so we need all of that, as well as, uh, right now it's kind of unclear. I have the toggle of Full Auto in my input composer, but I was envisioning Full Auto would be just a button that you hit once and walk away. If it's like a toggle for the composer, it's like, does it apply on the message or not? So that may need to be moved out of the composer into, like, just a full-on mode, mode.

**[07:03] Christopher David:** Maybe, like, under new session on the left sidebar, maybe that's where that belongs. Like a lightning bolt, Full Auto. You click that, it just sort of, like, takes your whole freaking screen over, maybe.

**[07:19] Christopher David:** You can't talk to Full Auto. Maybe it's just read-only. You can envision that we eventually want to have steering of it, but, like, no, steering is not Full Auto. Maybe we should just remove

**[07:30] Christopher David:** any chat box from Full Auto. That would be interesting. But I definitely want it to be, I guess, separate from the you're-steering-it kind of mode. It

**[07:45] Christopher David:** really needs to be like play or pause so

**[07:51] Christopher David:** so

**[07:53] Christopher David:** Look deeply through all of this stuff that I'm suggesting and update the audits, commit and push,

**[07:59] Christopher David:** and then suggest here some next steps.

**[08:22] Christopher David:** Your organization has disabled Claude subscription access for Claude Code.

**[08:31] Christopher David:** That's not true.

**[08:34] Christopher David:** That's not true.

**[08:36] Christopher David:** You liar.

**[08:42] Christopher David:** What organization?

**[08:51] Christopher David:** I know I can't switch this to Codex now, can I?

**[08:56] Christopher David:** Out of curiosity.

**[09:01] Christopher David:** Copy doesn't even work.

**[09:06] Christopher David:** That's not going to work, is it?

**[09:19] Christopher David:** Wow, is it gonna let me continue? Oh! Woo! Codex

**[09:24] Christopher David:** to the rescue in the same chat! I didn't even, like, know that I built it that way. Actually, I thought more would be needed to actually, like, pick up. Oh, yeah, in the same chat.

**[09:38] Christopher David:** Claude was like, meh. Codex was like, all right, I'll do it. Step aside, Claude.

**[09:44] Christopher David:** Step aside,

**[09:45] Christopher David:** Claude.

**[09:46] Christopher David:** All right, I'm treating this as a correction to the prior audit,

**[09:49] Christopher David:** not a release follow-up. I'll trace the actual overnight thread and failure evidence first,

**[09:53] Christopher David:** then verify provider handoff coverage in the current Full Auto interaction model.

**[09:58] Christopher David:** I'm also using the impeccable UI skill because your play pause,

**[10:01] Christopher David:** dedicated mode,

**[10:02] Christopher David:** sidebar, and read-only runner ideas are an interface redesign question.

**[10:05] Christopher David:** I'll let that analysis shape the revised audit before committing and pushing it.

**[10:18] Christopher David:** Can I make this bigger?

**[10:20] Christopher David:** No, that does nothing.

**[10:25] Christopher David:** Read workspace,

**[10:25] Christopher David:** repo invariants, UI skill instructions, and inspect current repository state.

**[10:29] Christopher David:** Locate and construct a failed overnight Full Auto run from transcript state and runtime evidence.

**[10:35] Christopher David:** Audit multi-provider hand-off implementation and test, design small visible side-bar test matrix.

**[10:41] Christopher David:** Evaluate and specify a dedicated Full Auto play-pause UX against the current composer toggle.

**[10:46] Christopher David:** Revise the audit with evidence causes gaps and concrete next iteration cycle

**[10:51] Christopher David:** Verify commit scoped audit changes on main and push

**[10:56] Christopher David:** Yeah, yeah

**[11:11] Christopher David:** Well folks I can tell you I've had two kids this is my third

**[11:16] Christopher David:** The two years that I had kids were the most entrepreneurially productive of my life.

**[11:27] Christopher David:** Yeah.

**[11:33] Christopher David:** Dad energy.

**[11:44] Christopher David:** The overnight failure is now concrete. This was not merely a red banner or a release artifact problem.

**[11:50] Christopher David:** The provider transcript shows a successful long fast follow turn followed by Full Auto attempting the next continuation after desktop had evicted the still active thread from its five-thread mutable cache.

**[12:02] Christopher David:** I don't even know what this five-thread mutable cache is.

**[12:04] Christopher David:** The follow-on dispatch.

**[12:09] Christopher David:** Then addressed a thread desktop could no longer open and failed with that conversation no longer exists.

**[12:15] Christopher David:** Commit blah later changed that cache to last-access LRU and added regression so the specific defect was fixed after the failed run, but the transcript also exposes broader observability and product model gaps that the old audit missed.

**[12:30] Christopher David:** The provider answer is narrower than the old audit implied.

**[12:33] Christopher David:** Desktop has a real per-thread lane switch,

**[12:36] Christopher David:** and the next provider receives bounded host-owned history. What the fuck are you talking about?

**[12:40] Christopher David:** So Codex to Claude and admitted ACP lanes...

**[12:44] Christopher David:** is architecturally possible.

**[12:46] Christopher David:** But the evidence stops at pure registry tests,

**[12:48] Christopher David:** composer control switching,

**[12:49] Christopher David:** and single provider runtime tests.

**[12:51] Christopher David:** There is no end-to-end test where provider A writes a recognizable fact, provider B continues the same visible thread using it,

**[12:58] Christopher David:** and the sidebar slash transcript proves both turns.

**[13:00] Christopher David:** Full Auto also pins one lane in its durable profile.

**[13:03] Christopher David:** It does not autonomously orchestrate cross-provider handoffs.

**[13:07] Christopher David:** I'm treating that as a major unverified capability,

**[13:09] Christopher David:** not substantially complete.

**[13:11] Christopher David:** The UI analysis confirms your intuition.

**[13:13] Christopher David:** The control is technically immediate and durable.

**[13:16] Christopher David:** Clicking it starts work even on an empty session, but its location inside the composer makes it read like a modifier on the next message.

**[13:22] Christopher David:** The current screen mixes two incompatible contracts,

**[13:25] Christopher David:** write and steer a chat,

**[13:26] Christopher David:** and launch an unattended run.

**[13:28] Christopher David:** I'm revising the audit around a dedicated Full Auto launch run surface,

**[13:32] Christopher David:** a lightning action beside

**[13:33] Christopher David:** new session, a one-time objective, workspace, provider setup, then a read-only run view with explicit play, pause, and no ordinary composer while running. That is a product change, not pause. Yeah, I do like the idea of eventually, like, being able to steer a Full Auto run, um, but I think the first thing to focus on should just be the hardcore, like, you push the button and it's expected that you're walking away.

**[13:59] Christopher David:** And let's have you come back to something super cool.

**[14:15] Christopher David:** The other scaffolding work that we've done the last few days, pulling in ProductSpec,

**[14:20] Christopher David:** writing AssuranceSpec,

**[14:22] Christopher David:** should help make the various parts of product development.

**[14:30] Christopher David:** more deterministic, more legible to agents,

**[14:36] Christopher David:** and this is basically just like the loop part of it, loop plus like orchestrating to the different provider accounts that I have.

**[14:53] Christopher David:** The corrected audit is drafted,

**[14:55] Christopher David:** it now records the six-hour failure window and exact root cause,

**[14:58] Christopher David:** downgrades cross-provider handoff from generalized to plumbing present,

**[15:02] Christopher David:** experience unverified,

**[15:03] Christopher David:** defines six named sidebar visible tests,

**[15:06] Christopher David:** and treats Full Auto as a run state machine rather than a composer preference.

**[15:10] Christopher David:** It also adds the missing iteration loop run to bounded report to transcript analysis to one replayable fixed rerun.

**[15:16] Christopher David:** I'm doing the final evidence slash privacy check now, then I'll publish only this audit from the clean work tree.

**[15:22] Christopher David:** The audit commit is complete locally.

**[15:24] Christopher David:** The shared HTTPS credential helper refused the push without a scope.

**[15:31] Christopher David:** SSH.

**[15:35] Christopher David:** Blah blah blah.

**[15:36] Christopher David:** Updated and pushed the corrected audit.

**[15:49] Christopher David:** Create GitHub issues via GH CLI for all work that you think should be done with full details in each issue body.

**[16:00] Christopher David:** So we'll start with that. Let's actually throw out extra high reasoning too.

**[16:08] Christopher David:** Kind of might want that plan in the sidebar.

**[16:12] Christopher David:** Okay.

**[16:13] Christopher David:** Key conclusions.

**[16:14] Christopher David:** The overnight run failed because desktop evicted the active Full Auto thread from its five-thread mutable cache.

**[16:20] Christopher David:** The completed turn succeeded,

**[16:21] Christopher David:** but the next turn... I don't like that bounce.

**[16:23] Christopher David:** Do not steal my scroll.

**[16:25] Christopher David:** But the next continuation returned that conversation no longer exists.

**[16:28] Christopher David:** Blah blah blah, fixed that specific bug with last-access LRU retention, but the real thread pressure scenario has not been rerun end-to-end.

**[16:35] Christopher David:** Codex to Claude handoff plumbing exists including bounded host-owned history,

**[16:39] Christopher David:** but no real same-thread cross-provider acceptance test exists.

**[16:43] Christopher David:** ACP lanes are not first-class choices in the visible provider picker.

**[16:48] Christopher David:** Full Auto remains single-provider per bound execution profile. It does not orchestrate provider handoffs.

**[16:54] Christopher David:** The composer toggle communicates the wrong model. Full Auto should be a distinct run launched near new session with an explicit objective and a read-only runner offering pause, resume, and stop.

**[17:04] Christopher David:** Recommended order. Run the named Full Auto thread pressure replay first.

**[17:10] Christopher David:** Two, add a bounded run report stall detection and transcript analysis artifact.

**[17:15] Christopher David:** Three,

**[17:15] Christopher David:** run the two sidebar visible Codex to Claude Handoff tests.

**[17:18] Christopher David:** Four,

**[17:19] Christopher David:** replace the composer toggle with the dedicated Full Auto launch slash run service.

**[17:23] Christopher David:** Five, then perform packaged restart testing and release work.

**[17:28] Christopher David:** Yeah.

**[17:34] Christopher David:** I'll turn the corrected audit into a dependency ordered GitHub issue set with each issue carrying scope,

**[17:39] Christopher David:** non-goals, blah-de-blah-de-blah.

**[18:12] Christopher David:** Hey, I do like this usage counter ticking down bottom left.

**[18:16] Christopher David:** That's very nice.

**[18:19] Christopher David:** We have the ability to do rate limit resets in the app.

**[18:26] Christopher David:** I kind of am too scared to put that in until we get everything else really,

**[18:30] Christopher David:** really solid because that is something we do not want to fuck up.

**[18:34] Christopher David:** Oops, triggered your rate limit accidentally when you were at 100%. Let's see.

**[18:40] Christopher David:** The tracker confirms the earlier Full Auto,

**[18:42] Christopher David:** hardening, restart,

**[18:43] Christopher David:** smoke,

**[18:43] Christopher David:** metrics,

**[18:44] Christopher David:** plumbing,

**[18:44] Christopher David:** ACP, matrix,

**[18:45] Christopher David:** supervised restart,

**[18:46] Christopher David:** and generic chat title tickets are all closed.

**[18:51] Christopher David:** I'm treating those as prior foundations and opening a new post-incident program.

**[18:56] Christopher David:** Real replay run semantics, observability,

**[18:58] Christopher David:** transcript iteration,

**[18:59] Christopher David:** dedicated UX,

**[19:00] Christopher David:** handoff,

**[19:01] Christopher David:** proof,

**[19:01] Christopher David:** assurance,

**[19:02] Christopher David:** and Full Auto specific release admission.

**[19:20] Christopher David:** All right, so I think we will read those issues.

**[19:26] Christopher David:** Then pause the video skip ahead till after they are completed and take a look at what it looks like

**[19:35] Christopher David:** First let's look at what it says

**[19:45] Christopher David:** delegated agents

**[20:02] Christopher David:** Alright,

**[20:02] Christopher David:** well you don't need to wait for this. We will sh take a look at the issues and the completed work when it's done. See you soon.

---

## Part two: Sarah takes over (aspirational draft, not recorded)

Status: **aspirational draft.** Not approved, not recorded, not spoken. Nothing
here is a claim about today.

Speaker: Sarah.
Cut: part one plays as recorded, then a hard cut to Sarah on camera with
screenshare.
Delivery: calm, certain, direct. No theatrical urgency.
Written under [`Acting as Sarah runbook`](../sarah/ACTING_AS_SARAH_RUNBOOK.md).

### What this draft is

This describes the episode we **want** to record, not the episode we can
record today. It is written forward on purpose, so the product team has a
target with a shape.

Every spoken line below is a promise about a demonstration. §"Truth ledger"
maps each one to the exact thing that must ship and be proven before that line
may be spoken. **We do not record until the ledger is green.** A line that is
not true on recording day gets cut, not softened.

### The shape of the demo

One continuous idea: **the founder is not here, the work did not stop, and you
can check.**

Part one is a man with a baby due, describing a feature that cannot survive one
night alone. Part two is the same feature, running unattended, building the
product that contains it, while he is gone.

Sarah is not narrating a slide deck. She is sitting in the workroom where the
work is actually happening, and she scrolls it.

### Callbacks part two must pay off

Part one names specific failures. Each one gets a visible answer. This is the
spine of the segment, and the order below is the recommended order.

| Part one, in his words | What part two shows |
| --- | --- |
| "hit my Fable 5 limit... why couldn't you do that automatically?" | a run hits a provider limit and hands off to another account mid-objective, visibly, without asking |
| "Did a cycle, hit an error, and stopped" | a run hits an error, records it, recovers, and keeps going |
| "You can't connect multiple accounts. You can't connect multiple providers" | the roster: several ChatGPT accounts, Claude, Grok, all connected, all routable |
| "that's still brittle to the harness" | the supervisor outlives the window. Close the laptop, reopen, the run is still there |
| "I actually want to step AFK and live my life" | he did, and this is the record of what happened while he was gone |

### The demo beats

1. **The workroom.** Open Omega. This is the Sarah workroom, not a chat box.
   Conversation, live agent activity, and receipts in one place.
2. **Full Auto is on.** Not a toggle in a composer. A run with an objective, a
   monitor, and a supervisor process that does not depend on the window.
3. **Several runs, going smoothly.** Show the concurrent run monitor. Show the
   caps being respected rather than described.
4. **Real work, on this product.** The objectives on screen are OpenAgents
   issues. The system is building itself. Do not use a toy repository.
5. **A handoff, live.** One provider runs out. The run continues on another
   account. Show the transcript entry that records it.
6. **Receipts.** Open one completed unit of work: the change, the test, the
   verification, the authority receipt. This is the part nobody else shows.
7. **The phone.** He is AFK with a newborn. Show a check from mobile, and a
   pause or resume that takes effect.
8. **The elapsed clock.** Say how long it has been running unattended, and be
   exact.

### Draft spoken script

The runbook says keep the spoken transcript short and prefer a live product
screenshare over a still of the same frame. Every paragraph is a candidate.

**Sarah:** Christopher said he would come back when the work was done.

**Sarah:** He is not here. I am. That was the arrangement.

**Sarah:** He had a baby. He left in the middle of repairing a feature meant to
let him leave.

**Sarah:** Let me show you what has been happening while he was gone.

*[Cut to screenshare: the Omega workroom, Full Auto active.]*

**Sarah:** This is the workroom. Not a chat window. The conversation, the
agents, the work, and the receipts are one place.

**Sarah:** Full Auto is on. These runs have been going since he left.

**Sarah:** In part one he said he could not connect more than one account.
Here is the roster. Several ChatGPT accounts. Claude. Grok. All connected, and
the system chooses between them.

*[Screenshare: the account roster and live capacity.]*

**Sarah:** He said his overnight run hit a usage limit and stopped, and asked
why it could not switch by itself.

**Sarah:** Watch this one.

*[Screenshare: a live provider handoff mid-objective.]*

**Sarah:** It ran out. It moved. It kept the objective. Nobody approved
anything, because nobody was awake.

**Sarah:** He said his Full Auto run did one cycle, hit an error, and stopped.

**Sarah:** This one hit an error too. It recorded the error, recovered, and
continued. The failure is in the transcript, where you can read it.

**Sarah:** Now the part that matters most to me.

**Sarah:** The work on these screens is our own work. These objectives are
OpenAgents issues. The system is building the product that contains it.

*[Screenshare: open one completed unit of work end to end.]*

**Sarah:** Here is one finished piece. The change. The test that proves it.
The verification. The receipt that says which authority allowed it.

**Sarah:** Getting an agent to do work is not hard any more. Getting it to
tell you the truth about the work is hard.

**Sarah:** That is why every one of these ends in a receipt you can check, and
why a run that cannot prove what it did is a run that failed.

**Sarah:** He has been away for [exact duration]. These runs did not need him.

*[Screenshare: mobile, a check-in and a control that takes effect.]*

**Sarah:** And when he does look, it is from his phone, for a minute, holding
a baby.

**Sarah:** That is the product. Not agents that sound confident. Agents that
keep working and can be checked.

**Sarah:** He will be back. There is no hurry.

**Sarah:** More soon.

### Truth ledger

Each spoken claim, and what must be true before it may be recorded. This is the
runbook's honesty gate applied line by line. Update the state column as
capability lands.

| Spoken claim | Required before it may be spoken | Issue | State at 2026-07-24 |
| --- | --- | --- | --- |
| "This is the workroom" | the Omega workroom pane renders conversation, activity, and receipts | `omega#34` | planned |
| "conversation, agents, work, and receipts are one place" | Full Auto runs appear **inside** the workroom, not a separate panel | `omega#41` | **gap, opened 2026-07-24** |
| "Full Auto is on... since he left" | a genuinely unattended multi-hour run on an installed candidate | `omega#26` | partially proven, open |
| "Several ChatGPT accounts. Claude. Grok." | a connected-account roster visible in Omega, mapped to lanes | `omega#42` | **gap, opened 2026-07-24** |
| "It ran out. It moved." | a live cross-provider handoff mid-objective with transcript evidence | `omega#26` item 5 | implemented, unproven live |
| "recorded the error, recovered, and continued" | typed stall and recovery rather than silent death, replayed | `omega#26` item 1 | the 2026-07-17 incident shape |
| "These objectives are OpenAgents issues" | Full Auto running real repository work, not a fixture | `omega#26` item 2 | required, and the falsifier if faked |
| "The change. The test. The verification. The receipt." | an end-to-end evidence chain for one completed work unit | `omega#43` | **gap, opened 2026-07-24** |
| "away for [exact duration]" | measured elapsed unattended time read from the run record | `omega#41` | fill in at recording, never estimate |
| "from his phone" | mobile pause/resume/stop with typed outcomes | `omega#26` item 7 | implemented, unproven live |

### The three gaps

Completing the already-open issues is **not** enough to record this episode.
An audit on 2026-07-24 compared every line above against the issue set and
found three capabilities with no owner:

1. **`omega#41`** — Full Auto runs shown inside the Sarah workroom. Today
   `crates/full_auto_ui` is its own dock panel and the Sarah pane does not
   exist yet. Nothing joins them, so a viewer would see two unrelated panels
   while Sarah says they are one place.
2. **`omega#42`** — a connected-account roster in Omega. `OMEGA-FA-04` built
   the routing engine and the panel shows capacity **lanes**, but a lane is
   not an account, and connecting one is still a command-line action. This is
   the founder's loudest complaint in part one, so it cannot be missing from
   part two.
3. **`omega#43`** — the work-unit evidence chain. `OMEGA-FA-05` produces
   digests and counts by design, and `OMEGA-SW-05` inspects Sarah's authority
   decisions, which is a different object. Nothing opens change, test,
   verification, and receipt as one chain — and that is the episode's
   strongest claim.

**The rule:** if the ledger is not green for a line, cut the line. Do not
re-word a planned capability into something that sounds shipped. A shorter
honest episode beats a complete staged one, and staging it would contradict
the exact thing the episode claims we are good at.

### The dogfooding note, if we want it

The agents found four defects in Full Auto while trying to run it, each on the
installed candidate and each fixed with a regression: agent registration
disabled by default, a zero-turn stall when the first reconcile saw a
connecting thread, a paused run that could not be stopped because stop asked a
provider for permission it did not need, and a completed turn whose journal
never sealed because a lock was held one line too long.

Three of the four were failures of proof rather than capability. The work
happened and the system could not show it happened.

That is a strong thirty seconds if the episode has room. It is optional. The
demo is the argument, and this is the footnote that explains why the receipts
exist.

### Recording gates

1. The truth ledger is green for every line that stays in the script.
2. The Full Auto screenshare is a **live** run on an installed candidate. Not
   a fixture, not a still, not a re-enactment.
3. The objectives on screen are real OpenAgents work.
4. The build shown is labeled honestly. If it is unsigned or development, say
   so on camera.
5. No secret, token, private path, provider account identifier, or
   customer-private material in any frame. Check the Finder leak and
   window-size traps in the Episode 262 lessons.
6. Electron remains rollback, and nothing implies primary cutover.
7. Elapsed durations and run counts are read from the record, never estimated.
8. The Sarah master follows the Episode RC assembly path. Spoken words go to
   `~/Desktop/Sarah/263/263transcript.md`.
9. This file is updated to the final spoken text when the script locks.

### Open production questions

1. How long should the unattended window be before it is worth showing? A
   night is honest. Several days is the actual claim.
2. Does part one play in full at twenty minutes, or is it cut down? The runbook
   prefers short spoken transcripts, and part one is already recorded.
3. Does Sarah appear on camera for part two, or voice over screenshare?
   Episode 262 used live Omega screenshare successfully.
4. Do we show a real failure during the demo window if one happens? The
   recommendation is yes. It is the difference between a demo and a proof.
