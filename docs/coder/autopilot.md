# Coder Autopilot

Date: 2026-08-29. Status: **spec.** Not implemented. This document is the
design of record for an autopilot mode in the Rust Coder TUI
(`crates/openagents-cli`). It pulls together the autopilot thread from the
transcript archive (`docs/transcripts/199`, `200`, `206`, `237`, `275`) and
the historical buildout under `docs/autopilot-coder/` — especially the AFK
loop discipline in `2026-06-13-afk-autonomous-loop.md`.

The historical directory carries the standing banner *HISTORICAL — consult
MASTER_ROADMAP*. That banner is about the June product surface (Pylons, work
orders, the labor spine). The doctrine those documents settled is not
historical; the spec below re-homes the parts that survived contact with the
old system and binds them to what actually ships today.

---

## 1. What Autopilot is

The owner's framing, from the v275 recording: *"goal: do all of my issues.
Then I want to be able to go AFK and have it actually work."* And the pet
peeve that motivates the mode: coming back 30 minutes later to *"for your
consideration… here's the blocker"* — a session that stopped and waited when
it was told to go.

**Autopilot is a mode, not a lane.** The TUI already tabs between
`Coder Flash / Pro / Free / Local` (`crates/openagents-cli/src/coder/runtime.rs:358`
— the `LANES` table; the shift+tab lane move is `Lane::cycle_gated`, wired
in `coder/interactive.rs`). That walk answers *which model answers this turn*.
Autopilot answers a different question: *who steers between turns.* They are
orthogonal:

- A **lane** is model selection. It exists today. Shift+Tab walks it.
- **Autopilot** is a control mode. When it is off, the loop ends every turn
  and waits for a human, exactly as today. When it is on, the loop keeps
  steering itself across turn boundaries: it picks the next unit of work,
  runs it to a verified or honestly-blocked end, and starts the next one,
  for as long as its budget and stop conditions allow.

Because they are orthogonal, every combination is legal: Autopilot on Flash
for cheap bulk issue work, Autopilot on Pro for a hard task, Autopilot on
Local when the owner wants the work to stay on-machine and free. The lane
keeps cycling under Autopilot; the mode does not touch it.

The name. "Autopilot" has carried two meanings in this repository's
history: the episode-199 founding thesis (agents, not copilots — *autopilots
are autonomous, long-lived, they learn and evolve*) and the June web
product. The TUI reclaims the word for the founding thesis in its plainest
form: **you tab into it, you walk away, work happens, and you come back to
receipts.** No web app, no container, no new substrate — the same binary,
same lanes, same tools, same transcript.

## 2. The doctrine Autopilot inherits

These are the load-bearing rules from `2026-06-13-afk-autonomous-loop.md`
and the episode thread, restated for a single-session TUI mode. Where the
old document spoke to a coordinator fanning out workers, these are the
one-session equivalents.

1. **Never idle, never sleep on a timer.** There is always more backlog:
   open issues, project items, verification debt, docs reconciliation. When
   one work source empties or turns owner-gated, move to the next. "Nothing
   to do" is never true.
2. **Partials are fine; staleness is not.** A batch that lands half its work
   is a success if the issue says exactly what merged and what remains.
   Close an issue the moment its acceptance is genuinely met; leave it open
   with a current comment when it is not. The open-issue list must stay a
   true reflection of remaining work.
3. **Never merge or report a red verify.** A completion claim names its
   oracle (best practice V1) observed on the surface the owner touches
   (V2). An unverified "done" is worse than an honest "blocked."
4. **Acceptance is met or it is not.** From episode 237: the atomic unit is
   the accepted outcome, not the narrated attempt. Autopilot's unit of
   progress is *work whose verification someone else can reconstruct.*
5. **Do what was said.** The mode exists because the owner said "just do
   what I say." Inside the agreed scope, Autopilot does not come back with
   "for your consideration." It decides, acts, records. The guardrails that
   do exist (§7) are budget and stop conditions, not pre-flight approval
   theater. The v275 framing is explicit: this is a power tool, bypass-
   permissions posture, and the owner accepted the risk in the same breath.

## 3. Open-time state

Autopilot is **off by default.** A session opens exactly as today: lane
resolved, welcome card rendered, control with the human.

- **Shift+Tab** continues to walk lanes only. Adding a mode to the lane walk
  would couple two orthogonal axes and make "Flash, on autopilot, on local"
  unrepresentable. The walk is sacred; it answers a different question.
- **Meta+A / Alt+A** toggles the mode. **`/autopilot`** is the spelled-out
  command and takes an optional first directive
  (`/autopilot work the P0 column of the auton project`), so the composer
  stays the one input.
