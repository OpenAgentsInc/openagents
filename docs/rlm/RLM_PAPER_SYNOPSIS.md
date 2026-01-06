# Recursive Language Models (RLMs) - Comprehensive Synopsis

**Paper:** arXiv:2512.24601v1 [cs.AI] 31 Dec 2025
**Authors:** Alex L. Zhang, Tim Kraska, Omar Khattab (MIT CSAIL)
**Source:** `docs/frlm/2512.24601v1.pdf`

---

## Executive Summary

Recursive Language Models (RLMs) are a general-purpose inference paradigm that allows LLMs to process arbitrarily long prompts by treating the input as part of an **external environment** rather than feeding it directly into the neural network. The LLM loads the prompt as a variable in a Python REPL and can programmatically examine, decompose, and recursively call itself over snippets. RLMs handle inputs **up to two orders of magnitude beyond model context windows** while maintaining comparable or lower costs. (p1-2)

---

## 1. Core Problem: Context Rot

Modern LLMs exhibit **context rot** - quality degrades as context length increases, even within stated context limits. This is illustrated in Figure 1 (p1) where GPT-5 performance degrades significantly as input length grows from 8K to 1M tokens, particularly on complex tasks.

**Key insight:** The effective context window cannot be understood independently of the specific task. More complex problems exhibit degradation at shorter lengths than simpler ones. (p3)

---

## 2. The RLM Architecture

### 2.1 Core Mechanism (p2, Figure 2)

RLMs expose the **same external interface as an LLM** (accepts string prompt, produces string response) but internally:

1. **Initialize REPL Environment:** Given prompt P, create a Python REPL where P is stored as a variable
2. **Provide Context:** Tell the LLM about the environment (e.g., string length of P)
3. **Code Execution:** LLM writes code to peek into and decompose P
4. **Recursive Calls:** LLM can programmatically construct sub-tasks and invoke itself recursively via `llm_query()` function
5. **Iterative Refinement:** Observe execution side effects and continue until final answer

### 2.2 Key Differentiator

Unlike prior approaches (Anthropic subagents, THREAD, Context Folding) that focus on recursive task decomposition but cannot scale inputs beyond context windows, RLMs treat the **prompt itself as an external object** that can be symbolically manipulated. (p2)

### 2.3 Implementation Details (p4, Appendix D p24-28)

- **Root LM:** GPT-5 or Qwen3-Coder-480B-A35B
- **Sub-LM (for GPT-5):** GPT-5-mini for recursive calls (cost/capability tradeoff)
- **Max recursion depth:** 1 (sub-calls are LMs, not RLMs)
- **Final answer format:** `FINAL(answer)` or `FINAL_VAR(variable_name)`

---

## 3. Benchmarks and Task Characterization

Tasks are characterized by **information density** - how much information must be processed and how this scales with input size. (p3)

### 3.1 S-NIAH (Single Needle-in-a-Haystack)

- **Source:** Based on RULER (Hsieh et al., 2024)
- **Task:** Find specific phrase/number in large unrelated text
- **Complexity:** O(1) - constant processing cost regardless of input length
- **Size:** 50 tasks
- **Context scaling:** 2^13 to 2^18 tokens

### 3.2 BrowseComp-Plus (1K documents)

- **Source:** Chen et al., 2025
- **Task:** Multi-hop QA for DeepResearch questions over 1000 documents
- **Context:** 6M-11M tokens per task
- **Evaluation:** 150 randomly sampled tasks
- **Metric:** Percentage correct answers
- **Complexity:** Constant (requires fixed number of evidence documents)
- **Note:** Gold and evidence documents guaranteed to exist in corpus

### 3.3 OOLONG (trec_coarse split)

- **Source:** Bertsch et al., 2025
- **Task:** Examine and transform chunks semantically, then aggregate for final answer
- **Size:** 50 tasks over question dataset with semantic labels
- **Complexity:** O(N) - linear, requires nearly all entries
- **Context:** 131K tokens
- **Scoring:** `score(ŷ) = 0.75^|y-ŷ|` for numerical, exact match for others

