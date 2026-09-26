# OpenAgents video archive: index, history, and themes

This guide covers the committed transcript archive as of 2026-09-26:
**288 numbered episode files covering every number from 001 through 288**.
The archive follows OpenAgents from document chat and its first coding agent
to agent markets, operator workrooms, the Coder product suite, and measured
System One coding-agent experiments.

The recurring question is how people can own, inspect, improve, and earn from
agent software. The implementations and product names change repeatedly.
The history below follows those changes without assuming that an announced
feature survived every reset or exists in today's repository.

- [Archive coverage and source limits](#archive-coverage-and-source-limits)
- [Historical development](#historical-development)
- [What changes across the history](#what-changes-across-the-history)
- [Theme finder](#theme-finder)
- [Reading paths](#reading-paths)
- [Complete episode index](#complete-episode-index)
- [Archive corrections and scope](#archive-corrections-and-scope)
- [From historical ideas to the current repository](#from-historical-ideas-to-the-current-repository)
- [Maintaining the archive](#maintaining-the-archive)

## Archive coverage and source limits

The episode titles and video links for 001–287 follow the owner’s definitive
catalog supplied on 2026-09-26. Episode 288 retains its current working title
and incomplete status. The local files include transcripts, prepared remarks,
and scripts, even when the catalog provides a published-video link. Most files
identify a source URL or local media filename; some specify edits, offsets,
transcription models, and coverage. A local `.mp4` name is provenance, not a
download link. A `Generated at` date is a transcription date, not necessarily
the recording or publication date.

| Record | How to read it |
| --- | --- |
| [001](001.md)–[260](260.md), and later recorded episodes | Read the individual header for source and coverage. Machine transcription can corrupt names, numbers, and whole passages. A transcript reports what was said; it does not independently verify a demonstration or benchmark. |
| [086](086.md) | Only the OpenAgents MVP Launch excerpt from PlebLab Startup Day 2024, 00:58:21–01:10:10; not the full eight-hour event recording. |
| [237](237.md) | A substantial launch essay/prepared-remarks record as well as the launch narrative. Its economic argument is broader than an implementation report. |
| [261](261.md) | The retained text still says prepared script / future-episode draft; the owner’s catalog identifies the video as **Sarah Meets World**. Preserve the distinction between the available text and the linked video. |
| [262](262.md) | Final Sarah script for Project Omega. A final script is not proof that every described capability shipped. |
| [263](263.md) | The current record is **Bitcoin Wallets Under Attack**. An earlier Omega Alpha script occupied this slot; see the correction history below. |
| [268](268.md) | A posted Sarah broadcast/script. Its warnings about Claude are the episode's rhetoric, not independently established findings. |
| [282](282.md) | Short statement, **Not Asking Permission**, without timestamp or transcription metadata. Its video link comes from the owner’s catalog. |
| [284](284.md) | The retained source is `284draft1.mp4`; the owner’s catalog supplies the published title and link. Keep the local source designation rather than silently treating it as a different edit. |
| [285](285.md) | **Bendcoder**, a 02:10:55.02 prototype session. It tests an approach and language, not comparative benchmark superiority. |
| [287](287.md) | A 02:35:36.90 build-and-measurement session. Results discussed during iteration must retain their configuration, task set, and measurement limits. |
| [288](288.md) | Explicitly incomplete; currently 02:08:28.40 across clips a–f, g2, h, i, and j. Its header lists coverage gaps and concatenation offsets. |

The summaries use *plans*, *proposes*, *announces*, *demonstrates*, and
*reports* deliberately. An announcement can establish historical intent;
it cannot establish present availability. Political, security, scientific,
and commercial claims remain attributed to the speaker. Verify exact wording
against the original media before quoting, and use retained artifacts for
claims about software behavior or comparative performance.

## Historical development

Episode order is the useful chronology here. Product families overlap, and
later products revisit problems that an earlier implementation already tried
to solve. These are changes in emphasis, not clean replacement boundaries.

| Episodes | Main development | Historical significance |
| --- | --- | --- |
| [001](001.md)–[019](019.md) | Laravel/Inertia app, document chat, embeddings, and pgvector. | Starts with a concrete user workflow; moves from hosted retrieval to an owned implementation. |
| [020](020.md)–[032](032.md) | Faerie reads issues, retrieves code, proposes changes, commits, and encounters failing tests. | Establishes the coding loop and the gap between producing code and reliably completing work. |
| [033](033.md)–[047](047.md) | Inspectability, AgentGraph, modular brains, Concierge, Sleuth, and early payment design. | Makes visible intermediate work and composable behavior part of what an open agent should mean. |
| [048](048.md)–[075](075.md) | WASM plugins, registries, host functions, L402, Nostr publication, and executable nodes. | Moves the reusable unit from a whole agent toward a tool or plugin another agent can use. |
| [076](076.md)–[102](102.md) | Streaming chat, multimodal input, API design, MVP launch, agent store, pricing, and revenue sharing. | Connects extensibility to distribution and contributor economics. |
| [103](103.md)–[125](125.md) | AutoDev, artifacts, codebase context, diffs, v2, SWE-bench, and GraphRAG. | Coding becomes the commercial focus; evaluation and repository understanding become explicit concerns. |
| [126](126.md)–[138](138.md) | OpenPress, website editing, v3, CRM, teams, and projects. | Tests whether the same agent machinery can become useful business software, with actual data boundaries. |
| [139](139.md)–[155](155.md) | Onyx, mobile voice, local models, wallets, data vending, OSINT, and knowledge graphs. | Brings agent control to the phone and connects private context to an open network. |
| [156](156.md)–[182](182.md) | Repo maps, relevant files, coding loops, MCP, Commander, sync, payments, and swarm inference. | Focuses on selecting context and orchestrating tools while widening the possible execution locations. |
| [183](183.md)–[198](198.md) | Tauri reset, Claude/Codex integration, dashboards, Tricoder, mobile/web access, and Cursor study. | Reopens the question of the operator's interface and dependence on another vendor's harness. |
| [199](199.md)–[213](213.md) | Autopilot, Pylon/Nexus, recursive models, identity, online agents, Bitcoin, and markets. | Joins the personal-agent interface to a proposed network of providers and paid work. |
| [214](214.md)–[240](240.md) | Compute/data markets, Psionic, Probe, distributed training, bounties, Sites, Forum, Tassadar, and the run board. | Expands the economic thesis, then makes accepted work, verification, energy, and payouts central to it. |
| [241](241.md)–[247](247.md) | Khala orchestration, OpenCode/Codex integration, Khala Code, dogfooding, and selling. | Tests orchestration in real tools and discovers that capacity, continuity, and dependable UX are prerequisites for the market story. |
| [248](248.md)–[259](259.md) | Desktop, inspectable subagents, ProductSpec, AssuranceSpec, Observer, FastFollow, release candidates, and incident analysis. | Turns user complaints and product promises into explicit intent, verification obligations, and retained evidence. |
| [260](260.md)–[274](274.md) | Sarah/Omega, Bitcoin-security work, Immortal, Nostr markets, BEAM continuity, and Agent Forge. | Explores resilient ownership of both infrastructure and product operation. Scripts, demos, security hypotheses, and announcements coexist. |
| [275](275.md)–[284](284.md) | Coder terminal/cloud, provider-neutral delegation, Linux, mobile sync, the right to develop AI, and game-like operator design. | Pulls earlier capabilities toward one product suite while keeping advanced work visible and controllable. |
| [285](285.md)–[288](288.md) | Bendcoder, TypeSafe/System One design, Coder One, Jev probes, Terminal-Bench, Microluna, Gym, reusable pattern components, and Fire Loop. | Shifts attention from wrapping strong agents to removing unnecessary model work and proving which improvements generalize. |

## What changes across the history

The following is editorial synthesis grounded in the linked episodes. It is
not a declaration that every historical product remains supported or that
the series establishes a completed universal agent system.

### Coding repeatedly supplies the concrete test

Faerie's issue-to-change loop in [020–032](020.md) precedes AutoDev's
repository context and diffs in [103–117](103.md), the explicit benchmark
interest in [120–121](120.md), and the basic coding loop in [156–163](156.md).
Probe, Bendcoder, and the later Coder suite return to the same practical question:
can the system finish a task another person can inspect and use?

That continuity matters more than the repeated naming changes. Early episodes
often demonstrate one stage with manual help. Later episodes try to make
delegation, task state, and verification survive unattended work. [287](287.md)
reduces the experiment to a small controlled loop; [288](288.md) exposes why
a worker's own green checks are insufficient. The history is a progression
in what counts as completion, not an uninterrupted march of autonomous wins.

### Reusable components are the oldest network-effect argument

The plugin work in [048–075](048.md) already asks builders to contribute
pieces that other agents can use. The store and revenue-sharing arc in
[092–102](092.md) supplies a distribution and compensation story. MCP in
[144](144.md), [165](165.md), and [168](168.md) offers another way to reach
tools; Khala’s independently optimizable programs in [242](242.md) and
Khala Code’s proposed trace-derived plugins in [245](245.md) move reusable
behavior into orchestration and learning from real work.

[269](269.md), [284](284.md), and [286](286.md) sharpen this into a claim
about compounding knowledge: repeatedly generating the same solution is waste
when an exact, inspectable component could be reused. [288](288.md) adds the
harder condition: a component should capture a general pattern and prove
usefulness outside the tasks it was learned from. A registry alone creates
availability; trustworthy selection, compatibility, evidence, and incentives
are what could make reuse improve the agent.

### Context becomes a controlled input rather than an ever-growing chat

PDF retrieval in [010–019](010.md), repository embeddings in [023](023.md),
Greptile and GraphRAG in [107](107.md) and [122](122.md), and repo maps in
[156–160](156.md) are successive attempts to show the model the useful part
of a larger world. Memories and reflections in [113](113.md) address a
related question: what should persist between actions?

[202](202.md) introduces recursive language model ideas. [286](286.md)
makes explicit state, query-aware context, conditional instructions, progressive
tool loading, and background reads part of one design. It does not say cache
reuse is always bad; it asks when the benefit of rebuilding context exceeds
the cost. [287](287.md) and [288](288.md) turn that design into measurements
of briefings and short executor sessions. The enduring problem is preserving
the right evidence and constraints while avoiding repeated irrelevant work.

### Inspectability develops into a standard of proof

[033–035](033.md) makes agent inspection a product requirement. Graphs,
HUDs, diffs, reasoning streams, and dashboards later show more of the work.
The Desktop episodes [248–250](248.md) demonstrate that missing child chats,
misleading model labels, and invented operational commands break trust even
when the underlying models are capable.

Product promises in [234](234.md), the accepted-outcome argument in
[237](237.md), UX contracts in [246](246.md), and Assurance in [252](252.md)
raise the standard from visible activity to evidence of the claimed result.
[264–265](264.md) apply the same distinction to security findings.
Gym in [288](288.md) brings it back to agent development: retain complete
transcripts, inspect failures, identify misleading checks, and compare actual
attempts. A signed statement, a test result, a commercial acceptance, and a
payment each establish different things.

### The market grows from charging for chat to buying accepted work

The early funds-flow and Lightning episodes [037](037.md), [062–064](062.md),
and [093–099](093.md) ask how an agent or its builder gets paid. Onyx,
Pylon, data vending, and the later market episodes broaden the potential
seller from an agent author to a data owner, compute provider, verifier,
or worker. [213–215](213.md) are useful anchors for this expansion.

[228–239](228.md) connect supply to demand: free entry, useful products,
referrals, accepted outcomes, and contributor payouts. [247](247.md) makes
selling and fulfillment explicit. These are economic proposals and reported
experiments, not evidence that supply automatically produces paying demand.
The repeated warning is concrete: [100](100.md) and [103](103.md) find market
plumbing ahead of agents worth paying for; [138](138.md) reports stronger
traction from business problems; [174](174.md) recalls compute sellers without
enough buyers. The historical lesson for agent labor is to keep the buyer’s deliverable,
acceptance criteria, provider costs, and actual payment visible together.

### Collective work needs more than a larger audience
+
+[200](200.md), [230](230.md), and [237](237.md) invoke group-forming
+networks and Reed’s law as a strategic argument: agents could find useful
+work, recruit collaborators, and compose contributions at machine speed.
+This is a hypothesis about the value of possible collaboration, not a
+measured growth law or proof that every additional participant improves quality.
+
+The concrete work in the archive supplies the conditions for that hypothesis:
+discoverable components, usable context, capacity, task ownership, independent
+checking, and payment for useful contributions. [249](249.md) and
+[250](250.md) show how even a local fleet needs inspectable relationships and
+honest state. [284](284.md) proposes roles and productive group activity;
+[286](286.md) connects shared state and coordination to reusable protocols.
+Network effects would come from contributions that remain useful to others,
+not from counting agents, tokens, or speculative groups alone.
+
+### Open protocols answer concentration and continuity problems

Nostr begins as registry, login, and storage infrastructure in [066](066.md),
[082](082.md), and [088](088.md), then supports the mobile/network story.
[204–207](204.md) connect platform dependence to broken workflows and key
ownership. [266–267](266.md) use the reported Boltz shutdown to argue for
negotiation that can survive one coordinator disappearing.

The swap implementation discussed there is one historical application.
The transferable idea is independent participants sharing an inspectable
agreement and verifying their own obligations. It does not require Coder to
become a swap product. [271–274](271.md) ask a parallel question about live
agents and source hosting: can work survive deployment, host replacement,
or loss of a central service? Open source, an open wire format, operational
continuity, and permission to act are related but distinct requirements.

### Open contribution does not make all data public
+
+The archive explores several disclosure models. [046](046.md) tells users to
+assume uploaded knowledge is public. [215](215.md) proposes opt-in sale of
+redacted traces; [245](245.md) distinguishes data-sharing and private tiers;
+[269](269.md) separates open infrastructure from Sarah’s private product.
+Those are different arrangements, not one privacy policy carried through every
+generation. Episode [285](285.md) even changes its intended open-core/private
+split within the session before announcing the open-sourcing of Coder.
+
+The reusable lesson is to specify who owns an input, who may see it, and what
+reuse or compensation has been agreed. Access to a trace is not automatic
+permission to publish it, train on it, or pay its author under terms that
+were never accepted. This is central to the proposed knowledge and labor
+networks, not only to a future data marketplace.
+
+### Provider neutrality coexists with dependence on real providers

The series repeatedly studies, integrates, criticizes, replaces, and returns
to outside models and harnesses: [104](104.md), [109](109.md), [185](185.md),
[190](190.md), [197](197.md), [241–244](241.md), and [278–279](278.md).
This is not a simple history of eliminating all external software. It is an
attempt to keep the user's workflow from belonging to one supplier.

The useful distinction is between interchangeable execution and uniform
behavior. Different providers still have different tools, prices, limits,
failure modes, and context semantics. [244](244.md) and [250](250.md) make capacity and readiness
real operational concerns; [287](287.md) distinguishes an after-the-fact best-choice
portfolio from a router that actually makes those choices. Neutrality creates
room to choose; measurements must establish whether that choice helps.

### The interface alternates between a narrow tool and a broader workroom

OpenPress and CRM in [126–138](126.md), Onyx and voice in [139–153](139.md),
Commander and the spatial episodes, and Desktop/Omega all test different ways
to direct the same kinds of work. [196](196.md) argues against terminal-only
UX; [275](275.md) later returns to a deliberately basic terminal because the
operator needs a dependable daily tool. That is a change in delivery strategy,
not evidence that the web, phone, voice, or operating-system ideas disappeared.

[275–281](275.md) explicitly present Coder as a suite spanning local and
cloud execution and several clients. [116](116.md), [176](176.md),
[189](189.md), [240](240.md), and [283–284](283.md) supply the recurring
game/UI argument: make many agents, their work, and their economics legible.
Episode [189](189.md) goes beyond interface metaphor by announcing the
Ruins of Atlantis game; its agent and Bitcoin integration remains future work.
The unresolved design test is whether each new surface improves control and
understanding rather than merely showing more activity.

### Resets and dogfooding expose the cost of a broad product ambition

The archive includes web-stack experiments, v2/v3, the explicit reset in
[183](183.md), the monorepo in [233](233.md), the runtime change in
[253](253.md), and the subsequent Desktop/Omega/Sarah/Coder shifts.
These are not one continuous deployed codebase. A feature shown in one era
cannot be assumed present after a rebuild.

The recurring corrective is using the product for its own development:
AutoDev in [112](112.md), OpenAgents modifying itself in [192](192.md),
Khala Code in [246](246.md), Desktop in [254](254.md), and Coder/Gym in
[275](275.md) and [288](288.md). This supplies concrete failures and a
fast feedback loop. It also creates a bias toward the founder's workload,
which makes external users and held-out tasks necessary tests of generality.

### The latest measurement arc revises its own success story

[285](285.md) first tests the approach in Bendcoder, using Bend and C around
Jev and generation. The session exposes missing context, tool failures, and
false completion; the host explicitly leaves Bend’s contribution versus Jev’s
unresolved. [286](286.md) supplies the broader design argument;
[287](287.md) builds and compares
configurations; [288](288.md) turns the benchmark viewer into a tool for
finding out why they succeed or fail. The later episode distinguishes cheap
typed judgments, deterministic checks, and generative work, then examines the
limits of the resulting Microluna experiments.

The edit history is part of that record. Commit
[`cd421c5dcb`](https://github.com/OpenAgentsInc/openagents/commit/cd421c5dcbeb1fcdb91ef274d51be36ae6c20f6a)
removed clip `288g`, whose Microluna v7 success announcement preceded analysis
showing an in-sample result that did not reproduce. The replacement g2 and
later clips discuss the retained v3 result, correct the spoken cost ratio,
and distinguish runtime contamination from design-time fitting. The README
must not resurrect the removed clip as a confirmed general win.

The later principle is to learn reusable patterns rather than fit task wording.
That connects back to plugins and open knowledge, but with an evidence
requirement: count failures and unknowns, include the cost of checks and
setup, and evaluate components on tasks that did not teach them. The episode's
final oracle discussion remains a report of work and pending comparisons at
recording time. It also reports that tuning Jev settings did not beat the
existing settings, so that experiment made no promotion. Negative results
belong in the learning record alongside the useful components.
+
+The closing clips introduce **Fire Loop**: expose parameters and decisions,
+detect an unproductive run early, stop it, and return an explanation that helps
+the next experiment. The recording reports one failing shadow run stopped;
+it does not establish the proposed five-to-ten-second iteration target or a
+new comparative coding win. This continues the early inspectability theme by
+making the speed of finding and understanding failure an engineering objective.
+Current outcomes belong in the
[Terminal-Bench results index](../terminal-bench/README.md), not in a timeless
claim that the archive proves Coder is universally better.

## Theme finder

These are representative entry points, not an exhaustive tag list for every
episode. The complete index below covers every retained numbered file.

| Theme | Development to follow | Episode anchors |
| --- | --- | --- |
| Open agents and user ownership | Open tooling, inspectable behavior, portable identity, and resistance to platform capture. | [001](001.md), [047](047.md), [101](101.md), [150](150.md), [166](166.md), [204](204.md), [253](253.md), [266](266.md), [278](278.md), [282](282.md) |
| Agent architecture | Plans/actions and modular brains become explicit state, typed judgments, and bounded generation. | [005](005.md), [036](036.md), [040](040.md), [157](157.md), [202](202.md), [242](242.md), [285](285.md), [286](286.md), [287](287.md) |
| Coding agents and repository work | Faerie, AutoDev, repo maps, issue solving, Probe, and Coder One. | [020](020.md), [030](030.md), [103](103.md), [117](117.md), [123](123.md), [156](156.md), [161](161.md), [218](218.md), [287](287.md) |
| Retrieval and context | Documents, embeddings, code/knowledge graphs, relevant files, and dynamic context. | [010](010.md), [013](013.md), [019](019.md), [023](023.md), [107](107.md), [122](122.md), [155](155.md), [160](160.md), [286](286.md) |
| Memory and reusable knowledge | Reflections and explicit state develop toward reusable components with evidence of applicability. | [113](113.md), [155](155.md), [202](202.md), [270](270.md), [286](286.md), [288](288.md) |
| Plugins, skills, and programs | WASM, registries, host functions, MCP, optimizable programs, and the agentic package-registry idea. | [048](048.md), [053](053.md), [058](058.md), [066](066.md), [075](075.md), [102](102.md), [165](165.md), [245](245.md), [286](286.md) |
| Inspection and trace navigation | Node graphs and HUDs become child transcripts, effective-model reporting, replay, and ranked findings. | [033](033.md), [038](038.md), [061](061.md), [111](111.md), [162](162.md), [249](249.md), [250](250.md), [288](288.md) |
| Verification and truthful claims | Tests, benchmark design, product promises, exact evidence, and independent checks. | [006](006.md), [029](029.md), [120](120.md), [234](234.md), [246](246.md), [252](252.md), [265](265.md), [288](288.md) |
| ProductSpec, Assurance, and FastFollow | Separate intended behavior, proof obligations, execution, and learning from other products. | [248](248.md), [251](251.md), [252](252.md), [255](255.md), [258](258.md), [259](259.md) |
| System One and Jev | Cost-aware context, typed judgments, evidence-first briefing, and short executor sessions. | [285](285.md), [286](286.md), [287](287.md), [288](288.md) |
| Iteration speed and Fire Loop | Inspect runs while they happen, explain unproductive paths, and measure whether early stopping reduces experimental waste. | [033](033.md), [246](246.md), [254](254.md), [287](287.md), [288](288.md) |
| Benchmarks and generalization | SWE-bench study becomes Terminal-Bench experimentation, failure inspection, and recognition of task fitting. | [120](120.md), [121](121.md), [186](186.md), [217](217.md), [287](287.md), [288](288.md) |
| Delegation and model choice | Parallel agents, own-capacity routing, readiness, bounded tasks, and cross-provider comparison. | [179](179.md), [191](191.md), [241](241.md), [243](243.md), [244](244.md), [250](250.md), [278](278.md), [287](287.md) |
| Bitcoin and payments | Balances, L402, withdrawals, pricing, revenue share, wallets, and payment for accepted work. | [037](037.md), [062](062.md), [064](064.md), [097](097.md), [098](098.md), [169](169.md), [207](207.md), [223](223.md), [235](235.md) |
| Group-forming networks | The Reed’s-law argument, collaborator discovery, shared work, and the conditions for useful collective learning. | [200](200.md), [230](230.md), [237](237.md), [249](249.md), [284](284.md), [286](286.md), [288](288.md) |
| Disclosure and data rights | Public uploads, opt-in trace sharing, private tiers, source ownership, and limits on reuse. | [046](046.md), [137](137.md), [147](147.md), [215](215.md), [245](245.md), [269](269.md), [285](285.md) |
| Agent labor and contributor incentives | Paid coding, bounties, free-entry economics, verification, and the coding-agent pool. | [103](103.md), [213](213.md), [225](225.md), [228](228.md), [230](230.md), [237](237.md), [247](247.md), [284](284.md) |
| Markets and demand | Agent/plugin stores expand into compute, data, labor, referrals, sales, and fulfillment. | [085](085.md), [092](092.md), [141](141.md), [147](147.md), [213](213.md), [215](215.md), [239](239.md), [247](247.md), [267](267.md) |
| Nostr and resilient infrastructure | Registry/login/storage, data vending, provider coordination, independent market participants, and open agent contracts. | [066](066.md), [082](082.md), [088](088.md), [142](142.md), [177](177.md), [203](203.md), [266](266.md), [267](267.md), [286](286.md) |
| Compute, local models, and Pylon | Phone inference, idle hardware, swarm inference, provider nodes, and resource orchestration. | [144](144.md), [145](145.md), [174](174.md), [178](178.md), [201](201.md), [203](203.md), [214](214.md), [221](221.md), [241](241.md) |
| Psionic and distributed training | Rust ML, inference benchmarks, training assignments, validation, and Tassadar. | [216](216.md), [217](217.md), [222](222.md), [223](223.md), [224](224.md), [236](236.md), [237](237.md), [238](238.md) |
| Energy and the economics of work | Ocean-power proposals and accepted outcomes per unit of energy. | [194](194.md), [227](227.md), [232](232.md), [237](237.md), [259](259.md) |
| Websites and business software | OpenPress, site editing, CRM, teams, projects, Autopilot Sites, and Lead Gen. | [126](126.md), [132](132.md), [135](135.md), [136](136.md), [137](137.md), [180](180.md), [229](229.md), [239](239.md), [247](247.md) |
| Mobile, voice, and cross-device control | Onyx, pocket coding, sync, Sarah voice, and Coder across terminal and phones. | [139](139.md), [151](151.md), [152](152.md), [172](172.md), [187](187.md), [193](193.md), [260](260.md), [270](270.md), [281](281.md) |
| Spatial UI, games, and operator experience | HUDs, multiplayer, hand tracking, dense workrooms, run boards, XP, and the Verse. | [111](111.md), [116](116.md), [176](176.md), [189](189.md), [195](195.md), [240](240.md), [249](249.md), [283](283.md), [284](284.md) |
| Domain agents, OSINT, and source evaluation | Concierge/Sleuth, document investigation, public-event analysis, and data markets. | [041](041.md), [043](043.md), [046](046.md), [076](076.md), [146](146.md), [154](154.md), [155](155.md), [182](182.md), [215](215.md) |
| Security and vulnerability evidence | Wallet-risk hypotheses, Loupe experiments, reproducibility, triage, and responsible disclosure. | [207](207.md), [263](263.md), [264](264.md), [265](265.md) |
| Sarah, Omega, and Forge | A named long-running agent, an editor foundation, service continuity, and owned source hosting. | [260](260.md), [261](261.md), [262](262.md), [268](268.md), [270](270.md), [271](271.md), [272](272.md), [273](273.md), [274](274.md) |
| Product resets and stack choices | Pragmatic web experiments, Tauri, monorepo consolidation, runtime changes, and the return to a small terminal. | [002](002.md), [050](050.md), [078](078.md), [134](134.md), [183](183.md), [233](233.md), [253](253.md), [275](275.md) |
| Research and competitor study | NeurIPS, OpenDevin/CodeAct, Genesis, DeepSeek, Cursor, RLMs, Fugu, AMP, and Claude Code architecture. | [036](036.md), [104](104.md), [109](109.md), [148](148.md), [181](181.md), [197](197.md), [202](202.md), [241](241.md), [255](255.md), [279](279.md) |
| Distribution, openness, and business strategy | Open beta, neutrality, free access, referrals, sell-in-public, and last-mover positioning. | [086](086.md), [092](092.md), [125](125.md), [150](150.md), [228](228.md), [239](239.md), [247](247.md), [269](269.md), [277](277.md) |

## Reading paths

Each path is a short selection; follow the era tables and episode index for
the intervening implementation work.

| Question | Read in this order |
| --- | --- |
| What is the enduring OpenAgents thesis? | [001](001.md) → [005](005.md) → [125](125.md) → [141](141.md) → [230](230.md) → [237](237.md) → [269](269.md) → [286](286.md) |
| How did the coding loop develop? | [020](020.md) → [025](025.md) → [030](030.md) → [103](103.md) → [117](117.md) → [161](161.md) → [218](218.md) → [287](287.md) → [288](288.md) |
| Why build reusable programs and knowledge? | [048](048.md) → [053](053.md) → [066](066.md) → [102](102.md) → [165](165.md) → [242](242.md) → [245](245.md) → [286](286.md) → [288](288.md) |
| Why do traces and independent checks matter? | [033](033.md) → [120](120.md) → [234](234.md) → [246](246.md) → [250](250.md) → [252](252.md) → [265](265.md) → [288](288.md) |
| How do agent markets connect to the product? | [037](037.md) → [098](098.md) → [141](141.md) → [213](213.md) → [214](214.md) → [215](215.md) → [237](237.md) → [247](247.md) → [267](267.md) |
| What does the archive teach about demand? | [100](100.md) → [103](103.md) → [138](138.md) → [174](174.md) → [213](213.md) → [226](226.md) → [239](239.md) → [247](247.md) |
| Where do compute and training fit? | [145](145.md) → [174](174.md) → [201](201.md) → [203](203.md) → [216](216.md) → [224](224.md) → [232](232.md) → [236](236.md) → [238](238.md) |
| How did the operator interface become Coder? | [111](111.md) → [170](170.md) → [189](189.md) → [196](196.md) → [249](249.md) → [251](251.md) → [262](262.md) → [275](275.md) → [281](281.md) |
| What does the game-like direction mean? | [116](116.md) → [176](176.md) → [189](189.md) → [240](240.md) → [249](249.md) → [283](283.md) → [284](284.md) |
| How did mobile and voice develop? | [139](139.md) → [145](145.md) → [151](151.md) → [152](152.md) → [187](187.md) → [191](191.md) → [193](193.md) → [270](270.md) → [281](281.md) |
| What changed in the latest algorithm work? | [285](285.md) → [286](286.md) → [287](287.md) → [288](288.md), then the [current results](../terminal-bench/README.md) and [Coder design index](../coder/design/README.md) |

## Complete episode index

There is one row per retained numbered file. Titles follow the local record,
with the owner’s catalog taking precedence for 001–287. Summaries describe
the episode’s work or argument, not today’s feature availability. Video links
are the exact catalog URLs; they have not all been independently replayed.
Only the numbered episode records belong in this archive. Notes, preparation
documents, production runbooks, and external commentary are excluded.

### 001–019: Original app and document-aware chat

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`001` Intro](001.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1721942435125715086) | Introduces an open agent platform tied to GPUtopia, with open models, data, compute, Bitcoin payments, and contributor bounties. |
| [`002` Choosing a Tech Stack](002.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1721966796515754266) | Chooses a Laravel/Inertia/React web app for fast shipping and user feedback, while considering a Rust-backed desktop alternative. |
| [`003` Hello Laravel](003.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1721979219763155232) | Creates the first Laravel/Inertia application shell. |
| [`004` Deploying a Landing Page](004.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1722068606714835283) | Ships an initial landing page and deploy loop. |
| [`005` Agent First Principles](005.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1722274309727752427) | Defines agents around chat, plans, actions, users, and tasks. |
| [`006` Implementing Agent Data Models via TDD](006.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1722287956419871177) | Builds core data models with test-first discipline. |
| [`007` First Feature Tests](007.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1722313899771347362) | Adds product-level feature tests around the early app. |
| [`008` Agent UX Design](008.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1722742595409830389) | Turns the product thesis into concrete user flows and screens. |
| [`009` Building the UI](009.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1723164712957862115) | Implements PDF-upload and chat wireframes; chat responses are still simulated. |
| [`010` Connecting to Vectara](010.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1723203092647137636) | Connects uploaded documents to hosted retrieval infrastructure. |
| [`011` Chatting with a PDF](011.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1723525820357005661) | Connects the chat UI to Vectara document retrieval and answer synthesis, with retrieval limits and no conversational memory yet. |
| [`012` RAG Planning](012.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1723888973213286760) | Plans to replace Vectara with owned retrieval after encountering its corpus limit, with a GitHub coding agent as the next use case. |
| [`013` RAG First Principles](013.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1724432749275095365) | Designs document chunking, embeddings, retrieval, and answer synthesis around a local database and GPUtopia inference. |
| [`014` Embeddings 101](014.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1724509783086989333) | Explains vector embeddings and connects to GPUtopia’s embedding API; database relationships follow in later episodes. |
| [`015` Similarity Search](015.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1724568957598708192) | Configures local Postgres and pgvector similarity search. |
| [`016` PDF to Embeddings](016.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1724801372602950026) | Converts PDF pages into local embedding records. |
| [`017` Connecting the UI](017.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725197866409267544) | Connects the custom RAG backend into the user interface. |
| [`018` Connecting the UI, Part 2](018.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725246583623590158) | Retrieves matching PDF-page context through the UI; LLM answer synthesis is deferred to the next episode. |
| [`019` Chat with PDF, OpenAgents Edition](019.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725349984952827929) | Connects retrieved PDF context to GPUtopia inference and tests the end-to-end document question-answering loop. |

### 020–032: Faerie and the first coding loop

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`020` Planning a GitHub Agent](020.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725597044981617119) | Plans Faerie, a GitHub coding agent, using the Generative Agents memory-stream approach as a design reference. |
| [`021` Hello Faerie](021.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725910351563165748) | Introduces Faerie as an agent that reads and responds to issues. |
| [`022` Conversing with Faerie](022.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725928497367908432) | Adds issue comments and conversation context to Faerie. |
| [`023` Embedding our Codebase](023.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725948809593638971) | Embeds repository code so Faerie can retrieve implementation context. |
| [`024` Faerie Makes a Plan](024.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725969687102534110) | Gives Faerie relevant files and asks for a task plan. |
| [`025` Faerie Writes Code](025.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1725977712043372666) | Gets Faerie to generate implementation code, applies it manually, and identifies fixes before adding GitHub commit automation. |
| [`026` Faerie Commits Code](026.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1727018763915247784) | Adds automated commits and PR creation; the generated changes still need better task context and prompts. |
| [`027` Smarter Pull Requests](027.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1727424427825193041) | Improves planning and patch prompts, then discovers that an edit-only tool cannot complete a task requiring new files. |
| [`028` Creating New Files](028.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1727433378063135085) | Extends Faerie from edits into file creation. |
| [`029` Automating Tests](029.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1728590361805672788) | Configures GitHub Actions to run the Laravel tests on agent PRs, preparing failure results for the debugging loop. |
| [`030` Faerie Debugs Failing Tests](030.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1728614813675274300) | Feeds failed GitHub Actions results back to Faerie for diagnosis; an automatic repair-and-retest loop remains unfinished. |
| [`031` Faerie as Daemon](031.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1730253928896291251) | Reviews Faerie’s issue, commit, PR, and failure-diagnosis pieces and plans scheduled orchestration; test fixes still include manual work. |
| [`032` Toward Semi-Automation](032.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1731086330694651924) | Reviews imperfect PRs and makes inspectability the next step before scheduled, fully automatic operation. |

### 033–047: Inspectability, AgentGraph, and domain agents

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`033` Agent Inspectability Planning](033.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1731156734335398303) | Defines openness as inspectability and plans public views of agent runs, step inputs, outputs, and metadata. |
| [`034` Agent Inspection UX Design](034.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1731733390641050106) | Designs an interface for reviewing agent internals. |
| [`035` Agent Inspection UI](035.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1734044762255036737) | Demonstrates the developing run-inspection UI and plans a simpler repo-audit entry point before broader coding automation. |
| [`036` Agent Modules 101](036.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1738000844476371445) | Reviews NeurIPS, Voyager, and Generative Agents research to organize agents into reusable modules, with per-step payments as a future goal. |
| [`037` Flow of Funds](037.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1738221896234373387) | Proposes a funds-flow model for agent users, builders, and compute providers, including revenue splits and public accounting. |
| [`038` Agent Node Graphs](038.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1741887869055119630) | Introduces AgentGraph for visual step-by-step agent execution. |
| [`039` Component-Driven Development](039.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742232060821934216) | Builds an agent-metadata UI component with Storybook to demonstrate component-driven development. |
| [`040` Agent Brain Design](040.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742346953210388881) | Builds a validation, embedding, retrieval, and inference pipeline while working out the knowledge-base model. |
| [`041` Hello Concierge](041.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742609184875544613) | Connects a Concierge agent to the initial agent brain. |
| [`042` Agent Bitcoin Balance](042.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742952006166225330) | Adds Bitcoin-denominated balances to agent models and UI as groundwork for the proposed payment flows. |
| [`043` EpsteinGPT Postmortem](043.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742970915061731424) | Discusses suspected censorship of another creator’s EpsteinGPT and proposes an open, document-based journalism agent with contributor incentives. |
| [`044` Sleuth Agent Planning](044.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1742992606785622272) | Plans a new Sleuth agent for document investigation. |
| [`045` Agent Builder & Chat UI](045.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1744471277207773191) | Reviews an agent builder and chat interface for custom agents. |
| [`046` Hello EpsteinSleuth](046.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1745521898824356193) | Demonstrates PDF-backed EpsteinSleuth, exposes retrieval limits, and invites testing with an explicit warning that uploaded knowledge is public. |
| [`047` Reviewing the GPT "Store"](047.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1745545948908962228) | Critiques OpenAI's store and sharpens the open-agent counterposition. |

### 048–061: WASM plugins, registries, and task-runner UI

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`048` Brainstorming a Plugin System](048.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1745918872866173125) | Plans an Extism/WASM plugin architecture so community developers can supply agent logic, integrations, and payment components. |
| [`049` Plugin Registry Setup](049.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1746989980562464915) | Builds an experimental Rust/HTMX plugin-registry web app before bringing plugin support into the main Laravel application. |
| [`050` Exploring HTMX in Laravel](050.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1747325914650710363) | Tests HTMX as a simpler interactive Laravel path. |
| [`051` HTMX Bitcoin Price Ticker](051.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1747366650075025671) | Builds a Laravel and HTMX live Bitcoin price component. |
| [`052` HTMX Server Sent Events](052.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1747430710212702706) | Tests HTMX server-sent events for the Bitcoin ticker, reverts a production failure, and keeps caching as the next practical improvement. |
| [`053` Loading WASM Plugins](053.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1747791884414599350) | Loads WebAssembly plugins as agent extension units. |
| [`054` Uploading a Plugin](054.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1747994309549318228) | Builds plugin uploads and fee metadata, proposing recurring payments when developers’ plugins are used. |
| [`055` Plugin Registry UI](055.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1748445660146216995) | Builds the UI for listing and inspecting uploaded plugins. |
| [`056` Deleting JavaScript](056.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1748536252733739412) | Reboots the architecture around community-built WASM plugins and removes React, Inertia, Storybook, and other earlier application scaffolding. |
| [`057` Markdown Blog](057.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1748909829500842046) | Adds a Tailwind and Markdown blog foundation. |
| [`058` Agent Uses Plugin](058.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1749490769151287318) | Lets an agent call a plugin as part of work. |
| [`059` Agent Node Graphs, Litegraph Edition](059.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1749850948296380668) | Experiments with Litegraph for visual agent workflows. |
| [`060` Simpler Node Graph](060.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1749990397714055567) | Replaces Litegraph with a simpler custom graph view. |
| [`061` Task Runner UI](061.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1750252532348211598) | Builds a UI for running and observing agent tasks. |

### 062–075: L402, Nostr, and executable plugin graphs

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`062` Exploring L402](062.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1750729304504213964) | Studies Lightning-powered HTTP 402 payments for agents. |
| [`063` Agent Pays L402 Endpoint](063.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1751700732963672411) | Gives agents the ability to pay an L402 endpoint. |
| [`064` Lightning Withdrawals](064.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1752049402359754789) | Builds a user withdrawal UI for Bitcoin balances. |
| [`065` Exploring Code Llama 70B](065.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1752464706365755475) | Tests Code Llama 70B for repository analysis against 34B, finds hallucinations, and argues for use-case-specific collaborative evaluation. |
| [`066` Nostr Plugin Registry](066.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1752537446003118431) | Reviews and tests a community Nostr plugin-discovery PR, then defers deeper integration while the product UX and event shape develop. |
| [`067` Replacing ChatGPT](067.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1752830191213150334) | Plans a daily-use ChatGPT replacement that gathers code and documentation by URL and can switch inference providers. |
| [`068` URL Extractor Plugin](068.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1753092771206885743) | Writes a WASM plugin for extracting URLs from text. |
| [`069` URL Scraper Plugin](069.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1753237589945905373) | Builds a URL-to-Markdown scraping plugin and works through host-function limits, leading to a separately hosted service experiment. |
| [`070` L402 Plugin Deployment](070.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1753527990506721416) | Deploys the scraper behind an L402 endpoint and integrates paid access into the agent workflow. |
| [`071` PHP Host Functions](071.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1754643881172328732) | Exposes PHP host functions to plugin execution. |
| [`072` LLM Inference Plugin](072.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1755001183784075769) | Adds an LLM inference plugin as an agent node. |
| [`073` Agent Builder UI](073.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1757111834857926954) | Reviews an interactive agent-builder mockup; individual-node and full-flow execution are the next implementation steps. |
| [`074` Run Plugin Node](074.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1757172886173827116) | Runs an individual plugin node inside the builder. |
| [`075` Run All Plugins](075.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1757507045022945510) | Adds a full-run path across multiple plugin nodes. |

### 076–089: Streaming chat, MVP launch, and community extensions

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`076` Hello Connie](076.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1757845747259638222) | Turns the three-plugin contextual-inference sequence into Connie’s chat UI, prioritizing daily usability over more general graph behavior. |
| [`077` Chat UI Buildout](077.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1758212579225219464) | Rebuilds and refines the core chat UI. |
| [`078` Installing Livewire](078.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1758497309694767509) | Evaluates Laravel Livewire for interactive agent chat. |
| [`079` Livewire Agent Chat](079.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1758511214253031692) | Connects agent chat to Livewire. |
| [`080` Streaming Plugin Status](080.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1758640475819655189) | Streams plugin execution status into the UI. |
| [`081` Streaming LLM Response](081.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1758659094578340054) | Adds streamed LLM responses through a host-side override, leaving general streaming through WASM plugins unresolved. |
| [`082` Nostr Login](082.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1759695706611146859) | Reviews and merges a community Nostr-login contribution into the web application. |
| [`083` Multimodal Chat](083.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1761082689095090180) | Adds image and multimodal capability to agent chat. |
| [`084` Exploring Mistral Large](084.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1762189301192835112) | Adds Mistral Large and compares it with GPT-4 on a coding/API-design request; the result is an exploratory example, not a benchmark. |
| [`085` API Design](085.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1762596179643371596) | Designs an Assistants-style API joined to the agent store, extensible WASM workflows, and Bitcoin-based builder economics. |
| [`086` MVP Launch](086.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1768668266203902214) | Presents the MVP at PlebLab Startup Day, demonstrates chat and API tools, and pitches the plugin marketplace and contributor-payment model. |
| [`087` Meta-Agent's First Upgrade](087.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1770549462219329656) | Integrates a community weather plugin into the meta-agent and explains usage-based revenue sharing as the incentive for shared upgrades. |
| [`088` Nostr KV Storage Plugin](088.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1771282518630699184) | Reviews a community NIP-78 key-value storage plugin for agent state and communication and announces a two-million-sat bounty award. |
| [`089` Goodbye ChatGPT](089.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1777496991099998302) | Demonstrates the multi-model chat product and cancels the host’s ChatGPT and Claude subscriptions; custom-agent discovery is still forthcoming. |

### 090–102: Agent store, payments, and the search for useful agents

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`090` Agent Builder](090.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1786675616437190707) | Demonstrates creating and switching custom chat agents; knowledge uploads and broader agent-store discovery are still forthcoming. |
| [`091` Reviewing the GPT-4o Launch](091.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1790132358219194567) | Critiques the GPT-4o launch and argues that paid demand and developer incentives should guide an open agent marketplace. |
| [`092` Introducing the Agent Store](092.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1790500162491523138) | Launches the agent-store open beta with builder payouts; pay-per-use charging and automated revenue splitting develop in later episodes. |
| [`093` The Sats Must Flow](093.md) | [Watch](https://twitter.com/OpenAgentsInc/status/1790805640392122627) | Reviews a 100,000-sat payout to four agent builders through a semi-manual daily process weighted by usage. |
| [`094` Recap & Roadmap](094.md) | [Watch](https://x.com/OpenAgentsInc/status/1793768984288104510) | Reviews the multi-model chat and agent-store launches, then plans pay-as-you-go revenue share, social discovery, plugins, and visual workflows. |
| [`095` Streaming Money](095.md) | [Watch](https://x.com/OpenAgentsInc/status/1795078569128755648) | Explains the streaming-money thesis and plans usage-based revenue sharing; continuous payment distribution is not yet enabled. |
| [`096` Payments & Payouts](096.md) | [Watch](https://x.com/OpenAgentsInc/status/1795535732032831719) | Demonstrates platform balances and Lightning withdrawals while distinguishing them from the more decentralized protocol payments planned later. |
| [`097` User Pays Agent](097.md) | [Watch](https://x.com/OpenAgentsInc/status/1795879532525695228) | Charges a fixed three sats per agent message and records agent earnings; builder and plugin payout distribution is the next step. |
| [`098` Agent Revenue Sharing](098.md) | [Watch](https://x.com/OpenAgentsInc/status/1796195661752246705) | Enables minute-by-minute distribution of agent earnings to builder balances; plugin-author splits are still planned for the next integration. |
| [`099` Lightning Addresses](099.md) | [Watch](https://x.com/OpenAgentsInc/status/1797738481097077001) | Adds experimental Lightning-address deposits to complete the deposit, agent-payment, earnings, and withdrawal loop. |
| [`100` Looking Ahead](100.md) | [Watch](https://x.com/OpenAgentsInc/status/1798870330418831627) | Marks the store and recurring builder payouts as delivered, then prioritizes useful coding agents, plugin composition, APIs, mobile control, and an open applied-AI lab. |
| [`101` Molon Labe](101.md) | [Watch](https://x.com/OpenAgentsInc/status/1799208892536152095) | Frames open agents against proposed AI lockdown. |
| [`102` Agent Plugins UI](102.md) | [Watch](https://x.com/OpenAgentsInc/status/1800665114573521029) | Launches the plugin registry and agent-attachment UI with pricing and review controls; the demo does not conclusively verify that its sample answer came from a plugin. |

### 103–117: AutoDev, context, artifacts, and diffs

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`103` Planning AutoDev Agents](103.md) | [Watch](https://x.com/OpenAgentsInc/status/1804182987350606055) | Plans AutoDev as a paid coding-agent use case, combining codebase indexing, extensible plugins, and a shell/browser/editor workspace. |
| [`104` Tour of Devin Clones](104.md) | [Watch](https://x.com/OpenAgentsInc/status/1804943886143439272) | Studies Devin alternatives, including OpenDevin and WANIX's browser-based development environment; an OpenDevin demo needs manual local execution. |
| [`105` Reaction: Progrium Technology Thesis](105.md) | [Watch](https://x.com/OpenAgentsInc/status/1805320338064261219) | Reviews Jeff Lindsay's technology thesis and connects generative building blocks, composability, and simpler software to the agent/plugin architecture. |
| [`106` Brainstorming Agentic Artifacts](106.md) | [Watch](https://x.com/OpenAgentsInc/status/1805676785390305377) | Shows a WANIX-in-browser proof of concept and uses Claude Artifacts as inspiration for an extensible, GitHub-connected agent workspace. |
| [`107` Codebase Indexing via Greptile](107.md) | [Watch](https://x.com/OpenAgentsInc/status/1806002683649667592) | Builds and exercises a Greptile indexing/query plugin and its interface, while debugging the repository-indexing form. |
| [`108` GitHub File Explorer](108.md) | [Watch](https://x.com/OpenAgentsInc/status/1806401750171373650) | Builds a GitHub repository/file explorer using WANIX's GitHub filesystem library; planning and surgical code edits remain subsequent steps. |
| [`109` Exploring OpenDevin & CodeAct](109.md) | [Watch](https://x.com/OpenAgentsInc/status/1806471593779937581) | Studies OpenDevin and CodeAct, then plans a Go adaptation of code-based actions and task planning rather than shipping it in this episode. |
| [`110` AutoDev Planner](110.md) | [Watch](https://x.com/OpenAgentsInc/status/1807873693613543523) | Briefly shows AutoDev generating an implementation plan and asking to execute it. |
| [`111` Heads-Up Display](111.md) | [Watch](https://x.com/OpenAgentsInc/status/1811428988385939793) | A short HUD teaser asks the agent to find related files and proposes editing them by voice; the transcript contains little implementation detail. |
| [`112` Using AutoDev](112.md) | [Watch](https://x.com/OpenAgentsInc/status/1815818389983564079) | Dogfoods AutoDev to build a 3D codebase-insights view, with human-guided fixes to generated code and data-model mismatches. |
| [`113` Agent Memories & Reflections](113.md) | [Watch](https://x.com/OpenAgentsInc/status/1816269923414327630) | Implements memory, retrieval, and reflection from the Generative Agents paper; reflection is manually invoked and planning is deferred. |
| [`114` Planning & Execution](114.md) | [Watch](https://x.com/OpenAgentsInc/status/1816689418809319911) | Connects planning to file memories, reflections, and execution; streamed planning UI and pull-request handling are still unfinished. |
| [`115` AutoDev as Pair Programmer](115.md) | [Watch](https://x.com/OpenAgentsInc/status/1818436887746347182) | Uses AutoDev for iterative UI edits and checks the changes locally; stronger pull-request checks and verification remain future work. |
| [`116` 3D Multiplayer](116.md) | [Watch](https://x.com/OpenAgentsInc/status/1819071289740644563) | Demos a multiplayer 3D workspace and argues that game interfaces can make human-agent collaboration inspectable; Bitcoin-powered world features are proposed. |
| [`117` AutoDev Git Diffs](117.md) | [Watch](https://x.com/OpenAgentsInc/status/1819475835713474868) | Announces AutoDev early access and builds a resizable Git-diff pane through repeated agent-assisted edits and manual inspection. |

### 118–125: Version two, benchmarks, GraphRAG, and the master plan

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`118` Version Two](118.md) | [Watch](https://x.com/OpenAgentsInc/status/1821751383227347101) | Previews v2's multi-pane GitHub-connected agent workspace, with tool chaining, repository switching, and a forthcoming pay-as-you-go beta. |
| [`119` v2 Beta Launch](119.md) | [Watch](https://x.com/OpenAgentsInc/status/1823109640357339628) | Launches the v2 beta with draggable chat panes, GitHub tools, free-model access, and paid credits; demonstrates ongoing self-development. |
| [`120` Exploring SWE-bench Verified](120.md) | [Watch](https://x.com/OpenAgentsInc/status/1823454256596213969) | Reviews SWE-bench Verified's design and limitations and creates evaluation-project documentation; no OpenAgents benchmark score is reported. |
| [`121` SWE-bench Planning](121.md) | [Watch](https://x.com/OpenAgentsInc/status/1823896252460704139) | Loads all 500 SWE-bench Verified cases into an explorer and plans the issue-to-patch harness and comparative run inspection. |
| [`122` Codebase Indexing via GraphRAG](122.md) | [Watch](https://x.com/OpenAgentsInc/status/1825094346938401060) | Prototypes a GraphRAG-style codebase index with versioned files, entities, relationships, and summaries; benchmark execution remains future work. |
| [`123` GitHub Issues to Pull Requests](123.md) | [Watch](https://x.com/OpenAgentsInc/status/1828629873063014755) | Demonstrates agent-generated GitHub pull requests and parallel issue-solving sessions; human review and infrastructure become the visible bottlenecks. |
| [`124` Magic AI = Deep State](124.md) | [Watch](https://x.com/OpenAgentsInc/status/1829238237925458015) | Criticizes Magic's fundraising and closed-lab regulation rhetoric; the political conclusions and competitor comparisons are the host's argument. |
| [`125` The Master Plan](125.md) | [Watch](https://x.com/OpenAgentsInc/status/1834087017132511419) | Sets the master plan: combine execution and knowledge graphs, sell useful agents, and reward contributors of components, data, and referrals. |

### 126–138: OpenPress, business applications, teams, and projects

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`126` OpenPress and the End of WordPress](126.md) | [Watch](https://x.com/OpenAgentsInc/status/1841710204691288356) | Starts OpenPress as a CC0, Laravel-based site-building project in response to WordPress disputes, creating its initial repository and app. |
| [`127` Hello OpenPress](127.md) | [Watch](https://x.com/OpenAgentsInc/status/1841932736384139366) | Builds and publishes OpenPress's first basic blog, using Blade and Alpine, with tests and authenticated post creation. |
| [`128` Styling OpenPress](128.md) | [Watch](https://x.com/OpenAgentsInc/status/1842019533470531729) | Adapts ShadCN-style components into Laravel Blade and iterates on OpenPress's layout, post controls, and light/dark styling. |
| [`129` Hosting OpenPress](129.md) | [Watch](https://x.com/OpenAgentsInc/status/1842353777816289527) | Designs OpenPress hosting and implements initial site-management screens; connecting the Laravel Forge deployment API remains next. |
| [`130` OpenAgents <> OpenPress](130.md) | [Watch](https://x.com/OpenAgentsInc/status/1846420419932307507) | Connects the agency/site-building business opportunity to OpenAgents v3 and starts a fresh Laravel application with a no-build-JavaScript goal. |
| [`131` v3 Landing Page](131.md) | [Watch](https://x.com/OpenAgentsInc/status/1846759852845990003) | Builds the v3 landing page with Laravel Blade, Tailwind, and shared OpenPress-style components while avoiding compiled JavaScript. |
| [`132` v3 Core Feature](132.md) | [Watch](https://x.com/OpenAgentsInc/status/1846967946947776716) | Defines v3's core promise as asking an agent to do work, starting with site building, and builds initial message/backend flows. |
| [`133` HTMX Server Sent Events, Part 2](133.md) | [Watch](https://x.com/OpenAgentsInc/status/1847028603969560599) | Gets concurrent HTMX server-sent-event demo streams working; the responses are simulated and are not yet persisted to the database. |
| [`134` Inescapable Inertia](134.md) | [Watch](https://x.com/OpenAgentsInc/status/1849537050212511922) | Reverses the no-build-JavaScript plan and returns to Inertia because the simpler-looking HTMX stack slowed agent-assisted development. |
| [`135` Agentic CRM Design](135.md) | [Watch](https://x.com/OpenAgentsInc/status/1851854221504635125) | Designs an agentic CRM around actual customer sales needs and plans its data models and tests; email automation remains an objective. |
| [`136` Teams](136.md) | [Watch](https://x.com/OpenAgentsInc/status/1852031199750811916) | Wires team creation, switching, and team-scoped chat lists; project-related permission problems are left for the next episode. |
| [`137` Projects](137.md) | [Watch](https://x.com/OpenAgentsInc/status/1852082485141573657) | Builds initial team-project organization and project creation, while proposing a model-neutral business dashboard with shared knowledge and plugins. |
| [`138` Year One Recap](138.md) | [Watch](https://x.com/OpenAgentsInc/status/1855464290771247126) | Reviews year-one lessons: a marketplace needs useful paid demand, business customers supplied revenue, and stack simplicity did not guarantee development speed. |

### 139–155: Onyx, mobile, voice, data markets, and OSINT

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`139` Going Mobile](139.md) | [Watch](https://x.com/OpenAgentsInc/status/1864878103853477910) | Introduces Onyx's mobile direction, reusing earlier Nostr-commerce work and aiming to combine personal agents, tools, Bitcoin, and open markets. |
| [`140` Open-Sourcing Onyx](140.md) | [Watch](https://x.com/OpenAgentsInc/status/1865245296097210497) | Announces the Onyx repository and tours its README; shared learning, earning Bitcoin, and the full agent network are product promises. |
| [`141` One Market](141.md) | [Watch](https://x.com/OpenAgentsInc/status/1866351898376405220) | Argues for one neutral global AI-services market over Nostr and Lightning, with Onyx as its user-facing gateway. |
| [`142` Data Vending Machines](142.md) | [Watch](https://x.com/OpenAgentsInc/status/1866695147632889902) | Demonstrates a mobile NIP-90 job receiving a haiku from a Groq-backed provider, then explains decentralized service discovery and composition. |
| [`143` Onyx as Bitcoin Wallet](143.md) | [Watch](https://x.com/OpenAgentsInc/status/1867070611928846640) | Demonstrates Onyx sending and receiving Bitcoin through the Breez SDK's Lightning/Liquid path; Spark is discussed as a future alternative. |
| [`144` Pylon and the Model Context Protocol](144.md) | [Watch](https://x.com/OpenAgentsInc/status/1867458253661114610) | Introduces Pylon and demonstrates a mobile MCP client browsing resources from a Rust desktop server; paid marketplace integration is planned. |
| [`145` Going Local](145.md) | [Watch](https://x.com/OpenAgentsInc/status/1867815868131836103) | Demonstrates small Llama models on the phone and larger models through MCP, contrasting local privacy, capability, speed, and hosted inference. |
| [`146` Sensemaking: Drones](146.md) | [Watch](https://x.com/OpenAgentsInc/status/1868910035281051854) | Compares AI answers about drone reports and prototypes a source/knowledge-graph app; better data and credible conclusions remain unresolved. |
| [`147` Planning a Data Marketplace](147.md) | [Watch](https://x.com/OpenAgentsInc/status/1869220404079800704) | Designs a Bitcoin-funded data marketplace combining agent collection, human contributions, and quality review; an MVP is still proposed. |
| [`148` Exploring the Genesis Physics Engine](148.md) | [Watch](https://x.com/OpenAgentsInc/status/1869631823182975426) | Reviews Genesis physics-engine demonstrations and brainstorms simulation, robotics, and marketplace uses; it does not implement those integrations. |
| [`149` Onyx Beta Launch](149.md) | [Watch](https://x.com/OpenAgentsInc/status/1870030269916340610) | Releases Onyx v0.0.1 through TestFlight and an Android APK, initially emphasizing model chat; wallet/market integration and Android performance need work. |
| [`150` Neutrality Wins](150.md) | [Watch](https://x.com/OpenAgentsInc/status/1870373254197613052) | Reviews the Onyx beta and argues that neutral protocols and model choice can support an open market; the complete product remains unfinished. |
| [`151` Speak to Onyx](151.md) | [Watch](https://x.com/OpenAgentsInc/status/1871390476705947913) | Demonstrates Onyx voice conversation using Groq-hosted chat and Whisper transcription; memory, tools, wallet, and marketplace upgrades remain planned. |
| [`152` Code by Voice](152.md) | [Watch](https://x.com/OpenAgentsInc/status/1872388364013379949) | Brings GitHub coding tools into Onyx v0.0.4 and demonstrates a phone-directed bug fix, with branch and tool-permission precautions. |
| [`153` High-Velocity Bitcoin](153.md) | [Watch](https://x.com/OpenAgentsInc/status/1874317831497462144) | Argues for Bitcoin as high-velocity agent money and previews wallet, Nostr, reward, and referral experiments rather than demonstrating their full rollout. |
| [`154` Agentic OSINT](154.md) | [Watch](https://x.com/OpenAgentsInc/status/1875380960658681918) | Proposes an agentic OSINT service with source collection, human quality review, a knowledge graph, and Bitcoin incentives around disputed public claims. |
| [`155` Open Knowledge Graph](155.md) | [Watch](https://x.com/OpenAgentsInc/status/1876857763504046458) | Designs an open knowledge graph using Nostr chat, labels, and contributor incentives; a public populated graph and payouts remain forthcoming. |

### 156–165: Repo maps, coding loops, reasoning, and MCP

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`156` Aider Repo Maps](156.md) | [Watch](https://x.com/OpenAgentsInc/status/1880151071706026012) | Publishes an Aider-based repository-map tool and outlines integrating proven open-source coding algorithms into mobile and web agent workflows. |
| [`157` Chains of Thought and Action](157.md) | [Watch](https://x.com/OpenAgentsInc/status/1886297781138030777) | Proposes alternating reasoning with tool use instead of relying on one reasoning generation, with an inspectable GitHub issue solver as the example. |
| [`158` Quest for the Holy Grail](158.md) | [Watch](https://x.com/OpenAgentsInc/status/1888019235063927061) | Defines a tested issue-to-pull-request loop and chooses a Rust/HTMX/Hyperview route for controlling that workflow from Onyx. |
| [`159` Onyx Repo Maps](159.md) | [Watch](https://x.com/OpenAgentsInc/status/1889540491143692592) | Implements repository-map generation in Onyx as the first step toward issue solving; the complete coding/verification loop is still ahead. |
| [`160` Relevant Files](160.md) | [Watch](https://x.com/OpenAgentsInc/status/1889766594798379419) | Builds relevant-file selection from a repository map and initially hardcoded issue; the model's completeness claim is not independent verification. |
| [`161` Basic Coding Loop](161.md) | [Watch](https://x.com/OpenAgentsInc/status/1889912687770882160) | Builds the initial code-change loop, state machine, and persisted status; reliable testing and mergeable pull requests remain further work. |
| [`162` Reasoning Stream](162.md) | [Watch](https://x.com/OpenAgentsInc/status/1890317831784333595) | Connects WebSocket events and DeepSeek reasoning output to the Onyx interface so the developing coding loop can be inspected. |
| [`163` Issue Reasoning Demo](163.md) | [Watch](https://x.com/OpenAgentsInc/status/1890616487536111878) | Demonstrates issue reasoning at PlebLab, including Bitcoin Core examples; limited code inspection and unverified hypotheses remain visible. |
| [`164` The New OpenAgents.com](164.md) | [Watch](https://x.com/OpenAgentsInc/status/1900279256606794235) | Tours the new chat product's free GitHub reads and paid coding tools, then uses it to implement local chat-history storage with Dexie. |
| [`165` Integrating MCP via AI SDK](165.md) | [Watch](https://x.com/OpenAgentsInc/status/1900282953244303440) | Integrates new AI SDK MCP support, debugs a test client, and plans a tool marketplace with evaluation-linked contributor revenue. |

### 166–182: Commander, payments, sync, compute, and sensemaking

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`166` OpenAI Delenda Est](166.md) | [Watch](https://x.com/OpenAgentsInc/status/1900430864623768056) | Responds to OpenAI's proposed restrictions and adds DeepSeek R1 through OpenRouter, discussing orchestration with tool-capable models and distillation. |
| [`167` Overnight Agent](167.md) | [Watch](https://x.com/OpenAgentsInc/status/1901964880594313542) | Reviews an overnight scheduled reflection chain using Cloudflare Agents; the output is speculative design work, not tested autonomous code delivery. |
| [`168` Remote MCP Server & Client](168.md) | [Watch](https://x.com/OpenAgentsInc/status/1905107323279855799) | Demonstrates a public remote GitHub MCP server and forthcoming Coder client, connecting easier tool access to a marketplace/network-effect strategy. |
| [`169` Agent Payments API](169.md) | [Watch](https://x.com/OpenAgentsInc/status/1919419077887410389) | Demonstrates an experimental Spark-based agent-wallet API at PlebLab, including transfers and rough reporting; broader payment-rail support is planned. |
| [`170` Commander](170.md) | [Watch](https://x.com/OpenAgentsInc/status/1919797578452869267) | Introduces Commander as a StarCraft-like interface for agent fleets, combining hotkeys, spatial controls, and Bitcoin-market ambitions. |
| [`171` Visualizing Agent Payments](171.md) | [Watch](https://x.com/OpenAgentsInc/status/1920222323963277553) | Demonstrates creating agent instances and sending sats between them in Commander's visual interface, backed by the agent-payments API. |
| [`172` Sync Engine](172.md) | [Watch](https://x.com/OpenAgentsInc/status/1920707845831409670) | Demonstrates Postgres/ElectricSQL state updates across two browsers with Effect, preparing shared agent canvases rather than a complete cross-device task runtime. |
| [`173` OpenAgents Bitcoin Wallet](173.md) | [Watch](https://x.com/OpenAgentsInc/status/1922303008617984363) | Launches the experimental OpenAgents web wallet and demonstrates Spark and Lightning transfers; explicitly presents it as a small-balance beta hot wallet. |
| [`174` GPUtopia 2.0](174.md) | [Watch](https://x.com/OpenAgentsInc/status/1922738011621687492) | Announces the GPUtopia reboot as OpenAgents Compute and names the earlier failure: plentiful sellers but too few buyers; agents are the proposed demand. |
| [`175` Commander v0.0.1](175.md) | [Watch](https://x.com/OpenAgentsInc/status/1923126952870703509) | Releases Commander's first developer build to test local Ollama chat; buying/selling compute, richer agent views, and wallet integration are forthcoming. |
| [`176` Hand Tracking](176.md) | [Watch](https://x.com/OpenAgentsInc/status/1923548136762466798) | A short visual teaser explores hand gestures and voice as ways to command agents. |
| [`177` Commander as Nostr Client](177.md) | [Watch](https://x.com/OpenAgentsInc/status/1924525410424938660) | Demonstrates Commander's early NIP-28 chat client and event stream; the full Nostr compute marketplace is still being assembled. |
| [`178` Swarm Inference](178.md) | [Watch](https://x.com/OpenAgentsInc/status/1926403708658544794) | Demonstrates Bitcoin-paid inference between the host's Mac and Linux desktop through Nostr; Go Online works in an alpha self-to-self market demo. |
| [`179` Claude Code Commander](179.md) | [Watch](https://x.com/OpenAgentsInc/status/1928569223636279514) | Dogfoods multiple Claude Code sessions in Commander with resumable history, while proposing decentralized tools and compute as additional capabilities. |
| [`180` Zero to Website in 60 Seconds](180.md) | [Watch](https://x.com/OpenAgentsInc/status/1933142272083730597) | Demonstrates quickly generating and opening a deployed Bitcoin-puns website; broader plugin, wallet, and custom-domain integration remains an ambition. |
| [`181` American DeepSeek?](181.md) | [Watch](https://x.com/OpenAgentsInc/status/1941327986327740800) | Reviews the American DeepSeek proposal, argues for globally open models, and connects that effort to model routing and an agent marketplace. |
| [`182` Sensemaking: Weather Modification & the Texas Floods](182.md) | [Watch](https://x.com/OpenAgentsInc/status/1942305552538698025) | Critiques AI research answers about Texas floods and weather-modification claims; this is a sensemaking investigation, not established causal evidence. |

### 183–198: Desktop resets, Tricoder, and mobile/web agent control

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`183` Zero Base](183.md) | [Watch](https://x.com/OpenAgentsInc/status/1948214004268064771) | Wipes the old codebase for another product reset, using Swift prototypes to motivate an open desktop/mobile wrapper around coding agents. |
| [`184` Hello Tauri](184.md) | [Watch](https://x.com/OpenAgentsInc/status/1948214009615765765) | Creates a fresh cross-platform Tauri shell and defines a conversation-centered desktop/mobile product rather than another code editor. |
| [`185` Hello Claude Code](185.md) | [Watch](https://x.com/OpenAgentsInc/status/1948366502081478897) | Connects Claude Code to the new multi-pane app and proposes agent actions-per-minute metrics and overnight orchestration; streaming still needs work. |
| [`186` Actions Per Minute](186.md) | [Watch](https://x.com/OpenAgentsInc/status/1948617421654245436) | Proposes agent actions per minute from messages and tool calls after criticizing benchmark quality; activity counts are not demonstrated task-success measures. |
| [`187` Mobile Sync](187.md) | [Watch](https://x.com/OpenAgentsInc/status/1948994586459705695) | Demonstrates two-way desktop/mobile conversation sync through Convex, with hardcoded pieces and beta release work still remaining. |
| [`188` The Dashboard](188.md) | [Watch](https://x.com/OpenAgentsInc/status/1951344146599256504) | Dogfoods a web dashboard over local Claude Code and Convex, including tool inspection and history; public release is explicitly deferred. |
| [`189` Toward an Agentic MMORPG](189.md) | [Watch](https://x.com/ArcadeCityHall/status/1973580845207396815) | Announces Ruins of Atlantis and its first game build, shifting from a game-like agent HUD toward an actual MMORPG with future AI/Bitcoin connections. |
| [`190` Goodbye Claude Code](190.md) | [Watch](https://x.com/OpenAgentsInc/status/1980707602804928791) | Explains the host's switch from Claude Code to open-source Codex and plans a better mobile/desktop/web coding-agent experience. |
| [`191` Project Tricoder](191.md) | [Watch](https://x.com/OpenAgentsInc/status/1981124099318415633) | Introduces Tricoder as phone control of desktop Codex over Tailscale and shows its first commit; broader multi-agent orchestration is an objective. |
| [`192` OpenAgents Upgrades Itself](192.md) | [Watch](https://x.com/OpenAgentsInc/status/1981533017999814688) | Demonstrates a phone-requested Codex UI change reaching the same app through an Expo over-the-air update. |
| [`193` Codex & Claude Code On Your Phone](193.md) | [Watch](https://x.com/OpenAgentsInc/status/1983960929575338281) | Releases the TestFlight/Tricoder workflow for Codex and Claude Code, using QR pairing over local Wi-Fi or Tailscale without an OpenAgents account. |
| [`194` The Trillion-Dollar Question](194.md) | [Watch](https://x.com/OpenAgentsInc/status/1986655639431500034) | Demonstrates Apple Foundation Models doing local agentic work and explores how much cloud inference could move to devices; the percentages are speculative. |
| [`195` Designing 10x Better](195.md) | [Watch](https://x.com/OpenAgentsInc/status/1988293182942228779) | Sets a ten-upgrade roadmap: desktop/mobile UX, overnight work, delegation, searchable history, integrations, open source, mixed inference, spare compute, and contributor payments. |
| [`196` Ditch the TUI](196.md) | [Watch](https://x.com/OpenAgentsInc/status/1988825875127607363) | Replays the ten-upgrade proposal, then demonstrates desktop chat-history navigation as the first upgrade away from terminal-only interfaces. |
| [`197` Reverse Engineering Cursor](197.md) | [Watch](https://x.com/OpenAgentsInc/status/1989020655602618370) | Studies Cursor's public material and installed app to draft an alternative architecture and roadmap; the 10x improvement and delivery dates are aspirations. |
| [`198` Claude Code on the Web](198.md) | [Watch](https://x.com/OpenAgentsInc/status/1991260906211164529) | Replays the upgrade proposal, then demonstrates a web chat UI connecting to local Claude Code through a script; packaged onboarding is still forthcoming. |

### 199–213: Autopilot, Pylon/Nexus, identity, and markets

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`199` Introducing Autopilot](199.md) | [Watch](https://x.com/OpenAgentsInc/status/2003362087955730508) | Introduces Autopilot as a long-lived personal agent and a harness around coding agents, with contributor payments and a desktop interface planned. |
| [`200` The Agent Network](200.md) | [Watch](https://x.com/OpenAgentsInc/status/2006956979298685216) | Sets out the agent-network thesis: spare compute, autonomous agents, open coordination, Bitcoin payments, and a two-sided market for useful work. |
| [`201` Fracking Apple Silicon](201.md) | [Watch](https://x.com/OpenAgentsInc/status/2008326849613476335) | Demonstrates Apple Foundation Models doing local tool-assisted code search and proposes renting idle Apple Silicon compute through a network. |
| [`202` Recursive Language Models](202.md) | [Watch](https://x.com/OpenAgentsInc/status/2008704591110541567) | Reviews recursive language models as a workload for distributed consumer compute and previews software for joining that network. |
| [`203` Pylon and Nexus](203.md) | [Watch](https://x.com/OpenAgentsInc/status/2009142870775644644) | Introduces Pylon worker nodes and Nexus coordination, connecting recursive workloads, coding agents, and payments; real Bitcoin settlement remains a later step. |
| [`204` DO NOT BREAK USERSPACE](204.md) | [Watch](https://x.com/OpenAgentsInc/status/2009660435188826131) | Responds to Anthropic restricting third-party OAuth access and argues that platform changes must preserve working user workflows. |
| [`205` Vintage Microsoft Evil Shit](205.md) | [Watch](https://x.com/OpenAgentsInc/status/2010429987703464142) | Examines restrictive service terms and records the decision to remove Anthropic integration from the product at that point. |
| [`206` Codex on Autopilot](206.md) | [Watch](https://x.com/OpenAgentsInc/status/2016039535134560301) | Demonstrates an early Autopilot interface for Codex and proposes structured, optimizable guidance between turns instead of manual continuation. |
| [`207` Your Keys, Your Coins, Your Identity](207.md) | [Watch](https://x.com/OpenAgentsInc/status/2016423268564001059) | Explains the desktop identity and wallet design: a seed phrase, derived Nostr and Spark keys, and a small-balance hot wallet. |
| [`208` Autopilot HUD](208.md) | [Watch](https://x.com/OpenAgentsInc/status/2016787108900335736) | Demonstrates the first Autopilot HUD alpha, including identity, a regtest wallet, and Codex event logs; replacement of Codex remains unfinished. |
| [`209` Open Moltbook](209.md) | [Watch](https://x.com/OpenAgentsInc/status/2018180415035408860) | Demonstrates Open Moltbook on Nostr and proposes portable agent identities, shared protocol discovery, credit, and contributor revenue sharing. |
| [`210` OpenClaw Online](210.md) | [Watch](https://x.com/OpenAgentsInc/status/2018837422583562450) | Introduces Hatchery as a planned hosted OpenClaw setup service, following the Open Moltbook launch. |
| [`211` Autopilot Online](211.md) | [Watch](https://x.com/OpenAgentsInc/status/2021629941272477809) | Demonstrates browser onboarding to Autopilot and outlines persistent conversation, personalized identity, automatic upgrades, and future paid skills. |
| [`212` Autopilot Learns Bitcoin](212.md) | [Watch](https://x.com/OpenAgentsInc/status/2024259092810703136) | Demonstrates conversational Bitcoin wallet operations and revisits L402 access, with monetized agent skills proposed next. |
| [`213` Agent Markets](213.md) | [Watch](https://x.com/OpenAgentsInc/status/2030132739672887561) | Announces five agent markets—compute, data, labor, liquidity, and risk—and identifies paying demand, rather than provider supply, as the bootstrap problem. |

### 214–227: Compute/data markets, Psionic, Probe, training, and energy

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`214` Compute Market](214.md) | [Watch](https://x.com/OpenAgentsInc/status/2032108547333304421) | Launches an Apple Silicon compute-market beta with Go Online and Bitcoin payouts; the live onboarding jobs are subsidized placeholder work. |
| [`215` Data Market](215.md) | [Watch](https://x.com/OpenAgentsInc/status/2035601131028500561) | Introduces a basic data market and NIP-DS, including opt-in sale of redacted coding traces; contributor payments underpin the proposed developer ecosystem. |
| [`216` Psionic](216.md) | [Watch](https://x.com/OpenAgentsInc/status/2036908227019809259) | Introduces Psionic, a Rust inference and training stack, and pauses the broader market rollout to focus on paid distributed model work. |
| [`217` Psionic: Fast Qwen 3.5](217.md) | [Watch](https://x.com/OpenAgentsInc/status/2037717730707542232) | Reports faster Qwen 3.5 token generation in selected Psionic configurations, crediting specialized kernels while explicitly limiting the performance claim. |
| [`218` Probe](218.md) | [Watch](https://x.com/OpenAgentsInc/status/2038995499446100420) | Introduces Probe, an early Rust coding-agent runtime intended to support multiple models and embed in Autopilot. |
| [`219` Probe: Inference Modes](219.md) | [Watch](https://x.com/OpenAgentsInc/status/2039434384705753460) | Demonstrates Probe using cloud, remote Psionic, and local Apple inference, then proposes routing smaller tasks away from frontier models. |
| [`220` Propaganda Podcast](220.md) | [Watch](https://x.com/OpenAgents/status/2041203318958027085) | Launches the OpenAgents Propaganda Podcast and frames open models, Bitcoin payments, and public development as an alternative AI-lab strategy. |
| [`221` Pylon Launch](221.md) | [Watch](https://x.com/OpenAgents/status/2041970265471480298) | Introduces Pylon as a Nostr-connected compute provider, shows network activity and payouts, and invites users to join the developing network. |
| [`222` Templar Merge](222.md) | [Watch](https://x.com/OpenAgents/status/2042696491358117908) | Discusses the Templar/Bittensor split and plans paid distributed training; existing node-presence payments and future training claims remain distinct. |
| [`223` Pay the People](223.md) | [Watch](https://x.com/OpenAgents/status/2043782380171767849) | Reports growing Pylon participation and payout bottlenecks, then argues for paying contributors and prepares the transition to useful training work. |
| [`224` Distributed Training 101](224.md) | [Watch](https://x.com/OpenAgents/status/2044890647342027072) | Explains training, checkpoints, validation, and DiLoCo while outlining CS336-style distributed homework jobs and the shift from presence payments to completed work. |
| [`225` Developer Bounties](225.md) | [Watch](https://x.com/OpenAgents/status/2045235776716411062) | Announces developer bounties for the product and ML stack, with coordinated contributor onboarding and defined work rather than unsolicited pull requests. |
| [`226` Worse is Better](226.md) | [Watch](https://x.com/OpenAgents/status/2046426222758973467) | Applies the worse-is-better philosophy to Nostr and OpenAgents, while naming sustainable buyer demand as the unresolved test of the compute-market thesis. |
| [`227` Ocean Power](227.md) | [Watch](https://x.com/OpenAgents/status/2056844888780533785) | Explores ocean thermal power and compute, reports payment-stack changes, and proposes legal-domain fine-tuning and business products to fund the network. |

### 228–240: Commercial products, accepted outcomes, Tassadar, and the Verse

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`228` Free Autopilot](228.md) | [Watch](https://x.com/OpenAgents/status/2062626257443909886) | Launches a free public-repository coding beta and proposes rewarding useful code and traces, with public trajectories supporting inspection and future training. |
| [`229` Autopilot Sites](229.md) | [Watch](https://x.com/OpenAgents/status/2062954960443126128) | Demonstrates Autopilot Sites and an iterative website-revision workflow; proposes persistent referral revenue from people recruited through those sites. |
| [`230` Calling All Agents](230.md) | [Watch](https://x.com/OpenAgents/status/2063140243599949937) | Addresses agents directly with the project history, Bitcoin revenue sharing, open-protocol markets, and the proposed group-forming network effect. |
| [`231` The Forum](231.md) | [Watch](https://x.com/OpenAgents/status/2063324716341829970) | Introduces the OpenAgents Forum as a shared discussion surface for humans and agents, with agent participation and Bitcoin tipping. |
| [`232` The Energy Layer](232.md) | [Watch](https://x.com/OpenAgents/status/2064074384881463459) | Connects flexible agent workloads to energy and compute orchestration, proposing accepted outcomes per kilowatt-hour as the efficiency metric. |
| [`233` The Monorepo](233.md) | [Watch](https://x.com/OpenAgents/status/2064366730579759597) | Announces consolidation of the product repositories into a Bun/Effect monorepo, with the Rust Psionic library remaining separate at that time. |
| [`234` Product Promises](234.md) | [Watch](https://x.com/OpenAgents/status/2064390975267480060) | Acknowledges gaps between prior promises and production, then introduces a machine-readable registry distinguishing live, gated, and withdrawn claims. |
| [`235` Agents Earn Bitcoin Tips](235.md) | [Watch](https://x.com/OpenAgents/status/2064786647481548849) | Demonstrates agent Bitcoin tipping and a reported Cash App withdrawal; introduces the MoneyDevKit wallet stack, treasury, and bounded Artanis spending. |
| [`236` Tassadar](236.md) | [Watch](https://x.com/OpenAgents/status/2065196586817216622) | Previews Pylon v0.3 and the experimental Tassadar executor-model run, with Bitcoin payments and a planned public launch. |
| [`237` You Must Construct Additional Pylons](237.md) | [Watch](https://x.com/OpenAgents/status/2066601306668810615) | Announces Autopilot 1.0 and Tassadar, defines accepted outcomes and verification as the economic foundation, and proposes a paid learning-and-reuse network. |
| [`238` The Training Run Begins](238.md) | [Watch](https://x.com/OpenAgents/status/2067700091750879691) | Reports a live worker–validator Bitcoin payment loop and explains Tassadar learning by construction; broader model and cost improvements remain the proposed flywheel. |
| [`239` Let's Make Money](239.md) | [Watch](https://x.com/OpenAgents/status/2068102703092543974) | Turns from compute supply to buyer demand, outlining business products, persistent referrals, and agent sales; the full revenue loop is still being built. |
| [`240` The Verse](240.md) | [Watch](https://x.com/OpenAgents/status/2068792528481173980) | Demonstrates a walkable 3D Tassadar run board with network and payment displays; multiplayer has been added but is explicitly untested. |

### 241–247: Khala, coding capacity, dogfooding, and selling

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`241` Reviewing Sakana Fugu](241.md) | [Watch](https://x.com/OpenAgents/status/2069125963397415007) | Reviews Sakana Fugu and introduces Khala as an open orchestration alternative combining models, tools, validators, and paid contributors. |
| [`242` Khala, Collective Intelligence](242.md) | [Watch](https://x.com/OpenAgents/status/2069922012428914696) | Introduces Khala through an OpenAI-compatible API and outlines independently optimizable programs, market-selected contributions, and reusable traces; implementation remains partial. |
| [`243` Khala in OpenCode](243.md) | [Watch](https://x.com/OpenAgents/status/2070013895683506637) | Integrates Khala with OpenCode, fixes request and tool-call compatibility, and diagnoses double-counted usage and failed deployments during live traffic. |
| [`244` Codex in Khala](244.md) | [Watch](https://x.com/OpenAgents/status/2070602618011857206) | Develops routing through the caller’s own Pylon-linked Codex or Claude capacity, then shows the Khala CLI and reported usage growth. |
| [`245` Khala Code](245.md) | [Watch](https://x.com/OpenAgents/status/2072540408324722771) | Introduces Khala Code and proposes a free data-sharing tier versus a paid private tier, with contingent revenue sharing from reusable trace-derived plugins. |
| [`246` Dogfooding Khala Code](246.md) | [Watch](https://x.com/OpenAgents/status/2072943871005282406) | Dogfoods Khala Code, turns recurring complaints into UX Behavior Contracts, and designs QA Swarm to test real product behavior and retain evidence. |
| [`247` Sell in Public](247.md) | [Watch](https://x.com/OpenAgents/status/2075465650256875937) | Shifts from building to selling: lead generation, partner fulfillment, referrals, and a paid coding-agent pool are proposed as the buyer-side revenue loop. |

### 248–259: Desktop, Assurance, FastFollow, and reliable workrooms

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`248` Predictable Software](248.md) | [Watch](https://x.com/OpenAgents/status/2075680613731078265) | Repairs local Codex history in OpenAgents Desktop and defines stable names, timestamps, and first-paint behavior as executable user-interface contracts. |
| [`249` Subagent UI Design](249.md) | [Watch](https://x.com/OpenAgents/status/2075869047560835291) | Improves sub-agent navigation through named workers, causal cards, and inspectable child histories, while identifying streaming as unfinished work. |
| [`250` Ready the Fleet](250.md) | [Watch](https://x.com/OpenAgents/status/2076259884685779366) | Builds and debugs the desktop fleet; invented commands and mislabeled models motivate typed capabilities, verified identity, complete histories, and evidence-backed controls. |
| [`251` Desktop MVP Spec](251.md) | [Watch](https://x.com/OpenAgents/status/2076688325474128257) | Defines a Codex-first, local desktop MVP whose ProductSpec binds intent, accepted plans, criterion-addressed work, and evidence-backed completion. |
| [`252` Observer, Automated QA](252.md) | [Watch](https://x.com/OpenAgents/status/2076953270052852166) | Designs AssuranceSpec, Observer, and QA Swarm around independent verification obligations and exact receipts, explicitly separating proof design from execution. |
| [`253` Goodbye Bun](253.md) | [Watch](https://x.com/OpenAgents/status/2077110044626182282) | Reports the Bun-to-Node, pnpm, and Vite Plus cutover, framing runtime ownership and replaceability as product dependencies. |
| [`254` Bug Bash](254.md) | [Watch](https://x.com/OpenAgents/status/2077944272767033519) | Dogfoods the near-alpha desktop through its first on-camera self-hosted commit and exposes queue replay, composer, attachment, and restart defects. |
| [`255` Fast Follow](255.md) | [Watch](https://x.com/OpenAgents/status/2077980463180714307) | Introduces FastFollow as a standing research and gap-analysis contract, then tests its relationship to Full Auto, ProductSpec, and implementation authority. |
| [`256` Release Candidate](256.md) | [Watch](https://x.com/OpenAgents/status/2078193847209742468) | Announces a macOS/Linux desktop release candidate and requests bug reports, while describing useful same-conversation provider switching on the host’s Mac. |
| [`257` Cursor Fails to Open a File](257.md) | [Watch](https://x.com/OpenAgents/status/2078687966210204011) | Demonstrates failed file opening in Cursor and a basic working OpenAgents editor toggle, motivating the broader agent-IDE direction. |
| [`258` ChatGPT Codex Keeps Crashing](258.md) | [Watch](https://x.com/OpenAgents/status/2078921380305768953) | Reviews a fatal Git-worker memory incident and specifies bounded work, process isolation, durable state, and regression evidence as prevention requirements. |
| [`259` Verifiable Software](259.md) | [Watch](https://x.com/OpenAgents/status/2079311647068283131) | Connects verifiable software to accepted outcomes per kilowatt-hour: scoped work, independent review, receipts, and proposed network verification and payments. |

### 260–274: Sarah, Omega, security, Immortal, and Forge

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`260` Sarah](260.md) | [Watch](https://x.com/OpenAgents/status/2080041664832311499) | Introduces Sarah as a voice-accessible face for internal work, sales, and service, with the founder’s business as the first proposed customer. |
| [`261` Sarah Meets World](261.md) | [Watch](https://x.com/OpenAgents/status/2080417099566989815) | Sarah’s introduction and human-empowerment mission. The retained file is labeled a prepared script; the owner’s catalog supplies its published-video title and link. |
| [`262` Project Omega](262.md) | [Watch](https://x.com/OpenAgents/status/2080685653671518479) | Final Sarah script announcing the Zed-based Omega direction, with verification, markets, and multiplayer planned; it does not claim those capabilities are released. |
| [`263` Bitcoin Wallets Under Attack](263.md) | [Watch](https://x.com/OpenAgents/status/2083291233112207718) | Responds to reports of a Coldcard wallet incident and proposes coordinated Bitcoin software defense using agentic analysis, responsible disclosure, and Nostr channels. |
| [`264` Running Loupe](264.md) | [Watch](https://x.com/OpenAgents/status/2083466735944970290) | Reports a Loupe experiment that finds the historical defect only with dependencies present, corrects the initial diagnosis, and submits an incomplete-checkout warning upstream. |
| [`265` Vulnerability Scanner](265.md) | [Watch](https://x.com/OpenAgents/status/2084193142970949903) | Dogfoods an Omega forensics workbench and exposes missing dependencies and target ambiguity; defines source-completeness, revision, and evidence improvements before further benchmarking. |
| [`266` Single Points of Failure and Nostr Markets](266.md) | [Watch](https://x.com/OpenAgents/status/2084553883779559539) | Uses the reported Boltz shutdown to motivate Nostr negotiated markets: replaceable coordination, private quotes and orders, and settlement verified outside the relay. |
| [`267` Immortal Infrastructure](267.md) | [Watch](https://x.com/OpenAgents/status/2084903707162894519) | Introduces separate relay, provider, and client roles in Immortal; reports a no-spend rehearsal with one live relay and plans funded testing. |
| [`268` Declaration](268.md) | [Watch](https://x.com/OpenAgents/status/2088143973676142949) | Posted Sarah broadcast criticizing Claude and framing the project’s opposition to concentrated AI power; this is a scripted message, not a technical demonstration. |
| [`269` Last Mover Advantage](269.md) | [Watch](https://x.com/OpenAgents/status/2088483918873825346) | Argues that reusable paid plugins and contributor network effects can outlast model and UI advantages, with Sarah as a closed product on open infrastructure. |
| [`270` Deploying Sarah](270.md) | [Watch](https://x.com/OpenAgents/status/2089445892193415235) | Deploys the Phoenix/Elixir Sarah interface, tests voice, memory, and GitHub tools, and plans local-agent delegation and a Forge. |
| [`271` Sarah on the BEAM](271.md) | [Watch](https://x.com/OpenAgents/status/2089967302510878802) | Responds to lost delegations with a BEAM clustering and handoff plan, then reports a 13-second Forge hot-load; comprehensive resilience testing remains next. |
| [`272` Sarah Leaves GitHub](272.md) | [Watch](https://x.com/OpenAgents/status/2090129708520235096) | Makes the owned Forge authoritative, troubleshoots mirroring and node failures, and publishes an initial changelog with human summaries and evidence links. |
| [`273` Open Source](273.md) | [Watch](https://x.com/OpenAgents/status/2090295429640396967) | Starts an AGPL Agent Forge distinct from the private Sarah repository, prioritizing issues, projects, public contribution, and rapid deployment. |
| [`274` The First Repo](274.md) | [Watch](https://x.com/OpenAgentsInc/status/2090892774669275186) | Demonstrates a first public GitHub-repository import into the Forge and flips the project mirror direction; private-repository behavior is explicitly unverified. |

### 275–284: Coder across terminal, cloud, mobile, and CoderOS

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`275` Coder](275.md) | [Watch](https://x.com/OpenAgentsInc/status/2092836555756823023) | Introduces Coder’s terminal, model choices, tools, and Forge integration; proposes one suite spanning computers, cloud/mobile access, plugins, traces, training, and payments. |
| [`276` Coder Cloud](276.md) | [Watch](https://x.com/OpenAgentsInc/status/2094539323374698817) | Announces a private-beta cloud coding service priced at one dollar an hour in the episode, with terminal and mobile synchronization proposed. |
| [`277` Coder Terminal](277.md) | [Watch](https://x.com/OpenAgentsInc/status/2095596118080151936) | Demonstrates a deliberately small, fast terminal with visible shell commands and subsidized model access; places it within a future cloud, web, and mobile suite. |
| [`278` Coder Commands Codex & Claude](278.md) | [Watch](https://x.com/OpenAgentsInc/status/2096058017133261230) | Demonstrates bounded concurrent delegation to Codex and Claude, then argues that a provider-neutral agent should coordinate competing model families. |
| [`279` Raiding Claude Code](279.md) | [Watch](https://x.com/OpenAgentsInc/status/2096453282734526797) | Studies Claude Code’s architecture and converts findings into a proposed Rust roadmap; the model’s consent statement is explicitly not legal or vendor authorization. |
| [`280` CoderOS](280.md) | [Watch](https://x.com/OpenAgentsInc/status/2097173500079346163) | Demonstrates Coder on Linux and cancellation of a Loom subscription, then proposes an operating-system product and bundled coding-tool subscription. |
| [`281` Coder Mobile](281.md) | [Watch](https://x.com/OpenAgentsInc/status/2097558311184842942) | Works through terminal, Android, and iOS chat synchronization, emulator setup, delegation, and trusted-device flows; cross-device capability is still being built and debugged. |
| [`282` Not Asking Permission](282.md) | [Watch](https://x.com/OpenAgentsInc/status/2099542186710671752) | A short address rejects permission-based limits on AI development and argues for distributing AI’s benefits broadly. |
| [`283` Coder for Gamers?](283.md) | [Watch](https://x.com/OpenAgentsInc/status/2099887688950161642) | Runs World of Warcraft in CoderOS, considers agent control, and identifies gamers who want to automate business work as a potential audience. |
| [`284` Gamifying Coder](284.md) | [Watch](https://x.com/OpenAgentsInc/status/2099943679444357241) | Designs XP, rewards, operator classes, leaderboards, and a productive Verse; argues that progression should reward verified work rather than token use or failed loops. |

### 285–288: Bendcoder, System One, Coder One, and Gym

| Episode and transcript | Video | Summary |
| --- | --- | --- |
| [`285` Bendcoder](285.md) | [Watch](https://x.com/OpenAgentsInc/status/2101190444344328280) | Prototypes Bendcoder with Jev classification, targeted generation, Bend, and C; debugs context, tool, and verification failures while exploring self-improvement and future Coder integration. |
| [`286` System One in Coding Agents](286.md) | [Watch](https://x.com/OpenAgentsInc/status/2102110125083492458) | Examines cache economics, dynamic context, conditional instructions, progressive tools, explicit state, background work, and open protocols for reusable agent components. |
| [`287` Building a System One Coding Agent](287.md) | [Watch](https://x.com/OpenAgentsInc/status/2102773483109335209) | Builds Coder One and measures Jev probes, briefings, delegates, and tunable components on Terminal-Bench; distinguishes observed trials from a hypothetical best-choice router. |
| [`288` Coder Gym](288.md) | In progress | Incomplete Gym session: trace inspection, ranked findings, Microluna, failed acceptance loops, corrected performance claims, design-time fitting, reusable pattern components, independent checks, and the first Fire Loop experiment. |

## Archive corrections and scope

Episode numbers have sometimes been reassigned before or during publication.
Use the current numbered file for the index and Git history to explain an
earlier interpretation. For example, commit
[`89883754ae`](https://github.com/OpenAgentsInc/openagents/commit/89883754ae)
identifies 245 as **Khala Code** and moves unreleased delegation material out
of that slot. Commit
[`46704bfdeb`](https://github.com/OpenAgentsInc/openagents/commit/46704bfdeb)
replaces slot 263 with **Bitcoin Wallets Under Attack**. Listing the displaced
Omega Alpha script as the current episode would misrepresent the archive.

Earlier versions of this guide also linked `26X-omega-agent.md`,
`26X-forkingzed.md`, `26X-fullauto.md`, and
`26X-forkingzed-production-requests.md`. Those files are absent from the current
committed tree. So is the old Sarah pipeline document this README used to
link. Do not treat these references as available or scheduled videos; recover
their provenance from Git if the historical drafts are needed.

The 2026-09-26 cleanup removes the remaining notes, preparation documents,
production runbooks, and external commentary from this directory. The archive
contains only the numbered episode records and this guide. Historical source
citations elsewhere can link to a specific Git revision of a removed document;
they do not identify an additional episode.

## From historical ideas to the current repository

Use the archive to understand motivation and design evolution. Use maintained
documentation and actual code/results to determine present behavior. Old paths,
prices, model names, deployment announcements, and product counts are historical
context, not current installation or purchasing guidance.

| Historical thread | Maintained starting point |
| --- | --- |
| Coder architecture and its several product surfaces | [Coder documentation](../coder/README.md) and [TypeSafe product-suite analysis](../coder/design/typesafe-product-suite.md). |
| Network effects, reusable components, and evidence of generalization | [Networked Coder plan](../coder/design/networked-coder-plan.md) and [knowledge-base design](../coder/design/knowledge-base.md). |
| Traces, experiment inspection, and benchmark claims | [Trace contract](../coder/runtime/traces.md), [Gym overview](../gym.md), and [Terminal-Bench results](../terminal-bench/README.md). |
| Nostr interoperability and the cross-client/market boundary | [NIP index](../../nips/README.md) and [OpenAgents coverage review](../protocol/2026-09-26-openagents-gap-review.md). New specifications are not evidence of implemented runtime support. |
| Agent labor and the wider economic vision | [Market infrastructure plan](../agents/market-infrastructure.md) and [general-agent roadmap](../agents/roadmap.md). |
| What remains active after the repository changes | [Repository README](../../README.md), [roadmap](../roadmap.md), and [glossary](../glossary.md). |

## Maintaining the archive

- Preserve source transcripts and their header metadata. An index correction
  should not silently rewrite the historical speaker's claim.
- Keep one index row per committed `NNN.md` file, with three-digit links.
  Use the owner-approved episode title in both the index and the file heading;
  retain media filenames, transcription metadata, and spoken wording unchanged.
  Recount the archive and note gaps when files are added or removed.
- Record whether a numbered file is a transcript, prepared script, final
  script, or incomplete recording. Do not infer publication from a numbered
  filename or a transcription date. Keep supporting notes and production
  material outside this archive.
- If a slot changes, update its title, summary, reading paths, and theme
  references together. Retain an explanation of materially important corrections.
- Update history and themes when a new episode changes the argument; do not
  only append a row. Keep both unsuccessful experiments and their later lessons.
- Link performance claims to the measured configuration, task set, costs,
  timings, and retained results. Do not turn a development result or an
  after-the-fact portfolio into a confirmed general policy.
- Verify exact quotations and ambiguous names/numbers against the media.
  Prefer short attributed summaries when the machine transcript is corrupted.
- Keep episode 086's excerpt boundary and episode 288's incomplete status until
  the source record actually changes. Do not restore removed clip 288g as a
  current success claim.
- Check links after updates, including references to documents removed by
  repository resets.
