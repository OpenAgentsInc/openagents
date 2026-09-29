layout: banner
id: title
title: Test-time
lead: Test-time compute, and the capabilities an agent gains while it runs
note: OpenAgents · 2026-09-29
source: docs/essays/2026-09-29-test-time-capabilities.md
notes: Three parts, as the essay has them. Part I is the concept in general terms: no product names, none of our numbers. Part II is how OpenAgents builds it and what we measured. Part III is what we will measure next.
notes: Every number in the deck comes from a dated record in the repository; the source line of each slide names it.

---

layout: points
id: test-time-compute
kicker: PART I · THE IDEA
title: Test-time compute: spend more when the model answers
source: docs/essays/2026-09-29-test-time-capabilities.md#what-test-time-compute-is

- **Think longer.** Chain of thought; o1 and DeepSeek-R1 learned long reasoning with reinforcement learning.
- **Control the budget.** s1's budget forcing raised AIME24 from 50 % to 57 %.
- **Sample many times and pick.** On SWE-bench Lite, one model went from 15.9 % with one sample to 56 %
  with 250.
- **Spend it where it helps.** Compute-optimal allocation beat best-of-N more than fourfold in efficiency.
- **Adapt the weights briefly.** Test-time training on ARC improved accuracy up to sixfold.

notes: Wei 2022, OpenAI 2024, DeepSeek-AI 2025, Muennighoff 2025, Brown 2024 (Large Language Monkeys), Snell 2024, Akyürek 2024.
notes: The point is not any one paper. A fixed set of weights answers better when the system around it spends more, and more wisely, per question.

---

layout: points
id: two-lessons
kicker: PART I · THE IDEA
title: Two lessons that carry past tokens
source: docs/essays/2026-09-29-test-time-capabilities.md#why-it-matters

1. **A verifier is what makes extra compute pay.** Sampling helps only as far as something can tell
   a right answer from a wrong one. Where the checker is weak, extra samples stop helping.
2. **Compute should be allocated per question.** Easy prompts do not need a long chain of thought.
   The allocation is itself a judgment, and it should cost far less than the work it allocates.

notes: Large Language Monkeys warns that without an automatic verifier, majority voting and reward models plateau after a few hundred samples.
notes: Hold on to both lessons. Evals are the verifier; a cheap typed judgment is the allocator.

---

layout: quote
id: definition
kicker: PART I · THE IDEA
lead: Test-time capability (TTCap), the term we propose
source: docs/glossary.md#test-time-capabilities; docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time

An ability an agent gains at inference time, without a weight update, by admitting something into
the run, and only if the same tests, run with it and without it, show the agent does measurably
better with it.

notes: A repository map, a written guide, a knowledge entry, another agent. The weights did not change; what the agent can do did.
notes: The last clause does the work. Having it installed, described, or demonstrated is not evidence; without the comparison it is a candidate.

---

layout: points
id: sources
kicker: PART I · THE IDEA
title: Five places a capability can come from
source: docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time

- **Tools and plugins.** Code with typed operations and bounded access to the host.
- **Skills.** A written guide the agent reads before a task.
- **Knowledge.** Cited entries, such as methods and known mistakes, retrieved for the task.
- **Delegation.** Another agent, briefed with selected evidence.
- **Typed judgment.** A fast, cheap decision that picks which of the others to use, and when.

notes: The last one makes the others usable. An agent with fifty tools and no good way to choose is worse than one with none.
notes: A typed judgment answers a yes-or-no, a choice, or a score with probabilities; ordinary code decides what they cause. The judge writes no text and grants no authority: Kahneman's System 1, with the slow work left to the generator.

---

layout: compare
id: lexicon
kicker: PART I · THE IDEA
title: Words for the parts of a capability's life
column: Term
column: What it names
source: docs/essays/2026-09-29-test-time-capabilities.md#a-lexicon-of-test-time-capabilities
row: Admit | Admission | A locked version of a component allowed into one run
row: Measure | Delta | With arm minus without arm, same tests, repeated
row: Use | Reach, restraint | Uses it when a test says to, leaves it alone otherwise
row: Allocate | Judgment budget | Deciding how to answer costs far less than answering
row: Verify | Verified | Someone else reran it and got the same verdict
row: Share | Adoption, credit | It joins everyone's defaults; its authors get credit

