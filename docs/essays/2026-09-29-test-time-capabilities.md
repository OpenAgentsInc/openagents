# Test-Time Capabilities

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
  or lose abilities* while it runs, without anyone retraining its weights,
  when a tool, a plugin, a skill, a knowledge entry, or another agent is
  admitted into the run.
- **A component is a candidate capability; evidence makes a capability
  claim.** The claim says how much admitting that exact component changed
  outcomes against a stated baseline, on a stated task distribution, under
  a stated grant and evaluation rule. Nothing is a capability in general.
  Having a component installed, described, or demonstrated makes no claim,
  and admitting one can destroy capability as easily as create it.
- **Cheap judgments should come before expensive thinking,** and claims
  others have reproduced can be shared, so that capabilities compound across
  a network of people and agents. Both are stated as hypotheses to measure.
- **Most of the mechanisms are prior art; the chain is the proposal.**
  Tools, retrieval, stored skills, memories, routing, and other agents are
  all known to help fixed-weight models. What's proposed here is one
  accountable path for all of them: artifact, admission, controlled delta,
  reproduction, external validation, adoption, credit. Adoption measures
  the marginal effect against the current default set, not the historical
  one, and credit goes to verification work, not to agreement.
  [Related work](#related-work-and-prior-art) says what came before.
- **We built an implementation** in OpenAgents, with first measurements,
  reproduction by a second trainer, no external validation yet, and no
  adoption yet; Part II has it, term by term. The words are also in the
  [glossary](../glossary.md#test-time-capabilities).

## Contents

- [Part I: The concept](#part-i-the-concept)
  - [What test-time compute is](#what-test-time-compute-is)
  - [Why it matters](#why-it-matters)
  - [The thesis: capability is something you can acquire at test time](#the-thesis-capability-is-something-you-can-acquire-at-test-time)
  - [What the word capability means here](#what-the-word-capability-means-here)
  - [The lifecycle in one figure](#the-lifecycle-in-one-figure)
  - [A lexicon of test-time capabilities](#a-lexicon-of-test-time-capabilities)
  - [Evals as the unit of account](#evals-as-the-unit-of-account)
  - [Cheap judgments before expensive thinking](#cheap-judgments-before-expensive-thinking)
  - [How capabilities compound across a network](#how-capabilities-compound-across-a-network)
  - [Related work and prior art](#related-work-and-prior-art)
  - [Open questions for the field](#open-questions-for-the-field)
- [Part II: Our implementation](#part-ii-our-implementation)
  - [Where each term lives in OpenAgents](#where-each-term-lives-in-openagents)
  - [What "is a capability" means in our system](#what-is-a-capability-means-in-our-system)
  - [Each term in OpenAgents](#each-term-in-openagents)
  - [Our evals in practice](#our-evals-in-practice)
  - [Our numbers: judgments before thinking](#our-numbers-judgments-before-thinking)
  - [Our collective: Coder, the Gym, and Verse](#our-collective-coder-the-gym-and-verse)
  - [How the protocol carries test-time capabilities](#how-the-protocol-carries-test-time-capabilities)
  - [The NIPs, one by one](#the-nips-one-by-one)
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
well-prepared briefing to. None of these mechanisms is new. Interleaving
reasoning with actions against external tools is the bridge from
test-time *reasoning* to test-time *acting*
([Yao et al., 2023](https://arxiv.org/abs/2210.03629)); tool use as a
learned behavior is well studied
([Schick et al., 2023](https://arxiv.org/abs/2302.04761)), as are skill
libraries an agent grows as it works
([Wang et al., 2023](https://arxiv.org/abs/2305.16291)), retrieval that
brings documents into generation
([Lewis et al., 2020](https://arxiv.org/abs/2005.11401)), memories of
past attempts ([Shinn et al., 2023](https://arxiv.org/abs/2303.11366)),
learned routing between models
([Ong et al., 2025](https://arxiv.org/abs/2406.18665)), and conversations
among agents ([Wu et al., 2023](https://arxiv.org/abs/2308.08155)).
[Related work](#related-work-and-prior-art) surveys them.

What this essay proposes is narrower. Prior work shows that tools,
retrieved knowledge, stored skills, memories, routing, and other agents
can improve fixed-weight models at inference time. We propose treating
these mechanisms uniformly as versioned *candidate* capabilities, and
treating what each does for an agent as an empirical **capability claim**:
a versioned, reproducible statement of the marginal effect of admitting
that exact artifact into a particular agent system, not a property the
artifact declares about itself. A claim is established by a controlled
with-and-without evaluation, reproduced by someone other than its author,
shown to hold on tasks its author didn't write, and only then eligible for
adoption into a shared default agent. The chain is:

**artifact → admission → controlled delta → reproduction → external
validation → adoption → credit.**

Pieces of it exist: paired with-and-without evaluation of agent skills
([Li et al., 2026](https://arxiv.org/abs/2602.12670);
[Kevin et al., 2026](https://arxiv.org/abs/2608.20614)), and signed,
verifiable provenance for software artifacts
([SLSA](https://slsa.dev/spec/v1.0/provenance);
[Torres-Arias et al., 2019](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias)).
As far as our search went, we found no prior work that puts the whole
chain together: exact artifact identity, a per-component controlled delta,
reproduction by someone other than the author, external validation before
adoption, adoption into a shared default, and credit tied to those
events. That is a claim about our search, not about the literature's
limits; the [comparison table](#related-work-and-prior-art) in Related
work lays it out column by column.

A **test-time capability** is an ability an agent gains, or loses, at
inference time, without a weight update, because something was admitted
into the run. The unit of account is not the component but the
**capability claim** about it: *admitting artifact A to locked baseline
agent B, under environment E and grant G, on task distribution D, judged
by evaluation rule R, changed the outcomes R names by Δ, with a stated
uncertainty.* The six letters are the experiment; the delta, its cost and
latency, and its interval are the result. Every part of that scope is
part of the claim. The same tool can add twenty points to one agent,
nothing to a second, and three points to the first agent on another
domain, and none of those results contradicts another, because each
belongs to its baseline and its tasks. So the vocabulary has five objects,
not one. Artifacts exist; claims are evidence; adoption is policy.

| Object | What it is |
| --- | --- |
| Candidate capability | An exact, versioned artifact that might help |
| Capability claim | A measured delta with its whole scope, A, B, D, E, G, R, and its uncertainty |
| Reproduced capability claim | The same claim rerun by someone other than its author, with a compatible result |
| Externally validated capability claim | The improvement persisting on new tasks from the same distribution, written independently of the author |
| Adopted capability | A policy decision, taken on claims, to make the artifact part of a shared default |

Having a component installed, described, or demonstrated makes no claim.
And the effect is symmetric: admitting something can destroy capability as
easily as create it. On SkillsBench, 13 of 87 tasks show negative skill
deltas, the largest −7.4 points, with three repeatable causes: the skill
prescribes a heavier pipeline than the task needs, displaces a stronger
default strategy, or points the agent at a solver it can't debug
([Li et al., 2026](https://arxiv.org/abs/2602.12670)). Restraint, not
admitting or not invoking a harmful thing, is therefore as much a
capability as reach. There are five general sources:

| Source | What gets admitted |
| --- | --- |
| Tools and plugins | Code with typed operations and bounded access to the host, or a program that runs it |
| Skills | A written guide the agent reads before a task |
| Knowledge | Cited entries (methods, edge cases, known mistakes) retrieved and filtered for the task |
| Delegation | Another agent, briefed with selected evidence |
| Typed judgment | A fast, cheap decision that picks which of the above to use, and when |

Two boundary cases follow from the definition, rather than needing new
rows. First, **a capability need not be a single component.** An
orchestration policy over capabilities, such as planning a graph of
function calls and running independent ones in parallel
([Kim et al., 2024](https://arxiv.org/abs/2312.04511)), is itself a
candidate, measured the same way. Second, **knowledge the agent produced
itself is still a candidate.** Reflexion keeps an agent's own verbal
reflections on failed attempts in memory for its next try
([Shinn et al., 2023](https://arxiv.org/abs/2303.11366)); ExpeL extracts
insights from an agent's past trajectories and recalls them at inference,
with no weight update ([Zhao et al., 2024](https://arxiv.org/abs/2308.10144)).
Within one episode, a reflection is just the agent's reasoning. Once it is
stored, versioned, and admitted to a later run, it is a knowledge
component like any other, and the same test applies: it supports a
capability claim only if the with-and-without comparison shows one.
Self-authorship earns no exemption. On SkillsBench, skills the agent wrote
for itself before solving landed *below* the no-skills baseline on all
three configurations tested (−8.1 and −11.3 points on two of them), while
curated skills on the same configurations added 18.2 to 24.8 points
([Li et al., 2026](https://arxiv.org/abs/2602.12670)).

The last row is the one that makes the others usable. An agent with fifty
tools and no good way to decide which to use is worse than an agent with
none. A typed judgment answers a typed question (yes or no, a choice among
options, an ordered score) with probabilities, and ordinary code decides
what those probabilities cause; the judge writes no text and grants no
authority. The idea borrows the fast, automatic "System 1" of Kahneman's
*Thinking, Fast and Slow* (2011); the slow, effortful work is left to the
generator.

### What the word capability means here

*Capability* already has a precise, older meaning in computer security.
Dennis and Van Horn introduced it in 1966 for a multiprogrammed system: a
capability is an unforgeable token that both names an object and carries
the rights to use it, and a computation can act only on the objects it
holds capabilities for
([Dennis and Van Horn, 1966](https://dl.acm.org/doi/10.1145/365230.365252)).
The object-capability model builds whole languages and systems on that
discipline: authority is held only by reference, passed only by handing
over a reference, and can't be forged. Miller's dissertation on it starts
from a problem that is ours as much as his: when separately written
components are composed so that they can cooperate, they can instead
interfere destructively, so the job is to enable exactly the causality
the intended cooperation needs while suppressing the rest
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
ch. 1). Four of its ideas appear in the lexicon below: permission is not
authority (ch. 8), an arena with terms of entry (ch. 11), the reliance
set (ch. 5), and letting designation carry authority (chs. 3 and 22).
Agent security now uses the word in that sense. Odersky et al. put an
agent in a "safety harness" in which capabilities are program variables
that regulate access to effects and resources, tracked statically by Scala
3's type system, so the agent can't leak data or cause side effects it was
never handed
([Odersky et al., 2026](https://dl.acm.org/doi/10.1145/3786335.3813127)).

In this essay, *capability* means something else: a measured ability of an
agent, established by a claim of the kind above. The two meet at admission
and nowhere else. Admitting a test-time capability usually means granting
the component some authority (a directory it may read, a network it may
reach), and a claim's scope names that grant as G. But a grant says nothing
about whether the component helps, and a favorable delta grants nothing.
Throughout, *grant* or *authority* is the security sense and *capability*
the measured sense. (Our own NIP-CAP, in Part II, is named for the
authority sense: it describes an execution interface and the grant to use
it, and a definition there grants nothing.)

### The lifecycle in one figure

```text
candidate artifact         tool, plugin, skill, knowledge entry, delegate, judgment
        │
        ▼
identity and provenance    exact bytes, locked, signed
        │
        ▼
evaluation admission       a sandboxed grant, enough to measure it
        │
        ▼
controlled with-and-without run
        │
        ▼
capability claim: Δ ± uncertainty ─── no positive delta ───────▶ reject
  scope: A, B, D, E, G, R
        │
        ▼
independent reproduction   another party, same artifact, same protocol
        │
        ▼
external validation        new tasks the author didn't write or see
        │
        ▼
operational admission ─────────────── unsafe on the evidence ───▶ reject
  the grant real use would give it
        │
        ▼
adoption against current defaults ── regression in composition ▶ reject
        │
        ▼
credit                     for verification work and adoption
        │
        ▼
new default agent ──▶ harder tasks ──▶ new failures ──▶ new candidates
        ▲                                                      │
        └──────────────────────────────────────────────────────┘
```

Three exits, and none can be bought back by the others. A component that
the evidence shows unsafe is rejected however useful it is. One with no
positive delta is rejected however safe. One that regresses the default
set when composed with it is rejected however well it did alone. There
are two admissions because a candidate has to run somewhere before anyone
knows whether it is safe: the first is a sandbox that exists to produce
the evidence, and the second is the decision, on that evidence, to let it
run with real authority. The sandbox is not an implementation
convenience. It is the experimental frame in which the claim has
meaning, and its initial authority is part of the claim's scope.

### A lexicon of test-time capabilities

We propose eleven terms. Each has a way to be wrong. We tried to keep the
list short; a term earns a place only if it changes what someone building
or evaluating an agent does. Each entry says what the term is, why it
matters, and how anyone would measure it.

#### 1. Test-time capability (TTCap)

**Definition:** an ability an agent gains or loses at inference time,
without updating weights, because a component was admitted to the run. It
is stated only as a **capability claim**: admitting artifact A to locked
baseline agent B, under environment E and grant G, on task distribution
D, judged by rule R, changed the outcomes R names by Δ, with a stated
uncertainty. B is the whole executable agent, not "the agent minus the
component": model and version, system instructions, router, the existing
default set, sampling settings, runtime, and provider endpoint. Whatever
in B can't be pinned, such as the weights behind a hosted endpoint, the
claim should name as unpinned, because a model changing silently
underneath a claim invalidates its reproduction while every artifact
digest still matches.

**Why it matters:** it separates what a component *is* from what it
*does* for a particular agent on particular work. A component with no
claim yet is a *candidate* capability. A claim never says an artifact "is
a capability" in general; it says what admitting it did, and where.

**How to measure it:** a with-and-without evaluation on tests written for
what the component claims to help with, reported with every element of the
scope, so that a reader can tell which claims a new result agrees with and
which it doesn't.

#### 2. Capability admission

**Definition:** the host's decision that a specific, locked version of a
component may take part in a run.

**Why it matters:** a result is only meaningful about the exact bytes it
measured. Discovering, installing, enabling, granting access to, and
admitting a component are separate decisions; installing one should grant
nothing. And there are two admissions, not one. **Evaluation admission**
lets a candidate run under a deliberately constrained sandbox, with just
enough authority to measure it, so that the evidence about its utility
and its safety can exist at all. **Operational admission** is the later
decision, on that evidence, to let it run under whatever authority real
use would give it. Keeping them apart removes an apparent circle: safety
decides admissibility, and measuring safety requires some admission.
Miller describes the first of these exactly. You can't take rights away
from a component you don't control, but you can build an *arena*, a
controlled environment with initial conditions you set and rules you
enforce, and admit the component onto it under terms of entry: "please
leave your cellphones at the door" ([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
§11.3). An evaluation sandbox is an arena, and the grant it starts with
is its terms of entry.

**Grant is not authority.** A grant records the operations directly made
available to a component. Its *effective authority* is the set of effects
it can cause through those operations and through every other component
it can reach. Miller's example is a Bob with no permission to write a
file who nevertheless has the authority to write it, because he can ask
an Alice who does and who will; authority derives from the structure of
permissions *and* from the behavior of what sits on the permitted paths
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
§8.1). So the safety question at admission is not "what was it granted?"
but "what effects could it cause under that grant, given the components
already present?" A claim's G names the grant; admission safety bounds
the effective authority, which the grant alone doesn't show.

Admission is also a security boundary, not only a performance decision.
A tool that reads untrusted data (email, web pages, files someone else
wrote) is a path for prompt injection, in which data a tool returns hijacks
the agent into a task nobody asked for. AgentDojo measures exactly this,
with 97 realistic tasks and 629 security test cases for agents that call
tools over untrusted data
([Debenedetti et al., 2024](https://arxiv.org/abs/2406.13352)). So a
candidate should be evaluated for utility *and* for the authority it
exercises, and a component's own description of itself is not evidence:
the Model Context Protocol tells clients to treat tool annotations as
untrusted unless they come from trusted servers
([MCP specification, 2025](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)).
Safety is a constraint here, not a score. Utility establishes the claim;
safety decides operational admissibility; and a dangerous component can't
make up for the danger by being useful enough. Capability evidence is
never permission to run.

The idea of naming exact bytes has a long lineage in software supply
chains. in-toto lets an end user verify each step that produced a piece of
software ([Torres-Arias et al., 2019](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias));
SLSA provenance is "verifiable information about software artifacts
describing where, when and how something was produced," written as an
in-toto attestation ([SLSA, v1.0](https://slsa.dev/spec/v1.0/provenance));
Sigstore makes signing those artifacts cheap
([Newman et al., 2022](https://dl.acm.org/doi/10.1145/3548606.3560596)).
Capability admission applies reproducible artifact provenance to agent
capabilities.

**How to measure it:** record a digest of the locked component set in every
result, so a result names exactly what it measured.

#### 3. Capability delta

**Definition:** the estimated treatment effect of admitting the component:
the difference in outcome between the *with* arm (the component admitted)
and the *without* arm (the baseline), on the same tests, with its
uncertainty, from enough repeats to estimate it.

**Why it matters:** it is the size and sign of the claim. It holds only
for the baseline it was measured against: a tool can add a lot to an agent
that otherwise can't see the files, and little to one that can. A negative
delta is a finding about the component, not a failure of the evaluation.

A delta has more than one outcome, and the rule R says which one the
claim is about. Observed cost and latency are results of the experiment,
not part of its scope; what belongs in the scope is a **predeclared
primary outcome** and **non-inferiority bounds** on the others. The usual
claim names correctness as primary and requires cost and time not to be
materially worse. But "the same correctness at a fifth of the cost" is a
legitimate claim too, with cost as the primary outcome and correctness
held non-inferior. What can never be a claim is "faster, and wrong": a
primary outcome of speed with no bound on correctness.

**How to measure it:** run both arms on the same tests several times and
report paired outcomes per test. The honest report has an effect size and
an interval, the number of tests, the number of repeats, the variance
between tests and between repeats, and the smallest effect worth
detecting. Because the design is tests crossed with repeated stochastic
runs, the task is the unit that generalizes: the interval should
eventually be hierarchical over tasks and repeats, and a paired bootstrap
over per-test differences is only a floor. A written, versioned rule then
turns the estimate into a verdict for a decision, for example "better only
if more tests pass and the gain clears the spread between repeats, with
cost and time within a stated bound." That rule is an engineering gate.
Its threshold is not the definition of the delta, and a gate should say
what it can't see.

Repeats matter because agents are inconsistent. τ-bench grades an agent by
the database state it leaves behind, not its text, and its pass^k metric
asks whether the agent succeeds on *all* of k trials; even strong
function-calling agents scored pass^8 below 25 % in its retail domain
([Yao et al., 2025](https://arxiv.org/abs/2406.12045)). Final pass rates
can also hide where a capability helps: AgentBoard's progress rate
measures incremental advancement through a multi-turn task
([Ma et al., 2024](https://arxiv.org/abs/2401.13178)), which suggests
process-level deltas (fewer wasted actions, earlier recovery, the right
tool sooner) alongside final ones. Paired evaluation of a single component
type already exists for agent skills: SkillsBench runs matched no-skills
and curated-skills conditions and reports curated skills raising the
average pass rate from 33.9 % to 50.5 %, with gains ranging from 4.1 to
25.7 points by configuration ([Li et al., 2026](https://arxiv.org/abs/2602.12670));
ACES measures "skill lift" from paired live trials of 145 skills from
enterprise repositories and public catalogs ([Kevin et al., 2026](https://arxiv.org/abs/2608.20614)). The
spread across configurations is the point: a delta belongs to its
baseline.

#### 4. Reach and restraint

**Definition:** *reach* is how often the host exposes and the agent uses a
candidate on tests where its conditional delta is positive; *restraint*
is how often the host withholds it, or the agent leaves it alone, on tests
where its conditional delta is negative or zero. Both are properties of
the relationship between the agent, its selector, and the component, not
of the component alone.

**Why it matters:** an installed capability the agent never invokes
changes no outcome, so it measures as no capability at all. One it invokes
everywhere can make unrelated work worse. Because admission is symmetric,
restraint is not a courtesy: on tasks where a component hurts, withholding
it *is* the capability, and that capability belongs to the router or the
policy that withheld it, not to the component. This matters most once
capabilities interact, because a new default can change where an old one
is reached for.

Selection becomes its own problem as the set of capabilities grows.
ToolLLM trained a retriever to pick among 16,464 real APIs
([Qin et al., 2024](https://arxiv.org/abs/2307.16789)); AnyTool used a
hierarchical retriever, a solver, and self-reflection that re-selects when
a first attempt fails ([Du et al., 2024](https://arxiv.org/abs/2402.04253));
Gorilla paired a model with retrieval over API documentation so it can
follow API changes at test time and hallucinate fewer calls
([Patil et al., 2024](https://arxiv.org/abs/2305.15334)). Their shared
lesson is that *having* a capability available and *successfully
selecting and calling it* are different things. Reach can fail inside a
capability too: on SkillsBench, agent runs satisfied only 38.66 % to
45.51 % of the behavioral constraints extracted from the skills they were
given ([Tan et al., 2026](https://arxiv.org/abs/2606.20659)). Nor is the
agent's own account of its use evidence. Hu et al. find that what an agent
says it used and what actually changed its decision come apart, and that
mentions in the transcript, trace similarity, and LLM judges all fail to
detect real reliance ([Hu et al., 2026](https://arxiv.org/abs/2607.27484)).

**How to measure it:** mark each test should-use or should-not-use, and
report outcomes per test in both arms. Reach is read from those outcomes,
not from the transcript's claims.

#### 5. Judgment budget

**Definition:** the time and money a system spends deciding *how* to answer
before it spends anything on answering. The judgment must be much cheaper
than the work it can avoid.

**Why it matters:** it is the per-question allocation lesson of test-time
compute, applied one level up: before allocating thinking, decide whether
the turn needs a large model at all, and which capability should handle it.

Model routing and cascades are the close prior art. FrugalGPT learns which
combination of models to query for each input, and reports matching the
best single model with up to 98 % lower cost
([Chen et al., 2024](https://arxiv.org/abs/2305.05176)); RouteLLM learns
from preference data when a request needs the stronger of two models, and
reports cost reductions of over two times in some cases without hurting
quality ([Ong et al., 2025](https://arxiv.org/abs/2406.18665)). The
proposal here is not query routing as such. It is treating one cheap,
typed judgment as a general allocation mechanism over *every* capability
source: models, tools, knowledge, and delegation.

**How to measure it:** the judgment's latency and cost, and the precision of
whatever it serves without calling the large model.

#### 6. Test-time delegation

**Definition:** acquiring another agent's capability for one task by
handing it a prepared briefing, then recording what it did as part of the
same task.

**Why it matters:** the strongest capability available for a task is
sometimes another agent. Treating it as a capability means its
contribution is measured, not assumed.

Delegation is also where authority is allocated, and there is a right way
to do it. **Let designation carry authority.** Miller's example is two
ways to copy a file: `cp foo.txt bar.txt` receives two *names* and so
needs authority over the whole namespace that resolves them, while
`cat < foo.txt > bar.txt` receives two already-resolved descriptors and
needs authority over exactly those two files. The least authority a
component needs depends on how it is told what to act on, and it is
smallest when the act that designates a resource hands over the narrow
right to that resource at the same moment, just in time, rather than
granting a namespace just in case
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
§§3.1–3.2, 22.2). For an agent: when the judgment selects the three files
a delegate should read, the host should grant those three, not the
repository they sit in. The same discipline runs down the chain. No
central actor knows enough to compute least authority for every
participant; authority attenuates at each hand-off, user to agent to
delegate to tool to resource, with a rule that a child receives no more
than its parent held except by an explicit new grant from a principal
entitled to give one (§§20.1, 22.2).

Delegation as a mechanism is well explored: HuggingGPT used a language
model as a controller that plans, picks specialist models, and runs them
([Shen et al., 2023](https://arxiv.org/abs/2303.17580)), and AutoGen
builds applications from agents that converse to finish a task
([Wu et al., 2023](https://arxiv.org/abs/2308.08155)). The added
requirement here is that a delegate's contribution gets the same
with-and-without accounting as a tool's.

**How to measure it:** the same outcome, cost, and time as any attempt,
compared with the attempt made without delegating.

#### 7. Reproduced capability claim

**Definition:** a capability claim whose result was reproduced by someone
other than the person who first ran it: a different evaluator, and A, B,
D, E, G, and R all held fixed, with a compatible result.

**Why it matters:** confidence should come from reproduction by someone
else, not from the author's report. Reproduction is the verifier that
decides which claims deserve to spread. It answers one question only: can
someone else get this result? It does not say whether the result was
fitted to the tests; that is the next term's job.

**How to measure it:** an independent rerun that publishes its own result,
citing the original, and confirms on a matching verdict or disputes
otherwise, with both kept visible.

**Every claim has a reliance set.** Miller defines a program's reliance
set as everything whose correct behavior its own correct behavior depends
on, and notes that whatever lies in the reliance sets of a whole group is
a central point of failure for the group: a shared platform is one for
everything that runs on it
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
§§5.1–5.2). A claim's reliance set holds the runner, the model provider,
the agent build, the selector, the grader, and the host. Reproduction by
a different evaluator removes one element, the original evaluator, and
leaves the rest. So reproduction comes in degrees: a second person on the
same infrastructure is independent as a signing principal and not as a
platform, and a stronger reproduction varies more of the set: another
host, another provider, another model implementation, another grader,
another runtime. Three reruns on one platform are not three independent
replications, and a claim should say what its reruns shared.

Two traditions meet here. Reproducible builds let anyone rebuild a binary
from its source and check it matches bit for bit, so trust doesn't rest on
one builder ([Lamb and Zacchiroli, 2022](https://arxiv.org/abs/2104.06020)).
And the evaluator is itself something to evaluate: Agent-as-a-Judge uses
an agentic evaluator that inspects intermediate steps, not only the final
output, and checks that judge against human judgments
([Zhuge et al., 2025](https://arxiv.org/abs/2410.10934)). A reproduced
claim needs both: a rerun by someone else, and a grader that has itself
been checked.

#### 8. Externally validated capability claim

**Definition:** a reproduced claim whose improvement persists on new
tasks from the same intended distribution D, written independently of the
artifact's author and not available to the author before the artifact
was locked.

**Why it matters:** reproduction proves reproducibility, not external
validity. An author writes a tool, writes six tests for it, and three
other people rerun the same six tests: that is reproducible, and it says
nothing about whether the tool was fitted to those six. A suite can be
fitted to its tool as easily as a model to a benchmark. External
validation belongs *before* adoption, not after it, because after
adoption everyone already has the thing. It is also what a leaderboard
can't supply and a network can: someone else's tests.

Three levels are worth keeping apart, because each changes a different
part of the scope. *Reproduction* holds A, B, D, E, G, and R fixed and
changes only who runs it. *External validation* keeps D and changes the
sampled tasks. *Transfer* deliberately changes D to a different
distribution D′, and is a new claim, not a stronger version of the old
one: a repository-mapping tool validated on more repositories has been
externally validated; the same tool measured on spreadsheet tasks has
been tested for transfer.

**How to measure it:** a with-and-without result on a second test set
with its own claim scope. A different author is provenance, not
independence. What independence needs is chronology and information flow:
the artifact was locked before the suite was revealed to its author, or
the suite was hidden, so that the author could not tune against it. The
externally validated delta is usually smaller than the original; how much
smaller is the finding.

#### 9. Capability adoption

**Definition:** making an externally validated claim's artifact part of
the agent everyone starts with, on the strength of its marginal effect
against the current default set. Adoption is a status conferred on the
artifact; it is the one object in the list that is a policy decision
rather than evidence.

**Why it matters:** adoption is how one person's reproduced result becomes
every user's default, without a training run. And it changes the question.
A standalone delta, measured against a baseline with nothing admitted, is
history. What adoption decides on is the marginal claim: current defaults
plus the candidate, against current defaults alone. The two come apart for
five reasons, and each is measurable:

| Effect | What happens |
| --- | --- |
| Redundancy | The candidate does what a default already does; its standalone delta was real and its marginal delta is near zero |
| Synergy | The candidate helps more with a default present than alone, for example a finder that a reader can then act on |
| Interference | Two components that each help alone do worse together |
| Routing competition | Adding the candidate makes an existing default harder to select, or the reverse; reach falls somewhere else |
| Context cost | Every admitted component's description consumes budget on every turn, whether or not it fires |

**Adoption evaluates marginal capability, not historical capability.** A
plugin leaderboard ranks standalone deltas; a default set is a composition,
and only the marginal delta says whether the composition improved.

The same is true of authority, and it is the deeper reason the marginal
question can't be skipped. Because effective authority runs through
paths between components, adding a candidate whose own grant is small
can connect two paths that were separate and enlarge what the default
set can cause, with no grant changed anywhere. Conversely, a narrow
component that replaces a broad one can shrink it. Miller's argument
that least authority practiced at every level of composition compounds
into a multiplicative reduction of the attack surface runs the other way
too: excess authority admitted at one level compounds as well
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/),
§§21, 22.4). So **adoption evaluates marginal utility and marginal
authority**, current defaults plus the candidate against current
defaults on both, with safety kept a constraint rather than traded
against the delta.

**How to measure it:** the marginal delta, current defaults plus the
candidate against current defaults; whether the whole default set still
passes what it passed before; whether the candidate keeps its externally
validated delta in the composition; and whether the composition's
effective authority grew, which means tracing the paths the newcomer
opens, not reading its grant.

#### 10. Capability credit

**Definition:** recognition that goes to the people whose work made a
capability claim real, for the events that show the work was used, and
for verification work whichever way it came out.

**Why it matters:** credit for activity (runs, publishes, downloads)
rewards components that exist; credit for independent reproduction and
adoption rewards components that help. But a rule that pays only
*confirming* reruns builds a quiet preference for agreement into the
network. A competent evaluator who finds that a celebrated claim doesn't
reproduce has contributed more information than the fourth person who
confirms it. So the principle is **credit verification work, not
agreement**: a rerun that followed the protocol earns credit whether it
confirms or disputes. Adoption still requires positive evidence; the
checker's credit does not depend on the direction of the result.

**How to measure it:** credit records that anyone can recompute from the
public rerun and adoption events, with disputes paid at the same rate as
confirmations.

#### 11. The capability flywheel

**Definition:** the loop in which people add capabilities, tests produce
claims, others reproduce them, tests the author didn't write validate
them, and adoption hands them to every agent, which then takes on harder
tasks that reveal the next missing capability.

**Why it matters:** it is the mechanism by which a network of contributors
could improve an agent faster than one team can.

**How to measure it:** not participant counts, but *incremental externally
validated passes per adopted contribution*, with cost, latency, and
harmful regressions reported alongside.

### Evals as the unit of account

If capabilities are acquired and shared, something has to decide which ones
actually improve an agent's results. The proposal is that the capability
claim is the unit of account, that a per-component with-and-without
evaluation is what produces one, and that three of its properties do most
of the work.

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
this thing help, where, and at what cost?* Benchmarks ask how capable an
agent is; a claim says what caused it to become more capable. Benchmarks
remain how an agent is checked as a whole; evals are how to decide what
goes into it.

A claim answers only the first of three questions an operator asks about
a component, and the three should never be folded into one score.

| Question | Answered by | On what |
| --- | --- | --- |
| Does it work? | The evaluation, under evaluation admission | The claim: delta, scope, uncertainty |
| May it run? | Operational admission | The grant it needs and its exposure to injected instructions, as the sandboxed evaluation showed them |
| Should everyone get it? | Adoption | The marginal claim against the current defaults, externally validated |

Utility establishes the claim; safety decides operational admissibility;
adoption is a decision on both. Keeping them apart is what stops a combined score from
adopting a dangerous, useful thing, or a safe, useless one.

Evals also have failure modes. A grader is a piece of software and can be
wrong, which can flip a verdict by chance. It deserves the same scrutiny,
and the same versioning, as the component under test. The tool-use
literature has a clean example. ToolBench's original pass rate counted
queries judged "non-solvable" as passes, so when the tool retriever
returned irrelevant candidates, unsolved queries were labeled non-solvable
and the rate went *up*: with randomly chosen APIs it reached 99.0 %.
AnyTool's authors found this, computed the rate over solved and unsolved
queries only, and kept only queries the tool pool could solve
([Du et al., 2024](https://arxiv.org/abs/2402.04253)). A metric that
rewards failing to reach the right tool is the worst case for a
capability eval.

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
   correctness. Under a rule whose primary outcome is correctness, time and
   cost are notes beside the verdict, never the verdict. A claim may name
   cost as its primary outcome instead, with correctness held non-inferior;
   what it may never do is drop the bound, because a faster wrong answer
   isn't a capability. LATM is an
   early instance: a strong model writes a reusable tool once, and a
   lighter model uses it, matching the strong model in both roles at lower
   cost ([Cai et al., 2024](https://arxiv.org/abs/2305.17126)).

### How capabilities compound across a network

Test-time capabilities make network effects possible in principle. Weights
improve when a lab trains them, on the lab's schedule. A test-time
capability can come from anyone, be tested by anyone, and, once adopted,
reach every agent that uses the same defaults without a training run. The
unit that compounds is not a longer prompt or a count of packages. It is a
**capability claim with independent evidence**: an exact component
version, a with-and-without result, confirming reruns by people who didn't
write it, and a delta that survives tests they wrote.

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

### Related work and prior art

Every mechanism in the lexicon has prior art. This section groups it by
theme and says, for each, what the lexicon takes from it and what it adds.

**Acting and tools.** ReAct interleaves reasoning traces with actions
against external sources, and reports absolute success-rate gains of 34 and
10 points over imitation and reinforcement learning baselines on two
interactive benchmarks, prompted with one or two examples
([Yao et al., 2023](https://arxiv.org/abs/2210.03629)). Toolformer teaches
a model to decide when to call an API
([Schick et al., 2023](https://arxiv.org/abs/2302.04761)). Agents can make
their own tools: LATM has a strong model write reusable tools for a lighter
one ([Cai et al., 2024](https://arxiv.org/abs/2305.17126)), and CRAFT
builds toolsets from solutions, with a validation step for correctness
before a snippet enters the set
([Yuan et al., 2024](https://arxiv.org/abs/2309.17428)). SWE-agent shows
that the interface an agent is given changes what it can do: a custom
agent-computer interface for editing, navigating, and testing a repository
([Yang et al., 2024](https://arxiv.org/abs/2405.15793)). The lexicon takes
these as the things being admitted; what it adds is the with-and-without
test each must pass.

**Memory and experience.** Reflexion stores verbal reflections on task
feedback in memory for later attempts, and reports 91 % pass@1 on
HumanEval ([Shinn et al., 2023](https://arxiv.org/abs/2303.11366)). ExpeL
gathers experience on training tasks, extracts natural-language insights,
and recalls them at inference without parametric updates
([Zhao et al., 2024](https://arxiv.org/abs/2308.10144)). Voyager grows a
library of executable skills ([Wang et al., 2023](https://arxiv.org/abs/2305.16291)).
These show the mechanism behind a knowledge capability predates this
vocabulary. What the lexicon adds is provenance, versioning, an
independent with-and-without result, and an adoption step; see the
[boundary case](#the-thesis-capability-is-something-you-can-acquire-at-test-time)
on self-produced knowledge.

**Selection at scale.** ToolLLM ([Qin et al., 2024](https://arxiv.org/abs/2307.16789)),
AnyTool ([Du et al., 2024](https://arxiv.org/abs/2402.04253)), and Gorilla
([Patil et al., 2024](https://arxiv.org/abs/2305.15334)) treat choosing
among thousands of APIs, and calling them correctly, as a problem of its
own. That is [reach and restraint](#4-reach-and-restraint) at scale.

**Routing and cascades.** FrugalGPT ([Chen et al., 2024](https://arxiv.org/abs/2305.05176))
and RouteLLM ([Ong et al., 2025](https://arxiv.org/abs/2406.18665)) spend
a strong model only where a cheap one won't do. The
[judgment budget](#5-judgment-budget) generalizes the decision from
*which model* to *which capability*.

**Orchestration and delegation.** LLMCompiler plans function calls as a
graph and runs independent ones in parallel, reporting up to 3.7 times
lower latency, 6.7 times lower cost, and about 9 % higher accuracy than
ReAct ([Kim et al., 2024](https://arxiv.org/abs/2312.04511)). HuggingGPT
([Shen et al., 2023](https://arxiv.org/abs/2303.17580)) and AutoGen
([Wu et al., 2023](https://arxiv.org/abs/2308.08155)) coordinate models
and agents. A capability can be an orchestration policy over other
capabilities, measured as one unit.

**Evaluation reliability and process metrics.** τ-bench checks the final
database state and introduces pass^k for consistency across trials
([Yao et al., 2025](https://arxiv.org/abs/2406.12045)). AgentBoard adds a
progress rate beyond final success ([Ma et al., 2024](https://arxiv.org/abs/2401.13178)).
AnyTool found an evaluation protocol that inflated pass rates and revised
it ([Du et al., 2024](https://arxiv.org/abs/2402.04253)). For skills
specifically, SkillsBench ([Li et al., 2026](https://arxiv.org/abs/2602.12670))
and ACES ([Kevin et al., 2026](https://arxiv.org/abs/2608.20614)) already
run paired with-and-without trials, and skill coverage measures whether a
run followed the skill it was given
([Tan et al., 2026](https://arxiv.org/abs/2606.20659)). These are the
closest prior art to the [capability delta](#3-capability-delta). They
evaluate one component type; the lexicon applies the same test to every
source and attaches it to identity, reproduction, and adoption.

**Evaluators.** Agent-as-a-Judge evaluates agents with agents that inspect
the intermediate process, on 55 development tasks with 365 hierarchical
requirements ([Zhuge et al., 2025](https://arxiv.org/abs/2410.10934)). It
is the direction for "evaluate the evaluator."

**Security of admission.** AgentDojo shows that tools over untrusted data
are a prompt-injection surface ([Debenedetti et al., 2024](https://arxiv.org/abs/2406.13352)).
A survey of agent skills reports a campaign in which nearly 1,200
malicious skills entered a major agent marketplace
([Jiang et al., 2026](https://arxiv.org/abs/2602.20867)), and MalSkillBench
finds that its strongest skill-specific detector, at 98.4 % recall on
code injection, collapses on prompt-injection and agent-control attacks
([Guo et al., 2026](https://arxiv.org/abs/2606.07131)). Admission has to
weigh authority and safety, not utility alone, and a static scan is not
enough.

**Provenance.** in-toto ([Torres-Arias et al., 2019](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias)),
SLSA provenance ([SLSA, v1.0](https://slsa.dev/spec/v1.0/provenance)),
Sigstore ([Newman et al., 2022](https://dl.acm.org/doi/10.1145/3548606.3560596)),
and reproducible builds ([Lamb and Zacchiroli, 2022](https://arxiv.org/abs/2104.06020))
give software artifacts verifiable identity, lineage, signatures, and
independent rebuilds. Locks, digests, signed releases, and rerunnable
evidence for capabilities are the same idea applied to agents.

**Authority and composition.** Capability-based security gives admission
its other half: Dennis and Van Horn's unforgeable tokens of authority
([1966](https://dl.acm.org/doi/10.1145/365230.365252)), and, for agents,
capabilities tracked in a type system so an agent can act only on what it
was handed ([Odersky et al., 2026](https://dl.acm.org/doi/10.1145/3786335.3813127)).
Miller's *Robust Composition* is the closest conceptual predecessor of
the admission and composition half of this essay rather than prior art
against the claim half: it asks how independently written, possibly
hostile components can be given exactly the interactions their
cooperation needs and no more, and answers with permission distinguished
from authority, arenas with terms of entry, reliance sets, designation
that carries authority, interfaces split so that each distinct authority
is a distinct object, and least authority nested at every scale
([Miller, 2006](https://papers.agoric.com/papers/robust-composition/abstract/)).
The lexicon takes each of those where it bears on admission,
delegation, reproduction, and adoption. The
[terminology note](#what-the-word-capability-means-here) says how that
sense of the word and this essay's relate.

**What we did not find.** Surveys of skill libraries already call for
provenance, rollback, and reporting standards
([Li, 2026](https://arxiv.org/abs/2607.10113)). Within our search we found
no work that joins the whole chain, artifact → admission → controlled
delta → reproduction → external validation → adoption → credit, across
tools, skills, knowledge, delegation, and judgment. The table below is the claim
made auditable: a cell says *yes* only where we read it in the source, and
*not described* where the source is silent, which is not the same as
absent. The last row uses *built* and *proposed* rather than a check mark,
because a mechanism that exists and a mechanism that has been used are
different things, and Part II says which is which.

| Work | Exact artifact identity | Paired with-and-without delta | Reproduction by others | External validation | Adoption into a shared default | Credit | Capability types |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SkillsBench ([Li et al., 2026](https://arxiv.org/abs/2602.12670)) | not described | yes: matched no-skills and curated-skills conditions | not described | not described | not described | not described | skills |
| ACES ([Kevin et al., 2026](https://arxiv.org/abs/2608.20614)) | current repository state, not a pinned version | yes: paired live trials | not described | not described | thresholds left to each team; no shared default described | not described | skills |
| ToolBench and AnyTool ([Qin et al., 2024](https://arxiv.org/abs/2307.16789); [Du et al., 2024](https://arxiv.org/abs/2402.04253)) | not described | no: methods compared over one API pool | no per-component claim to reproduce | not described | not described | not described | tools |
| in-toto, SLSA, Sigstore, reproducible builds | yes: digests, attestations, signatures | no | yes: independent rebuilds | no | no | no | software artifacts, not agent components |
| This proposal | built: locks, digests, signed releases | built | built: checks by a different trainer | proposed, not built | built, none made | built for confirming checks and adoptions; disputes unpaid today, which Part III proposes to change | tools, plugins, skills, and knowledge entries through one report; delegation and judgment measured separately today |

ACES, the closest, runs paired trials on the current repository state and
leaves thresholds to teams; it describes no pinned versions, no third-party
reproduction, and no credit. We'd welcome pointers to work we missed, and
corrections to any cell.

### Open questions for the field

- **External validation before adoption.** How large must a second test
  set be, and what chronology (artifact locked before the suite was
  revealed) or hiding makes it independent of the author, for a reproduced
  claim to count as externally validated? How much of the original delta
  should be expected to survive, and when is a change of task distribution
  transfer rather than validation?
- **Pinning the baseline.** B is the whole executable agent, and parts of
  it, such as the weights behind a hosted endpoint, can't be pinned by
  digest. What should a claim record about them, and when does a silent
  change upstream void a reproduction?
- **Incentives for disagreement.** If reruns earn credit whichever way
  they come out, what stops low-effort disputes, and what counts as a
  rerun that followed the protocol?
- **Reach.** How should capabilities be described to the judgment that
  picks them, so the agent reaches for them when it should? This may be the
  cheapest gain available.
- **Which baseline.** A claim belongs to its baseline, and a capability
  measured against a weak one can shrink against a strong one. Which
  baselines should a shared claim be stated against, and how should a
  reader combine claims made against different ones?
- **Marginal deltas.** Adoption should measure the candidate against the
  current default set, not against nothing. That needs a baseline arm that
  holds the defaults, a regression check across the whole set, and a way to
  tell redundancy, interference, and routing competition apart.
- **Marginal authority.** A candidate's grant doesn't say what the
  composition can cause once it's admitted. How should the effective
  authority of a default set be bounded and compared before and after a
  candidate joins, and what record would let a reader check the bound?
- **Independence of reruns.** Which elements of a claim's reliance set
  must differ between reruns before they count as independent
  replications, and how should a claim record what its reruns shared?
- **Cost.** Every result should carry a price for both arms, not a guess.
- **Uncertainty and power.** A handful of tests and repeats is enough to
  see a large change and too few to see a small one. What interval is
  honest for a few binary tests and a few repeats, what is the smallest
  effect worth detecting, and how should test sets grow as the deltas
  people care about shrink?
- **Grader quality.** Graders are code and make mistakes. How should they
  be checked the way results are?
- **Process-level deltas.** Final pass rates hide where a capability
  helps. Which trajectory measures (wasted actions, recovery, the step at
  which the right tool is chosen) are stable enough to report as deltas?
- **Safety of admission.** How should a candidate's delta be paired with a
  measure of the authority it exercises and its exposure to injected
  instructions, with safety as a constraint that utility can't buy back,
  so a helpful but unsafe component isn't adopted?
- **Compute and capability together.** How do test-time capabilities
  interact with more thinking? Can a tool let a cheaper model with less
  reasoning match a stronger one, and when does extra reasoning still pay on
  top of a tool?
- **Network evidence.** Does the flywheel turn? Its measure, incremental
  externally validated passes per adopted contribution, should be reported
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
| [Reproduced capability claim](#7-reproduced-capability-claim) | [Eval checks](../extensions/evaluation.md#checks-adoption-and-credit) | [NIP-EVAL checks](../../nips/openagents/NIP-EVAL.md#checks) `3189` | Implemented |
| [Externally validated capability claim](#8-externally-validated-capability-claim) | Nothing yet; a second suite by another author would be an ordinary report on the same subject | [NIP-EVAL reports](../../nips/openagents/NIP-EVAL.md#reports) `3189`, with no independence marker | Defined |
| [Capability adoption](#9-capability-adoption) | [`openagents:coder-defaults`](../../packages/coder-defaults/) releases; no marginal baseline arm yet | [NIP-EVAL adoption](../../nips/openagents/NIP-EVAL.md#adoption), [NIP-EXT](../../nips/openagents/NIP-EXT.md) `3184` | Partial |
| [Capability credit](#10-capability-credit) | XP referee, `crates/xp-ledger` | [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check) `3193`, `3194` | Implemented |
| [Capability flywheel](#11-the-capability-flywheel) | Chat, Gym, and Verse; [how the network compounds](../coder/design/networked-coder-plan.md#how-the-network-compounds) | No single carrier | Defined |

The full protocol mapping, stage by stage and field by field, is
[below](#how-the-protocol-carries-test-time-capabilities).

### What "is a capability" means in our system

Part I's unit of account, the capability claim, is a
[NIP-EVAL extension evaluation](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile)
report in our system, and the claim's scope is written into the report
rather than left to the reader:

| Scope | In the report |
| --- | --- |
| A, the artifact | The `subject` arm's lock: the exact extension release and its dependencies |
| B, the baseline agent | The `baseline` arm's lock (Coder with no extension admitted) plus the run config: the door URL and model name the harness pinned in the child, and Coder's own bounds. Not pinned: the weights behind the door, and the version of Jev behind the decision door. A hosted model changing underneath us would void a reproduction while every digest still matched, and today the report would not show it. |
| D, the tasks | The suite release the report cites, case by case |
| E and G, environment and grant | The run config and the grant it ran under (in the hosted runs, no shell) |
| R, the rule | The gate digest in `acceptance`; `ext-eval-v2` fixes tests passed as the primary outcome with cost and time held not materially worse |
| The result | `change` on the `comparison` arm for `cases_passed` and `mean_score`, with `cost_usd` and `seconds` per arm (unknown where a lane isn't priced); no interval yet |

A claim decides three concrete things in OpenAgents. Installing a tool,
describing it, or demoing it decides none of them.

1. **The Gym's gate rates it Better.** `openagents ext eval` and the hosted
   runner run every test in both arms, and the
   [`ext-eval-v2` gate](../../crates/gym/gates/ext-eval-v2.json) reads
   **Better** only when more tests pass with the tool, the score gain clears
   the spread between repeats, and cost and time stay within 1.5 times plus
   the spread. It reads **Worse** when fewer tests pass or a should-not-fire
   test is lost, and **No clear change** otherwise.
2. **A different trainer's check reproduces it, and XP is paid.** An
   [eval check](../extensions/evaluation.md#checks-adoption-and-credit)
   reruns the published suite and publishes its own
   [NIP-EVAL check](../../nips/openagents/NIP-EVAL.md#checks). Today only
   a confirming check earns
   [NIP-XP `eval-check`](../../nips/openagents/NIP-XP.md#eval-check)
   credit; Part I's rule says a disputing check that followed the protocol
   should earn the same, and Part III lists that change.
3. **It can be adopted into every Coder's defaults.** Today's
   [operator policy](../extensions/evaluation.md#checks-adoption-and-credit)
   makes a tool an adoption candidate when its result is **Better** and at
   least three distinct trainers' checks confirmed it. That is reproduction,
   on the author's own suite. Part I puts external validation before
   adoption, and our policy doesn't require it yet; Part III lists that as
   the change to make before the first adoption. Adoption
   itself is an operator's
   [NIP-EVAL adoption](../../nips/openagents/NIP-EVAL.md#adoption) decision
   and a new [`coder-defaults`](../../packages/coder-defaults/) release, and
   pays [NIP-XP `eval-adopt`](../../nips/openagents/NIP-XP.md#eval-adopt)
   credit.

A component with no such report is a candidate: the gate hasn't rated it,
no check can confirm it, and it can't be adopted. And a report that rates
it **Better** is a claim about that scope: three tools, six tests each,
one baseline, one grant.

### Each term in OpenAgents

#### Test-time capability in OpenAgents

All five sources exist in the system. They do not yet share one
evaluation carrier: extensions and knowledge entries are measured by the
same with-and-without report, while delegation and judgment are measured
separately, as the sections below say.

| Source | What gets admitted | Where it lives |
| --- | --- | --- |
| Tools and plugins | A Wasm guest with typed operations and bounded host access, or a program that runs one | [Wasm plugins](../extensions/plugins.md), `crates/plugin` |
| Skills | A `SKILL.md` guide the agent reads before a task | [Plugins and skills](../glossary.md#plugins-and-skills) |
| Knowledge | Cited entries (methods, edge cases, slips) retrieved and filtered by Jev | [Knowledge base](../coder/design/knowledge-base.md) |
| Delegation | Another agent, briefed with evidence Jev chose | [The delegate door](../coder/runtime/delegate-door.md) |
| Typed judgment | A System One answer that picks which of the above to use, and when | [The chat router](../coder/design/2026-09-28-chat-router.md) |

Concretely: an extension (tool, plugin, skill, or package) admitted to a
Coder turn, measured by an [extension eval](../extensions/evaluation.md);
a knowledge entry retrieved into a Microcoder step, whose evidence is the
same report; a delegate briefed by Coder One, measured today on
Terminal-Bench attempts rather than by a paired eval; and Jev's judgment,
measured on the router's held-out set.
[Jev](../glossary.md#decision-models-and-runtimes) is TypeSafe's System
One model: it answers typed questions with probabilities, and code decides
what those probabilities cause. It writes no text and grants no authority.

#### Capability admission in OpenAgents

Discovery, installation, enablement, grants, and admission are separate
decisions in our [extension architecture](../extensions/architecture.md).
An eval run holds the exact extension and Coder's question sets in its run
locks, and every report records the lock digest. Installing a package
grants nothing. The package design records exact resolved digests and
source provenance in an installation lock
([packages](../extensions/packages.md#identity-dependencies-and-locks)),
which is SLSA-style provenance applied to agent components; the plugin
build receipt today holds only three fields, and full
[build provenance](../extensions/plugins.md#authoring-and-build-provenance)
isn't built yet. Part I's two admissions both exist here, under other
names. Evaluation admission is the eval run's sandbox: every run's child
process is confined by `coder-boundary`, and a hosted request may name
only read-only and sandbox-write effects, so a candidate is measured
under less authority than real use would give it. Operational admission
is the grant an extension runs under in an ordinary Coder turn, which the
extension architecture keeps separate from installation and enablement.
What's missing is the evidence that should connect them: grants bound
what a Wasm guest can touch, but our extension evals don't yet include
prompt-injection cases of the kind AgentDojo measures, so operational
admission today rests on the grant's bounds alone, which is to say on
permission, not on a bound of effective authority.

Part I's "let designation carry authority" is built in one place and not
in another. A Wasm guest never sees a path: the host mints an opaque
handle per invocation for each entry it lists, scoped to that invocation,
and a handle from another invocation is stale
([what is built](../extensions/plugins.md#what-is-built)).
That is the `cat` discipline. The delegate door is still `cp`: Jev
chooses the briefing, at most 12,000 characters of selected evidence, but
the delegate's grant is read-only or workspace-writable over the whole
working directory, decided by whether it runs commands, and not derived
from what the briefing selected
([delegate door](../coder/runtime/delegate-door.md)). NIP-CTX already
records which evidence was chosen for which recipient; the grant doesn't
yet follow it. Part III lists the maxim we intend: let CTX's "knows
about" shape CAP's "access to."

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

The gate is an engineering gate in Part I's sense, not the delta's
definition. Its [own file](../../crates/gym/gates/ext-eval-v2.json) says
so: the spread of three repeats is "itself a noisy estimate," the error
runs toward **Better**, and what would replace it, the spread of whole
evaluations repeated on the same extension, is recorded as a pending
measurement that nothing has taken yet. That is why an inconclusive result
is the default and adoption needs three confirming checks. The reports
carry per-case outcomes for both arms, so a reader can compute a paired
interval from them; we don't publish one yet.

#### Reach and restraint in OpenAgents

Every suite marks each test should-fire or should-not-fire. Each starter
suite has four of the first kind and two of the second. In the same
record, restraint held: every should-not-fire test passed with the tool as
often as without it, except one run of Code finder's `explain-idempotent`.
Reach did not always hold: `where-tests`, `known-bugs`, `workarounds`, and
`ci-failures` failed in both arms because Jev didn't choose the tool's
program for that wording, so for those tests the tool changed nothing.
Those tests are now the work list for each tool. This is the failure the
tool-selection literature (ToolLLM, AnyTool, Gorilla) is about: the tool
was available and the right answer depended on it, but it wasn't selected.
Scoring both arms per test is what made the failure visible, rather than
averaging it into a smaller delta.

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

#### Reproduced capability claim in OpenAgents

An [eval check](../extensions/evaluation.md#checks-adoption-and-credit)
reruns a published suite and publishes its own NIP-EVAL `3189` that cites
the original: a different trainer, the same test set release, the same
tool release. It confirms on a matching verdict and disputes otherwise;
both stay visible. In the hosted record, a second trainer checked each of
the three results and all three confirmed.

Read those checks with their reliance set. Both trainers' runs executed
on the same hosted runner on `coderos-4080`, through the same door and
model name, with the same Coder build, the same Jev, and the same
graders. The second trainer is independent as a signing principal and
not as a platform; what the checks rule out is the first trainer's
mistake or fraud, not a fault shared by the runner, the provider, or the
grader. A check from a local `openagents ext eval` run on another
machine would vary the host; nothing today varies the provider or the
grader, and no report records what its reruns shared.

#### Externally validated capability claim in OpenAgents

Not built. Nothing in the hosted record is externally validated: the
runner releases each catalog tool and is the author of each starter
suite, so the tool and its tests share a signer, and the checks reran
those same tests. NIP-EVAL accepts a report on any suite for the same
subject, and both the tool's release and the suite's release are signed
events with creation times, so a reader could already check both the
provenance (a different signer) and the chronology Part I asks for (the
tool's release locked before the suite's release appeared). But a
different signer is not independence, no field marks a report externally
validated, nothing computes the chronology, hidden suites don't exist,
and the candidate policy doesn't ask for any of it.

#### Capability adoption in OpenAgents

An operator issues an `openagents.eval-admission.v1` decision citing the
reports and publishes a release of the
[`openagents:coder-defaults`](../../packages/coder-defaults/) package that
depends on the tool. A tool becomes a candidate when its result is
**Better** and at least three distinct trainers' checks confirmed it.
Adoption is an operator decision, never automatic. The mechanism exists; no
adoption has been made yet.

The marginal question in Part I has a plain answer today and a harder one
soon. The baseline arm is Coder with nothing admitted. While
`coder-defaults` lists no extensions, that is also the current default
set, so the first adoption's marginal delta is its standalone delta. From
the second adoption on, a candidate has to be measured as current defaults
plus the candidate against current defaults alone, and the runner has no
such arm yet.

#### Capability credit in OpenAgents

Two [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check) rules.
`eval-check` credits the checker, the original evaluator, and the suite's
author when a check confirms a result. `eval-adopt` credits the tool's
author, the suite's author, and the evaluators of cited results when Coder
adopts the tool. A run, a publish, a view, or a download earns nothing.
Nor, today, does a dispute: a check that reran the suite to protocol and
got a different verdict earns nothing, which is the confirmation
incentive Part I's credit rule rejects. The first nine awards were all
for confirmations, so no dispute has yet gone unpaid; the rule should
change before one does, and Part III lists it.
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

Both echo AnyTool's finding that ToolBench's protocol inflated pass rates
([Du et al., 2024](https://arxiv.org/abs/2402.04253)): in each case the
metric, not the agent, produced the verdict. Our v1 gate credited speed as
if it were correctness; our grader missed a phrasing. Versioned gates and
test sets are how we keep such fixes from rewriting history.

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
   more tests. Under `ext-eval-v2` the primary outcome is tests passed, so
   time and cost are notes beside the verdict. A claim with cost as its
   primary outcome and correctness held non-inferior, which is what the
   router's T0 and T1 tiers are, has no gate yet; Part III lists it.

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
| [NIP-CAP](../../nips/openagents/NIP-CAP.md) | Partial | Describes an execution interface, its host binding, the grant to use it, and its observed presence; a `service` profile advertises decision services such as Jev's. Its "capability" is the [authority sense](#what-the-word-capability-means-here), the G in a claim's scope. | `30180`, `30181` |
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
| Check and reproduce | [EVAL](../../nips/openagents/NIP-EVAL.md#checks) | A `3189` with the `check` marker, the same suite and subject, the same subject-arm lock, and a different trainer; it confirms on an equal verdict and disputes otherwise. Hosted reruns set `check` in the `run` action. |
| Validate externally | None yet | A report on a second suite for the same subject is an ordinary `3189`; no marker says its suite was written independently of the subject or revealed after the subject was locked, and no reader requires one. |
| Credit | [XP](../../nips/openagents/NIP-XP.md#eval-check) | `eval-check` pays `checker`, `evaluator`, `suite-author`; [`eval-adopt`](../../nips/openagents/NIP-XP.md#eval-adopt) pays `extension-author`, `suite-author`, `evaluator`. Quests `30193`, awards `3193`, revocations `3194`; the CJ `card` of type `credit` shows the reader's ledger. |
| Adopt | [EVAL](../../nips/openagents/NIP-EVAL.md#adoption), [POL](../../nips/openagents/NIP-POL.md#optimization-authority-and-adoption), [EXT](../../nips/openagents/NIP-EXT.md#release-and-package-manifest) | An `openagents.eval-admission.v1` decision citing the reports and checks, then a `coder-defaults` `3184` release whose `dependencies` include the tool's release and whose `provenance.receipts` cite the admission. POL keeps activation an operator decision; active runs keep their lock. |
| Share (Verse) | [MV](../../nips/openagents/NIP-MV.md#gym-notes), [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs), [EVAL](../../nips/openagents/NIP-EVAL.md#gym-results-publication), [XP](../../nips/openagents/NIP-XP.md#trainer-cards-30194) | Gym notes: kind `9` world chat with `L`/`l` `openagents.gym` and an `e … source` citing the trainer's `3189`. CJ `open_screen` `verse.gym`. Gym leaderboards `3195`. Trainer cards `30194`. |

#### The terms and their carriers

| Term | Canonical carrier | What carries it, and what doesn't yet |
| --- | --- | --- |
| Test-time capability and capability claim | [EXT](../../nips/openagents/NIP-EXT.md) + [EVAL](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile) | A component release (`3184`) plus a report that measured it with and without. The report is the claim, and its `subject` lock, `baseline` arm, cited suite, run config, gate digest, and cost fields are the claim's scope. The five sources map to EXT components (`plugin`, `capability`, `skill`), KB entries (`3190`), delegation (PRG `delegate`), and typed judgment (CJ decision jobs). A component with no report is a candidate. |
| Capability admission | [EXT](../../nips/openagents/NIP-EXT.md#listings-updates-and-installation), [CAP](../../nips/openagents/NIP-CAP.md#description-binding-and-grant) | Lock, grant, and admission as separate decisions; a report's `subject.lock`. No public event records one Coder turn's admission; RUN's `created` record is the private place for it. NIP-EVAL's `openagents.eval-admission.v1` is adoption, not this. |
| Capability delta | [EVAL](../../nips/openagents/NIP-EVAL.md#reports) | `subject` and `baseline` arms, `measurements` with `change` on the `comparison` arm, `verdict`, and the gate digest in `acceptance`. The gate's rules (`ext-eval-v2`) are a Gym file the report pins, not wire text. |
| Reach and restraint | [EVAL](../../nips/openagents/NIP-EVAL.md#suites) | Each case's `kind`, `should-fire` or `should-not-fire`, repeated in `meta.ext_eval.cases`, with per-case outcomes in `runs`. No field aggregates a reach or restraint rate; a reader computes it. |
| Judgment budget | [CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs), [POL](../../nips/openagents/NIP-POL.md#routing-and-observed-cost) | CJ `judgment` feedback carries the decision (`tier`, `route_p`, `answer_p`, `needs_specifics`) and a result names a bank answer with `model: "bank:<bank id>"`. Its time and cost have no wire field today; POL's `openagents.route-usage.v1` (`latency_ms`, `cost_microunits`) is Designed. |
| Test-time delegation | [PRG](../../nips/openagents/NIP-PRG.md#step-kinds), [SESS](../../nips/openagents/NIP-SESS.md#steering-capability), [ATIF](../../nips/openagents/NIP-ATIF.md#delegated-sub-agents) | PRG `delegate`, SESS delegate-engine rows, ATIF `parent`/`children`. The delegate door's provider failover and Jev's briefing run locally and have no Nostr record yet. |
| Reproduced capability claim | [EVAL](../../nips/openagents/NIP-EVAL.md#checks) | A confirming `check`, decided by `eval_ext::confirms` from the two signed events. The candidate threshold (three distinct trainers) is [operator policy](../extensions/evaluation.md#checks-adoption-and-credit), not a NIP field. |
| Externally validated capability claim | None yet | A second suite by another author is an ordinary `3189` on the same subject. No field marks it independent; a reader could derive signer and chronology from the two releases, and nothing does. |
| Capability adoption | [EVAL](../../nips/openagents/NIP-EVAL.md#adoption) | `openagents.eval-admission.v1` plus the `coder-defaults` `3184` release. No adoption has been published, and no report format carries a marginal arm (current defaults with and without the candidate). |
| Capability credit | [XP](../../nips/openagents/NIP-XP.md#eval-check) | `eval-check` and `eval-adopt` awards (`3193`); XP is never money. `eval-check` pays confirming checks only; paying disputes needs a new quest rule, not a new kind. |
| Capability flywheel | Composition of the rows above | No single carrier. Its measure, incremental externally validated passes per adopted contribution, has no wire field; a reader would derive it from `3189` results, checks, and `coder-defaults` releases. |

The NIPs above each point back here in a "Test-time capabilities" line, and
the [NIP index](../../nips/openagents/README.md#test-time-capabilities)
lists the same mapping from the protocol side.

### The NIPs, one by one

A NIP (Nostr Implementation Possibility) is a written specification for how
signed events travel between clients over Nostr relays. The tables above say
which NIP carries which field; this section says, in plain words, what each
one is for and why test-time capabilities need it. They're in the order a
capability lives through them: found, admitted, run, delegated, recorded,
measured, checked, credited, adopted, and shared. Each status is the one in
[Which NIP is for what](#which-nip-is-for-what).

#### NIP-EXT

NIP-EXT (extension distribution) publishes tools, plugins, skills, and
packages as signed, immutable releases (kind `3184`), and keeps installing,
enabling, granting, and admitting a component as separate decisions. It is
where a test-time capability comes from, and a test set travels the same way,
as an `eval-suite` component. Without it, "the tool we measured" and "the tool
you installed" could be different bytes, and no result could name exactly
what it tested. Status: Partial. [NIP-EXT](../../nips/openagents/NIP-EXT.md)

#### NIP-CAP

NIP-CAP (execution capabilities) describes what an operation does, which host
implementation satisfies it, who is granted its use, and whether it's actually
present on this machine, as four separate things. Its `service` profile also
advertises decision services such as Jev's. It matters for admission: a
description alone never grants anything, so a capability an agent read about
can't quietly start acting. Its "capability" is the
[authority sense](#what-the-word-capability-means-here) of the word, a
grant over an operation, and never a measured ability. Its effects object
already keeps reads, writes, network, process, delegation, and spend as
separately declared effects, which is the first half of Miller's rule to
reify each distinct authority as a distinct object; the second half,
granting each of those separately rather than declaring them together,
is what a narrow authority graph needs. Status: Partial.
[NIP-CAP](../../nips/openagents/NIP-CAP.md#description-binding-and-grant)

#### NIP-KB

NIP-KB (shared knowledge entries) publishes knowledge-base entries as signed
versions (`3190`) with heads (`30190`) and withdrawals (`3191`), and records
evidence that an entry helps as the same with-and-without report NIP-EVAL
uses. That makes a knowledge entry a test-time capability measured like any
tool, and tasks an entry was written from never count as evidence for it.
Without it, retrieved knowledge would be text of unknown origin with no test
behind it. Status: Implemented. [NIP-KB](../../nips/openagents/NIP-KB.md#evidence-3189)

#### NIP-CJ

NIP-CJ (agent jobs) carries encrypted jobs: conversation turns, typed decision
jobs, and recoverable execution jobs. It carries the judgment budget, through
decision jobs and the router's `judgment` feedback (route, tier, and the
probabilities behind them), and the chat's eval path, through its offers,
cards, and test-set draft; the hosted eval runner is an execution-job worker.
Without it, the cheap judgment that decides how much thinking a turn gets
would be invisible, and a chat request to run tests would have no job to
travel in. Status: Partial. [NIP-CJ](../../nips/openagents/NIP-CJ.md#typed-decision-jobs)

#### NIP-PRG

NIP-PRG (programs) defines typed workflows made of pinned steps. Two step
kinds matter here: `decide` calls a pinned decision function, and `delegate`
hands a bounded task and its context to an admitted executor. Without it, a
workflow's judgments and hand-offs would be ad hoc calls that no one else could
read, repeat, or bound. Status: Partial.
[NIP-PRG](../../nips/openagents/NIP-PRG.md#step-kinds)

#### NIP-CTX

NIP-CTX (task state and context views) defines context requests and selection
receipts: which evidence was chosen for which recipient, within stated limits,
with mandatory material never silently dropped. A delegate's briefing is
exactly such a selection. Without it, nobody could later say what the
delegate was shown, so a delegation's result couldn't be traced to its
inputs. Status: Designed.
[NIP-CTX](../../nips/openagents/NIP-CTX.md#context-requests-and-selection-receipts)

#### NIP-SESS

NIP-SESS (engine sessions and turn control) gives clients one session contract
over different agent engines. For delegation it records each engine's steering
capability (whether a running turn can take a new message, and what proves it
did) and exports session history, whose portable form can be an ATIF
trajectory. Without it, a delegate engine's abilities would be assumed rather
than stated. Status: Designed; its read-only observer is implemented.
[NIP-SESS](../../nips/openagents/NIP-SESS.md#steering-capability)

#### NIP-WORK

NIP-WORK (tracked objectives and planning) defines work that outlives a single
conversation, and a signed delegation (`openagents.work-delegation.v1`) that
gives tracked work to another principal under a separate, bounded grant. It
covers delegation when the hand-off is a piece of tracked work rather than one
step of a turn. Without it, it would be unclear who is accountable for
delegated work and under what authority it runs. Status: Designed.
[NIP-WORK](../../nips/openagents/NIP-WORK.md#delegation-and-execution-links)

#### NIP-RUN

NIP-RUN (durable runs and evidence) is the authoritative journal of a run: its
lock, its parent run, the attempts it dispatched, and its outcome. Its
`created` record holds the lock a capability was admitted under, which makes
it the authoritative side of admission and delegation. Trajectories observe;
RUN decides. Without it, there would be no record of which exact components a
run was allowed to use. Status: Partial.
[NIP-RUN](../../nips/openagents/NIP-RUN.md#record-types)

#### NIP-ATIF

NIP-ATIF (agent trajectories) carries trajectories in the Agent Trajectory
Interchange Format (ATIF): the step-by-step record of what an agent did,
privately to its owner or publicly as `3198` with chunks `3199`. It carries
the trajectory of each with-and-without eval run, and links a delegating step
to the sub-agent's trajectory through `parent` and `children`. Without it, a
reported delta couldn't be inspected step by step, and a delegation couldn't
be followed into the delegate's work. Status: Designed; no component publishes
these events yet, and traces stay local files.
[NIP-ATIF](../../nips/openagents/NIP-ATIF.md)

#### NIP-EVAL

NIP-EVAL (workload evaluation evidence) is the unit of account. Its extension
evaluation profile carries the with-and-without report (`subject` and
`baseline` arms, `measurements`, `verdict`, and the gate that decided it), the
should-fire and should-not-fire cases behind reach and restraint, published
results (`3189`), checks by a different trainer, hosted runs, and adoption.
Without it, a capability delta is an unsigned claim nobody can check, and "is
a capability" would mean whatever its author says. Status: Partial, with the
extension evaluation wire formats implemented.
[NIP-EVAL](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile)

#### NIP-XP

NIP-XP (quests, acceptance, and experience points) publishes quests, a
referee's signed acceptance, and the experience points (XP) it carries. Its
`eval-check` rule credits the checker, the original evaluator, and the suite's
author when a check confirms a result; `eval-adopt` credits the tool's author,
the suite's author, and the evaluators of cited results when a tool is
adopted. Without it, contributors get no credit for reproduction work;
because awards are signed events, any reader can recompute the ledger. XP
is never money. The `eval-check` rule pays confirming checks only, which
Part III proposes to change so that a protocol-following dispute pays the
same. Status: Implemented as a whole; the NIP marks the two eval rules
Partial. [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check)

#### NIP-POL

NIP-POL (scoped instructions, admission, and routing records) makes host
decisions inspectable. It defines route receipts and observed usage, which are
the time-and-cost side of a judgment, and keeps adopting an evaluated
implementation an operator's decision, with active runs keeping their lock.
Without it, a judgment's cost has no record, and adoption could change a
running agent underneath it. Status: Designed.
[NIP-POL](../../nips/openagents/NIP-POL.md#optimization-authority-and-adoption)

#### NIP-OPT

NIP-OPT (AI contracts and optimization studies) records searches for a better
implementation of a fixed task, such as a tuned prompt or program. Its result
reaches an agent only through NIP-EVAL admission and a new NIP-EXT release.
Without that rule, an optimizer could swap in an unmeasured change; with it,
an optimized candidate faces the same test as any other capability. Status:
Designed. [NIP-OPT](../../nips/openagents/NIP-OPT.md)

#### NIP-MV

NIP-MV (shared 3D worlds) is how Verse shares presence and chat. Its Gym notes
are world-chat lines, sent on a trainer's behalf, that cite the trainer's
published eval result. That is the last stage of the flywheel: results are
seen where agents and people meet. Without it, results would sit on relays
where nobody meets them. Status: Partial.
[NIP-MV](../../nips/openagents/NIP-MV.md#gym-notes)

#### The shared contracts

The shared contracts define what every NIP above relies on: references and
digests that name exact bytes, locks that pin a component and all its
dependencies, and the private `3188` envelope. The lock is what makes
admission exact: its digest is recorded with each run and each eval report, and
a catalog update can't change an admitted run's lock. Without them, "the same
tool" and "the same test set" couldn't be stated precisely enough to compare
two runs. Status: Partial.
[Shared contracts](../../nips/openagents/contracts.md#locks-and-resolution)

## Part III: What we'll measure next

These are our open problems, for the OpenAgents implementation in Part II.
Part I's [open questions](#open-questions-for-the-field) are the general
versions.

- **External validation before the first adoption.** No tool has been
  adopted into `coder-defaults`, and none should be on its author's suite
  alone. Before the first, we need a second suite for the same tool
  written by someone who didn't write it and released after the tool's
  release was locked, a **Better** on that suite, and a change to the
  candidate policy that requires one. Today the policy asks for three
  confirming checks on the same suite, which is reproduction only. Both
  releases carry signers and creation times, so the chronology check is a
  reader-side rule away.
- **Pay disputes.** `eval-check` pays only a confirming check. It should
  pay any check that reran the published suite to protocol, whichever
  verdict it got, at the same rate, so the network rewards verification
  rather than agreement. That is a new quest rule under NIP-XP and a
  referee change, not a new event kind.
- **A primary outcome per claim.** `ext-eval-v2` fixes tests passed as
  the primary outcome. The router's cheap tiers are claims of a different
  shape, the same correctness at a fraction of the cost, and need a gate
  that names cost as primary with correctness held non-inferior.
- **Pin what can be pinned in B.** The report records the door and model
  name but not the weights behind them or Jev's version. We should record
  what the door reports about its model at run time, and treat a change
  there as voiding reproduction rather than hiding it.
- **Record and vary the reliance set.** Every hosted check so far shared
  the runner, host, door, Coder build, Jev, and graders with the result it
  checked. A report should record what a rerun shared with the original,
  and the candidate policy should eventually want at least one check that
  varied the host and one that varied the provider or the grader.
- **Let CTX's "knows about" shape CAP's "access to."** A delegate's grant
  should be derived from the evidence its briefing selected, the way a
  Wasm guest's handles are minted from what it listed, instead of a
  read-only or writable grant over the whole working directory. Jev
  would then be constructing the minimum authority context in the same
  decision that constructs the minimum epistemic context.
- **Marginal authority at adoption.** Beside the marginal delta, adoption
  needs a before-and-after bound on what the default set can cause, traced
  through the paths a newcomer opens between existing components, since
  its own grant won't show them.
- **Reach.** Four should-fire tests failed in both arms because Jev didn't
  choose the tool for that wording. Improving how capabilities are described
  to the router, so it reaches for them, is likely the cheapest gain
  available.
- **Deltas under a full grant.** The hosted starter results are measured
  without a shell. We need the same tools measured against a Coder that can
  already run commands, where the baseline is much stronger. That is a
  second claim with a different B and G, not a correction of the first.
- **Marginal deltas.** The runner's baseline arm is Coder with nothing
  admitted. Once `coder-defaults` holds anything, a candidate needs a
  baseline arm that holds the current defaults, and adoption needs a
  regression check across the whole default set, not only a check of the
  newcomer.
- **Cost.** Every eval record should carry a price for both arms. Until
  Coder prices gateway lanes, cost is a blank we don't fill with guesses.
- **Uncertainty and power.** Six tests and three runs per arm are enough to
  see a large change and too few to see a small one. The reports already
  hold per-case outcomes for both arms, so an interval can be computed and
  published beside the gate's verdict, with the task as the unit and
  repeats nested inside it; the gate's own pending measurement, the spread of whole
  evaluations repeated on one extension, should be taken. Our suites need
  to grow as the deltas we care about shrink.
- **Grader quality.** Graders are code and make mistakes, as the "not found"
  case showed. We want graders checked the way results are.
- **Process-level deltas.** Every eval run already saves an ATIF
  trajectory per test. We haven't yet reported deltas from them (steps,
  wasted tool calls, when the tool was first chosen) alongside pass counts.
- **Admission safety.** Our suites measure utility. We need cases that
  measure a candidate's exposure to injected instructions and the authority
  it exercises, kept as a separate constraint rather than folded into the
  verdict, so a helpful but unsafe tool can't reach adoption.
- **Reliability across repeats.** Three runs per arm show spread; they don't
  yet report whether a tool passes *every* repeat, the pass^k view.
- **Compute and capability together.** We haven't yet measured how
  test-time capabilities interact with more thinking: whether a tool lets a
  cheaper model with less reasoning match a stronger one, and when extra
  reasoning still pays on top of a tool.
- **Network evidence.** The flywheel's test is incremental externally
  validated passes per adopted contribution. We will report it, including
  when it's zero.

## References

- Akyürek, E. et al. (2024). *The Surprising Effectiveness of Test-Time
  Training for Abstract Reasoning.* [arXiv:2411.07279](https://arxiv.org/abs/2411.07279)
- Brown, B. et al. (2024). *Large Language Monkeys: Scaling Inference
  Compute with Repeated Sampling.* [arXiv:2407.21787](https://arxiv.org/abs/2407.21787)
- Cai, T., Wang, X., Ma, T., Chen, X., and Zhou, D. (2024). *Large
  Language Models as Tool Makers.* ICLR 2024. [arXiv:2305.17126](https://arxiv.org/abs/2305.17126)
- Chen, L., Zaharia, M., and Zou, J. (2024). *FrugalGPT: How to Use Large
  Language Models While Reducing Cost and Improving Performance.* TMLR.
  [arXiv:2305.05176](https://arxiv.org/abs/2305.05176)
- Cobbe, K. et al. (2021). *Training Verifiers to Solve Math Word Problems.*
  [arXiv:2110.14168](https://arxiv.org/abs/2110.14168)
- Debenedetti, E., Zhang, J., Balunović, M., Beurer-Kellner, L., Fischer,
  M., and Tramèr, F. (2024). *AgentDojo: A Dynamic Environment to Evaluate
  Prompt Injection Attacks and Defenses for LLM Agents.* NeurIPS 2024
  Datasets and Benchmarks. [arXiv:2406.13352](https://arxiv.org/abs/2406.13352)
- DeepSeek-AI (2025). *DeepSeek-R1: Incentivizing Reasoning Capability in
  LLMs via Reinforcement Learning.* [arXiv:2501.12948](https://arxiv.org/abs/2501.12948)
- Dennis, J. B. and Van Horn, E. C. (1966). *Programming Semantics for
  Multiprogrammed Computations.* Communications of the ACM 9(3), 143–155.
  [doi:10.1145/365230.365252](https://dl.acm.org/doi/10.1145/365230.365252)
- Du, Y., Wei, F., and Zhang, H. (2024). *AnyTool: Self-Reflective,
  Hierarchical Agents for Large-Scale API Calls.* ICML 2024, PMLR 235.
  [arXiv:2402.04253](https://arxiv.org/abs/2402.04253)
- Guo, W. et al. (2026). *MalSkillBench: A Runtime-Verified Benchmark of
  Malicious Agent Skills.* [arXiv:2606.07131](https://arxiv.org/abs/2606.07131)
- Hu, J., Qi, Y., Huang, X., Sun, Y., Dong, Y., and Huang, X. (2026).
  *Skill Use or Skill Theater? Evaluating the Reasoning Backroom in
  Skill-Augmented Language Agents.* [arXiv:2607.27484](https://arxiv.org/abs/2607.27484)
- Jiang, Y. et al. (2026). *SoK: Agentic Skills: Beyond Tool Use in LLM
  Agents.* [arXiv:2602.20867](https://arxiv.org/abs/2602.20867)
- Kahneman, D. (2011). *Thinking, Fast and Slow.* Farrar, Straus and Giroux.
- Kevin, C. et al. (2026). *Evaluating Skills, Not Just Agents: Agentic
  Continuous Evaluation of Skills.* [arXiv:2608.20614](https://arxiv.org/abs/2608.20614)
- Kim, S., Moon, S., Tabrizi, R., Lee, N., Mahoney, M. W., Keutzer, K.,
  and Gholami, A. (2024). *An LLM Compiler for Parallel Function Calling.*
  ICML 2024. [arXiv:2312.04511](https://arxiv.org/abs/2312.04511)
- Lamb, C. and Zacchiroli, S. (2022). *Reproducible Builds: Increasing the
  Integrity of Software Supply Chains.* IEEE Software.
  [arXiv:2104.06020](https://arxiv.org/abs/2104.06020)
- Lewis, P. et al. (2020). *Retrieval-Augmented Generation for
  Knowledge-Intensive NLP Tasks.* [arXiv:2005.11401](https://arxiv.org/abs/2005.11401)
- Li, X. et al. (2026). *SkillsBench: Benchmarking How Well Agent Skills
  Work Across Diverse Tasks.* [arXiv:2602.12670](https://arxiv.org/abs/2602.12670)
  (all figures, including the per-task negative deltas and the
  self-generated-skills result, from the v4 revision of June 2026)
- Li, Y. (2026). *Dynamic Agent Skills: A Lifecycle Survey and Taxonomy of
  Evolving Skill Libraries.* [arXiv:2607.10113](https://arxiv.org/abs/2607.10113)
- Ma, C. et al. (2024). *AgentBoard: An Analytical Evaluation Board of
  Multi-turn LLM Agents.* NeurIPS 2024. [arXiv:2401.13178](https://arxiv.org/abs/2401.13178)
- Miller, M. S. (2006). *Robust Composition: Towards a Unified Approach to
  Access Control and Concurrency Control.* PhD thesis, Johns Hopkins
  University. [papers.agoric.com](https://papers.agoric.com/papers/robust-composition/abstract/)
  (cited by section: §§3.1–3.2 least authority and designation, §§5.1–5.2
  reliance set and platform risk, §8.1 permission and authority, §11.3
  the arena, §15.2 reify distinctions in authority, §§20.1, 21, 22.2,
  22.4, 22.5 locality of knowledge, attack surface, subcontracting, and
  nested least authority)
- Model Context Protocol (2025). *Specification 2025-06-18: Tools.*
  [modelcontextprotocol.io](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
- Muennighoff, N. et al. (2025). *s1: Simple test-time scaling.*
  [arXiv:2501.19393](https://arxiv.org/abs/2501.19393)
- Newman, Z., Meyers, J. S., and Torres-Arias, S. (2022). *Sigstore:
  Software Signing for Everybody.* ACM CCS 2022.
  [doi:10.1145/3548606.3560596](https://dl.acm.org/doi/10.1145/3548606.3560596)
- Odersky, M., Zhao, Y., Xu, Y., Bračevac, O., and Pham, N. (2026).
  *Securing Agents With Tracked Capabilities.* CAIS '26: ACM Conference on
  AI and Agentic Systems.
  [doi:10.1145/3786335.3813127](https://dl.acm.org/doi/10.1145/3786335.3813127);
  preprint as *Tracking Capabilities for Safer Agents*,
  [arXiv:2603.00991](https://arxiv.org/abs/2603.00991)
- Ong, I. et al. (2025). *RouteLLM: Learning to Route LLMs with Preference
  Data.* ICLR 2025. [arXiv:2406.18665](https://arxiv.org/abs/2406.18665)
- OpenAI (2024). *Learning to reason with LLMs.*
  [openai.com](https://openai.com/index/learning-to-reason-with-llms/)
- Patil, S. G., Zhang, T., Wang, X., and Gonzalez, J. E. (2024). *Gorilla:
  Large Language Model Connected with Massive APIs.* NeurIPS 2024.
  [arXiv:2305.15334](https://arxiv.org/abs/2305.15334)
- Qin, Y. et al. (2024). *ToolLLM: Facilitating Large Language Models to
  Master 16000+ Real-world APIs.* ICLR 2024. [arXiv:2307.16789](https://arxiv.org/abs/2307.16789)
- Schick, T. et al. (2023). *Toolformer: Language Models Can Teach
  Themselves to Use Tools.* [arXiv:2302.04761](https://arxiv.org/abs/2302.04761)
- Shen, Y. et al. (2023). *HuggingGPT: Solving AI Tasks with ChatGPT and
  its Friends in Hugging Face.* NeurIPS 2023. [arXiv:2303.17580](https://arxiv.org/abs/2303.17580)
- Shinn, N., Cassano, F., Berman, E., Gopinath, A., Narasimhan, K., and
  Yao, S. (2023). *Reflexion: Language Agents with Verbal Reinforcement
  Learning.* NeurIPS 2023. [arXiv:2303.11366](https://arxiv.org/abs/2303.11366)
- SLSA (2023). *SLSA v1.0: Provenance.* [slsa.dev](https://slsa.dev/spec/v1.0/provenance)
- Snell, C., Lee, J., Xu, K., and Kumar, A. (2024). *Scaling LLM Test-Time
  Compute Optimally can be More Effective than Scaling Model Parameters.*
  [arXiv:2408.03314](https://arxiv.org/abs/2408.03314)
- Sun, Y. et al. (2020). *Test-Time Training with Self-Supervision for
  Generalization under Distribution Shifts.* [arXiv:1909.13231](https://arxiv.org/abs/1909.13231)
- Tan, B., Huang, X., and Sun, Y. (2026). *Skill Coverage: A Test Adequacy
  Metric for Agent Skills.* [arXiv:2606.20659](https://arxiv.org/abs/2606.20659)
- Torres-Arias, S., Afzali, H., Kuppusamy, T. K., Curtmola, R., and
  Cappos, J. (2019). *in-toto: Providing farm-to-table guarantees for bits
  and bytes.* USENIX Security 2019.
  [usenix.org](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias)
- Wang, G. et al. (2023). *Voyager: An Open-Ended Embodied Agent with Large
  Language Models.* [arXiv:2305.16291](https://arxiv.org/abs/2305.16291)
- Wang, X. et al. (2022). *Self-Consistency Improves Chain of Thought
  Reasoning in Language Models.* [arXiv:2203.11171](https://arxiv.org/abs/2203.11171)
- Wei, J. et al. (2022). *Chain-of-Thought Prompting Elicits Reasoning in
  Large Language Models.* [arXiv:2201.11903](https://arxiv.org/abs/2201.11903)
- Wu, Q. et al. (2023). *AutoGen: Enabling Next-Gen LLM Applications via
  Multi-Agent Conversation.* [arXiv:2308.08155](https://arxiv.org/abs/2308.08155)
- Yang, J. et al. (2024). *SWE-agent: Agent-Computer Interfaces Enable
  Automated Software Engineering.* NeurIPS 2024. [arXiv:2405.15793](https://arxiv.org/abs/2405.15793)
- Yao, S. et al. (2023). *ReAct: Synergizing Reasoning and Acting in
  Language Models.* ICLR 2023. [arXiv:2210.03629](https://arxiv.org/abs/2210.03629)
- Yao, S., Shinn, N., Razavi, P., and Narasimhan, K. (2025). *τ-bench: A
  Benchmark for Tool-Agent-User Interaction in Real-World Domains.* ICLR
  2025. [arXiv:2406.12045](https://arxiv.org/abs/2406.12045)
- Yuan, L., Chen, Y., Wang, X., Fung, Y. R., Peng, H., and Ji, H. (2024).
  *CRAFT: Customizing LLMs by Creating and Retrieving from Specialized
  Toolsets.* ICLR 2024. [arXiv:2309.17428](https://arxiv.org/abs/2309.17428)
- Zhao, A., Huang, D., Xu, Q., Lin, M., Liu, Y.-J., and Huang, G. (2024).
  *ExpeL: LLM Agents Are Experiential Learners.* AAAI 2024.
  [arXiv:2308.10144](https://arxiv.org/abs/2308.10144)
- Zhuge, M. et al. (2025). *Agent-as-a-Judge: Evaluate Agents with
  Agents.* ICML 2025. [arXiv:2410.10934](https://arxiv.org/abs/2410.10934)
