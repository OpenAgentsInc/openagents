# RLM Source Notes

**STATUS: Point-in-time research record (2026-07-21).** Not product authority.
Consult `MASTER_ROADMAP.md` for sequencing.

## Paper package

| Field | Value |
| --- | --- |
| Title | Recursive Language Models |
| Authors | Alex L. Zhang, Tim Kraska, Omar Khattab |
| Affiliation | MIT CSAIL |
| Identifier | `arXiv:2512.24601v3` |
| Public page | <https://arxiv.org/abs/2512.24601> |
| Local package | `/Users/christopherdavid/Downloads/arXiv-2512.24601v3/` |
| Package size | ~11 MB, 52 files |
| Compiler | `pdflatex` (texlive 2025 per `00README.json`) |
| Venue style | NeurIPS 2026 preprint style (`neurips2026.sty`) |

### Local digests (selected)

| Path | SHA-256 |
| --- | --- |
| `main.tex` | `a74d30a288af17107f21a8fa00655245d62bca337aadd3f5357e42f8e1fa944b` |
| `sections/sec1-intro.tex` | `ce09f63168686644bae9ca8a7993b789ac549ab1f56b1117af88aed23ebb7703` |
| `sections/sec3-rlm.tex` | `9876c4e66759cba13b54b1a801fa882ad2e6ab0c34529201ad8e2f135b05a7b6` |
| `sections/sec4-results.tex` | `23edb9963060177197f6a5fbcc9ff042b391c414503cba13661ace4cb86ddf91` |

Extraction for these notes used the local LaTeX source tree (sections, tables,
appendix). The public OpenAgents tree does not vendor the package.

### Paper layout (source tree)

- `main.tex` — title, abstract, section includes, appendix
- `sections/sec1-intro.tex` — problem, context rot, contribution
- `sections/sec3-rlm.tex` — definition and algorithm
- `sections/sec4-results.tex` — tasks, baselines, observations, qualitative
- `sections/sec6-related-works.tex` — long-context systems and sub-LM work
- `sections/sec7-limitations.tex` / `sec7-conclusion.tex`
- `tables/main.tex` — main benchmark table
- `tables/pseudocode.tex` / `bad-algorithm.tex` — good vs weak agent algorithms
- `appendix/` — training, negative results, methods, benchmarks, trajectories, cost

## Code repository

| Field | Value |
| --- | --- |
| Remote | <https://github.com/alexzhang13/rlm> |
| Local clone | `/Users/christopherdavid/work/projects/repos/rlm` |
| Manifest entry | `alexzhang13/rlm` in `~/work/projects/manifest.txt` |
| Pinned commit (this note) | `72d6940142ddfb84ee6be573dc999a37e633e671` |
| Commit date | 2026-06-25 |
| Subject | `bump version` |
| PyPI package name | `rlms` |
| Version in `pyproject.toml` | `0.1.3` |
| License | MIT (Copyright 2025 Alex Zhang) |
| Python | `>=3.11` |
| Classifier | Development Status :: 4 - Beta |

Companion repos and links cited by the project:

- Minimal implementation: <https://github.com/alexzhang13/rlm-minimal>
- Docs: <https://alexzhang13.github.io/rlm/>
- Blog (2025 idea sketch): <https://alexzhang13.github.io/blog/2025/rlm/>
- Training stack: Prime Intellect `prime-rl` + `verifiers`
- Example trained model:
  <https://huggingface.co/mit-oasys/rlm-qwen3-30b-a3b-v0.1>

## Citation (from project README)

```bibtex
@misc{zhang2026recursivelanguagemodels,
  title={Recursive Language Models},
  author={Alex L. Zhang and Tim Kraska and Omar Khattab},
  year={2026},
  eprint={2512.24601},
  archivePrefix={arXiv},
  primaryClass={cs.AI}
}
```

## What this folder does not contain

- Full paper PDF or LaTeX vendor copy
- Upstream `rlm` source tree
- Secrets, API keys, or private trajectories
- Product-promise or release authority
