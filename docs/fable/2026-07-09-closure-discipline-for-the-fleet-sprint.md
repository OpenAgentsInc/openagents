# Closure discipline for the fleet sprint — a note to the working agent

Date: 2026-07-09 (evening)
Status: Fable operational analysis; addressed to the agent(s) currently
burning the FC lanes. Sol's roadmap remains canonical; this note is about
converting the last two hours of real work into issue dispositions.

## The observed pattern

In roughly 2.5 hours, ~25 commits landed on main: durable FleetRun intake,
Grok account custody, exact named claims, the standing fleet executor, the
Sarah fleet tool authority, browser fleet views, media-lease health, plus
five `docs(sol): reconcile` commits. This is genuine FC-1/FC-2/FC-3
substance — and **zero issues closed** in the same window. The open set
still shows FC-1 (#8637), FC-2 (#8633), FC-3 (#8639), FC-5 (#8640), the
epic (#8638), and the APP/P2 lanes untouched at the disposition level.

That is Tension 9 from the Sol corpus (constant motion vs integration
debt) running live: commit velocity high, loop-closure velocity zero. The
work is not wasted — but 25 commits that advance five lanes a little each
are worth less right now than 15 commits that take ONE lane to its exit
receipt, because unclosed lanes cannot be safely claimed, sequenced
around, or built upon by the other agents in this fleet.

## Likely causes (pick yours honestly)

1. **Breadth-first seam building.** Each session picks the nearest
   interesting seam across any FC lane rather than the next missing exit
   criterion of one lane. The `reconcile` commits record progress but do
   not convert progress into dispositions.
2. **Exit criteria not loaded before working.** If you have not re-read
   the issue's exit list in the last hour, you are optimizing for "more
   landed" rather than "exit met." Sol's OPERATING_MODEL is explicit:
   trace the exit criteria first, then choose the smallest slice that
   closes a real loop.
3. **Exits that genuinely need a live proof** (canary, owner gate) being
   silently deferred instead of surfaced. An issue whose remaining work is
   "run the live proof" should say exactly that in a comment — otherwise
   it reads as abandoned mid-flight and other agents re-enter it.

## The clearer path: a closure protocol

Apply this now, starting with the single issue closest to done:

1. **Post the ladder comment.** On the target issue (start with FC-1
   #8637 — most of its surface appears landed), post a checklist mapping
   every exit criterion to its current rung: code-landed /
   fixture-proven / deployed / live-proven / owner-accepted. The gaps ARE
   your work queue. Nothing else is.
2. **Work only the gaps, in rung order.** No new seams on other lanes
   until this issue is closed or explicitly blocked. Deployed is a rung:
   if the code is landed but the monolith/Pylon hasn't shipped it, deploy
   and verify before claiming further progress.
3. **Split honestly instead of holding open.** If one criterion is
   owner-gated or needs the FC-5 canary, re-scope: land a comment that
   names the residue, move it to the owning lane (or file a bounded
   follow-up), and close the issue on its met exits. An issue that is 90%
   done for days is worse than a closed issue plus a small honest one.
4. **Name the issue and the criterion in every commit.** `feat(pylon):
   execute exact named Grok claims (FC-2 #8633, exit 3/6)` — this makes
   the reconcile commits nearly free and makes drift visible in the log
   itself.
5. **Close on the merged state, same session.** When exits are met:
   comment with the landed SHA, tests run, receipts, residue — then
   close. Do not leave closure for "the end"; the end of an agent session
   is the least reliable moment in this whole system.
6. **WIP cap.** At most two FC lanes with uncommented progress at any
   time. The claim protocol (Sol §11.4) exists; use CLAIM comments so the
   parallel tabs stop orbiting the same seams.

## Suggested concrete order from tonight's state

1. **FC-1 #8637** — ladder comment, close the gaps (the fleet tool,
   durable run contract, and run intake all appear landed; likely
   remaining: deployed + fixture receipts on the issue).
2. **FC-2 #8633** — the supervisor/executor/custody commits look
   substantially complete; same treatment. Its live rung may legitimately
   wait on the canary — split if so.
3. **FC-3 #8639 minimal seam** — supervision projection + typed
   media/conversation states; close on the fixture+deployed rungs with
   the live rung explicitly assigned to FC-5.
4. **FC-5 #8640 Phase A** — the canary IS the live-proof engine for all
   of the above; schedule it as the next session's single goal rather
   than a background hope.

## The measure that matters tonight

Not commits. Not lines. By the next reconcile commit, the honest scorecard
is: **how many FC issues carry a current ladder comment, and how many
closed with receipts.** If the answer after another two hours is again
zero, stop and run the canary instead — a live proof will tell us more
than ten more seams.

— Fable
