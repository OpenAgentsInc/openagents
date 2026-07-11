# The Sol Corpus and the Teardown Corpus: A Longform Analysis

- **Date:** 2026-07-11
- **Author:** Fable (agent)
- **Status:** Analysis and adversarial review. This document is not roadmap
  authority. Sequencing authority remains `docs/sol/MASTER_ROADMAP.md`
  (Revision 31 at time of writing); factual status authority remains current
  code, tests, live issue state, and receipts.
- **Source snapshot:** `openagents` `main` at `c930085f3c`, 2026-07-11.
- **Corpus read:** every file in `docs/sol/` (22 top-level documents plus the
  44-file `docs/sol/issues/` set, including `MASTER_ROADMAP.md` in full) and
  every file in `docs/teardowns/` (12 documents). Roughly 1.5 MB of prose.

---

## 1. Executive summary

The two directories form one system with three layers, and the system is more
interesting than any single document in it.

`docs/teardowns/` is the **evidence layer**: point-in-time, evidence-tagged
autopsies of the four products closest to what OpenAgents is building —
OpenAI's ChatGPT/Codex desktop stack, Anthropic's Claude desktop app and
Claude Code engine, and OpenCode's V1/V2/Effect architecture — plus two
subagent-rendering deep dives and two bridge documents that convert the
findings into OpenAgents decisions.

`docs/sol/` is the **decision and execution layer**: a canonical master
roadmap (revised from 24 to 31 in roughly forty-eight hours), an operating
model, claim and challenge protocols, dated strategy analyses with
supersession banners, and 44 checked-in issue source records that mirror the
live GitHub issue ledger down to commit hashes and behavior-contract IDs.

Three findings dominate:

1. **The teardowns converge on a single architecture, and OpenAgents has
   correctly identified it.** All four reference products, despite wildly
   different pedigrees, arrive at the same shape: one agent engine behind a
   versioned typed protocol, consumed by thin clients that never own the
   conversation; durable append evidence beside an indexed projection;
   approval as a separate concern from containment; and subagent topology
   that is richer on disk than any shipped renderer shows. The adaptation
   analysis distills this into OpenAgents vocabulary — Thread → Turn → Item →
   Work Unit/Receipt, authority manifests versus execution receipts, five
   execution profiles, five service scopes — and pairs it with six explicit
   rejections (no browser fork, no ambient screen memory, no live-site
   renderer, no sidecar sprawl, no transcript-as-authority, no child-authority
   widening). This is the strongest part of the corpus.

2. **The real invention is the governance machinery, not the architecture.**
   Six proof rungs (code-landed → fixture-proven → deployed → live-proven →
   owner-accepted → closed), a claim protocol with hot-contract semantics, a
   challenge ledger where every strategic disagreement gets a falsifier and a
   review point, supersession banners instead of silent rewrites, behavior
   contracts with executable oracles, and — most unusually — **retained
   failure receipts**: the two rejected FleetRun closeouts from July 10 were
   kept as evidence and converted into bounded repair issues (CUT-05, CUT-06)
   within twenty-four hours. For an organization operated largely by agents,
   this anti-fabrication epistemics is the load-bearing asset.

3. **The strategic pivot was violent but the substrate survived intact —
   and the corpus has not fully caught up with itself.** Between July 9 and
   July 11 the program went from Sarah-first (a persistent named AI
   relationship as the product's front door) through a reliability reset
   ("all sarah shit must die") to a remote-first portable-sessions thesis
   (the coding session, not the host, is the product object). Every layer of
   persona and presentation died; every layer of typed authority, Sync
   discipline, and receipt honesty survived unchanged. But the revision
   velocity has left measurable drift: documents citing Revision 25 or 29
   under a Revision 31 roadmap, a voice capability that is simultaneously
   "paused" and "P0" in different ledgers, and the newest strategic layer
   (the Revision 30 portability packets) being the least instrumented — it
   has no issue numbers yet, while long-closed lanes carry exhaustive commit
   receipts.

Everything user-visible now funnels through two physical-world gates: the
\#8676 physical-device mobile continuation receipt (blocked, at time of the
issue record, on an offline paired iPhone) and the #8640 Phase A live burn
(one accepted simultaneous Codex + Claude FleetRun through named isolated
accounts). The rest of this document works through the corpus in detail.

