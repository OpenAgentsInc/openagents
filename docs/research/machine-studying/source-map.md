# Machine Studying Source Map

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Primary source: <https://jacobxli.com/blog/2026/machine-studying/>

Read date: June 17, 2026

## Source metadata

- Title: "Machine Studying"
- Published: June 17, 2026
- Authors and affiliations:
  - Jacob Xiaochen Li, MIT CSAIL
  - Rick Battle, Broadcom
  - Omar Khattab, MIT CSAIL
- Page generator: Jekyll / al-folio
- The page includes an explicit "Copyright 2026 Jacob X. Li" footer.
- The HTML canonical tag observed in the page points to
  `https://seuperhakkerja.github.io/blog/2026/machine-studying/`, while the
  user-provided and readable URL is `https://jacobxli.com/blog/2026/machine-studying/`.

## Citation

```bibtex
@misc{li2026machinestudying,
  title  = {Machine Studying},
  author = {Li, Jacob Xiaochen and Battle, Rick and Khattab, Omar},
  year   = {2026},
  month  = {Jun},
  url    = {https://jacobxli.com/blog/2026/machine-studying/}
}
```

## Section map

1. Introduction
   - Defines machine studying as autonomous preparation from a corpus before
     downstream evaluation is known.
   - Contrasts studying with RAG, long context, and hand-built RL environments.
   - Introduces expertise as efficiency over inference compute.

2. Studying converts a corpus into expertise
   - Frames the corpus as the post-training source of domain knowledge.
   - Allows studying to modify either weights or harness state.
   - Emphasizes that test-time corpus access remains available.

3. Can't the agent just search the corpus?
   - Argues that search depends on prior knowledge and can be misdirected by
     stale or conflicting priors.
   - Uses a model-name example to illustrate the danger of confident priors.

4. Expertise is the efficiency of turning inference compute into accuracy
   - Defines expertise as weighted area under a performance curve over
     log-scaled inference budgets.
   - Introduces studying intelligence as expertise gained per study budget.

5. StudyBench: can agents acquire expertise in novel domains?
   - Introduces three tasks: DSPy, OpenClaw, and literature review.
   - Describes coding exams with hidden rubrics and a literature task with
     hidden citation targets.

6. Equally capable models can have very different levels of expertise
   - Compares frontier models with different training cutoffs.
   - Reports that the newer model has an advantage on DSPy but not OpenClaw.
   - Establishes Qwen3.5-9B as the smaller model for studying experiments.

7. Three broad paradigms for studying
   - Self-supervised objectives over the corpus.
   - Synthetic data, questions, environments, and RL-style practice.
   - Amortized context management through notes, skills, or wikis.

8. Memorization is no substitute for expertise
   - Reports preliminary studying runs on Qwen3.5-9B.
   - Continual pre-training does not improve the reported expertise.
   - Synthetic SFT helps closed-book answers but not the agentic curve.
   - A simple cheatsheet improves DSPy low-budget performance.

9. Retrieval is no substitute for expertise either
   - Uses the literature task to split retrieval reach from expert selection.
   - Reports that two models retrieve similar evidence, but the newer model
     keeps more important references, especially recent ones.

10. Epilogue
    - Calls for agents that can turn reading and thinking into expertise from
      a corpus, instead of relying on hand-built skills, RL environments, or
      the next pre-training cycle.

11. Appendix
    - Outcome distribution chart for GPT-model attempts.
    - Per-budget score and generation-token tables for Qwen3.5-9B variants.

## Source links observed

- StudyBench: <https://huggingface.co/datasets/jacobli/studybench>
- MSBench: <https://huggingface.co/datasets/jacobli/msbench>
- DSPy: <https://dspy.ai>
- OpenClaw: <https://openclaw.ai/>
- Pedagogical RL note: <https://noahziems.com/pedagogical-rl>
- On-policy distillation recipe:
  <https://thinkingmachines.ai/blog/on-policy-distillation/>
- Continual learning discussion:
  <https://www.dwarkesh.com/p/timelines-june-2025>
- Letta continual learning post:
  <https://www.letta.com/blog/continual-learning>
- Interconnects post:
  <https://www.interconnects.ai/p/contra-dwarkesh-on-continual-learning>
- Karpathy gist on document-to-wiki compilation:
  <https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f>
- X/Twitter reference on reasoning and search:
  <https://x.com/lateinteraction/status/1909011076605518124>
- X/Twitter reference on compute curves:
  <https://x.com/polynoamial/status/2064210146558136827>

ArXiv and ACL references observed:

- <https://arxiv.org/abs/2004.10964>
- <https://arxiv.org/abs/2302.00487>
- <https://arxiv.org/abs/2407.10930>
- <https://arxiv.org/abs/2409.07431>
- <https://arxiv.org/abs/2504.07952>
- <https://arxiv.org/abs/2504.13171>
- <https://arxiv.org/abs/2506.06266>
- <https://arxiv.org/abs/2506.10943>
- <https://arxiv.org/abs/2508.06813>
- <https://arxiv.org/abs/2508.09494>
- <https://arxiv.org/abs/2512.23675>
- <https://arxiv.org/abs/2512.24601>
- <https://arxiv.org/abs/2602.16284>
- <https://arxiv.org/abs/2604.17680>
- <https://arxiv.org/abs/2605.12484>
- <https://arxiv.org/abs/2605.19932>
- <https://arxiv.org/abs/2606.05661>
- <https://aclanthology.org/2023.findings-acl.738.pdf>
- <https://aclanthology.org/anthology-files/anthology-files/pdf/findings/2025.findings-emnlp.1172.pdf>

