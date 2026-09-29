layout: title
id: title
title: Test-Time Capabilities
source: docs/essays/2026-09-29-test-time-capabilities.md
notes: Three parts, as the essay has them. Part I is the concept in general terms: no product names, none of our numbers. Part II is how OpenAgents builds it and what we measured. Part III is what we will measure next.
notes: Every number in the deck comes from a dated record in the repository; the source line of each slide names it.

---

layout: compare
id: compute-vs-capabilities
source: docs/essays/2026-09-29-test-time-capabilities.md#what-test-time-compute-is; docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time

column: Test-time compute
column: Test-time capabilities

row: What changes | How much the model computes | What the running agent can do
row: The weights | Fixed | Fixed
row: Where the gain comes from | Longer reasoning, more samples | A component admitted to the run
row: Who supplies it | The lab, at serving time | Anyone, and it can be shared
row: What proves it | A verifier picking the best sample | A claim others can rerun
row: What is left after | Nothing is kept | The capability, for every agent

notes: Test-time compute: think longer (chain of thought, o1, DeepSeek-R1), control the budget (s1: AIME24 from 50 to 57 percent), sample many and pick (SWE-bench Lite: 15.9 to 56 percent with 250 samples), allocate per question (compute-optimal beat best-of-N fourfold), adapt the weights briefly (test-time training on ARC). Wei 2022, OpenAI 2024, DeepSeek-AI 2025, Muennighoff 2025, Brown 2024, Snell 2024, Akyürek 2024.
notes: Both leave the weights alone. Compute spends more per answer and keeps nothing. A capability is admitted to the run, proven by a claim someone else can rerun, and stays: adopted once, every agent that shares the defaults has it.
notes: Two lessons carry over from the compute literature: a verifier is what makes extra effort pay, and the allocation is itself a judgment that should cost far less than the work it allocates.
---

layout: compare
id: protocol-find
title: The NIPs: from finding a capability to delegating
column: Carries
column: Why it matters
column: Status
source: docs/essays/2026-09-29-test-time-capabilities.md#the-nips-one-by-one; nips/openagents/README.md
row: EXT | releases | the exact version tested | Partial
row: CAP | grants | describing never grants use | Partial
row: KB | knowledge | tested with and without | Implemented
row: CJ | judgment, jobs | judgment and eval jobs on wire | Partial
row: PRG | decide, delegate | pinned, bounded steps | Partial
row: CTX | briefings | what a delegate was shown | Designed
row: SESS | delegate engines | how each engine is steered | Designed
row: WORK | tracked delegation | who answers for the work | Designed
notes: EXT keeps installing, enabling, granting, and admitting separate, so the lock the with arm held is exact. CAP keeps definition, host binding, grant, and presence apart, and a grant now records whether it was for evaluation or for real use. KB never counts a task an entry was written from as its evidence.
notes: CJ carries decision jobs and the router's judgment feedback; the hosted eval runner is an execution job. PRG's decide calls a pinned decision function, and delegate hands a bounded task to an admitted executor. The delegate door's briefing and failover run locally, with no Nostr record yet.
---

layout: compare
id: protocol-prove
title: The NIPs: from the run to the shared result
column: Carries
column: Why it matters
column: Status
source: docs/essays/2026-09-29-test-time-capabilities.md#the-nips-one-by-one; nips/openagents/README.md
row: RUN | run journal | lock, grant, baseline agent | Partial
row: ATIF | trajectories | each arm, step by step | Designed
row: EVAL | claims, checks | signed claims, whole scope | Partial
row: XP | credit | paid to confirm or dispute | Implemented
row: POL | cost, adoption | operator adopts; runs keep lock | Designed
row: OPT | optimization | faces the same test | Designed
row: MV | Verse | Gym notes cite the result | Partial
row: Contracts | locks | "the same tool", exactly | Partial
notes: RUN decides; trajectories only observe. ATIF traces stay local files today; no component publishes them yet. EVAL carries the subject and baseline arms, what the run relied on, the tool's identity strength, the task distribution, a verdict from a gate that declares its primary outcome, 3189 results, checks by a different trainer, and validations on a second suite read for independence by signer and chronology.
notes: XP: eval-check pays the checker, evaluator, and suite author whether the check confirms or disputes; eval-adopt needs a confirming check and a validation and pays the tool's author too; XP is never money. POL's route receipts would carry a judgment's time and cost. OPT's result reaches an agent only through EVAL admission and a new EXT release.
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
lead: Test-time capability, and the capability claim that states it
note: Key K = (A, B, D, S, E, G, M): subject, baseline agent, distribution, sampled suite, environment, grant, measurement. Records on K estimate Δ ± CI; a policy P reads them.
source: docs/glossary.md#test-time-capabilities; docs/essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time

