# Reflection on Related Work Building on Generative Agents and Voyager

**Date**: December 4, 2025
**Context**: Follow-up to `no-gradient-lifelong-learning.md`, analyzing recent extensions to the foundational papers.

---

## Overview

The PDF "Related Work Building on Generative Agents and Voyager (2023)" catalogs several 2024-2025 papers that extend the core ideas we analyzed. This document reflects on their implications for MechaCoder and identifies papers we should read in full.

---

## Key Findings by Category

### 1. Memory-Augmented LLM Agents

#### R2-MGA (Ji et al., 2025)
**"Retrieval and Reflection Memory-augmented Generative Agent"**

- Combines retrieval + reflection before generation (not just retrieval)
- **+58% answer correctness, +154% citation quality** by grounding in retrievable facts
- **Implication for MechaCoder**: Our Archivist does retrieval but may not do explicit reflection *on retrieved memories* before using them. R2-MGA suggests a two-step process: (1) retrieve relevant memories, (2) reflect on them to form reasoning, (3) then generate.

**Gap in our analysis**: We proposed retrieval scoring (recency/importance/relevance) but didn't emphasize the reflection-on-retrieval step.

#### Reflexion (Shinn et al., 2023)
**Self-critique after each step**

- Agent notes mistakes and lessons after each reasoning step, stores in memory
- **97% success rate** on virtual task benchmarks vs. far lower without memory-feedback
- **Implication for MechaCoder**: This validates Healer's approach but suggests we should do this more granularly - not just post-failure reflection, but **per-step critique**. Every tool call could generate a brief self-assessment stored in L1 memory.

**Gap in our analysis**: We focused on post-task reflection. Reflexion shows per-step reflection may be more powerful.

#### A-Mem (Xu et al., 2025)
**Zettelkasten-style note-taking for agents**

- Each observation = structured "note" with context, keywords, links
- **Dynamically links** new notes to related past notes
- Creates **evolving knowledge graph** (not just flat vector store)
- **Implication for MechaCoder**: Our `AgentMemory` schema is relatively flat. A-Mem suggests we should add:
  - Explicit bidirectional links between memories
  - Automatic link formation when new memories are added
  - Links that can **update old notes** when new evidence arrives

**Gap in our analysis**: We proposed hierarchical reflection but not graph-structured memory with dynamic linking.

#### Memory-of-Thought (Li et al., 2023)
**Save high-confidence reasoning chains**

- LLM tackles problems, saves high-confidence reasoning to memory
- Later retrieves past "thoughts" to inform new answers
- Gains in math, commonsense, factual QA
- **Implication for MechaCoder**: Our skill library stores code, but MoT suggests we should also store **reasoning chains** - the "why" behind solutions, not just the "what". When facing a new problem, retrieve similar reasoning processes.

**Gap in our analysis**: We focused on code patterns. Should also store reasoning/thought patterns.

---

### 2. Skill Libraries Beyond Voyager

#### Odyssey (Chen et al., 2025)
**Open-source Voyager with 8B model**

- Shared skill library + fine-tuned 8B LLM (not GPT-4)
- **Matches Voyager's performance at fraction of cost**
- Removing skill library causes severe drop - confirms it's the key differentiator
- **Implication for MechaCoder**:
  1. Skill library is even more important than we thought - it enables smaller models to match larger ones
  2. We could potentially use smaller/cheaper models if our skill library is rich enough
  3. Validates our Phase 1 priority (Skill Library first)

**Exciting finding**: This suggests MechaCoder with a rich skill library could work well even with cheaper models, enabling longer autonomous runs.

#### PLAP (Cui et al., 2025)
**Plan-with-Language, Act-with-Parameter**

- **Parameterized skills** - high-level action templates with slots
- LLM skill planner sequences skills, executor grounds them
- Outperformed 80% of baselines zero-shot in RTS games
- **Implication for MechaCoder**: Our skills are currently stored as raw code. PLAP suggests **parameterized templates** might be more powerful:

```typescript
// Current: Raw code skill
const skill = `
const result = yield* fs.readFile(path);
return result;
`;

// Better: Parameterized skill
const skill = {
  template: `const result = yield* fs.readFile({{path}});`,
  parameters: [
    { name: "path", type: "string", description: "File path to read" }
  ],
  executor: (params) => `const result = yield* fs.readFile("${params.path}");`
};
```

**Gap in our analysis**: We proposed raw code skills. Parameterized skills with explicit slots may compose better.

---

### 3. Multi-Timescale Memory

