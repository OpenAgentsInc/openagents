# Episode 288 - Test-Time Capabilities

Status: draft script, 2026-09-29. Not recorded.
Speaker: Christopher.
Delivery: talking through the deck, conversational, one idea per slide. Say the zeros out loud.
Audience: people who use coding agents, people building them, and the OpenAgents followers who have watched the last four episodes.
Follows: [285 Bendcoder](285.md), [286 System One in Coding Agents](286.md), [287 Building a System One Coding Agent](287.md), and the unreleased [288 preparation session](288-prep.md) on the Coder Gym.
Deck: `crates/openagents-deck/decks/test-time-capabilities.md` (20 slides; the essay it follows is [Test-Time Capabilities](../essays/2026-09-29-test-time-capabilities.md)).
Sources for every number: named at each slide. Where the deck is behind the essay, the script says so, so the slide can be fixed before recording.

What this episode is for: we are about to start rolling out the next version of Coder and the Gym inside the OpenAgents app, and this episode explains what we're releasing and why. It introduces a term, test-time capabilities, walks the deck, explains the protocol underneath it, and ends on the rollout.

---

## Cold open (no slide yet)

Here's what the last few days looked like. We spent them on one question: what is possible with Jev and coding agents. And we had some success. We put Jev in front of harnesses like Claude Code, same models, and got the same results on the same Terminal-Bench tasks faster and cheaper. The best Opus configuration we found, Jev probes in front of a lean Opus 5.5, passed 24 of 24 trials on the four-task development panel, 63 percent cheaper and 32 percent faster than Claude Code on Opus alone, and 57 percent cheaper and 48 percent faster on four newer tasks. Same model, same answers, less money and less waiting.

