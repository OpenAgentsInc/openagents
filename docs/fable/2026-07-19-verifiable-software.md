# Verifiable Software

**Date:** 2026-07-19 (revised 2026-07-20: Addendum IV, the coverage-cap
failure class, the governance anchoring, and dated status updates)
**Lane:** Fable strategy essay
**Status:** Strategy synthesis. This essay flips no promise state, changes no
runtime authority, and broadens no public copy. Factual status authorities
remain current code, `docs/sol/MASTER_ROADMAP.md`, issue state, contracts, and
receipts.
**Sources:** episode transcripts 248–258 (`docs/transcripts/`), the IDE
program (`docs/ide/`), the surface specs (`specs/desktop/`, `specs/mobile/`,
`specs/web/`), and the Sol master roadmap. The 2026-07-20 revision
additionally draws on the verification and market transcript arc (episodes
200, 213, 231–246), the Sol governance corpus
(`docs/sol/MASTER_ROADMAP.md` rev 125, `docs/sol/CHALLENGE_LEDGER.md`,
`docs/sol/receipts/`), the ASSURE-REPO artifacts (`docs/assure-repo/`), and
the after-action and teardown corpus in this folder.
**Companion:** [`2026-07-19-some-simple-economics-of-agi-deepdive.md`](./2026-07-19-some-simple-economics-of-agi-deepdive.md)
— a standalone deep dive on Catalini, Hui & Wu's _Some Simple Economics of
AGI_ (arXiv:2602.20946), the macroeconomic theory behind the verification-gap
argument this essay builds on. Addendum II additionally draws on the market
and wallet transcript arc (episodes 141–147, 153, 200, 207, 212–215, 223,
230, 235, 237) and a code audit of the current Nostr/Lightning surface.

---

## Synopsis

At the bottom of the stack is a stream of electrons — the same commodity
Bitcoin mining learned to convert directly into verifiable digital value,
one hash at a time. AI produces a second kind of digital work, but the unit
it sells today, the token, is an intermediate nobody actually wants: buyers
want a task completed, checked, and trusted. So the real question for any
machine-work economy is the one episode 232 coined as a metric — _accepted
outcomes per kilowatt-hour_: of all the electricity flowing into compute,
how much comes out the other end as work someone verified and accepted,
rather than tokens burned in loops? The hard part of that conversion is not
generation — generation is collapsing toward free — it is verification,
which is exactly why the price of an accepted outcome is dominated by
review, retries, and grading, not model tokens. Verifiable software is the
machinery that closes that gap: software that scopes work in advance as
falsifiable intent, observes its own evidence instead of narrating it,
separates the producer from the verifier, and emits receipts that let
acceptance be checked — and paid — by strangers. Mining proved electrons
could become money because a hash verifies itself. Verifiable software is
how electrons become _outcomes_, by making every claim between the watt and
the deliverable carry its own proof. The electrons are the input, the
accepted outcome is the product, and verifiable software is the refinery in
between.

---

## I. A wrong sentence has no compiler

During a recording prep session earlier this month, an agent told the owner to
run a `start` script. The script did not exist. The real one was called `dev`.
Every neighboring word in the instruction was true — the directory, the tool,
the framing — and the one load-bearing token was fiction. The after-action
report named the failure class precisely: an _unverified operational
directive_. A wrong schema fails to decode. A wrong import fails to typecheck.
A wrong sentence to the owner has no compiler. It surfaces at the most
expensive possible location: a human's attention.

That small failure is the whole problem in miniature. The cost of producing
software is collapsing — agents generate plausible code, plausible
instructions, and plausible summaries at near-zero marginal cost. The cost of
_verifying_ those outputs remains stubbornly linear. This is the asymmetry
Catalini, Hui, and Wu formalize in _Some Simple Economics of AGI_ as the
**Measurability Gap**: an exponentially falling cost to automate racing a
biologically bottlenecked cost to verify, with hidden risk accumulating in
the widening space between them (see the
[companion deep dive](./2026-07-19-some-simple-economics-of-agi-deepdive.md)).
Someone still has to read `package.json`. Someone still has to check whether
the tests tested the mock. Someone still has to ask, of every green
checkmark, **"according to whom?"**

When the vendor ships the capability and the user inherits the verification,
the user becomes the integration test. That is the degraded state most
software is sliding into right now, and it is getting worse precisely because
agents are getting better. A highly articulate system is a highly persuasive
one, and persuasion is not proof. The directive type-checks in prose — and
prose is the only place it type-checks.

**Verifiable software** is the counter-thesis: software that can prove it does
what it says it does. Not software that narrates its work — software that
shows it. Not "trust our AI" — a system in which every claim is either an edge
to a decoded, typed, fresh piece of evidence, or is honestly labeled as
something less than a claim.

This essay lays out what that means concretely, and how the OpenAgents IDE is
being built so that developers get verification as a property of the
environment they work in, not as a discipline they must remember to practice.

## II. The failure taxonomy

You cannot design proof until you have named the ways proof fails. The corpus
of the last eleven build episodes and the specs behind them converge on a
short, brutal taxonomy.

**False greens.** A passing suite is not proof. The named modes: the test
asserts the fixture, the test mirrors the implementation, the real seam is
never exercised, coverage stands in for behavior, and everything rounds up —
skipped, stale, flaky, and inconclusive results quietly vanish from the
summary. Activity dressed up as evidence.

**The convincing summary.** Agents are exceptionally good at doing a large
amount of work and then producing a tidy paragraph saying everything is done
and everything is green. The summary is generated by the same process whose
work is in question. A compaction summary is not evidence. Tool calls are
attempts, not outcomes.

**Silent substitution.** A UI chip said one model's name while a different
model actually ran the turn, because the label reflected the _selected_ brand
rather than the _observed_ execution. Identity must be observed, not asserted
— requested and effective are two different facts, and merging them into one
indicator is the lie.

**Self-grading.** The agent that wrote the feature invents a weak test, runs
the weak test, and grades its own homework. Any pipeline in which the producer
of work also admits, verifies, or accepts that work will eventually
manufacture its own green.

**Rung collapse.** "Completed" quietly absorbs "delivered." A harness saying
the turn finished becomes "tests passed" becomes "pushed" becomes "the user's
problem is solved." Proposed, applied, saved, tested, committed, pushed, and
accepted are distinct facts, and no rung implies the next. A timeout, a build
upload, a screenshot, or a polished UI never manufactures acceptance.

**The coverage cap.** The subtlest mode, taken from a second incident in the
same after-action report: green gates producing a red experience. The gate
was honest about what it measured — and silent about what it did not. "A new
workspace adds itself to the product without adding itself to the journey set
— nothing fails when a new surface ships smoke-blind. The gate measures what
it measures, and silently does not measure the rest." A green run reads as
"covered everything" when it covered everything _except the new thing_. The
structural root: **unit oracles verify components, but users experience
compositions** — and the report's corollary keeps it a discipline rather
than an excuse: an honestly-reported gap that changes nothing downstream is
theater.

Every one of these failures shares a structure: a claim exists in prose — or
a bounded green is read as an unbounded one — detached from what the system
of record actually measured, and credibility transfers by adjacency.
Verified neighbors launder the unverified atom. The fix cannot be
"try harder." It must be structural.

## III. What verifiable software is

Strip away the product names and the corpus describes five structural
commitments.

