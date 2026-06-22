# TMAX — An Open RL Recipe for Terminal Agents

Research notes on **TMAX: A Simple Recipe for Terminal Agents** (Ivison, Yin,
Shao, Xiao, Lambert, Hajishirzi — Ai2 / UW, 2026), prompted by Nathan Lambert's
writeup framing it as *the* recipe-work exemplar for terminal-agent RL.

- Code: <https://github.com/hamishivi/tmax> · local clone `projects/repos/tmax`
- Models & data (incl. raw RL rollouts): <https://huggingface.co/collections/allenai/tmax>
- Paper PDF: `projects/repos/tmax/assets/paper.pdf` (GitHub-only; no arXiv)
- Blog: <https://wai-org.com/blog/tmax/>

## Contents

- [`paper.md`](paper.md) — full structured Markdown rendition of the paper
  (converted from the PDF; no arXiv HTML exists, so `arxiv2md` does not apply).
- [`synthesis.md`](synthesis.md) — what TMAX means for us: the "recipe work"
  thesis, and concrete connections to **Tassadar** (verified-work RL, the
  reward-hacking → independent-replay argument, the container-cost bottleneck the
  compute market answers) and to **Sakana / Fugu** (DPPO/FP32 stability lessons
  for training the coordinator; TMAX-trained terminal agents as Fugu's *worker*
  vs Fugu as the *coordinator*).

## Why this paper matters (Lambert's framing)

- **RL research in mid-2026 ≠ the 2025 RLVR era.** Vanilla RLVR could hillclimb
  math on base models cheaply. Agentic terminal tasks are *hard*: complex
  tool-use, self-managed history, heavy infra, small eval gains per unit
  training. The field is shifting "from a renaissance of RL study to rapidly
  needing to improve empirical rigor."
- **"Recipe work" is the need.** A recipe paper documents *all* the steps for a
  crucial model improvement — data, algorithm, codebase, pitfalls — so others can
  study small ablations against an established, stable baseline rather than
  spending weeks and \$10K–\$1M+ just to get a baseline to move. TMAX is meant to
  be that baseline for terminal agents (akin to Olmo 3's "RL Zero" families).
- **Cost reality.** A standard TMAX job is **8× H100 nodes (2 train / 6 inference)
  for 2–3 days**; establishing the recipe took **O(100)** such jobs — expensive,
  but approaching what academics can study.
- **Algorithm note.** TMAX uses **DPPO** (a GRPO variant that masks tokens when
  inference/training logprobs diverge, via a binary total-variation threshold) +
  an **FP32 LM head** — most innovations are aimed at *stability*, not raw
  learning rate. Frameworks in vogue: SLIME, SkyRL (TMAX itself forks
  open-instruct).

## One-paragraph summary

TMAX generates a large (≈15K), diverse, difficulty-aware corpus of containerized
terminal tasks with programmatic verifiers by sampling tasks as an independent
product of structured axes, then trains small dense Qwen 3.5 models (2B–27B) with
a simple outcome-only DPPO recipe. TMAX-9B reaches ~27% on Terminal-Bench 2.0 —
state-of-the-art among open-data recipes and Pareto-dominant under 32B — and the
gains generalize to SWE-Bench and AIME. The contribution is as much the *recipe*
(data pipeline, DPPO + FP32-head stability, documented pitfalls incl.
reward-hacking) as the numbers.