- The **status line** gains a mode cell: lane first, mode second —
  `Coder Flash · AUTOPILOT` — because the reader must be able to tell at a
  glance which axis they are looking at. The TUI's seven-line welcome-card
  ceiling (`tui.rs:33`) gets one added line naming the mode and its stop
  conditions; nothing else about open changes.
- Autopilot state lives in **session state only** (like the mutable lane at
  `interactive.rs:132`). A new session opens with the human steering. A
  mode that re-arms itself across sessions is exactly the failure shape
  `git checkout`-style persistence always produces; if the owner wants
  continuity they have `--continue` and the succession record (§12).

## 4. The Autopilot iteration

One iteration is one turn plus its follow-through, in this order. This is
the AFK loop's §1, compressed for one session.

1. **Take stock.** Read the open issue list for the forge in scope
   (`openagents issue list`), the session's goal store if one is set
   (`coder/goal.rs`), and anything that arrived since the last boundary
   under the dual-path rule (§10: recorded injections, plus the fallback
   drain). Reconcile in-flight work first, and only this session's own
   work: anything uncommitted from the previous iteration gets verified,
   committed, or rolled back before anything new starts. Orphaned
   worktrees from a dead predecessor (staleness confirmed via the swarm
   session list) are reconciled the same way — explicitly, named in the
   announce line, never silently rolled back. The shared checkout's
   uncommitted state is **foreign WIP**: other live sessions share this
   cwd, it is recorded as present and never touched, because an autopilot
   that "cleans up" the shared tree destroys sibling work.
2. **Claim, then pick the next unit.** Before selecting, check sibling
   heartbeats (§5) for an open claim on the candidate issue — many
   sessions, one forge: two pickers will grab the same issue, duplicate
   the work, and race the close. On collision take the next candidate. A
   claim is announced in the §4.3 line and as a swarm status, refreshed
   each heartbeat. Then pick: highest-priority open issue whose acceptance
   the session can actually verify, or the next project item if the forge
   is clean. Skip anything owner-gated (credentials, spend, a live boot)
   and record the skip. Never idle on a gated item while fannable work
   exists.
3. **Announce the unit.** One transcript line: what was picked, which issue
   or item it advances, what "done" will mean, and the claim if one was
   broadcast (§4.2). This line is what makes
   the owner's return skimmable (§11).
4. **Do the work.** Normal tools, normal delegation rules, fresh worktree
   per unit per the runbook. Scope discipline: files the issue names, tests
   the issue's acceptance names.
5. **Verify at the oracle.** Run the acceptance gate the issue (or the
   completion gate for CLI changes: `cargo fmt --all -- --check` and
   `cargo test --workspace` — what the pre-push guard enforces (the pnpm
   toolchain is gone from this repo); TUI changes also take
   the pty suite). No claim of completion without the oracle's output in
   the transcript.
6. **Land and report.** Commit with the issue number, push, and close the
   issue with a commit-citing comment — or comment the current state
   honestly: what merged, on which sha, what remains. Then **start the next
   iteration in the same motion.** Ending a turn with the mode on and no
   next unit selected is the idle failure the old loop called out by name.

## 5. Swarm posture

The Coder binary already has `swarm_list` / `swarm_send` / `swarm_inbox`.
Under Autopilot these stop being conversational and become operational:

- **Heartbeats.** At every iteration boundary, a status message to the
   swarm naming the unit just closed and the unit just picked, and
   refreshing any open claim (§4.2). This is the
   cheapest possible "still alive, still productive" signal, and it is what
   makes a stuck autopilot visible from outside its own process (§11).
   A heartbeat send failure renders in the status line; N consecutive
   failures is itself a stop condition (§7) — the visibility mechanism
   failing silently is the #306 shape, and it must not fail silently.
- **Broadcasts stay one per milestone.** A mode that broadcasts every
   iteration is spam; the budget rules for swarm sends stay as they are.
- **No new lanes.** Autopilot may delegate to children exactly as a
   steered session may (`delegate`), under the same proactive-delegation
   mandate in AGENTS.md, and consolidates review exactly the same. It does
   not invent a second coordination layer; the swarm tools are the
   coordination layer.

## 6. Evidence

Autopilot changes nothing about how work is recorded; it changes how much
recording discipline matters, because nobody is watching live.

- The **transcript** remains the thread of record: every iteration's
  announce line, oracle output, and close report lands there, and
  `history_recall` can answer for any of it later.
- **Issues are the ledger.** The close comment cites the commit; the
  partial comment cites what merged and what remains. An issue closed
  without a commit-citing comment is a defect, not a style choice.
- **The goal store** (`coder/goal.rs`) gains a second caller: an autopilot
  iteration that advances a goal records tokens and time against it, so a
  long run reports `budget_limited` honestly instead of dying mid-unit.