[Show the results page for a second. Source: `docs/terminal-bench/development-results.md`, the "Repeated runs" section. Say the caveat in the same document: a development panel we tuned on, three trials per task, not a held-out or leaderboard result. The Terminal-Bench 4 numbers are separate and smaller; don't mix them in here.]

That's cool. I'm proud of it. And it's still an incremental improvement. It's a better harness around the same brain. My sense, and I've had it the whole time we were doing this, is that there's a ten-x improvement in here somewhere, and it isn't in the harness.

Here's the chain of thought. Why do we want System One at all? Because the whole point of System One is that you make decisions programmatically, at machine speed, on typed evidence, instead of waiting for model weights to guess. Jev answers a typed question in about two hundred milliseconds for a fraction of a cent. Fine. So if you're making decisions at machine speed, wouldn't you want those decisions to have access to all of the relevant knowledge? Not just what happens to be in the prompt. Everything anyone has already figured out about this kind of task.

That's test-time knowledge. But it's more than that. You don't only want the agent to know things at the moment it runs. You want it to be able to *do* things at the moment it runs. You want it to draw on all of the great stuff that's already happened: the tools people have written, the guides, the fixes for the mistake it's about to make, the other agents that are better at this than it is. Test-time capabilities.

We have a kind of amnesia right now. We expect an agent to be great at using the seven tools that were baked into it yesterday. What were the first tools that got trained into model weights? Web search, read a file, run a command. A handful. Then it moved down into the harness, and now every harness ships its own twenty tools and its own markdown skills folder and everyone glues the same things together over and over. That misses the much larger opportunity, which is the oldest idea in software: composability. Nobody writes their own web server anymore. You pull in a package. The agent world doesn't have that yet, and it's not because the idea is hard. It's because nobody has agreed on how to measure whether a component helps, how to say exactly which version was measured, and how to share the result so someone else's agent can inherit it.

We don't want to figure all of this out ourselves. We want to open it up. We want agent software to be an open, composable effort, the way open source has always been. That's what this episode is about. First the concept, then how we built it, then the protocol, then what ships.

---

## Slide 1: Test-Time Capabilities (title)

This is a term we're proposing, and there's an essay behind it in the repository. Three parts. Part one is the idea, and it names no product of ours. Part two is how OpenAgents implements it and what we measured. Part three is what we haven't shown yet. Every number in this deck comes from a dated record in the repo, and every slide names it.

---

## Slide 2: Test-time compute

Everyone in AI knows this one by now. Test-time compute means spending more computation when the model answers, not when it's trained. Think longer, that's chain of thought and o1 and R1. Control the budget, that's the s1 paper forcing the model to keep checking. Sample many times and pick, that's Large Language Monkeys: on SWE-bench Lite one model went from about 16 percent with one sample to 56 percent with 250. Spend it where it helps, that's the compute-optimal work. Adapt the weights briefly on the test input.

The point isn't any one paper. The point is that a fixed set of weights answers better when the system around it spends more, and more wisely, per question. Hold onto "the system around it."

---

## Slide 3: Two lessons

Two things from that literature carry past tokens. One: a verifier is what makes extra compute pay. Sampling only helps as far as something can tell right from wrong. Where the checker is weak, more samples stop helping. Two: compute should be allocated per question. Easy prompts don't need a long chain of thought. And the allocation decision is itself a judgment, and it should cost far less than the work it allocates.

Both of those generalize to the whole system around a model. What is it allowed to use, who decides, and how does anyone know it helped. Evals are the verifier. A cheap typed judgment is the allocator.

---

## Slide 4: The definition

[The deck's wording on this slide is behind the essay. It still says a capability exists "only if the same tests, run with it and without it, show the agent does measurably better." The essay has moved past that to the capability claim. Update the slide to the wording below before recording.]

Here's the term. A test-time capability is an ability an agent gains, or loses, at inference time, without a weight update, because something was admitted into the run. A repository map. A written guide. A knowledge entry about a recurring mistake. Another agent.

And here's the part that took the most work to get right. Nothing is a capability in general. A component is a *candidate*. Evidence makes a *capability claim*, and the claim says exactly how much admitting that exact component changed outcomes, against a stated baseline, on a stated set of tasks, under a stated grant and a stated rule. The same tool can add twenty points to one agent, nothing to another, and three points on a different kind of task, and none of those contradicts the others. Each belongs to its baseline and its tasks.

Having a tool installed makes no claim. Having it described makes no claim. A demo makes no claim. And it cuts both ways: admitting something can destroy capability as easily as create it. SkillsBench found that on 13 of its 87 tasks, giving the agent a skill made it worse.

---

## Slide 5: Five sources

Where can a capability come from? Five places. Tools and plugins: code with typed operations and bounded access to the host. Skills: a written guide the agent reads before a task. Knowledge: cited entries retrieved for the task. Delegation: another agent, briefed with selected evidence. And typed judgment.

The last one is different in kind. Tools act. Knowledge informs. Delegates work. A typed judgment turns fuzzy evidence into a typed decision that ordinary software can build around: ambiguous intent goes in, a probability comes out, and a state machine takes it from there. It's closer to a probabilistic branch instruction than to a chatbot. The code keeps the state machine, the effects, and the invariants; the judgment supplies the one thing deterministic code couldn't express economically, the decision at the branch. Choosing which of the other four to use is one application of it. An agent with fifty tools and no good way to decide which to use is worse than an agent with none.

---

## Slide 6: The lexicon

[The deck row "Verify | Verified" is retired wording; the essay now has "reproduced capability claim" and "externally validated capability claim" as two separate terms, and eleven terms total. Fix the row and the "ten terms" note.]

Words for the parts of a capability's life. Admission: a locked version of a component allowed into one run, and that's a security boundary, not just a switch. Delta: the with arm minus the without arm, on the same tests, repeated enough to see the spread. Reach and restraint: does the host actually reach for the tool where it helps, and leave it alone where it doesn't. Judgment budget: deciding how to answer has to cost far less than answering. Reproduced: someone else reran it and got a compatible result. Externally validated: it still helps on tests its author didn't write. Adoption: it joins everyone's defaults, measured against the current defaults, not against nothing. Credit: the people behind it get recognized, for verification work and adoption, not for activity.

Each of these has a way to be wrong. That's the test for whether a word earns a place.

---

## Slide 7: Evals are the unit of account

Why evals and not a leaderboard. A leaderboard rewards one system on one fixed task set, and it rewards fitting that set. A per-component eval asks a narrower question with a clearer answer: does this thing help, where, and at what cost. Two arms, not one score. A written rule gives the verdict, and the rule is a versioned file whose digest travels with every result, so when the rule has a bug, and ours did, the old results keep their old digest. And others can rerun it.

Benchmarks ask how capable an agent is. A claim says what caused it to become more capable. Benchmarks still check the agent as a whole. Evals decide what goes into it.

And one more thing the essay had to say plainly: once adoption and credit depend on evals, the evals stop being a measurement and become the objective function of the whole network. People will build what gets adopted. So independent test sets, held-out tasks, and paying for disputes aren't hygiene. They're what keeps the flywheel pointed at capability instead of at the tests.

---

## Slide 8: Cheap judgment before expensive thinking

The literature allocates thinking per question. We allocate one level up, before any thinking. When a message comes in, decide first whether it needs a large model at all. Offer a ladder at rising cost: a prepared answer, a prepared answer a small model finishes, a grounded answer from a knowledge base, the full model, and an agent with a computer.

Never wrong fast. A prepared answer is served only when its readings clear thresholds tuned for precision. A fast answer to the wrong question is worse than a slow right one. And capability can substitute for compute: a tool that answers directly can beat a model that has to search for the answer, on time and on correctness. That third one's a hypothesis. We'll show the number in a minute.

---

## Slide 9: Capabilities compound across a network

This is the hypothesis the whole thing rests on, and I want to say it as a hypothesis. Weights improve when a lab trains them, on the lab's schedule. A test-time capability can come from anyone, be tested by anyone, and, once adopted, reach every agent that shares the defaults, with no training run.

What a network adds: more sources, because people bring the task families and libraries they actually know. More verification, because reruns by other people are the verifier that extra effort depends on, supplied by people instead of a reward model. Inheritance, because one validated result becomes a default for everyone. And credit that tracks use.

The unit that compounds is not a longer prompt and not a count of packages. It's a capability claim with independent evidence: an exact version, a with-and-without result, reruns by people who didn't write it, and a delta that survives tests they wrote. Whether adding people makes an agent measurably better has to be shown. We haven't shown it. That's the honest state and I'll say the zero out loud when we get there.

---

## Slide 10: The chat router (Part II begins)

Now our implementation. Everything from here is OpenAgents. When you send a message in the app, before any model runs, Jev answers a batch of typed questions in one request: what kind of message is this, is there a prepared answer, does the reply need specifics, what's the risk, which lane. A policy table in code, not a model, picks a tier: T0 a whole prepared answer, T1 a prepared stem a cheap model finishes, T2 grounded in a knowledge base, T3 the full model, T4 an offer to run Coder on a computer. If Jev is slow or fails, you get the model's reply. The judgment can only save time.

Jev writes no text and grants no authority. Code decides what its probabilities cause. That sentence is the whole safety story of putting a learned primitive inside software.

---

## Slide 11: A judgment costs a fraction of the answer it can skip

Measured from Send on the phone. The Jev judgment: 170 milliseconds median, 235 at the 95th percentile. A prepared answer on screen: at most 700 milliseconds. A full model answer: up to 5.2 seconds. Prepared answers: 36 of 36 correct on 138 held-out messages, 100 percent precision.

[Sources on the slide: `docs/coder/measurements/2026-09-28-chat-router-eval.md`, `2026-09-28-first-reply.md`. Calibration for the route question alone on the development partition: ECE 0.046, Brier 0.083. That's one question, one partition; say so if asked.]

A turn answered at T0 costs a Jev call and no generation at all. That's the judgment budget in practice.

---

## Slide 12: Five ways Coder acquires a capability

The five sources, as built. Tools and plugins are Wasm guests with typed operations and bounded host access. A guest never sees a path; the host mints an opaque handle per invocation for exactly what it listed. Skills are a SKILL.md the agent reads. Knowledge is cited entries retrieved and filtered by Jev. Delegation goes through the delegate door: Microcoder on the first provider with capacity, failing over, or Claude Code or Codex briefed with evidence Jev chose. And Jev is the judgment.

The note on this slide matters: one declared Terminal-Bench 4 attempt passed fin-saccr-rwa for 94 cents in about 150 seconds, under Fable 5.1's cheapest and fastest. It was in-sample and tuned, and across seven series only 2 of 13 attempts beat the bar. Delegation is a capability to measure, not a guaranteed win.

---

## Slide 13: Gym evals, with the tool and without it

Here's the first real with-and-without record. Three tools, six tests each, three runs per arm, on our hosted runner. Project map: 5 of 6 with the tool, 2 of 6 without. Code finder: 4 of 6 against 2 of 6. Test reader: 5 of 6 against 2 of 6. All three read Better under the v2 gate.

Read it at its scope, because that's the whole point of the claim idea. In these runs the grant has no shell. Without the tool Coder can't read the files at all. So the delta is what each tool adds under that grant, not what it adds on top of an agent that already has a shell. That's a real, honest, reproducible claim, and it's a narrow one. Project map also ran in 10.7 seconds with the tool against 24.9 without, and passed more tests. Time is a note, not the verdict.

[Source: `docs/extensions/measurements/2026-09-29-hosted-runner-live.md`.]

---

## Slide 14: What the first runs taught us

Restraint held: tests that shouldn't use the tool passed as often with it as without, with one exception. Reach did not: four tests failed in both arms because Jev didn't pick the tool for that wording. The tool was there, the right answer depended on it, and the router didn't reach for it. Scoring both arms per test is what made that visible instead of averaging it into a smaller delta. Those four tests are now the work list.

The first gate was wrong. Under v1, a tool that made Coder faster but no more correct read Better. We found it in a live run and replaced the rule the same day; the old result keeps its old digest. And graders are software: one looked for "not found" and missed "the server cannot find the requested resource," flipping one run to Worse by chance. We fixed the pattern and released a new test set version.

Two of those are the same lesson the ToolBench people learned the hard way: the metric, not the agent, produced the verdict. Versioning is how you keep the fix from rewriting history.

---

## Slide 15: From a chat to every Coder

[Deck note on this slide says a candidate needs Better plus three confirming checks. As of today the policy also requires one externally validating result, a Better on a second test set that someone other than the tool's author released after the tool. Add it.]

The loop. Make a tool, in chat. Run both arms, on our computers. Publish it. Others check it. Adopt. The hosted runner lives on one of our machines with a per-trainer daily quota. Suites are NIP-EXT releases. Results and checks are NIP-EVAL events. Credit is NIP-XP awards. Adoption is a coder-defaults release, and it's an operator decision, never automatic.

A tool becomes a candidate when its result is Better, three distinct trainers' checks confirmed it, and at least one result on an independent second test set validates it. That last requirement is new and it's the one I care about most. Three people rerunning the author's own six tests proves the result reproduces. It doesn't prove the tool wasn't built to pass those six. Someone else's tests do.

Credit is XP and your name. It is never money. And as of today a check gets paid whether it confirms or disputes, because a good dispute is worth more than a fourth confirmation, and a network that only pays agreement learns to agree.

---

## Slide 16: The protocol, from finding a tool to delegating

Now the part I've wanted to talk about since the first episode of this series. Why is this on Nostr? Because we want multiple clients and multiple projects to speak the same signed JSON over WebSockets, and to be able to check each other's work without asking anyone's permission. Every record here is a signed event. Every reference is a digest. "The tool we measured" and "the tool you installed" are provably the same bytes or provably not.

Which NIP carries what. EXT: releases. A result names the exact tool version it tested, and installing, enabling, granting, and admitting stay separate decisions. CAP: grants. Describing a tool never grants its use, and a grant is now an object with a purpose, evaluation or operational, so the sandbox a claim was measured in is on the record. KB: knowledge entries tested with and without, like tools. CJ: the jobs, including the router's judgment and the hosted eval runs. PRG: decide and delegate as pinned, bounded steps. CTX: what evidence a delegate was actually shown. SESS: how each delegate engine can be steered. WORK: who answers for delegated work.

The statuses on the slide are honest. Several are Designed, not Implemented. That column is there so nobody has to take my word for it.

---

## Slide 17: The protocol, from the run to sharing the result

RUN is the run's journal: the lock a capability was admitted under, the grant, the baseline agent. Trajectories observe; RUN decides. ATIF carries the step-by-step trace of each arm and each delegate. EVAL is the unit of account: the with-and-without report with the claim's whole scope in it, what the run relied on, the tool's identity, the task distribution, and the verdict from a pinned gate; then the published result, the checks by another trainer, and the validations on a second suite. XP: credit anyone can recompute from public events. POL: adoption stays an operator's call and running agents keep their lock. OPT: an optimized candidate faces the same test as anything else. MV: the Gym in the Verse, where results get seen.

The shared contracts sit under all of it: exact references, locks, and the private envelope.

Here's the reason this is a shared language and not just our stack. If you write a different client, a different agent, a different runner, and you speak these events, your results are checkable by our readers and ours by yours. Nobody owns the registry. That's the difference between an agentic package registry that's a company and one that's a protocol.

---

## Slide 18: Where it stands today

[Update the third metric's note: the policy now requires a validation before adoption, and none exists.]

Three of three hosted results confirmed by a second trainer. Nine XP awards signed from those checks, recomputable by anyone with the ledger crate. Zero tools adopted into the defaults.

Say the zero. Every part of the loop is built. The first runs, checks, and awards are live. The flywheel has not been shown turning. And it can't turn yet, because no tool has a test set that someone other than us wrote. That's not a bug in the software. It's the thing only other people can supply, which is the point.

---

## Slide 19: What we haven't shown yet

The first validation, and then the first adoption, and whether the tool keeps its delta once it's in everyone's defaults. Reach: describing tools so the router picks them, probably the cheapest gain we have. Deltas under a full grant: the same tools against a Coder that can already run commands, where the baseline is much stronger; that's a new claim, not a correction of this one. Cost for both arms, which stays blank until our gateway lanes are priced. Uncertainty: six tests and three runs show large effects only. Marginal adoption: once the defaults hold anything, a candidate has to be measured against them, not against nothing. And network evidence: marginal, externally validated utility per adopted contribution, reported even when it's zero. Which it is.

---

## Slide 20: The claim

Test-time compute asks how long a model should think. Test-time capabilities ask what it should be able to use, and prove the answer with a test.

---

## The rollout (no slide; close on camera)

So here's what we're releasing, starting tomorrow.

The OpenAgents app, on a public TestFlight link and an Android APK. You open it and the first tab is Chat with OpenAgents. It needs no computer. Common questions get an instant prepared answer, product questions get answered from sourced notes, and the moment you ask for something that needs a machine, you get an offer to run Coder on your own computer over your tailnet.

And inside that chat is the Gym. Ask what's new. Pick a tool we recommend, Project map, Code finder, Test reader, or describe a tool you want and we'll draft it and a test set with you. Tap Start the test. Our hosted runner runs the tests with the tool and without it, three times each, and shows you the change: tests passed without, tests passed with, and a verdict. Add it to the Gym. Someone else runs the check. You earn XP when their check comes in, and when Coder adopts your tool for everyone. A new install reaches Start the test in three taps. We ran the whole loop live from a phone this morning: one tap, 2 of 6 to 5 of 6, Better, added to the Gym, a second trainer's check, plus 25 XP on the menu.

[Honest limits, say them: Coder on your computers needs your own Mac or Linux box on the same tailnet; chat needs nothing. No push notifications, no attachments, no model picker yet. XP is never money. Nothing has been adopted yet. Source: `docs/roadmap/2026-09-29-launch-roadmap.md`, `docs/extensions/measurements/2026-09-29-build-23-launch-audit.md`.]

This is the plugin marketplace we've been talking about since January 2024, episode 48, the Extism and Wasm brainstorm. It's the composability we've been chasing since episode one of this series. The difference now is that we finally have the two things it needed: a primitive that can decide at machine speed which of a million components to reach for, and a way to prove, in public, in signed events anyone can check, that a component actually helps. Without the first, a big registry is noise. Without the second, it's a popularity contest.

The big labs built incredible engines. We're building the thing that knows what to hand them, and the record that says whether it helped. Come put your tool in the Gym. Better yet, write a test set for someone else's. That's the one thing we can't do for ourselves.

See you tomorrow.
