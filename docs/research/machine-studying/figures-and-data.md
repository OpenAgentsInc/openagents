# Machine Studying Figure And Data Capture

Source: <https://jacobxli.com/blog/2026/machine-studying/>

This file captures the article's figure inventory and numerical measurements in
a compact, non-verbatim form. The original page uses inline canvas charts and
HTML tables. See the source URL for the complete prose and rendered diagrams.

## Figure inventory

| Figure | Subject | Captured content |
| --- | --- | --- |
| 1 | Stale prior causing wrong search | A model asked about `Qwen 3.6` treats the prompt as likely wrong and searches for `Qwen3 0.6B`. |
| 2 | Expertise as weighted area | Conceptual curves for ordinary agent, shallow crammer, brute-force searcher, and expert. |
| 3 | GPT-5.1 vs GPT-5.4-mini | Strict-grading accuracy across DSPy and OpenClaw inference budgets. |
| 4 | Qwen3.5-9B baseline | Strict and lenient accuracy across inference budgets. |
| 5 | Studied Qwen variants | Base, synthetic SFT plus OPSD, CPT(code), and CPT(doc). |
| 6 | Cheatsheet baseline | Score versus generation tokens for base and cheatsheet agents. |
| 7 | Literature task reach vs recall | GPT-5.1 and GPT-5.5 retrieve similar evidence, but differ in final selection. |
| 8 | Retrieval-controlled selection | Selection rates after holding retrieved evidence constant. |
| 9 | Outcome distributions | Appendix chart of failure/success outcome categories. |

## Expertise weighting example

The article computes expertise as a weighted average over log-scaled inference
compute. In its worked example, budgets below the first measured point are
floored at zero.

| Budget | Score | Weight |
| --- | ---: | ---: |
| 5k tokens | 10% | 0.223 |
| 10k tokens | 20% | 0.289 |
| 20k tokens | 30% | 0.189 |
| 100k tokens | 40% | 0.099 |

Reported weighted expertise: about 17.6%. The measured weights sum to 0.80;
the remaining 0.20 is assigned to the below-5k region with score zero.

## StudyBench task cards

| Task | Corpus | Hidden-exam target | Main stressor |
| --- | --- | --- | --- |
| Studying-DSPy | DSPy codebase and tests | Coding questions about current DSPy usage | Correct stale library knowledge. |
| Studying-OpenClaw | OpenClaw codebase and issues | Configuration and extension questions | Learn a new framework from scratch. |
| Studying-Literature | About 50k ML papers, 2018-2025 | Related-work writing for ICLR 2026 targets | Select important evidence from a corpus too large for context. |

The original page includes sample prompts and rubrics for the DSPy,
OpenClaw, and literature tasks. Those are not reproduced here verbatim.

## Figure 3: GPT-5.1 vs GPT-5.4-mini strict accuracy

Each row is accuracy by inference setting. The article labels the settings by
maximum ReAct iterations and includes a no-early-exit forced 20-iteration point.

| Domain | Model | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | ---: | ---: | ---: | ---: |
| DSPy | GPT-5.1 | 0.0 | 12.7 | 10.8 | 29.8 |
| DSPy | GPT-5.4-mini | 0.0 | 26.8 | 34.2 | 39.7 |
| OpenClaw | GPT-5.1 | 0.0 | 5.0 | 5.0 | 10.0 |
| OpenClaw | GPT-5.4-mini | 0.0 | 8.3 | 9.7 | 9.7 |

## Figure 4: Qwen3.5-9B baseline accuracy

| Domain | Grading | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | ---: | ---: | ---: | ---: |
| DSPy | Lenient | 3.3 | 9.0 | 11.6 | 28.0 |
| DSPy | Strict | 0.0 | 0.0 | 2.0 | 0.8 |
| OpenClaw | Lenient | 2.3 | 6.9 | 15.8 | 17.6 |
| OpenClaw | Strict | 0.0 | 0.0 | 6.6 | 6.4 |

## Figure 5: Qwen3.5-9B studied variants

DSPy:

| System | Grading | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | ---: | ---: | ---: | ---: |
| Original | Lenient | 3.3 | 9.0 | 11.6 | 28.0 |
| SFT + OPSD | Lenient | 9.4 | 7.4 | 8.5 | 21.0 |
| CPT(code) | Lenient | 5.1 | 7.4 | 7.0 | 14.3 |
| CPT(doc) | Lenient | 3.8 | 7.2 | 6.2 | 14.6 |
| Original | Strict | 0.0 | 0.0 | 2.0 | 0.8 |
| SFT + OPSD | Strict | 1.1 | 0.0 | 0.0 | 2.2 |
| CPT(code) | Strict | 0.8 | 0.9 | 1.0 | n/a |
| CPT(doc) | Strict | 0.0 | 0.0 | 0.0 | n/a |

OpenClaw:

