# Test-time compute, and the capabilities we can add at test time

Essay, 2026-09-29. It states a thesis and proposes vocabulary. Every claim
about our system links the code or the dated record behind it. Where
something is a plan or a hypothesis, the text says so.

## TL;DR

- **Test-time compute** means spending more computation when a model
  answers, not when it's trained: longer reasoning, many samples with a
  checker picking the best, or search. The published work is clear that
  this helps, often a lot.
- **Our claim goes one step further.** An agent can also *gain abilities*
  while it runs, without anyone retraining its weights: we admit a tool, a
  plugin, a skill, a knowledge entry, or another agent into the run. We call
  what it gains a **test-time capability**.
- **A capability only counts if a test shows it.** We run the same tests
  with the tool and without it, and a fixed rule gives the verdict. Other
  people rerun those tests to confirm the result. That record, not a claim
  in a README, is what makes something a capability.
- **Cheap judgments come before expensive thinking.** A typed judgment from
  Jev takes about 0.15 to 0.26 seconds. It decides whether a turn needs a
  full model at all. A prepared answer reaches the phone in 0.62 to 0.70
  seconds; a full model answer takes 3 to 5 seconds.
- **Proven capabilities can be shared.** When a tool's result is confirmed
  by other trainers, it becomes a candidate for Coder's defaults, and every
  Coder would then start with it. That is how we intend network effects to
  make Coder better. It is a hypothesis we are measuring, not a result yet.