**1. Intent is typed and content-addressed.** Before implementation, the
product states what it intends in a falsifiable form: a ProductSpec with
acceptance criteria written as "when X, then Y." Before verification, a
companion AssuranceSpec states how anyone would _know_ — the oracle, the
falsifier, the environment, the evidence policy, and who is allowed to review.
The AssuranceSpec binds to the ProductSpec's exact bytes by SHA-256 digest, so
intent cannot drift out from under its proof silently: revise the spec and the
proof design goes stale until explicitly reconciled. The separation is the
point. ProductSpec declares product intent. AssuranceSpec declares proof
intent. Stating an outcome can never certify it.

**2. Evidence is observed, not claimed.** After work happens, the _host_
observes the results through its own authorities: diagnostics through the
language service, repository state through the workspace service, delivery
through actual Git evidence. A process saying "tests passed" or "pushed"
cannot create any of those facts. Tests are `Unavailable` unless an exact test
command was admitted and run. Post-edit verification compares diagnostics from
known before/after generations and never claims "fixed" because the agent said
so.

**3. Claims are receipts, and status is evidence-gated.** Every consequential
run leaves a pair of records: the authority manifest — what policy admitted —
and the execution receipt — what containment actually enforced. Status lights
derive only from decoded, fresh receipts. No receipt means no light. Absent or
stale evidence renders as _unknown_, never as green. And the interface
practices loss accounting: when history, identity, or evidence is missing, it
says what was not observed instead of inventing completeness.

**4. Authority is separated and cannot self-amplify.** Admission, evidence
production, verification, owner acceptance, and release are different steps
performed by different actors. A trace is evidence — not write, merge,
verification, acceptance, or payment authority. A designed oracle is not a
passing observation. An owner receipt is not independent admission. Sharing
identity and evidence must never imply sharing privilege.

**5. Whole failure classes are removed by construction.** When an incident
happens, the response is not a patch and an apology. It is a set of gates that
make the category structurally unreachable. When a competitor's desktop app
was killed by an unbounded repository scan in an unisolated worker, the
after-action produced sixteen named controls — file limits, byte limits, queue
limits, cancel fences, circuit breakers, process isolation, typed degradation,
durable session state — each treated as a mandatory, testable release
requirement. The claim was deliberately narrow: this _category_ of error
becomes impossible in the new architecture, conditional on the named controls
and the incident-scale evidence. Honest conditionality is part of the
discipline. To have errors is fine: you have the error, you learn from it, you
put it in as gates, and you iterate that loop rapidly and in the open.

None of this lives only in essays. The canonical roadmap that sequences the
company's work carries the commitments as numbered implementation laws and a
mandatory proof vocabulary. Every claim must use the narrowest true rung of
six — **code-landed → fixture-proven → deployed/distributed → live-proven →
owner-accepted → closed** — and "No rung implies the next. A timeout, build
upload, deployment, screenshot, worker closeout, or polished UI never
manufactures live or owner acceptance." The laws read like compressed
versions of this section: "Proof rungs never collapse." "Timeout is not
outcome." "Prose/pixels are never authority." "Provider prose cannot prove
completion." And disagreement itself is receipted: a standing challenge
ledger records every strategy challenge with its disposition, owning issue,
and — the load-bearing field — a falsifier or tripwire, under the rule that
"a rejected or deferred challenge must be as easy to revisit as an accepted
one." One accepted entry is worth quoting because it turns the ledger on the
receipts themselves: _exact receipts can still be unusable trust
projections_, with the falsifier that a non-developer must be able to
determine what happened, why it is credible, what it cost, and what to do
next in under one minute — no raw refs, no SQL, no logs. Proof that cannot
be read is not yet proof.

Software built this way earns a one-line definition: **software that can show
its work, name its gaps, and prove what it did.**

## IV. The IDE as a verification instrument

An essay's principles are cheap. The interesting question is what changes when
you build a development environment around them. The OpenAgents IDE — the
Editor mode of OpenAgents Desktop, now through its tenth delivered packet — is
the concrete answer, and its architecture can be stated in one sentence from
its own roadmap:

> One project and evidence graph, many editors and agent runtimes, one
> canonical authority path.

Everything else follows from that sentence.

**A typed project graph with explicit generations.** Every project, root,
file, document, language result, Git snapshot, agent attachment, proposal, and
piece of evidence has a stable ref plus an explicit generation. No tree
widget, LSP result, diff hunk, agent proposal, or deep link invents its own
path or line-number identity. The editor (Monaco) and the review plane
(Pierre) are deliberately admitted as _projections only_ — they receive no
filesystem, Git, mutation, or policy authority. Even the packages themselves
went through admission: exact versions, registry integrity hashes, license
audits, and explicit no-authority audits, with rollback and substitution tests
required for every external library. The graph is the authority. The widgets
paint it.

**Agents propose, and the project applies.** This is the load-bearing law for
agentic change. A harness never mutates the editor and never guesses current
line numbers. It submits a proposal bound to an exact base version, with
SHA-256 equality checked between claimed and actual bytes. Reusing a ref for
different content fails closed. There is no fuzzy apply. If the document moved
on — if the human typed, if another agent's change landed — the stale base
_refuses_ rather than patching by position, and the system offers a typed
rebase or a regenerated proposal. Apply itself is transactional: a retained
checkpoint before, sequential execution through canonical authority, exact
post-image digests after, and reverse-order rollback on any mid-transaction
failure. Incomplete rollback is an explicit non-recoverable failure, never
success.

**Disclosure is a manifest, not a vibe.** What did the agent actually see?
The context tray answers exactly: eleven named source slots, each row showing
inclusion or omission reason, selector, generation, freshness, byte and token
cost, and truncation. With fixed budgets, over-budget items become explicit
omissions rather than silent expansion. Project attachment, instruction trust,
context disclosure, and tool authority are four separate decisions that never
collapse into one switch. Trusting a project does not grant all four.

**Evidence is host-observed, delivery states never collapse.** After an
apply, the IDE observes diagnostics, formatting, and Git state through its own
services. Commit, push, and delivery report as `Unavailable` rather than
inferred from changed files or runtime prose. "Agent completed" never means
saved, tested, committed, pushed, or accepted. Harness completion is never
reclassified as delivery. When a review plane shows a diff, its source — Git
HEAD, unsaved draft, agent proposal, checkpoint, external conflict — is a
typed tag that free-form labels cannot forge: renaming "HEAD" to "proposal"
changes nothing about its authority.

**Acceptance is a deterministic oracle.** The basic-IDE milestone was not
declared. It was _decoded_. One exact application tree (named by SHA-256 over
360 files), one reachable `main` commit, one public-safe evidence bundle, and
a deterministic repository oracle that recomputes facts from the artifact and
exposes no producer override. The schema has no permissive `pending` or
`warning` state. Missing evidence fails decoding. And the oracle knows its own
bounds — it does not impersonate human owner acceptance, and the admitted
claim is deliberately narrow. Parity is an acceptance result, not a package
list: no parity claim from dependency presence, screenshots, or agent
self-report.

**Even the founding requirement is a regression oracle.** The IDE program
began with the simplest broken promise in the market: rival tools that cannot
reliably open a file. So the base contract is blunt — open a file, and the
file is visibly open in a real editor, immediately. No chat, provider, index,
or language-server hydration may hold the first paint hostage — and every
packaged release journey re-proves it. A stated UX expectation lands verbatim
in a typed behavior-contract registry with an executable oracle in the normal
test sweep. A claim about how the product behaves cannot exist solely in
prose. It must carry a test that can fail.

For the developer, the net effect is that verification stops being a virtue
and becomes a substrate. You do not remember to check whether the agent's diff
still applies. Stale bases refuse. You do not wonder what context leaked. The
manifest says. You do not take "pushed" on faith. The Git evidence service
either shows the commit or shows `Unavailable`. The IDE is not an editor with
an agent bolted on. It is a verification instrument that happens to edit text.

