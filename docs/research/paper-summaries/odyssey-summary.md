# ODYSSEY: Empowering Minecraft Agents with Open-World Skills

**Paper Summary**

- **Authors:** Shunyu Liu, Yaoru Li, Kongcheng Zhang, Zhenyu Cui, Wenkai Fang, Yuxuan Zheng, Tongya Zheng, Mingli Song
- **Institutions:** VIPA Lab, Zhejiang University
- **Published:** IJCAI 2025 (Thirty-Fourth International Joint Conference on Artificial Intelligence)
- **arXiv:** 2407.15325
- **Source:** https://arxiv.org/abs/2407.15325

---

## Executive Summary

ODYSSEY is a framework that empowers LLM-based agents with an **open-world skill library** to explore Minecraft beyond basic programmatic tasks. Unlike prior work that treats obtaining diamonds as the ultimate goal, Odyssey provides agents with 223 pre-built skills and a domain-specific language model (MineMA) to enable diverse gameplay and autonomous exploration.

---

## Core Contribution

While previous Minecraft agents like Voyager focus on building skills from scratch through self-play, Odyssey takes a different approach: **bootstrapping agents with a comprehensive skill library and domain knowledge**. The key insight is that agents shouldn't need to learn basic Minecraft mechanics from scratch—instead, they should start with foundational knowledge and focus on higher-level planning and adaptation. This reduces reliance on expensive models like GPT-4 while achieving competitive or superior performance.

---

## Architecture / Methodology