### 3.4 OOLONG-Pairs (Novel benchmark created for this paper)

- **Task:** Modified trec_coarse requiring aggregation over **pairs** of chunks
- **Size:** 20 new queries (full query list in Appendix E.1, p29-32)
- **Complexity:** O(N²) - quadratic, requires nearly all pairs
- **Context:** 32K tokens
- **Metric:** F1 score
- **Purpose:** Tests extremely information-dense scenarios where base models fail catastrophically

### 3.5 LongBench-v2 CodeQA

- **Source:** Bai et al., 2025
- **Task:** Multi-choice code repository understanding
- **Context:** 23K-4.2M tokens
- **Metric:** Percentage correct answers

---

## 4. Baselines and Methods (p4)

### 4.1 Base Model
Direct LLM call without any scaffolding.

### 4.2 RLM with REPL
Full implementation with context as variable + recursive `llm_query()` calls.

### 4.3 RLM with REPL (no sub-calls)
Ablation: REPL environment but no recursive LM calls. Tests importance of recursion vs. just code execution.

### 4.4 Summary Agent
Following Sun et al., Wu et al., Yu et al. (2025):
- Iteratively views documents and summarizes when context fills
- For contexts exceeding window, chunks input first
- GPT-5 variant uses GPT-5-nano for summarization (cost management)

### 4.5 CodeAct (+ BM25)
- ReAct loop with code execution (Wang et al., 2024)
- Does NOT offload prompt to code environment (key difference from RLM)
- BM25 retriever for BrowseComp+ (following Jimenez et al., 2024)

---

## 5. Main Results (Table 1, p4)

### 5.1 Performance Table

| Model | CodeQA | BrowseComp+ (1K) | OOLONG | OOLONG-Pairs |
|-------|--------|------------------|--------|--------------|
| **Task Length** | 23K-4.2M | 6M-11M | 131K | 32K |
| **Qwen3-Coder-480B** |||||
| Base Model | 20.00* | 0.00* | 36.00 | 0.06 |
| CodeAct + BM25 | 24.00* | 12.66 | 38.00 | 0.28 |
| Summary agent | 50.00 | 38.00 | 44.06 | 0.31 |
| RLM | 56.00 | 44.66 | 48.00 | **23.11** |
| RLM (no sub-calls) | **66.00** | **46.00** | 43.50 | 17.34 |
| **GPT-5** |||||
| Base Model | 24.00* | 0.00* | 44.00 | 0.04 |
| CodeAct + BM25 | 22.00* | 51.00 | 38.00 | 24.67 |
| Summary agent | 58.00 | 70.47 | 46.00 | 0.01 |
| RLM | **62.00** | **91.33** | **56.50** | **58.00** |
| RLM (no sub-calls) | 58.00 | 88.00 | 36.00 | 43.93 |

*\* indicates runs hitting input context limits*

### 5.2 Cost Analysis (in USD, ± std dev)

| Model/Method | CodeQA | BrowseComp+ | OOLONG | OOLONG-Pairs |
|--------------|--------|-------------|--------|--------------|
| GPT-5 Base | $0.13±0.07 | N/A | $0.14±0.02 | $0.16±0.10 |
| GPT-5 RLM | $0.11±0.10 | $0.99±1.22 | $0.43±0.85 | $0.33±0.20 |
| GPT-5 Summary | $1.31±1.46 | $0.57±0.10 | $0.13±0.01 | $0.13±0.09 |

**Key finding:** RLM median cost is often **cheaper** than base model, but has high variance due to trajectory length differences. (p6, Figure 3)

---

## 6. Key Observations (p5-6)

