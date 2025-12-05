# R2-MGA: Retrieval and Reflection Memory-Augmented Generative Agent

**Paper Summary**

- **Authors:** Bin Ji, Huijun Liu, Mingzhe Du, Shasha Li, Xiaodong Liu, Jun Ma, Jie Yu, See-Kiong Ng
- **Institutions:** National University of Defense Technology; Nanyang Technological University; National University of Singapore
- **Published:** AAAI 2025 (posted April 11, 2025)
- **Source:** https://chatpaper.com/paper/161990

---

## Executive Summary

R2-MGA tackles citation-grounded text generation by pairing retrieval with reflective memory. It builds a memory bank of cited snippets, retrieves the best match for a new query, reflects on that snippet to form an explicit rationale, and uses the combined snippet+rationale as an in-context demonstration. Two refinement modules further polish answers and citations. On the ALCE benchmark across five LLMs, R2-MGA boosts answer correctness by up to **+58.8%** and citation quality by **+154.7%** versus baselines, showing that lightweight memory + reflection can outperform one-shot prompting for verifiable generation.

---

## Core Contribution

R2-MGA shows that **retrieval plus reflective reasoning over prior cited snippets** can produce more verifiable, citation-backed outputs than direct prompting. The system treats retrieved evidence as a seed, transforms it into an explicit rationale, and reuses that rationale as the best-fit demonstration for the current query.

---

## Architecture / Methodology

1) **Memory bank creation:** Store snippet-level memories with citations to serve as demonstrations.  
2) **Retrieve + reflect:** For a new query, retrieve the best memory snippet, then generate a reflection that explains how it answers the query.  
3) **Demonstration construction:** Concatenate snippet + reflection into an in-context example fed to the LLM.  
4) **Answer refinement:** Apply two refinement modules (one focused on answer quality, one on citation grounding) to polish the draft output and links.  
5) **Evaluation:** Tested on ALCE with five different LLMs to measure answer accuracy and citation quality.

---

## Key Results

- Across five LLMs on ALCE, R2-MGA delivers **up to +58.8%** relative gains in answer correctness and **+154.7%** gains in citation quality over selected baselines.  
- Both retrieval quality and the reflection step are necessary; one-step prompting underperforms the full pipeline.  
- Refinement modules further improve citation grounding beyond the initial demonstration-driven draft.

---

## Limitations / Considerations

- Depends on a high-quality memory bank; missing or noisy snippets can misguide reflection and demonstrations.  
- Evaluated primarily on ALCE; generalization to other domains/tasks is unverified.  
- Retrieval/reranking overhead may add latency, and benefits may diminish if queries lack close matches in memory.  
- Details of the refinement modules are dataset-specific; portability may require re-tuning.

---

## Relevance to MechaCoder

- Store **high-quality, cited reasoning traces** (e.g., test failures with fixes) in a memory bank and retrieve them as demonstrations for similar errors.  
- Add a **reflection step over retrieved traces** to surface why a past fix applies before drafting a new patch.  
- Use **refinement passes** to polish outputs: one pass for correctness (tests/typecheck hints), another for “citation” (linking to file/commit evidence).  
- Emphasize **verifiability** by attaching links to source code or commit hashes in generated explanations.

---

## Citation

```bibtex
@inproceedings{ji2025r2mga,
  title={Towards Verifiable Text Generation with Generative Agent},
  author={Ji, Bin and Liu, Huijun and Du, Mingzhe and Li, Shasha and Liu, Xiaodong and Ma, Jun and Yu, Jie and Ng, See-Kiong},
  booktitle={Proceedings of the AAAI Conference on Artificial Intelligence},
  year={2025},
  url={https://chatpaper.com/paper/161990}
}
```
