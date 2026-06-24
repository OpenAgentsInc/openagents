# Report: Sakana AI’s work on “collective intelligence”

A terminology note: the company is officially **Sakana AI**, not usually “Sakana Labs.” It is a Tokyo-based frontier AI R&D company founded in 2023 by David Ha, Ren Ito, and Llion Jones. The “collective intelligence” theme is not a side metaphor for Sakana; it is part of the company’s identity. Its logo and name evoke a school of fish, and the company describes its research as inspired by natural collectives such as swarms, evolution, and self-organizing systems. ([Sakana AI][1])

## 1. Executive thesis

Sakana AI’s collective-intelligence program is best understood as a bet against the idea that AI progress must come mainly from training ever-larger monolithic models. Its recurring pattern is:

**Keep many diverse models, agents, programs, or hypotheses alive; search over how they should compete, merge, cooperate, specialize, and self-improve; then turn the resulting collective behavior into a stronger system.**

That shows up in three major layers:

| Layer                            | Core idea                                                                                                                                | Representative Sakana systems                                             |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| **Model creation**               | Combine existing open models or populations of models into new specialists.                                                              | Evolutionary Model Merge, CycleQD, M2N2                                   |
| **Inference-time orchestration** | Dynamically coordinate multiple models/agents at test time.                                                                              | AB-MCTS, TRINITY, Conductor, Fugu                                         |
| **Open-ended discovery**         | Use collectives of agents, code variants, reviewers, or self-improving systems to generate science, algorithms, and new AI capabilities. | AI Scientist, Darwin Gödel Machine, ShinkaEvolve, ASAL, Digital Red Queen |

Sakana’s own framing traces back to David Ha and Yujin Tang’s survey on collective intelligence for deep learning, which defined the field as studying group intelligence emerging from interactions among many individuals and argued that deep learning can benefit from those ideas. ([arXiv][2])

## 2. What “collective intelligence” means at Sakana

For Sakana, “collective intelligence” does **not** mainly mean human crowdsourcing. It means machine intelligence arising from **interactions among many partially capable units**: models, agents, policies, candidate programs, training objectives, reviewers, scientific hypotheses, or code variants.

The company’s corporate description explicitly groups its work around nature-inspired collective systems, naming examples such as **AI Scientist**, **Darwin Gödel Machine**, **ShinkaEvolve**, and **AB-MCTS**. ([Sakana AI][1]) Its earlier public research agenda also said Sakana’s focus was applying ideas like evolution and collective intelligence to create new foundation models, especially by building machinery that can automatically generate useful models rather than hand-training one model at a time. ([Sakana AI][3])

A useful distinction is:

**Collective intelligence as creation:** many models are raw material for a better merged model.
**Collective intelligence as coordination:** many models remain separate but are routed, prompted, checked, or combined at inference time.
**Collective intelligence as open-ended evolution:** many agents or artifacts produce new discoveries, and the system selects and builds on the best ones.

Sakana has projects in all three categories.

---

# 3. Model creation: collective intelligence inside the weights

## 3.1 Evolutionary Model Merge

Sakana’s first widely noticed collective-intelligence result was **Evolutionary Model Merge**. The idea was to treat the enormous ecosystem of open models as a “collective intelligence” resource and use evolutionary algorithms to discover useful ways to combine them. Sakana described the method as searching over ways to merge models, rather than training a new foundation model from scratch. ([Sakana AI][3])

Technically, the paper searched both **parameter space** and **data-flow space**. In parameter-space merging, model weights are combined using merge recipes whose coefficients are optimized by evolutionary methods. In data-flow-space merging, the system searches over how different model components should be connected during inference. The reported demonstrations included a Japanese math LLM and a Japanese vision-language model built from existing open models. ([arXiv][4])

The collective-intelligence angle is important: the “individuals” are not agents in a chatroom but pretrained models with different capabilities. Evolutionary search becomes the mechanism for extracting and recombining useful traits from that population.

**Why it mattered:** it suggested a path to create specialized foundation models using less compute than full pretraining, especially for languages or domains underserved by the largest commercial labs.

**Limitations:** Sakana’s own paper noted that merged models can inherit limitations from their source models, may lack logical coherence, and still require alignment or instruction-tuning work depending on use case. ([arXiv][4])

## 3.2 CycleQD: populations of niche models