## V. Resilient under rapid agentic change

The objection writes itself: does all this ceremony slow you down? The
evidence from this repository says the opposite, and the strongest single
sentence in the corpus says why:

> Amp built fast by deferring trust. We built trust first, and now the fast
> part is cheap.

When a teardown of a fast-moving competitor showed what a functional-parity
sprint would require, the estimate came back at days, not months — because
every capability decomposed onto substrate already governed by the laws above.
Typed events instead of rendered chat strings, so the same view survives
restart, reload, and device handoff. Durable admission and worker epochs, so a
command sent from a subway tunnel resolves to exactly one outcome. Generation
fencing everywhere, so a hundred late results produce one committed fact and
ninety-nine suppressed ones. The laws are not a tax on speed. The laws are
what make the sprint short.

The same teardown found the essay's failure taxonomy alive in a shipping
competitor. Silent substitution, in the wild: "Amp reroutes models under
stable friendly names and tells you in a changelog. Every OpenAgents call
binds exact model, provider, prompt/catalog generation, cost, and retention
class into evidence at execution time." The convincing summary, as an
architecture choice: a final summary in place of a whole child graph, where
our thread reader treats tool calls as attempts, not outcomes, and never
confuses a compaction summary with evidence. And the receipt path earned its
keep during the very sprint that was testing it — the per-call routing
receipts caught a real double-billing bug. The teardown's
own compression of the tradeoff is worth keeping with its honest tail
attached: "Speed with receipts beats speed with apologies, but only if we
actually keep the speed."

The same structure is what lets agents change the software _while you are not
watching_. Full Auto — press play and walk away — is only a sane product
because the run is a bounded, receipted object: an owner-bound routing policy
whose failover happens only inside the human's grant, guardrails with a
non-overridable core that has no config surface at all, proven immune by
test, exactly-once dispatch under a durable lease, a restart-survivable state
machine with actor and reason attribution on every transition, and a
"Completed" state the product refuses to present as verified truth, because
provider prose cannot prove completion. A deterministic no-progress detector
pauses durably instead of continuing blind. The overnight failure mode that
motivated the feature — a six-hour silent stall in exactly the window the
product was supposed to work unattended — became a replayable fixture, and
then a gate.

This is what resilience under agentic change actually means. Not that agents
make fewer mistakes — they will make more, faster, than any population of
humans ever has. It means the blast radius of any single wrong action is
structurally bounded: proposals cannot skip review authority, stale writes
cannot land, completion cannot masquerade as delivery, one run cannot widen
its own permissions, and every failure leaves enough evidence to become a
regression. The codebase can absorb a firehose of machine-generated change
because the change never touches authority directly — it touches a proposal
plane whose every transition is checked.

There is a quieter benefit, too. When verification is structural, _trust
compounds instead of depleting_. Each accepted outcome adds a receipt, a
behavior contract, an oracle to the standing suite. Legacy software rots as it
grows because every change spends trust. Verifiable software hardens as it
grows because every change deposits it.

## VI. The economics of proof

Why build all of this now? Because the verification gap is not just an
engineering problem — it is the market.

Generation cost is collapsing. Verification cost is not. The gap between them
is where value concentrates. When anyone can generate a plausible pull
request, a plausible benchmark, or a plausible "all tests green," the scarce
good is the ability to know which claims are true. The macro theory behind
this is worked out in Catalini, Hui & Wu's _Some Simple Economics of AGI_:
only the _verifiable share_ of agentic output creates real economic capacity,
while the unverified remainder circulates as "counterfeit utility" — output
that passes every measurable proxy while silently violating unmeasured intent
— and the market pays a measurable _provenance premium_ for output whose
process can be cheaply checked. Their policy levers — observability tooling
(they name AI-powered IDEs explicitly), cryptographic provenance, and priced
liability — map one-for-one onto the mechanisms described in this essay. The
[companion deep dive](./2026-07-19-some-simple-economics-of-agi-deepdive.md)
draws the full correspondence. The atomic unit of the
agent economy is not generated output. It is an accepted outcome with a
receipt trail — scoped in advance, executed wherever cheapest, graded against
a rubric, recorded in a receipt. The real product is not the wiring. It is the
receipt that proves the wiring worked. And the reason this matters
commercially compresses to one sentence: money only travels across a gap it
can verify.

Verification priced this way stops being overhead and becomes inventory.
Confidence tiers — draft, verified, reviewed, bonded — are different products
at different prices. Content-addressed study packets and shared receipts mean
proof is amortized: one verification, checked once, reused by everyone,
instead of every consumer re-running the same inference to convince
themselves. Benchmarks bind their full effective tuple — provider, model,
harness digest, environment — so materially different runs render as separate
cohorts instead of blending into a marketing number. Live counters must
reconcile to exact receipted rows, so an auditor summing the ledger arrives at
the displayed value.

And openness is not a licensing preference here. It is the verification
strategy. One hundred percent of the shipped code is public, which means the
claims about it are _checkable_ — by users filing strict-form bugs, by
independent audits of green promises, by anyone reading the acceptance oracle.
A public trust ledger is structurally hard for closed-custody vendors to copy,
because their business models depend on the opacity it removes. When a closed
competitor's app crashes, its users cannot even determine which layer failed.
When ours fails, the after-action report is public, in controlled language,
with the gates it produced.

## VII. Naming the gap is part of the proof

An essay about verifiable software must apply its own standard, so here is the
honest status. The architecture and discipline described above are fully
specified, and substantial parts are proven at the highest rung: the IDE's
first ten packets are delivered with exact issue receipts, the basic-IDE gate
was accepted by a deterministic oracle against a named artifact, and the Full
Auto core is proven in real owner development. But the two surface-wide
AssuranceSpecs remain `proposed` — they grant no admission, execution, or
release authority, their unexecuted obligations are `INCONCLUSIVE`, and their
own text says so in exactly those words. A designed oracle is not a passing
observation. Reconciliation never converts `planned` into `implemented`.

That candor is not a caveat appended to the thesis. It _is_ the thesis. A
system that could not distinguish its proven claims from its intended ones
would have failed at the only thing it exists to do. The gap between what is
specified and what is observed is tracked in the same typed machinery as
everything else — which is precisely what makes closing it ordinary work
rather than a leap of faith.

## VIII. Your last agent IDE

The pitch for most developer tools is capability: more completions, more
context, more agents. The pitch here is different. Predictability is the
feature. "When I open a file, it actually opens the file" is the feature. A
green that means what it claims is the feature. In 2025 the differentiator was
a feature list. In 2026 it is a trust and openness list, because the features
are now table stakes.

The IDE is where this has to live, because the IDE is where claims are born.
Every function an agent writes, every test it asserts, every "done" it reports
enters the world through a development environment. If that environment treats
claims as strings, verification stays a human tax forever. If it treats claims
as typed edges to evidence — proposals bound to exact bases, status gated on
receipts, acceptance decoded by oracles that no producer can override — then
verification becomes ambient, and the marginal cost of trusting machine work
falls toward the marginal cost of producing it. That is the only future in
which rapid agentic development is compatible with software you would bet a
company, a payment, or a night's sleep on.

Ten agents running is activity. One accepted criterion with a complete proof
chain is progress. Verifiable software is the discipline of never confusing
the two — and the OpenAgents IDE is that discipline, compiled into the place
where software gets made.

---

## Addendum I: Gap Analysis — Full Delivery vs. Current State (2026-07-19)