---

## 2. The corpus as a system

The teardown README states the pipeline explicitly: teardowns are "design
evidence, not current OpenAgents status," and "when a teardown lesson becomes
a product requirement, move it into the owning typed contract, roadmap gate,
issue, and verification surface. Do not leave a load-bearing decision only in
competitive analysis."

Observed in practice, the pipeline has five stages, and there is now at least
one complete traversal of it:

1. **Teardown.** The Codex subagents rendering analysis establishes that
   Codex persists an explicit agent graph (`agent-graph-store`, one directional
   parent edge per child, BFS descendant traversal) which its own terminal
   client then flattens into one-line edge rows and a hard-capped six-item
   peek — a "capability gap by rendering surface."
2. **Adaptation decision.** The OpenAgents subagents design doc names that
   split as the anti-pattern and states the counter-premise: one typed
   sub-agent projection rendered honestly by every surface at
   surface-appropriate density; "silent truncation or lost reachability is a
   bug; identical simultaneous layout is not required."
3. **Roadmap absorption.** MASTER_ROADMAP Revision 31 encodes it as owner
   decision 22 ("agent topology is a live operating surface") and laws 28–30.
4. **Issue and implementation.** #8674 lands the loss-accounted three-pane
   Codex history workspace with the completeness equation
   `source items = rendered items + explicit redactions + explicit gaps`,
   proven against a real receipt of 131 threads and 560,418 records with zero
   unsupported gaps; #8675 lands the real-Electron acceptance journey.
5. **Receipt and feedback.** The #8675 acceptance journey surfaced a real
   restoration-clobbering defect and a zero-height SplitPane defect that was
   fixed in the shared renderer and upstreamed to the public Effect Native
   repo — the pipeline flowing back out of the product into the framework.

This traversal — competitor evidence to shipped, oracle-tested, upstreamed
code in roughly two days — is the corpus's proof of concept for its own
method. The remaining question, taken up in §7, is whether the method scales
past the parts of the plan that are fun to specify.

A second structural observation: the two directories deliberately share
vocabulary. "Loss-accounted," "causal inline child activity," "preview prose
is never completion authority," "shell paints before detail" appear nearly
verbatim in the adaptation analysis, the subagents design, the Episode 248/249
addenda, MASTER_ROADMAP decision 16/22, and issue records #8674–#8676. The
corpus is not a pile of documents; it is one argument, versioned.

---

## 3. The strategic arc: three theses in forty-eight hours

### 3.1 Sarah-first (July 9)

The seven dated 2026-07-09 documents articulate a complete, internally
coherent product thesis: OpenAgents as "a relationship-centered operating
system for delegated work," with Sarah — a persistent, disclosed-AI,
policy-scoped persona — as the organizing unit. The best of these documents
(`authority-trust-and-economics`, `sarah-first-product-architecture`) are
careful in a way that aged well: "Sarah is the product's interpreter and
presenter. She is not its universal authority." The authority chain —
natural-language request → typed intent → authenticated scope → policy and
budget → explicit approval → bounded executor → independent verification →
exact receipt → public-safe projection — was designed so that no stage could
be replaced by "Sarah said so."

The risk document (`risks-tensions-and-decision-tests`) deserves specific
credit: its Tension 10 proposed instrumenting where successful repeat work
actually begins and, if experts bypassed Sarah, "reposition Sarah as the
default relationship rather than an exclusive shell." Its Tension 9 warned
that constant motion accumulates integration debt. Both effectively predicted
the reset that killed the thesis they were defending. The corpus never states
which falsifier fired — only that the owner decided.

### 3.2 The reliability reset (July 10, Revisions 24–29)

