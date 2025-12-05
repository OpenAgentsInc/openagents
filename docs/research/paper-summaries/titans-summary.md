# Titans: Surprise-Aware Multi-Timescale Memory for Agents

**Paper Summary**

- **Authors:** Not publicly confirmed (referenced as Google Research work, 2023)
- **Institutions:** Google Research (as cited in related-work-reflection)
- **Published:** 2023 (citation inferred from secondary sources)
- **Source:** Not publicly available; summary based on secondary references (Nested Learning / HOPE discussions and related-work-reflection notes)

---

## Executive Summary

Titans is referenced as a multi-timescale memory architecture that explicitly separates fast episodic memory from slower semantic memory while adding a surprise/importance gate to decide what should be retained long-term. The design aims to mitigate catastrophic forgetting by retaining only “surprising or important” experiences and using them to prime later reasoning. In benchmarks cited alongside HOPE/Nested Learning, Titans serves as a strong memory baseline for ultra-long-context tasks.

---

## Core Ideas

- **Two-speed memory:** Combines a rapid, short-term store with a slower consolidated buffer so that rare/important events can persist beyond immediate context.
- **Surprise-driven retention:** Experiences with high surprise/importance scores are written into long-term memory; routine events decay quickly.
- **Self-modifying memory reads:** Memory lookups influence subsequent memory parameters (reported in HOPE discussions as “self-referential Titans”), effectively adapting retrieval keys/values over time.
- **Explicit long-context handling:** Evaluated on long-context benchmarks (e.g., Needle-in-a-Haystack variants), where memory-aware models retain performance while standard Transformers degrade.

---

## Reported Results (from secondary sources)

- On long-context probes, Titans maintains near-perfect recall on simple NIAH tasks (100% on short-span setups) but lags HOPE on multi-query variants, suggesting memory helps but retrieval/composition still bottleneck performance.
- In language modeling comparisons, Titans improves perplexity and logical accuracy over a Transformer++ baseline, indicating better retention of older tokens.
- Serves as a competitive baseline in continual-learning setups; surprise gating reduces forgetting relative to plain replay.

*These numbers are drawn from summaries in `nested-learning-summary.md`; primary paper was not publicly available during this review.*

---

## Relevance to MechaCoder

- **Surprise-weighted memory writes:** Incorporate “surprise” (unexpected failures or rare edge cases) into Archivist scoring so unusual events are retained longer than routine successes.
- **Multi-timescale stores:** Maintain a fast session cache plus a slower consolidated store; promote entries based on surprise/importance rather than sheer recency.
- **Self-modifying retrieval:** Allow reflection outputs to adjust retrieval keys/weights for subsequent tool calls (e.g., bias toward files/commands implicated in surprising failures).

---

## Open Questions

- No public PDF/DOI found; confirm official title/authors and release a proper citation.
- Clarify how surprise is quantified (prediction error vs. entropy vs. task-specific heuristic).
- Determine whether memory writes are differentiable or rule-based, and how they interact with downstream planners.