An ability an agent gains, or loses, at inference time, without a weight update, because
something was admitted into the run. A component is a candidate; evidence makes a capability
claim: a key (subject, baseline agent, tasks, environment, grant, measurement), the records on it,
and a written policy that reads them. Reports are evidence; claims summarize; adoption is policy.

notes: A repository map, a written guide, a knowledge entry, another agent. The weights did not change; what the agent can do did.
notes: Nothing is a capability in general. The same tool can add twenty points to one agent and nothing to another; each claim belongs to its baseline and its tasks. Installed, described, or demoed makes no claim.
notes: It cuts both ways. On SkillsBench, 13 of 87 tasks got worse when the agent was given a skill. Restraint is a capability too.

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

notes: The last one is different in kind. Tools act, knowledge informs, delegates work; a typed judgment turns fuzzy evidence into a typed decision that ordinary code can build around. Ambiguous intent in, a probability out, a state machine after: a probabilistic branch instruction, not a chatbot.
notes: Choosing which of the other four to use is one application of it. An agent with fifty tools and no good way to choose is worse than one with none. The judge writes no text and grants no authority; code decides what its probabilities cause.

---

layout: compare
id: lexicon
kicker: PART I · THE IDEA
title: Words for the parts of a capability's life
column: Term
column: What it names
source: docs/essays/2026-09-29-test-time-capabilities.md#a-lexicon-of-test-time-capabilities
row: Admit | Admission | A locked component let into one run
row: Measure | Delta | With minus without, same tests, repeated
row: Use | Reach, restraint | Used where it helps, withheld where not
row: Allocate | Judgment budget | Deciding costs far less than answering
row: Reproduce | Reproduced | Someone else reran it: compatible result
row: Validate | Validated | Helps on tests its author didn't write
row: Adopt | Adoption | Joins the defaults, measured against them
row: Credit | Credit | For reruns either way, and for adoption

notes: Eleven terms in the essay; these are the ones that change what someone does. Test-time delegation and the capability flywheel are the other two, and adoption is not terminal: a claim reopens when its scope changes.
notes: Each term names something built or measured, and each has a way to be wrong. Reproduction proves the result repeats; validation proves it wasn't fitted to the author's own six tests.

---

layout: points
id: evals
kicker: PART I · THE IDEA
title: Capability claims are the unit of account
source: docs/essays/2026-09-29-test-time-capabilities.md#capability-claims-as-the-unit-of-account

- **Two arms, not one score.** A benchmark says how an agent did. A with-and-without result says what
  one component changed, which is the thing worth sharing.
- **A written rule gives the verdict.** The rule is a versioned file, and every result carries the
  digest of the rule that judged it. A replaced rule reinterprets old results; it reruns nothing.
- **Others can rerun it.** Published tests and exact component versions let someone else confirm or
  dispute the result.

notes: Why not a leaderboard: it rewards one system fitting one fixed task set. A per-component eval asks a narrower question with a clearer answer: does this help, where, and at what cost? Benchmarks ask how capable an agent is; a claim says what caused it.
notes: Once adoption and credit depend on evals, the evals are the network's objective function: people build what gets adopted. Independent test sets, held-out tasks, and paying for disputes keep the flywheel pointed at capability instead of at the tests.

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
notes: The third principle is a hypothesis to test. Under a correctness-primary rule, time and cost are notes on a result, never the verdict. A claim may name cost as its primary outcome with correctness held non-inferior; what it can never be is faster and wrong.

---

layout: points
id: compounding
kicker: PART I · THE IDEA
title: Capabilities can compound across a network
note: A hypothesis. Whether more participants make an agent measurably better has to be shown, measured the way the lexicon says.
source: docs/essays/2026-09-29-test-time-capabilities.md#how-capabilities-compound-across-a-network

- **More sources.** People bring the task families, libraries, and environments they know.
- **More verification.** Independent reruns, including disputes, are the verifier extra effort depends on.
- **Inheritance.** One validated result becomes a default for every agent, with no training run.
- **Credit that tracks use.** Recognition for verification work and adoptions, not for agreement.

