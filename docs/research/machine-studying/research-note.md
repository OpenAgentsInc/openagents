# Machine Studying: OpenAgents Research Note

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Source: <https://jacobxli.com/blog/2026/machine-studying/>

Published: June 17, 2026

## Thesis

Machine studying is a proposed problem setting for agents that must become
useful in domains they did not see during training. The agent receives a corpus
of documents and no known downstream task distribution, then performs some
preparation before evaluation. That preparation may change model weights,
prompts, tools, notes, indexes, or other harness state.

The key distinction is that the corpus remains available at test time. The goal
is not simply to memorize the corpus or replace retrieval. The goal is to make
the agent more expert: better able to convert a given inference budget into
accurate work.

## Core definitions

- Corpus: the document set that defines the new domain.
- Agent: model plus harness.
- Studying algorithm: anything the agent does to itself with the corpus before
  it knows the downstream exam.
- Expertise: weighted area under performance as inference compute increases.
- Studying intelligence: weighted area under expertise as study compute
  increases.

The article's central measurement move is to score the whole
quality-versus-cost curve, not just peak accuracy. This penalizes agents that
can solve a task only by rereading or searching heavily during the exam.

## Why search is not enough

The article argues that search access and expertise are different capabilities.
An agent can have the corpus, tools, and more tokens, yet still search for the
wrong things, distrust the wrong claims, or fail to recognize relevant evidence
once retrieved.

This matters for OpenAgents because our systems already rely on repo search,
retrieval, skills, and runbooks. Machine studying gives us a sharper test: does
the preparation make the agent cheaper and more accurate at the same downstream
work, or does it only move effort from one phase to another?

## StudyBench formulation

StudyBench is the benchmark introduced by the article. Each task gives a corpus
and a hidden exam, with no RL environment or known reward function.

Current tasks:

- Studying-DSPy: codebase and tests for a programming-library domain where
  stale model priors are likely.
- Studying-OpenClaw: a newer self-hosted AI-assistant framework intended to
  test learning from scratch.
- Studying-Literature: roughly 50,000 ML papers from major conferences,
  evaluated through a hidden related-work/retrieval task.

The benchmark uses multiple inference budgets, including direct answers,
bounded ReAct-style tool use, and forced longer search. Expertise is computed
from the resulting performance curve.

## Main findings

1. Equally capable models can differ sharply in domain expertise.

   The article compares a newer smaller frontier model against an older larger
   one. On DSPy, where the newer model plausibly has more recent training data,
   it wins across budgets. On OpenClaw, which appears after both cutoffs, both
   remain weak even with search.

2. Small competent models have large expertise headroom.

   Qwen3.5-9B can write normal Python/PyTorch, but performs poorly on the
   domain exams without studying. Forcing more search improves some lenient
   scores, which suggests useful evidence is reachable but not efficiently used.

3. Naive weight updates did not create expertise in the early runs.

   Continual pre-training on corpus text underperformed the base agent in the
   reported coding tasks. Synthetic SFT improved closed-book behavior but did
   not improve the tool-using performance curve enough to raise expertise.

4. A simple cheatsheet helped most where cheap inference matters.

   A note created by exploring the repo improved DSPy expertise, mostly at low
   inference budgets. This is shallow but important: amortized context
   management can be an effective baseline against weight-update methods.

5. Retrieval reach does not imply expert selection.

   On the literature task, two models retrieved similar amounts of relevant
   evidence, but the newer model kept more of the important references in its
   final selection, especially recent papers. The article treats this as a
   controlled signal that recognizing and prioritizing found evidence depends
   on expertise, not just retrieval.

## Three studying paradigms

- Self-supervised objectives: next-token prediction, test-time training, or
  approximating long-context attention. These are broad and universal, but may
  optimize memorization instead of agent expertise.
- Synthetic data and environments: generate questions, rubrics, retellings, or
  self-made tasks from the corpus. This can become supervised learning or RL,
  but the alignment between synthetic tasks and hidden future tasks remains
  uncertain.
- Amortized context management: have the agent build notes, indexes, skills,
  wikis, or other durable context artifacts. This does not update weights, but
  it directly targets cheaper future inference.

The article's practical conclusion is not that one paradigm wins. It suggests
deep studying probably needs a combination of all three, and that
"approximate long context" is too narrow as the objective.

## OpenAgents implications

1. Treat skills and runbooks as studying artifacts.

   `AGENTS.md`, repo-local runbooks, `skill.md` files, and semantic indexes are
   not just context. They are amortized study outputs. We should evaluate them
   by whether they shift the quality/cost curve on real downstream work.

2. Add budget curves to agent benchmarks.

   For research tasks and Pylon/Autopilot work, record performance at direct,
   small-search, medium-search, and forced-search budgets. A result that only
   improves at high budgets may be less valuable than a small low-budget gain.

3. Measure "retrieved" separately from "used well."

   For codebase and literature tasks, log whether the right evidence entered
   the trajectory, then separately score whether the agent selected or applied
   it. This mirrors the article's reach versus recall@100 split.

4. Make study packets first-class.

   A useful OpenAgents experiment would give an agent a new repo, let it create
   a durable study packet, then evaluate against hidden coding tasks. The study
   packet should include a semantic source map, invariants, examples, tests,
   known traps, and a short task-facing cheatsheet.

5. Avoid claiming learning from retrieval alone.

   Retrieval, long context, and repeated search are necessary tools, but they
   should not be described as expertise unless they reduce inference cost or
   improve accuracy at fixed cost on unseen tasks.

## Candidate local experiment

Create a small "study a repo" benchmark inside OpenAgents:

1. Select a repo or package with current behavior not obvious to a base model.
2. Prepare a hidden exam of 10 to 30 implementation questions with deterministic
   checks.
3. Run a baseline agent at four budgets: direct, k=5, k=20, and forced k=20.
4. Let the same agent create a study packet from the corpus.
5. Re-run the same budgets with the study packet injected.
6. Report both peak score and weighted area under the score-vs-token curve.

This would convert the article's concept into a directly usable OpenAgents
measurement loop.