- **The words we propose** are in [a short lexicon](#a-lexicon-of-test-time-capabilities)
  below, and in the [glossary](../glossary.md#test-time-capabilities).

## Contents

- [What test-time compute is](#what-test-time-compute-is)
- [Why it matters](#why-it-matters)
- [The thesis: capability is something you can acquire at test time](#the-thesis-capability-is-something-you-can-acquire-at-test-time)
- [A lexicon of test-time capabilities](#a-lexicon-of-test-time-capabilities)
- [Evals are the unit of account](#evals-are-the-unit-of-account)
- [The economics: cheap judgments before expensive thinking](#the-economics-cheap-judgments-before-expensive-thinking)
- [The collective: how capabilities compound](#the-collective-how-capabilities-compound)
- [Open problems and what we measure next](#open-problems-and-what-we-measure-next)
- [References](#references)

## What test-time compute is

A language model's quality used to be discussed almost entirely in terms of
training: more parameters, more data, more training compute. *Test-time
compute* is the other axis: the computation spent when the model is asked a
question. The literature has converged on a few families.

- **Think longer.** Chain-of-thought prompting showed that letting a model
  write out intermediate steps improves multi-step reasoning
  ([Wei et al., 2022](https://arxiv.org/abs/2201.11903)). OpenAI's o1 was
  trained with reinforcement learning to use a long chain of thought, and
  OpenAI reported that its performance improves with both more
  reinforcement learning and more time spent thinking
  ([OpenAI, 2024](https://openai.com/index/learning-to-reason-with-llms/)).
  DeepSeek-R1 showed that reasoning behavior of this kind can be
  incentivized with reinforcement learning, and released open weights
  ([DeepSeek-AI, 2025](https://arxiv.org/abs/2501.12948)).
- **Control the budget.** s1 fine-tuned a 32B model on 1,000 curated
  questions and added *budget forcing*: ending the model's thinking at a
  limit, or appending "Wait" when it tries to stop early so it keeps
  checking. The authors report that forcing more thinking raised AIME24
  from 50 % to 57 % ([Muennighoff et al., 2025](https://arxiv.org/abs/2501.19393)).
- **Sample many times and pick.** Self-consistency samples several
  reasoning paths and takes the majority answer
  ([Wang et al., 2022](https://arxiv.org/abs/2203.11171)). Training a
  verifier and choosing the highest-ranked of many candidates goes back at
  least to GSM8K ([Cobbe et al., 2021](https://arxiv.org/abs/2110.14168)).
  *Large Language Monkeys* found that the fraction of problems solved by
  *any* sample keeps growing with the number of samples over four orders of
  magnitude; on SWE-bench Lite, one model went from 15.9 % with one sample
  to 56 % with 250. The same paper warns that without an automatic
  verifier, majority voting and reward models plateau after a few hundred
  samples ([Brown et al., 2024](https://arxiv.org/abs/2407.21787)).
- **Spend it where it helps.** Snell et al. showed that the best way to
  spend test-time compute depends on how hard the prompt is. A
  compute-optimal strategy improved efficiency more than fourfold over a
  best-of-N baseline, and on problems where a smaller model has some
  success, test-time compute could beat a model 14 times larger at matched
  compute ([Snell et al., 2024](https://arxiv.org/abs/2408.03314)).
- **Adapt the weights briefly.** Test-time training updates a model's
  parameters on the test input itself before answering
  ([Sun et al., 2020](https://arxiv.org/abs/1909.13231)). Applied to ARC,
  it improved accuracy up to sixfold over the base fine-tuned model
  ([Akyürek et al., 2024](https://arxiv.org/abs/2411.07279)).

## Why it matters

Test-time compute changes what "a better model" means. A fixed set of
weights can answer better if the system around it spends more, and more
wisely, per question. Two lessons from that work shape everything below.

1. **A verifier is what makes extra compute pay.** Sampling helps only as far
   as something can tell a right answer from a wrong one. Where the checker
   is weak, extra samples stop helping.
2. **Compute should be allocated per question.** Easy prompts don't need a
   long chain of thought; hard ones do. The allocation decision is itself a
   judgment, and it should cost far less than the work it allocates.

Both lessons generalize past tokens. Most of the work in this repository is
about the *system* around a model: what it's allowed to use, who decides
what to use, and how we know it helped.

## The thesis: capability is something you can acquire at test time

The test-time compute literature mostly asks how to get more out of one
model by letting it think more. We ask a second question: **what can an
agent become able to do, at the moment it runs, without anyone retraining
it?**

A coding agent that can't see a repository can't answer "what's the largest
file here?" no matter how long it thinks. Give it a tool that maps the
repository, and it can. The weights didn't change; the agent's capability
did. The same is true of a checked-in guide it reads before a task, a
knowledge entry about a recurring mistake, or a stronger agent it hands a
well-prepared briefing to. Tool use as a learned behavior is well studied
([Schick et al., 2023](https://arxiv.org/abs/2302.04761)), as are skill
libraries an agent grows as it works
([Wang et al., 2023](https://arxiv.org/abs/2305.16291)) and retrieval that
brings documents into generation
([Lewis et al., 2020](https://arxiv.org/abs/2005.11401)). What we add is a
discipline for treating each of these as a measured, shareable unit.

We define a **test-time capability** as an ability an agent gains at
inference time, without a weight update, by admitting something into the
run, and whose effect is shown by a test with and without it. We have five
ways to acquire one, all built:

| Source | What gets admitted | Where it lives |
| --- | --- | --- |
| Tools and plugins | A Wasm guest with typed operations and bounded host access, or a program that runs one | [Wasm plugins](../extensions/plugins.md), `crates/plugin` |
| Skills | A `SKILL.md` guide the agent reads before a task | [Plugins and skills](../glossary.md#plugins-and-skills) |
| Knowledge | Cited entries (methods, edge cases, slips) retrieved and filtered by Jev | [Knowledge base](../coder/design/knowledge-base.md) |
| Delegation | Another agent, briefed with evidence Jev chose | [The delegate door](../coder/runtime/delegate-door.md) |
| Typed judgment | A System One answer that picks which of the above to use, and when | [The chat router](../coder/design/2026-09-28-chat-router.md) |

The last row is the one that makes the others usable. An agent with fifty
tools and no good way to decide which to use is worse than an agent with
none. [Jev](../glossary.md#decision-models-and-runtimes), TypeSafe's System
One model, answers typed questions (yes or no, a choice among options, an
ordered score) with probabilities, and code decides what those
probabilities cause. The model writes no text and grants no authority. The
name borrows the fast, automatic "System 1" of Kahneman's *Thinking, Fast
and Slow* (2011); the slow, effortful work is left to the generator.

## A lexicon of test-time capabilities

We propose ten terms. Each names something we already build or measure,
and each has a way to be wrong. We tried to keep the list short; a term
earns a place only if it changes what someone does.

### 1. Test-time capability (TTCap)

**Definition:** an ability an agent gains or loses at inference time,
without updating weights, because a component was admitted to the run, and
whose effect is shown by a test with the component and without it.

**How we build it:** an extension (tool, plugin, skill, or package)
admitted to a Coder turn; a knowledge entry retrieved into a Microcoder
step; a delegate briefed by Coder One.

**How it's measured:** an [extension eval](../extensions/evaluation.md). A
component that has no test yet is a *candidate* capability, not a
capability.

### 2. Capability admission

**Definition:** the host's decision that a specific, locked version of a
component may take part in a run.

**How we build it:** discovery, installation, enablement, grants, and
admission are separate decisions in our
[extension architecture](../extensions/architecture.md). An eval run holds
the exact extension and Coder's question sets in its run locks. Installing
a package grants nothing.

**How it's measured:** the lock digest recorded in every report, so a
result names exactly which bytes it measured.

### 3. Capability delta

**Definition:** the difference in outcome between the *with* arm (the
component admitted) and the *without* arm (nothing admitted), on the same
tests, repeated enough times to see the spread.

**How we build it:** `openagents ext eval` and the hosted runner run every
test in both arms as confined `coder -p` turns with an ATIF trajectory
each. The [`ext-eval-v2` gate](../../crates/gym/gates/ext-eval-v2.json)
reads **Better** only when more tests pass with the tool, the score gain
clears the spread between repeats, and cost and time stay within 1.5 times
plus the spread.

**How it's measured:** our first hosted runs, three runs per arm, six tests
per tool
([record](../extensions/measurements/2026-09-29-hosted-runner-live.md)):

| Tool | With the tool | Without it | Verdict |
| --- | --- | --- | --- |
| Project map | 5 of 6 | 2 of 6 | **Better** |
| Code finder | 4 of 6 | 2 of 6 | **Better** |
| Test reader | 5 of 6 | 2 of 6 | **Better** |

Read the delta at its stated scope. In those hosted runs the grant has no
shell, so without the tool Coder can't look at the files at all. The delta
measures what the tool adds *under that grant*, not what it adds to a Coder
that already has a shell.

### 4. Reach and restraint

**Definition:** *reach* is how often the agent actually uses a capability
when a test says it should; *restraint* is how often it leaves the
capability alone when a test says it shouldn't.

**How we build it:** every suite marks each test should-fire or
should-not-fire. Each starter suite has four of the first kind and two of
the second.

**How it's measured:** per test, in the same record. Restraint held: every
should-not-fire test passed with the tool as often as without it, except
one run of Code finder's `explain-idempotent`. Reach did not always hold:
`where-tests`, `known-bugs`, `workarounds`, and `ci-failures` failed in both
arms because Jev didn't choose the tool's program for that wording. A
capability the router doesn't reach for is a capability the agent doesn't
have. Those tests are now the work list for each tool.

### 5. Judgment budget

**Definition:** the time and money a system spends deciding *how* to answer
before it spends anything on answering. The rule is that the judgment must
be much cheaper than the work it can avoid.

**How we build it:** the [chat router](../coder/design/2026-09-28-chat-router.md)
asks Jev one request of independent questions (route, prepared answer,
whether the reply needs specifics, risk, lane, opener) and a policy table in
code chooses a tier: T0 a whole prepared answer, T1 a prepared stem finished
by a cheap model, T2 an answer grounded in a knowledge base, T3 the full
model, T4 a dispatch or command offer. A slow or failed judge falls back to
the model's reply, so the judgment can only save time.

**How it's measured:** judgment latency (170 ms median, 235 ms p95 on the
held-out set) and the precision of what it serves (100 % canned precision,
36 of 36, on 138 held-out messages)
([router evaluation](../coder/measurements/2026-09-28-chat-router-eval.md)).

### 6. Test-time delegation

**Definition:** acquiring another agent's capability for one task by
handing it a prepared briefing, then recording what it did as part of the
same task.

**How we build it:** Coder's [delegate door](../coder/runtime/delegate-door.md)
runs Microcoder through the first connected provider with capacity (a Codex
login, then a Claude Code login, then our cloud fallback) and fails over on
usage or rate limits. When Microcoder can't run, Claude Code or Codex CLI
takes the turn, briefed with what Jev chose from the workspace. OpenCode and
Devin routes run as [delegate sessions](../glossary.md#the-openagents-app-and-chat)
copied into the task's history.

**How it's measured:** the same outcome, cost, and time as any attempt. In
one declared Terminal-Bench 4.0 attempt, Coder One with Jev's briefing
passed `fin-saccr-rwa` for $0.9429 in 149.5 s, below Fable 5.1 low's cheapest
($1.2246) and fastest (222.5 s) wins. That attempt was in-sample and tuned,
and across 7 series only 2 of 13 attempts beat the bar
([record](../terminal-bench/2026-09-27-fable-delegate.md)). Delegation is a
capability to measure, not a guaranteed win.

### 7. Verified capability

**Definition:** a capability whose **Better** verdict was reproduced by
someone other than the person who first ran it: a different trainer, the
same test set release, the same tool release.

**How we build it:** an [eval check](../extensions/evaluation.md#checks-adoption-and-credit)
reruns a published suite and publishes its own NIP-EVAL `3189` that cites
the original. It confirms on a matching verdict and disputes otherwise;
both stay visible.

**How it's measured:** in the hosted record, a second trainer checked each
of the three results and all three confirmed.

### 8. Capability adoption

**Definition:** making a verified capability part of the agent everyone
starts with.

**How we build it:** an operator issues an `openagents.eval-admission.v1`
decision citing the reports and publishes a release of the
[`openagents:coder-defaults`](../../packages/coder-defaults/) package that
depends on the tool. A tool becomes a candidate when its result is
**Better** and at least three distinct trainers' checks confirmed it.
Adoption is an operator decision, never automatic.

**How it's measured:** the tool exists; no adoption has been made yet. The
first adoption, and whether the adopted tool keeps its delta on new tests,
is the next result to report.

### 9. Capability credit

**Definition:** recognition that goes to the people whose work made a
capability real, and only for the events that show it was used.

**How we build it:** two [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check)
rules. `eval-check` credits the checker, the original evaluator, and the
suite's author when a check confirms a result. `eval-adopt` credits the
tool's author, the suite's author, and the evaluators of cited results when
Coder adopts the tool. A run, a publish, a view, or a download earns
nothing. Credit is XP and your name. It is never money, and no payout
exists.

**How it's measured:** the XP referee on `coderos-4080` signed the first
nine awards from the hosted checks, and any reader can recompute them with
`crates/xp-ledger`.

### 10. The capability flywheel

**Definition:** the loop in which people add capabilities, tests prove
them, others confirm them, and adoption hands them to every agent, which
then takes on harder tasks that reveal the next missing capability.

**How we build it:** the chat is the front door. A person asks OpenAgents
what to test or makes a tool by chatting, runs the tests on our computers,
adds the result to the Gym, and earns credit when others check it or Coder
adopts it. The [README's loop](../../README.md#the-loop) draws it.

**How it's measured:** the network plan's test, not participant counts:
*incremental out-of-sample verified passes per adopted contribution*, with
cost, latency, and harmful regressions
([how the network compounds](../coder/design/networked-coder-plan.md#how-the-network-compounds)).
We have not shown the flywheel turning yet; we have built each part of it.

## Evals are the unit of account

If capabilities are acquired and shared, something has to say which ones
are real. For us that is the extension eval, and three of its properties do
most of the work.

- **Two arms, not one score.** A benchmark score tells you how an agent did.
  A with-and-without result tells you what one component changed. That is
  the thing you'd want to share, adopt, or pay attention to.
- **A written rule gives the verdict.** The gate is a versioned file, and
  every report carries the digest of the gate that judged it. The gate
  already taught us something: under `ext-eval-v1`, a tool that made Coder
  *faster* but no more correct read **Better**. We found that in a
  [live run](../extensions/measurements/2026-09-29-ext-eval-runner-live.md)
  where both arms passed the same tests, and replaced the rule the same day:
  under `ext-eval-v2`, faster or cheaper alone is **No clear change** with a
  note. The old result keeps its old digest and stays readable.
- **Others can rerun it.** A published suite is a NIP-EXT release, a
  published result carries its trainer's signed request, and a check
  verifies every file against the release before rerunning it. Confidence
  comes from reproduction by someone else, not from the author's report.

Why this beats chasing a leaderboard: a leaderboard rewards one system on
one fixed task set, and it rewards fitting that set. We've been careful
about this in our own [Terminal-Bench](../terminal-bench/README.md) work,
which separates in-sample development wins from out-of-sample results (for
example, 30 confirmed out-of-sample wins on 65 TB2.1 tasks, measured on
cost against Fable 5 xhigh, on an older and easier benchmark than TB4). A
per-component eval asks a narrower question with a clearer answer: *does
this thing help, where, and at what cost?* Benchmarks remain how we check
Coder as a whole; evals are how we decide what goes into it.

Evals also have failure modes, and we've hit them. A grader looked for "not
found" and missed "the server cannot find the requested resource", turning
one run's result to **Worse** by chance. We fixed the pattern and released a
new test set version; the old one stays readable. An eval is a piece of
software and gets the same scrutiny.

## The economics: cheap judgments before expensive thinking

Test-time compute costs money and time. The literature's answer is to
allocate it per question. Ours is the same idea one level up: before
allocating *thinking*, decide whether the turn needs a model at all, and
which capability should handle it.

These are the numbers from our chat, measured from Send on the phone:

| What happens | Time | Source |
| --- | --- | --- |
| Jev judgment (all router questions, one request) | 170 ms median, 235 ms p95 | [router evaluation](../coder/measurements/2026-09-28-chat-router-eval.md) |
| Prepared answer on screen (T0) | 0.62 to 0.70 s | [build 21 verification](../extensions/measurements/2026-09-29-build-21-verification.md) |
| Opener before a model answer (T3) | 0.60 to 0.75 s | [first-reply measurement](../coder/measurements/2026-09-28-first-reply.md) |
| Full model answer, first words | median 4.2 s | [chat worker](../deployment/chat-worker.md) |
| Full model answer, complete | 3.2 to 5.2 s | [first-reply measurement](../coder/measurements/2026-09-28-first-reply.md) |

About 0.3 s of every phone number is the relay setup for a fresh
connection; the design's budget for a prepared answer with a kept
connection is 0.4 s. A T1 personalization call, a cheap model finishing a
prepared sentence, measured 496 ms at the median and costs about $0.00005 a
call
([measurement](../coder/design/2026-09-28-chat-router.md#implemented-and-measured-2026-09-28)).
A turn answered at T0 costs a Jev call and no generation at all.

Three principles follow.

1. **Never wrong fast.** A prepared answer is served only when the route,
   the answer, and the "needs specifics" readings all clear thresholds tuned
   for precision. A fast answer to the wrong question is worse than a slow
   right one, so the canned tier is gated on measured precision, not on
   confidence alone.
2. **Spend the big model where it adds something.** Identity questions,
   small talk, and product questions with reviewed answers don't need it.
   Requests for work need a computer, and get a one-tap offer to run Coder
   instead of a model's guess at doing the work in chat.
3. **Capability can substitute for compute.** In the hosted runs, a Project
   map run took 10.7 s with the tool against 24.9 s without it, and passed
   more tests. A tool that answers directly can beat a model that must
   search for the answer, on both time and correctness. We report time and
   cost as notes, never as the verdict, because a faster wrong answer isn't
   a capability.

We don't yet price every lane: Coder doesn't price gateway lanes, so the
eval records list cost as unknown. That gap is on the list below.

## The collective: how capabilities compound

We are building the best coding agent in the world by using network
effects: an agent collective. Coder is the first agent. The Gym is where
people help agents get better, through the plugin system and the evals
that measure it. Verse is where agents and people meet, and where the Gym's
results and evals boards live.

Test-time capabilities are what makes that compounding possible in
principle. Weights improve when a lab trains them, on the lab's schedule. A
test-time capability can come from anyone, be tested by anyone, and, once
adopted, reach every Coder without a training run. The unit that compounds
is not a longer prompt or a count of packages. It is a **reusable
improvement with independent evidence**: an exact component version, a
with-and-without result, and confirming checks by people who didn't write
it.

What the network adds, concretely:

- **More sources of capability.** Different people bring different task
  families, libraries, and environments, and write tools and tests for the
  work they know.
- **More verification.** Checks by other trainers turn one person's claim
  into a reproduced result. That is the verifier the test-time compute
  literature says extra effort depends on, supplied by people instead of a
  reward model.
- **Inheritance.** Adoption into `coder-defaults` turns one confirmed
  result into a default for everyone. Traces in
  [ATIF v1.8](../coder/runtime/traces.md) make each run inspectable, and
  NIP-ATIF, still designed and not yet published by any component, is how
  they're meant to travel.
- **Credit that tracks use.** XP goes to checks and adoptions, the two
  events that show someone else's work was used. That keeps the incentive
  on tools that help rather than tools that exist.

This section describes a design and a hypothesis. The parts are built and
the first runs, checks, and awards are live. Whether adding participants
makes Coder measurably better is the claim we still have to earn, measured
the way the lexicon says.

## Open problems and what we measure next

- **The first adoption.** No tool has been adopted into `coder-defaults`.
  After the first, we need to show it keeps its delta on tests its author
  didn't write.
- **Reach.** Four should-fire tests failed in both arms because Jev didn't
  choose the tool for that wording. Improving how capabilities are described
  to the router, so it reaches for them, is likely the cheapest gain
  available.
- **Deltas under a full grant.** The hosted starter results are measured
  without a shell. We need the same tools measured against a Coder that can
  already run commands, where the baseline is much stronger.
- **Interaction effects.** Two capabilities that each help alone can
  interfere together. Adoption needs a regression check across the whole
  default set, not only a check of the newcomer.
- **Cost.** Every eval record should carry a price for both arms. Until
  Coder prices gateway lanes, cost is a blank we don't fill with guesses.
- **Statistical power.** Six tests and three runs per arm are enough to see
  a large change and too few to see a small one. Suites need to grow as the
  deltas we care about shrink.
- **Grader quality.** Graders are code and make mistakes, as the "not found"
  case showed. We want graders checked the way results are.
- **Compute and capability together.** We haven't yet measured how
  test-time capabilities interact with more thinking: whether a tool lets a
  cheaper model with less reasoning match a stronger one, and when extra
  reasoning still pays on top of a tool.
- **Network evidence.** The flywheel's test is incremental out-of-sample
  verified passes per adopted contribution. We will report it, including
  when it's zero.

## References

- Akyürek, E. et al. (2024). *The Surprising Effectiveness of Test-Time
  Training for Abstract Reasoning.* [arXiv:2411.07279](https://arxiv.org/abs/2411.07279)
- Brown, B. et al. (2024). *Large Language Monkeys: Scaling Inference
  Compute with Repeated Sampling.* [arXiv:2407.21787](https://arxiv.org/abs/2407.21787)
- Cobbe, K. et al. (2021). *Training Verifiers to Solve Math Word Problems.*
  [arXiv:2110.14168](https://arxiv.org/abs/2110.14168)
- DeepSeek-AI (2025). *DeepSeek-R1: Incentivizing Reasoning Capability in
  LLMs via Reinforcement Learning.* [arXiv:2501.12948](https://arxiv.org/abs/2501.12948)
- Kahneman, D. (2011). *Thinking, Fast and Slow.* Farrar, Straus and Giroux.
- Lewis, P. et al. (2020). *Retrieval-Augmented Generation for
  Knowledge-Intensive NLP Tasks.* [arXiv:2005.11401](https://arxiv.org/abs/2005.11401)
- Muennighoff, N. et al. (2025). *s1: Simple test-time scaling.*
  [arXiv:2501.19393](https://arxiv.org/abs/2501.19393)
- OpenAI (2024). *Learning to reason with LLMs.*
  [openai.com](https://openai.com/index/learning-to-reason-with-llms/)
- Schick, T. et al. (2023). *Toolformer: Language Models Can Teach
  Themselves to Use Tools.* [arXiv:2302.04761](https://arxiv.org/abs/2302.04761)
- Snell, C., Lee, J., Xu, K., and Kumar, A. (2024). *Scaling LLM Test-Time
  Compute Optimally can be More Effective than Scaling Model Parameters.*
  [arXiv:2408.03314](https://arxiv.org/abs/2408.03314)
- Sun, Y. et al. (2020). *Test-Time Training with Self-Supervision for
  Generalization under Distribution Shifts.* [arXiv:1909.13231](https://arxiv.org/abs/1909.13231)
- Wang, G. et al. (2023). *Voyager: An Open-Ended Embodied Agent with Large
  Language Models.* [arXiv:2305.16291](https://arxiv.org/abs/2305.16291)
- Wang, X. et al. (2022). *Self-Consistency Improves Chain of Thought
  Reasoning in Language Models.* [arXiv:2203.11171](https://arxiv.org/abs/2203.11171)
- Wei, J. et al. (2022). *Chain-of-Thought Prompting Elicits Reasoning in
  Large Language Models.* [arXiv:2201.11903](https://arxiv.org/abs/2201.11903)
