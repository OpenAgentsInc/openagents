> **External draft.** This note was drafted outside the repository by an AI research assistant, given the task-index proposal, and is kept here as received apart from cleanup: tracking parameters stripped from links, paste leftovers removed, and second-person text rewritten neutrally. Its claims about each paper have not all been checked against the papers; arXiv IDs were checked to resolve (see [papers.md](papers.md)).

There is a useful body of research around this. The proposal is best framed as **verified procedural memory for repository-level coding agents**, rather than simply “better code search”: a strong model turns repository understanding into reusable task recipes, and a cheaper model uses those recipes to make complete changes. That is the proposal's central hypothesis, and it distinguishes the idea from file summaries and raw navigation histories.

Below are **25 arXiv papers across six research avenues**. The first six to read are **Memp, Code2Skill, CodePlan, SkillsBench, SWE-Skills-Bench, and SkillGenBench**. Those most directly address whether the recipes can work, how to construct them, and how to test them.

The “avenue” beneath each paper is a suggested application to the task index—not a claim that the paper already validates the proposed system.

## 1. Turning past investigations into reusable procedures

### 1. Memp: Exploring Agent Procedural Memory
**2025 · arXiv:2508.06433 · Priority read**

Studies how to build, retrieve, and update procedural memory from agent trajectories. Particularly relevant: it explicitly tests **transferring procedural memory constructed by a stronger model, GPT-4o, to a weaker model, Qwen2.5-14B**, rather than merely letting an agent reuse its own experience. Its experiments include ALFWorld and TravelPlanner, not repository editing. [arXiv](https://arxiv.org/html/2508.06433v4)

**Avenue:** This is a close experimental analogue to the strong-builder/cheap-executor hypothesis. Compare raw successful trajectories, distilled instructions, and higher-level procedures while holding the cheaper model fixed. Read its memory-transfer and retrieval-volume experiments first.

### 2. Agent Workflow Memory
**2024 · arXiv:2409.07429**

Introduces a method for extracting reusable workflows from previous experiences and supplying relevant workflows during subsequent tasks. It considers both offline construction and online accumulation, with experiments in web navigation. [arXiv](https://arxiv.org/abs/2409.07429)

**Avenue:** Use this for the **investigation episode → recurring task → recipe** pipeline. The interesting question is how much instance-specific detail to remove: a workflow must generalize beyond the original task without losing the repository-specific wiring that makes it useful.

### 3. ReasoningBank: Scaling Agent Self-Evolving with Reasoning Memory
**2025 · arXiv:2509.25140**

Distills reusable reasoning strategies from both successful and failed experiences. It also studies memory-aware test-time scaling: additional exploration produces experience that can improve the memory bank. A significant qualification is that its memory construction uses **self-judged** successes and failures. [arXiv](https://arxiv.org/abs/2509.25140)

**Avenue:** Mine failed investigations for useful exclusions: “this registration path is for inference models, not 3D assets.” Borrow the contrast between successful and unsuccessful approaches, but retain the proposal's stronger requirement that promoted warnings be backed by source or behavioral evidence.

### 4. Agentic Context Engineering: Evolving Contexts for Self-Improving Language Models
**2025 · arXiv:2510.04618**

ACE treats context as an evolving playbook, using generation, reflection, and curation. It identifies **brevity bias**, where concise summaries discard useful details, and **context collapse**, where repeated rewriting erodes accumulated knowledge. Its proposed remedy is structured, incremental updating. [arXiv](https://arxiv.org/abs/2510.04618)

**Avenue:** This is especially relevant to the pilot's finding that shorter briefings omitted required sibling files. Explore updating individual recipe rules and companion-role records instead of repeatedly asking a model to rewrite the whole recipe more concisely. The pilot already makes completeness an important concern.

### 5. XRepoSkill: Learning Transferable Skills for Software Engineering Agents
**September 29, 2026 · arXiv:2609.36807**

Contrasts successful and failed trajectories on the same issue to derive conditional action rules. Rules receive executable predicates and are retained when their prescribed behaviors are associated with success and recur across repositories. Its evaluation repositories are excluded from the skill-learning trajectory pool. [arXiv](https://arxiv.org/abs/2609.36807)

**Avenue:** Explore **which behavior actually deserves to become a rule**. An action appearing in a successful trace is not necessarily what made the trace successful. This paper is also useful for separating transferable engineering procedures from repository-local knowledge; the index likely needs both, stored distinctly.

## 2. Grounding recipes in source—and preserving complete changes

The proposal’s `requires_companion` relationship is more demanding than ordinary file retrieval: it must represent that several roles are jointly required under particular conditions. That makes planning and structured knowledge representation relevant, not just embedding models.

### 6. Grounded Skill Synthesis from Code at Scale for Agentic Intelligence
**September 4, 2026 · arXiv:2609.05571 · Code2Skill · Priority read**

A particularly close source-to-recipe paper. Code2Skill extracts atomic operations, composite workflows, and recurring patterns from source, with procedural steps, boundaries, invariants, and provenance. It checks records through source-body-blind reconstruction followed by source-aware comparison. **That comparison is LLM-based consistency checking, not a substitute for execution-based verification.** [arXiv](https://arxiv.org/html/2609.05571v1)

**Avenue:** Study its record schema and construction pipeline. One important difference: it seeks transferable abstractions and filters project-local glue, whereas **project-local integration obligations are part of what the task index needs to preserve**. Borrow the grounding structure without automatically borrowing its selection criteria.

### 7. CodePlan: Repository-level Coding using LLMs and Planning
**2023 · arXiv:2309.12499 · Priority read**

Frames repository-wide coding as a planning problem. It combines incremental dependency analysis, change-impact analysis, and adaptive planning to construct interdependent edit sequences. Its evaluations include changes spanning multiple files, rather than only localized completion. [arXiv](https://arxiv.org/abs/2309.12499)

**Avenue:** This is the paper to read for **“what else must change?”** Explore representing a recipe as a conditional edit plan: adding a protocol field can create obligations for serialization, consumers, validation, and tests. Retrieval finds the starting points; planning keeps the change coherent.

### 8. LocAgent: Graph-Guided LLM Agents for Code Localization
**2025 · arXiv:2503.09089**

Represents repositories as directed heterogeneous graphs containing files, classes, functions, imports, invocations, and inheritance. Agents use multi-hop graph reasoning to connect natural-language issue descriptions to relevant code. It also studies a fine-tuned open model for localization. [arXiv](https://arxiv.org/abs/2503.09089)

**Avenue:** Use this as a comparison for the **task language → code entities** bridge. The key experiment is whether the capability map improves localization beyond a competent graph-guided agent—not merely beyond lexical search. Keep localization success separate from complete-edit success.

### 9. Know Before Fix: QA-Driven Repository Knowledge Acquisition for Software Issue Resolution
**July 2026 · arXiv:2607.11111 · ACQUIRE**

Separates repository knowledge acquisition from patch generation. A Questioner identifies knowledge gaps, an Answerer explores the repository to produce evidence-grounded answers, and a Resolver uses the resulting structured knowledge to generate patches. [arXiv](https://arxiv.org/abs/2607.11111)

**Avenue:** Let the builder generate explicit questions before drafting a recipe: “Where is identity assigned?”, “Which registry is authoritative?”, “Which consumers require updates?” Then investigate whether answered questions can be reused across a task family instead of being regenerated for every issue.

### 10. HyperGraphRAG: Retrieval-Augmented Generation via Hypergraph-Structured Knowledge Representation
**2025 · arXiv:2503.21322**

Uses hyperedges to represent relationships among more than two entities, addressing a limitation of binary-edge knowledge graphs. Its experiments concern retrieval-augmented question answering across several domains, not coordinated code edits. [arXiv](https://arxiv.org/abs/2503.21322)

**Avenue:** Explore representing a recipe as a grouped relationship among **definition + implementation + registration + consumers + tests**. But a hyperedge alone does not establish that every member is mandatory. The index would still need explicit applicability conditions, required-role semantics, and coverage checks.

## 3. Testing whether recipes help—and whether agents can build them

### 11. SkillsBench: Benchmarking How Well Agent Skills Work Across Diverse Tasks
**2026 · arXiv:2602.12670 · Priority read**

Evaluates agents with no skills, curated skills, and self-generated skills using deterministic verifiers. The authors report a **16.2-percentage-point average gain from curated skills**, but substantial variation by domain; the reported software-engineering gain is 4.5 points. Self-generated skills provide no average benefit in their setting, while some smaller models with skills match larger models without them. [arXiv](https://arxiv.org/abs/2602.12670)

**Avenue:** This closely supports the logic of the reviewed-recipe experiment: **consuming useful procedural knowledge and generating useful procedural knowledge are separate capabilities**. Demonstrate the first before investing heavily in automating the second.

### 12. SWE-Skills-Bench: Do Agent Skills Actually Help in Real-World Software Engineering?
**2026 · arXiv:2603.15401 · Priority read**

Pairs public software-engineering skills with repositories pinned to commits and requirements checked through execution-based tests. Its findings are much less optimistic: **39 of 49 skills yield no pass-rate improvement**, and some introduce substantial token overhead. It also identifies degradation from version-mismatched guidance. [arXiv](https://arxiv.org/abs/2603.15401)

**Avenue:** Treat this as a valuable skeptical reference. Ask what distinguishes the proposed recipes from generic skill documents: repository specificity, verified companion coverage, current-version applicability, or better selection. Test those distinctions rather than assuming “skills” as a category will help.

### 13. SkillGenBench: Benchmarking Skill Generation Pipelines for LLM Agents
**May 2026 · arXiv:2605.18693 · Priority read**

Evaluates the **skill generator itself**. It distinguishes task-conditioned generation, where the task is already known, from task-agnostic generation, where a reusable skill library must be constructed before downstream tasks are revealed. It covers both repository-grounded and document-grounded procedures with pinned environments and execution-based checks. [arXiv](https://arxiv.org/abs/2605.18693)

**Avenue:** This is directly relevant to evaluating the offline builder. Building a recipe after seeing the exact evaluation task can conceal a weak index-construction method. Use its task-agnostic framing for the chronological, task-family-held-out evaluation.

### 14. Evaluating AGENTS.md: Are Repository-Level Context Files Helpful for Coding Agents?
**2026 · arXiv:2602.11988**

Studies repository-level context files across coding agents and models. In its evaluated settings, context files tend to reduce task success while increasing inference cost; unnecessary instructions can induce broader exploration and make tasks harder. [arXiv](https://arxiv.org/abs/2602.11988)

**Avenue:** Include always-loaded repository guidance as a baseline, not just “no memory.” A task-specific lookup should earn its complexity by delivering applicable information without importing unrelated requirements. This paper helps motivate measuring whether guidance redirects work usefully or simply creates more work.

### 15. Reliable Memory from Unstable Growth: Curating Self-Written Agent Playbooks with Randomized Experiments
**October 13, 2026 · arXiv:2610.18427**

A very recent paper proposing Randomized Block Curation. It randomly includes or excludes related blocks of lessons on practice tasks, estimates their effect on success, and chooses among curated, whole, or absent playbooks using fresh practice episodes. [arXiv](https://arxiv.org/abs/2610.18427)

**Avenue:** Explore **admitting memory based on measured downstream value**, not how convincing the explanation sounds. For the task index, randomize complete recipes or coherent rule bundles—not individual required companion roles. Keep these curation experiments on development tasks, separate from the locked evaluation set.

## 4. Keeping recipes valid as the repository changes

### 16. PACMI: Provenance-Aware Cascading Memory Invalidation for Long-Term LLM Agents
**October 5, 2026 · arXiv:2610.05732**

Represents memory and evidence in a provenance graph with typed dependencies, propagating changes through a four-state validity structure. Its diagnostic benchmark supplies the pre-existing dependency graph; it **does not establish how to discover those dependencies from raw histories**, and its controlled scope limits broader conclusions. [arXiv](https://arxiv.org/abs/2610.05732)

**Avenue:** This is a close architectural reference for the index's refresh system: invalidate dependent claims, distinguish historical usefulness from current validity, and prevent stale premises from silently guiding new work. The unsolved implementation question for the task index is dependency discovery and completeness.

### 17. STALE: Can LLM Agents Know When Their Memories Are No Longer Valid?
**2026 · arXiv:2605.06527**

Studies implicit conflicts: new information can invalidate earlier memory without explicitly negating it. Its evaluation distinguishes recognizing outdated state, resisting questions with stale premises, and adapting subsequent behavior. The setting is long-term personal memory rather than source-code repositories. [arXiv](https://arxiv.org/abs/2605.06527)

**Avenue:** Translate its failure categories into repository tests. For example, add a new registration mechanism without deleting the old function. Can the index recognize that the old “how to register” recipe is no longer sufficient, even though its cited source still exists?

### 18. Build Scripts with Perfect Dependencies
**2020 · arXiv:2007.12737**

Introduces the Rattle build system, which captures dependencies through execution tracing rather than relying entirely on manually declared dependency lists. It explores incremental execution, caching, and safe speculative parallelism. [arXiv](https://arxiv.org/abs/2007.12737)

**Avenue:** Think of the semantic index as a **build artifact** with tracked inputs and invalidation rules. However, the builder’s observed reads cannot by themselves prove that semantic dependencies are complete. The proposal’s additional dependency on sibling-discovery queries is important: a newly added file may matter even when no previously cited file changed.

### 19. Memory as Middleware for Self-Improving AI Agents
**September 2026 · arXiv:2609.32091**

Argues for memory as a pluggable layer independent of particular agents and storage engines. It covers isolation, write consistency, sharing with provenance, and lifecycle governance, with ALTK-Evolve as a reference implementation. This is primarily an architectural research agenda, not proof of improved coding-task completion. [arXiv](https://arxiv.org/abs/2609.32091)

**Avenue:** Relevant to the proposal's separation of a private trace overlay from a public, source-backed index. Explore a common memory contract usable by different coding agents while keeping repository identity, workspace scope, access control, and publication eligibility explicit.

## 5. Selective retrieval, escalation, and eventual distillation

### 20. Repoformer: Selective Retrieval for Repository-Level Code Completion
**2024 · arXiv:2403.10059**

Learns when retrieval is likely to improve code completion instead of retrieving unconditionally. It also addresses robustness to unhelpful or harmful retrieved context. Its experiments concern repository-level completion, not full issue-resolution agents. [arXiv](https://arxiv.org/abs/2403.10059)

**Avenue:** Treat “use a recipe” as a decision rather than a default. Compare unconditional recipe injection with a policy that considers applicability and expected benefit. The no-match and fallback behavior should be part of the evaluated system, not classified automatically as failure.

### 21. RouteLLM: Learning to Route LLMs with Preference Data
**2024 · arXiv:2406.18665**

Trains routers to choose between stronger and weaker language models using preference data, balancing response quality against cost. It also examines transfer when the underlying model pair changes. [arXiv](https://arxiv.org/abs/2406.18665)

**Avenue:** For the task index, routing should consider **the task plus available recipe coverage**, not only the apparent difficulty of the question. A difficult task with a complete validated recipe might be cheap-model work; a simple-looking task with stale or ambiguous guidance may deserve escalation.

### 22. SERA: Soft-Verified Efficient Repository Agents
**2026 · arXiv:2601.20789**

Studies efficient supervised fine-tuning of repository-specialized coding agents using synthetically generated trajectories. Unlike external recipe retrieval, its central mechanism places repository-specific capability into model weights. [arXiv](https://arxiv.org/abs/2601.20789)

**Avenue:** Read this for the later distillation phase. Once recipes demonstrably help, compare retaining knowledge externally against teaching a local model repository-specific behavior. The external index and specialized weights have different update and verification requirements; this is a later comparison, not a prerequisite for the first experiment.

## 6. Deeper conceptual foundations worth exploring

These are less direct implementation recipes, but they provide useful ways to reason about what the system is becoming.

### 23. An Overview of Hierarchical Task Network Planning
**2014 · arXiv:1403.7426**

Explains planning through hierarchies of compound tasks and primitive actions. Rich domain knowledge describes how higher-level tasks can be decomposed into executable subtasks. [arXiv](https://arxiv.org/abs/1403.7426)

**Avenue:** Extension recipes could become **conditional task-decomposition methods** rather than prose checklists. “Add a primitive” decomposes into required roles; optional branches depend on the primitive’s characteristics. Explore a small typed planning language before assuming a general-purpose knowledge graph is the best representation.

### 24. DreamCoder: Growing Generalizable, Interpretable Knowledge with Wake-Sleep Bayesian Program Learning
**2020 · arXiv:2006.08381**

Alternates between solving programming tasks, learning reusable symbolic abstractions, and training a neural model that guides future program search. The learned abstractions compose into increasingly expressive libraries. [arXiv](https://arxiv.org/abs/2006.08381)

**Avenue:** Explore whether task recipes can eventually **compose and factor into reusable subrecipes**. Several extension patterns may share “introduce a tagged type,” “wire module exports,” or “register a new handler.” This is a longer-term library-learning direction, not evidence that such abstraction will automatically work on this repository.

### 25. A Knowledge Compilation Map
**arXiv:1106.1819 · Foundational logic paper**

Analyzes knowledge representations according to how compact they are and which queries and transformations they support efficiently. Its formal results concern logical compilation languages, not LLM-generated software recipes. [arXiv](https://arxiv.org/abs/1106.1819)

**Avenue:** Start by specifying which questions the compiled artifact must answer: applicability, missing roles, evidence resolution, affected claims, and alternatives. Then choose a representation around those operations. The useful lesson is not “use a particular logical formalism,” but **design the compiled form around the queries it must make cheap**.

## Where to focus

The strongest recommendation is to investigate these four questions in order:

| Research question | Most useful papers | Suggested experiment |
|---|---|---|
| **Can complete recipes close the cheaper model’s quality gap?** | Memp, SkillsBench, SWE-Skills-Bench | Hold the cheap model fixed; compare source locations alone, summaries, and human-reviewed complete recipes. |
| **Can the builder reliably produce those recipes?** | Code2Skill, SkillGenBench, ACQUIRE | Build before held-out tasks are revealed; separately score grounding, companion completeness, applicability, and actual edit success. |
| **What structure is necessary?** | CodePlan, HyperGraphRAG, HTN overview | Compare prose recipes against typed conditional role bundles and explicit edit plans. |
| **Can the benefit survive repository evolution?** | PACMI, STALE, Rattle | Introduce new companions, changed registration paths, moved examples, and altered checks; measure invalidation and safe fallback. |

**The distinction to keep central is “source-backed” versus “task-complete.”** A recipe can cite only real symbols and still omit the one companion change needed for the task to succeed. Code2Skill is useful for the first property; CodePlan and the required-role evaluations are useful for the second.

That is why the proposed human-reviewed recipe arm remains the right first test. It isolates whether reusable repository understanding actually enables the cheaper agent to make the same correct change—before investing in a large automatic index or a distilled selector.