notes: Weights improve when a lab trains them, on the lab's schedule. A test-time capability can come from anyone, be tested by anyone, and reach every agent once adopted.
notes: The unit that compounds is a capability claim with independent evidence: an exact version, a with-and-without result, reruns by people who didn't write it, and a delta that survives tests they wrote. Say the zero on the status slide.

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
notes: A turn answered at T0 costs a Jev call and no generation at all. Calibration is measured for one question on one partition: the route question alone, ECE 0.046 and Brier 0.083 on 151 development items. Say so if asked; the thresholds are bets we have checked once.

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
notes: The ext-eval-v2 gate reads Better only when more tests pass with the tool, the score gain clears the spread between repeats, and cost and time stay within 1.5 times plus the spread. It is an engineering gate, not the delta's definition; its own file says the spread of three repeats is a noisy estimate.
notes: Project map took 10.7 s with the tool against 24.9 s without it, and passed more tests. Time is a note, not the verdict. Reports written from now on record what the run relied on: runner, host, door, agent build, selector, graders.

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
step: make
step: run both
step: publish
step: check
step: validate
step: adopt
note: The hosted runner runs on coderos-4080 with a per-trainer daily quota. Suites are NIP-EXT releases; results, checks, and validations NIP-EVAL 3189 events; credit NIP-XP awards; adoption a coder-defaults release.
source: docs/essays/2026-09-29-test-time-capabilities.md#what-is-a-capability-means-in-our-system; docs/deployment/eval-runner.md; nips/openagents/NIP-EVAL.md; packages/coder-defaults/policy.md
notes: A tool becomes a candidate when its result is Better, three distinct trainers' checks confirmed it, and a Better result on a second test set validates it: written by someone other than the tool's author, released after the tool, on the same kind of task. Adoption is an operator decision, never automatic.
notes: Credit is XP and a name, never money. A check is paid whether it confirms or disputes; a good dispute is worth more than a fourth confirmation. eval-adopt credits the tool's author too.

---

layout: metrics
id: status
kicker: PART II · OUR IMPLEMENTATION
title: Where it stands today
metric: 3/3 | hosted results a second trainer confirmed
metric: 9 | XP awards signed from those checks
metric: 0 | results validated on a second test set
metric: 0 | tools adopted into the defaults
note: Every part of the loop is built and the first runs, checks, and awards are live. The policy now requires a validation before any adoption, and none exists: the tests have to come from someone else. The flywheel has not been shown turning.
source: docs/extensions/measurements/2026-09-29-hosted-runner-live.md; docs/essays/2026-09-29-test-time-capabilities.md; packages/coder-defaults/policy.md
notes: The XP referee on coderos-4080 signed the nine awards; any reader can recompute them with crates/xp-ledger. Both trainers' checks ran on the same runner, door, build, and graders: independent as signers, not as a platform.
notes: Say both zeros out loud. No validation yet means no adoption; no adoption yet means no evidence yet that the network compounds. The runner cannot validate its own catalog.

---

layout: points
id: next
kicker: PART III · WHAT WE'LL MEASURE NEXT
title: What we have not shown yet
source: docs/essays/2026-09-29-test-time-capabilities.md#part-iii-what-well-measure-next

- **The first validation,** then the first adoption, and whether it keeps its delta once it is a default.
- **Reach.** Describe tools so the router picks them: likely the cheapest gain available.
- **Deltas under a full grant.** The same tools against a Coder with a shell: a new claim, not a correction.
- **Cost and uncertainty.** Unpriced lanes stay blank; six tests show large changes only.
- **Marginal adoption.** Once the defaults hold anything, measure against them, not against nothing.
- **Revalidation.** Nothing reopens a claim yet when its baseline, grant, or model changes.
- **Network evidence:** marginal validated utility per adopted contribution, reported when zero.

notes: Also on the list: grader quality, checked the way results are, with the prose judge measured against the typed grader before either gets trusted; and compute and capability together, whether a tool lets a cheaper model with less reasoning match a stronger one.

---

layout: statement
id: close
kicker: THE CLAIM
source: docs/essays/2026-09-29-test-time-capabilities.md

Test-time compute asks how long a model should think. Test-time capabilities ask what it should be
able to use, and prove the answer with a test.

notes: Close here. The essay and every record behind the numbers are in the repository under docs/.