#### Titans (Google, 2023)
**Explicit long-term memory buffer for surprising/important info**

- Influenced Nested Learning's design
- Key insight: **selectively retain "important" experiences** mitigates forgetting
- Combine episodic (fast) and semantic (slow) memory
- **Implication for MechaCoder**: Our importance scoring should probably weight "surprising" events higher. A failed task that usually succeeds is more informative than another routine success.

**Gap in our analysis**: We discussed importance but not "surprise" as a factor. Titans suggests surprise-based importance could be powerful.

---

### 4. Automatic Curriculum

#### Yuan et al. 2023, Zhou et al. 2024
**Agent proposes tasks based on competency gaps**

- Extends Voyager's task-selection to self-directed curriculum
- Growth loop: new challenges → new skills → new challenges
- **Implication for MechaCoder**: Validates our Phase 4 proposal (Automatic Curriculum). These papers may have concrete algorithms we should study.

---

## Updated Implementation Priorities

Based on this related work, I'd refine our priorities:

| Original Priority | Updated Priority | Rationale |
|-------------------|------------------|-----------|
| 1. Skill Library | **1. Skill Library (Parameterized)** | PLAP shows parameterized skills compose better |
| 2. Enhanced Archivist | **2. Graph-Structured Memory** | A-Mem shows linking > flat storage |
| 3. Multi-Timescale Memory | **3. Per-Step Reflection** | Reflexion shows this is powerful |
| 4. Automatic Curriculum | **4. Reasoning Chain Storage** | MoT shows storing "why" matters |
| (new) | **5. Surprise-Based Importance** | Titans insight |

---

## Papers to Request for Full Read

### High Priority (Directly Applicable)

1. **A-Mem: Agentic Memory for LLM Agents** (Xu et al., 2025)
   - arXiv: https://arxiv.org/abs/2502.12110
   - Why: Zettelkasten-style memory with dynamic linking is directly applicable to Archivist redesign

2. **Reflexion: Language Agents with Verbal Reinforcement Learning** (Shinn et al., 2023)
   - Why: Per-step self-critique architecture could transform our Golden Loop

3. **Odyssey: Empowering Minecraft Agents with Open-World Skills** (Chen et al., 2025)
   - IJCAI: https://www.ijcai.org/proceedings/2025/0022.pdf
   - Why: Proves skill library enables smaller models - cost implications for MechaCoder

4. **PLAP: Empowering LLMs with Parameterized Skills** (Cui et al., 2025)
   - arXiv: https://arxiv.org/abs/2509.13127
   - Why: Parameterized skill templates for our Skill Library

### Medium Priority (Theoretical Foundation)

5. **Memory-of-Thought Prompting** (Li et al., 2023)
   - Why: Reasoning chain storage patterns

6. **R2-MGA: Retrieval and Reflection Memory-augmented Generative Agent** (Ji et al., 2025)
   - ChatPaper: https://chatpaper.com/paper/161990
   - Why: Reflection-on-retrieval architecture

7. **Titans** (Google, 2023)
   - Why: Surprise-based memory prioritization

### Lower Priority (Curriculum Approaches)

8. **Yuan et al. 2023** - Automatic curriculum generation
9. **Zhou et al. 2024** - Competency-gap based task proposal

---

## Concrete Changes to Our Analysis

If we update `no-gradient-lifelong-learning.md`, we should add:

### 1. Parameterized Skill Schema

```typescript
interface ParameterizedSkill extends CodeSkill {
  template: string;                    // Code with {{placeholders}}
  parameters: SkillParameter[];        // Typed parameter definitions
  executor: (params: Record<string, unknown>) => string;  // Instantiation
  compositionRules?: CompositionRule[]; // How this skill chains with others
}

interface SkillParameter {
  name: string;
  type: "string" | "number" | "path" | "identifier" | "expression";
  description: string;
  defaultValue?: unknown;
  validation?: string;  // Regex or predicate
}
```

### 2. Graph-Structured Memory

```typescript
interface LinkedMemory extends AgentMemory {
  links: MemoryLink[];
  linkedBy: string[];  // Bidirectional
}

interface MemoryLink {
  targetId: string;
  linkType: "supports" | "contradicts" | "extends" | "supersedes" | "related";
  strength: number;    // 0-1
  createdAt: string;
  evidence?: string;
}

// When adding a new memory:
async function addMemoryWithLinking(memory: AgentMemory): Effect.Effect<LinkedMemory> {
  // 1. Store the memory
  const stored = yield* MemoryService.store(memory);

  // 2. Find related memories
  const related = yield* MemoryService.findRelated(memory);

  // 3. Create links (bidirectional)
  for (const relatedMem of related) {
    const linkType = yield* classifyRelationship(memory, relatedMem);
    yield* MemoryService.link(stored.id, relatedMem.id, linkType);

    // 4. Possibly update old memory based on new evidence
    if (shouldUpdateOld(memory, relatedMem)) {
      yield* MemoryService.updateWithEvidence(relatedMem.id, memory.id);
    }
  }

  return stored;
}
```

