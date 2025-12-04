# Reflexion: Language Agents with Verbal Reinforcement Learning

**Paper Summary**

- **Authors:** Noah Shinn, Federico Cassano, Edward Berman, Ashwin Gopinath, Karthik Narasimhan, Shunyu Yao
- **Institutions:** Northeastern University, MIT, Princeton University
- **Published:** NeurIPS 2023
- **arXiv:** 2303.11366
- **Source:** https://arxiv.org/abs/2303.11366
- **Code:** https://github.com/noahshinn/reflexion

---

## Executive Summary

Reflexion introduces a novel framework for reinforcing language agents through linguistic feedback rather than traditional weight updates. The key insight is that agents can improve their performance by generating natural language self-reflections on failed attempts and storing these in an episodic memory buffer for future reference, achieving state-of-the-art results on coding benchmarks (91% pass@1 on HumanEval vs. GPT-4's 80%).

---

## Core Contribution

The paper's fundamental innovation is **verbal reinforcement learning** — replacing numerical reward signals and gradient updates with natural language feedback and reflection. Instead of learning through weight updates, agents learn by analyzing their failures in natural language and using those reflections to guide subsequent attempts. This approach is more interpretable, requires no model fine-tuning, and can incorporate diverse feedback sources (external evaluators, self-generated critiques, or scalar rewards translated to language).

---

## Architecture / Methodology

```
┌─────────────────────────────────────────────────────────────────┐
│                      REFLEXION LOOP                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │   Task   │───▶│  Actor   │───▶│Evaluator │───▶│  Result  │  │
│  │  Input   │    │  Agent   │    │(External)│    │ Success? │  │
│  └──────────┘    └──────────┘    └──────────┘    └────┬─────┘  │
│                        ▲                               │        │
│                        │                               ▼        │
│                        │                          ┌─────────┐   │
│                        │                          │   NO    │   │
│                        │                          └────┬────┘   │
│                        │                               │        │
│  ┌──────────────────────────────────────────────────────┐      │
│  │              Self-Reflection Module                   │      │
│  │  ┌─────────────────────────────────────────────────┐ │      │
│  │  │ "I failed because I didn't check the edge case  │ │      │
│  │  │  where the input list is empty. Next time I     │ │      │
│  │  │  should add a guard clause at the start."       │ │      │
│  │  └─────────────────────────────────────────────────┘ │      │
│  └───────────────────────┬──────────────────────────────┘      │
│                          │                                      │
│                          ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Episodic Memory Buffer                      │   │
│  │  [Reflection 1] [Reflection 2] ... [Reflection N]       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Reflexion Strategies

The framework supports four distinct memory strategies:

| Strategy | Description |
|----------|-------------|
| `NONE` | Baseline: no information about previous attempts |
| `LAST_ATTEMPT` | Only the previous reasoning trace is provided |
| `REFLEXION` | Only the self-generated reflection is provided |
| `LAST_ATTEMPT_AND_REFLEXION` | Both trace and reflection are provided (best performance) |

### Key Components

1. **Actor**: The LLM agent that generates actions/solutions (ReAct, CoT, etc.)
2. **Evaluator**: External or internal system that provides feedback (binary, scalar, or natural language)
3. **Self-Reflection**: A separate LLM call that analyzes failure and generates actionable insights
4. **Memory**: Sliding window of previous reflections (typically last 1-3)

### Feedback Types Supported

- **Binary**: Success/failure signals
- **Scalar**: Numerical scores
- **Natural Language**: Free-form textual feedback from external systems
- **Self-Simulated**: Agent generates its own hypothetical feedback

---

## Key Results

| Benchmark | Reflexion | Previous SOTA | Improvement |
|-----------|-----------|---------------|-------------|
| HumanEval (pass@1) | **91%** | 80% (GPT-4) | +11% |
| HotPotQA | Significant gains | - | Multi-hop QA |
| AlfWorld | 97% | 75% (ReAct) | +22% |
| WebShop | Improved | - | Interactive web tasks |

### Ablation Insights

- Self-reflection alone provides substantial gains over no reflection
- Combining trace + reflection (`LAST_ATTEMPT_AND_REFLEXION`) yields best results
- Benefits persist across different base LLM models
- Reflection quality matters: more specific reflections lead to better recovery

---

## Limitations and Future Work

- **Bounded Memory**: Episodic buffer has limited capacity; older reflections are discarded
- **Reflection Quality**: Dependent on LLM's ability to generate useful self-critique
- **No Persistent Learning**: Reflections don't transfer across tasks/sessions
- **Compute Cost**: Multiple LLM calls per retry (action + reflection)
- **Task-Specific**: Works best on tasks with clear success/failure signals
- **Hallucinated Reflections**: Agent may generate incorrect self-diagnoses

---

## Relevance to MechaCoder

This paper is **highly relevant** to MechaCoder's autonomous coding capabilities.

### Applicable Patterns

1. **Retry-with-Reflection Loop**: When tests fail, generate a natural language analysis of *why* before retrying
   ```
   Test failed → Analyze failure → Generate reflection → Retry with reflection in context
   ```

2. **Structured Failure Analysis**: Instead of just retrying with the error message, have the agent explicitly reflect:
   - "What was I trying to do?"
   - "What went wrong?"
   - "What should I do differently?"

3. **Episodic Memory for Multi-Task Sessions**: Store reflections from earlier tasks to inform later ones
   - "Earlier I learned that this codebase uses Effect for error handling, not try/catch"

### Implementation Considerations

1. **Add a Reflection Phase to Golden Loop v2**:
   ```
   Understand → Implement → Test → [FAIL] → Reflect → Implement (with reflection) → Test
   ```

2. **Reflection Prompt Template**:
   ```
   You attempted: {action}
   Result: {error_or_failure}

   Analyze what went wrong and provide a specific, actionable reflection
   for how to fix this on the next attempt.
   ```

3. **Memory Management**: Keep last 2-3 reflections in sliding window (paper shows diminishing returns beyond this)

4. **Selective Application**: Only trigger reflection on meaningful failures, not syntax errors

### Potential Value

| Problem | Reflexion Solution |
|---------|-------------------|
| Agent makes same mistake repeatedly | Explicit reflection prevents repetition |
| Context window pollution with error logs | Concise reflection summarizes key insight |
| No learning across attempts | Memory buffer persists actionable lessons |
| Debugging is trial-and-error | Structured analysis improves fix accuracy |

### Integration Points

- **`src/agent/orchestrator/`**: Add reflection step after failed test runs
- **`src/healer/`**: Use reflections to improve self-healing prompts
- **Progress tracking**: Store reflections alongside task attempts for post-mortem analysis

---

## Related Work

- **ReAct** (Yao et al., 2022): Reasoning + Acting framework that Reflexion builds upon
- **Chain-of-Thought** (Wei et al., 2022): Reasoning traces that Reflexion extends
- **Self-Refine** (Madaan et al., 2023): Similar self-improvement but without episodic memory

---

## Citation

```bibtex
@inproceedings{shinn2023reflexion,
  title={Reflexion: Language Agents with Verbal Reinforcement Learning},
  author={Shinn, Noah and Cassano, Federico and Berman, Edward and Gopinath, Ashwin and Narasimhan, Karthik and Yao, Shunyu},
  booktitle={Advances in Neural Information Processing Systems},
  volume={36},
  year={2023}
}
```