notes: Ten terms in the essay; these are the ones that change what someone does. Test-time delegation and the capability flywheel are the other two.
notes: Each term names something built or measured, and each has a way to be wrong.

---

layout: points
id: evals
kicker: PART I · THE IDEA
title: Evals are the unit of account
source: docs/essays/2026-09-29-test-time-capabilities.md#evals-as-the-unit-of-account

- **Two arms, not one score.** A benchmark says how an agent did. A with-and-without result says what
  one component changed, which is the thing worth sharing.
- **A written rule gives the verdict.** The rule is a versioned file, and every result carries the
  digest of the rule that judged it. Rules have bugs; old results keep their old digest.
- **Others can rerun it.** Published tests and exact component versions let someone else confirm or
  dispute the result.

notes: Why not a leaderboard: it rewards one system fitting one fixed task set. A per-component eval asks a narrower question with a clearer answer: does this help, where, and at what cost?
notes: Benchmarks still check the agent as a whole. Evals decide what goes into it. Graders are software too, and get the same scrutiny and versioning as the component under test.

---

layout: points
id: judgment-first
kicker: PART I · THE IDEA
title: Cheap judgment before expensive thinking
source: docs/essays/2026-09-29-test-time-capabilities.md#cheap-judgments-before-expensive-thinking

- **Decide first whether the turn needs a large model at all.** Offer a ladder at rising cost: a
  prepared answer, one a small model finishes, a grounded one, the full model, an agent with a computer.
- **Never wrong fast.** A prepared answer is served only when its readings clear thresholds tuned for
  precision. A fast answer to the wrong question is worse than a slow right one.
- **Capability can substitute for compute.** A tool that answers directly can beat a model that has
  to search for the answer, on time and on correctness.

notes: The literature allocates thinking per question. This allocates one level up, before any thinking.
notes: The third principle is a hypothesis to test. Time and cost are notes on a result, never the verdict: a faster wrong answer is not a capability.

---

layout: points
id: compounding
kicker: PART I · THE IDEA
title: Capabilities can compound across a network
note: A hypothesis. Whether more participants make an agent measurably better has to be shown, measured the way the lexicon says.
source: docs/essays/2026-09-29-test-time-capabilities.md#how-capabilities-compound-across-a-network

- **More sources.** People bring the task families, libraries, and environments they know.
- **More verification.** Checks by other people are the verifier extra effort depends on.
- **Inheritance.** One confirmed result becomes a default for every agent, with no training run.
- **Credit that tracks use.** Recognition for checks and adoptions keeps effort on tools that help.

notes: Weights improve when a lab trains them, on the lab's schedule. A test-time capability can come from anyone, be tested by anyone, and reach every agent once adopted.
notes: The unit that compounds is a reusable improvement with independent evidence: an exact component version, a with-and-without result, and confirming checks.

---

layout: flow
id: router
kicker: PART II · OUR IMPLEMENTATION
title: The chat router asks Jev before it asks a model
step: phone: Send
step: Jev judges
step: policy in code
step: tier T0 to T4
note: T0 a prepared answer · T1 a stem a cheap model finishes · T2 grounded in a knowledge base · T3 the full model · T4 an offer to run Coder. A slow or failed judge falls back to the model.
source: docs/coder/design/2026-09-28-chat-router.md
notes: Jev is TypeSafe's System One model. It answers typed questions (yes or no, a choice, a score) with probabilities; it writes no text and grants no authority. Code decides what the probabilities cause.
notes: One request asks route, prepared answer, needs specifics, risk, lane, and opener together.

---

layout: metrics
id: latency
kicker: PART II · OUR IMPLEMENTATION
title: A judgment costs a fraction of the answer it can skip
metric: 170 | ms: Jev judgment, median
metric: 700 | ms: prepared answer on screen, at most
metric: 5200 | ms: full model answer, at most
note: Measured from Send on the phone. Judgment p95 235 ms; about 0.3 s of each phone time is relay setup. Prepared answers: 36 of 36 correct (100 % precision) on 138 held-out messages.
source: docs/essays/2026-09-29-test-time-capabilities.md#our-numbers-judgments-before-thinking; docs/coder/measurements/2026-09-28-chat-router-eval.md; docs/coder/measurements/2026-09-28-first-reply.md
notes: Every number is in milliseconds so the three compare at a glance. Prepared answer 0.62 to 0.70 s (build 21). Full model 3.2 to 5.2 s complete, median 4.2 s to first words. A T1 finish measured 496 ms median at about $0.00005 a call.
notes: A turn answered at T0 costs a Jev call and no generation at all.