| System | Grading | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | ---: | ---: | ---: | ---: |
| Original | Lenient | 2.3 | 6.9 | 15.8 | 17.6 |
| CPT(code) | Lenient | 2.2 | 8.8 | 15.3 | 14.1 |
| Original | Strict | 0.0 | 0.0 | 6.6 | 6.4 |
| CPT(code) | Strict | 0.0 | 4.5 | 6.7 | 6.3 |

## Expertise table, lenient weighted AUC

| System | Studying-DSPy | Studying-OpenClaw |
| --- | ---: | ---: |
| Qwen3.5-9B base | 12.44 | 12.40 |
| SFT + OPSD | 4.88 | n/a |
| CPT(code) | 5.86 | 11.93 |
| CPT(doc) | 6.39 | n/a |
| Cheatsheet | 15.46 | 12.43 |

## Figure 6: Cheatsheet score versus generation tokens

DSPy:

| System | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | --- | --- | --- |
| Base | 4.1k / 3.3 | 7.9k / 8.6 | 8.6k / 9.6 | 34.6k / 29.4 |
| Cheatsheet | 3.9k / 6.3 | 6.1k / 14.4 | 7.1k / 14.1 | 29.9k / 23.1 |

OpenClaw:

| System | Direct | k=5 | k=20 | k=20 forced |
| --- | --- | --- | --- | --- |
| Base | 4.1k / 2.3 | 4.6k / 6.9 | 9.7k / 15.9 | 24.3k / 17.6 |
| Cheatsheet | 3.8k / 4.3 | 6.0k / 8.6 | 9.1k / 15.2 | 20.1k / 18.1 |

Cells are generation tokens per question / score.

## Figure 7: Literature task reach versus recall@100

Overall macro means:

| Target set | Metric | GPT-5.1 | GPT-5.5 |
| --- | --- | ---: | ---: |
| Must-cite, n=50, gold=884 | reach | 60.0 | 63.9 |
| Must-cite, n=50, gold=884 | recall@100 | 42.8 | 57.0 |
| Related-work, n=50, gold=969 | reach | 52.3 | 56.4 |
| Related-work, n=50, gold=969 | recall@100 | 37.3 | 47.9 |

Must-cite reach by publication-year bucket:

| Model | <=2020 | 2021 | 2022 | 2023 | 2024 | 2025 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GPT-5.1 | 36.4 | 44.8 | 51.8 | 56.6 | 68.7 | 69.9 |
| GPT-5.5 | 40.6 | 49.3 | 55.3 | 64.5 | 71.8 | 74.2 |

Must-cite recall@100 by publication-year bucket:

| Model | <=2020 | 2021 | 2022 | 2023 | 2024 | 2025 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GPT-5.1 | 29.4 | 35.8 | 42.4 | 39.5 | 49.3 | 44.5 |
| GPT-5.5 | 39.2 | 47.8 | 49.4 | 57.2 | 62.6 | 64.1 |

## Figure 8: Retrieval-controlled selection rate

Share of reached must-cite papers kept:

| Model | <=2020 | 2021 | 2022 | 2023 | 2024 | 2025 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GPT-5.1 | 88.1 | 87.0 | 83.8 | 68.9 | 72.5 | 65.6 |
| GPT-5.5 | 95.2 | 95.7 | 89.2 | 91.9 | 88.7 | 89.3 |
| GPT-5.5 lead | +7 | +9 | +5 | +23 | +16 | +24 |

## Appendix: per-budget scores and tokens

Studying-DSPy, lenient grading:

| System | Direct | k=5 | k=20 | k=20 forced | Expertise |
| --- | --- | --- | --- | --- | ---: |
| Qwen3.5-9B base | 3.3 / 4.1k | 8.6 / 7.9k | 9.6 / 8.6k | 29.4 / 34.6k | 12.44 |
| Cheatsheet | 6.3 / 3.9k | 14.4 / 6.1k | 14.1 / 7.1k | 23.1 / 29.9k | 15.46 |
| SFT + OPSD | 9.4 / 9.2k | 7.4 / 15.2k | 8.5 / 16.6k | 21.0 / 155.8k | 4.88 |
| CPT(code) | 5.1 / 5.9k | 7.4 / 8.8k | 7.0 / 10.6k | 14.3 / 59.3k | 5.86 |
| CPT(doc) | 3.8 / 4.8k | 7.2 / 8.2k | 6.2 / 9.9k | 14.6 / 74.9k | 6.39 |

Studying-OpenClaw, lenient grading:

| System | Direct | k=5 | k=20 | k=20 forced | Expertise |
| --- | --- | --- | --- | --- | ---: |
| Qwen3.5-9B base | 2.3 / 4.1k | 6.9 / 4.6k | 15.8 / 9.7k | 17.6 / 24.3k | 12.40 |
| Cheatsheet | 4.3 / 3.8k | 8.6 / 6.0k | 15.2 / 9.1k | 18.1 / 20.1k | 12.43 |
| CPT(code) | 2.2 / 3.4k | 8.8 / 4.7k | 15.3 / 11.6k | 14.1 / 32.1k | 11.93 |

Token counts are generation tokens per question averaged over three seeds in
the article's coding benchmark runs.

