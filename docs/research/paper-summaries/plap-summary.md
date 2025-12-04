# Empowering LLMs with Parameterized Skills for Adversarial Long-Horizon Planning (PLAP)

**Paper Summary**

- **Authors:** Sijia Cui, Shuai Xu, Aiyao He, Yanna Wang, Bo Xu
- **Institutions:** Not explicitly listed (Chinese research team based on author names)
- **Published:** IJCNN 2025
- **arXiv:** 2509.13127
- **Source:** https://arxiv.org/abs/2509.13127
- **Code:** https://github.com/AI-Research-TeamX/PLAP

---

## Executive Summary

PLAP ("Plan with Language, Act with Parameter") introduces parameterized skills as an intermediate abstraction layer between high-level LLM planning and low-level action execution. This approach enables LLMs to generate strategic plans without needing to understand atomic action sequences, achieving state-of-the-art performance in long-horizon adversarial planning within the MicroRTS environment.

---

## Core Contribution

PLAP addresses the gap where LLMs struggle to translate high-level strategic intent into coordinated low-level actions. Rather than having the LLM output raw actions (which is error-prone and requires extensive domain knowledge), PLAP exposes a **parameterized skill interface** where the LLM only needs to output skill names and parameters. A specialized executor then converts these parameterized plans into executable action sequences. This separation allows LLMs to focus on strategic reasoning while leaving action coordination to deterministic skill implementations.

---

## Architecture / Methodology

```
┌─────────────────────────────────────────────────────────────────┐
│                     PLAP Framework                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐     ┌──────────────────┐                  │
│  │  Skill Planner   │────▶│  Skill Executor  │                  │
│  │  (LLM-powered)   │     │                  │                  │
│  │                  │     │  • Iterative     │                  │
│  │  Outputs:        │     │  • State-aware   │                  │
│  │  - Skill name    │     │  • Terminates    │                  │
│  │  - Parameters    │     │    dynamically   │                  │
│  └──────────────────┘     └────────┬─────────┘                  │
│           │                        │                             │
│           │                        ▼                             │
│           │               ┌──────────────────┐                  │
│           └──────────────▶│  Skill Library   │                  │
│                           │                  │                  │
│                           │  Pre-implemented │                  │
│                           │  parameterized   │                  │
│                           │  functions       │                  │
│                           └──────────────────┘                  │
└─────────────────────────────────────────────────────────────────┘
```

### Three Core Components

1. **Skill Library**: Environment-specific parameterized skill implementations
2. **Skill Planner**: LLM that reasons in the parameterized skill space
3. **Skill Executor**: Converts parameterized plans into action sequences

### Five Parameterized Skills (Skill-RTS Benchmark)

| Skill | Parameters | Purpose |
|-------|-----------|---------|
| **Deploy Unit** | unit type, location | Move a unit to a specified location |
| **Harvest Mineral** | mineral location | Move, harvest resources, return to base |
| **Build Building** | building type, location | Construct specified building |
| **Produce Unit** | unit type, direction | Produce unit from a specified direction |
| **Attack Enemy** | unit/enemy type | Command unit to attack specified enemy |

### Executor Mechanism

The executor operates every step within a k-step interval (k=100 default):

1. Identifies active skills using availability conditions
2. Generates action sequences from skill policy given current state + parameters
3. Extracts first action from each skill's sequence
4. Aggregates individual actions into execution vector
5. **Regenerates sequences each step** (not pre-computed) ensuring actions remain relevant
6. Termination conditions dynamically remove completed skills

---

## Key Results

| Metric | Result |
|--------|--------|
| GPT-4o (zero-shot) | Outperformed 80% of baseline agents |
| Qwen2-72B (few-shot) | Surpassed CoacAI (top-tier scripted agent) |
| LLMs tested | 8 total (6 closed-source, 2 open-source) |
| Planning interval | Every 100 steps (reduces LLM invocation costs) |

### Evaluation Metrics (Skill-RTS)

- **Win Rate (WR)**: Win/draw/loss outcomes vs baselines
- **Resource Harvesting Ratio (RHR)**: resources_harvested / game_time × 100
- **Resource Utilization Rate (RUR)**: resources_spent / game_time × 100
- **Unit Production Rate (UPR)**: unit_production / game_time × 100
- **Combat Efficiency Ratio (CER)**: damage_dealt / damage_taken

---

## Limitations and Future Work

- **Limited Scope**: Only tested in MicroRTS; lacks validation in realistic adversarial scenarios
- **Manual Skill Design**: Skill functions require manual construction, limiting cross-domain generalization
- **No Opponent Modeling**: Current approach lacks strategic intent modeling of adversaries
- **Future Directions**:
  - Extend to complex real-world domains
  - LLM-based code generation for skill creation (reducing human effort)
  - Incorporate opponent strategy modeling and exploitation

---

## Relevance to MechaCoder

This paper presents a compelling architecture pattern for structuring how LLMs interact with complex, long-horizon tasks.

- **Applicable patterns:**
  - **Parameterized skill abstraction**: Instead of having the LLM output raw tool calls or code edits, define higher-level "skills" (e.g., `RefactorFunction(fn_name, pattern)`, `AddTest(component, test_type)`, `FixBug(file, issue_description)`) that encapsulate common multi-step operations
  - **Skill executor separation**: Keep LLM planning separate from execution details - the executor handles the "how" while the LLM handles the "what" and "why"
  - **Dynamic replanning**: The k-step replanning interval concept applies well to coding - replan after major state changes rather than executing a rigid upfront plan
  - **Termination conditions**: Skills should self-terminate when their objective is met, rather than running for a fixed duration

- **Implementation considerations:**
  - MechaCoder could benefit from a **skill library** of pre-defined coding operations (test writing, refactoring patterns, documentation generation)
  - The **regenerate-each-step** approach maps well to coding where file state can change between steps
  - Parameterized skills could reduce hallucination by constraining LLM outputs to skill + params rather than arbitrary code
  - The executor pattern could provide better error recovery - if a skill fails, retry with adjusted parameters rather than regenerating the entire plan

- **Potential value:**
  - **Reduces planning complexity**: LLM reasons at skill level, not action level
  - **Improves reliability**: Skill implementations are deterministic and tested
  - **Enables specialization**: Different skills can use different execution strategies
  - **Better metrics**: Can track skill success rates, execution efficiency, etc.

---

## Citation

```bibtex
@article{cui2024plap,
  title={Empowering LLMs with Parameterized Skills for Adversarial Long-Horizon Planning},
  author={Cui, Sijia and Xu, Shuai and He, Aiyao and Wang, Yanna and Xu, Bo},
  journal={arXiv preprint arXiv:2509.13127},
  year={2024}
}
```