### Observation 1: RLMs Scale to 10M+ Tokens
- Outperform base models and scaffolds by up to **2x performance**
- On BrowseComp+ (1K): RLM(GPT-5) achieves **91.33%** vs 70.47% for summary agent
- Cost of GPT-5-mini ingesting 6-11M tokens would be $1.50-$2.75; RLM averages $0.99

### Observation 2: REPL is Necessary; Recursion Helps Dense Tasks
- Even without sub-calls, RLM ablation scales beyond context limits
- On CodeQA/BrowseComp+ with Qwen3-Coder, no-sub-calls ablation outperforms full RLM by 17.9%/3%
- On information-dense tasks (OOLONG, OOLONG-Pairs), recursion provides **10-59% improvement**

### Observation 3: Performance Degrades with Length and Complexity, but RLMs Degrade Slower
- GPT-5 degrades significantly faster on complex tasks (Figure 1)
- RLM performance degrades at **much slower rate**
- For contexts beyond 2^14 tokens, RLM **consistently outperforms** base GPT-5
- Tradeoff: Base LM slightly outperforms RLM at very short contexts

### Observation 4: Comparable Cost, High Variance
- Median RLM run cheaper than median base model run
- Outlier RLM runs significantly more expensive
- Compared to summarization baseline: RLMs up to **3x cheaper** while maintaining stronger performance

### Observation 5: Model-Agnostic but Model-Specific Behavior
- Same system prompt used across all experiments per model
- GPT-5 and Qwen3-Coder exhibit different decomposition strategies
- RLM(GPT-5) nearly solves BrowseComp+ (91.33%) while RLM(Qwen3-Coder) struggles (44.66%)

---

## 7. Emergent Patterns in RLM Trajectories (p6-7, Figure 4)

### 7.1 Filtering with Code + Model Priors
RLMs filter context without explicitly seeing it using model priors:
```python
# Example: Using regex to search for keywords based on priors
keywords = ["dinengdeng", "pinakbet", "bagong", "festival", "La Union"]
results = find_snippets(keyword, window=400, max_hits=5)
```
Common strategy: Probe context by printing a few lines, then filter based on observations.

### 7.2 Chunking and Recursive Sub-calling
- Defer unbounded-length reasoning to sub-RLM calls
- Typically uniform chunking or keyword-based partitioning
- Example: Qwen3-Coder chunks 1000+ lines by newline for OOLONG

### 7.3 Answer Verification via Sub-LM Calls
- Use sub-LMs to verify answers (avoids context rot in root LM)
- Can be redundant and expensive (observed cases of 5+ verification attempts)

### 7.4 Long Output via Variable Stitching
- Return REPL variables as output for unbounded token generation
- Iteratively construct variables mixing programmatic and sub-LM outputs
- Heavy usage in OOLONG-Pairs trajectories

---

## 8. Detailed Trajectory Examples (Appendix B, p13-21)

### 8.1 RLM(GPT-5) on BrowseComp-Plus Query 74 (p13-15)
- **Cost:** $0.079
- **Task:** Multi-hop query about a festival, stew, and beauty pageant winner
- **Process:**
  1. Probe 1000 documents with regex for keywords ("festival", "La Union", etc.)
  2. Find interesting snippet at index 6
  3. Launch sub-LM call to extract relevant info → finds "Maria Dalmacio"
  4. Verify answer with 2 additional sub-LM calls
  5. Return correct answer

### 8.2 RLM(Qwen3-Coder) on OOLONG-Pairs Query 3 (p15-19)
- **Cost:** $1.12
- **Task:** List all user ID pairs where both have specific instance types
- **Process:**
  1. Probe context structure (multiple code blocks in single step)
  2. Classify entries via sub-LM calls in batches of 100
  3. Build answer programmatically
  4. **Problem:** Model repeatedly verifies answer 5+ times, eventually returns wrong answer from root LM generation instead of stored variable