### 3. Per-Step Reflection (Reflexion-style)

```typescript
interface StepReflection {
  stepId: string;
  toolCall: string;
  outcome: "success" | "failure" | "partial";
  reflection: string;           // Self-critique
  lessonsLearned: string[];     // Extracted lessons
  shouldRetry: boolean;
  retryStrategy?: string;
}

// After every tool call:
async function reflectOnStep(
  toolCall: ToolCall,
  result: ToolResult
): Effect.Effect<StepReflection> {
  const reflection = yield* LLM.generate({
    prompt: `
You just executed: ${toolCall.name}(${JSON.stringify(toolCall.args)})
Result: ${result.output}
Status: ${result.success ? 'success' : 'failure'}

Reflect briefly:
1. Did this achieve what you intended?
2. What did you learn?
3. Should you retry or proceed?
`,
    maxTokens: 150
  });

  // Store in L1 memory
  yield* MemoryService.storeL1({
    type: "step_reflection",
    content: reflection,
    toolCall: toolCall.name
  });

  return parseReflection(reflection);
}
```

### 4. Surprise-Based Importance

```typescript
function computeImportance(
  memory: AgentMemory,
  context: MemoryContext
): number {
  const baseImportance = memory.importance ?? 0.5;

  // Surprise factor: how unexpected was this outcome?
  const expectedOutcome = yield* predictOutcome(memory.context);
  const actualOutcome = memory.outcome;
  const surprise = computeSurprise(expectedOutcome, actualOutcome);

  // Surprising events get importance boost
  const surpriseBoost = surprise * SURPRISE_WEIGHT;

  // Combine base importance with surprise
  return Math.min(1.0, baseImportance + surpriseBoost);
}

function computeSurprise(expected: Outcome, actual: Outcome): number {
  // High surprise = unexpected failure or unexpected success
  if (expected.success && !actual.success) return 0.8;  // Expected success, got failure
  if (!expected.success && actual.success) return 0.6;  // Expected failure, got success
  if (expected.confidence < 0.5) return 0.3;            // Was uncertain
  return 0.1;                                            // As expected
}
```

---

## Questions for Further Research

1. **A-Mem's dynamic linking**: How computationally expensive is automatic link formation? Can it scale to thousands of memories?

2. **Reflexion's per-step critique**: Does this slow down execution significantly? Is there a lightweight version?

3. **PLAP's parameterization**: How do we discover the right parameters for a skill? Is this learned or hand-specified?

4. **Odyssey's cost reduction**: What's the actual cost comparison? Could we achieve similar results with Claude Haiku + rich skill library?

5. **Surprise detection**: How do we predict "expected outcome" without significant overhead?

---

## Conclusion

The related work validates our core thesis (no-gradient lifelong learning via skills + memory) while suggesting several refinements:

1. **Parameterize skills** for better composition
2. **Link memories** into a knowledge graph
3. **Reflect per-step**, not just post-task
4. **Store reasoning chains**, not just code
5. **Weight by surprise**, not just importance

The most actionable paper to read next is **A-Mem** for its graph-structured memory approach, followed by **PLAP** for parameterized skills.

---

## Paper Request List

Please acquire PDFs for analysis:

| Paper | Year | URL | Priority |
|-------|------|-----|----------|
| A-Mem: Agentic Memory for LLM Agents | 2025 | arxiv.org/abs/2502.12110 | HIGH |
| Reflexion: Language Agents with Verbal Reinforcement Learning | 2023 | arxiv.org/abs/2303.11366 | HIGH |
| Odyssey: Empowering Minecraft Agents | 2025 | ijcai.org/proceedings/2025/0022.pdf | HIGH |
| PLAP: Parameterized Skills for Planning | 2025 | arxiv.org/abs/2509.13127 | HIGH |
| Memory-of-Thought Prompting | 2023 | (need to find) | MEDIUM |
| R2-MGA | 2025 | chatpaper.com/paper/161990 | MEDIUM |
| Titans | 2023 | (Google paper, need to find) | MEDIUM |
