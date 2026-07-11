# Cursor Product Teardown — 2026-07-11

Updated analysis of Cursor (Anysphere), revisiting OpenAgents episode 197's
"Reverse Engineering Cursor" study and episode 195's "10x better" thesis
against what Cursor actually became through mid-2026.

- Date: 2026-07-11
- Subject: Cursor 2.x–3.x, the agent-platform pivot, and the ground it left
  behind
- Method: **transcript-grounded retrospective plus archival and public web
  evidence.** Unlike the sibling desktop teardowns, this document performed no
  fresh bundle, runtime, or source inspection of the current Cursor build.
  Its evidence classes are:
  - `[source]` — the recovered episode-197-era reverse-engineering corpus,
    pinned in this repository's own Git history (see §1);
  - `[public]` — a named public source (Cursor's own blog/changelog, press,
    or community forum), fetched 2026-07-11;
  - `[inferred]` — reasoned conclusions from multiple observations;
  - `[limitation]` — a boundary on what this evidence can prove.
- Transcripts [`195`](../transcripts/195.md) and [`197`](../transcripts/197.md)
  are machine-generated; product claims from them are paraphrased intent, not
  quote-grade authority. The reconciliation of 195 against the current roadmap
  already exists in
  [`2026-07-10-episode-195-followup-analysis.md`](../sol/2026-07-10-episode-195-followup-analysis.md)
  and is not re-derived here.

## 1. What we knew then — the recovered era-197 corpus

Episode 197 (recorded 2025-11-13) showed a then-new repo folder of Cursor
reverse-engineering reports. That corpus was located for this teardown in the
openagents repository's own history: **`docs/re/cursor/`**, added 2025-11-13
(the episode day, alongside a `docs/re/DISCLAIMER.md` educational/interop
notice) and deleted 2025-12-01 in a commit titled `Nuke`. It is readable at
pinned commit `ecc0a9054e` (`git show ecc0a9054e:docs/re/cursor/...`) [source].
An older, separate Commander-era "curser/" analysis corpus (December 2024
vintage: fifteen Cursor system specifications, CLI analysis, security review,
tool-capability mapping) survives in `OpenAgentsInc/dashboard` under
`docs/internal/bi/cursor/`, consolidated there 2025-08-15 [source]. The
lineage matters: OpenAgents has now studied Cursor seriously in three distinct
eras.

What the era-197 corpus established, from local inspection of Cursor 2.0.43
on this machine [source]:

- Cursor was a **VS Code fork on Electron** (VS Code base 1.99.3 in logs),
  bundle id `com.todesktop.230313mzl4w4u92`, with its own extension gallery
  at `marketplace.cursorapi.com` and a native `cursor-tunnel` binary of
  VS Code remote-CLI lineage.
- Its runtime delta over stock VS Code was mostly **observability and agent
  plumbing**, not local intelligence: Sentry SDKs plus an unusually broad
  OpenTelemetry instrumentation set, a `cursor-proclist` native process
  addon, `chrome-remote-interface`, and SolidJS panels grafted alongside the
  VS Code webview UI.
- **No local models shipped.** The bundle confirmed the "heavy compute in the
  cloud" architecture; tokenization, file walking, and shadow-workspace
  process machinery prepared context locally, but inference was cloud-only.
- The companion `openagents-cursor-integration-plan.md` (3,301 lines) turned
  episode 195's ten upgrades into a differentiation table — hybrid
  local/swarm/cloud inference, desktop+mobile sync, orchestrator plus
  CLI-agent subagents, open marketplace with revenue sharing, scheduled
  overnight work, SQLite-backed searchable history — and closed with the
  line the episode read on camera: "6 weeks to MVP. 20 weeks to 10x better
  than Cursor."

Episode 195's demand set (paraphrased; see the follow-up analysis for the
full reconstruction) was: a real desktop app over the TUI, mobile parity,
overnight/scheduled work, CLI agents as subagents in one conversation,
discoverable history and memory, hassle-free integrations, open source,
mixed local/cloud inference, idle-compute markets, and revenue sharing.

`[limitation]` The era corpus proves what Cursor 2.0.43 shipped on one
machine in November 2025 and what OpenAgents intended; it proves nothing
about current Cursor builds, which this document covers only through public
evidence.