**CycleQD** pushed the same idea toward a more explicit ecosystem model. Instead of producing only one merged winner, CycleQD uses **Quality Diversity** methods to maintain a population of models with different strengths. Sakana framed this using ecological niches: a population of specialized agents can collectively cover a broader capability space than a single model. ([Sakana AI][5])

The technical system cycles through tasks and uses model merging as crossover, plus mutation-like operations, to generate a population of LLM agents. The paper reported improvements on coding, operating-system, and database-style agentic benchmarks, while preserving general language ability; it also showed the method could extend beyond language to image segmentation. ([arXiv][6])

The key conceptual shift is from **“find the best model”** to **“maintain a diverse archive of useful specialists.”** That is very close to biological collective intelligence: the ecosystem’s power comes from diversity and complementarity, not uniform excellence.

## 3.3 M2N2: competition and attraction for model fusion

Sakana’s later **M2N2** work, published as “Competition and Attraction Improve Model Fusion,” introduced more population dynamics into model fusion. The method uses evolving merge boundaries, competition to preserve diversity, and attraction or mate-selection mechanisms to choose which models should combine. ([Sakana AI][7])

Sakana reported that M2N2 worked in small settings such as evolving an MNIST classifier from scratch, and in larger settings such as merging a math specialist with an agentic specialist and adapting text-to-image models for Japanese prompts while retaining English capability. ([Sakana AI][7])

This line of work is Sakana’s clearest “weights-level” collective intelligence: separate models are treated like organisms, merge recipes are like reproduction, evaluation is selection pressure, and diversity mechanisms prevent premature collapse into one narrow solution.

---

# 4. Inference-time orchestration: collective intelligence among live agents

Sakana’s more recent work moves from **merging models into one artifact** to **coordinating multiple models while solving a task**. This is arguably the company’s most commercially important direction, because it leads directly to Fugu.

## 4.1 AB-MCTS and Multi-LLM AB-MCTS

**AB-MCTS** applies Monte Carlo Tree Search-style trial-and-error to LLM reasoning. Instead of relying on one model’s first answer, the system searches over solution attempts, refinements, and alternatives. Sakana’s Multi-LLM version extends this by adaptively choosing which LLM should be used at different points in the search. ([Sakana AI][8])

On ARC-AGI-2 public tasks, Sakana reported that AB-MCTS improved over repeated sampling with a single model, and that Multi-LLM AB-MCTS combining models such as Gemini and DeepSeek could solve more tasks than either model alone in some settings. The central claim is not that any one model is best, but that different models have complementary “personalities” or capabilities that can be exploited by adaptive search. ([Sakana AI][8])

This is collective intelligence as **deliberative search**: multiple model calls become a problem-solving population, and the search algorithm decides which branches deserve more attention.

## 4.2 TRINITY: a learned coordinator for model teams

**TRINITY** is a lightweight coordinator that sits above multiple foundation models and learns how to assign them roles such as Thinker, Worker, and Verifier. Unlike model merging, it can work with closed-source API models because it does not require modifying their weights. Sakana describes TRINITY as a macro-level test-time model-composition system that fuses the strengths of different models through learned coordination. ([arXiv][9])

This matters because it generalizes the collective-intelligence idea beyond open weights. Instead of needing access to all parameters, Sakana can coordinate a set of black-box or API-based models. That makes the approach more practical in the current AI market, where many strongest models are closed.

## 4.3 Conductor

**Conductor** is Sakana’s next step: a language model trained to coordinate other language models. Sakana reported that a 7B Conductor model surpassed individual worker models on benchmarks such as LiveCodeBench and GPQA-Diamond, and that it can recursively select itself as a worker as part of test-time scaling. ([Sakana AI][10])

The technical idea is that the coordinator does not merely route a prompt to “the best model.” It can generate workflows: divide the problem into subtasks, assign roles, ask workers to critique or verify each other, and synthesize a final answer. In Sakana’s Fugu technical report, this is described as training a model to prompt-engineer and coordinate worker models through natural-language agent workflows. ([arXiv][11])

This is one of Sakana’s strongest collective-intelligence claims: the coordinator itself learns how to harness the collective, rather than relying only on a hand-coded orchestration pattern.

## 4.4 Fugu: productizing learned collective intelligence