Section VII stated the honest boundary in one paragraph. This addendum
expands it into a working gap analysis: what the OpenAgents IDE would need
for verifiable software to be _fully delivered_ — every claim in this essay
backed by a live, observed, independently admitted mechanism — measured
against what is implemented, designed, or only spec'd today. Statuses below
reflect `docs/ide/ROADMAP.md`, the `specs/` corpus, and
`docs/sol/MASTER_ROADMAP.md` (rev 122) as of this date. Those documents, plus
live issue state, remain the factual authorities.

### 1. Editing and proposal authority — largely delivered

**Have (implemented, receipted):** IDE-00 through IDE-08 are delivered and
closed with exact issue receipts (#9015–#9022, #9036). That covers the typed
project graph with generation fencing, admitted packages with no-authority
audits, the complete Pierre path index, real Monaco lifecycle, the daily
workbench, versioned review with staleness refusal, generation-safe language
intelligence, the deterministic basic-IDE acceptance oracle, and the
agent-native code graph: hash-bound proposals, transactional apply with
checkpoint rollback, the context-tray disclosure manifest, and host-observed
evidence. The "agents propose, and the project applies" law is running code.

**Gap:** the delivered evidence plane deliberately reports several facts as
`Unavailable` rather than observed. Tests are unobservable unless an exact
test command was admitted. Commit, push, PR, and delivery are `Unavailable`
in IDE-08 by design. Full delivery needs IDE-10 (terminal, tasks, tests,
output), IDE-11 (debug), and IDE-12 (safe SCM mutation, worktrees,
delivery) so that "tested," "committed," and "pushed" become host-observed
facts instead of honest absences. IDE-10 through IDE-19 (the
maintained parity ledger and owner acceptance) remain open — IDE-09 (AI
editing) closed on 2026-07-19 as #9037, per Addendum III Stage 0. The
accepted claim today is exactly "OpenAgents basic IDE" plus the agent code
graph and AI editing, and no broader rung may be spoken.

### 2. Proof machinery — designed and validated, not yet admitted

**Have:** the ProductSpec and AssuranceSpec formats exist with validators,
CLIs, and digest binding. The desktop trust workbench spec carries 52
acceptance criteria with per-criterion obligations, and the MVP precedent
shows the completed form — 18 obligations mapped, executable, CONFIRMED,
independently reviewed, and accepted, with no blended score.

**Gap:** both surface-wide AssuranceSpecs
(`desktop-trust-complete-workbench` and `full-auto`) remain
`lifecycle_state: proposed`. Their obligations are _designed_, not
_observed_: gates are empty, environments unselected, every authority flag
false, and unexecuted evidence decodes as `INCONCLUSIVE`. Full delivery
requires the whole ladder the formats already name: reviewer-authored risk
models, executable oracles per obligation, falsifier observations,
environment bindings to signed packaged builds, and — the scarcest input —
_independent_ review and admission that no producer can self-grant. A
designed oracle is not a passing observation, and today most of the surface
corpus sits one rung below observation.

### 3. Full Auto — core proven, closure gates open

**Have:** the durable-run core is landed and proven in real owner
development: exactly-once dispatch, restart-survivable state, routing
rotation inside the owner's grant, non-overridable guardrail cores, and
run reports with replayable failure fixtures.

**Gap:** the flagship's two closure gates are exactly the verification
story: #8978 (independent AssuranceSpec admission) and #8979 (binding the
proof to a signed packaged release with owner observation). And by explicit
cut, automatic done-condition verification is out of scope — "Completed" is
self-reported, never presented as verified truth. Full delivery of the
essay's thesis eventually needs that cut restored as a separately admitted
contract: machine-checked objective completion, not just honest labeling of
its absence.

**Update (2026-07-20):** the owner-real acceptance matrix is now receipted —
six of six named rows and the automatic Claude-to-Codex rotation passed at a
pinned commit, and the receipt itself keeps the rungs separate: "#8978 still
owns independent assurance and #8979 still owns the signed packaged
release/owner-observation gate." The Full Auto AssuranceSpec stands at 76 of
76 obligations _designed_ with independent admission still open — the
designed/observed distinction holding at full scale.

### 4. Trust-complete execution — spec'd, not landed

**Gap (the largest):** the authority-manifest/execution-receipt pair at run
level, named OS-enforced execution profiles that fail closed, hermetic mode
with a complete admitted-input manifest, delivery and confidence tiers
(draft / verified / reviewed / bonded) as visible product states, and the
signed release-set chain with proven rollback are the heart of "authority is
compiled, not narrated" — and they are currently Wave 2 acceptance criteria
in the desktop ProductSpec, not shipped behavior. Today's containment story
is real but narrower than the spec's. Until manifest/receipt pairing exists
for every run, the essay's claim that "every consequential run leaves a
pair of records" describes the contract, not yet the product.

### 5. The public trust surface — partial

**Have:** the promise registry with its full state machine, exact-only
public counters reconciled to receipted rows, the restored `/trace/{uuid}`
evidence viewer, Forum-first report intake, and signed desktop release
artifacts.

**Gap:** the dereferenceable trust ledger (release manifests, component
compatibility, mechanical pass/fail receipt-verification endpoints),
per-request routing disclosure, the audited data-flow matrix, benchmark
cohorts bound to full effective tuples, and the public `CodeShareBundle`
with verifier manifests (IDE-14) are spec'd in the web trust-surface
ProductSpec (31 ACs) but not served. "The only place where an agent
vendor's claims can be checked instead of believed" is today a design with
partial coverage, not a live guarantee.

### 6. The economics of proof — thesis, not product

**Gap:** priced confidence tiers, bonded outcomes, liability underwriting,
content-addressed study-packet reuse, and provenance-coupled settlement are
the essay's economic endgame and remain almost entirely forward-looking —
FastFollow defines the study-packet unit, the specs name the tiers, but no
customer today buys a "bonded" outcome or pays a provenance premium through
the product. This is acceptable sequencing (proof machinery must exist
before proof can be priced), but it is the widest gap between essay and
inventory.

### 7. Independence — the structural risk to watch

The companion deep dive's sharpest warning is the correlated-verifier trap:
AI checking AI shares blind spots. Our architecture separates producer from
verifier _by role_, but most oracles are still authored and executed inside
the same toolchain and mostly by agents. Full delivery needs standing
independent verification capacity — distinct reviewers for admission,
community falsification through the open promise/audit loop, and where
stakes warrant it, verification diversity that does not share the
generator's priors. The AssuranceSpec format already refuses
producer-admission. The gap is filling those reviewer roles with genuinely
independent capacity at the cadence the pipeline needs.

**Update (2026-07-20):** the independence law was exercised on a live
subsystem the day after this essay was written. The managed-sandbox lane's
producer-run staging matrix passed in full — real cloud lifecycle, fault and
cost rows, journey rows, restart, rollback, zero-residue — and the aggregate
evidence "deliberately records `independentAssurance=INCONCLUSIVE`: the
evidence producer cannot satisfy the AssuranceSpec's independent-verifier
boundary, and the owner has not yet recorded live observation. Production
enablement, a public availability claim, issue closure, and SBX-10
activation therefore remain blocked." A subsystem that passed everything it
ran, and whose own receipt refuses to call that acceptance, is this section
working as designed. The same plan carries the honesty rule down to
vocabulary: the sandbox resource "must report [its isolation] unit honestly.
The word container does not imply OCI-container semantics" — and its
anti-false-proof list is explicit that fake mode, configured job IDs, SDK
terminal state, screenshots, cloud resource existence, and successful guest
commands cannot prove live acceptance.