## 2. What Cursor became — the 2.x–3.x agent-platform pivot

The striking fact is that episode 197 was recorded roughly two weeks **after**
Cursor's own pivot began, studying a build (2.0.43) that was already the new
era. The trajectory since:

- **Cursor 2.0 (2025-10-29)** introduced Composer, an in-house frontier model
  marketed as "4x faster than similarly intelligent models" completing most
  turns under 30 seconds, and a redesigned interface "centered around agents
  rather than files," running up to eight parallel agents isolated via git
  worktrees or remote machines, plus a native browser tool for the agent to
  test its own work [public: Cursor 2.0 blog and changelog;
  artificialintelligence-news.com; Thurrott].
- **Cursor CLI cloud handoff (2026-01-16)**: plan and ask modes came to the
  CLI, and prepending `&` to any message pushes a local conversation to a
  Cloud Agent that keeps running, resumable on web or mobile
  [public: cursor.com/changelog/cli-jan-16-2026].
- **Cursor 3.0 (2026-04-02)** made the "Agents Window" the new primary
  interface — "run many agents in parallel across repos and environments:
  locally, in worktrees, in the cloud, and on remote SSH" — with agent tabs
  in side-by-side/grid layouts, `/worktree`, `/best-of-n` (same task across
  multiple models, compare results), Design Mode, an `Await` tool for
  background shells and subagents, and plans shared alongside transcripts.
  The classic IDE remains available ("switch back to the IDE anytime")
  [public: cursor.com/changelog/3-0].
- **Automations (3.8, 2026-06-18)**: an `/automate` skill, GitHub triggers
  (issue comments, PR reviews, workflow completion), and the computer-use
  tool enabled by default for automations [public: cursor.com/changelog].
- **Marketplace and customization (3.9, 2026-06-22)**: one customization
  surface for plugins, skills, MCPs, and subagents, with a marketplace
  leaderboard; team MCP distribution and org-group access control followed
  in 3.10 [public: cursor.com/changelog].
- **Cursor Mobile for iOS (3.9, 2026-06-29)**: cloud agents on the phone with
  voice input, push notifications and Live Activities for agent status, and
  a "Remote Control" feature to direct desktop agents from the phone
  [public: cursor.com/changelog].
- **Side chats and transcript search (3.11, 2026-07-10)**: parallel side
  conversations that do not interrupt the main agent thread, and agent
  transcript search via command palette backed by local indexing
  [public: cursor.com/changelog].
- **Composer provenance controversy (2026-03)**: users discovered via
  internal identifiers that Composer 2 was post-trained on Moonshot AI's
  open-weights Kimi K2.5; Cursor confirmed within hours, a co-founder called
  the non-disclosure a mistake, and Cursor stated roughly a quarter of
  Composer 2's total compute came from the base model [public: TechCrunch
  2026-03-22; VentureBeat; datastudios.org].
- **Pricing turbulence**: the June 2025 move from 500 "fast requests" to
  usage-based credits produced surprise charges, a public apology, and
  refunds for the 2025-06-16–07-04 window; June 2026 reworked Teams again,
  splitting usage pools between first-party Composer/Auto and third-party
  API models [public: Vantage; finout.io; wearefounders.uk timeline;
  eesel.ai].
- **Stability and UX churn**: Cursor's own 2.3 changelog is titled "Layout
  Customization and Stability Improvements"; community forums from April
  through July 2026 track a persistent class of complaints that the Agents
  Window force-opens on startup, ignores the setting meant to disable it,
  and forgets open projects, alongside reports of release-breaking updates
  corrupting chat histories and worktrees [public: forum.cursor.com threads
  "Trapped in Cursor Agents Window", "Cursor always opens in Agent Window
  mode", "Cursor defaults to Agents Window on application restart";
  vibecoding.app 2026 problem catalog].
- **Business posture**: after a $2.3B Series D at $29.3B (2025), third-party
  trackers report ARR estimates near $1B (late 2025) rising toward ~$4B by
  mid-2026 with a majority-enterprise mix; on 2026-06-16 SpaceX announced an
  all-stock acquisition of Anysphere at $60B, expected to close in Q3
  pending approvals, following the SpaceX–xAI merger [public: TechCrunch
  2026-04-17; CNBC 2026-06-16; Forbes; Yahoo Finance; qz.com. ARR figures
  are third-party estimates, not audited disclosures].

