# AgentCL

Research notes on `AgentCL: Toward Rigorous Evaluation of Continual Learning in
Language Agents` (`arXiv:2606.02461v2`, June 2, 2026).

Full source PDF for local agents:

- Private workspace copy:
  `/Users/christopherdavid/work/docs/research/agentcl/sources/2606.02461v2.pdf`
- SHA-256:
  `ae411b3ef03bc07fb9a67e2ac637bc4a520f877942c3a77539ff732599e356eb`
- Public arXiv page: <https://arxiv.org/abs/2606.02461>
- Dataset noted by the paper:
  <https://huggingface.co/datasets/osunlp/AgentCL>

## Source Boundary

This public repo does not vendor the PDF. The notes here are paraphrased,
OpenAgents-oriented summaries and design implications. Agents that need to audit
the full paper should read the private workspace copy above, or fetch the public
arXiv source.

## Contents

- [`source.md`](source.md) - exact source location, extraction metadata, and
  citation notes.
- [`paper-summary.md`](paper-summary.md) - structured summary of the framework,
  benchmark construction, metrics, methods, results, and limitations.
- [`openagents-implications.md`](openagents-implications.md) - what the paper
  suggests for OpenAgents memory, StudyBench-style evaluation, Pylon/Khala
  traces, and product-claim boundaries.

## One-Paragraph Summary

AgentCL argues that language-agent continual-learning benchmarks need controlled
task streams, not just long histories or arbitrary task sequences. The benchmark
contrasts naive streams with compositional streams where earlier tasks expose
sub-solutions, evidence, or workflows that later tasks can reuse. Its two-pass
protocol separates plasticity gain, stability gain, and held-out generalization
gain. Across coding, deep-research, and reasoning settings, compositional
streams reveal much larger differences between memory systems than naive
streams, while naive and held-out settings expose memory interference. For
OpenAgents, the immediate lesson is to make memory claims only after measuring
reuse, persistence, and harmlessness separately.