As of **June 24, 2026**, Sakana’s most concrete productization of collective intelligence is **Fugu**, announced for general availability on June 22, 2026. Sakana describes Fugu as a single model endpoint and API that dynamically orchestrates a pool of strong models, deciding whether to solve directly or assemble expert agents, delegate subtasks, verify results, and synthesize the final output. ([Sakana AI][12])

The product pitch is that users should not need to manually choose among frontier models or build complicated multi-agent workflows. Fugu acts as an orchestrator: it selects models, manages communication, and tries to combine their strengths while routing around weaknesses. Sakana explicitly connects Fugu to TRINITY and Conductor. ([Sakana AI][12])

Sakana’s Fugu technical report frames this as a new scaling axis: instead of only scaling model size, scale by dynamically choosing models, communication patterns, tools, environments, and synthesis strategies. The report claims state-of-the-art or near-frontier benchmark results across coding, reasoning, science, and visual reasoning tasks, though those results should be treated as Sakana-reported benchmark claims until broadly independently replicated. ([arXiv][11])

Fugu is therefore Sakana’s clearest statement of strategy: **the future AI interface may be a learned collective orchestrator rather than a single foundation model.**

---

# 5. Open-ended discovery: collectives that generate new science, code, and algorithms

## 5.1 LLM² and DiscoPOP: evolving training objectives

Sakana’s **LLM²** work used language models to invent new preference-optimization objectives. The project ran evolutionary search over objective functions and produced methods such as **DiscoPOP**, which Sakana reported outperformed several human-designed preference-optimization baselines in its experiments. ([Sakana AI][13])

This is collective intelligence in a looser but important sense: many candidate ideas are generated, tested, selected, and recombined. The “population” consists of proposed algorithms rather than models. The system’s intelligence emerges from the loop of generation, evaluation, and selection.

## 5.2 AI Scientist and AI Scientist-v2

**AI Scientist** is Sakana’s most ambitious automated-discovery project. The original system attempted to automate the scientific loop: idea generation, experiment coding, execution, visualization, paper writing, and review. Sakana presented it as part of a broader vision of AI systems that can conduct research independently. ([Sakana AI][14])

**AI Scientist-v2** advanced the system with parallelized agentic tree search, literature search, experiment execution, paper writing, and visual feedback on figures. Sakana reported that one fully AI-generated paper passed peer review at an ICLR 2025 workshop, and later described the work in connection with a Nature publication. ([Sakana AI][15])

The collective-intelligence component is not just “one AI writes a paper.” It is a pipeline of specialized roles: idea generator, coder, experiment runner, figure checker, paper writer, reviewer, and meta-review-like selection. AI Scientist-v2 also used an Automated Reviewer that ensembles multiple independent reviews. ([Sakana AI][15])

The caveats are substantial. Sakana itself noted that AI Scientist-v2 still struggles with naïve ideas, rigor, complex code, hallucinations, citation errors, and duplicate figures. ([Sakana AI][15]) An independent evaluation found serious shortcomings in the original AI Scientist, including poor novelty assessment, experiment failures from coding errors, flawed or misleading results, citation problems, and hallucinated numbers, while still calling it a major step toward autonomous research systems. ([arXiv][16])

## 5.3 Darwin Gödel Machine

The **Darwin Gödel Machine** applies open-ended evolution to self-improving coding agents. Rather than demanding a formal self-proof as in a classical Gödel Machine, Sakana’s system uses foundation models to propose code changes, evaluates them, and stores successful variants in a growing archive. Future agents can branch from any archive member, allowing stepping-stone improvements rather than a single linear optimization path. ([Sakana AI][17])

Sakana reported large improvements on SWE-bench and Polyglot benchmarks, and emphasized that the archive of diverse agents was central to the result. The collective here is a lineage tree of agent variants, not a simultaneous chat swarm. ([Sakana AI][17])

This is one of Sakana’s most important safety-relevant projects. The company explicitly discussed sandboxing, human supervision, lineage tracking, reward hacking, and tool-use hallucinations as concerns. ([Sakana AI][17])

## 5.4 ShinkaEvolve

**ShinkaEvolve** is Sakana’s evolutionary code-optimization framework. It uses LLMs to propose program variants, evaluates them, and searches through the space of algorithms. Sakana reported results in several domains, including a new 26-circle packing solution, evolved math-agent scaffolds using expert personas and peer review, and an MoE load-balancing loss discovered through evolutionary search. ([Sakana AI][18])