`[inferred]` The owner's "from IDE land to wacky agent IDE, shifting sands"
description matches the record: in roughly eight months Cursor moved its
center of gravity from an editor with an AI sidebar to an agent-orchestration
platform where the editor is one reachable pane — while the fork itself was
never abandoned, only demoted.

## 3. The dropped ground

What the IDE-era Cursor did well, and what the agent-era Cursor abandoned or
destabilized. This is the owner's core question, and the evidence supports
five specific drops:

1. **Startup predictability and user-controlled defaults.** The Agents
   Window hijacks startup, the opt-out setting is reported not to hold, and
   open-project state is forgotten across restarts [public: forum threads
   above]. An editor whose core loyalty was "your workspace, where you left
   it" now renegotiates the workspace on every launch. `[inferred]` This is
   the exact failure class OpenAgents encoded as decision 16 (predictable
   software as executable release contracts): Cursor ships the feature but
   not the contract, so regressions in the promise are invisible until users
   complain.
2. **Editor trust under rapid release cadence.** Corrupted chat histories
   and worktrees after updates, file-save failures, and crash reports form
   the dominant negative feedback theme [public: vibecoding.app;
   checkthat.ai review synthesis]. The IDE-era product earned trust as a
   daily driver; the agent-era product spends that trust on velocity.
3. **Billing legibility.** The 2025 pricing transition converted a flat,
   understood entitlement into an opaque metered pool without adequate
   notice, then required an apology and refunds [public: wearefounders.uk;
   Vantage]. Two regimes later, third-party explainers exist because the
   pricing needs explaining. `[inferred]` The durable lesson is not "never
   change pricing"; it is that usage truth must be visible before the bill —
   the posture OpenAgents encodes as exact-or-`not_measured` usage accounting
   on every receipt.
4. **Model identity transparency.** Shipping Composer 2 without disclosing
   its base model, until users forensically identified it, converted a
   defensible engineering choice (post-training an open base is normal) into
   a trust incident [public: TechCrunch; VentureBeat]. `[inferred]` This is
   the provider/model axis of OpenAgents' no-silent-substitution law
   surfacing at the vendor level: users treat model identity as part of the
   product contract even when the vendor does not.
5. **The open seam.** Cursor's pivot doubled down on closed: closed fork,
   closed models, closed cloud, a marketplace whose extensions target a
   closed host. No public evidence was found of contributor revenue sharing
   in the marketplace, of an open agent-engine boundary, or of any local
   inference option — three of episode 195's demands remain entirely
   unclaimed by the incumbent [public: absence across cursor.com changelog
   and docs as of 2026-07-11; `[limitation]` absence of evidence in public
   channels is weaker than a verified negative].

What Cursor did **not** drop, and should be credited for: the parallel-agent
isolation model (worktrees/remote machines) is genuinely good mechanics; the
CLI `&` handoff is a clean gesture for local-to-cloud continuation; plan mode
before execution and best-of-N comparison are honest concessions that agent
output needs review structure [public: changelogs cited above].

## 4. Where the 10x thesis stands now

Episode 195's ten demands, scored against what Cursor itself did:

