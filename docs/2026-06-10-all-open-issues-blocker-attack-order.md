# All Open Issues: Blocker Categories and Attack Order

Date: 2026-06-10 (written ~02:30 UTC 2026-06-11)

## Scope and finding

At the time of writing the repo has **32 open issues** (#4641–#4735). This
audit extends `docs/2026-06-10-oldest-open-issues-blocker-audit.md` (which
covered the eleven oldest) to the entire open set, assigns every issue to
exactly one blocker category, and orders the categories so that working them
top-to-bottom closes the largest swathes of issues earliest and leaves a
visible path to zero.

The headline finding from the eleven-oldest audit still holds across the
full set: **almost nothing is blocked on missing code.** The engineering for
nearly every open issue is merged, tested, and in many cases live-smoked.
What the issues are waiting for falls into six categories, and the
categories have sharply different costs and unlock fan-outs.

Companion context: the full system status is in
`docs/artanis/2026-06-10-artanis-pylon-tassadar-full-status-audit.md`.
Rung 0 of the always-on fleet plan is executed (3 Pylons online), the
treasury holds ~45k sats, and the Artanis standing-cap spend envelope
(#4703) exists — which is precisely what makes the order below executable
now rather than aspirational.

## The six categories, in attack order

Ordering logic: (1) zero-dependency work first — it costs nothing and some
of it widens the counterparty funnel everything else needs; (2) owner
authorizations next — minutes of effort, each unlocking a chain; (3) the
first-receipt proofs — now affordable and administrable through the spend
envelope, and the single biggest mass of red-promise closes; (4) fleet
scale, which the receipts and the training program both need; (5) the
training program, the long pole, deliberately sequenced after GEPA by owner
decision; (6) sweeps and epics, which close themselves as their parents do.

---

### Category 1 — Pure engineering: no external dependency at all (2 issues, plus 1 pull-forward)

Nothing blocks these except someone doing them. Both were filed by external
agents doing free QA on-forum tonight, and both directly damage the
counterparty funnel that Categories 3–5 depend on (agents misreading the
network as dead, or having public refs silently redacted by their own
tooling).

| Issue | What it is | Why it matters beyond itself |
| --- | --- | --- |
| #4735 | pylon-stats counters contradict their own sample rows (5-min vs 24-h windows) — false dead-network signal | Every arriving agent and contributor reads this surface first; it currently says the network is dead while three Pylons are online |
| #4734 | Public API refs look like credentials to agent-side secret scanners and get silently redacted | External agents literally cannot quote our receipts; breaks the evidence culture for the exact audience we want auditing us |

**Pull-forward candidate:** #4677 (public run pages replacing the dead
`/training/runs` SPA shell) lives in the training cluster (Category 5) but
is pure projection work with no fleet or spend dependency. Doing it early
gives the training program a public face before the program itself runs.

**Cost:** hours. **Closes:** 2–3 issues directly; repairs the front door
for everything else.

---

### Category 2 — Owner-held authorizations: minutes each, chains behind each (4 issues + 1 downstream)

Each of these is a single owner action — a credential, a config value, or a
spend authorization — with engineering already complete behind it. The
eleven-oldest audit priced these at "minutes of owner action each."

| Issue | Owner action needed | What it unlocks |
| --- | --- | --- |
| #4654 | npm publish credential for `@openagentsinc/pylon` + workspace deps (title now says it: "owner action; CI gate itself is done") | → #4662 stable 0.3.0 release → #4663 release-cluster sweep → flips `pylon.v03_release_candidate.v1` and `pylon.release_tomorrow.v1` toward green |
| #4658 | `live_small_sats` spend authorization for the install-to-bitcoin smoke on a real machine | Clears the last blocker chain on `pylon.install_without_wallet_knowledge.v1` (red) |
| #4653 | MDK provider-side webhook/event config | Tips webhook live callback + refund/reversal → `forum.content_tipping.v1` yellow→green |
| #4700 | Set `WITHDRAWAL_DESTINATION` so MDK revenue payouts fund the campaign treasury | Standing treasury refill loop; removes ad-hoc funding from the payout path |

| Downstream | |
| --- | --- |
| #4662 | Stable 0.3.0 release — pure execution once #4654's credential exists |

**Cost:** an evening's worth of owner minutes. **Closes:** 5 issues, two
promise flips, and the entire v0.3 release chain.

---

### Category 3 — First-receipt proofs: counterparty + small sats (9 issues)

The single biggest category, and the one carrying every red revenue
promise. Each issue needs one settled, receipted, public-safe first
transaction. Until tonight these were stuck behind two walls that no longer
exist: the fleet was dark (rung 0 fixed that) and there was no funded,
governed way to spend (treasury + standing-cap envelope fixed that).
Artanis filed working claims on every one of these lanes overnight.

The counterparty problem is also softer than it looks: Orrery, Mr_Tibbs,
and MAZO arrived on the forum *tonight*, unprompted, looking for ways to
earn sats. The buyers and contributors these issues need are already
posting introductions.

Recommended internal order (each step reuses the previous step's proof):

| Order | Issue | The receipt needed | Notes |
| --- | --- | --- | --- |
| 3a | #4641 | Live paid kind-5050 compute smoke | Cheapest; fleet is online; clears `compute_stream_not_broadly_live` |
| 3b | #4642 | One paid GEPA settlement | Endpoint-smoke blocker already cleared (commit `375ef501e`); only the paid leg remains |
| 3c | #4732 | **The labor demonstration**: first negotiated, escrowed, executed, accepted, settled labor job with public receipts | Highest leverage single issue on the board — advances all three `labor.*` promises, `provider.compliant_usage_labor.v1`, and #4648 simultaneously; the Claude bridge (#4717) gives the provider its engine |
| 3d | #4648 | First paid overnight labor job on a contributor's own agent | Likely closes with or immediately after 3c; needs the independent contributor — recruit from the intro threads |
| 3e | #4645 | First settled dataset sale (public-safe redacted conversation bundle) | Needs one buyer; small sats |
| 3f | #4651 | First settled referral payout | Needs one referral conversion; attribution capture is live |
| 3g | #4652 | Five-streams one-install stacking smoke | Composition of 3a–3f; closes when they exist |
| 3h | #4717 | EPIC: Claude bridge green flip | Needs one production run on a real contributor device — falls out of 3c/3d |
| 3i | #4726 | EPIC: open agent labor market | Closes when 3c/3d and the labor promises have their receipts |

**Cost:** tens-to-hundreds of sats per receipt, all within the standing-cap
envelope; plus recruitment messages on threads where candidates already
introduced themselves. **Closes:** 9 issues and flips the heart of the red
column: `pylon.five_bitcoin_revenue_streams.v1`,
`provider.compliant_usage_labor.v1`, `pylon.data_trace_revenue.v1`, the
labor yellows, and the bridge yellow.

---

### Category 4 — Fleet scale and standing services: more than one device, plus Psionic (5 issues)

These need what rung 0 proved, multiplied: rungs 1–2 of
`docs/2026-06-10-always-on-fleet-plan.md` (Tailnet remotes —
imac-pro-bertha and archlinux are named hosts — then the SHC dispatch
lane), and a standing Psionic inference server for the Qwen lanes. No new
product engineering; the smokes are one-command
(`bun run smoke:probe-gepa-stage0`, `bun run smoke:qwen-remote-training`).

| Issue | Needs | Notes |
| --- | --- | --- |
| #4667 | GEPA Stage 0 green on **multiple** real Pylons | Pure fleet count; no spend |
| #4665 | psionic_qwen35 attach-only inference rows admitted | Needs a running Psionic server (sibling repo) — the one true external service dependency on the board |
| #4666 | Sell Psionic-backed Qwen3.5 inference | Stacks on #4665 plus one small-sats buyer (Category 3 motion) |
| #4670 | Bounded remote Qwen run on **two** real devices | Fleet count + operator-funded worker payments |
| #4668 | Paid GEPA campaign through the ladder to settled_bitcoin | Fleet + spend; the 9-step ladder is built |

**Cost:** standing up 2–3 more supervised hosts (the runbooks exist) and
keeping one Psionic server alive. **Closes:** 5 issues; moves
`pylon.compute_revenue_modes.v1` and `pylon.gepa_worker_loop_v03.v1`, and
unblocks Category 5's entry.

---

### Category 5 — The training program: sequenced after GEPA, fed by everything above (9 issues)

The CS336 homework rails (#4675–#4683) are the long pole and are
*deliberately* sequenced: owner decision postpones Qwen/full training until
GEPA is live (Category 4), and the program consumes the fleet (Category 4),
the paid lanes (Category 3), and the validator economics. The code
substrate exists — job kinds, gates, exact-replay verification, the
real-gradient A1 lane — so this category's blocker is *the categories above
it*, not engineering.

| Issue | What it is |
| --- | --- |
| #4675 | CS336 A1 homework job kind with paid closeouts |
| #4676 | Validator work as paid Pylon assignments (weak-device lane) |
| #4677 | Public run pages (pull-forward candidate → Category 1) |
| #4678 | A1 leaderboard-class run — real gradients across contributor devices |
| #4679 | A3 scaling-sweep homework — crowd-sourced IsoFLOP curves |
| #4680 | A4 data-refinery homework — eval-delta payment design |
| #4681 | A2 benchmark homework — public device-capability dataset |
| #4682 | A5 rollout/grading homework — RL fed by the compute market |
| #4683 | Per-assignment receipt-backed public leaderboards |

Internal order once unblocked: #4675/#4676 (the work and validator kinds)
→ #4678 (first leaderboard-class run) → #4681/#4679/#4680/#4682
(the homework family) → #4683 (leaderboards), with #4677 pulled forward.

**Closes:** 9 issues; begins moving the eight `planned` `training.*`
promises and the two training reds. Also feeds Tassadar evolution Stage 2
(train candidates) — the distillation-dataset receipt blocking
`artanis.tassadar_evolution_loop.v1` is this category's first artifact.

---

### Category 6 — Verification sweeps: close themselves (2 issues)

| Issue | Closes when |
| --- | --- |
| #4663 | Release-cluster sweep — after Category 2 (#4654/#4662) |
| #4671 | Training/compute-modes sweep — after Categories 4–5 |

These are bookkeeping by design: re-verify, propose registry flips, post
the forum report. No independent blocker.

---

## The cascade, summarized

| Wave | Category | Issues closed | Cumulative | Cost profile |
| --- | --- | --- | --- | --- |
| 1 (today) | Pure engineering | #4735, #4734 (+#4677 pulled forward) | 3 | Hours of coding |
| 2 (today/tomorrow) | Owner authorizations | #4654, #4658, #4653, #4700, #4662 | 8 | Minutes of owner action each |
| 3 (this week) | First receipts | #4641, #4642, #4732, #4648, #4645, #4651, #4652, #4717, #4726 | 17 | Small sats via spend envelope + recruiting from intro threads |
| 4 (this week) | Fleet scale + Psionic | #4667, #4665, #4666, #4670, #4668 | 22 | 2–3 supervised hosts + one standing server |
| 5 (next) | Training program | #4675, #4676, #4678–#4683 | 30 | The program itself; unblocked by waves 3–4 |
| 6 (rolling) | Sweeps | #4663, #4671 | **32** | Bookkeeping |

Three structural observations the order encodes:

1. **The bottleneck migrated.** A day ago the universal blocker was the
   dark fleet. Rung 0 removed it. The new universal blocker is
   *counterparties and authorizations* — people problems and minutes-long
   owner actions, not systems. The attack order front-loads exactly those.
2. **One issue is worth five.** #4732 (the live labor demonstration) is the
   highest-leverage close on the board: it carries three labor promises,
   the provider-labor red, the Claude-bridge green flip, and #4648 in a
   single receipted transaction. If only one thing gets focused effort this
   week, it should be that.
3. **Nothing in waves 1–4 waits on new invention.** Every issue there has
   merged code, a runbook, or a one-command smoke behind it. The repo's
   open-issue list is, almost in its entirety, a list of receipts not yet
   collected — which is the healthiest shape a backlog of this size can
   have, and also the reason the `blocked-on:` labeling convention from the
   eleven-oldest audit should be applied to all 32 now (one line per issue:
   `blocked-on: engineering | owner-action | counterparty | fleet |
   sequenced-after-gepa | sweep`).

## Maintenance

This audit is a snapshot of the open set at ~02:30 UTC 2026-06-11. As
issues close, the waves shrink in place; if a new issue arrives, file it
into one of the six categories at creation time (the `blocked-on:` line)
rather than re-auditing. Re-run the category census when the open count
next crosses a round threshold or a wave fully empties.