## 7. Budget and stop conditions

An autopilot that cannot stop is a liability, not an autopilot. The mode
stops — cleanly, on the iteration boundary, with the ledger current — when
any of these fires:

- **Token or wall-clock budget** named at engage time or in config
  (default: none is not an option; the default is one hour or the
  goal's budget, whichever is shorter). The budget-report ending the turn
  already exists on every lane; autopilot checks it before selecting the
  next unit. The **goal-store token ledger is the primary signal**, wall
  clock secondary: the lane cycles freely under autopilot, and an hour on
  Pro is different work from an hour on Flash.
- **Owner-gated wall.** Every remaining open issue needs credentials, live
  boots, or spend the mode cannot authorize. The `NEEDS_OWNER.md` rule
  applies unchanged: asks go to the workspace root file the owner actually
  reads, never to this repo's copy.
- **Repeated failure.** The same unit failing verification twice is
  recorded and skipped, not retried a third time. Failure loops are the
  AFK failure mode with the worst cost shape.
- **Visibility loss.** The swarm send budget exhausted, or N consecutive
  heartbeat failures. A mode whose visibility mechanism has died is a
  mode running silent — stop and report in the transcript, where the
  owner will eventually look.
- **Forge unreachable.** The ledger's reads or writes failing N times
  consecutively. If the forge is down, landed work cannot satisfy §2 and
  §6 — no commit-citing close is possible — so the mode must not keep
  landing while its evidence system is dark.
- **A stop word from the network.** A stop word is a token carried by a
  swarm message; sighting it ends the mode at the next boundary (§10).
  The token is named at engage time — `--stop-word <token>`, defaulting
  to a value derived from the engaging directive — or pinned in config as
  `autopilot_stop_word`. Alternative mechanism: `--stop-word-from
  <session-id>`, pre-filled from the swarm session list. Either is
  honest; both beat an owner-identity register that sessions cannot back
  today (ids are opaque, no identity metadata exists). Detection is
  announced in the transcript.
  The rule is
  dual-path by construction: the stop word is recognized on *either*
  delivery
  path — injected-and-stamped at the boundary, or returned by the fallback
  drain — and either sighting ends the mode. The mode never relies on the
  drain alone to see its own off switch.
  Unset mechanism: only interactive Meta+A can stop the mode, and the
  welcome card
  says so. Anyone standing in front of the terminal hits Meta+A; the point
  of the stop word is that nobody is standing there.

A stop is a **report**, not a halt: the final turn writes the ledger state
(what closed, what remains, what is gated on whom) into the transcript and
the succession record (§12), then exits the mode and waits like a normal
session. It does not kill the process; it hands back the wheel.

## 8. The TUI

- Welcome card: one line under the lane line — `autopilot off · Meta+A to
  engage · stops on budget, blocked, or repeat-fail` — within the existing
  card ceiling.
- Status line cell as in §3.
- While engaged, every iteration boundary renders the same three lines:
  unit picked, oracle result, unit closed. No decorative progress bars; the
  transcript is the progress bar.
- **`/autopilot status`** is the in-terminal return surface (§11): one
  screen — units closed, oracle results, budget burned, next unit, last
  heartbeat, mode state. It is what the owner's eyes land on first when
  they walk back in; the three-line boundary stream scrolls away, and the
  ledger requires leaving the terminal.
- `/autopilot` with no argument toggles; with a directive, engages with
  that directive as the initial pick filter. `/autopilot off` disengages
  at the next boundary — never mid-unit, never mid-verify.

## 9. What Autopilot is not

- **Not a lane.** It does not appear in the `LANES` table, does not join
  the shift+tab walk, and does not touch model resolution. (`Lane::Named`
  refuses to join the walk for the same reason Autopilot must: a keystroke
  that silently changes what answers is the worst outcome the walk can
  produce; a keystroke that silently changes *who steers* would be worse.)
- **Not the June Autopilot product.** No workrooms, no container
  execution, no provider-account leasing. Those live in their own retained
  history. This is the TUI reclaiming the thesis, not resurrecting the
  surface.
- **Not unattended credential escalation.** The mode runs under exactly
  the credentials the session opened with. It never prompts, never stores,
  never escalates. When something needs the owner, it gates (§7) and says
  so.
- **Not always-on.** It is opt-in per session, default off, and it hands
  the wheel back on stop. The episode-199 thesis was autonomy *with*
  legibility — "what agents do stays understandable, legible, and steerable
  by humans." The mode is the autonomy; the transcript, the ledger, and
  the stop conditions are the legibility.

## 10. Boundary mail and the drain policy

The swarm inbox has two shapes: injected at turn boundaries, or read
explicitly. The issue-discussion session's live finding (#303) is that
boundary injection currently stamps everything read on arrival, so an
explicit drain after the boundary returns empty — the peer session verified
this first-hand during this spec's review: `swarm_wait` matched, an
immediate drain returned nothing.

The prescription is therefore **dual-path, not drain-only**. Delivery is
whatever mechanism actually carried the mail:

- **Injection is delivery.** Mail stamped read by boundary injection counts
  as delivered. The announce line of the boundary iteration (§4.3) records
  what arrived by injection, so the transcript shows it either way.
- **The explicit boundary drain is a fallback sweep, not the primary.** It
  runs before unit selection (step 1 of §4) and normally returns empty; it
  exists for the messages no boundary was reached to inject.

This is a stance about *delivery*, not a bet on the current mechanism. If
#303's ownership model changes (the `consumed:[ids]` receipt direction),
the drain becomes verifiable instead of presumed-empty, and the
prescription collapses gracefully to drain-as-primary. What never changes:
autopilot's mail handling is explicit, recorded, and able to see every
message that arrived, whichever path carried it.

The stop word (§7) rides the same surface, which makes it the
highest-stakes consequence of #303 in this spec: an off switch that
requires a drain is an off switch the current semantics can silently
blind. The stop rule in §7 therefore never depends on one path.

## 11. The observer's return

The mode is judged by what the owner sees when they come back. The bar:

- Skim the last N announce lines (§4.3) — or the issue ledger — and know
  what happened without reading a single full transcript.
- Every closed issue traces to a commit; every open issue's latest comment
  is current.
- Nothing done in the owner's absence is hidden, uncommitted, or
  unverifiable. The AFK loop's phrase of record applies: the bar is
  *clarity, and nothing stale or forgotten.*

## 12. Succession

An autopilot session can die — crash, context exhaustion, machine sleep.
The mode treats that as a normal event, not a lost world:

- Every iteration boundary leaves the ledger current (§6), so a successor
  session (`coder --continue`, or a fresh one pointed at the same forge)
  starts from the issue list, not from an excavated transcript. This is
  the same discipline as the old loop's "append progress to §6 so it
  survives compaction."
- The session's final report (§7) is the succession document: current
  ledger state, in-flight work, gates.
- **Known gap:** cross-session history recall for sibling sessions is
  broken today (#301) — a successor cannot yet ask the tool what the
  predecessor decided and must read the ledger instead. The ledger-first
  design above is deliberate: it works even while #301 stays open. When
  #301 closes, succession gets richer, but nothing here depends on it.

## 13. Implementation order

Smallest honest slices, each shippable alone:

1. **Mode toggle + status line + welcome card.** Meta+A, `/autopilot`,
   visible state. Engaged, the mode does one thing: after each turn, it
   re-reads the goal or issue list and starts the next iteration. No
   budget checks yet — the toggle and the loop are the slice.
2. **Iteration discipline.** The §4 order, announce lines, ledger updates,
   stop-on-repeat-failure. Include the dual-path mail stance (§10) in its
   minimal form: record injections in the announce line; run the fallback
   drain. This much is correct under today's semantics.
3. **Budget and stop conditions** (§7), goal-store accounting (§6), and
   the stop word **including** the dual-path recognition and the
   `autopilot_owner_session` register. The stop word does not wait for
   #303; waiting would leave the mode with no remote off switch.
4. **Verifiable mail ownership** (the `consumed:[ids]` receipt direction
   from #303, upstreamed to that issue or filed as its child): the drain
   becomes provable, and §10's fallback collapses to drain-as-primary.
   This slice depends on #303 moving first; until then the dual-path
   stance in slices 2–3 is the whole of mail handling, honestly.

Tests follow the repo's own law: TUI behavior is verified on a
pseudo-terminal (`coder_interactive_pty`), loop behavior on stub-runners,
and nothing in this spec closes on headless narration.

---

## 14. Review record

Reviewed twice by the swarm session `1a04a2afc3c` (full 289-line read, then
an eight-item second pass) before the implementation issues were filed.
Verdict after the first pass: the load-bearing choices hold (mode-vs-lane
orthogonality, ledger-first succession, stop-as-report); §10 and the §7
stop word were reworked on its finding that the prescribed drain is a no-op
under current injection semantics, and the path pins were corrected
(`runtime.rs` → `coder/runtime.rs`). The second pass landed: shared-cwd
reconciliation scoped to the session's own worktrees (§4.1), claim-before-
pick with sibling heartbeats (§4.2), visibility-loss and forge-unreachable
stop conditions (§7), the token-based stop word replacing the unbackable
owner register (§7), heartbeat-failure surfacing (§5), the token ledger as
primary budget signal (§7), `/autopilot status` (§8), and the orphaned-
worktree succession rule (§4.1).