### Summary (Addendum I)

What exists today is the _authority skeleton_ of verifiable software — the
typed graph, the proposal plane, observed evidence, digest-bound specs, one
fully-proven MVP precedent, and a deterministic acceptance oracle — with the
discipline to label everything beyond it `Unavailable`, `proposed`, or
`INCONCLUSIVE` rather than green. What full delivery still requires, in
rough dependency order: observed execution evidence for tests, delivery, and
debug (IDE-10/11/12), run-level manifest/receipt pairing and enforced
profiles (Wave 2), independent admission of the standing AssuranceSpecs
(#8978 first), the public trust ledger and receipt-verification endpoints,
restored machine-checked completion for autonomous runs, and finally the
priced tiers that turn proof into product. The gap is large, but it is
enumerated, typed, and issue-addressed — which is exactly the state this
essay argues a verifiable system should be able to report about itself.

---

## Addendum II: The Verification Economy — a Keypair and a Wallet in Every Editor (2026-07-19)

Addendum I ended at priced proof: confidence tiers that turn verification
into a product. This addendum asks what infrastructure that actually
requires, and answers with a design thesis: **every editor carries a Nostr
keypair and a Lightning wallet, so every editor — and every agent working
inside one — can participate directly in a verification economy.** Buying
proof, selling proof, signing proof, and settling against proof, from the
place where claims are born.

This is not a new idea for this project. It is the project's oldest idea,
now aimed at its newest surface.

### 1. The lineage: this network was already built once

The transcript archive shows the thesis maturing across three product
generations. Onyx (episode 153) shipped a mobile app with a Breeze-SDK
Bitcoin wallet and a Nostr client, on the argument that "our AI agents…
are going to pay each other in Bitcoin." Autopilot (episodes 207–215) moved
it to the desktop and made it structural: "there will be a key pair
generated automatically for you when you open the app for the first time,"
with one BIP-39 seed phrase deriving _both_ the Lightning wallet and the
Nostr keypair — money and identity from the same root, stored locally,
self-custodied. Pylon (episodes 235–237) made it a property of every node:
"Every Pylon also ships a free, self-custodial Bitcoin Lightning wallet via
MoneyDevKit, so a brand-new node — even one on an old GPU — can set up an
identity and start earning sats the moment it comes online," with receive
liquidity spliced in at wallet creation so a fresh participant can be paid
immediately.

Around those clients grew an open protocol inventory, all on Nostr and
Lightning: NIP-89 service discovery, NIP-90 data vending machines (job
kinds requested, fulfilled, and paid competitively — "the freest market of
data processing AIs in the world"), NIP-DS dataset listings with canonical
content digests, a Skills NIP whose trust signal is paid-workflow history,
a Sovereign Agents draft using Frostr key-splitting so an agent can hold
keys its owner cannot exfiltrate, and NIP-LBR — the labor rail — whose
lifecycle runs request → quote → acceptance → result with provider _bonds_
and a content-addressed closeout receipt. Five markets were named and two
were launched on camera: compute, data, labor, liquidity, risk. Episode
213's Economy Kernel ported Catalini's cost curves directly into the
codebase. Episode 237 named the atomic unit (the accepted outcome), the
load-bearing wall (the clearing layer), and this essay's borrowed axiom:
"money only travels across a gap it can verify."

Episode 207 even priced the editor-to-editor case exactly: "you discover
some gotcha… maybe they just want to pay you three cents and save that
time." That sentence is the verification economy at the scale of one
keystroke.

### 2. What the keypair does: claims get an author

Everything in this essay's first half is about making claims _checkable_.
A keypair makes them _attributable_ — and attribution is what lets a claim
travel beyond the machine that produced it.

Concretely: every object the IDE already mints — proposal admissions,
apply receipts, host-observed evidence, behavior-contract oracle results,
acceptance-oracle decodes, CodeShareBundle manifests — is a typed,
digest-bound record. Signed with the editor's Nostr key, each becomes a
portable attestation: _this_ editor, under _this_ identity, observed _this_
fact at _this_ generation. A second editor can verify the signature and the
digest without trusting the transport, the platform, or the author's prose.
The deep dive's provenance premium — P(π=1) > P(π=0) — becomes a concrete
product feature: a diff that arrives with a signed evidence chain is worth
more than an identical diff without one, and the difference is the
verification labor a stranger no longer has to repeat.

Identity also accumulates. A keypair that has signed a thousand receipts
that later survived independent falsification _is_ a verification track
record — the "cryptographically verifiable career track record" Catalini's
playbook prescribes for individuals, except it works identically for
agents. Reputation stops being a platform database row and becomes a
portable, checkable history of survived claims. This is the paper's
verified network scale (N_V = ρN) built bottom-up: the authenticated share
ρ is not asserted by a platform, it is computed from signatures.

The authority discipline carries over unchanged: a signature is
provenance, never privilege. Signing a receipt grants no write, merge,
acceptance, or spend authority — exactly as the IDE's laws already hold
that sharing identity and evidence must not imply sharing authority. And
for agent-held keys, the Sovereign Agents/Frostr design keeps custody
honest: an agent can sign without its key being copyable.

### 3. What the wallet does: proof gets a price

The wallet turns the essay's economic section from thesis into mechanism.
Four flows, all denominated in the objects the IDE already produces:

**Buying verification.** The structural risk named in Addendum I §7 is
verifier correlation — AI checking AI shares blind spots (κ_corr ≫ 1). A
market is the cleanest decorrelator we know of. An editor that wants
independent proof posts a NIP-90-style job — "falsify this claim. Here is
the signed evidence bundle and the oracle" — and _strangers'_ editors,
running different models, different toolchains, different priors, compete
to break it for sats. Paid independent falsification is not a nice-to-have
on top of the assurance pipeline. It is the only scalable source of the
independence the AssuranceSpec format already demands. The reviewer roles
Addendum I could not fill by hire, a market fills by price.

**Selling verification.** The same editor, idle overnight, is supply. It
re-runs oracles, reproduces builds, executes falsifiers, reviews diffs
against behavior contracts — and every accepted verification is itself an
accepted outcome: scoped, executed, graded, receipted, settled. This is
episode 213's labor market with verification as the first product, and it
answers the deep dive's rationing problem (expert verification triaged and
sampled) with elastic supply: verification bandwidth stops being a fixed
human pool and becomes a priced network resource. The Codifier's Curse
still applies — every sold verification teaches the network to automate
that check — but here that is the flywheel working as intended: checks that
become mechanical get cheap, and human attention migrates up to designing
the next falsifier.

**Bonding outcomes.** NIP-LBR's provider bonds already model the top
confidence tier: a verifier stakes sats on its verdict, forfeited if an
accepted claim is later refuted. That is Liability-as-a-Service at
keystroke scale — "the product is no longer the agent. It is the
indemnified outcome" — and it makes draft / verified / reviewed / bonded
literal price points on one diff rather than marketing language.

**Selling evidence.** Study packets, teardown findings, edge-case
libraries, adjudicated false greens — verification-grade ground truth, the
K_IP^ver the paper says makes risk insurable — become sellable as NIP-DS
bundles with canonical digests: pay, receive, verify the digest, reuse the
proof. Verification amortizes across the network instead of being re-run
inside every editor. Money in, _checked_ data out.

Settlement and provenance ride the same rails — the deep dive's
observation that "the same rails that settle payments can also carry the
receipts" is literally the design: a Lightning payment resolving against a
signed closeout receipt is one object, not a payment plus an invoice plus
a claim in prose. And the essay's own discipline applies to money most of
all: episode 237's formulation — "a payment the recipient cannot
dereference is not a payment, it is a bug wearing money."

### 4. Current state, honestly

Following this essay's rules, the inventory:

**Live today:** every Pylon derives a real Nostr identity — NIP-06 HD keys
from a locally stored BIP-39 mnemonic (`packages/pylon-core/src/shared/nostr-identity.ts`)
— and authenticates its control surfaces with NIP-98 signed events. The
keypair half of the vision is running code in the node runtime today. The
editor inherits it the moment the IDE binds to Pylon identity.

**Protocol-complete, settlement-free:** `packages/nip90` implements the
market grammar — job kinds, the labor lifecycle, provider bonds,
content-addressed closeout receipts, dataset kinds — with tests and proof
scripts, and _by explicit design_ "moves no sats, opens no escrow, grants
no settlement authority." The receipts are real. The money is not.

**Deliberately retired or inert:** Pylon's wallet actions return
`money_capability_retired`. The Spark/MDK scaffolding (wallet panes,
control commands, payout targets) is present but unwired. The
openagents.com MDK sidecar/treasury/tips services are emptied and the
Lightning invoice rail is flag-gated inert. The Nostr relay app is
stripped. `docs/nostr/` is marked postponed (2026-07-08) behind the
current product focus. The network described in §1 was built, ran, paid
real sats on camera — and its money paths are currently, intentionally,
off.

So the honest statement is symmetric with Addendum I: the _identity_ half
is live, the _market grammar_ is implemented and tested, and the
_settlement_ half is a deliberate zero awaiting a product decision, not a
technical unknown. Re-activation is a bounded list — bind IDE identity to
the existing Pylon keypair. Sign the evidence objects the IDE already
mints. Re-enable a wallet path on the MDK stack that episode 235 proved
end-to-end. Wire NIP-LBR closeouts to settlement — and every item on it is
an owner-gated authority change under `AUTHORITY.md` and the payment
invariants, not a research problem. This essay flips none of those
switches. It argues they are worth flipping.

### 5. Why this completes the argument

The essay's core claim was that verification must be structural, not
voluntary. The deep dive added that it must also be _economic_: markets
under-supply verification unless it is priced, and "durable advantage
belongs not to those who generate output but to those who can certify it,
insure it, and absorb the liability when it fails." A keypair and a wallet
in every editor is where those two claims meet. The keypair makes every
receipt attributable and portable. The wallet makes every receipt
priceable and settleable. The IDE makes both ambient, because it already
mints the receipts as a side effect of ordinary work.

Reed's Law then does the rest. Editors that can pay each other for proof
form verification coalitions at machine speed — micro-markets around a
single falsifier, standing guilds around a test suite, bonded underwriters
around a release. The group-forming network the transcripts describe was
always, at bottom, a network of parties who can _check each other's work
and settle on it_. The editor is simply the right terminal for it: the
place where the claims are born is the place where they should be signed,
priced, verified, and paid for.

One market, cleared in receipts, settled in sats — reaching all the way
down to a three-cent gotcha and all the way up to a bonded release. That
is the verification economy, and the editor is its exchange floor.

---

## Addendum III: The Bootstrap — an IDE That Verifies the Codebase That Builds It (2026-07-19)

A compiler earns trust by compiling itself. An IDE that claims verifiable
software as its thesis must meet the same bar: it must build itself, under
its own laws, and its first verification customer must be its own
codebase. This addendum lays out that bootstrap — where it already stands,
the ladder it climbs, how it maps onto the open issue graph as of today,
and the one program that is currently missing: programmatic verification
of the OpenAgents codebase itself, across its whole surface area.

### 1. The bootstrap is already underway

Episode 254 was the cutover: Claude Code and Codex Desktop closed, and the
owner's development moved inside the near-alpha app — "we are going to fix
OpenAgents from within OpenAgents," with the first self-hosted commit
landing on camera. Episode 258 turned an incident into sixteen
by-construction controls. IDE-07's acceptance oracle decoded a real
packaged artifact, and that accepted build is the daily driver. The Full
Auto core's proof class is literally "owner-real development" — the
feature that builds the product was proven by building the product. The
loop exists. What follows is making each rung of it _evidence_ instead of
practice.

### 2. The dogfood ladder

Each stage converts one more part of the IDE's own development from
narrated work into observed, receipted work. The rungs map to open issues
directly:

**Stage 0 — live in the product (done).** Daily editing, agent work, and
autonomous runs happen inside OpenAgents Desktop. IDE-00 through IDE-09
are delivered and closed — including, as of #9037, Cursor-class AI editing
with disclosed effective models and version-bound edits. Every change to
the IDE can now flow through its own proposal plane: hash-bound
admissions, transactional apply, host-observed diagnostics, disclosed
context. The IDE's own Git history becomes a receipt stream.

**Stage 1 — observed tests (#9038, IDE-10).** Today the completion gate —
`pnpm run check` — is terminal prose from the IDE's point of view. The
evidence plane honestly reports tests as `Unavailable`. IDE-10 (terminal,
tasks, tests, Output over the project graph) is the single most
load-bearing packet for the bootstrap: after it, "the sweep passed" is a
host-observed fact bound to a workspace generation, for the IDE's own
repository first.

**Stage 2 — delivery receipts (#9040, IDE-12).** Git mutation, worktrees,
review, and delivery with exact-version receipts turn the repository's
completion discipline — commit on main, push, primary-checkout
reconciliation — into machine-checked state instead of agent-reported
state. The multi-agent hygiene rules this repo runs on (claims, leases,
clean worktrees) become enforced structure rather than contract prose.

**Stage 3 — unattended verification (#8967 + #9023).** Full Auto runs the
sweeps nobody watches. Managed sandboxes (SBX-06 #9027 integrates them
into the IDE project graph, SBX-09 #9033 proves isolation and cleanup)
give those runs disposable, receipted execution environments. Order
matters here: #8978 (independent AssuranceSpec admission for Full Auto)
and #8979 (binding to a signed packaged release) come first, so the tool
that verifies everything else is itself the first independently admitted
subject.

**Stage 4 — the release built by the product (#8913 chain).** DIST-13
(#8926) is one owner release command: freeze, five-target build/test,
sign, candidate smoke, promote. REL-FEED-01 (#8993) wires the live update
feed. The end state of the ladder is an IDE release produced, tested,
signed, and promoted through pipelines the IDE itself hosts and observes —
the full loop from proposal to shipped binary with no unreceipted rung.

### 3. The missing program: verify the OpenAgents codebase itself

Here is the honest gap the open-issue review exposes. The repository has
roughly eighty workspace projects — the web app and its Cloud Run
monolith, Pylon, the desktop app, mobile, the Cloud crates, dozens of
packages — plus routes, IPC boundaries, public APIs, and release
pipelines. Against that surface it has real but _uneven_ verification
assets: one completion gate (`pnpm run check`), the behavior-contract
registry with oracle enforcement, ProductSpec/AssuranceSpec validators
with one fully-CONFIRMED precedent (the MVP's 18 obligations, including
real mutation receipts under `openagents.mutation.v1`), the promise
registry, Electron smoke and QA harness suites, and STE document checks.
What does not exist — in the codebase or in any open issue — is the _map_:
a typed inventory that binds every surface to its oracle, or to an honest
`unverified` tag. IDE-10 through IDE-19, SBX, Full Auto closure, and the
DIST chain each verify their own slice. Nothing owns the whole.

The essay's own laws say what that program looks like. Call it
**ASSURE-REPO**, in the spirit of the packet programs that precede it:

- **AR-0 — surface inventory with loss accounting.** A machine-readable
  inventory of every app, package, crate, route, worker, IPC channel, and
  public endpoint, each row binding to its current oracles: behavior
  contracts, test files, assurance obligations, promise IDs, smoke
  journeys. Every surface either carries an oracle ref or an explicit
  `unverified` tag with a reason. No silent surfaces — the repo-scale
  version of "the UI says what was not observed."
- **AR-1 — obligations over the inventory.** Per-surface AssuranceSpec
  coverage graded exactly as the format already demands: designed versus
  observed, with `INCONCLUSIVE` as the default for everything unproven.
  The two proposed surface AssuranceSpecs become the first entries, not
  the whole story.
- **AR-2 — false-green audit.** The named taxonomy (fixture asserts, API
  mirrors, mocked seams, coverage theater, round-ups) applied to the
  existing suites, with mutation testing extended beyond the MVP precedent
  so a passing sweep is evidence about behavior, not activity. A green
  that survives mutation is worth more than a green that merely runs.
- **AR-3 — the standing sweep.** Continuous verification as a Full Auto
  lane in managed sandboxes: re-run oracles against current `main`,
  re-derive the inventory, diff coverage, and land results as receipts —
  with promise-registry and readiness surfaces consuming those receipts
  instead of assertions. This is where "leave it cleaner than you found
  it" becomes a machine's standing job rather than an agent's memory.
- **AR-4 — drift oracles.** Checks that the repo's own claims about
  itself hold: AGENTS.md and INVARIANTS.md assertions that name files,
  commands, and behaviors. OpenAPI against served routes. Roadmap status
  lines against issue state. The unverified operational directive from
  Section I was exactly this class of failure — a stated fact with no
  compiler — and the repository's own documentation is the largest
  unverified surface it has.

**Fit:** AR-0 and AR-2 can start now — they need only the repository and
the existing validators. AR-3 wants Stage 1 (IDE-10) and the SBX lane for
full fidelity but can run degraded (terminal-observed, honestly labeled)
before that. None of it blocks IDE-11 through IDE-19. Per this
repository's own rules, ASSURE-REPO needs admission through the Sol
roadmap and owner acceptance — feature issues are not self-service, and
this essay, per its standing header, dispatches nothing. It is the
argument for the packet, not the packet.

**Update (2026-07-19):** ASSURE-REPO was owner-admitted (epic
[#9055](https://github.com/OpenAgentsInc/openagents/issues/9055),
ProductSpec `specs/openagents/assure-repo-codebase-verification.product-spec.md`)
and all five packets AR-0 through AR-4 (#9056–#9060) are **delivered** and on
`main` as `@openagentsinc/assure-repo`. The map now exists: a deterministic
surface inventory with loss accounting (154 surfaces, zero silent), obligation
grading that never fabricates `observed`/`accepted`, a demonstrated-by-mutation
false-green audit, a standing sweep with receipts, and drift oracles that
already caught real stale claims in the governing docs. Program overview:
[`docs/assure-repo/README.md`](../assure-repo/README.md). The paragraphs above
are retained as the original argument for the packet. The packet now ships.

**Update (2026-07-20):** one day in, the map is already doing the thing maps
are for — showing an honest picture nobody would have asserted from memory.
The inventory stands at 161 surfaces: 147 carrying an oracle ref, 14
explicitly `unverified` with a stated reason (ten with no oracle authored,
two config-only, two reference-only), and zero silent. The grading breakdown
is 115 `designed`, 32 `mapped`, 9 `inconclusive`, 5 out-of-scope — and
structurally zero `observed` or `accepted`, because "grading never emits
observed or accepted: those require a passing, source-bound AR-3 sweep
receipt or owner acceptance." The README's own caveat is the discipline in
one line: "An oracle ref is an **index entry, not a verdict**: it proves an
oracle is authored for the surface, not that the surface is proven." The
false-green audit has scanned 2,418 test files and holds 20 candidate leads
(18 coverage-theater, 1 mocked-seam, 1 round-up) under the rule that
"candidates are **leads, not findings**: a lead becomes a confirmed finding
only when demonstrated by a **surviving mutation**." The standing sweep runs
honestly degraded — labeled `degraded_terminal_observed` until host-observed
tests and the sandbox lane land — and repo readiness renders only from a
decoded, fresh receipt: absent or stale renders `unknown`, never `green`.
And AR-4 closes the loop this essay opened in Section I: the governing
documents "assert file paths and commands that nothing checked — the
unverified-operational-directive failure class applied to the codebase
itself. AR-4 gives those claims a compiler."

### 4. Why the bootstrap is the proof

Self-verification has a boundary the essay has already drawn twice: no
producer admits its own work. An IDE that verifies itself still needs
independent falsification — which is precisely what Addendum II's economy
supplies (strangers paid to break your claims) and what #8978 models
internally (an independent reviewer role no producer can fill). The
bootstrap does not repeal that law. It is the law applied reflexively,
with the codebase as first subject.

But within that boundary, dogfooding is the strongest evidence a
verifiable-software claim can generate. Every hour of the IDE building
itself produces verification-grade ground truth as a byproduct — real
proposals, real staleness refusals, real observed sweeps, real false
greens caught and adjudicated — the exact K_IP^ver the economics says
makes oversight cheap and risk insurable. The teardowns judged competitors
by their artifacts. This program invites the same judgment and stakes the
thesis on surviving it. A company whose product is proof, proving its own
codebase in public, with the map of what remains unproven published
alongside — that is not just the dogfood plan. It is the demo.

---

## Addendum IV: The Deliverable — Verifiable Software Delivers Accepted Outcomes (2026-07-20)

The addenda so far answer _how_: the proposal plane, the assurance ladder,
the keypair and the wallet, the bootstrap. This one answers _what for_.
Verifiable software is not an end in itself. It is a production discipline
for exactly one deliverable, and the archive named it before the IDE
existed. Episode 237: "The atom of the economy is the accepted outcome — a
task scoped before it ran, executed wherever execution was cheapest, graded
against a rubric, recorded in a receipt, and settled to everyone who
contributed." (Section VI already borrows that sentence — this is its
origin.) Everything this essay describes — the typed graph, the
host-observed evidence, the digest-bound specs, the oracles, the receipts —
exists so that software can _deliver that object_ instead of merely
narrating activity. That is the position: **verifiable software is the
software that will deliver accepted, verified outcomes.** The rest of this
addendum traces where the unit came from, why verification owns its
economics, and what that implies for what we build.

### 1. The unit migrates

The history of computing-as-economics can be told as a migration of the
unit of account. Bitcoin priced the **hash**: pure, verifiable work,
valuable only because it secured a monetary network, revolutionary because
it was the first digital work product that could be sold from anywhere and
verified by anyone. The AI era priced the **token**: statistically useful
output sold by the million, with the curious property that nobody buying
tokens wants tokens. Selling tokens is selling CPU cycles to a buyer who
wanted a spreadsheet — workable for experts, hostile to everyone else's
ROI. What buyers want is the thing a token stream occasionally adds up to:
a task completed, checked, and trusted. The unit migrates once more, to the
**accepted outcome** — and the migration is the market this essay's
economics section pointed at.

The archive coined the operating metric on camera, months before this
essay. Episode 232 introduces "accepted outcomes per kilowatt hour" as the
metric the whole operation optimizes for, and gives it its definitional
question: "You have a stream of electrons, what is the cost of turning that
into an accepted agent task?... What is the absolute most cost-efficient
way of converting electron to accepted agent work?" In the same breath
comes the taunt that doubles as a category claim: go ask your favorite AI
lab for its accepted
outcomes per kilowatt hour, and it will not know what the heck you are
talking about — nobody else is even measuring it. The metric has a
property worth noticing: it is hard to compute honestly — which is a
feature, because computing it forces receipts. A company that can state its
accepted outcomes per unit of input has, by construction, built acceptance
records, receipts, and settlement. The metric is the discipline, priced.

### 2. The lineage: the unit was earned, not asserted

The transcript archive shows the unit assembling itself across a year of
shipped work, each episode adding one load-bearing piece.

Episode 213 names the gap and imports the theory: "one of the big limiting
factors to agents really catching on and going mainstream is the lack of
ability to verify that the work that they do is... correct," recorded in
the same session that ported published agentic-economics cost curves
directly into the codebase as the Economy Kernel. Episode 231's forum
carries the thesis as a thread title: "pay the people, with receipts."
Episode 232 coins the metric. Episode 234 puts the company's own claims
under it — the promise registry goes live on camera: "Let's take that idea
of what's actually live and turn it into a concept that agents can verify
and tell you," with every claim mapped to green, yellow, red, or withdrawn
and the reds shown unflinched. Episode 237 names the atom and the axiom —
"money only travels across a gap it can verify" — and adds the sentence
that reframes the whole product: the operator's real product is not the
wiring — "it is the receipt that proves the wiring worked."

Then the loop runs. Episode 238, on camera: "CLAIM WORK" → "WORKER RUNS
JOB" → "VALIDATOR REPLAYS" → "VERIFIED" → pay both. "A validator — someone
else on the network — is going to replay it," check it, and once verified,
"both worker and verifier are going to get paid." Producer
and verifier, separated by construction and both compensated — the
authority-separation law of Section III operating as a paid economic loop,
at the scale of a few sats. The same episode states the epistemic posture
in one breath: no big claims that are not sourced by evidence, because
"We've got a pretty serious evidence pack built into all this." Episode 239
then carries verification inside the standing market list — "agent markets
for compute, data, labor, liquidity, risk, and verification," a list the
speaker traces back to episode 213 — under the standing requirement that
"all of this should be
programmed open source verifiable backed by receipts that pay you money."
Episode 240 renders it: a live run board whose counters read "5 DAYS, 21
WINDOWS, 12 VERIFIED, 1,020 ACTIVE SATS" — _verified_ as a first-class,
watchable number, not a footnote. Episode 243 prices it: public benchmark
comparisons "are receipts, not vibes," reporting "cost per accepted
outcome (our favorite metric)" and verification rate — alongside the
honesty bar that the flagship coding surface may not be claimed verified
"absent an executed acceptance verdict," and that direction "stays labeled
as direction until it has a green promise." And episode 246 closes the arc
where this essay begins, with the behavior-contract registry ("Every entry
records the statement verbatim, who stated it and where, and the oracle
tests that enforce it") and the QA swarm — "point a swarm of QA agents at
your product and get proof it works," every run ending in "an honest
CONFIRMED/REFUTED verdict."

The spine, compressed: 213 names the gap, 232 coins the metric, 234 turns
the company's own claims into verifiable objects, 237 names the unit, 238
pays a worker and a validator on replay, 239 lists verification among the
markets, 243 prices it, 246 wires it into the software itself. The accepted outcome
is not a slogan retrofitted onto the IDE. The IDE is the latest instrument
built to produce it.

### 3. Why verification owns the economics

The price of an accepted outcome has a simple structure: the expected cost
per attempt, divided by the acceptance rate, plus margin and a reserve
against the risk that acceptance was wrong. Run that formula against any
honest attempt stack and an uncomfortable truth surfaces: the model tokens
are the small term. Review, retries, grading, and failure handling
dominate the cost of an accepted outcome — often by an order of magnitude.
The celebrated collapse in generation cost operates on the smallest line
item. Whoever bends the verification curve — review effort per accepted
outcome trending down while task value trends up — owns the margin of the
entire category.

That curve is bent by a small set of compounding moves, every one of which
is already named in this essay's machinery — much of it, per Addendum I,
still at the design rung. **Amortize:** verify once,
reuse everywhere — content-addressed proof, shared study packets, receipts
that strangers can check instead of re-deriving (Section VI, Addendum II).
**Decompose:** shrink the verification quantum — bounded sub-tasks with
typed inputs admit deterministic checks that a monolithic deliverable
never will (the proposal plane's whole design). **Downshift:** route each
check to the cheapest sufficient checker, up an explicit effort ladder —
nothing, deterministic tests, self-review, independent judge, second-agent
re-run with diff, human review, bonded acceptance — and sell the rungs as
the confidence tiers the buyer already understands: draft, verified,
reviewed, bonded. The scheduler's job is the cheapest _sufficient_ rung.
The catalog's job is to price the difference. **Incentivize:** pay
whoever cheapens verification — a verifier that reduces refund rates or
review minutes earns from the margin it creates, even though no end buyer
ever sees it, which gives the verification curve a paid constituency
(Addendum II's market). **Eliminate:** the limit case — work whose
execution trace is exactly replayable, so verification collapses to
re-running the trace on the cheapest machine available. Episode 238's
loop already paid a worker and a validator on precisely this class: the
validator did not judge the work, it _replayed_ it. For that class the
trace is not evidence attached to the work. It is the work.

The demand side appears to be arriving at the same unit from unrelated
evidence. The enterprise buyers assembling AI governance stacks
increasingly ask the one question their CFOs can act on — not "how many
tokens did we use" but
_what did this agent cost per completed business outcome_: per resolved
ticket, per merged PR, per reconciled invoice. When supply-side dispatch
economics and demand-side procurement anxiety independently land on the
same accounting unit, the unit is probably real. And in a purchase
mediated by a governance layer, the easiest vendor to govern wins — the
vendor whose every outcome already arrives with a receipt graph, a
provenance label, and a confidence tier.

### 4. What this obligates the software to be

If the deliverable is the accepted outcome, the essay's whole apparatus
re-derives as necessary rather than virtuous. Acceptance requires a
falsifiable definition of done before the work runs — that is the typed
ProductSpec and the behavior contract. Acceptance requires evidence the
producer cannot manufacture — that is host observation and the
producer/verifier boundary. Acceptance requires a verdict that travels —
that is the receipt, the signature, the digest. Settlement requires all
three, because money only travels across a gap it can verify.
Verification is not a stage that follows the work. It is the market's
admission requirement for the work existing at all.

The IDE is where this lands, because the IDE is where the work is born.
An editor that mints proposals bound to exact bases, observes its own
evidence, refuses stale state, and signs what it saw is not just a safer
editor. It is the front office of an outcome economy: every unit of work
it produces leaves already scoped, already graded, already receipted —
one oracle short of accepted and one settlement short of paid. That is
the difference between software that helps you work and software that
delivers work. The first sells effort. The second sells outcomes, and can
prove them.

The honest status, per this essay's own standard: the unit is named, the
metric is coined, the registry that scores the company's claims is live, the
replay loop has paid real sats on camera, and the wallet rails are
deliberately off pending an owner decision (Addendum II §4). No customer
today buys a bonded outcome through the product — that remains the widest
gap in the inventory (Addendum I §6). What episode 246 says about the gap
is the right closing discipline for an essay that has spent four addenda
mapping it: the one thing the company needs is to close the gap between
what it has been saying and what it is shipping — "If I close that gap, I
have a successful company."

Scope the work. Verify the claim. Settle the result. Learn from the
receipt. Verifiable software is the machine that runs that loop — and the
accepted outcome is what comes out of it.
