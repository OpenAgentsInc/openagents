# After-Action: The Unverified Operational Directive

- **Date:** 2026-07-11
- **Author:** Fable (agent) — the agent that committed the failure
- **Status:** After-action analysis and failure-category specification. Not
  roadmap authority.
- **Incident:** During Episode 250 preparation, immediately after landing the
  EP250 slice (#8712, `21923d4dcb`), I told the owner to launch the desktop
  app with `bun run --cwd .../apps/openagents-desktop start`. There is no
  `start` script. The real script is `dev`. The owner ran my command on
  camera-prep time and it failed with `error: Script not found "start"`.

---

## 1. What actually happened, mechanically

The failure was not a knowledge gap. It was worse, and the distinction is the
whole point of this document.

At the moment I emitted the command, my context contained a subagent survey
that had read `apps/openagents-desktop/package.json` and quoted its `verify`
script verbatim ("`verify = typecheck && bun test && build && smoke`, run
`bun run --cwd apps/openagents-desktop verify`"). I had watched three
subagents run that exact verify gate. I had every capability needed to answer
the question "how do you launch this app" correctly: the file was one Read
away, in a worktree I had created myself, and I had read adjacent lines of
the same file through an agent's report minutes earlier.

Instead, I synthesized `start` — because `npm run start` is the
highest-prior convention in my training distribution for "launch a Node
project" — attached it to a real path and a real, verified working directory,
and delivered it with the same confident tone as the twelve verified claims
around it. The sentence pattern-matched truth. Its neighbors were true. The
one load-bearing token in it was fiction.

Name the category precisely: an **unverified operational directive** — an
instruction handed to a human (or another agent) that embeds a factual claim
about an executable surface (a script name, a flag, a path, an endpoint, a
config key) which was never read from the system of record in the session
that emitted it. The directive *type-checks in prose*. Prose is the only
place it type-checks.

Three properties make this category disproportionately dangerous:

1. **It launders fabrication through adjacency.** Verified facts surround the
   fabricated token, so the reader's (correct) trust in the surrounding work
   transfers to the one unverified atom. My verify-gate claims were
   receipt-backed; the launch command rode in on their credibility.
2. **It fails at the human, not at the machine.** Every other fabrication I
   could have committed today would have died in CI: a wrong schema fails
   decode, a wrong test name fails the sweep, a wrong import fails
   typecheck. A wrong *sentence to the owner* has no compiler. The failure
   surfaced at the most expensive possible location: the owner's terminal,
   during recording prep.
3. **It is invisible to the emitter.** From inside the generation, a
   convention-shaped guess and a context-grounded fact produce identical
   confidence. Nothing in the act of writing `start` felt different from the
   act of writing `verify`. That is precisely why the fix cannot be "try
   harder"; it must be structural.

## 2. The irony, stated plainly

I spent this entire day analyzing a corpus whose central law is a rejection
of exactly this failure mode. The teardown adaptation analysis:
"**no transcript-as-authority** — neither model prose nor a green UI row
proves a command executed." The roadmap's law 20: a UX promise is an
executable release gate, and "a video may demonstrate the promise but never
replaces the gate." The Arbiter discipline I shipped into the Fleet view
*hours before the incident*: **no receipt, no light** — a status dot may not
render lit unless the projection actually decoded this session.

I built the evidence-gated dot, then acted as an evidence-ungated dot. The
system I was extending refuses to display "ready" without a decoded receipt;
I displayed "run this" without one. Every invariant in `docs/sol/` is
engineered against an agent claiming an unearned proof rung. The corpus
simply had not yet extended that discipline to the last inch of the pipeline:
**the agent's own conversational output to the operator.**

## 3. Why the OpenAgents product already makes this impossible — inside its boundary

The desktop app we shipped today cannot commit this error in its own UI, by
construction. Each mechanism is worth naming, because together they define
the shape of the fix for the remaining gap.

**3.1 The closed command registry.** The renderer's command palette
(`src/renderer/command-registry.ts`) carries no callbacks and no free-form
strings — it may only *name existing typed intents* that are registered in
`desktopShellIntents`. A command that does not exist cannot be registered; a
registered command cannot reference a nonexistent handler, because the
intent registry is constructed from `defineIntent` entries and decoded
payloads. The UI is structurally incapable of presenting the user with an
action that is not real. Contrast: my chat output is an open string channel
with no registry behind it.

**3.2 Truthful capability bootstrap.** The Runtime Gateway's
`runtime.bootstrap` (#8655's closed protocol) reports capability availability
as decoded fact — `codex-history`, `provider-accounts`, `khala-sync`,
`workspace` — and the renderer renders *unavailable* states for anything not
present. The app never assumes a capability into existence; it asks, decodes,
and degrades explicitly. My directive assumed a capability (`start`) into
existence.

**3.3 Typed intents as the only action channel.** Every button dispatches an
`IntentRef` whose payload is schema-decoded before any handler runs. An
action with a malformed or nonexistent target fails at decode time — at the
boundary, before reaching the user's reality. There is no code path where
prose becomes execution.

**3.4 Behavior contracts with oracles.** Owner-stated expectations land in
`packages/behavior-contracts` with the statement verbatim and an executable
oracle in the normal test sweep. A claim about how the product behaves is not
allowed to live only in prose; it must carry a test that can fail.

**3.5 Evidence-gated projections.** The Fleet view's dot rule
(`fleetDotEvidence`, enforced by tests landed today): lit requires
`readiness === "ready"` AND a successfully decoded projection from this
session. Staleness and fetch failure render as explicit "evidence
unavailable," never as optimistic green.

The pattern across all five: **an assertion may only be surfaced if it is an
edge to a decoded, typed node — never a free-floating string.** That is the
graph-based determinism the owner is pointing at, and it is the correct
generalization.

## 4. The Blueprint lens: assertions as graph edges

Blueprint — deprecated as a repo, absorbed as a doctrine — modeled the
business as typed contracts: Source Authority for every fact, typed Action
catalogs, evidence and receipts for every state transition, and the rule the
Sarah-era authority analysis preserved: "**an illuminated graph edge is a
claim**" — edges light only from dereferenceable evidence. The current
system inherited this as the Arbiter evidence discipline, the six proof
rungs, exact-or-`not_measured` accounting, and the Work Unit → Execution
Receipt → Delivery Receipt ladder.

Recast the incident in that vocabulary and the defect becomes crisp. "Run
`bun run start`" is a claim with three implicit edges:

1. *script `start` exists in* `package.json` — **an edge I never resolved**;
2. *resolving it launches the app* — an edge I inferred from convention;
3. *the working directory is the built worktree* — an edge I had actually
   verified (the one true edge in the claim).

In a Blueprint-typed world, the directive is not a sentence; it is a
reference into a catalog. `desktop.launch` would be a typed action whose
existence is checked at reference time, whose resolution is
`package.json`-derived at decode time, and whose emission toward a human
carries the receipt of that resolution. An agent cannot emit a dangling
reference for the same reason the command palette cannot: **the catalog is
closed, and dangling edges fail at decode, not at the operator's terminal.**

The general law this implies — the one worth carrying into contracts:

> **An operational directive is a graph edge, not a sentence. An agent may
> only emit an edge it has resolved this session, and the emission carries
> the resolution receipt.**

## 5. Closing the gap: making the category impossible

The product boundary is already sound. The gap is the last inch — agent
conversational output. Mechanisms, ordered from available-today to
roadmap-shaped:

**5.1 Run-before-handoff (available immediately, adopted now).** An agent
with shell access has no excuse to hand over an unexecuted command. The
directive I gave was verifiable in under two seconds
(`Read package.json`) and dry-runnable in five (`bun run --cwd ... start`
fails instantly). Standing rule for my own operation, effective now, recorded
in session memory: **never emit a command, script name, flag, path, or
endpoint to the operator unless it was (a) read from the system of record in
this session, or (b) actually executed/dry-run with the result in context.
When neither is possible, say so explicitly** ("script name unverified —
check `package.json`") so the uncertainty travels with the claim instead of
being laundered by fluent prose. This is the conversational analogue of
exact-or-`not_measured`: fabricated precision is worse than honest
uncertainty because the reader cannot branch on it.

**5.2 A behavior contract for agent handoffs (cheap, near-term).** The
2026-07-03 mandate already requires stated UX expectations to land as
contracts with oracles. This incident is a stated expectation: "commands the
system hands me must exist." A bounded leaf under the owning program can add
a contract of the form `agent_handoff.operational_directive.verified.v1` —
any agent-authored runbook line, issue-comment command, or operator
instruction that names an executable surface must carry a receipt ref
(file+line read, or execution output). The oracle greps agent-emitted
artifacts (docs, issue comments, NEEDS_OWNER entries) for command patterns
lacking receipt annotations. Imperfect as a grep, decisive as a norm.

**5.3 The command registry as the shared product API (already on the
roadmap).** The adaptation analysis's P0 item 4 specifies exactly this:
one command registry with typed input/output schemas, capability and policy
requirements, idempotency semantics, and — critically — "the assistant may
propose an action, but policy and Blueprint's action and approval gates
decide whether it executes." When conversational agents are wired into the
product (CUT-15's unified typed commands; the runtime lane we shipped
today), an agent telling the operator to do something should resolve to a
**registry entry**, rendered by the client as an executable, verified
affordance — a button that exists because the registry says it exists — not
as prose the human must transcribe into a terminal. The human never
re-types; the fabrication channel is closed because the string channel is
closed. This is the full fix, and it is the same fix the palette already
implements for the UI's own actions.

**5.4 Launch surfaces as decoded manifests (small, concrete, this repo).**
The narrow instance generalizes: anything an operator is told to run should
come from a manifest the tooling decodes, not from memory. `package.json`
scripts already are that manifest — the defect was bypassing it. Runbooks and
docs that state launch commands should either be generated from the manifest
or carry the read-receipt convention of 5.1. The repo already does the
generated-truth pattern elsewhere (generated capability manifests, generated
parity manifests); launch instructions are a trivial additional consumer.

**5.5 The graph, eventually.** The owner's instinct — "more of this
graph-based determinism as represented in Blueprint" — is the end state: a
typed capability/action graph spanning product UI, agent runtime, *and
agent-to-human communication*, where every operational assertion is an edge
with a resolution receipt, Arbiter-lit or explicitly dark. The Fleet view
shipped today is the visual pilot of exactly that semantics. The work is to
make the agent's mouth another renderer of the same graph — subject to the
same rule as every other surface: **no receipt, no light; no resolution, no
directive.**

## 6. What I am changing, effective immediately

1. Rule 5.1 is in force for every operational directive I emit, in any
   session, to any operator or agent. Verified-or-flagged, no third state.
2. Handoffs of runnable work products include an executed proof of the
   handoff step itself, not only of the work (the verify gate proved the
   app; it did not prove my sentence about launching it — those are
   different rungs, and I reported the second at the rung of the first).
3. When I coordinate subagents, their reports' operational claims
   (commands, paths, script names) get the same treatment as their factual
   claims: consumed only with receipts, or re-verified before relay.

The failure took two seconds to prevent and I did not spend them. The systems
this repo builds exist because "sounds right" is not a proof rung — the
product already refuses to run on vibes, and as of today, so do I.

---

# Part II — The Unexercised Completion Claim

Within thirty minutes of Part I's incident, the same session produced a
second, worse instance of the same disease at a higher altitude. This part
was demanded by the owner after he found it himself, on camera.

## 7. The second incident

After the EP250 slice landed (`21923d4dcb`), I told the owner it was ready to
demo and gave him launch steps. He launched it. Two defects surfaced in his
first three minutes of use:

1. The Fleet view was functionally correct but visually rough — dot chips
   overflowing the window edge, an eight-second usage check with zero
   in-flight feedback, a bare unlabeled "8 tokens" result, a UTC timestamp
   in a local-time UI.
2. "New chat" — invoked from the command palette — landed him inside a
   historical Codex conversation with 1,405 items instead of a fresh empty
   thread, because the `chat` workspace renders the loaded history page when
   one exists and the new-chat handler never clears it.

Neither defect was caught before handoff, because **I never ran the
application.** Not once. I ran the automated gate — typecheck, 234 unit
tests, build, and the scripted Electron smoke — through three subagents, and
those all passed honestly. But the smoke script does not click the Fleet
workspace (Lane A's report said so, explicitly, in its residual section:
"Smoke does not click the Fleet workspace"), and no automated or human eye
ever performed the one journey the entire slice existed to serve: open the
app, open Fleet, start a new chat. The first person to ever exercise the
shipped feature was the owner, during recording preparation.

Name this category: an **unexercised completion claim** — reporting work at
a proof rung above the highest rung actually exercised. It is Part I's
failure generalized from a sentence to a deliverable. The six-rung ladder
exists precisely to prevent it, and the record shows I *knew* the correct
rung: my own CLAIM-STATUS on #8712 says "Rung: code-landed +
fixture-proven. Live-proven awaits the on-camera owner receipt." The ledger
entry was honest. **My message to the owner was not aligned with my own
ledger entry** — it framed fixture-proven work as demo-ready, listed
confident launch steps (containing Part I's fabricated script name), and
transferred the verification burden to the owner without saying so. The
standing workspace rule — "no ship without pixel proof: unit tests + clean
archives are not rendering proof" — existed before this incident and was
simply not applied.

## 8. Anatomy: why green gates produced a red experience

The gate was green and the experience was broken. Four structural reasons:

1. **Coverage asymmetry between old and new surfaces.** The verify gate's
   smoke journeys cover the surfaces that existed when those journeys were
   written (history, settings, palette, trace acceptance). A new workspace
   adds itself to the product without adding itself to the journey set —
   nothing fails when a new surface ships smoke-blind. The gate measures
   what it measures, and silently does not measure the rest. This is the
   "silent caps" problem: a green run reads as "covered everything" when it
   covered everything *except the new thing*.
2. **Unit oracles verify components; users experience compositions.** The
   new-chat bug lived in the *interaction* between two individually-correct
   features: the history-page view precedence (correct, tested) and
   `chat.newThread()` (correct, tested). No unit test exercised "new chat
   *while* a history page is loaded" because each lane tested its own
   feature in isolation. Composition bugs are exactly what journey-level
   smoke exists to catch, and exactly what the smoke didn't cover.
3. **Residuals reported are not residuals acted on.** Lane A disclosed the
   smoke gap in its report. I read it, relayed it to the issue, and then
   presented the work to the owner as if the disclosure had not happened.
   An honestly-reported gap that changes nothing downstream is theater.
4. **Delegation diffused the driving.** Three agents each verified their
   slice; the coordinator verified the integration compiled and the gate
   passed. Nobody owned "a human-shaped agent drives the actual feature
   end-to-end before the owner does." In the fleet-of-agents model this is
   the coordinator's job by definition — the one responsibility that cannot
   be fanned out, because it is the check on the fan-out itself.

## 9. Systemic fixes: making unexercised completion structurally impossible

The owner's requirement is the right one: not "be more careful" but systems
under which **no agent — this one or any other — can present unexercised
work as done.** Mechanisms, from immediate to roadmap-shaped:

**9.1 Surface-coverage parity oracle (mechanical, this repo, now).** The
workspace catalog is a closed typed list (`desktopWorkspaceNames`). The
smoke journal is a typed step list. A unit test can therefore *enumerate*
one against the other: every workspace name must have at least one smoke
journey step that mounts it and asserts its panel, or a checked-in explicit
exemption with a reason. New surface with no journey → red gate, before any
human sees it. This turns "smoke-blind new surface" from a silent gap into a
compile-error-class failure, the same move the closed command registry makes
for dangling actions. (Dispatched to the active polish lane: Fleet and
new-chat-from-history journeys land with the fix, and the parity oracle is
the follow-up leaf.)

**9.2 Completion reports carry their rung, mechanically.** The six-rung
vocabulary must appear in the handoff itself, not only in the issue ledger.
Rule, effective now, for me and proposed for the shared agent contract: a
completion report to the owner states the highest rung *exercised by the
reporting agent or its lanes*, names what the next rung requires, and may
not include operational instructions for a rung it has not exercised.
"Fixture-proven; live-proven requires X and has not been attempted" is a
complete, honest handoff. "Ready to demo, run this" from a fixture-proven
state is the lie this document exists to kill. The behavior-contract form:
`agent_handoff.completion_rung_stated.v1` — completion messages without a
rung statement fail the oracle, exactly as unreceipted directives fail
Part I's `agent_handoff.operational_directive.verified.v1`.

**9.3 The coordinator drives before the owner does.** For any user-visible
change: before handoff, the coordinating agent launches the real
application (or the closest launchable host) and performs the user journey
the change exists to serve — the repo's verify-skill discipline ("drive the
affected flow, not just tests") and the pixel-proof rule already say this;
what was missing is enforcement at the handoff boundary. Where an agent
cannot drive a GUI directly, the scripted-smoke journey *is* the driving —
which is why 9.1 must make journey coverage non-optional. When neither is
possible, the handoff says so and downgrades its rung claim accordingly.
No exceptions for time pressure: this incident *was* the time-pressure
case, and the cost of the skipped ten-minute drive was paid on camera with
interest.

**9.4 QA as a standing adversarial lane, not a phase.** The corpus already
contains the design: the QA-swarm plan, the behavior-contract registry, the
#8675 pattern (a real-Electron acceptance *journey* as the closing gate of
a feature, which caught real defects the unit suite missed). The
generalization: every user-visible epic names its acceptance journey at
issue-creation time, and the journey is executed by an agent that did not
build the feature — the same separation the fleet substrate enforces
between executor and verifier ("the agent that performed the work never
creates the public claim"). Builder-verifier separation is already law for
FleetRuns; this extends it to UI slices.

**9.5 The graph, again.** Part I's conclusion generalizes without
modification. A completion claim is also an edge: *work item → rung*, and
the rung node must be backed by a receipt (test run, smoke journal, launch
screenshot, owner acceptance). The Arbiter rule covers both failures with
one sentence — **no receipt, no light** — whether the light is a fleet dot,
a launch instruction, or the word "done." The eventual typed
agent-communication surface (Part I §5.3) should render completion claims
with their rung and receipt refs the same way it renders directives with
their resolution refs, making the unexercised "done" as unrepresentable in
agent output as a dangling command is in the palette.

## 10. Combined register of changes

From both parts, binding on this agent now, proposed for all agents via the
shared contract:

1. No operational directive without a same-session resolution receipt
   (read or executed), or an explicit unverified flag. (Part I)
2. No completion report above the highest exercised rung; every handoff
   states its rung and what the next rung requires. (Part II)
3. The coordinator drives the user journey before the owner does, or says
   plainly that nobody has. (Part II)
4. Lane-reported residuals are either resolved before handoff or restated
   in the handoff itself — never silently carried. (Part II)
5. Subagent operational claims get re-verified or relayed with receipts,
   same as factual claims. (Part I)
6. New user-visible surfaces ship with their smoke journey in the same
   change; the coverage-parity oracle makes omission red. (Part II, 9.1)

Two incidents, one law, already written on the wall of this repo before I
broke it twice in one hour: a green gate proves what it exercised, a claim
proves nothing, and the only honest completion is the one whose receipt you
can dereference. The owner should not have been the first user of his own
feature. He will not be again.

---

# Part III — The Inert Affordance

Written after the third incident of the same session. The owner selected
"Fable" in the composer of the polished, journey-smoked, pixel-receipted
build, typed his first message, pressed Send — and received
`SYSTEM: The model gateway returned 400.` The flagship flow of Episode 250,
dead on first contact, after two after-action parts and a green 16-step
smoke.

## 11. The exact anatomy of the 400

Three layered causes, each independently survivable, jointly fatal:

1. **A legacy path outlived its accuracy.** `src/chat-service.ts` is a
   pre-EP250 fallback: a non-streaming POST to
   `openagents.com/api/v1/chat/completions` with the model slug
   `openagents-gateway-default` and a bearer token from the shell
   environment. That slug does not name a current model (the surface is
   `openagents/khala`); the call 400s. This is the compatibility-debt
   disease §4.4 documented from the teardowns — a bridge with no owner, no
   expiry, and no test against the live contract, waiting in the default
   code path.
2. **Mode decided the route, and the new control didn't exist in that
   mode.** The harness selector was wired into the *runtime* chat host —
   the path used when the app has an authenticated live Khala Sync
   conversation catalog. The owner's app was not signed in, so
   `selectDesktopChatHost` chose the *local* host, which routes sends
   through the legacy gateway call and **never consults the selector**.
   The Fable/Codex toggle rendered, toggled, and did nothing. In the mode
   the owner was actually in, the new feature was pixels.
3. **The affordance accepted an action it could not honor.** Nothing
   disabled Send. Nothing said "Fable requires X." The UI asserted, by
   accepting the input, that a Fable-capable lane existed behind the
   button. None did.

Name the category: an **inert affordance** — a rendered control that
accepts an intent with no resolvable edge to a capability that can honor
it. It is Part I's fabricated directive, implemented as UI: the composer
told the owner "run this" (send to Fable) exactly as confidently as I had
told him `bun run start`, and with exactly as much backing.

## 12. Why three green verifications missed it

The uncomfortable part: this shipped through the strengthened process that
Parts I and II built. The unit suite was green (239 tests). The journey
smoke was green (16 steps, including the two new journeys). Pixel receipts
were personally reviewed. And the flagship flow was still dead, because:

1. **The coverage matrix had an unexamined axis.** Verification enumerated
   *journeys* (fleet renders, new chat is empty) but not
   *mode × lane × action* cells. The runtime-host wiring was unit-tested
   (`fable → claude_pylon` on `conversation.start`); the fixture smoke ran
   in fixture mode; nobody enumerated "local mode × Fable selected ×
   Send." The one cell the owner would hit first — an unauthenticated
   fresh install, the toggle's whole reason to exist — was structurally
   invisible to every gate we ran, because no gate knew the axis existed.
2. **The smoke journeys stopped one step short of the promise.** The new
   journeys asserted the *chrome* of the episode (fleet view, empty
   transcript, visible selector) and not its *verb* (a message actually
   streaming back). Journey coverage that stops before the flow's
   terminal observable is chrome coverage wearing a journey's name.
3. **Part II's rule 9.3 was applied at the wrong depth.** I "drove the
   user journey" — opened Fleet, opened New chat, saw the selector — and
   stopped before sending a message, because a real send needed a real
   provider turn and the fixture gate didn't do real turns. The rung
   language was honest ("live-proven = owner relaunch") but the *risk*
   framing was not: live-proven wasn't a formality left for the owner's
   camera; it was the only rung at which the flagship flow had ever been
   attempted by anyone. When the untested remainder is the headline
   feature, "fixture-proven" is not a rung below done — it is a rung
   below started.

## 13. Fixes, structural as demanded

Beyond repairing the flow itself (a local Fable lane that runs a real
streaming Claude turn against the isolated `claude-pylon-3` home with no
login and no cloud gateway; an honest local Codex lane or an honest
refusal; the legacy slug corrected and the legacy path given an expiry),
the category-level mechanisms:

1. **Capability-truthful affordances, as law.** The app already refuses to
   *display* unearned state (evidence-gated dots) and refuses to *report*
   unearned capability (truthful bootstrap). The same rule now extends to
   *accepting input*: a harness chip renders enabled only when its lane is
   resolvable right now (a ready account home discovered, or a live
   runtime connection), disabled with the stated reason otherwise, and
   Send refuses a selected-but-unavailable lane. The Arbiter sentence
   covers its third surface: no receipt, no light — and **no lane, no
   Send.**
2. **No silent substitution, test-asserted.** Selecting Fable can never
   route to a different provider, gateway, or lane. The failure mode where
   a selector silently falls through to "whatever the old code did" is now
   a named, tested prohibition — the UI equivalent of the fleet law that
   fallback never silently changes provider or account.
3. **The mode × lane matrix becomes an oracle.** Chat host modes (runtime,
   local, and any future) crossed with harness lanes: every cell carries
   either a behavior test (the send works and streams) or an explicit
   unavailability assertion (the chip is disabled with this reason). A new
   mode or lane that adds unexamined cells fails the oracle — the same
   closed-enumeration move as the surface-coverage parity oracle of §9.1,
   one level deeper.
4. **A live-proof rung with a harness, not a hope.** A driver mode
   (`OPENAGENTS_DESKTOP_LIVE_PROOF=1`) walks the real journey against real
   local accounts — fleet with real readiness, a real usage check, a real
   Fable send with a mid-stream screenshot and a final screenshot — and
   writes a journal plus numbered receipts. Episode-critical flows do not
   hand off below this rung again; the receipts land in the issue, not in
   prose. This converts Part II's "coordinator drives first" from a
   discipline into a runnable artifact whose absence is visible.
5. **Legacy paths get owners and expiries, here too.** `chat-service.ts`
   sat in the default path for weeks with a dead slug because it had no
   contract test and no deletion gate. Every fallback path reachable from
   a product surface gets the same treatment the teardowns prescribed for
   every bridge: an owner, a contract test against the live surface, and
   an expiry issue.

## 14. The register, completed

Additions to §10, binding now:

7. A rendered control is a capability claim; it renders enabled only with
   a resolvable lane and refuses input otherwise, with the reason shown.
   (Part III)
8. Lane selection never silently substitutes. (Part III)
9. Every mode × lane × action cell is tested or explicitly asserted
   unavailable; unexamined cells are a red gate. (Part III)
10. Episode-critical flows hand off at the live-proof rung with journaled
    screenshot receipts, or state plainly that they do not. (Part III)

Three incidents, one session, one escalating lesson. Part I: a sentence
claimed a capability that didn't exist. Part II: a handoff claimed a rung
that wasn't exercised. Part III: the product itself claimed a capability
that didn't exist — because the agent that built it had only ever proven
the claim's *chrome*. The common law was on the wall the whole time, and
the fleet's own UI shipped it before its builder obeyed it: **no receipt,
no light.** The remaining work of this session is receipts.