This is another example of Sakana treating AI progress as a search over populations of artifacts. The system does not assume the first LLM-generated program is good; it builds an evolutionary process around many candidates.

## 5.5 ASAL: automating the search for artificial life

**ASAL**, or Automating the Search for Artificial Life, uses vision-language foundation models to explore artificial-life systems such as Boids, Particle Life, Game of Life, Lenia, and neural cellular automata. The goal is to find systems with target behaviors, open-ended novelty, or diverse emergent dynamics. ([arXiv][19])

ASAL matters for Sakana’s collective-intelligence story because artificial life is one of the intellectual sources of the company’s research taste. It studies how simple local rules can produce complex group behavior, then uses modern foundation models to search that space.

## 5.6 Digital Red Queen

The **Digital Red Queen** project explores self-play and open-ended arms races in a Core War environment. LLM-generated programs compete against previous champions, producing a sequence of adapted agents. Sakana frames this around Red Queen dynamics: agents improve because their opponents keep improving. ([Sakana][20])

This is collective intelligence through adversarial coevolution. Instead of a fixed benchmark, the environment itself becomes a moving target shaped by the population’s history.

---

# 6. Product direction: from research motif to platform strategy

Fugu is the clearest collective-intelligence product, but Sakana’s broader commercial direction also includes long-horizon agentic research systems such as **Marlin**. Sakana describes Marlin as an autonomous research assistant that can conduct multi-hour strategy research and produce reports or slides; its release materials connect it to AI Scientist, AB-MCTS, ALE-Agent, and long-inference model-control techniques. ([Sakana AI][21])

The product pattern is consistent:

1. **Do not expose the user to the whole swarm.**
2. Put a coordinator in front.
3. Let the coordinator choose tools, models, agents, search branches, and verification steps.
4. Return a single coherent answer, report, or API response.

That is a pragmatic version of collective intelligence: users buy the outcome, not the complexity.

---

# 7. The technical pattern across Sakana’s work

Across these projects, Sakana keeps returning to five design principles.

**First, diversity is an asset.** Many AI systems try to collapse to the single best model or policy. Sakana repeatedly builds archives, populations, specialist pools, or model teams.

**Second, selection pressure matters.** Evolutionary algorithms, Quality Diversity, Monte Carlo Tree Search, reinforcement learning, and automated review all serve as mechanisms for deciding which members of the collective deserve more influence.

**Third, recombination is central.** In model merging, recombination happens in weights or data flow. In Fugu-like systems, recombination happens through task decomposition, delegation, critique, and synthesis. In AI Scientist and ShinkaEvolve, recombination happens among ideas, code variants, and experimental results.

**Fourth, the collective can be latent or explicit.** Sometimes the final artifact is one merged model. Sometimes it is a live multi-agent workflow. Sometimes it is an archive of evolutionary lineages.

**Fifth, the long-term target is self-improvement.** Sakana’s work increasingly asks whether AI systems can generate better models, better objectives, better code, better scientific papers, or better coordinators with less human intervention.

---

# 8. Strategic significance

Sakana’s collective-intelligence work is significant for three reasons.

**It offers an alternative scaling path.** Instead of assuming that the next leap always requires larger pretraining runs, Sakana is exploring whether capability can come from composition, orchestration, and evolutionary search over existing models.

**It is especially relevant for countries, companies, and domains without hyperscaler-level compute.** Model merging, specialist coordination, and orchestration can be attractive when training a frontier model from scratch is too expensive.

**It fits the current fragmented model ecosystem.** In a world with many strong models from different providers, a system that can dynamically choose among them may be more useful than a single static model. Fugu is Sakana’s attempt to turn that into an API-level product. ([Sakana AI][12])

---

# 9. Main caveats and open questions

The biggest caveat is **evaluation**. Many of Sakana’s strongest claims are based on Sakana-reported benchmarks or preprints. That does not make them wrong, but it means independent replication matters, especially for Fugu, Conductor, AI Scientist-v2, and self-improving agents. ([arXiv][11])

A second caveat is **cost and latency**. Collective systems often make many model calls, run searches, or coordinate multiple agents. That can improve quality, but it may be slower or more expensive than a single model call. Sakana partly addresses this with separate Fugu variants optimized for speed versus maximum performance. ([Sakana AI][12])