---

layout: points
id: coder
kicker: PART II · OUR IMPLEMENTATION
title: Five ways Coder acquires a capability at test time
note: One declared Terminal-Bench 4.0 attempt passed fin-saccr-rwa for $0.94 in 149.5 s, under Fable 5.1 low's best. It was in-sample and tuned; 2 of 13 attempts beat the bar.
source: docs/coder/runtime/delegate-door.md; docs/terminal-bench/2026-09-27-fable-delegate.md

- **Tools and plugins.** Wasm guests with typed operations and bounded host access.
- **Skills.** A `SKILL.md` guide the agent reads before a task.
- **Knowledge.** Cited entries, retrieved and filtered by Jev.
- **Delegation.** Microcoder through the first provider with capacity, with failover; Claude Code or
  Codex briefed with the evidence Jev chose.
- **Typed judgment.** Jev picks which of the others to use, and when.

notes: The delegate door tries a Codex login, then a Claude Code login, then our cloud fallback, and fails over on usage or rate limits.
notes: Delegation is a capability to measure, not a guaranteed win. That is why the note carries the 2 of 13.

---

layout: compare
id: with-without
kicker: PART II · OUR IMPLEMENTATION
title: Gym evals: the same six tests, with the tool and without it
column: With the tool
column: Without it
column: Verdict
note: Hosted runs, three per arm. The grant has no shell, so without the tool Coder cannot read the files at all: the delta is what each tool adds under that grant, not on top of a shell.
source: docs/essays/2026-09-29-test-time-capabilities.md#capability-delta-in-openagents; docs/extensions/measurements/2026-09-29-hosted-runner-live.md
row: Project map | 5 of 6 | 2 of 6 | Better
row: Code finder | 4 of 6 | 2 of 6 | Better
row: Test reader | 5 of 6 | 2 of 6 | Better
notes: The ext-eval-v2 gate reads Better only when more tests pass with the tool, the score gain clears the spread between repeats, and cost and time stay within 1.5 times plus the spread.
notes: Project map took 10.7 s with the tool against 24.9 s without it, and passed more tests. Time is a note, not the verdict.

---

layout: points
id: lessons
kicker: PART II · OUR IMPLEMENTATION
title: What the first runs taught us
source: docs/essays/2026-09-29-test-time-capabilities.md#our-evals-in-practice; docs/extensions/measurements/2026-09-29-hosted-runner-live.md

- **Restraint held.** Tests that should not use the tool passed as often with it, with one exception.
- **Reach did not.** Four tests failed in both arms: Jev did not pick the tool for that wording.
- **The first gate was wrong.** Under v1, faster but no more correct read Better. v2 replaced the
  rule the same day; the old result keeps its old digest.
- **Graders are software.** One looked for "not found" and missed another phrasing; we released a
  new test set version.

notes: The four reach failures: where-tests, known-bugs, workarounds, ci-failures. They are now the work list for each tool.
notes: The one restraint exception was a single run of Code finder's explain-idempotent test.

---

layout: flow
id: lifecycle
kicker: PART II · OUR IMPLEMENTATION
title: From a chat to every Coder
step: make a tool
step: run both arms
step: publish it
step: others check
step: adopt
note: The hosted runner runs on coderos-4080 with a per-trainer daily quota. Suites are NIP-EXT releases, results and checks NIP-EVAL 3189 events, credit NIP-XP awards; adoption is a coder-defaults release.
source: docs/essays/2026-09-29-test-time-capabilities.md#what-is-a-capability-means-in-our-system; docs/deployment/eval-runner.md; nips/openagents/NIP-EVAL.md
notes: A tool becomes a candidate when its result is Better and three distinct trainers' checks confirmed it. Adoption is an operator decision, never automatic.
notes: Credit is XP and a name, never money. eval-check credits the checker, the evaluator, and the suite author; eval-adopt credits the tool's author too.

---

