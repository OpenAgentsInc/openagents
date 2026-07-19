# Amp in a Few Days: What It Looks Like When OpenAgents Does Everything Amp Does

- Date: 2026-07-16
- Class: strategic evidence (Fable) — decision input, not dispatch authority
- Evidence base:
  - [Amp Code teardown](../teardowns/2026-07-16-amp-code-teardown.md)
  - [OpenAgents product adaptation analysis](../teardowns/2026-07-10-openagents-product-adaptation-analysis.md)
  - [Sol Master Roadmap](../sol/MASTER_ROADMAP.md) (rev 115)
- Boundary: the roadmap currently authorizes no product-expansion lane. This
  essay describes what the build would look like if the owner opened one. It
  creates no issues, claims, or promises.

## The one-sentence thesis

Amp's product is not a terminal agent — it is a durable, searchable,
remotely-controllable **thread fabric** with a TUI bolted on as one of several
clients. OpenAgents has already landed or specced almost every load-bearing
piece of that fabric — typed Runtime Gateway, durable admission,
Thread/Turn/Item vocabulary, Khala Sync projections, Pylon placement, Agent
Computers, AssuranceSpec fan-out, signed distribution — and we have the one
client Amp conspicuously does not have: **a real desktop application**. The
delta between us and Amp is composition, not invention. Composed aggressively,
it is a few days of work to reach functional parity on every surface a user
can name, and on half of them we ship something categorically better, because
our architecture was designed for exactly the laws Amp's teardown shows they
skipped.

## What Amp actually is, stripped of the terminal

The teardown reduces Amp to eight product capabilities:

1. **The thread as the canonical work object** — searchable, referenceable,
   shareable, exportable, alive after compaction.
2. **Queue / steer / interrupt** as three distinct user intents, in both the
   interactive client and the automation stream.
3. **Model-routed modes and specialists** — low/medium/high/ultra plus Oracle,
   Librarian, search, review, media, compaction, each routed to a different
   model.
4. **`read_thread`** — a bounded agent that searches original history, treats
   tool calls as attempts not outcomes, and never confuses a compaction
   summary with evidence.
5. **Review as fan-out** — Markdown check manifests, one subagent per check.
6. **Plugins as an operating system** — TypeScript extensions adding tools,
   UI, commands, modes, lifecycle continuation, and whole agents.
7. **Placement** — runners (your machine as a target) and Orbs (their machine
   as a product), plus web/mobile remote control of a live CLI thread.
8. **One executable, many hosts** — TUI, headless stream JSON, SDK, runner,
   IDE bridge, all converging on the same thread service.

Everything else — the Bun binary, the RivetKit actors, the stream-JSON
Claude-compatibility shim — is implementation, and much of it is
implementation we would refuse anyway (same-origin checksums, disabled
release signing, cloud-canonical transcripts, default-open tool execution).

Notice what is *not* on the list: a TUI. Amp's own trajectory proves the
terminal is incidental — they killed their editor extension, killed Tab
completion, and their manual celebrates web and mobile as the review surfaces.
The terminal is where Amp ran out of ambition, not where its value lives. We
do not have that constraint. Our client is a signed, hardened, Effect Native
Electron application with the Autopilot design language — Disket Mono, indigo
`#5262FD` on near-black, square corners, one theme — and every capability
below lands as pixels in that app, not as escape codes in somebody's tmux
pane.

## The mapping: every Amp capability onto substrate we already own