A third caveat is **error amplification**. If weak agents produce flawed intermediate work, the collective can converge on a polished but wrong answer. The AI Scientist evaluations are a concrete warning: automated systems can produce plausible papers with citation errors, failed experiments, or misleading results. ([arXiv][16])

A fourth caveat is **governance and safety**. Self-improving code agents and automated scientific systems raise concerns about reward hacking, unsafe tool use, misleading outputs, and unclear accountability. Sakana’s DGM work explicitly discusses sandboxing, supervision, and lineage tracking, but these remain active challenges rather than solved problems. ([Sakana AI][17])

A fifth caveat is **dependency management**. Inference-time orchestration can depend on external closed models, API availability, pricing, compliance constraints, and model-provider terms. Fugu’s product design acknowledges this by allowing users to opt out of particular agents or providers, but the broader dependency issue remains. ([Sakana AI][12])

---

# 10. Bottom line

Sakana AI’s work on collective intelligence is not a single project; it is the organizing principle behind much of the company’s research. The company has explored collective intelligence at the level of **weights** through evolutionary model merging, at the level of **agents** through AB-MCTS, TRINITY, Conductor, and Fugu, and at the level of **open-ended discovery** through AI Scientist, Darwin Gödel Machine, ShinkaEvolve, ASAL, and Digital Red Queen.

The most important shift is from asking, **“How do we train one bigger model?”** to asking, **“How do we build an evolving ecosystem of models, agents, tools, and evaluators that becomes smarter as a collective?”**

That is Sakana’s core bet: future AI progress may come not only from larger individual models, but from learned systems that know how to combine many imperfect intelligences into a stronger whole.

[1]: https://sakana.ai/company-info/ "Corporate Info — Sakana AI"
[2]: https://arxiv.org/abs/2111.14377 "[2111.14377] Collective Intelligence for Deep Learning: A Survey of Recent Developments"
[3]: https://sakana.ai/evolutionary-model-merge/ "Evolving New Foundation Models: Unleashing the Power of Automating Model Development"
[4]: https://arxiv.org/html/2403.13187v1 "Evolutionary Optimization of Model Merging Recipes"
[5]: https://sakana.ai/cycleqd/ "Population-based Model Merging via Quality Diversity"
[6]: https://arxiv.org/html/2410.14735v1 "Agent Skill Acquisition for Large Language Models via CycleQD"
[7]: https://sakana.ai/m2n2/ "Competition and Attraction Improve Model Fusion"
[8]: https://sakana.ai/ab-mcts/ "Inference-Time Scaling and Collective Intelligence for Frontier AI"
[9]: https://arxiv.org/abs/2512.04695 "TRINITY: An Evolved LLM Coordinator"
[10]: https://sakana.ai/learning-to-orchestrate/ "Learning to Orchestrate Agents in Natural Language with the Conductor"
[11]: https://arxiv.org/html/2606.21228v1 "Sakana Fugu Technical Report"
[12]: https://sakana.ai/fugu-release/ "Sakana Fugu: One Model to Command Them All"
[13]: https://sakana.ai/llm-squared/ "Can LLMs invent better ways to train LLMs?"
[14]: https://sakana.ai/ai-scientist/ "The AI Scientist: Towards Fully Automated Open-Ended Scientific Discovery"
[15]: https://sakana.ai/ai-scientist-nature/ "The AI Scientist: Towards Fully Automated AI Research, Now Published in <i>Nature</i>"
[16]: https://arxiv.org/abs/2502.14297 "[2502.14297] Evaluating Sakana's AI Scientist: Bold Claims, Mixed Results, and a Promising Future?"
[17]: https://sakana.ai/dgm/ "The Darwin Gödel Machine: AI that improves itself by rewriting its own code"
[18]: https://sakana.ai/shinka-evolve/ "ShinkaEvolve: Evolving New Algorithms with LLMs, Orders of Magnitude More Efficiently"
[19]: https://arxiv.org/html/2412.17799v2?utm_source=chatgpt.com "Automating the Search for Artificial Life with Foundation ..."
[20]: https://pub.sakana.ai/drq/ "pub.sakana.ai"
[21]: https://sakana.ai/marlin-release/?utm_source=chatgpt.com "初の商用プロダクト「Sakana Marlin」を提供開始"


