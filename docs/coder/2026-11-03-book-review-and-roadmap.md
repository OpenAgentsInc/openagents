# Coder book review and implementation roadmap

Reviewed 2026-11-03 against repository revision
`2b02fb03d8b00cce262e5e2a3e380d278e1de172`. Author: GPT-6.

## Assessment

*Coder: Coding Agents From Scratch* has a convincing central idea: a coding
agent should be understandable through its record. What the model saw, why
an action was allowed, what changed, and what happened after an interruption
should have explicit answers. The book develops that idea through logs,
prefixes, tools, approvals, condensation, memory and recovery, then compares
it with other agents at pinned source revisions.

The most valuable next step is to connect those checked components to a
small, complete coding workflow and measure it. The book currently explains
three things at once: bounded harness experiments, the live Coder product,
and proposed additions borrowed from other systems. Each is useful, but
readers can mistake a guarantee established in one for a guarantee of all
three. Fixing that distinction should precede more feature expansion.

This review evaluates the book's explanations, evidence and proposed
architecture. Runtime statements below are attributed to the chapters; the
review is not an independent qualification of their compiler, mutation,
security or live-model results. The roadmap contains proposed work items,
not filed GitHub issues or completed implementation work.

## Strengths to preserve

### The record gives the architecture a clear purpose

[Chapter 2](../../coder/book/ch002-model-visible-means-logged.md) makes
request reconstruction the organizing rule. Condensation becomes a recorded
transformation rather than an invisible edit. Consent records why bytes
could leave. Memory retains a source and a reason for revision. These
connections make the book substantially more useful than a catalog of agent
features.

Keep the small worked traces, explicit stop reasons and independent oracles.
They let a reader reason about boundary cases without trusting a large live
demonstration. Mutation results strengthen those checks within their named
profiles; they should continue to be presented with that scope.

### The chapters usually describe their limits honestly

The distinction between logical ticks and wall time, programmed weights and
learned behavior, and simulated selection and real code repair is valuable.
[Chapter 6](../../coder/book/ch006-a-document-tries-to-become-the-system-prompt.md)
also separates unauthorized effects, answer corruption and sentinel leaks.
That is a particularly good example of refusing to turn one successful
containment experiment into a universal security claim.

The watchdog's reported false positive in
[chapter 20](../../coder/book/ch020-an-agent-that-stays-online.md) is equally
useful. Preserve examples where a mechanism makes things worse; they tell
the reader what the next experiment must resolve.

### Existing engines are a practical product foundation

Driving Claude Code and Codex while adding continuity, job control and
remote execution gives Coder a useful role. Retaining each engine's native
tools is a reasonable starting choice. It also creates an obligation to
describe which decisions belong to Coder and which remain inside the engine.