### 8.3 RLM(Qwen3-Coder) on OOLONG Query 212 (p19-20)
- **Cost:** $0.38
- **Task:** Compare frequency of two semantic labels
- **Key difference:** Qwen3-Coder makes **one sub-LM call per line** (thousands of calls)
- GPT-5 is more conservative about sub-querying

### 8.4 RLM(GPT-5) on CodeQA Query 44 (p21)
- **Cost:** $0.27
- **Task:** Answer question about 900K token codebase
- **Strategy:** Partition codebase, sub-query each chunk for clues, aggregate via final sub-query
- Returns correct answer (choice 1)

---

## 9. Scaling Analysis (p22-23, Appendix C)

### 9.1 Runtime Quartiles (Figures 5-6)
- RLMs show high variance in runtime
- Can be improved via async LM calls and better prompting
- All implementations used blocking/sequential calls

### 9.2 Cost Distributions (Figures 7-8)
- Long-tailed, high-variance cost distributions for RLMs
- Mean costs generally reasonable but outliers exist

### 9.3 API Cost vs Input Length (Figure 9)
- S-NIAH (O(1)): Relatively flat cost scaling
- OOLONG (O(N)): Linear cost increase
- OOLONG-Pairs (O(N²)): Steeper cost increase
- RLM costs scale proportionally to task complexity

---

## 10. BrowseComp+ Scaling Study (Appendix E.2, p32-33, Figure 10)

Testing on subset of 20 tasks with varying document counts (10 to 1000):

### Performance Scaling
- **RLM(GPT-5):** Only method achieving **100% at 1000 documents**
- **RLM (no sub-calls):** ~90% at 1000 documents
- **Base GPT-5 approaches:** Clear performance dropoff as documents increase
- **GPT-5 + pre-query BM25:** Degrades significantly

### Cost Scaling
- RLM inference cost scales **log-linearly**
- Cheaper than extrapolated cost of GPT-5 with infinite context window

---

## 11. System Prompts (Appendix D, p24-28)

### 11.1 RLM System Prompt Key Elements
- Context loaded as variable with metadata (type, length, chunk lengths)
- `llm_query()` function for recursive calls (~500K char capacity)
- `print()` for observing REPL output
- Examples of chunking strategies provided
- Final answer via `FINAL()` or `FINAL_VAR()`

### 11.2 Model-Specific Adjustments
**Qwen3-Coder addition:**
> "IMPORTANT: Be very careful about using 'llm_query' as it incurs high runtime costs. Always batch as much information as reasonably possible into each call (aim for ~200k characters per call)."

Without this, Qwen3-Coder makes thousands of sub-calls for basic tasks.