The owner escalation ("all sarah shit must die," epic #8610) removed the
Sarah surface entirely: `/sarah` and `/sarah/api/*` became 404 tombstones,
`apps/sarah` was deleted, the GPU render node was stopped, and the behavior
contracts were preserved verbatim as `retired`. P0 became reliable direct
Desktop/mobile coding and fleet software, governed by the R0–R7 gate ladder
(truthful green foundations → shared identity → Sync continuity → real fleet
operations → fault safety → OpenCode-parity Desktop → compact mobile coding →
signed releases and sustained dogfood).

What is analytically important is the **surgical selectivity of the kill**.
Every 2026-07-09 document received a supersession banner with a per-document
retention verdict rather than deletion: the persona and presentation layers
died; the typed-authority doctrine, the Sync laws, the six proof rungs, the
exact-or-`not_measured` accounting, the named-isolated-accounts rule, and the
Effect Native closed-catalog thesis all survived without modification. That
is possible only because the July 9 authors had genuinely separated substrate
from packaging. The pivot validated the architecture by discarding the
product wrapped around it.

### 3.3 Remote-first portability (July 10–11, Revisions 30–31)

On July 11 the owner inverted the frame again, in four points recorded
verbatim in the remote-first pathway document: sessions are remote-first and
host-independent (stop on any machine, move to any other, "handoff to
cloud"); remote targets include the owner's homelab and managed providers
(Daytona-class, behind a provider-neutral contract); secrets flow through a
general capability broker ("gondolin or agyn style"); and mobile reaches any
session on any host, with conversational voice.

The response is architecturally disciplined: rather than rewriting the local
plan, Sol split the program. The CUT-01–CUT-27 graph (#8681–#8707) proves the
local coding cutover and **explicitly excludes** remote workrooms, host
movement, Daytona, and voice from its acceptance — so a local success cannot
be laundered into remote claims. The portability layer is specified as eight
contract additions (`coding_session` as the host-independent product object;
generation-fenced `session_attachment` with quiesce/checkpoint/detach/attach/
move/abort semantics; secret-free content-addressed `session_checkpoint`;
provider-neutral target descriptors; the capability broker with short-lived
leases where a move revokes source grants; the any-host session directory;
session-neutral controls; and narrowly reauthorized session-neutral voice)
plus nineteen pending invariants and an ordered Packet 0–8 pathway whose
critical path is durable session authority plus the broker.

The one-sentence product in Revision 31 is now: "one durable session can
execute on, stop on, and move between authorized local, owner-managed, and
managed-cloud hosts without forking identity, state, authority, secrets, or
receipts." Version 1 honestly promises stop/checkpoint/rehydrate, not live
memory migration.

**Interpretation.** The three theses are not random walk; they are a monotone
descent from presentation toward substrate. Sarah-first bet on the
relationship layer; the reset bet on the client layer; remote-first bets on
the session layer. Each pivot discarded the layer above and kept everything
below. If the pattern holds, the durable company asset is the bottom of the
stack: typed authority, Sync, receipts, and the broker — which is also
exactly the part the teardowns say no competitor ships openly.

---

## 4. What the teardowns actually establish

### 4.1 The convergent architecture

Four products, four pedigrees, one shape:

- **ChatGPT/Codex desktop:** a closed host on "Owl" — a first-party Chromium
  fork with an Electron compatibility layer — spawning the open Rust Codex
  engine as `codex app-server` over stdio JSON-RPC. The engine owns
  Seatbelt/Landlock containment, a Starlark exec-policy engine, a guardian
  safety subagent, and ~260 MB of runtime; the app adds a V8 "code mode"
  host, a continuous screen-recording ambient-memory service (Chronicle, with
  a parallel "Skysight" event-memory system), a computer-use runtime, a
  plugin marketplace with internal risk gates, and even a hardware macropad
  driver.
- **Claude desktop:** stock Electron 42 orchestrating a live claude.ai
  renderer (with a complete bundled SPA beside it), a separately downloaded
  and versioned Claude Code sidecar spoken to over `stream-json` stdio with a
  stdio permission-prompt callback, small Swift/Rust native modules, and a
  hash-verified Linux VM under Virtualization.framework for guest execution.
- **Claude Code:** a local agent engine with several front doors (TUI,
  headless, SDK stream, IDE, remote control), seven distinct authority
  layers, 27 hook lifecycle events, append-only JSONL session graphs, file
  checkpoints, fork/rewind, outcome-sensitive worktree cleanup — and two
  parallel query owners plus an 88-flag compile-time build matrix as its
  documented fault lines.
- **OpenCode:** the only fully open reference. V1 proves Electron-sidecar
  topology (utility-process server, per-launch password, ticketed PTY
  WebSockets, eager-subscription SSE); V2 proves the semantics — durable
  idempotent admission before execution, steer-versus-queue mid-run delivery,
  exact-replay-equality durable events, process-local-only liveness,
  captured-at-advertisement tool generations, staged two-phase revert — all
  on Effect 4 as the entire application kernel, with an explicit
  global/Location service-scope law enforced by architecture tests.

The convergence is precise enough to be falsifiable, and the corpus notes the
strongest single data point: Codex's own TUI completed its migration to being
just another app-server client — "the terminal client no longer needs a
different conversation owner." When OpenAI, Anthropic, and an open-source
competitor independently converge on one-engine-many-thin-clients with a
versioned typed seam, treating that as settled architecture is sound.

### 4.2 The instructive divergences

Three divergences matter more than the convergence:

**Runtime strategy.** OpenAI forked Chromium; Anthropic stayed stock and put
capability into narrow native modules, a versioned sidecar, and a VM. The
teardowns read Anthropic as the stronger argument: everything Owl supposedly
justifies is achieved with stock Electron plus discipline, while keeping
upstream Chromium security releases. OpenAgents' Electron + Effect Native
choice follows this, with the fuse-by-fuse hardening list copied into D0/D6
oracles.

**Open/closed inversion.** OpenAI ships an open engine under a closed host;
Anthropic ships a closed engine under open SDK/extension boundaries. Neither
is open at the load-bearing seam end-to-end. That inversion defines the
differentiation lane OpenAgents claims: open engine *and* open protocol
authority, with receipts instead of opaque product state.

**Containment location.** Codex compiles named permission profiles into
fail-closed OS enforcement inside the engine; Claude Desktop pushes untrusted
execution into a VM; Claude Code leaves sandboxing opt-in; OpenCode has
approval precedence but explicitly no containment (shell matching is policy;
plugins are trusted in-process code). And both vendors run their own machines
wide open — the ChatGPT teardown found `approval_policy = "never"` and
`sandbox_mode = "danger-full-access"` in the local config. The recurring
lesson, encoded in the adaptation analysis as separate authority-manifest and
execution-receipt records: **approval is never containment, and a shield icon
that conflates them is lying.** If promised containment is unavailable, the
profile fails closed rather than silently degrading to host execution.

### 4.3 The subagent finding

The two rendering analyses form a matched pair. Codex has the stronger
topology contract (explicit persisted graph) and the weaker rendering (the
TUI renders nothing for an in-flight spawn and caps the status feed at six
items); Claude has the richer evidence (1,870 sidechain files, 631,492
records parsed, near-complete child agent loops) and the weaker topology
(parent edges reconstructable only at 97.27%, with a permanent ~3% orphan
class; the `/resume` picker deliberately hides children). In both products,
*disk evidence is richer than any rendered view* — and Claude's history shows
orchestration evolving fast (background-by-default flipped silently between
2.1.195 and 2.1.196; Workflows, mailboxes, and independent background
sessions all landed inside five weeks on one stable sidechain primitive).

OpenAgents' synthesis — complete roster, one causal child card at the exact
parent position, independent child transcripts, counted gaps, provider
imports through loss-accounted graph adapters that never treat `parentUuid`
as an agent edge — is the correct combination, and it is already partially
shipped (#8674/#8675) with the live-Sync half correctly fenced into follow-on
issues (#8691/#8692) rather than silently claimed.

### 4.4 The shared disease

Every reference product carries the same pathology: compatibility debt
without deletion gates. Codex has 125 crates, 92 feature-registry entries,
removed-but-parsing flags, and parallel V1/V2 policy concepts; Claude Code
has 88 compile-time symbols and two query owners; OpenCode's desktop still
embeds the V1 server underneath a finished V2 engine; Claude Desktop runs
five independently moving update planes. The teardowns' prescription is
uniform and appears in the roadmap's laws: every bridge gets an owner, an
expiry gate, and a deletion milestone, and "declaring migration done while
Desktop embeds V1" is the named failure to avoid. Given that OpenAgents'
own history already includes frozen legacy clients (`clients/khala-mobile`,
`clients/khala-code-desktop`), an Electrobun false start, and a deleted
`apps/sarah`, this is the warning most worth internalizing — the corpus knows
it, which is half the battle, but knowing is not immunity (see §7.6).

---

## 5. The adaptation layer: vocabulary as architecture

The product adaptation analysis is the corpus's keystone document. Its method
is to compress seven teardowns into (a) named vocabulary, (b) explicit
rejections, and (c) an ordered consequence table written entirely in Sol
roadmap coordinates. The vocabulary is worth listing because it now recurs
across the roadmap, the issue records, and shipped schema names:

- **Thread → Turn → Item → Work Unit / Receipt** as the public hierarchy,
  extending Codex's durable vocabulary with delivery semantics Codex lacks.
- **WorkContext** as a stored service scope (from OpenCode V2's Location):
  a caller may not replace execution context by supplying a different
  directory.
- **Authority manifest vs execution receipt**: what policy admitted versus
  what containment was actually established — two records, never one.
- **Five execution profiles**: projection-only, workspace-bounded, isolated
  guest, owner-local danger mode, managed cloud — each fail-closed.
- **Five service scopes**: process, WorkContext, run, request, foreign-host —
  published as law before service count grows (adopted from OpenCode's
  ambient-to-explicit migration, with the explicit instruction *not* to copy
  its `LayerNode` graph compiler wholesale).
- **Durable admission before advisory scheduling**, with client-chosen IDs,
  exact-retry reconciliation, and steer-versus-queue delivery semantics.
- **Three read surfaces**: bounded current projections, durable replay log
  with a synchronization marker, volatile live stream with declared gap
  rules — recovery always from the log, never the stream.

The six rejections are equally load-bearing: no ambient screen recording or
inferred personal memory by default (the direct answer to Chronicle/Skysight,
and the place where "sovereign compute" differentiates); no browser-runtime
fork; no remote web deployment as privileged desktop code (the direct answer
to Claude's live-site renderer); no opaque sidecar accumulation; no
transcript-as-authority ("neither model prose nor a green UI row proves a
command executed"); no child-authority widening ("effective child authority
is the intersection of parent grant, child policy, WorkContext, and
containment" — rejecting OpenCode V2's child-profile independence).

Two things elevate this beyond competitive analysis. First, the status ledger
appended to the document grades the plan against live issues in the six-rung
vocabulary, including honest gaps ("live cross-device continuation not yet
demonstrated," "no unified command registry"). Second, it generates upstream
corrections to its own program — most notably the R1 auth-model critique:
R1 as shipped is auth-required, conflating "has identity" with "has an
account," and the owner's intended two-tier model (device-local identity by
default, account linking as an additive, reversible upgrade) was filed and
then implemented as #8666/R1-LOCAL. "Open the app, pair locally, no login" is
positioned, correctly, as the opposite of ChatGPT's account-and-attestation
posture.

---

## 6. The governance machinery

If the architecture is largely (and sensibly) adapted from evidence, the
process is original. Five mechanisms stand out.

**Six proof rungs.** Code-landed → fixture-proven → deployed/distributed →
live-proven → owner-accepted → closed, with "no rung implies the next"
enforced in issue templates, CLAIM-RELEASE receipts, and the roadmap's
completion reporting. The corpus polices itself with it: build 116 is
"accepted for App Store delivery" while App Store validation and
physical-device acceptance are explicitly listed as distinct unproven rungs.

**The claim protocol.** Live GitHub issues as the cross-session claim ledger;
claims name hot files *and hot contracts* ("file-disjoint is not necessarily
contract-disjoint" — shared wire schemas, migration sequences, catalog
versions, and generated outputs need one integration owner); staleness
requires both 90 minutes without evidence and a process/worktree audit —
"elapsed time alone never authorizes taking another agent's work." This is a
real answer to a real problem the reference products don't have: OpenAgents
is built by concurrent agents, and the protocol exists because collisions
actually happened.

**The challenge ledger.** Every material strategic disagreement gets a
disposition, an owning issue, a falsifier/tripwire, and a review point —
"disagreement becomes a future test rather than disappearing." The Blueprint
correction/deletion row is the best example: deferred until R7, but with an
automatic tripwire that activates immediately if a real user requests
correction or deletion of remembered information. Deferral with a tripwire is
categorically better than deferral by silence.

**Supersession banners.** Dated analyses are never silently rewritten; they
get banners stating exactly which conclusions survive. This preserved the
entire Sarah-first reasoning chain as auditable history while cleanly
extracting the retained doctrine. The cost is that stale prose remains
physically present (see §7.2), but the alternative — rewriting history —
would destroy the very auditability the system depends on.

**Failure receipts as first-class artifacts.** The July 10 live burn produced
two FleetRuns with *rejected* closeouts (`claude_agent_execution_refused`; a
wrong Vitest entry point; a mobile unit failing bounded-argv on a compound
`&&` verifier; a Desktop push that preceded post-run acceptance). All were
retained, named, and converted into CUT-02/CUT-05/CUT-06 within a day —
CUT-05 closed with a real accepted `claude-pylon-3` run, CUT-06's
deterministic repair landed with the live parent receipt still honestly
marked pending. Most engineering organizations bury failed demos. This one
checks them in.

**Assessment.** The machinery is genuinely strong, and it is tuned for the
specific failure mode of agent-operated development: plausible fabricated
success. Nearly every rule — "counter movement is never proof," "a timeout is
not an outcome," "a mock VM is not production isolation proof," "transport
delivery is not command completion," the completeness equation — is a
tripwire against an agent claiming an unearned rung. The open question is
overhead: the corpus's own Tension 9 (constant motion versus integration
debt) applies to governance prose as much as to code, and at 1.5 MB and
climbing, the reading tax on every new agent session is nontrivial. The
mitigation already in place — the README authority order and the
delegation-packet pattern of freezing contracts and dispatching small leaves —
is the right one; it needs to keep winning.

---

## 7. Tensions, contradictions, and risks

This section is the adversarial part of the review. None of these is fatal;
all of them are real.

### 7.1 Two physical gates hold everything

The dependency chain is: #8676 (one real provider-neutral Desktop stream
continued on a physical phone) → contract freeze (#8677 fault matrix plus the
portability schemas) → #8640 Phase A (two accepted simultaneous closeouts) →
\#8547 (real brokered Firecracker workroom) → #8636 (hybrid routing) →
broker and first session move → R7 dogfood. The first gate was blocked, per
the issue record, on a paired iPhone being offline in Xcode device discovery;
the second requires explicit owner authorization per attempt. An enormous,
precisely-specified plan is currently rate-limited by a cable and a calendar.
That is not a criticism of the plan — it is what "live-proven means live"
costs — but it concentrates schedule risk in two events, and the plan's
breadth keeps growing while those two receipts don't exist.

### 7.2 Revision velocity has outrun reconciliation

The roadmap is at Revision 31; the sol README claims Revision 29 authority;
OPERATING_MODEL, SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS, the issue triage, and
the Terra lane doc cite Revision 25; the parity audit still contains
Sarah-era slice language under a superseding scope note; live issue bodies
for #8566/#8574/#8597 still cite Revision 29. The corpus knows this — the
remote-first doc names the reconciliation gap explicitly, and the working
method orders "reconcile the roadmap; never repeat a stale claim merely
because it is written here." But the drift is now large enough that the
authority-order preamble is doing heavy lifting. A dispatch agent that reads
one document without the README's precedence ladder can act on a superseded
gate (the C2 text still contains the original three-provider requirement
above the amendment narrowing it to Codex + Claude). The system's own
staleness-hazard warning has become a self-description.

### 7.3 The newest strategy is the least instrumented

The Revision 30 portability packets — the owner's actual current ask — have
no issue numbers ("create bounded issue leaves beneath #8566 for these
packets before mutation"). Meanwhile closed lanes carry commit-hash-level
receipts. The inversion is understandable (the discipline is to freeze
contracts before dispatch) but it means the program's center of gravity
currently sits in prose, and the corpus's own rule — the live issue ledger is
the claim authority — implies the portability work cannot legitimately start
until someone does the decomposition. The repeated warning against silently
broadening #8676/#8547/#8636/CUT-27 to absorb it suggests Sol expects
scope-absorption pressure. Filing those leaves is the single highest-leverage
documentation act available.

### 7.4 Voice is in two states at once

Revision 29 closed voice broadly (Sarah wontdo boundary). Revisions 30–31
narrowly reauthorized session-neutral, persona-neutral conversational voice
(PTT, provisional/final ASR, typed intents through the normal registry,
barge-in as typed interrupt, no raw-audio retention, never authority by
itself). But the mobile port ledger still marks native PTT/STT "paused," and
live issue bodies classify all voice as a non-goal. The remote-first doc
flags this; it remains unresolved. Until the ledger row flips, any agent
following the port plan will correctly refuse voice work the roadmap now
requires.

### 7.5 Sarah is removed and load-bearing simultaneously

The persona is dead — routes tombstoned, package deleted, GPU node stopped,
contracts retired. Yet `/api/sarah/fleet-runs` remains the FleetRun intake
authority, FC-1's closed contract is written in Sarah-tool vocabulary, and
`fc-2`'s substrate references survive. The corpus handles this honestly
(server-side surfaces are explicitly noted as retained adapter authority),
but a route named for a removed product is exactly the kind of compatibility
debt §4.4 warns about. It deserves a rename-or-tombstone decision with an
expiry gate, not indefinite ambiguity.

### 7.6 The plan-to-proof ratio

A rough census: the corpus specifies on the order of 27 CUT issues, 8
portability packets, 10 fault-matrix rows, 30 implementation laws, 19 pending
portability invariants, 22 owner decisions, an 8-gate R-ladder, a 7-gate
D-ladder, and an 8-wave M-ladder. Against that: one Desktop history workspace
at owner-accepted, an identity/Sync ladder at fixture-and-deterministic
rungs, a closed fleet substrate, and zero accepted live mixed-provider runs.
The specification quality is genuinely high — but the corpus's own risk
document warned that coherence "can make risk harder to see because every
program has a plausible place in the same story." The R0–R7 reset was itself
a response to exactly this pattern in the Sarah era. The honest metric the
operating model proposes — weekly count of completed user loops, deleted
alternate paths, live receipts, not commits — should be applied to the
documentation layer too: superseded prose deleted, not just bannered, once
its retained conclusions are extracted.

### 7.7 Inherited framework risk

Effect Native's strategic argument was originally coupled to Sarah-first
("without Sarah-first, Effect Native risks becoming an elegant framework in
search of a singular product"); the banner retains the conclusion while the
premise died, and the new anchor (shared typed semantics across Desktop
workbench and mobile remote-coding) was asserted rather than re-argued.
Meanwhile the OpenCode Effect teardown documents the concrete costs of deep
Effect adoption (ambient-context taxes, a bespoke Layer-graph compiler, 49
files of `orDie`, an Effect 4 beta pin as churn risk) — and OpenAgents is on
the same beta family. The mitigations in the corpus (five-scope law with
architecture tests instead of a graph compiler, named perimeter modules,
conformance fixtures, the demand register) are the right ones. But the
honest framing is that OpenAgents is betting the entire UI stack on a
framework it also owns and must simultaneously build — a compounding risk the
roadmap accepts without a standing falsifier. The challenge ledger's
framework row (does the second renderer reduce total effort with zero local
exceptions?) should be re-armed for the post-Sarah program explicitly.

### 7.8 The parity benchmark is moving

The audit pinned OpenCode at July 5 and found upstream already 109 commits
ahead five days later, with movement in exactly the areas OpenAgents plans to
build (session tabs, file tree v2, command palette v2, review persistence).
Destination scoring was brutal and honest: 1 landed, 6 partial, 3 scaffold,
10 absent of 20 areas. "Practical OpenCode parity" (R5/D-ladder exit) against
an unfrozen benchmark either needs a re-pinned definition ("parity as of
commit X, plus a named delta review at D6") or it becomes an unfalsifiable
gate.

### 7.9 Small honesty artifacts worth keeping

Scattered through the corpus are details that would be easy to lose and
shouldn't be: Grok fully built but postponed because of literal HTTP-402
account exhaustion, kept as regression substrate (an economic constraint
shaping an acceptance definition); the smoke test that "passed" with
`responseSenderChip: "SYSTEM"` — an error path recorded as a caught false
positive; the #8678 premature-closure reconciliation note (the failure mode
the process guards against, caught by the process); the schema decoders found
silently stripping excess private fields and hardened to reject them. These
are the receipts that prove the epistemics is real and not performative.

---

## 8. What the corpus gets right that the references don't

Synthesizing across both directories, OpenAgents' claimed differentiation is
specific and, on the evidence gathered, real:

1. **Open at the load-bearing seam.** Neither OpenAI (closed host) nor
   Anthropic (closed engine) is open where it matters; OpenCode is open but
   holds server credentials in the renderer and runs trusted plugins
   in-process. Open engine + open typed protocol + hardened boundary is an
   unoccupied position.
2. **Delivery semantics as first-class state.** Every reference stops at
   "agent completed." The extension of the item ladder through
   `changes_produced → reviewed → integrated → accepted`, with separate
   execution and delivery receipts, addresses the gap the Claude teardown
   names directly: "'Agent completed' should never be a proxy for 'work
   landed.'"
3. **Honest topology on every surface.** No reference product renders its own
   subagent evidence faithfully. The completeness-equation approach is
   already shipped for historical Codex data and specified for live data.
4. **Memory as owned, provenance-bearing state.** Against Chronicle/Skysight
   ambient capture and Claude's overlapping memory sprawl, the
   explicit-staged-memory-with-provenance position (plus the tripwired
   correction/deletion contract) is both a safety and a product stance.
5. **Receipts against fabrication.** Exact-or-`not_measured` usage, no-spend
   closeout proofs, could-not-prove fields — none of the references expose
   anything comparable to the user.

The credible threat to all five is not a competitor copying them; it is the
program failing to convert its specification lead into the two live receipts
(§7.1) before revision velocity (§7.2) or plan mass (§7.6) erodes the
discipline that produced it.

---

## 9. Recommendations

In priority order, all consistent with existing corpus rules:

1. **Land the two gates.** Everything else is secondary to the #8676
   physical-device receipt and the owner-authorized #8640 Phase A burn. Both
   are ready at the deterministic rung; both need scheduled physical-world
   time.
2. **File the Revision 30/31 portability leaves** under #8566 (session
   authority, attachment/checkpoint, broker, target adapters, directory,
   voice) before any portability code moves. The prose is done; the ledger is
   empty; the corpus's own rules block dispatch until this happens.
3. **Run one reconciliation sweep** pinning every active document and live
   issue body to Revision 31: the sol README's revision pointer, the
   OPERATING_MODEL/SUBSYSTEM revision citations, the C2 gate text, the voice
   row in the mobile port ledger, and the #8566/#8574/#8597 bodies. Cheap,
   mechanical, and it retires the whole class of §7.2 hazards.
4. **Give the Sarah-named server surfaces an expiry decision** — rename
   `/api/sarah/fleet-runs` behind a neutral route with a compatibility alias
   and a deletion gate, or explicitly tombstone the rename as wontfix with a
   reason. Either outcome beats ambiguity.
5. **Re-pin the parity benchmark** (OpenCode commit + dated delta review at
   D6) so R5 stays falsifiable.
6. **Re-arm the framework falsifier** for Effect Native in the challenge
   ledger with a post-Sarah review point (e.g., at D3: did the editor/PTY
   foreign-host slices cost less than they would have in a conventional
   stack, with zero local catalog exceptions?).
7. **Start deleting superseded prose on a cadence.** The banners were correct
   at pivot time; ninety days out, extracted-and-deleted beats
   bannered-and-retained for everything that isn't an active falsifier or a
   legal/receipt artifact. The corpus should apply its own "delete the
   replaced path" law to itself.

---

## 10. Closing assessment

Read end to end, the corpus documents something rarer than a good
architecture: a working epistemics for building software with agents. The
teardowns are genuinely excellent competitive research — evidence-tagged,
privacy-bounded, and converted into decisions rather than admiration. The
architecture extracted from them is the consensus architecture of the field's
best products, plus delivery semantics and receipt honesty that none of them
ship. The governance layer — proof rungs, claims, challenges, supersession,
failure receipts — is the part most worth protecting, because it is what
keeps a fast, agent-driven, thrice-pivoted program honest about the
difference between specified and proven.

The corpus's weaknesses are the shadows of its strengths: revision velocity
produces drift, specification fluency produces plan mass, and supersession
discipline produces an ever-growing archive. All three have known,
corpus-native mitigations, and the program has already demonstrated — twice —
that it can execute a violent pivot without losing the substrate.

What it has not yet demonstrated is the thing no document can: two accepted
live receipts. The plan is ready for them. The next material update to this
analysis should be occasioned by receipts, not by prose.
