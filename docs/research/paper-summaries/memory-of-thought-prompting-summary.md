# MoT: Memory-of-Thought Enables ChatGPT to Self-Improve

**Paper Summary**

- **Authors:** Xiaonan Li, Xipeng Qiu
- **Institutions:** School of Computer Science, Fudan University; Shanghai Key Laboratory of Intelligent Information Processing
- **Published:** arXiv:2305.05181v2, October 2023
- **Source:** https://arxiv.org/abs/2305.05181

---

## Executive Summary

MoT adds a lightweight “memory-of-thought” layer to ChatGPT: before testing, the model generates chain-of-thought solutions for an unlabeled question set, filters for high-confidence traces, and stores them as external memory. At test time, it retrieves relevant thoughts to guide reasoning without any parameter updates or labeled data. Across arithmetic, commonsense, factual QA, and NLI benchmarks, this approach raises ChatGPT’s average score by ~9 points over zero-shot CoT and ~4 points over few-shot CoT, showing that cached reasoning traces can substitute for supervised demonstrations.

---

## Core Contribution

MoT demonstrates that **self-generated, high-confidence reasoning traces can act as reusable supervision**, enabling in-context performance gains without annotation or fine-tuning. The key idea is to pre-compute a library of validated thoughts and retrieve them as targeted demonstrations during inference.

---

## Architecture / Methodology

1) **Pre-thinking on unlabeled data**
- Run multi-path CoT on each question; compute answer entropy to estimate confidence.
- Keep only low-entropy (high-confidence) thoughts; store {question, rationale, answer} triples as memory entries.

2) **Memory store and retrieval**
- Memories are retrievable by BM25/MPNet/LLM-based similarity; LLM-based retrieval performs best.
- Optional filtering removes noisy thoughts; ablations show quality gating is critical.

3) **Recall-time prompting**
- Given a test query, retrieve top-k memories and insert them as demonstrations plus a “Let’s think step by step” prompt.
- Variants: (a) use retrieved thoughts but skip new reasoning (“no thinking”), or (b) use retrieved answers without rationales (“no rationale”); both still beat zero-shot CoT on several datasets.

---

## Key Results

- **Overall gains:** On ChatGPT (GPT-3.5-Turbo), MoT averages **+3.7 points over few-shot CoT** and **+9.1 over zero-shot CoT** across AQuA, DROP, ANLI (A1–A3), OBQA, BoolQ, FactCK, and WikiQA.
- **Retrieval quality matters:** LLM-based retrieval outperforms random/BM25/MPNet; poor retrieval erodes gains.
- **Filtering is essential:** Removing low-confidence thoughts prevents accuracy drops and even surpasses few-shot CoT on harder sets like ANLI-A3 and OBQA.

---

## Limitations / Considerations

- Requires an unlabeled, in-domain question set for pre-thinking; domain shift may reduce memory utility.
- Memory size and retrieval latency grow with coverage; needs pruning or clustering in longer runs.
- Quality hinges on confidence estimation; mis-filtered thoughts can hurt performance.
- Evaluated mainly with ChatGPT; transfer to smaller or open models may need retuning of filters/retrieval.

---

## Relevance to MechaCoder

- Treat **passing test runs and solid debugging traces as “thoughts”**: cache high-confidence reasoning steps (what failed, why, how it was fixed) and retrieve them when similar errors appear in new tasks.
- Add a **pre-think phase** for new repos: run lightweight dry-runs or static checks to collect reusable traces before the first task.
- Use **confidence filters** (e.g., tests passed, low flake rate) to keep only trustworthy traces in the skill/memory store.
- Prefer **LLM-aware retrieval** (semantic similarity over logs + task text) to surface the most relevant past fixes during retries or recovery.

---

## Citation

```bibtex
@article{li2023mot,
  title={MoT: Memory-of-Thought Enables ChatGPT to Self-Improve},
  author={Li, Xiaonan and Qiu, Xipeng},
  journal={arXiv preprint arXiv:2305.05181},
  year={2023},
  url={https://arxiv.org/abs/2305.05181}
}
```
