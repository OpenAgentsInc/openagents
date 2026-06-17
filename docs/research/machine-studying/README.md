# Machine Studying

Source: <https://jacobxli.com/blog/2026/machine-studying/>

Article metadata:

- Title: "Machine Studying"
- Authors: Jacob Xiaochen Li, Rick Battle, Omar Khattab
- Published: June 17, 2026
- Benchmark: StudyBench / MSBench

## Copyright note

The source page ends with an explicit copyright notice. This folder therefore
does not vendor a verbatim article mirror, full HTML snapshot, screenshots, or
copied prose. It captures the research content in a reusable OpenAgents form:
source metadata, a section-by-section map, paraphrased notes, figure/data
tables, and links back to the original article for the complete text.

## MVP source boundaries

The OpenAgents StudyBench MVP uses the machine-studying work as an internal
dogfood and benchmark substrate, not as product copy.

Boundary refs:

- `openagents_studybench.v0`: OpenAgents-owned StudyBench-compatible dataset
  shape over a pinned `openagents` commit.
- `openagents_repo_corpus_manifest.v0`: deterministic manifest of admitted repo
  files, digests, exclusions, and source authority refs.
- `openagents_study_packet.v0`: refs-only study packet for repository memory
  and launch work.
- `probe.studybench_rubric_closeout.v0`: Probe evidence bundle extension for
  StudyBench claim and rubric scores.

Dataset boundaries:

- Upstream `jacobli/studybench` rows are external public calibration only. They
  can test loaders, rubric scoring, and DSPy/GEPA behavior, but cannot be the
  only evidence for OpenAgents product claims.
- OpenAgents public-retained rows are committed examples and regression
  fixtures. They may include public gold answers and rubrics.
- OpenAgents private validation and holdout rows are not committed to the
  public repo. Public docs may carry only split refs, checksums, and policy
  refs for those rows.
- Private holdout rows cannot feed study packets, public retained examples, or
  GEPA training. If they leak, retire the split and mint a new one.

Evaluation boundaries:

- Answer-mode and agentic patch-mode evaluation are both required for MVP.
  Answer mode tests source-grounded codebase understanding; patch mode tests
  tool-loop repo editing with budgets, tests, patches, and closeouts.
- Candidate agents must not see private gold answers or private rubrics.
  Scorers may see scorer-visible gold, rubric, and evidence material.
- Probe records evidence and closeouts. Psionic may optimize candidates.
  Tassadar may verify deterministic substrate. Blueprint and product-promise
  gates remain the authority for runtime, marketplace, payout, and public-copy
  changes.

## Files

- `research-note.md` - OpenAgents-oriented summary and implications.
- `source-map.md` - article outline, source links, and citation metadata.
- `figures-and-data.md` - figure inventory and numerical results captured from
  the article's charts and appendix.
- `2026-06-17-blueprint-marketplace-ties.md` - Blueprint, marketplace, Probe,
  Pylon, Tassadar, and DSPy/GEPA integration audit.
- `2026-06-17-tassadar-openagents-repo-studying-roadmap.md` - roadmap for
  pairing Tassadar and machine studying over the `openagents` repo.
- `2026-06-17-studybench-openagents-benchmark-audit.md` - audit of using
  upstream StudyBench as calibration while building an OpenAgents-owned
  StudyBench-style repo benchmark.
- `2026-06-17-openagents-studybench-mvp-issue-roadmap.md` - issue-level MVP
  roadmap for contracts, corpus manifests, runners, rubric scoring, Psionic
  GEPA feedback, Blueprint gates, and Forge Coder projection.