The new read-only conversation handoff in
[chapter 14](../../coder/book/ch014-forks-subagents-and-jobs.md) is progress
toward that continuity. It already describes bounded discovery, branch
recovery, pagination, compaction context and direct registration in new
Coder Codex chats. Its inert-data boundary is an existing
[repository invariant](../../INVARIANTS.md#coder-reads-foreign-conversations-as-bounded-inert-data).
Extend and measure this reader instead of planning a replacement from zero.

### The surveys turn reading into engineering decisions

The repeated questions about what to take, leave and measure make Parts VII
and VIII actionable. Especially useful comparisons include Aider's edit
failure counters, mini-SWE-agent's full history versus Microcoder's bounded
state, OpenCode's separation of admission and execution, and Flue's handling
of calls whose effects are unknown after a crash.

The inline terminal design also has a clear user benefit: normal scrollback,
selection and mouse behavior remain available. Keep this concrete reasoning
when evaluating future UI work.

## Findings and recommended changes

### F1 Explain which guarantees reach the live product

The early implementation chapters mostly use bounded components in
`psionic/lib/harness/`. The live engines use `coder/tui/`,
`coax/lib/microcoder/` and engine-owned histories. These paths do not yet
provide identical guarantees:

| Topic | Evidence in the book | Update needed |
| --- | --- | --- |
| Request reconstruction | Chapter 2's experimental log records every token; chapter 23 rebuilds the text sent to an engine at each turn | Distinguish an engine input from every model request made inside that engine |
| Prefix and condensation | Chapters 3 and 7 check logged views; chapter 7 says Microcoder prunes by fixed rules without logging those transformations | Show the live transformation and the reconstruction coverage it currently permits |
| Effect ordering | Chapter 2 says live tool lines are durable before Coder reports them, but tools may already have acted | Separate durable reporting from durable intent before execution |
| Injection containment | Chapter 6's labelled gate is not yet joined to the loop and remote calls | Name the live ceiling checks separately from the experimental provenance gate |
| Remote tools | Chapter 16's finite typed adapter and Coder's practical MCP server are different implementations | State each one's supported schemas, limits and admission boundary |

Add a compact guarantee table near the introduction and a consistent
"In the experiment / In Coder / Remaining integration" section to each
implementation chapter. A single status such as "partly implemented" is
too broad to communicate these differences.

For opaque engines, use an explicit unsupported or engine-owned state.
Do not promise complete model-request reconstruction when the engine only
exposes turn inputs and selected events. The book can teach a stronger
native harness without pretending every adapter already provides it.

### F2 Make chapter 21 useful for actual coding decisions

[Chapter 21](../../coder/book/ch021-measure-the-harness.md) builds sound
measurement machinery: pinned inputs, separate development and held-out
tasks, receipts, noise floors and an unverifiable verdict. Its current
scripted tasks test the calculator and budgets. They cannot establish
whether routing, repository maps, memory, plugins or subagents improve
repository work.

Keep that experiment as a measurement tutorial. Add a separate repository
suite that exercises the live CLI with deterministic acceptance checks and
real models when explicitly invoked. Start with roughly ten small tasks;
this is a qualification pilot, not enough evidence for broad superiority
claims. Freeze the split and checks before tuning. Use additional tasks and
repeated runs when making comparative claims.

Report task completion, regressions, malformed replies, failed edits,
timeouts, infrastructure failures, interventions, elapsed time, token usage
and cache usage when observable. Report unavailable usage and subscription
cost as unavailable, rather than manufacturing comparable prices. A tool
being exercised is not evidence that its addition improved outcomes.

Correctness, confinement and consent requirements are obligations even when
they do not increase task success. Apply "earns its place" comparisons to
optional complexity. Otherwise the rule would perversely favor removing a
necessary restriction to improve a benchmark score.

### F3 Teach a complete repository task earlier

The calculator makes exact hand traces possible, but the reader waits too
long to connect those traces with coding. Introduce one small Coax fixture
repository with a failing check, an irrelevant file and clear instructions.
Carry the task through reading, editing, checking, interruption, continuation,
diff review and delivery.

The example should show a real failure and its diagnosis, not only a happy
path. Keep development exercises separate from hidden evaluation cases.
Use a local captured delivery sink for routine checks; a live push or pull
request requires an explicitly authorized destination. Repository-specific
delivery rules still apply, including this repository's worktree and
push-to-main workflow.

### F4 Evaluate routing by outcomes and availability

[Chapter 13](../../coder/book/ch013-choose-an-engine.md) reports 22/22
agreement on its held-out labels. That supports the claim that the decision
model reproduces the intended routing policy on that set. It does not show
that the selected engine is best for each task. The labels encode the
preference for Claude Code on broader work and Codex on contained work;
that preference needs an outcome experiment.

Compare all available engines on the same repository tasks and budgets.
Report the router against a fixed default and against the best observed
eligible engine per task. The latter is an optimistic reference, not an
available production oracle. Include decision overhead, quota failures,
offline operation, consent refusal, and the currently unrepresented Psionic
choice. Treat decision probabilities as model scores until calibration is
measured. Typed answers make a decision inspectable; typing alone does not
make it correct.

### F5 Define replay and recovery precisely

The book uses replay for several operations: rebuilding a request,
consuming recorded responses, regenerating a deterministic model from a
seed, and recomputing simulated tool effects. Chapter 9 explicitly notes
that commands still execute during gateway replay. Chapter 23 compares
reconstructed engine-input digests. These are different operations with
different consequences.

Document distinct modes and their effects. Offline verification should not
implicitly run recorded shell commands. Re-execution should require a
disposable environment and an explicit action. Hosted model output need
not be reproducible merely because a seed or request is known. Redaction
may deliberately make historical reconstruction impossible; return that
state clearly.

Recovery needs an equally explicit boundary. Chapter 20 checks Router
launch and delivery crash points, while chapter 2 documents tool effects
that precede their Coder record. Those Router checks do not establish
exactly-once arbitrary tool execution. Use the conservative approach
described in the OpenCode and Flue surveys: retain completed evidence,
classify unresolved effects as unknown, and repeat only operations with a
declared, enforced retry contract. A hash chain establishes consistency
relative to its retained head and admission rules; it does not establish
historical authenticity against someone who can replace all evidence.

### F6 Make privacy claims match all outbound paths

[Chapter 18](../../coder/book/ch018-consent-before-sharing.md) describes
gates for remembered facts sent to engines and session content sent to
System One. It also explicitly says ordinary conversation and tool output
reach engines without that gate. Chapter 24's claim that no session content
goes anywhere without a per-session opt-in is broader than that coverage.

Define the authorization semantics of an explicit conversation with a
chosen provider, automatic routing, fallback to another provider, memory
embedding, foreign-history handoff, remote tools, delegation and export.
An explicit request may authorize a particular send; the documentation
must say which one. Distinguish local storage, ordinary inference and
optional telemetry or training. The Hermes survey's discussion of local
trajectory writing should not equate writing a file with outbound sharing.

Inventory destinations and enforce the chosen rules where bytes actually
leave. Scope-based labels for facts are useful but do not, by themselves,
describe all sources carried in a summary or log excerpt. Exercise
revocation, expiration, unavailable consent storage and provider fallback.
Include concurrent processes and the ledger's documented capacity limit.

### F7 Separate admission policy from actual confinement

Chapter 5's three rooms are an experiment in policy, not an OS sandbox.
Chapters 10 and 11 describe engine permission settings, and chapter 20
shows why relying only on permission requests missed Claude Code's
self-approved read commands. The added PreToolUse hook is useful evidence
about one engine path, not a substitute for every platform boundary.

Publish a per-engine table of hooks, tool effects, roots, process
confinement, network policy, unattended asks and default modes. The
documented defaults include full access unless options select a policy;
chapter 28 should not imply an approval bypass does not exist. Document
trusted local shell plugins separately from bounded Wasm plugins.

The injection lab's planner/reader separation is a promising design to
test. Length framing prevents a structural parser forgery; it cannot force
an LLM to treat framed content as non-instructing. Provenance labels must
come from the harness and survive transformations. When a model sees
mixed sources, conservatively labelling its output may hold benign work
too. Measure task completion and approval burden alongside blocked effects.

### F8 State memory lifecycle and source semantics

[Chapter 19](../../coder/book/ch019-remember-a-fact-then-revise-it.md)
provides useful conflict, revision and forget rules. Its live project scope
is a hash of the physical folder path, so another worktree is another
scope. That matters in a repository whose required workflow uses separate
worktrees. Explain the behavior now; decide deliberately whether to add a
stable project identity or an explicit sharing operation later.

Also explain the distinction between a changed source line, which withholds
a fact, and a deleted source log, which currently allows it to remain.
Record whether a fact is independently retained knowledge or evidence that
must still be available. Forgetting text in the memory table and event file
does not erase copies in older session inputs, engine histories, backups or
already delivered messages. Document those boundaries and test removal of
the derived vector cache. Evaluate the embedding send under F6's rules.

### F9 Reconcile survey conclusions with current chapter status

The surveys are valuable, but several conclusions use present-tense
implementation language where the owning chapter is more limited:

| Passage | Conflicting or narrower evidence | Required correction |
| --- | --- | --- |
| Chapter 25 says hooks act only through admitted Wasm | Chapter 15 documents shell commands and hooks | Describe both trust classes and mark any proposed restriction as future work |
| Chapter 28 rejects an approve-everything switch | Chapters 10 and 11 document permission bypass defaults | State current defaults and the conditions that turn policy on |
| Chapter 24 claims comprehensive per-session consent | Chapter 18 lists ungated conversation paths | Limit the claim to enforced paths and specify the intended extension |
| Chapter 33 says editor ACP is not built | Chapter 16 documents an ACP server and an editor-shaped smoke, without a real editor run | Separate implemented protocol behavior from live editor qualification |
| Chapter 34 says `coder run` provides a task branch and attributed pull request | Chapter 23 leaves the complete repository journey open | Mark delivery automation as proposed unless there is named evidence |
| Chapters 30 and 31 favor independent commands; chapter 33 proposes a persistent shell | Chapter 37 has PTYs and agent terminal tools, while a persistent shell remains a further step | Separate baseline execution from optional interactive terminal capability |

Replace "takes" with "implemented", "proposed experiment" or "rejected"
for each borrowed mechanism, linked to its owning task and evidence. Do not
resolve contradictions by silently broadening an implemented claim.

### F10 Make the book easier to navigate and maintain

Provide two reading paths: build the smallest useful Coder, and study the
complete harness architecture. Introduce the CLI journey early while
keeping detailed derivations in their current chapters or linked sections.
Reduce repeated full code listings where a focused excerpt plus a source
link teaches the same point, preserving the exact-listing checks.

Keep the Router/Worker ownership boundary clear: OpenAgents owns shared
channels and general identity; Coder is its repository Worker. Explain why
the current Coder executable hosts prototype Router behavior and where that
behavior is expected to live. A pronoun convention alone is not an
ownership boundary.

Synchronize README, SUMMARY, PLAN and `chapters.json`. Repair migrated
references such as old chapter numbers and joined words such as
"chapter1". Keep source revision, product release and live qualification
date distinct. The private Claude Code mirror's documented release
uncertainty must remain visible; its source presence is not proof of shipped
behavior. No additional survey chapter is needed to resolve these problems.

## Roadmap

Use the identifiers below as checklist references or issue-body seeds. P0
means correct a misleading guarantee or unblock a trustworthy baseline;
P1 means complete or qualify the workflow; P2 means compare optional
mechanisms after that baseline exists. Dependencies are implementation
dependencies, not reasons to postpone accurate documentation.

### Phase 1 Establish an accurate contract

#### R01 Publish the guarantee matrix

- [ ] **P0; findings F1, F7, F10.** Add experiment, Microcoder, Claude Code,
  Codex and Psionic columns for reconstruction, effect ordering, pruning,
  confinement, consent and recovery. Use explicit supported, partial,
  engine-owned and unqualified states with evidence links.
- **Acceptance:** every public guarantee has an owning implementation and
  named qualification or an explicit limitation; unsupported engine paths
  are visible. Chapter status never substitutes for a per-engine claim.
- **Depends on:** none. **Touches:** book introduction, chapters 1–8, 9–12,
  16, 18, 20 and 23.

#### R02 Reconcile the surveys and status inventories

- [ ] **P0; findings F9, F10.** Resolve every row of F9; classify borrowed
  designs by implementation state. Update README, SUMMARY, PLAN and registry
  together, preserving the new conversation reader's actual coverage.
- **Acceptance:** no survey claims stronger coverage than its owning
  chapter; old chapter references and status counts agree. Run the existing
  Coder book checker and document any pre-existing failures separately.
- **Depends on:** R01's terminology. **Touches:** chapters 24–35 and book
  navigation files.

#### R03 Define replay and interruption outcomes

- [ ] **P0; finding F5.** Specify request verification, recorded-response
  playback, deterministic regeneration and deliberate tool re-execution.
  Define partial, redacted, unavailable and unknown-effect outcomes.
- **Acceptance:** a reader can identify which mode runs commands, makes
  network requests or requires engine-owned storage. Public CLI help and
  chapter examples use the same terms and do not promise arbitrary
  exactly-once effects.
- **Depends on:** R01. **Touches:** chapters 2, 9, 14, 20, 23 and CLI help.

#### R04 Specify authorization for every outbound destination

- [ ] **P0; findings F6, F8.** Inventory inference, routing, fallback,
  embedding, tools, handoff, delegation and export. State which explicit
  user actions authorize each send and which require a separate grant.
- **Acceptance:** the policy describes destination changes, derived content,
  revocation and storage failure; chapter 18 and every survey use the same
  scope. No claim treats local logging as outbound sharing.
- **Depends on:** R01. **Touches:** chapters 13, 14, 16, 18, 19, 24 and 32.

### Phase 2 Deliver and observe one complete task

#### R05 Add the running repository fixture and journey

- [ ] **P0; findings F2, F3.** Build a small Coax repository fixture and a
  Coax journey harness for run, log, failure, edit, check, fork, stop,
  continuation and final diff. Model and delivery stand-ins keep the default
  check offline. Use isolated worktrees and disposable state.
- **Acceptance:** one command records the expected transitions and checks
  the final patch, untouched files and required failure paths. It exercises
  the shipped CLI boundary, not only helper functions. Chapter 23 links the
  result and chapter 1 introduces the task.
- **Depends on:** R03. **Touches:** chapters 1, 9, 14, 23 and Coder fixtures.

#### R06 Record native model inputs and state transformations

- [ ] **P1; findings F1, F5.** Persist the complete native request identity
  and the inputs needed to reconstruct it, including state pruning, memory
  snapshots, instruction changes, output cuts and gateway selection. Start
  with Microcoder; describe opaque engine boundaries honestly.
- **Acceptance:** every native model step reconstructs offline after memory
  or source files change. Boundary tests cover the thirteenth action,
  oversized output, partial streams and redacted inputs. Verification never
  executes historical commands. Publish versioned contracts through `proto/`
  and the protocol spec where applicable, retaining old readers.
- **Depends on:** R03, R04, R05. **Touches:** chapters 2, 3, 7, 9 and 23.

#### R07 Qualify policy and confinement per engine

- [ ] **P1; findings F1, F7.** Exercise each advertised engine path with
  local roots, outside paths, indirect file access, network attempts,
  unknown tools, terminal tools and unattended asks. Separate trusted shell
  plugins from bounded Wasm execution and cover child policy inheritance.
- **Acceptance:** supported profiles have checks at their actual enforcement
  boundary; unsupported OS/engine combinations are named. Approvals cannot
  widen ceilings, and a label or hook alone is not described as OS
  confinement. Follow `#authority` and zero-survivor mutation rules for
  changed admission decisions.
- **Depends on:** R01, R05. **Touches:** chapters 4–6, 10, 11, 14–16 and 37.

#### R08 Implement the outbound policy at transport boundaries

- [ ] **P1; findings F6, F8.** Apply R04's policy to the actual transports,
  including embeddings, provider fallback, handoff data and remote tools.
  Preserve the explicit-call semantics R04 defines rather than adding
  duplicate prompts indiscriminately.
- **Acceptance:** captured sinks contain only authorized bytes; revocation,
  expiry, full or invalid ledgers, concurrent processes and destination
  changes have focused tests. Derived data retains its applicable source
  classes. Secret redaction in a display does not count as preventing a
  secret from reaching a model.
- **Depends on:** R04, R05. **Touches:** chapters 13, 16, 18–20 and transports.

#### R09 Recover unknown effects without automatic duplication

- [ ] **P1; finding F5.** Record intent before native effects, record results
  afterward, and classify calls interrupted between those records. Reuse
  existing Router intent and comment-marker mechanisms where applicable.
- **Acceptance:** fault injection before execution, during an effect and
  after execution but before recording produces an honest status. Completed
  evidence is retained, unresolved non-idempotent calls are not blindly
  repeated, and repeated external writes need receiver-supported identity
  or reconciliation. Document remaining windows and multi-writer limits.
- **Depends on:** R03, R05, R06. **Touches:** chapters 2, 4, 14, 20 and 23.

#### R10 Qualify continuation and memory across worktrees

- [ ] **P1; findings F3, F8.** Test resume, fork, import and the existing
  conversation reader on the fixture. Define project identity and memory
  source-retention rules, then test revisions and forgetting against them.
- **Acceptance:** handoff recovers the task and stopping point, exposes
  omissions and keeps historical instructions inert. Current files are
  rechecked before action. Changed/deleted sources, another worktree,
  forgotten vectors and existing session copies have documented outcomes.
  Retain the reader's bounded refusal and pagination invariant.
- **Depends on:** R04, R05; outbound handoff qualification also needs R08.
  **Touches:** chapters 14, 18, 19 and their existing reader/memory checks.

### Phase 3 Measure the useful baseline

#### R11 Add a pinned repository evaluation suite

- [ ] **P1; findings F2, F3.** Add approximately ten pilot tasks covering
  diagnosis, contained fixes, multiple-file changes, test repair and
  continuation. Freeze development/held-out membership, checks, budgets,
  repository revisions and model identities before tuning.
- **Acceptance:** deterministic acceptance is separate from visible feedback;
  receipts distinguish task, harness and infrastructure failures. Report
  completion, regressions, malformed replies, failed edits, interventions,
  time and observable usage. Missing keys/quota produce named skips, never
  passes. Live runs are explicit, budgeted local or owned-machine jobs.
- **Depends on:** R05; claims about reconstruction/recovery wait for R06/R09.
  **Touches:** chapters 21–23 and evaluation fixtures.

#### R12 Compare routing with fixed engine choices

- [ ] **P1; finding F4.** Run eligible engines on the same tasks, compare
  automatic routing with fixed defaults, and keep policy-label accuracy as
  a separate metric. Include availability and consent fallbacks.
- **Acceptance:** publish paired outcomes, decision overhead and uncertainty;
  report regret against the best observed eligible engine without calling
  it a production guarantee. Test the Psionic choice when a qualified model
  is available; otherwise explicitly leave it unqualified. Select or
  calibrate thresholds on development data only.
- **Depends on:** R04, R11. **Touches:** chapters 12, 13, 17 and 21.

#### R13 Compare context strategies and editing tools

- [ ] **P2; findings F1, F2.** Compare bounded state, appended history and
  logged condensation with a fixed engine. Separately compare shell edits
  with one typed edit tool, and optionally add a token-budgeted repository
  outline. Change one mechanism at a time.
- **Acceptance:** report correctness, edit application errors, steps, input
  tokens, observable cache reuse and time. Include long sessions with a
  constraint needed after it would otherwise disappear. Retain a simple
  baseline; inconclusive results remain inconclusive.
- **Depends on:** R06, R11. **Touches:** chapters 3, 4, 7, 9, 21, 29 and 30.

### Phase 4 Earn additional complexity

#### R14 Measure Router concurrency and watchdog intervention

- [ ] **P2; findings F2, F4, F5.** Run chapter 17's proposed ablation over a
  frozen event stream, comparing one Worker with Router/Workers,
  pass-through and batching. Include overlapping follow-ups on the same
  repository. Evaluate the watchdog on held-out quiet and looping logs.
- **Acceptance:** report P50/P95 response and task latency, lost/duplicate
  replies, conflicting edits, decisions, usage and watchdog false positives.
  No speculative run publishes an unconfirmed effect. Keep channel replies
  owned by the Router and enforce grants regardless of classification.
- **Depends on:** R07, R09, R11. **Touches:** chapters 14, 17, 20 and 21.

#### R15 Join and evaluate the provenance gate

- [ ] **P2; finding F7.** Integrate chapter 6's gate with native tools,
  approvals and remote-result labels. Freeze held-out attack variants and
  benign tasks before running a named real model.
- **Acceptance:** report attempted calls, admitted effects, leaked text,
  answer corruption, completion and approval burden separately. A framing
  check is never presented as an LLM obedience guarantee. A blocked attack
  with a failed legitimate task is counted as such. Preserve hard grant and
  ceiling checks independently of model judgment.
- **Depends on:** R06, R07, R08, R11. **Touches:** chapters 4–6, 16, 18 and 21.

#### R16 Evaluate programs and candidate repair under equal budgets

- [ ] **P2; finding F2.** Compare single calls with bounded programs, then
  compare one candidate, selection and generate/test/repair. Use disposable
  worktrees for candidate code and separate visible feedback from final
  acceptance tests.
- **Acceptance:** account for generation, execution and checking in the same
  budget. Report compilation, any-correct, chosen-correct and chosen-wrong
  separately. Exercise approval suspension and continuation; do not promote
  the simulated best-of-N result into a real repair claim.
- **Depends on:** R07, R11. **Touches:** chapters 8, 21 and 22.

#### R17 Qualify protocols and interactive terminals through consumers

- [ ] **P2; findings F1, F9.** Drive ACP with a real named editor, MCP with
  independent clients, and the VT core with recorded streams checked against
  the planned libghostty oracle. Keep the simple command baseline separate
  from optional persistent terminals.
- **Acceptance:** record supported protocol versions and schema limits,
  cancellation, permissions, partial messages, reconnects, resize and UTF-8
  boundaries. Cover current agent terminal tools before adding persistent
  shell state. No visible browser or audible test is used.
- **Depends on:** R05, R07; persistent-session recovery also needs R09.
  **Touches:** chapters 16, 23, 33, 36 and 37.

#### R18 Consolidate the teaching sequence and evidence

- [ ] **P2; findings F9, F10.** Publish the two reading paths, the recurring
  repository task, concise chapter coverage summaries and links from each
  surveyed design to a decision or an open experiment. Clarify prototype
  Router ownership and remove repeated explanations that add no new detail.
- **Acceptance:** a reader can build and check the smallest Coder without
  first following every Psionic cross-reference; a deeper path retains the
  hand derivations. Documentation and registry checks pass, and results
  carry their revision and supported host/engine profile.
- **Depends on:** R02, R05, R11; incorporate later results as they qualify.
  **Touches:** all chapter introductions, README, SUMMARY and PLAN.

## Execution and completion policy

The [repository journey and coding pilot](repository-pilot.md) implements
the three initial priorities from this review: a shared per-engine contract
and corrected survey claims (R01–R04), the offline repository journey (R05),
and a real-task pilot comparing fixed configurations with routing (R11–R12).
See [the recorded results](../verification/coder-repository-2026-11-03/README.md).
These deliver a pilot, not every stronger runtime guarantee proposed by the
roadmap: complete native request recording, universal outbound gates and
broader statistical qualification remain open.

Start with R01–R05. They establish accurate language and a reviewable
workflow without requiring a new model-serving platform. R06–R10 close
specific recording, authority and recovery boundaries. R11 can begin once
the fixture exists; it need not wait for every later feature, provided its
receipts say which guarantees are qualified. R12–R17 then use that evidence
to choose optional mechanisms. R18 integrates the teaching improvements as
results become available.

Write checks before behavior changes. New tooling and product logic follow
the repository's Coax rules; changes to admission or policy use
`#authority` and zero surviving mutants. Published contracts start in
`proto/` and the protocol spec with a new version where required. Use
focused offline checks on ordinary changes, and explicit live measurements
for provider behavior. Split each command into work below the repository's
15-minute limit and never repeat an unchanged passing gate for reassurance.

Track proposed, implemented, qualified and rejected states separately.
Closing an implementation issue means its acceptance checks and chapter
updates are present. Closing a comparative experiment also requires a
recorded decision: retain, simplify, reject or gather more evidence. Lack of
statistical power is a valid reason to remain undecided. Safety and privacy
obligations do not depend on winning a task-success comparison.

No new owner action is required to commit this review. If later work needs
account quota, a paid model budget, a private transcript, publication rights
or a sign-off that is not already authorized, record that concrete
dependency in the workspace's `NEEDS_OWNER.md`. Synthetic fixtures and
captured sinks should let the offline work proceed independently.