layout: compare
id: protocol-find
kicker: PART II · OUR IMPLEMENTATION
title: The protocol: which NIP carries what, from finding a tool to delegating
column: Carries
column: Why it matters
column: Status
note: Status is each whole NIP's, from the implementation coverage report.
source: docs/essays/2026-09-29-test-time-capabilities.md#the-nips-one-by-one; nips/openagents/README.md
row: EXT | releases | a result names the exact tool version it tested | Partial
row: CAP | grants | describing a tool never grants its use | Partial
row: KB | knowledge | an entry is tested with and without, like a tool | Implemented
row: CJ | judgment, jobs | the cheap judgment and the eval jobs are on the wire | Partial
row: PRG | decide, delegate | judgments and hand-offs are pinned, bounded steps | Partial
row: CTX | briefings | says which evidence a delegate was given | Designed
row: SESS | delegate engines | states how each engine can be steered | Designed
row: WORK | tracked delegation | says who answers for delegated work | Designed
notes: EXT keeps installing, enabling, granting, and admitting separate, so the lock the with arm held is exact. CAP keeps definition, host binding, grant, and presence apart. KB never counts a task an entry was written from as its evidence.
notes: CJ carries decision jobs and the router's judgment feedback; the hosted eval runner is an execution job. PRG's decide calls a pinned decision function, and delegate hands a bounded task to an admitted executor. The delegate door's briefing and failover run locally, with no Nostr record yet.

---

layout: compare
id: protocol-prove
kicker: PART II · OUR IMPLEMENTATION
title: The protocol: which NIP carries what, from the run to sharing the result
column: Carries
column: Why it matters
column: Status
note: The shared contracts sit under every row: exact references, locks, the private envelope.
source: docs/essays/2026-09-29-test-time-capabilities.md#the-nips-one-by-one; nips/openagents/README.md
row: RUN | the run journal | records the lock a capability was admitted under | Partial
row: ATIF | trajectories | each arm and each delegate, step by step | Designed
row: EVAL | deltas, checks | without it, a delta is an unsigned claim | Partial
row: XP | credit | verified work earns credit anyone can recompute | Implemented
row: POL | cost, adoption | adoption is an operator's call; runs keep their lock | Designed
row: OPT | optimization | an optimized candidate faces the same test | Designed
row: MV | Verse | Gym notes cite the trainer's published result | Partial
row: Contracts | locks | "the same tool" and "the same tests", stated exactly | Partial
notes: RUN decides; trajectories only observe. ATIF traces stay local files today; no component publishes them yet. EVAL carries the subject and baseline arms, a verdict from a pinned gate, 3189 results, and checks by a different trainer.
notes: XP: eval-check pays the checker, evaluator, and suite author; eval-adopt pays the tool's author too; XP is never money. POL's route receipts would carry a judgment's time and cost. OPT's result reaches an agent only through EVAL admission and a new EXT release.

---

layout: metrics
id: status
kicker: PART II · OUR IMPLEMENTATION
title: Where it stands today
metric: 3/3 | hosted results a second trainer confirmed
metric: 9 | XP awards signed from those checks
metric: 0 | tools adopted into the defaults
note: Every part of the loop is built and the first runs, checks, and awards are live. The flywheel has not been shown turning.
source: docs/extensions/measurements/2026-09-29-hosted-runner-live.md; docs/essays/2026-09-29-test-time-capabilities.md
notes: The XP referee on coderos-4080 signed the nine awards; any reader can recompute them with crates/xp-ledger.
notes: Say the zero out loud. No adoption yet means no evidence yet that the network compounds.

---

layout: points
id: next
kicker: PART III · WHAT WE'LL MEASURE NEXT
title: What we have not shown yet
source: docs/essays/2026-09-29-test-time-capabilities.md#part-iii-what-well-measure-next

- **The first adoption,** and whether it keeps its delta on tests its author did not write.
- **Reach.** Describe tools so the router picks them: likely the cheapest gain available.
- **Deltas under a full grant.** The same tools against a Coder that can already run commands.
- **Cost for both arms.** Until gateway lanes are priced, cost stays a blank, not a guess.
- **Power and interactions.** Six tests show large changes only; adoption needs a check of the whole set.
- **Network evidence:** verified out-of-sample passes per adopted contribution, reported when zero.

notes: Also on the list: grader quality, checked the way results are; and compute and capability together, whether a tool lets a cheaper model with less reasoning match a stronger one.

---

layout: statement
id: close
kicker: THE CLAIM
source: docs/essays/2026-09-29-test-time-capabilities.md

Test-time compute asks how long a model should think. Test-time capabilities ask what it should be
able to use, and prove the answer with a test.

notes: Close here. The essay and every record behind the numbers are in the repository under docs/.