| 195 demand | Cursor's own verdict by mid-2026 | Status for OpenAgents |
| --- | --- | --- |
| Desktop app over TUI | Validated: rebuilt its product around an agent-first desktop surface [public: 2.0/3.0] | Contested lane; win on reliability, not novelty |
| Mobile with the same work | Validated: iOS app with cloud agents, voice, Remote Control of desktop agents [public: 3.9] | Contested; OpenAgents' any-host portable-session model is deeper than remote-controlling one desktop |
| Overnight/scheduled work | Validated: Automations with triggers and computer use [public: 3.8] | Contested; OpenAgents differentiates on budgets, receipts, restart-safety per the 195 follow-up |
| CLI agents as subagents | Partially validated: subagents exist, `Await` coordinates them; single-conversation delegation across foreign harnesses (Codex+Claude in one graph) is not Cursor's shape | Open lane; this is the live #8712/Lane-C direction |
| Discoverable history | Validated late: transcript search with local indexing, side chats [public: 3.11] | Contested; loss-accounting and provenance (#8674 discipline) remain undone there `[inferred]` |
| Hassle-free integrations | Partially: marketplace + one customization surface, but MCP/plugin/skill/subagent vocabulary is still exposed [public: 3.9/3.10] | Contested; lifecycle-not-plumbing (D4) still open |
| Open source | Not attempted | **Open lane** — the load-bearing differentiation seam |
| Local + cloud inference mix | Not attempted (cloud-only; first-party models are cloud models) | **Open lane** — post-R7 placement-class work |
| Idle-compute market | Not attempted | Open, deferred behind its revisit gates |
| Revenue sharing | Not attempted (no public evidence) | Open, deferred behind safe extension lifecycle |

`[inferred]` Reading the table honestly: Cursor spent 2025–2026 validating
the *market* for roughly six of the ten demands — the faster-horse company
bought the car drawings. What it fumbled is everything OpenAgents' laws are
about: predictability contracts, usage truth, identity truth, completeness
truth. And what it never touched — the open engine, the typed public
protocol, local execution, economic participation — is precisely the
open-at-the-load-bearing-seam differentiation the adaptation analysis already
names. The 10x thesis survives, but its content shifted: in 2025 it was a
feature list; in 2026 it is a trust and openness list, because the features
are now table stakes.

The 197-era plan's specific technical bets read differently now: codebase
indexing/semantic search, shadow-workspace validation, and fast-apply remain
sound reference systems [source: integration plan §§1.1–1.4]; the
"6 weeks to MVP, 20 weeks to 10x" timeline was wrong the way all such
timelines are wrong, but its ordering (intelligence core before marketplace
before mobile) was inverted by events — mobile and continuity turned out to
be the differentiating floor, which is what the current P0 encodes.

## 5. Lessons for OpenAgents

Each lesson is bound to an owning program coordinate. Per the teardown-README
rule, none of these may live only here: anything load-bearing moves into the
named roadmap gate, issue, or contract.

### Adapt

1. **Make startup predictability a permanent oracle, not a fixed bug.**
   Cursor's Agents Window regressions show the failure mode recurs release
   after release when the promise is not executable. The decision-16 contract
   (episode 248's stable recent-work promise, #8675 acceptance) should be
   treated as a *standing* regression suite across every future default
   change, with "user's chosen surface is restored on restart" as an explicit
   behavior contract in the packages/behavior-contracts registry.
   Owner: decision 16, D0 truthful-baseline gate, CUT-27 (#8707) installed-
   product acceptance.
2. **Cloud handoff proves the portable-session market; win it on identity.**
   Cursor's `&` handoff moves a conversation to *their* cloud only, and
   Remote Control drives *one* desktop. The Rev 30/31 portable-session and
   capability-broker packets (move a durable session between authorized
   local, owner-managed, and managed-cloud hosts without forking identity,
   authority, secrets, or receipts) are the strictly stronger contract.
   Ship the difference visibly: session movement with receipts, not
   fire-and-forget cloud copies. Owner: the remote-first portable sessions
   pathway doc and its Revision 30/31 work packets; #8547/#8636 for targets.
3. **Best-of-N and plan mode belong in the fleet vocabulary as typed
   comparisons.** Running one task across multiple models/harnesses and
   comparing outcomes is a natural FleetRun shape OpenAgents already has the
   substrate for (mixed Codex+Claude proof #8640; Lane C per-child usage
   rollups). Adapt it as a typed work-unit fan-out with per-child receipts
   and an explicit comparison record — not as UI garnish. Owner: D5 fleet
   cockpit within #8574; conversation-native delegation per the 195
   follow-up Amendment A.
4. **Transcript search validated demand; do it loss-accounted.** Cursor
   added local-indexed transcript search eight months after the agent pivot.
   OpenAgents' ordering (completeness #8674 first, then discovery) is
   correct; keep Amendment B (owner-private search with counted gaps and
   exact source navigation) as the D2 follow-on rather than racing to a
   lossy index. Owner: D2 within #8574, post-#8674.
5. **Worktree/remote isolation as the default parallel-agent mechanic.**
   Cursor's eight-agents-without-interference model matches the CUT graph's
   worktree discipline and the Firecracker Agent Computer lane. No new work
   needed; treat Cursor as market confirmation that isolation-by-default is
   the correct consumer-visible posture, and keep "which host, which
   isolation, which account" on the receipt. Owner: CUT graph #8681–#8707;
   #8547.
6. **Model provenance is part of the product contract.** Composer/Kimi shows
   users will forensically audit model identity. OpenAgents' model-level
   no-substitution pin (EP250: Fable pinned to claude-fable-5, refusing
   substitution) is the right posture; extend the same disclosure honesty to
   any future first-party or fine-tuned model: name the base, the
   post-training, and the serving path in public docs from day one.
   Owner: no-silent-substitution law (all identity axes); harness-selector
   surfaces from #8712.
7. **Price with usage truth in the loop.** Both Cursor pricing crises came
   from bills users could not predict from what they could see. OpenAgents'
   exact-or-`not_measured` receipts must reach the *pre-spend* surface:
   before a fleet run, show the account/lane/budget that will be consumed;
   after, reconcile against exact rows. Owner: capability-truthful
   affordances ("no lane, no Send") and the usage-receipt law; D5 fleet
   controls.

### Reject

1. **Do not fight for VS-Code-fork parity.** The incumbent itself demoted
   the editor; the era-197 corpus shows the fork's runtime delta was mostly
   telemetry and glue [source]. OpenAgents Desktop's OpenCode-parity
   workbench target (R5 exit) is the right editor scope; a full IDE is not.
2. **Do not ship computer-use-on-by-default in unattended paths.** Cursor
   enables the computer-use tool by default for Automations [public: 3.8
   changelog]. Under OpenAgents law that is an authority grant inside an
   unattended lane and must remain deny/ask-by-default with typed policy,
   per the 195 follow-up's Automations slice (budgeted, pauseable,
   Inbox-visible). Owner: Amendment D boundary; approvals authority classes.
3. **Do not let defaults evangelize.** Force-opening the flagship surface on
   startup is growth pressure expressed as UI. OpenAgents surfaces earn
   attention through the Inbox/attention model, never by overriding the
   user's last chosen context. Owner: decision 16; behavior contracts.
4. **Do not copy the closed-marketplace shape.** A leaderboard of closed
   extensions for a closed host is the weakest form of ecosystem. The A10
   signed-catalog lifecycle (provenance, capability review, rollback) plus
   eventual economic receipts is the stronger form; keep payment out of the
   install gate. Owner: D4 integration lifecycle; deferred revenue-sharing
   horizon.
5. **Do not treat valuation as verdict.** `[inferred]` The $60B SpaceX
   outcome proves distribution and enterprise demand for agent coding; it
   does not prove the product posture is right, and the acquisition
   consolidates Cursor further into one closed corporate stack — widening,
   not narrowing, the open-seam lane.

## 6. What not to conclude

- Do not conclude Cursor abandoned the IDE: the editor remains one switch
  away [public: 3.0 changelog]. The claim supported by evidence is a
  center-of-gravity move plus a defaults fight, not a product deletion.
- Do not conclude Cursor is failing commercially: every business indicator
  points the other way [public: §2]. The dropped ground is trust mechanics,
  not revenue.
- Do not conclude the 195 feature list is still differentiating by itself:
  most of it is now the incumbent's roadmap too. The differentiation that
  remains is the part Cursor structurally cannot copy without ceasing to be
  Cursor: the open engine, typed public protocol, host-portable sessions
  with receipts, capability-truthful UI, and economic participation.
- `[limitation]` This teardown had no access to current Cursor binaries,
  private forums, or usage data; community-complaint prevalence is not
  quantified, and third-party ARR figures are estimates. Claims here are
  bounded by the named public sources as of 2026-07-11.

## Sources

Primary: recovered era-197 corpus at openagents commit `ecc0a9054e`
(`docs/re/cursor/`), Commander-era corpus in `OpenAgentsInc/dashboard`
(`docs/internal/bi/cursor/`). Public: cursor.com blog/changelog (2.0, 2.3,
3.0, 3.8–3.11, CLI 2026-01-16), TechCrunch (2026-03-22, 2026-04-17), CNBC
(2026-06-16), Forbes, Yahoo Finance, qz.com, VentureBeat, datastudios.org,
Vantage, finout.io, wearefounders.uk, eesel.ai, forum.cursor.com threads,
vibecoding.app, checkthat.ai, artificialintelligence-news.com, Thurrott.