ODYSSEY comprises three key components working together:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        ODYSSEY ARCHITECTURE                          │
│                                                                      │
│  ┌──────────────────┐    ┌────────────────────┐    ┌───────────────┐│
│  │   Open-World     │───▶│   Multi-Agent      │───▶│   MineMA      ││
│  │   Skill Library  │    │   System           │    │   LLM         ││
│  │                  │◀───│                    │◀───│               ││
│  └──────────────────┘    └────────────────────┘    └───────────────┘│
│         │                        │                       │           │
│         ▼                        ▼                       ▼           │
│   [40 Primitive +         [Action Agent]          [390k+ Minecraft  │
│    183 Compositional]     [Curriculum Agent]       Wiki Q&A pairs]  │
│                           [Critic Agent]                             │
│                           [Comment Agent]                            │
└─────────────────────────────────────────────────────────────────────┘
```

### 1. Open-World Skill Library

**Primitive Skills (40):** Low-level actions that directly interact with the game environment:
- Movement (walk, jump, swim, fly)
- Resource gathering (mine, harvest, fish)
- Combat (attack, defend, use items)
- Crafting/smelting operations
- Inventory management

**Compositional Skills (183):** Higher-level behaviors composed from primitives:
- Multi-step crafting sequences (e.g., obtaining a diamond pickaxe)
- Combat strategies (single mob, multi-mob encounters)
- Exploration patterns (cave exploration, village discovery)
- Farming routines (crop cultivation, animal husbandry)

### 2. Multi-Agent System

Four specialized LLM agents coordinate task execution:

| Agent | Role | Function |
|-------|------|----------|
| **Action Agent** | Executor | Selects and executes appropriate skills |
| **Curriculum Agent** | Planner | Proposes farming/exploration tasks |
| **Critic Agent** | Evaluator | Assesses action appropriateness and success |
| **Comment Agent** | Feedback | Provides combat feedback for task rescheduling |

### 3. MineMA: Domain-Specific LLM

A fine-tuned LLaMA-3 model trained on Minecraft domain knowledge:
- **Training Data:** 390k+ instruction entries derived from Minecraft Wiki
- **Model Variants:** MineMA-8B and MineMA-70B
- **Purpose:** Provides accurate Minecraft-specific reasoning without relying on expensive API calls

---

## Benchmark Suite

ODYSSEY introduces a comprehensive evaluation framework with three task categories:

### 1. Long-Term Planning Tasks
- **Subgoal Tasks:** Five classic objectives (wooden/stone/iron/diamond pickaxes, mining diamonds)
- **Combat Tasks:** Single and multi-mob encounters with iterative strategy refinement
- Measures planning horizon and execution persistence

### 2. Dynamic-Immediate Planning Tasks
- **Farming Tasks:** Resource collection and cultivation
- **Adaptive Tasks:** Animal husbandry requiring real-time decision-making
- Tests ability to handle environmental variability

### 3. Autonomous Exploration Tasks
- Open-ended discovery without predefined objectives
- Measures emergent behavior and curiosity-driven learning
- Evaluates diversity of discovered items and locations

---

## Key Results

| Metric | Result |
|--------|--------|
| Primitive Skills | 40 pre-built actions |
| Compositional Skills | 183 higher-level behaviors |
| Training Data | 390k+ Minecraft Wiki instruction entries |
| Performance vs Voyager | Competitive with GPT-4o-mini/GPT-3.5 at lower cost |
| MineMA vs Open-Source LLMs | Outperforms on dynamic planning tasks |
| Combat Task Improvement | Significant reduction in execution time with iterative planning |

**Model Comparison Highlights:**
- MineMA-70B outperforms smaller variants on complex adaptive tasks
- ODYSSEY with MineMA-8B matches Voyager with GPT-4o-mini on autonomous exploration
- Fine-tuned domain knowledge reduces hallucinations about Minecraft mechanics

---

## Limitations and Future Work

- **Skill Library Scope:** Current skills may not cover all edge cases; expansion needed for niche scenarios
- **Model Size Trade-offs:** Larger MineMA models perform better but require more compute
- **Benchmark Coverage:** Current tasks focus on survival mode; creative/adventure modes not evaluated
- **Multi-Agent Coordination:** Room for improvement in agent communication efficiency
- **Visual Understanding:** Still relies on text-based state representation; multimodal integration could improve spatial reasoning

---

## Relevance to MechaCoder

### Applicable Patterns

1. **Pre-Built Skill Library:**
   - Instead of learning every coding pattern from scratch, bootstrap MechaCoder with a library of verified code patterns
   - Analogous to Odyssey's 40 primitive + 183 compositional skills
   - Could include: git operations, test patterns, refactoring recipes, API integration templates

2. **Domain-Specific Fine-Tuning:**
   - MineMA demonstrates value of training on domain-specific Q&A data
   - For MechaCoder: fine-tune on coding Q&A, stack overflow, documentation
   - Reduces reliance on expensive models for routine decisions

3. **Multi-Agent Specialization:**
   - Action Agent → Code execution agent
   - Curriculum Agent → Task planning/selection agent
   - Critic Agent → Code review/verification agent
   - Comment Agent → Feedback integration for iterative improvement

4. **Hierarchical Skill Composition:**
   - Primitive skills (file operations, git commands) compose into higher-level behaviors
   - "Implement feature" = plan + write code + test + commit
   - Enables reasoning at appropriate abstraction levels

### Implementation Considerations

- **Skill Library Format:** Store skills as executable code with semantic embeddings (like Voyager) + metadata about prerequisites and postconditions
- **Domain Knowledge:** Build MechaCoder's equivalent of MineMA by training on:
  - Codebase-specific patterns and conventions
  - Language/framework documentation
  - Past successful implementations
- **Benchmark Tasks:** Adapt Odyssey's three-tier evaluation:
  - Long-term: Multi-file feature implementation
  - Dynamic: Bug fixing with uncertain root cause
  - Autonomous: Codebase exploration and improvement discovery

### Potential Value

1. **Reduced API Costs:** Domain-specific fine-tuning (MineMA approach) could reduce dependency on expensive frontier models for routine coding tasks
2. **Faster Onboarding:** Pre-built skill library means less trial-and-error when starting new projects
3. **Better Evaluation:** Odyssey's benchmark structure provides a template for measuring MechaCoder capabilities across different task types
4. **Skill Transfer:** Compositional skills learned in one codebase could transfer to similar projects

---

## Comparison with Voyager

| Aspect | Voyager | Odyssey |
|--------|---------|---------|
| Skill Acquisition | Learns from scratch via exploration | Bootstrapped with 223 pre-built skills |
| LLM Backend | GPT-4 (expensive) | MineMA fine-tuned LLaMA-3 (cost-effective) |
| Focus | Open-ended exploration | Structured benchmark evaluation |
| Curriculum | Automatic (learned) | Multi-agent with specialized roles |
| Domain Knowledge | In-context from GPT-4 | Explicit via 390k Wiki Q&A training |
| Goal | Discover diverse gameplay | Evaluate agent capabilities systematically |

Odyssey builds on Voyager's foundation but prioritizes **cost-effectiveness** and **systematic evaluation** over pure exploration.

---

## Citation

```bibtex
@inproceedings{liu2025odyssey,
  title={Odyssey: Empowering Minecraft Agents with Open-World Skills},
  author={Liu, Shunyu and Li, Yaoru and Zhang, Kongcheng and Cui, Zhenyu and Fang, Wenkai and Zheng, Yuxuan and Zheng, Tongya and Song, Mingli},
  booktitle={Proceedings of the Thirty-Fourth International Joint Conference on Artificial Intelligence (IJCAI-25)},
  year={2025},
  doi={10.24963/ijcai.2025/22}
}
```