### 11.3 CodeAct Prompt
- THINK/ACT format
- Python code execution in ```python blocks
- SEARCH(query) for BM25 retrieval (BrowseComp+ only)
- Variables persist across executions

### 11.4 Summary Agent
- Follows Sun et al., Wu et al., Yu et al. (2025) scaffold
- Iterative summarization when context fills
- For GPT-5: Uses GPT-5-nano for summarization (20x cheaper than Qwen3-Coder approach)

---

## 12. Negative Results (Appendix A, p13)

### What Didn't Work

1. **Same prompt across models:** GPT-5 prompt caused undesirable behavior in Qwen3-Coder without modification

2. **Models without coding capabilities:** Smaller models like Qwen3-8B struggled without sufficient coding abilities

3. **Thinking models with limited output tokens:** Qwen3-235B-A22B showed smaller improvements because thinking tokens exceeded max output length

4. **Synchronous LM calls:** Made experiments slow; async implementation would help

5. **Distinguishing final answer from thought:** Using `FINAL()` tags is brittle; model sometimes outputs plans as final answers

---

## 13. Related Work Context (p7-8)

### Long Context LM Systems
- **Architecture changes:** ALiBi (Press et al., 2022), S4 (Gu et al., 2022), Infini-attention (Munkhdalai et al., 2024)
- **Lossy context management:** MemWalker (Chen et al., 2023), ReSum (Wu et al., 2025)
- **Memory hierarchy:** MemGPT (Packer et al., 2024), Mem0 (Chhikara et al., 2025)
- **RLM difference:** All context management is implicit, handled by LM itself

### Task Decomposition via Sub-LM Calls
- **Human-engineered workflows:** Multi-agent systems (Guo et al., 2024)
- **LM-chosen decomposition:** ViperGPT (Surís et al., 2023), THREAD (Schroeder et al., 2025), DisCIPL (Grand et al., 2025), ReDel (Zhu et al., 2024), Context Folding (Sun et al., 2025), AgentFold (Ye et al., 2025)
- **RLM difference:** Can handle inputs beyond base LM context window via symbolic manipulation

---

## 14. Limitations and Future Work (p8)

### Current Limitations
1. **Synchronous sub-calls:** Async + sandboxed REPLs could reduce runtime/cost
2. **Shallow recursion:** Only depth 1 tested; deeper recursion unexplored
3. **No explicit training:** Models not trained to be RLMs

### Future Directions
1. **Async sub-calls and sandboxed REPLs** for efficiency
2. **Deeper recursion layers** for more complex tasks
3. **Explicit RLM training:**
   - Train models specifically as root or sub-LMs
   - RLM trajectories as form of reasoning (like o1/DeepSeek-R1)
   - Bootstrap via STaR (Zelikman et al., 2022, 2024)

---

## 15. OOLONG-Pairs Benchmark Details (Appendix E.1, p29-32)

20 synthetically generated tasks requiring pair-wise aggregation. All tasks follow pattern:

> "List all pairs of user IDs (no duplicate pairs, list lower ID first) where [CONDITION]. In your answer, list all pairs in format (user_id_1, user_id_2), separated by newlines."

### Task Categories
- **Tasks 1-3:** Both users have instances of specific label combinations
- **Tasks 4-10:** Label combinations with date constraints
- **Tasks 11-20:** Asymmetric conditions (one user has X, other has Y)

### Design Rationale
Many pair-aggregation tasks can be solved via inclusion-exclusion without examining pairs. These tasks explicitly require listing all pairs to ensure O(N²) complexity.

---

## 16. Key Takeaways for Implementation

### When to Use RLMs
- Contexts significantly beyond model window
- Information-dense tasks requiring processing of most/all content
- Tasks where summarization would lose critical details

### When Base LM May Suffice
- Short contexts well within model window
- Simple retrieval tasks (needle-in-haystack)
- Tasks where summarization preserves needed information

### Implementation Considerations
1. **Use smaller model for sub-calls** (e.g., GPT-5-mini with GPT-5 root)
2. **Add batching guidance for aggressive models** (Qwen3-Coder needs explicit instruction)
3. **Expect high variance in cost/runtime** - outliers can be expensive
4. **Consider task complexity** - costs scale with information density

### Cost-Performance Tradeoffs
- Median RLM cost often cheaper than base model
- 95th percentile costs can be 3-4x higher
- Summary agents cheaper for simple tasks but lose on complex ones

---

## Appendix: Quick Reference

### Models Tested
- GPT-5 (medium reasoning) + GPT-5-mini for sub-calls
- Qwen3-Coder-480B-A35B (Fireworks provider)

### Context Windows
- GPT-5: 272K tokens
- Tasks tested: 8K to 11M tokens

### Key Performance Numbers (GPT-5 RLM)
- BrowseComp+ (1K docs, 6-11M tokens): **91.33%** @ $0.99/query
- OOLONG-Pairs (32K tokens): **58.00%** (vs 0.04% base)
- CodeQA (23K-4.2M tokens): **62.00%**
- OOLONG (131K tokens): **56.50%**

### Repository
Code and trajectories available in paper's codebase (referenced but URL not provided in paper).

---

*Synopsis generated from arXiv:2512.24601v1, 33 pages including appendices.*
