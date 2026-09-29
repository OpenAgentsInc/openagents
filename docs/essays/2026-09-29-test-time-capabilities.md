# Test-time compute, and the capabilities we can add at test time

Essay, 2026-09-29. It states a thesis and proposes vocabulary. It has three
parts. [Part I](#part-i-the-concept) states the concept in general terms,
for anyone building agents; it names no product of ours. [Part II](#part-ii-our-implementation)
says how OpenAgents implements it and what we've measured; every claim
there links the code or the dated record behind it. [Part III](#part-iii-what-well-measure-next)
lists our own open problems. Where something is a plan or a hypothesis, the
text says so.

## TL;DR

- **Test-time compute** means spending more computation when a model
  answers, not when it's trained: longer reasoning, many samples with a
  checker picking the best, or search. The published work is clear that
  this helps, often a lot.
- **Test-time capabilities go one step further.** An agent can also *gain
  abilities* while it runs, without anyone retraining its weights, when a
  tool, a plugin, a skill, a knowledge entry, or another agent is admitted
  into the run.
- **Something is a test-time capability only if a controlled comparison
  shows it.** The same tests are run with and without it, and the agent
  does measurably better with it. Having it installed, described, or
  demonstrated is not evidence.
- **Cheap judgments should come before expensive thinking,** and results
  others have reproduced can be shared, so that capabilities compound across
  a network of people and agents. Both are stated as hypotheses to measure.
- **We built an implementation** in OpenAgents, with first measurements and
  no adoption yet; Part II has it, term by term. The words are also in the
  [glossary](../glossary.md#test-time-capabilities).

## Contents

- [Part I: The concept](#part-i-the-concept)
  - [What test-time compute is](#what-test-time-compute-is)
  - [Why it matters](#why-it-matters)
  - [The thesis: capability is something you can acquire at test time](#the-thesis-capability-is-something-you-can-acquire-at-test-time)
  - [A lexicon of test-time capabilities](#a-lexicon-of-test-time-capabilities)
  - [Evals as the unit of account](#evals-as-the-unit-of-account)
  - [Cheap judgments before expensive thinking](#cheap-judgments-before-expensive-thinking)
  - [How capabilities compound across a network](#how-capabilities-compound-across-a-network)
  - [Open questions for the field](#open-questions-for-the-field)
- [Part II: Our implementation](#part-ii-our-implementation)
  - [Where each term lives in OpenAgents](#where-each-term-lives-in-openagents)
  - [What "is a capability" means in our system](#what-is-a-capability-means-in-our-system)
  - [Each term in OpenAgents](#each-term-in-openagents)
  - [Our evals in practice](#our-evals-in-practice)
  - [Our numbers: judgments before thinking](#our-numbers-judgments-before-thinking)
  - [Our collective: Coder, the Gym, and Verse](#our-collective-coder-the-gym-and-verse)
  - [How the protocol carries test-time capabilities](#how-the-protocol-carries-test-time-capabilities)
- [Part III: What we'll measure next](#part-iii-what-well-measure-next)
- [References](#references)

## Part I: The concept

This part is vendor-neutral. It uses no product names, and none of our
numbers; Part II has those.

### What test-time compute is

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

### Why it matters

Test-time compute changes what "a better model" means. A fixed set of
weights can answer better if the system around it spends more, and more
wisely, per question. Two lessons from that work shape everything below.

1. **A verifier is what makes extra compute pay.** Sampling helps only as far
   as something can tell a right answer from a wrong one. Where the checker
   is weak, extra samples stop helping.
2. **Compute should be allocated per question.** Easy prompts don't need a
   long chain of thought; hard ones do. The allocation decision is itself a
   judgment, and it should cost far less than the work it allocates.

Both lessons generalize past tokens, to the *system* around a model: what
it's allowed to use, who decides what to use, and how anyone knows it
helped.

### The thesis: capability is something you can acquire at test time

The test-time compute literature mostly asks how to get more out of one
model by letting it think more. A second question follows: **what can an
agent become able to do, at the moment it runs, without anyone retraining
it?**

A coding agent that can't see a repository can't answer "what's the largest
file here?" no matter how long it thinks. Give it a tool that maps the
repository, and it can. The weights didn't change; the agent's capability
did. The same is true of a written guide it reads before a task, a
knowledge entry about a recurring mistake, or a stronger agent it hands a
well-prepared briefing to. Tool use as a learned behavior is well studied
([Schick et al., 2023](https://arxiv.org/abs/2302.04761)), as are skill
libraries an agent grows as it works
([Wang et al., 2023](https://arxiv.org/abs/2305.16291)) and retrieval that
brings documents into generation
([Lewis et al., 2020](https://arxiv.org/abs/2005.11401)). What this essay
adds is a discipline for treating each of these as a measured, shareable
unit.

A **test-time capability** is an ability an agent gains at inference time,
without a weight update, by admitting something into the run. Something is
a test-time capability only if a controlled comparison, the same tests run
with and without it, shows the agent does measurably better with it.
Having it installed, described, or demonstrated is not evidence. There are
five general sources:

| Source | What gets admitted |
| --- | --- |
| Tools and plugins | Code with typed operations and bounded access to the host, or a program that runs it |
| Skills | A written guide the agent reads before a task |
| Knowledge | Cited entries (methods, edge cases, known mistakes) retrieved and filtered for the task |
| Delegation | Another agent, briefed with selected evidence |
| Typed judgment | A fast, cheap decision that picks which of the above to use, and when |

The last row is the one that makes the others usable. An agent with fifty
tools and no good way to decide which to use is worse than an agent with
none. A typed judgment answers a typed question (yes or no, a choice among
options, an ordered score) with probabilities, and ordinary code decides
what those probabilities cause; the judge writes no text and grants no
authority. The idea borrows the fast, automatic "System 1" of Kahneman's
*Thinking, Fast and Slow* (2011); the slow, effortful work is left to the
generator.

### A lexicon of test-time capabilities

We propose ten terms. Each has a way to be wrong. We tried to keep the list
short; a term earns a place only if it changes what someone building or
evaluating an agent does. Each entry says what the term is, why it
matters, and how anyone would measure it.

#### 1. Test-time capability (TTCap)

**Definition:** an ability an agent gains or loses at inference time,
without updating weights, because a component was admitted to the run, and
whose effect a controlled comparison shows: the same tests, run with the
component and without it.

**Why it matters:** it separates what a component *is* from what it
*does* for a particular agent. A component with no such comparison yet is a
*candidate* capability, not a capability.

**How to measure it:** a with-and-without evaluation on tests written for
what the component claims to help with.

#### 2. Capability admission

**Definition:** the host's decision that a specific, locked version of a
component may take part in a run.

**Why it matters:** a result is only meaningful about the exact bytes it
measured. Discovering, installing, enabling, granting access to, and
admitting a component are separate decisions; installing one should grant
nothing.

**How to measure it:** record a digest of the locked component set in every
result, so a result names exactly what it measured.

#### 3. Capability delta

**Definition:** the difference in outcome between the *with* arm (the
component admitted) and the *without* arm (nothing admitted), on the same
tests, repeated enough times to see the spread.

**Why it matters:** it is the size of the capability. It holds only for
the baseline it was measured against: a tool can add a lot to an agent that
otherwise can't see the files, and little to one that can.

**How to measure it:** run both arms on the same tests several times, and
let a written rule decide the verdict, for example "better only if more
tests pass and the gain clears the spread between repeats, with cost and
time within a stated bound."

#### 4. Reach and restraint

**Definition:** *reach* is how often the agent actually uses a capability
when a test says it should; *restraint* is how often it leaves the
capability alone when a test says it shouldn't.

**Why it matters:** an installed capability the agent never invokes
changes no outcome, so it measures as no capability at all. One it invokes
everywhere can make unrelated work worse.

**How to measure it:** mark each test should-use or should-not-use, and
report outcomes per test in both arms.

#### 5. Judgment budget

**Definition:** the time and money a system spends deciding *how* to answer
before it spends anything on answering. The judgment must be much cheaper
than the work it can avoid.

**Why it matters:** it is the per-question allocation lesson of test-time
compute, applied one level up: before allocating thinking, decide whether
the turn needs a large model at all, and which capability should handle it.

**How to measure it:** the judgment's latency and cost, and the precision of
whatever it serves without calling the large model.

#### 6. Test-time delegation

**Definition:** acquiring another agent's capability for one task by
handing it a prepared briefing, then recording what it did as part of the
same task.

**Why it matters:** the strongest capability available for a task is
sometimes another agent. Treating it as a capability means its
contribution is measured, not assumed.

**How to measure it:** the same outcome, cost, and time as any attempt,
compared with the attempt made without delegating.

#### 7. Verified capability

**Definition:** a capability whose favorable verdict was reproduced by
someone other than the person who first ran it: a different evaluator, the
same test set version, the same component version.

**Why it matters:** confidence should come from reproduction by someone
else, not from the author's report. Reproduction is the verifier that
decides which claims deserve to spread.

**How to measure it:** an independent rerun that publishes its own result,
citing the original, and confirms on a matching verdict or disputes
otherwise, with both kept visible.

#### 8. Capability adoption

**Definition:** making a verified capability part of the agent everyone
starts with.

**Why it matters:** adoption is how one person's reproduced result becomes
every user's default, without a training run.

**How to measure it:** whether the adopted capability keeps its delta on
tests its author didn't write, and whether the whole default set still
performs as well together.

#### 9. Capability credit

**Definition:** recognition that goes to the people whose work made a
capability real, and only for the events that show it was used.

**Why it matters:** credit for activity (runs, publishes, downloads)
rewards components that exist; credit for independent confirmation and
adoption rewards components that help.

**How to measure it:** credit records that anyone can recompute from the
public confirmation and adoption events.

#### 10. The capability flywheel

**Definition:** the loop in which people add capabilities, tests prove
them, others confirm them, and adoption hands them to every agent, which
then takes on harder tasks that reveal the next missing capability.

**Why it matters:** it is the mechanism by which a network of contributors
could improve an agent faster than one team can.

**How to measure it:** not participant counts, but *incremental
out-of-sample verified passes per adopted contribution*, with cost,
latency, and harmful regressions reported alongside.

### Evals as the unit of account

If capabilities are acquired and shared, something has to decide which ones
actually improve an agent's results. The proposal is that a
per-component, with-and-without evaluation plays that role, and three of
its properties do most of the work.

- **Two arms, not one score.** A benchmark score tells you how an agent did.
  A with-and-without result tells you what one component changed. That is
  the thing you'd want to share, adopt, or pay attention to.
- **A written rule gives the verdict.** The rule is a versioned file, and
  every result carries the digest of the rule that judged it. Rules have
  bugs too; when one is replaced, old results keep their old digest and stay
  readable.
- **Others can rerun it.** A published test set, a published result, and
  the exact component versions let someone else rerun the evaluation and
  confirm or dispute it.

Why this is more useful than chasing a leaderboard: a leaderboard rewards
one system on one fixed task set, and it rewards fitting that set. A
per-component eval asks a narrower question with a clearer answer: *does
this thing help, where, and at what cost?* Benchmarks remain how an agent
is checked as a whole; evals are how to decide what goes into it.

Evals also have failure modes. A grader is a piece of software and can be
wrong, which can flip a verdict by chance. It deserves the same scrutiny,
and the same versioning, as the component under test.

### Cheap judgments before expensive thinking

Test-time compute costs money and time. The literature's answer is to
allocate it per question. The same idea applies one level up: before
allocating *thinking*, decide whether the turn needs a large model at all,
and which capability should handle it. A system can offer a ladder of
answers at rising cost: a prepared answer, a prepared answer finished by a
small model, an answer grounded in a knowledge base, the full model, and a
hand-off to an agent with a computer.

Three principles follow. They are design principles, and the third is a
hypothesis to test.

1. **Never wrong fast.** A prepared answer should be served only when the
   judgment clears thresholds tuned for precision. A fast answer to the
   wrong question is worse than a slow right one, so a cheap tier is gated
   on measured precision, not on confidence alone.
2. **Spend the big model where it adds something.** Identity questions,
   small talk, and questions with reviewed answers don't need it. Requests
   for work need a computer, not a model's guess at doing the work in
   chat.
3. **Capability can substitute for compute.** A tool that answers directly
   can beat a model that must search for the answer, on both time and
   correctness. Time and cost belong in the report as notes, never as the
   verdict, because a faster wrong answer isn't a capability.

### How capabilities compound across a network

Test-time capabilities make network effects possible in principle. Weights
improve when a lab trains them, on the lab's schedule. A test-time
capability can come from anyone, be tested by anyone, and, once adopted,
reach every agent that uses the same defaults without a training run. The
unit that compounds is not a longer prompt or a count of packages. It is a
**reusable improvement with independent evidence**: an exact component
version, a with-and-without result, and confirming reruns by people who
didn't write it.

What a network would add:

- **More sources of capability.** Different people bring different task
  families, libraries, and environments, and write tools and tests for the
  work they know.
- **More verification.** Reruns by other evaluators turn one person's claim
  into a reproduced result. That is the verifier the test-time compute
  literature says extra effort depends on, supplied by people instead of a
  reward model.
- **Inheritance.** Adoption turns one confirmed result into a default for
  everyone, and inspectable run traces let anyone see what happened.
- **Credit that tracks use.** Recognition for confirmations and adoptions,
  the two events that show someone else's work was used, keeps the
  incentive on components that help rather than components that exist.

This is a hypothesis. Whether adding participants makes an agent
measurably better has to be shown, measured the way the lexicon says.

### Open questions for the field

- **Durability of adoption.** Does an adopted capability keep its delta on
  tests its author didn't write?
- **Reach.** How should capabilities be described to the judgment that
  picks them, so the agent reaches for them when it should? This may be the
  cheapest gain available.
- **Deltas depend on the baseline.** A capability measured against a weak
  baseline can shrink against a strong one. Which baseline should a shared
  result be measured against?
- **Interaction effects.** Two capabilities that each help alone can
  interfere together. Adoption needs a regression check across the whole
  default set, not only a check of the newcomer.
- **Cost.** Every result should carry a price for both arms, not a guess.
- **Statistical power.** A handful of tests and repeats is enough to see a
  large change and too few to see a small one. Test sets need to grow as the
  deltas people care about shrink.
- **Grader quality.** Graders are code and make mistakes. How should they
  be checked the way results are?
- **Compute and capability together.** How do test-time capabilities
  interact with more thinking? Can a tool let a cheaper model with less
  reasoning match a stronger one, and when does extra reasoning still pay on
  top of a tool?
- **Network evidence.** Does the flywheel turn? Its measure, incremental
  out-of-sample verified passes per adopted contribution, should be reported
  even when it's zero.

## Part II: Our implementation

This part is about OpenAgents. It says how we implement each term in Part
I, what we've measured, and what "is a capability" concretely decides in
our system. Every claim links the code or the dated record behind it.

### Where each term lives in OpenAgents

Status is the mechanism's, as the
[glossary](../glossary.md#test-time-capabilities) records it.

| Concept | Our component, crate, or doc | NIP and kinds | Status |
| --- | --- | --- | --- |
| [Test-time capability](#1-test-time-capability-ttcap) | Extensions (tool, plugin, skill, package) admitted to a Coder turn; [Wasm plugins](../extensions/plugins.md), `crates/plugin`; [extension eval](../extensions/evaluation.md) | [NIP-EXT](../../nips/openagents/NIP-EXT.md) `3184`, [NIP-EVAL](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile) `3189` | Implemented |
| [Capability admission](#2-capability-admission) | [Extension architecture](../extensions/architecture.md); eval run locks | [NIP-EXT](../../nips/openagents/NIP-EXT.md#listings-updates-and-installation), [NIP-CAP](../../nips/openagents/NIP-CAP.md#description-binding-and-grant), [NIP-RUN](../../nips/openagents/NIP-RUN.md#record-types) `3187` | Partial |
| [Capability delta](#3-capability-delta) | `openagents ext eval`, the hosted runner, the [`ext-eval-v2` gate](../../crates/gym/gates/ext-eval-v2.json) | [NIP-EVAL reports](../../nips/openagents/NIP-EVAL.md#reports) | Implemented |
| [Reach and restraint](#4-reach-and-restraint) | Should-fire and should-not-fire cases in every [suite](../extensions/evaluation.md) | [NIP-EVAL suites](../../nips/openagents/NIP-EVAL.md#suites) | Implemented |
| [Judgment budget](#5-judgment-budget) | Jev and the [chat router](../coder/design/2026-09-28-chat-router.md), tiers T0 to T4 | [NIP-CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs) `25900`, `25910`/`26910` | Implemented |
| [Test-time delegation](#6-test-time-delegation) | Coder's [delegate door](../coder/runtime/delegate-door.md); delegate sessions | [NIP-PRG](../../nips/openagents/NIP-PRG.md#step-kinds), [NIP-SESS](../../nips/openagents/NIP-SESS.md#steering-capability), [NIP-ATIF](../../nips/openagents/NIP-ATIF.md#delegated-sub-agents) `3198`/`3199` | Implemented |
| [Verified capability](#7-verified-capability) | [Eval checks](../extensions/evaluation.md#checks-adoption-and-credit) | [NIP-EVAL checks](../../nips/openagents/NIP-EVAL.md#checks) `3189` | Implemented |
| [Capability adoption](#8-capability-adoption) | [`openagents:coder-defaults`](../../packages/coder-defaults/) releases | [NIP-EVAL adoption](../../nips/openagents/NIP-EVAL.md#adoption), [NIP-EXT](../../nips/openagents/NIP-EXT.md) `3184` | Partial |
| [Capability credit](#9-capability-credit) | XP referee, `crates/xp-ledger` | [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check) `3193`, `3194` | Implemented |
| [Capability flywheel](#10-the-capability-flywheel) | Chat, Gym, and Verse; [how the network compounds](../coder/design/networked-coder-plan.md#how-the-network-compounds) | No single carrier | Defined |

The full protocol mapping, stage by stage and field by field, is
[below](#how-the-protocol-carries-test-time-capabilities).

### What "is a capability" means in our system

Part I's test, a controlled with-and-without comparison, decides three
concrete things in OpenAgents. Installing a tool, describing it, or demoing
it decides none of them.

1. **The Gym's gate rates it Better.** `openagents ext eval` and the hosted
   runner run every test in both arms, and the
   [`ext-eval-v2` gate](../../crates/gym/gates/ext-eval-v2.json) reads
   **Better** only when more tests pass with the tool, the score gain clears
   the spread between repeats, and cost and time stay within 1.5 times plus
   the spread. The result is a
   [NIP-EVAL extension evaluation](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile)
   report.
2. **A different trainer's check confirms it, and XP is paid.** An
   [eval check](../extensions/evaluation.md#checks-adoption-and-credit)
   reruns the published suite and publishes its own
   [NIP-EVAL check](../../nips/openagents/NIP-EVAL.md#checks). Only a
   confirming check earns
   [NIP-XP `eval-check`](../../nips/openagents/NIP-XP.md#eval-check) credit.
3. **It can be adopted into every Coder's defaults.** A tool becomes an
   adoption candidate only when its result is **Better** and at least three
   distinct trainers' checks confirmed it
   ([operator policy](../extensions/evaluation.md#checks-adoption-and-credit));
   adoption itself is an operator's
   [NIP-EVAL adoption](../../nips/openagents/NIP-EVAL.md#adoption) decision
   and a new [`coder-defaults`](../../packages/coder-defaults/) release, and
   pays [NIP-XP `eval-adopt`](../../nips/openagents/NIP-XP.md#eval-adopt)
   credit.

A component with no such result is a candidate: the gate hasn't rated it
**Better**, no check can confirm it, and it can't be adopted.

### Each term in OpenAgents

#### Test-time capability in OpenAgents

We have five ways to acquire one, all built:

| Source | What gets admitted | Where it lives |
| --- | --- | --- |
| Tools and plugins | A Wasm guest with typed operations and bounded host access, or a program that runs one | [Wasm plugins](../extensions/plugins.md), `crates/plugin` |
| Skills | A `SKILL.md` guide the agent reads before a task | [Plugins and skills](../glossary.md#plugins-and-skills) |
| Knowledge | Cited entries (methods, edge cases, slips) retrieved and filtered by Jev | [Knowledge base](../coder/design/knowledge-base.md) |
| Delegation | Another agent, briefed with evidence Jev chose | [The delegate door](../coder/runtime/delegate-door.md) |
| Typed judgment | A System One answer that picks which of the above to use, and when | [The chat router](../coder/design/2026-09-28-chat-router.md) |

Concretely: an extension (tool, plugin, skill, or package) admitted to a
Coder turn; a knowledge entry retrieved into a Microcoder step; a delegate
briefed by Coder One. Each is measured by an
[extension eval](../extensions/evaluation.md).
[Jev](../glossary.md#decision-models-and-runtimes) is TypeSafe's System
One model: it answers typed questions with probabilities, and code decides
what those probabilities cause. It writes no text and grants no authority.

#### Capability admission in OpenAgents

Discovery, installation, enablement, grants, and admission are separate
decisions in our [extension architecture](../extensions/architecture.md).
An eval run holds the exact extension and Coder's question sets in its run
locks, and every report records the lock digest. Installing a package
grants nothing.

#### Capability delta in OpenAgents

`openagents ext eval` and the hosted runner run every test in both arms as
confined `coder -p` turns with an ATIF trajectory each, judged by the
[`ext-eval-v2` gate](#what-is-a-capability-means-in-our-system). Our first
hosted runs, three runs per arm, six tests per tool
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

#### Reach and restraint in OpenAgents

Every suite marks each test should-fire or should-not-fire. Each starter
suite has four of the first kind and two of the second. In the same
record, restraint held: every should-not-fire test passed with the tool as
often as without it, except one run of Code finder's `explain-idempotent`.
Reach did not always hold: `where-tests`, `known-bugs`, `workarounds`, and
`ci-failures` failed in both arms because Jev didn't choose the tool's
program for that wording, so for those tests the tool changed nothing.
Those tests are now the work list for each tool.

#### Judgment budget in OpenAgents

The [chat router](../coder/design/2026-09-28-chat-router.md) asks Jev one
request of independent questions (route, prepared answer, whether the reply
needs specifics, risk, lane, opener) and a policy table in code chooses a
tier: T0 a whole prepared answer, T1 a prepared stem finished by a cheap
model, T2 an answer grounded in a knowledge base, T3 the full model, T4 a
dispatch or command offer. A slow or failed judge falls back to the model's
reply, so the judgment can only save time. Judgment latency is 170 ms
median, 235 ms p95 on the held-out set, and the precision of what it serves
is 100 % canned precision, 36 of 36, on 138 held-out messages
([router evaluation](../coder/measurements/2026-09-28-chat-router-eval.md)).

#### Test-time delegation in OpenAgents

Coder's [delegate door](../coder/runtime/delegate-door.md) runs Microcoder
through the first connected provider with capacity (a Codex login, then a
Claude Code login, then our cloud fallback) and fails over on usage or rate
limits. When Microcoder can't run, Claude Code or Codex CLI takes the turn,
briefed with what Jev chose from the workspace. OpenCode and Devin routes
run as [delegate sessions](../glossary.md#the-openagents-app-and-chat)
copied into the task's history.

In one declared Terminal-Bench 4.0 attempt, Coder One with Jev's briefing
passed `fin-saccr-rwa` for $0.9429 in 149.5 s, below Fable 5.1 low's
cheapest ($1.2246) and fastest (222.5 s) wins. That attempt was in-sample
and tuned, and across 7 series only 2 of 13 attempts beat the bar
([record](../terminal-bench/2026-09-27-fable-delegate.md)). Delegation is a
capability to measure, not a guaranteed win.

#### Verified capability in OpenAgents

An [eval check](../extensions/evaluation.md#checks-adoption-and-credit)
reruns a published suite and publishes its own NIP-EVAL `3189` that cites
the original: a different trainer, the same test set release, the same
tool release. It confirms on a matching verdict and disputes otherwise;
both stay visible. In the hosted record, a second trainer checked each of
the three results and all three confirmed.

#### Capability adoption in OpenAgents

An operator issues an `openagents.eval-admission.v1` decision citing the
reports and publishes a release of the
[`openagents:coder-defaults`](../../packages/coder-defaults/) package that
depends on the tool. A tool becomes a candidate when its result is
**Better** and at least three distinct trainers' checks confirmed it.
Adoption is an operator decision, never automatic. The mechanism exists; no
adoption has been made yet.

#### Capability credit in OpenAgents

Two [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check) rules.
`eval-check` credits the checker, the original evaluator, and the suite's
author when a check confirms a result. `eval-adopt` credits the tool's
author, the suite's author, and the evaluators of cited results when Coder
adopts the tool. A run, a publish, a view, or a download earns nothing.
Credit is XP and your name. It is never money, and no payout exists. The XP
referee on `coderos-4080` signed the first nine awards from the hosted
checks, and any reader can recompute them with `crates/xp-ledger`.

#### The capability flywheel in OpenAgents

The chat is the front door. A person asks OpenAgents what to test or makes
a tool by chatting, runs the tests on our computers, adds the result to the
Gym, and earns credit when others check it or Coder adopts it. The
[README's loop](../../README.md#the-loop) draws it. Its test is the network
plan's
([how the network compounds](../coder/design/networked-coder-plan.md#how-the-network-compounds)).
We have not shown the flywheel turning yet; we have built each part of it.

### Our evals in practice

Two lessons from running our evals:

- **The v1 gate was wrong, and we replaced it.** Under `ext-eval-v1`, a tool
  that made Coder *faster* but no more correct read **Better**. We found
  that in a [live run](../extensions/measurements/2026-09-29-ext-eval-runner-live.md)
  where both arms passed the same tests, and replaced the rule the same
  day: under `ext-eval-v2`, faster or cheaper alone is **No clear change**
  with a note. The old result keeps its old digest and stays readable.
- **A grader bug flipped a result.** A grader looked for "not found" and
  missed "the server cannot find the requested resource", turning one run's
  result to **Worse** by chance. We fixed the pattern and released a new
  test set version; the old one stays readable.

Reruns by others rest on publication: a published suite is a NIP-EXT
release, a published result carries its trainer's signed request, and a
check verifies every file against the release before rerunning it.

On benchmarks, we've kept the same distinction in our own
[Terminal-Bench](../terminal-bench/README.md) work, which separates
in-sample development wins from out-of-sample results (for example, 30
confirmed out-of-sample wins on 65 TB2.1 tasks, measured on cost against
Fable 5 xhigh, on an older and easier benchmark than TB4). Benchmarks
remain how we check Coder as a whole; evals are how we decide what goes
into it.

### Our numbers: judgments before thinking

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

How Part I's three principles show up here:

1. **Never wrong fast.** A prepared answer is served only when the route,
   the answer, and the "needs specifics" readings all clear thresholds tuned
   for precision.
2. **Spend the big model where it adds something.** Identity questions,
   small talk, and product questions with reviewed answers don't need it.
   Requests for work get a one-tap offer to run Coder.
3. **Capability can substitute for compute.** In the hosted runs, a Project
   map run took 10.7 s with the tool against 24.9 s without it, and passed
   more tests. We report time and cost as notes, never as the verdict.

We don't yet price every lane: Coder doesn't price gateway lanes, so the
eval records list cost as unknown.

### Our collective: Coder, the Gym, and Verse

We are building the best coding agent in the world by using network
effects: an agent collective. Coder is the first agent. The Gym is where
people help agents get better, through the plugin system and the evals
that measure it. Verse is where agents and people meet, and where the Gym's
results and evals boards live.

Part I's network effects, as we build them:

- **More verification** comes from checks by other trainers.
- **Inheritance** is adoption into `coder-defaults`. Traces in
  [ATIF v1.8](../coder/runtime/traces.md) make each run inspectable, and
  NIP-ATIF, still designed and not yet published by any component, is how
  they're meant to travel.
- **Credit that tracks use** is XP for checks and adoptions.

This is a design and a hypothesis. The parts are built and the first runs,
checks, and awards are live. Whether adding participants makes Coder
measurably better is the claim we still have to earn.

### How the protocol carries test-time capabilities

The lexicon is ours; the wire formats that carry it are the
[OpenAgents NIPs](../../nips/openagents/README.md). This section says which
NIP is for what, and which kinds and fields carry each term and each stage
of a capability's life. It cites only what the NIP files define. Where a
term has no carrier yet, the table says so rather than naming one.

Status is each whole contract's, as the
[implementation coverage report](../protocol/2026-09-26-nip-implementation-coverage.md)
records it and the [glossary's protocol table](../glossary.md#nostr-and-shared-protocols)
labels it; the shared contracts, which the glossary doesn't label, are
Partial by the report's own account. A profile inside a NIP can be further
along than the NIP as a whole; the notes say where.

#### Which NIP is for what

| NIP | Status | What it's for in this lifecycle | Kinds it owns |
| --- | --- | --- | --- |
| [NIP-EXT](../../nips/openagents/NIP-EXT.md) | Partial | The components a capability comes from: signed immutable releases of tools, plugins, skills, and `eval-suite` test sets, with installation, enablement, grants, and admission kept separate. | `3184`, `3185`, `3186`, `30184`, `30185` |
| [NIP-CAP](../../nips/openagents/NIP-CAP.md) | Partial | Describes an execution interface, its host binding, the grant to use it, and its observed presence; a `service` profile advertises decision services such as Jev's. | `30180`, `30181` |
| [NIP-KB](../../nips/openagents/NIP-KB.md) | Implemented | Knowledge as a capability: signed entry versions and heads, with evidence that an entry helps as a with-and-without `3189` report. | `3190`, `30190`, `3191` |
| [NIP-PRG](../../nips/openagents/NIP-PRG.md) | Partial | Typed workflows whose `decide` and `delegate` steps call a pinned decision function or hand a bounded task to an admitted executor. | `30182`, `30183` |
| [NIP-CJ](../../nips/openagents/NIP-CJ.md) | Partial | The jobs: conversation turns with the router's `judgment` feedback, typed decision jobs, execution jobs (the hosted eval runner), and the chat's eval `offer`s, `card`s, and test-set `draft`. | `25900`/`26900`/`27000`, `25910`/`26910`/`27010`, `25920`/`26920`/`27020` |
| [NIP-CTX](../../nips/openagents/NIP-CTX.md) | Designed | Context requests and selection receipts: which evidence was chosen for a recipient, which is what a delegate's briefing is. | None; shared `3188` |
| [NIP-POL](../../nips/openagents/NIP-POL.md) | Designed | Route receipts and observed usage (the cost side of a judgment), and the authority under which an evaluated implementation is adopted. | None; shared `3188` |
| [NIP-SESS](../../nips/openagents/NIP-SESS.md) | Designed (read-only observer implemented) | Engine sessions: each delegate engine's steering capability row, and session history exports whose portable form can be an ATIF trajectory. | None; shared `3188` |
| [NIP-RUN](../../nips/openagents/NIP-RUN.md) | Partial | The authoritative journal of a run: its lock, parent run, dispatched attempts, and outcome. Trajectories observe; RUN decides. | `3187`, `30186` |
| [NIP-ATIF](../../nips/openagents/NIP-ATIF.md) | Designed | Carries the trajectory of each run, and links a delegating step to the sub-agent's trajectory. | `3198`, `3199` |
| [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) | Partial (extension evaluation wire formats implemented) | The unit of account: with-and-without reports, the gate's verdict, `3189` publications, checks, hosted runs, and adoption. | `3189`, `3195` |
| [NIP-XP](../../nips/openagents/NIP-XP.md) | Implemented | Credit: the `eval-check` and `eval-adopt` rules, awards, revocations, and per-reader ledgers. | `30193`, `3193`, `3194`, `3196`, `3197`, `13193`, `13195`, `30194` |
| [NIP-OPT](../../nips/openagents/NIP-OPT.md) | Designed | Searching for a better implementation; its result is promoted only through EVAL admission and a new EXT release. | None; shared `3188` |
| [NIP-WORK](../../nips/openagents/NIP-WORK.md) | Designed | A signed delegation of tracked work to another principal. | None; shared `3188` |
| [NIP-MV](../../nips/openagents/NIP-MV.md) | Partial | Verse: Gym notes, the world-chat lines that cite a trainer's published eval result. | `23300`–`23302`, `33300`, `33301` |
| [Shared contracts](../../nips/openagents/contracts.md) | Partial | The locks, references, and private `3188` envelope every row above relies on. | `3188` |

Block [NIP-AO](../../nips/block/NIP-AO.md), [NIP-AM](../../nips/block/NIP-AM.md),
and [NIP-AE](../../nips/block/NIP-AE.md) (telemetry, turn metrics, memory)
are not capability records; NIP-ATIF's
[Block mapping](../../nips/openagents/NIP-ATIF.md#relationship-to-block-nips)
says how a host turns them into trajectory steps. HOST, REACH, TERM, CTRL,
ENV, WS, AUTO, LIVE, COORD, MKT, LAB, X402, and SOV carry the computers,
access, control, and payment a run needs, not the capability or its
evidence, so they don't appear below.

#### The lifecycle, stage by stage

| Stage | Carrier | Kinds and fields |
| --- | --- | --- |
| Discover | [EXT](../../nips/openagents/NIP-EXT.md#listings-updates-and-installation), [CAP](../../nips/openagents/NIP-CAP.md#discovery-and-probes), [KB](../../nips/openagents/NIP-KB.md#heads-30190), [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs) | EXT listing `30184` and release `3184`; the operation descriptor's `summary`, `input`, `output`, `effects`, and `evaluation` support shortlisting. CAP `30180` heads with presence `present`/`absent`/`unavailable`/`unprobed`/`unknown`. KB heads `30190`. In chat, the CJ `card` of type `tool` and `news`. |
| Admit | [EXT](../../nips/openagents/NIP-EXT.md#listings-updates-and-installation), [CAP](../../nips/openagents/NIP-CAP.md#description-binding-and-grant), [contracts](../../nips/openagents/contracts.md#locks-and-resolution), [RUN](../../nips/openagents/NIP-RUN.md#record-types) | Installation commits one lock; enablement, grants, and invocation admission are separate. CAP separates definition, host binding, grant, and presence. RUN's `created` record holds the lock and grant references. An eval report's `subject.lock` is the lock the with arm held. |
| Run and judge | [CJ](../../nips/openagents/NIP-CJ.md#typed-decision-jobs), [CAP](../../nips/openagents/NIP-CAP.md#decision-services), [PRG](../../nips/openagents/NIP-PRG.md#step-kinds) | Decision jobs `25910`/`26910` (`openagents.systemone.v1`, question types `noul`, `choice`, `score`); conversation `25900` with `router`, answered by `judgment` feedback carrying `route`, `route_p`, `answer_p`, `needs_specifics`, `risk`, `lane`, and `tier`. CAP's `service` profile lists the lanes and doors. PRG's `decide` step. |
| Delegate | [PRG](../../nips/openagents/NIP-PRG.md#step-kinds), [SESS](../../nips/openagents/NIP-SESS.md#steering-capability), [ATIF](../../nips/openagents/NIP-ATIF.md#delegated-sub-agents), [WORK](../../nips/openagents/NIP-WORK.md#delegation-and-execution-links), [CTX](../../nips/openagents/NIP-CTX.md#context-requests-and-selection-receipts) | PRG `delegate` step. SESS steering rows for the delegate engines (OpenCode `run` as Coder One's delegate executor, Devin and OpenCode ACP routes). ATIF `subagent_trajectory_ref` and the manifest's `parent` and `children`. WORK `openagents.work-delegation.v1`. CTX `openagents.context-selection.v1` for the briefing's evidence. |
| Trajectory | [ATIF](../../nips/openagents/NIP-ATIF.md#manifest), [EVAL](../../nips/openagents/NIP-EVAL.md#reports) | Manifest `openagents.atif-manifest.v1` (`trajectory_id`, `steps_digest`, `task`, `run`, `coverage`, `derivation`), private on `3188` or public as `3198` with chunks `3199`. Each eval run's `artifacts` include its ATIF log ArtifactRef (schema `ATIF-v1.8`). |
| Measure (with and without) | [EVAL](../../nips/openagents/NIP-EVAL.md#reports) | `openagents.eval-report.v1`: `subject` and `baseline` arms, `runs`, `coverage`, `measurements` (`cases_passed`, `mean_score`, `cost_usd`, `seconds`, and `change` on the `comparison` arm), `verdict`, and `acceptance` (the gate's digest, repeated in `meta.ext_eval.gate`). KB's [evidence](../../nips/openagents/NIP-KB.md#evidence-3189) uses the same report for knowledge entries. |
| Publish | [EVAL](../../nips/openagents/NIP-EVAL.md#publication), [EXT](../../nips/openagents/NIP-EXT.md#component-types-and-operation-descriptors), [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs) | The suite as an EXT `3184` release with one `eval-suite` component. The result as a `3189` with `t: oa:ext-eval:v1`, `e` markers `suite`, `subject`, `request`, and `meta.ext_eval_report`. CJ `publish_eval` offer and the hosted runner's `publish` action. Released leaderboards as `3195`. |
| Check and verify | [EVAL](../../nips/openagents/NIP-EVAL.md#checks) | A `3189` with the `check` marker, the same suite and subject, the same subject-arm lock, and a different trainer; it confirms on an equal verdict and disputes otherwise. Hosted reruns set `check` in the `run` action. |
| Credit | [XP](../../nips/openagents/NIP-XP.md#eval-check) | `eval-check` pays `checker`, `evaluator`, `suite-author`; [`eval-adopt`](../../nips/openagents/NIP-XP.md#eval-adopt) pays `extension-author`, `suite-author`, `evaluator`. Quests `30193`, awards `3193`, revocations `3194`; the CJ `card` of type `credit` shows the reader's ledger. |
| Adopt | [EVAL](../../nips/openagents/NIP-EVAL.md#adoption), [POL](../../nips/openagents/NIP-POL.md#optimization-authority-and-adoption), [EXT](../../nips/openagents/NIP-EXT.md#release-and-package-manifest) | An `openagents.eval-admission.v1` decision citing the reports and checks, then a `coder-defaults` `3184` release whose `dependencies` include the tool's release and whose `provenance.receipts` cite the admission. POL keeps activation an operator decision; active runs keep their lock. |
| Share (Verse) | [MV](../../nips/openagents/NIP-MV.md#gym-notes), [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs), [EVAL](../../nips/openagents/NIP-EVAL.md#gym-results-publication), [XP](../../nips/openagents/NIP-XP.md#trainer-cards-30194) | Gym notes: kind `9` world chat with `L`/`l` `openagents.gym` and an `e … source` citing the trainer's `3189`. CJ `open_screen` `verse.gym`. Gym leaderboards `3195`. Trainer cards `30194`. |

#### The terms and their carriers

| Term | Canonical carrier | What carries it, and what doesn't yet |
| --- | --- | --- |
| Test-time capability | [EXT](../../nips/openagents/NIP-EXT.md) + [EVAL](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile) | A component release (`3184`) plus a report that measured it with and without. The five sources map to EXT components (`plugin`, `capability`, `skill`), KB entries (`3190`), delegation (PRG `delegate`), and typed judgment (CJ decision jobs). A component with no report is a candidate. |
| Capability admission | [EXT](../../nips/openagents/NIP-EXT.md#listings-updates-and-installation), [CAP](../../nips/openagents/NIP-CAP.md#description-binding-and-grant) | Lock, grant, and admission as separate decisions; a report's `subject.lock`. No public event records one Coder turn's admission; RUN's `created` record is the private place for it. NIP-EVAL's `openagents.eval-admission.v1` is adoption, not this. |
| Capability delta | [EVAL](../../nips/openagents/NIP-EVAL.md#reports) | `subject` and `baseline` arms, `measurements` with `change` on the `comparison` arm, `verdict`, and the gate digest in `acceptance`. The gate's rules (`ext-eval-v2`) are a Gym file the report pins, not wire text. |
| Reach and restraint | [EVAL](../../nips/openagents/NIP-EVAL.md#suites) | Each case's `kind`, `should-fire` or `should-not-fire`, repeated in `meta.ext_eval.cases`, with per-case outcomes in `runs`. No field aggregates a reach or restraint rate; a reader computes it. |
| Judgment budget | [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs), [POL](../../nips/openagents/NIP-POL.md#routing-and-observed-cost) | CJ `judgment` feedback carries the decision (`tier`, `route_p`, `answer_p`, `needs_specifics`) and a result names a bank answer with `model: "bank:<bank id>"`. Its time and cost have no wire field today; POL's `openagents.route-usage.v1` (`latency_ms`, `cost_microunits`) is Designed. |
| Test-time delegation | [PRG](../../nips/openagents/NIP-PRG.md#step-kinds), [SESS](../../nips/openagents/NIP-SESS.md#steering-capability), [ATIF](../../nips/openagents/NIP-ATIF.md#delegated-sub-agents) | PRG `delegate`, SESS delegate-engine rows, ATIF `parent`/`children`. The delegate door's provider failover and Jev's briefing run locally and have no Nostr record yet. |
| Verified capability | [EVAL](../../nips/openagents/NIP-EVAL.md#checks) | A confirming `check`, decided by `eval_ext::confirms` from the two signed events. The candidate threshold (three distinct trainers) is [operator policy](../extensions/evaluation.md#checks-adoption-and-credit), not a NIP field. |
| Capability adoption | [EVAL](../../nips/openagents/NIP-EVAL.md#adoption) | `openagents.eval-admission.v1` plus the `coder-defaults` `3184` release. No adoption has been published. |
| Capability credit | [XP](../../nips/openagents/NIP-XP.md#eval-check) | `eval-check` and `eval-adopt` awards (`3193`); XP is never money. |
| Capability flywheel | Composition of the rows above | No single carrier. Its measure, incremental out-of-sample verified passes per adopted contribution, has no wire field; a reader would derive it from `3189` results, checks, and `coder-defaults` releases. |

The NIPs above each point back here in a "Test-time capabilities" line, and
the [NIP index](../../nips/openagents/README.md#test-time-capabilities)
lists the same mapping from the protocol side.

## Part III: What we'll measure next

These are our open problems, for the OpenAgents implementation in Part II.
Part I's [open questions](#open-questions-for-the-field) are the general
versions.

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
- **Interaction effects.** Adoption into `coder-defaults` needs a
  regression check across the whole default set, not only a check of the
  newcomer.
- **Cost.** Every eval record should carry a price for both arms. Until
  Coder prices gateway lanes, cost is a blank we don't fill with guesses.
- **Statistical power.** Six tests and three runs per arm are enough to see
  a large change and too few to see a small one. Our suites need to grow as
  the deltas we care about shrink.
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
