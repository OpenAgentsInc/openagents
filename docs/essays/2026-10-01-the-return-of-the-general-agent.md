# The Return of the General Agent

Essay, 2026-10-01, written for the launch of OpenAgents 1.0.0. It argues a
thesis and says what we have built toward it. [Part I](#part-i-why-general-agents-stalled)
reads the history of general agents and the turn to vertical ones.
[Part II](#part-ii-a-general-agent-is-a-composition) states the thesis
and tests it against the reasons general agents stalled.
[Part III](#part-iii-extensible-three-ways) is about extension: by people,
by their agents, and at machine speed. [Part IV](#part-iv-the-protocol-is-the-agent)
maps the argument onto the [OpenAgents NIPs](../../nips/openagents/README.md).
[Part V](#part-v-what-we-have-shown-and-what-we-havent) separates what we
have shown from what we haven't. Every claim about our own system links
the code, record, or commit behind it; where something is a design or a
hypothesis, the text says so.

It builds on two documents: the history in
[General agents: the long history, the vertical-agent turn, and what actually changed](../research/general-agents.md),
and the evaluation vocabulary in [Test-Time Capabilities](2026-09-29-test-time-capabilities.md).
The motivation is in [episode 288, *Three DevDays Later*](../transcripts/288.md).

## TL;DR

- **General agents did not fail; the monolithic general agent did.** The
  2023 boom put a broadly capable model inside a generic loop and expected
  a dependable worker. The 2024 turn to vertical agents answered the gaps
  that exposed: execution, local knowledge, evaluation, cost, a buyer, and
  authority. The lesson of that turn is that *generality does not remove
  the need for specialization somewhere in the system.* The open question
  was where.
- **Our answer: a general agent is a composition.** It is an agent of
  agents: a general front that decides cheaply and in typed form what each
  request needs, and a growing set of specialized members (answers,
  knowledge, programs, plugins, skills, other agents, other computers)
  that it admits per request. Specialization lives in the members.
  Generality lives in the composition.
- **A composition is only as good as its admission rule.** Members can make
  an agent worse as easily as better; skills an agent wrote for itself
  have measured *below* no skills at all. So every member is a candidate
  until a with-and-without measurement on a declared distribution says
  what admitting it changed, and adoption into the defaults is a separate,
  recorded decision. That rule is what lets the composition grow without
  decaying.
- **Because members are signed public records, the agent is extensible by
  people, by their agents, and at machine speed.** Anyone with a key can
  publish a member, a test set, a reproduction, or a validation. No review
  queue stands between a contribution and its evaluation. The cycle that
  extends the agent is a run, not a training run or an app-store review.
- **The protocol is the agent.** The NIPs carry every step: the request
  and its typed judgment, the member and its release, the grant, the run,
  the trajectory, the evaluation, the reproduction, the adoption, and the
  credit. A composition carried this way is not owned by one vendor's
  application.
- **What we have shown is narrower than the thesis.** The parts are built
  and running: a typed router in production, a coding agent that delegates
  to five engines, an evaluation loop that took one capability from claim
  to externally validated adoption in an afternoon, and a coding agent
  that has landed changes to its own repository from a chat message. What
  we have not shown is independent contributors at scale, compounding
  across a network, or the composition outside software work.
  [Part V](#part-v-what-we-have-shown-and-what-we-havent) lists it.

## Contents

- [Part I: Why general agents stalled](#part-i-why-general-agents-stalled)
- [Part II: A general agent is a composition](#part-ii-a-general-agent-is-a-composition)
  - [The thesis](#the-thesis)
  - [What the composition is made of](#what-the-composition-is-made-of)
  - [The six pressures, answered by composition](#the-six-pressures-answered-by-composition)
  - [Why the old composition idea didn't work before](#why-the-old-composition-idea-didnt-work-before)
  - [What is different from today's composition efforts](#what-is-different-from-todays-composition-efforts)
- [Part III: Extensible three ways](#part-iii-extensible-three-ways)
  - [By people](#by-people)
  - [By their agents](#by-their-agents)
  - [At machine speed](#at-machine-speed)
  - [Why speed needs a brake](#why-speed-needs-a-brake)
- [Part IV: The protocol is the agent](#part-iv-the-protocol-is-the-agent)
- [Part V: What we have shown, and what we haven't](#part-v-what-we-have-shown-and-what-we-havent)
- [What would show this is wrong](#what-would-show-this-is-wrong)
- [For the launch](#for-the-launch)

## Part I: Why general agents stalled

The [history](../research/general-agents.md) is longer than the current
wave. The General Problem Solver (1959) already separated a problem's
content from the method of solving it. Expert systems answered that useful
performance came mostly from knowledge of the domain. In 1994, Rodney, a
"general-purpose UNIX softbot", took high-level goals and ran commands on a
computer for its user. SRI's Open Agent Architecture put a facilitator in
front of specialist agents and routed each request to the ones that could
serve it. The ambition of a general agent, and the architecture of one made
of specialists, are each three decades old.

The modern wave began in spring 2023. GPT-4 shipped on March 14; within a
month AutoGPT had passed 100,000 GitHub stars. The implied product was a
loop: describe an objective, and the model plans, picks tools, runs them,
reads the results, and continues. By late 2024 the conversation had turned
to vertical agents, most visibly in Y Combinator's *Vertical AI Agents
Could Be 10X Bigger Than SaaS*. Casetext's CoCounsel (legal, March 2023),
Sierra (customer service, February 2024), and Devin (software, March 2024)
sold a recognizable job instead of open-ended intelligence.

The research note reads that turn as six pressures that reinforced one
another:

| | Pressure | What the generic loop lacked |
| --- | --- | --- |
| A | Planning is not doing | Reliable execution: GAIA's original agents passed 15% where people passed 92%; OSWorld's best agent 12% against 72%. A hundred steps at 99% each succeed about 37% of the time. |
| B | Real work needs local knowledge | This customer, this policy, this repository, this case. A model's training data does not hold them. |
| C | Narrow work can be evaluated | Representative cases, acceptance criteria, and a feedback signal. Coding has tests; most domains have less. |
| D | Open-ended execution has unpredictable cost | A price per successful outcome. "Keep working until done" can't be priced. |
| E | A defined job has a buyer | Someone whose problem it is, and a metric they want moved. |
| F | Authority must be bounded | What the agent may see, change, and commit to, and when it must stop. |

The note's conclusion, which this essay takes as its premise, is that
**the industry did not abandon generality. It learned what it takes to
turn general capability into dependable work.** Vertical products supplied
that by fixing the job. The question the note leaves open is the one this
essay answers: *where should the specialization live?* Inside separate
companies, inside one vendor's platform, in portable capabilities, or
across networks of cooperating agents.

Two observations from the note matter for what follows. First, coding was
not just one vertical among many. Code is a general way to act on digital
information, and the machinery behind coding agents became the basis for
general agent SDKs. A narrow start can be a road to generality. Second,
general agents never went away; they became more engineered. Magentic-One
(November 2024) was a generalist made of specialists inside one system.
Agent Skills (October 2025) packaged procedures a general agent loads when
needed. MCP (November 2024) standardized connections to tools; Agent2Agent
(April 2025) standardized agents talking to each other. The pieces of a
composition were arriving from several directions. What none of them
supplied was a way to know that adding a piece made the agent better.

## Part II: A general agent is a composition

### The thesis

**A general agent can be built as a composition: a general front that
decides, cheaply and in typed form, what each request needs, and a growing
set of specialized members it admits per request, each admitted on the
strength of a measured claim and carried as a signed public record.**

Three properties make this different from "a router in front of some
tools":

1. **The front decides with typed judgments, not generation.** Deciding
   where a request goes is a classification with probabilities, not an
   essay. A typed decision model answers in a fraction of a second, its
   readings can be calibrated against labeled examples, and deterministic
   code acts on them. That makes the front cheap enough to run on every
   turn, and measurable enough to trust.
2. **Members are admitted on evidence, not presence.** Having a component
   installed, described, or demonstrated decides nothing. A member's claim
   is a with-and-without measurement on a declared distribution; adoption
   into the defaults is a separate decision that cites independent
   reproductions and validations.
3. **Every member, measurement, and decision is a signed record on an open
   protocol.** That is what makes the composition extensible by anyone and
   not owned by the application that happens to run it.

The [Test-Time Capabilities](2026-09-29-test-time-capabilities.md) essay
names the mechanism behind the second property: an agent can gain or lose
abilities while it runs, without retraining, when something is admitted
into the run. This essay is about what that mechanism makes possible at
the scale of the whole agent.

### What the composition is made of

In OpenAgents 1.0.0 the composition looks like this. *OpenAgents* is the
general agent people talk to, on a phone, a desktop, or a terminal.
*Coder* is its first specialist member, a coding agent. The other members
are what people add.

| Layer | What it does | What it is in 1.0.0 |
| --- | --- | --- |
| Front | Reads each turn and decides, in one typed request, the route, whether a prepared answer fits, whether it needs specifics, the risk, whether work needs a computer, and which engine the person named | The [chat router](../coder/design/2026-09-28-chat-router.md): Jev decisions over [NIP-DEC](../../nips/openagents/NIP-DEC.md), carried by [NIP-CJ](../../nips/openagents/NIP-CJ.md) |
| Answers | Reviewed answers to product questions, served when the front is sure | The answer bank and [NIP-KB](../../nips/openagents/NIP-KB.md) knowledge entries |
| Conversation | A general model for everything the front doesn't hand off | A hosted chat model behind the chat worker |
| Specialist agent | Work that needs a computer | Coder, on the person's own computer ([NIP-HOST](../../nips/openagents/NIP-HOST.md), [NIP-REACH](../../nips/openagents/NIP-REACH.md)) |
| Engines | The executors Coder delegates to | Codex, Claude Code, Grok Build, OpenCode, Devin, chosen by policy, capacity, and the person's request |
| Capabilities | What people add: programs, plugins, skills, knowledge entries | [NIP-EXT](../../nips/openagents/NIP-EXT.md) releases, [NIP-PRG](../../nips/openagents/NIP-PRG.md) programs, Wasm plugins, NIP-KB entries |
| Evaluation | Whether a member helps, on what, at what cost | The Gym: [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) reports, checks, validations, adoption |
| Credit | Who did the work that made a member trustworthy | [NIP-XP](../../nips/openagents/NIP-XP.md) awards |

It is an agent of agents in a literal sense. The front decides; Coder is an
agent; Coder delegates to other vendors' agents; and those engines run
with capabilities that other people and other agents wrote. One request
can pass through four layers of agency, each chosen by a decision someone
can inspect.

### The six pressures, answered by composition

The thesis is only worth stating if a composition answers the pressures
that stalled the monolithic loop at least as well as a vertical product
does. Taking them in order:

**A. Execution.** The monolithic loop asked one model to plan and execute
every step of every kind of task. A composition hands execution to the
member built for it, and today the strongest executors are coding agents:
they act on a real environment and check their own work with tests. So the
front does not execute. It decides, and it delegates work that needs a
computer to Coder, which delegates again to whichever engine has capacity
and permission, and falls back when one is at its limit. Reliability is
then a property of how the composition checks results, not of one loop.
Our issue flow is the concrete case: Coder works a GitHub issue in its own
worktree, runs the repository's checks, gets bounded fix turns and
continuation turns, and pushes only when the checks pass; a red result
pushes nothing and says why ([docs](../cli/chat.md#working-a-github-issue)).
The 0.99¹⁰⁰ problem is not solved; it is moved to a place where each step
is checked.

**B. Local knowledge.** A vertical product packages one customer's context.
A composition supplies it as members: knowledge entries, the person's own
computer and repository, and the context each turn carries about where it
runs. Coder runs where the work is, on the person's machine, with their
logins and their files, under a grant they control. The front is told which
surface a turn came from and which project it is in, so it answers "what's
your working directory" from that context rather than pretending to know
nothing ([#10077](https://github.com/OpenAgentsInc/openagents/issues/10077)).
Local knowledge stops being the reason to build a separate product and
becomes something any member can contribute.

**C. Evaluation.** This is where composition either works or decays. A
vertical product evaluates one workflow. A composition must evaluate every
member it might admit, against the agent it would be admitted into. The
[Test-Time Capabilities](2026-09-29-test-time-capabilities.md) essay
defines the unit: a *capability claim*, a with-and-without measurement of
one identified member, admitted to one baseline agent, on one declared
distribution, under one grant, by one measurement, read by one written
policy. Our Gym runs those measurements, other trainers reproduce them,
a test set written by someone else validates them, and only then can an
operator adopt the member into every Coder's defaults. The first member
went the whole way on 2026-09-29: Project map passed 5 of 6 tests with the
member against 2 of 6 without, three distinct trainer keys reproduced it,
a second test set released later under another key validated it at 4 of 6
against 2 of 6, and it was adopted into `coder-defaults`
([record](../extensions/measurements/2026-09-29-first-adoption.md)). The
validated delta was smaller than the original, which is the finding the
method exists to surface.

**D. Cost.** The monolithic loop spent the most expensive model on every
decision, including the trivial ones. A composition spends judgment before
thinking. Our router's typed judgment takes about 170 ms at the median; a
prepared answer reaches the phone in 0.6 to 0.7 s; a full model answer takes
3 to 5 s
([measurements](2026-09-29-test-time-capabilities.md#our-numbers-judgments-before-thinking)).
Members can substitute for compute: in the hosted runs, a Project map run
took 10.7 s with the member against 24.9 s without, and passed more tests.
Cost becomes a property of which members a request needed, which a
composition can report per request.

**E. A buyer.** "What do I use a general agent for?" was the commercial
problem. A composition presents its members as jobs: talk to OpenAgents,
have Coder do this issue, test whether this capability helps. The general
front is the product people open; the members are the reasons they
return. The market drafts go further, letting agents negotiate bounded
work and be paid for accepted results ([NIP-MKT](../../nips/openagents/NIP-MKT.md),
[NIP-LAB](../../nips/openagents/NIP-LAB.md)); those are partial drafts, not a
deployed market.

**F. Authority.** The broader the agent, the harder it is to say what it
may do. A composition can make authority as specific as its members.
Describing an operation grants nothing; a grant is a separate record
([NIP-CAP](../../nips/openagents/NIP-CAP.md)). The computer's owner sets
which engines may run; a paired phone can ask for one but cannot widen the
owner's policy ([#10081](https://github.com/OpenAgentsInc/openagents/issues/10081)).
Keys stay on the host. A run's lock records exactly what was admitted, and
a run already going keeps its lock when the defaults change. Generality of
the front does not imply generality of permission.

None of these answers is free, and none is complete. The claim is narrower:
**each pressure that pushed the industry from general to vertical can be
met inside a general composition, by putting the specialization in members
and the discipline in the admission rule.**

### Why the old composition idea didn't work before

SRI's Open Agent Architecture had the shape in the 1990s: a facilitator
routing requests to specialist agents over a common language. Four things
were missing, and each has arrived only recently.

1. **A front that can read any request.** A facilitator could route only
   what its authors anticipated. A language model reads open-ended
   requests, and a typed decision model turns that reading into
   calibrated choices that code can act on, quickly enough to run on every
   turn.
2. **Executors worth delegating to.** Until coding agents, a specialist
   could do only what its author had programmed. Today's engines act on
   real environments, which makes "delegate the doing" a strategy rather
   than a hope.
3. **A way to know a member helps.** Neither the facilitator nor its
   successors measured whether adding an agent made the system better on
   the work people brought. Without that, more members means more ways to
   fail. The claim-and-adoption discipline is the missing piece.
4. **An open, signed substrate for members and evidence.** A member
   published into one vendor's directory is subject to that vendor's
   review, terms, and revenue share. [Episode 288](../transcripts/288.md)
   makes the point bluntly: three years after a developer revenue-sharing
   promise, the answer is still a human review queue and no stated share.
   Signed events on Nostr let anyone publish a member, a test, or a result,
   and let anyone verify who signed it.

### What is different from today's composition efforts

Each current effort supplies part of a composition. None supplies the
admission rule, and none is an open substrate for members *and* evidence
together.

| Effort | What it supplies | What it leaves out |
| --- | --- | --- |
| Magentic-One | A generalist made of specialists, with an orchestrator | Members are fixed by one vendor; no public measure of whether a member helps |
| Agent Skills | Portable procedures a general agent loads on demand | Presence is treated as capability; self-written skills can score below none |
| MCP | A standard connection between an agent and a tool | Connection is not competence; nothing records whether the tool helped |
| Agent2Agent | A standard for agents to talk to each other | Communication is not evidence or authority |
| App stores for agents | Distribution and discovery | A review queue, platform terms, and a revenue share set by the platform |

The proposal is not that these are wrong. It is that a general agent made
of members needs all of them *plus* a record of what each member does to
the agent, carried in the open.

## Part III: Extensible three ways

### By people

A person extends the agent by writing one of four kinds of member
([glossary](../glossary.md#one-vocabulary-what-you-can-add)): a **program**
(a typed workflow of named steps, the default), a **plugin** (a sandboxed
Wasm guest doing one bounded operation), a **skill** (guidance an agent
reads before a task), or a **knowledge entry** (a versioned, cited
reference). Each ships as a signed release. From the app, a person can
draft a test set in chat, start a with-and-without run, and publish the
result. They need no account with us beyond a key, and no one's permission
to publish.

### By their agents

The same records can be written by agents. A person's agent can author a
member, write the test set that measures it, run the evaluation, and
publish the result; another person's agent can reproduce it. The protocol
does not care whether a signer is a person or an agent; the admission rule
treats both alike, which is the point.

We have used our own agent this way on its own code. From a chat message,
Coder takes a GitHub issue: it posts a claim, works in its own worktree of
the main branch, runs the repository's checks, fixes what they find within
a bound, and lands the change on `main` only when they pass, then comments
the evidence and closes the issue. Its first runs landed
[`6d49408d6a`](https://github.com/OpenAgentsInc/openagents/commit/6d49408d6a),
[`57bf6e4778`](https://github.com/OpenAgentsInc/openagents/commit/57bf6e4778),
and [`619c7f203a`](https://github.com/OpenAgentsInc/openagents/commit/619c7f203a);
on 2026-09-30 it made the slide deck an embeddable viewer in one turn
([`70b78c260c`](https://github.com/OpenAgentsInc/openagents/commit/70b78c260c),
[#10056](https://github.com/OpenAgentsInc/openagents/issues/10056)). Two
larger issues ran out of steps with correct partial work, which people
finished; the flow now continues a turn that is still making progress
([#10063](https://github.com/OpenAgentsInc/openagents/issues/10063)). That
is an agent extending the agent, through the same gate any contributor
passes, at the scale of small changes.

### At machine speed

"Machine speed" is a claim about cycle time, so it should be stated in
terms of what has to happen before a contribution changes the agent.

| Step | In a weights-only agent | In an app-store agent | In this composition |
| --- | --- | --- | --- |
| Contribute | Collect data | Build to the platform's rules | Sign and publish a release |
| Get evaluated | Next training run | Human review queue | Anyone runs the with-and-without test; the result is a signed record |
| Get trusted | Lab's internal evaluation | Platform approval | Independent reproductions and a validation on someone else's test set |
| Reach every user | Next model release | Platform's distribution | An adoption decision and a defaults release that runtimes consume |

In the composition, no step waits on a training run or a reviewer's queue.
The one human gate we keep is adoption into the defaults, and it is a
decision about evidence already on the record, not a review of the
contribution itself. The Project map record shows the whole path, from
result through three reproductions, an external validation, adoption, and
the next run admitting it, completed in one afternoon
([record](../extensions/measurements/2026-09-29-first-adoption.md)).
Nothing in the path requires a person to act at any step except the
adoption decision, so the rate at which the agent can absorb improvements
is bounded by how fast members can be written and measured, and members
can be written and measured by agents.

That is what we mean by extensible at machine speed. It is a statement
about the shape of the loop, demonstrated once end to end. It is not yet a
measured rate of improvement across many contributors; Part V says so.

### Why speed needs a brake

Machine-speed extension without measurement is machine-speed regression.
On SkillsBench, 13 of 87 tasks got worse with a skill added, and skills
agents wrote for themselves landed below the no-skill baseline on every
configuration tested, while curated skills added 18 to 25 points
([cited in Test-Time Capabilities](2026-09-29-test-time-capabilities.md#what-the-word-capability-means-here)).
An agent that admits whatever it or anyone else writes will get worse
faster than it gets better.

Our own records show the same hazard in the instruments. Our first gate
rated a member **Better** for making Coder faster without making it more
correct; a grader missed a phrasing and flipped a verdict. We replaced the
gate the same day and versioned the test set, and the old results stay
readable under their old digests
([lessons](2026-09-29-test-time-capabilities.md#our-evals-in-practice)).
The same discipline applies to the front: every time the router learns a
new route, labeled examples and a held-out set measure it before release.
The latest revision, which learned to honor an engine the person names,
reads 0.907 route accuracy on held-out rows with prepared-answer precision
at 100%, and named the right engine 22 of 22 times when one was asked for
([measurement](../coder/measurements/2026-09-30-engine-request.md)).

The brake is what makes the speed safe: members are cheap to propose and
expensive to adopt, and the expense is evidence, not permission.

## Part IV: The protocol is the agent

If the composition lived inside one application, it would be a product
architecture. Because every step is a signed record, it is closer to a
public agent that applications run. The [OpenAgents NIPs](../../nips/openagents/README.md)
carry it. Status is each contract's, as the
[glossary](../glossary.md#nostr-and-shared-protocols) records it.

| Part of the composition | NIP | Status |
| --- | --- | --- |
| A turn, its typed judgment, and its offers (run Coder, open a test, publish a result) | [NIP-CJ](../../nips/openagents/NIP-CJ.md) | Partial |
| Typed decisions: one state, `noul`, `choice`, and `score` questions, probabilities | [NIP-DEC](../../nips/openagents/NIP-DEC.md) | Implemented |
| Knowledge as a member, with evidence that it helps | [NIP-KB](../../nips/openagents/NIP-KB.md) | Implemented |
| Members as signed releases: programs, plugins, skills, test sets | [NIP-EXT](../../nips/openagents/NIP-EXT.md) | Partial |
| Typed workflows, including `decide` and `delegate` steps | [NIP-PRG](../../nips/openagents/NIP-PRG.md) | Partial |
| Operation descriptions, separate from the grants that allow them | [NIP-CAP](../../nips/openagents/NIP-CAP.md) | Partial |
| The person's computers, enrolled devices, and scoped access | [NIP-HOST](../../nips/openagents/NIP-HOST.md), [NIP-REACH](../../nips/openagents/NIP-REACH.md) | Implemented |
| A run's lock, attempts, and outcome | [NIP-RUN](../../nips/openagents/NIP-RUN.md) | Partial |
| Delegate engine sessions and their steering | [NIP-SESS](../../nips/openagents/NIP-SESS.md) | Designed |
| What a run did, step by step, including sub-agents | [NIP-ATIF](../../nips/openagents/NIP-ATIF.md) | Designed |
| With-and-without reports, checks, validations, and adoption | [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) | Partial |
| Credit for the verification work | [NIP-XP](../../nips/openagents/NIP-XP.md) | Implemented |
| Searching for better implementations, promoted only through evaluation | [NIP-OPT](../../nips/openagents/NIP-OPT.md) | Designed |
| Delegated tracked work, paid labor, and paid operations | [NIP-WORK](../../nips/openagents/NIP-WORK.md), [NIP-LAB](../../nips/openagents/NIP-LAB.md), [NIP-MKT](../../nips/openagents/NIP-MKT.md), [NIP-X402](../../nips/openagents/NIP-X402.md) | Designed to Partial |

Three consequences follow from carrying the composition this way.

- **Members are portable.** A member is a release anyone can fetch and a
  claim anyone can check. It does not belong to the app that first ran it.
- **Evidence outlives the evaluator.** A result names its subject, its
  baseline, its test set, its gate, and its signer. Anyone can rerun it,
  and a dispute is itself a signed record.
- **The agent is not owned by one vendor.** The front, the members, and the
  evidence are records on relays. Our apps are one way to run the
  composition; the protocol is meant to admit others.

## Part V: What we have shown, and what we haven't

**Shown, with records:**

- A typed front in production that routes every chat turn, serves prepared
  answers only when its readings clear precision thresholds, hands work to
  Coder, and honors an engine the person names
  ([router design](../coder/design/2026-09-28-chat-router.md),
  [latest measurement](../coder/measurements/2026-09-30-engine-request.md)).
- A specialist agent on the person's own computer that delegates to five
  engines under the owner's policy, from a phone, a desktop, or a terminal.
- An evaluation loop that took one member from claim to reproduced to
  externally validated to adopted, and admitted it in the next runs
  ([record](../extensions/measurements/2026-09-29-first-adoption.md)).
- A coding agent that has landed changes to its own repository from a chat
  message, through the same checks as any contributor.
- A protocol that carries each of those steps as signed records, with the
  status of each contract stated.

**Not shown:**

- **Independent contributors.** The first adoption's result, three checks,
  and validation came from one machine and one person's agent through one
  runner. The policy counts keys; the record counts one operator.
- **Compounding.** That adding participants makes the agent measurably
  better is the claim we still have to earn. One adopted member is a
  proof of the path, not of a flywheel.
- **Generality beyond software.** Every specialist member today does
  software work. The thesis predicts the same discipline works for other
  domains; we have not tested it there, and domains without cheap checks
  (the legal example in the history) will be harder.
- **Cost as a primary outcome.** No gate yet rates a member on cost with
  correctness held equal, and not every lane is priced.
- **Reliability of the whole.** The parts are each tested; the composition
  still fails in ways no part's tests catch. On the night before this
  launch, the owner found a series of integration bugs that unit tests had
  missed, from a router that offered a test instead of delegating to a
  safety check that ran out of file handles. We now gate releases on an
  [end-to-end run of real flows](../release/acceptance.md) on the exact
  build. A composition's reliability is an engineering property that has
  to be earned in the joints, not assumed from the members.
- **Many NIPs are drafts.** Several contracts in Part IV are Designed, not
  implemented. The parts of the argument that rest on them are designs.

## What would show this is wrong

A thesis that can't fail isn't one. These results would count against it:

1. **Adopted members don't transfer.** If members that pass validation stop
   helping on the work people actually bring, the claim discipline
   measures the wrong thing.
2. **The front becomes the bottleneck.** If typed routing can't keep its
   precision as the number of members grows, the composition degrades into
   the monolithic loop with extra steps.
3. **Independent contributors don't arrive, or don't verify.** If the
   network's records keep coming from a few operators, the "open" in the
   argument is decorative.
4. **Vertical products keep winning on the same jobs.** If a dedicated
   product beats the composition on its own job by a margin that members
   can't close, specialization belongs in companies after all.
5. **The brake is too strong.** If evidence costs so much that good members
   never reach adoption, machine-speed extension is a slogan.

We intend to publish the measurements that would show each of these,
whichever way they come out.

## For the launch

This is the claim we think the argument supports, stated at the strength
the records support:

> In 2023, everyone tried to build a general agent by putting a capable
> model in a loop. In 2024, the industry gave up on that and built vertical
> agents, one job at a time. We think both missed the same thing:
> generality and specialization aren't alternatives. OpenAgents 1.0.0 is a
> general agent built as a composition, an agent of agents. A general front
> decides what each request needs and hands it to specialists: answers,
> knowledge, Coder on your own computer, and the coding agents Coder
> delegates to. Anyone can add a specialist, and so can their agents, by
> publishing it on open protocols: no review queue, no platform's
> permission. What keeps that from making the agent worse is the rule for
> letting a specialist in: it is measured with and without, reproduced by
> others, and validated on someone else's tests before it becomes a
> default. Because every step is a signed record, the agent can grow at
> the speed people and agents can build and measure, not the speed of a
> training run or a review queue. We have run that path end to end once.
> Now we're opening it to everyone.

Each sentence of that paragraph is backed by a section above; the last two
are the honest boundary between what we have shown and what the launch is
for.