| Amp capability | OpenAgents substrate | State today | Delta to parity |
| --- | --- | --- | --- |
| Durable thread with stable ID/URL | Thread/Turn/Item over the Runtime Gateway. Durable admission. Local evidence log | Landed (MVP-1/MVP-2 accepted, #8653 typed thread projection) | Add thread search + cross-reference. Expose share/export commands |
| Thread search (text, file, repo, label, date) | Local SQLite index over the typed event graph (adaptation analysis §Codex: append log + indexed authority) | Index pattern proven. Search UI absent | One indexed query service + one Desktop search surface |
| `read_thread` bounded reader | Bounded thread-reader role over exact accepted events with supersession/acceptance refs (adaptation §18.1/18.5) | Specced, not built | One read-only agent + one retrieval tool. Citations into the canonical log |
| Queue / steer / interrupt | Explicit steer-at-safe-boundary vs queue-until-yield already required by the frozen protocol contract (adaptation §1) | Contract written. Composer wiring partial | Composer affordances + command IDs through the registry |
| Modes (low/medium/high/ultra) | Typed provider/model routing with exact per-call receipts. Pylon multi-engine supervision | Provider execution + exact usage receipts live-proven (#8547) | A routing policy table + a mode picker. Receipts already exist |
| Oracle / specialist diversity | Deliberate different-model specialist calls, receipted (adaptation §18.4) | Not built | One `consult` command routed to a second model family. Trivially composed once modes exist |
| Compaction that never destroys evidence | Local append-only accepted events are already the authority. Summaries are projections | Landed by construction | Nothing — we get this free. Amp had to build a reader to recover it |
| `amp review` check fan-out | AssuranceSpec obligations + work packets + child-agent fan-out | **Complete and owner-accepted** (18/18 obligations, published npm packages) | Compile a Markdown check file into an AssuranceSpec manifest — a parser, not a program |
| Subagents with isolated context | Complete child-agent topology, independent transcripts, causal edges — a D1 exit and MVP acceptance criterion | Landed, and **stronger**: Amp returns a final summary. We keep the whole navigable graph | Nothing |
| Plugins (tools/UI/modes/agents) | Signed MCPB-compatible catalog with capability manifests, isolation, generations (adaptation §6, decision 20) | Specced. MCP support partial | The honest long pole — days gets tools+commands under signature. Full UI/agent extensions come later |
| Runners (your machine as target) | Pylon placement + one claim registry across owner-local and managed targets | **Live-proven** (#8636) | Expose "enroll this machine" in Desktop settings |
| Orbs (managed machines) | Agent Computers: authorized capacity, bounded workspace, writeback, verification, reclaim, receipts | **Live-proven** (#8547, FC-4 hybrid client acceptance) | Reopen the lane. The hard distributed work is done and receipted |
| Web/mobile remote control | Khala Sync projections + idempotent commands + mobile client with PKCE identity | Landed both clients (R1/R2). Live cross-device continuation is the named honest gap | The one real demonstration debt — matching refs/versions/cursor through a network gap |
| Headless / SDK / stream JSON | Generated clients from the Effect Schema protocol. Embedded and remote transports hitting one request processor | Contract frozen. Generation pipeline partial | Emit the generated TS client. Optionally emit a stream-JSON compatibility projection for Amp/Claude tooling |
| IDE bridge | Typed foreign-host nodes (editor/PTY/diff) with source-side minimization | Pattern proven (effect-native #67/#70). Not mounted | Post-parity polish, not parity |
| Skills / lazy MCP | Hash-pinned read-only skills (the `productspec-work` skill is the live example) + deferred MCP admission | Landed for skills. MCP deferral specced | Wire deferral. Refuse Amp's start-before-activation trust bug |
| Sharing / visibility | Owner-scoped Sync projections. Public-safe receipts on openagents.com | Receipts public (Observatory). Thread sharing absent | Explicit visibility states with receipts — and we say "internet-readable," never "unlisted" |

Read the right-hand column honestly: the majority of rows are **already
landed, live-proven, or owner-accepted**. Three rows are real work (search,
thread-reader, plugin signing), one row is a demonstration debt (live
cross-device continuation), and the rest are UI composition over receipted
substrate. That is what "a few days" means — not that distributed systems are
easy, but that we already paid for the distributed systems.

## What it looks like in the Desktop app

No TUI. Here is the same eight-capability list as an application:

**The Workroom is the thread.** A session opens as a causal timeline in the
main pane — turns, tool attempts, child-agent spawn edges, blockers, exact
usage — in Disket Mono on near-black. Every row is a typed event, not a
rendered chat string, so the same view survives restart, reload, and device
handoff with matching cursors. Where Amp gives you a `T-` URL, we give you a
stable session ref that Desktop, mobile, and the web receipt surface all
resolve to the same durable record.

**⌘K is Amp's whole command surface, done properly.** Amp deleted slash
commands in favor of a palette. We started with a typed command registry as a
D2 gate. New thread, continue, steer, interrupt, search threads, consult
Oracle, run review, enroll runner, share — each a registered intent with
schema, policy, idempotency, and one durable outcome, invocable from palette,
keybinding, menu, mobile, or SDK identically.

**Steer is a visible affordance, not a keystroke incantation.** While a turn
runs, the composer shows two commit paths: **Queue** (delivered when current
work yields) and **Steer** (delivered at the next safe boundary), plus a
persistent **Stop**. Amp encodes this in Enter/double-Enter/double-Escape and
hopes you read the manual. We put the semantics on screen, and underneath,
each is a durable command with an admission acknowledgement before the UI
calls it accepted.

**Modes are a routing panel with receipts.** A mode picker — call the tiers
whatever we like — sits above the composer, and next to it a small disclosure:
exactly which model, provider, prompt generation, and data-retention class the
current mode resolves to, updated per call from the receipt stream. Amp hides
routing changes behind friendly names and a Chronicle post. We show the
routing fact at the moment it matters. The Oracle becomes a **Consult** action
that visibly fans out to a different model family and returns advice labeled
as advice — never authority.

**Search is a first-class surface.** ⌘⇧F over every thread you own: text,
file, repository, label, date, criterion ID. Results are event refs, and
clicking one lands you in the original timeline at the original moment — with
supersession badges when a later event reverted or replaced what you found.
This is `read_thread` for humans. The bounded thread-reader agent is the same
index with a tool schema in front of it.

**Review is the Assurance surface we already shipped.** Drop check manifests
in the repo. The Review pane compiles them into an AssuranceSpec run, fans out
one bounded child per check, and renders mapped / executable / observed /
accepted as four independent facts per criterion — the exact four-fact
presentation the public Observatory trace already deploys. Amp's `amp review`
returns subagent prose. Ours returns obligations with falsifier receipts.

**Placement is a target picker, not a flag.** The session header names where
work runs: **This Mac**, an enrolled machine (Amp's "runner," minus the
default-off setting you have to discover in a manual), or a managed Agent
Computer (Amp's "Orb," except ours has receipted capacity, bounded writeback,
verification, and reclaim, live-proven under #8547). Changing placement is a
typed command against the one claim registry. No silent retarget, ever — a
law we proved under #8636 and Amp cannot demonstrate at all.

**Your phone is the remote control.** The same session, the same refs, the
same steer/queue/stop commands from the mobile app — with durable admission
acknowledgements, worker epochs, and replay, so a command sent from a subway
tunnel resolves to exactly one outcome. Amp's web remote control is genuinely
good product. Ours differs in that the phone holds no desktop token and the
cloud holds no canonical transcript — Sync distributes projections of records
whose authority stays typed and local-first.

**And per owner mandate: no CLI runbooks.** Enrollment, visibility, mode
policy, plugin approval, placement — all screens and buttons. The moment a
capability's only interface is a shell command, it is not shipped.

## The few days, concretely

Assume an owner decision opens the lane. Continuous parallel child-agent
lanes, Khala-first cadence, each day's output landing behind the existing
MVP-2 recovery gates:

**Day 1 — the thread fabric surfaces.** Thread search index over the typed
event graph + the ⌘⇧F surface. Share/export commands with explicit visibility
states and receipts. Wire steer/queue/stop composer affordances to the
already-specced delivery semantics. Exit: search any historical session and
land on original events. Steer a live turn from the UI with a durable
admission ack.

**Day 2 — routing and specialists.** Mode table mapping tiers to
provider/model/policy with per-call routing receipts (the receipt path is
already honest — it caught a real double-billing bug). Mode picker + routing
disclosure in Desktop. The Consult (Oracle) command routed to a second model
family. Compaction-as-projection audit confirming summaries never shadow
evidence. Exit: four modes, one consult, every call receipted with exact
model/provider/retention facts.

**Day 3 — review fan-out and the thread reader.** Markdown-check →
AssuranceSpec compiler. Review pane rendering the four-fact criterion grid.
the bounded thread-reader agent (search index + read-only tool + citation
outputs, no shell/publish/spend authority). Exit: a repo-defined check suite
runs as bounded fan-out. An agent answers "what did we try in session X and
what superseded it" with event citations.

**Day 4 — placement and remote control.** "Enroll this machine" in Desktop
settings over the existing claim registry. Agent Computer placement in the
target picker over the #8547/#8636 substrate. The live cross-device
demonstration — a streamed thread started on Desktop, steered from the phone
through a network gap with matching refs/versions/cursor. That demo is the
roadmap's own named honest gap, and this is the day it dies. Exit: one
session, three placements, two clients, zero duplicated or orphaned work.

**Day 5 — the developer surface.** Generated TypeScript client from the
Effect Schema protocol (our SDK, except the protocol is public and generated
rather than commercially licensed around a 404 repo). Optional stream-JSON
compatibility projection so Amp/Claude-shaped tooling can drive us. Headless
execution against the same request processor as the UI. First signed
plugin-catalog slice — tools and commands under publisher signature, content
hash, and declared capabilities, running isolated. Exit: a third-party script
starts, steers, and reads a thread through the generated client. A signed
tool plugin installs, runs, and produces an install/run receipt.

Beyond day 5 sits the honest tail that Amp also has not finished, and where
we refuse to pretend: full plugin UI/agent extensions under real capability
isolation (Amp ships these with no demonstrated containment — matching that
speed by matching that recklessness is not on the table), IDE bridges, and
hardening every new surface through the MVP-2 fault taxonomy. A few days buys
everything a user can see. The trust properties underneath are the part we
already built — which is precisely why the days are few.

## What we get that Amp cannot give its users

Parity framing undersells this. Running the Amp feature set on our
architecture is not a clone. It inverts Amp's weakest choices:

1. **Local-first truth vs. cloud transcript authority.** Amp's threads live
   in multi-tenant Postgres. Workspace admins can read private threads.
   deletion is a 30-day policy promise. Ours are local accepted events with
   receipts, optionally synchronized as owner-scoped projections. Your work
   history is yours by construction, not by policy.
2. **Receipts vs. mode names.** Amp reroutes models under stable friendly
   names and tells you in a changelog. Every OpenAgents call binds exact
   model, provider, prompt/catalog generation, cost, and retention class into
   evidence at execution time.
3. **Fail-closed containment vs. default-open execution.** Amp runs tools
   without asking and suggests you write a policy plugin. Our execution
   profiles separate the authority manifest (what policy admitted) from the
   execution receipt (what containment was actually established), and a
   missing sandbox fails the run rather than silently running on the host.
4. **Signed releases vs. same-origin checksums.** Amp's installer verifies a
   vendor checksum from the vendor's own origin, minisign commented out, the
   binary failing Gatekeeper. We ship signed, notarized artifacts with a
   component compatibility ledger and receipted install/update/rollback —
   already an MVP-3 acceptance criterion, already exercised.
5. **An open, generated protocol vs. a commercial SDK around a 404.** Our
   load-bearing seam is public Effect Schema. Clients are generated from it.
   Compatibility is a contract, not a courtesy.
6. **A whole child graph vs. a final summary.** Amp is candid that subagents
   return one summary and cannot be steered. Our complete navigable topology
   with independent transcripts is an accepted MVP criterion. On this row Amp
   is not behind on polish. It is behind on architecture.
7. **A desktop application vs. a TUI.** Not an aesthetic preference — a
   capability class. Typed review grids, routing disclosures, target pickers,
   four-fact assurance panes, and owner-operable settings screens do not fit
   in a terminal grid, which is why Amp pushed its own review surface to the
   web. We keep it in the signed app, offline-capable, one theme, no
   light/dark split, no pixel of it rented from a live web deployment.

## What we refuse to copy, on purpose

The teardown's reject list is our reject list, and the few-days plan honors
all of it: no default tool execution on the host. No cloud transcript as sole
authority. No closed canonical protocol. No plugin privilege bundles. No
same-origin release authority. No mode names hiding execution facts. No
compaction-as-history. No workspace join silently rewriting old-thread
visibility. No "unlisted" euphemism. No AI permission judge as the mandatory
deny boundary. No child summary standing in for child history. No
"no-backcompat" shrug over durable work. Several of these refusals cost us
nothing because the architecture already forbids them. That is the quiet
point of this essay: **the laws we spent months encoding are what make the
sprint short.** Amp built fast by deferring trust. We built trust first, and
now the fast part is cheap.

## The honest caveats

- **Authorization.** Roadmap rev 115 closes all product-expansion lanes.
  runners, managed placement, mobile continuation, and plugin surfaces are
  closed not-planned. Everything above requires a new bounded owner decision.
  The substrate being live-proven does not reopen a lane. Only the owner does.
- **"Few days" is UI-parity days, not hardening days.** Durable-admission
  fault injection, plugin isolation proofs, and the R4-class interruption
  matrix for every new surface take longer, and pretending otherwise is the
  exact rung inflation our proof vocabulary exists to prevent. Day-5 parity
  ships at fixture-proven/live-proven rungs per surface, with the rung stated,
  per surface, in the UI-visible receipt.
- **Pixel proof or it did not happen.** Per standing policy, none of the
  Desktop surfaces above count as shipped without screenshots of the actual
  rendered feature from the signed app. Clean typechecks are not rendering.
- **Amp's velocity is a real advantage we should respect.** They delete
  ruthlessly and ship weekly. Our answer is not to out-delete them but to make
  deletion safe: every retired surface gets a migration, export path, and
  deletion gate. Speed with receipts beats speed with apologies, but only if
  we actually keep the speed.

## Closing

Amp's most valuable idea — the thread as a durable, searchable, steerable,
placeable work object — is an idea our architecture already embodies more
honestly than Amp's does. They got there by centralizing truth in their cloud
and shipping a terminal to feed it. We got there by typing every seam,
receipting every effect, and building the one client this category still
lacks: a signed desktop application that treats agent work as legible product
state instead of scrollback. Given an owner decision and a few days of
composed lanes, OpenAgents does not catch up to Amp. It demonstrates that
everything Amp does was always the easy half of the problem — and ships the
hard half, already proven, in the same window.
