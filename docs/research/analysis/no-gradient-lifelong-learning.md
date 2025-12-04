# No-Gradient Lifelong Learning for Autonomous Coding Agents

**A Research Analysis of Voyager, Nested Learning, and Generative Agents
Applied to OpenAgents and MechaCoder**

---

## Executive Summary

This document explores a paradigm shift in how we think about "training" autonomous agents. Rather than gradient descent over neural network weights, we can achieve continual learning through **code execution, skill accumulation, and structured memory**. This "no-gradient architecture" was pioneered by Voyager in Minecraft and has profound implications for autonomous coding agents like MechaCoder.

### The Core Insight

From Jim Fan's Voyager announcement (May 2023):

> GPT-4 unlocks a new paradigm: "training" is code execution rather than gradient descent. "Trained model" is a codebase of skills that Voyager iteratively composes, rather than matrices of floats. We are pushing no-gradient architecture to its limit.

This insight reframes how MechaCoder can achieve genuine lifelong learning:

| Traditional ML | No-Gradient Architecture |
|---------------|-------------------------|
| Training = gradient updates | Training = code execution |
| Model = neural weights | Model = skill library (code) |
| Memory = activations | Memory = structured knowledge base |
| Forgetting = weight overwrite | Forgetting = managed via multi-timescale storage |
| Improvement = more data | Improvement = more verified skills |

### MechaCoder's Position

MechaCoder already implements several components of this architecture:
- **Iterative prompting** via Golden Loop's test feedback
- **Self-verification** through type checking and test suites
- **Basic memory** via Archivist's lesson storage

What's missing:
- **Skill Library** - Executable code patterns, not just lessons
- **Multi-timescale memory** - Different update frequencies for different knowledge types
- **Automatic curriculum** - Self-proposed tasks based on capability gaps

This analysis explores how to close these gaps, potentially transforming MechaCoder from a task executor into a genuinely learning coding agent.

---

## Part I: The Research Foundation

### 1. Voyager: Lifelong Learning Through Skill Composition

Voyager demonstrates that an LLM-powered agent can continuously improve without any fine-tuning by maintaining an ever-growing library of verified, composable skills stored as executable code.

#### 1.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        VOYAGER LOOP                              │
│                                                                  │
│  ┌──────────────────┐    ┌────────────────────┐    ┌───────────┐│
│  │    Automatic     │───▶│     Iterative      │───▶│   Skill   ││
│  │   Curriculum     │    │     Prompting      │    │  Library  ││
│  │                  │◀───│    Mechanism       │◀───│           ││
│  └──────────────────┘    └────────────────────┘    └───────────┘│
│         │                        │                       │       │
│         ▼                        ▼                       ▼       │
│   [Propose Task]          [Generate Code]         [Store/Retrieve│
│                           [Execute]                Skills]       │
│                           [Self-Verify]                          │
└─────────────────────────────────────────────────────────────────┘
```

#### 1.2 Key Components

**Automatic Curriculum**: GPT-4 proposes tasks based on current state (inventory, position, completed/failed tasks). Tasks are designed to be just beyond current capabilities - neither too easy nor impossible. This creates a natural learning progression without human intervention.

**Skill Library**: The crucial innovation. Each verified skill is stored as:
- **Key**: Embedding of natural language description
- **Value**: Executable JavaScript code

Skills are compositional - complex skills can call simpler skills. The library grows monotonically; successfully verified skills are never removed.

**Iterative Prompting**: Three feedback types enable self-correction:
1. **Environment feedback**: Runtime logs, intermediate results
2. **Execution errors**: Syntax and runtime exceptions
3. **Self-verification**: A separate GPT-4 agent acts as critic

#### 1.3 Results That Matter for Us

- **3.3x more discoveries** than prior SOTA
- **Only agent to unlock diamond tools** - others plateau
- **Skills transfer to new worlds** - the library generalizes
- **Skill library helps other methods too** - it's the key differentiator

The ablation studies are particularly instructive:
- Remove random curriculum: -93% items discovered (curriculum is essential)
- Remove self-verification: -73% (critic feedback is crucial)
- Remove skill library: plateau in later stages (accumulation enables growth)

#### 1.4 Why This Matters for Coding Agents

Software development is remarkably similar to Minecraft:
- Both involve composing primitives into complex solutions
- Both have clear success/failure signals (tests pass/fail)
- Both benefit from building on past solutions
- Both have "tech trees" (you can't build advanced features without basics)

The skill library concept maps directly: store verified code solutions indexed by semantic description, retrieve relevant past solutions for new tasks.

---

### 2. Nested Learning: Multi-Timescale Memory for Continual Learning

The Nested Learning paper provides the theoretical foundation for how memory should be structured in learning systems, explaining why LLMs suffer from "anterograde amnesia" and how to address it.

#### 2.1 The Anterograde Amnesia Problem

LLMs have a condition analogous to anterograde amnesia - they cannot form new long-term memories after pre-training:

> "Their knowledge is limited to: the immediate context (fits in context window) and long-past knowledge in MLPs (before 'end of pre-training')."

This creates a gap: knowledge that's too old (pre-training) and too new (current context), but nothing in between. Every session starts fresh.

#### 2.2 Multi-Timescale Learning

The key insight from neuroscience: the brain uses different frequencies for different types of learning:
- **Gamma waves (30-150 Hz)**: Sensory processing
- **Beta waves (13-30 Hz)**: Active thinking
- **Delta/Theta waves (0.5-8 Hz)**: Memory consolidation

Nested Learning proposes similar multi-timescale updates for AI systems:

```
┌────────────────────────────────────────────────────────────────┐
│                 CONTINUUM MEMORY SYSTEM (CMS)                   │
│                                                                 │
│  Level 1 (f₁ = high)   ─────────────────────────────────────── │
│  Updates every token    [■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■] │
│                                                                 │
│  Level 2 (f₂ = medium) ─────────────────────────────────────── │
│  Updates every chunk    [■■■■■     ■■■■■     ■■■■■     ■■■■■ ] │
│                                                                 │
│  Level 3 (f₃ = low)    ─────────────────────────────────────── │
│  Updates per session    [■■■■■■■■■■                           ] │
│                                                                 │
│  Level 4 (f₄ = glacial) ────────────────────────────────────── │
│  Updates rarely         [■                                    ] │
└────────────────────────────────────────────────────────────────┘
```

#### 2.3 Catastrophic Forgetting as Compression

A crucial reframing:

> "Catastrophic forgetting is a natural consequence of compression—limited capacity forces the model to forget to retain capacity for new information."

The solution isn't to prevent forgetting - it's to **manage where forgetting happens**. Lower-frequency memories preserve important patterns while higher-frequency memories handle immediate context. When a high-frequency memory is overwritten, the knowledge may still exist in a lower-frequency level.

#### 2.4 The "Loop Through Time"

Nested Learning introduces knowledge transfer between levels via initialization:

```
When updating MLP^(f_s):
1. Forgotten knowledge may still exist in MLP^(f_s') where s' < s
2. Backpropagation through initial states can "circle back" forgotten knowledge
3. Creates a "loop through time" preventing catastrophic forgetting
```

This has direct implications for agent memory: periodic consolidation from fast to slow memory creates durability.

#### 2.5 Application to MechaCoder

We can implement a Continuum Memory System:

| Level | Update Frequency | What's Stored | MechaCoder Implementation |
|-------|-----------------|---------------|---------------------------|
| L1 | Every tool call | Current context, recent errors | Progress.md, subtask state |
| L2 | Every subtask | Patterns that worked/failed | Archivist per-subtask memories |
| L3 | Every task | Lessons learned, heuristics | Project-level memory bank |
| L4 | Every session | Strategic insights | Cross-session memory |
| L5 | Cross-project | Universal patterns | Global ~/.openagents/memory |

The key is **consolidation**: important L1 patterns should propagate to L2, important L2 patterns to L3, and so on. This prevents the "each session starts fresh" problem.

---

### 3. Generative Agents: Memory, Reflection, and Planning

The Generative Agents paper provides the practical architecture for implementing memory in LLM-powered agents, demonstrating that believable long-term behavior requires more than a powerful model - it requires thoughtful systems for memory, retrieval, reflection, and planning.

#### 3.1 Memory Stream Architecture

Every experience is stored with:
- Natural language description
- Creation timestamp
- Last access timestamp

```
┌─────────────────────────────────────────────────────────────────┐
│                       MEMORY STREAM                              │
│                                                                  │
│  [Observation] "Fixed type error in auth.ts"                     │
│  [Observation] "Test suite passed after 3 retries"               │
│  [Observation] "Claude Code timed out on large file"             │
│  [Reflection] "Effect patterns require careful error handling"   │
│  [Reflection] "Large files should be split before processing"    │
│  [Plan] "Tomorrow: tackle the API rate limiting issue"           │
└─────────────────────────────────────────────────────────────────┘
```

#### 3.2 Retrieval Function

The breakthrough is the retrieval scoring formula:

```
score = α_recency × recency + α_importance × importance + α_relevance × relevance
```

Where:
- **Recency**: Exponential decay from last access (recent memories score higher)
- **Importance**: LLM-rated 1-10 score (breaking changes > routine fixes)
- **Relevance**: Cosine similarity between query and memory embedding

This enables contextually appropriate memory retrieval - not just "what's recent" but "what's relevant AND important AND recent."

#### 3.3 Reflection Mechanism

Periodically (when importance threshold exceeded), the agent reflects:

1. Query recent memories
2. Ask: "What are the 3 most salient high-level insights?"
3. Generate abstract reflections with citations to evidence
4. Store reflections back into memory stream

This creates hierarchical abstraction:
```
[Reflection] "This codebase uses Effect for error handling"
        ↑
[Reflection] "Effect.gen pattern seen in multiple files"
        ↑
[Observation] "auth.ts uses Effect.gen"
[Observation] "api.ts uses Effect.gen"
[Observation] "db.ts uses Effect.gen"
```

Reflections can build on other reflections, creating ever-higher abstractions.

#### 3.4 Planning and Reacting

Agents maintain hierarchical plans:
- Day-level goals
- Hour-level tasks
- 5-15 minute action items

Crucially, plans are **reactive**: new observations can trigger plan revision. This prevents rigid behavior that ignores changed circumstances.

#### 3.5 Evaluation Results

The full architecture (observation + reflection + planning) outperformed all ablations:

| Condition | TrueSkill μ |
|-----------|------------|
| Full Architecture | **29.89** |
| No Reflection | 26.88 |
| No Reflection, No Planning | 25.64 |
| Human Crowdworker | 22.95 |
| No Memory/Reflection/Planning | 21.21 |

**Key finding**: Full architecture vs. prior work shows Cohen's d = 8.16 - a massive 8 standard deviation improvement. Architecture matters more than model power.

#### 3.6 Application to MechaCoder

Archivist already implements a basic version of this:
- Memory stream = `AgentMemory` entries
- Reflection = post-run lesson extraction
- Retrieval = tag + scope + keyword search

What's missing:
- **Importance scoring** during memory creation
- **Recency weighting** in retrieval
- **Hierarchical reflection** (reflections on reflections)
- **Reactive planning** (revising plans based on new observations)

---

## Part II: The MechaCoder Opportunity

### 4. Current State Analysis

Let's map the research concepts to MechaCoder's existing architecture:

| Research Concept | MechaCoder Component | Status | Gap |
|-----------------|---------------------|--------|-----|
| **Skill Library** | Archivist Memory Bank | Partial | Stores lessons, not executable code |
| **Automatic Curriculum** | Task Picker | Partial | Picks tasks, doesn't propose new ones |
| **Iterative Prompting** | Golden Loop | Implemented | Test/typecheck feedback works well |
| **Self-Verification** | Test suites + Healer | Implemented | Strong verification chain |
| **Environment Feedback** | Build output, logs | Implemented | Rich feedback available |
| **Memory Stream** | Archivist episodes/lessons | Partial | Missing importance scoring |
| **Reflection** | Archivist post-run | Partial | Not hierarchical |
| **Multi-timescale Memory** | Not implemented | Missing | Major opportunity |
| **Planning** | Subtask decomposition | Partial | Not reactive |

### 5. The Voyager-MechaCoder Analogy

The mapping between Minecraft and software development is surprisingly direct:

| Voyager (Minecraft) | MechaCoder (Software Development) |
|--------------------|------------------------------------|
| Minecraft world | Codebase |
| Block/item inventory | Dependencies, imports, available APIs |
| Biome, time, position | Project structure, branch, test status |
| Mine/craft/smelt primitives | Read/edit/write/bash tools |
| JavaScript game APIs | File system, git, build tools |
| "Mine 1 stone" | "Read src/auth.ts" |
| "Craft 1 stone pickaxe" | "Fix type error in auth.ts" |
| Skill library (JS code) | Solution library (code patterns) |
| GPT-4 code generation | Claude/LLM code generation |
| Self-verification (critic) | Test suites + type checking |
| Environment feedback | Build output, test results |
| Tech tree (wood → stone → iron) | Feature tree (auth → permissions → roles) |

The tech tree analogy is particularly apt. Just as Voyager can't craft iron tools without first obtaining iron (which requires stone tools to mine), MechaCoder can't implement complex features without first having the foundational code in place. The automatic curriculum in Voyager proposes tasks that respect these dependencies - MechaCoder should do the same.

### 6. What Would a "Trained" MechaCoder Look Like?

In the no-gradient paradigm, a "trained" MechaCoder has:

#### 6.1 A Rich Skill Library

```typescript
// Example skill from the library
{
  id: "sk-fix-effect-type-error",
  description: "Fix Effect.gen type errors by adding explicit type annotations",
  embedding: [0.23, -0.45, ...],  // Semantic embedding
  code: `
// Pattern: When Effect.gen fails with type inference
// Solution: Add explicit type parameter to yield*

// Before (fails)
const result = yield* someEffect;

// After (works)
const result = yield* someEffect as SomeType;

// Or with explicit generic
Effect.gen(function* () {
  const result: SomeType = yield* someEffect;
  return result;
});
`,
  context: {
    projectPatterns: ["Effect", "TypeScript"],
    taskTypes: ["type-error", "effect-pattern"],
    errorSignatures: ["Type 'Effect<...>' is not assignable"]
  },
  verification: {
    successCount: 47,
    failureCount: 3,
    lastUsed: "2025-12-04T10:00:00Z",
    successRate: 0.94
  },
  composesFrom: ["sk-read-error-message", "sk-locate-effect-code"]
}
```

#### 6.2 Multi-Level Memory

```
Session Memory (L1)
├── "Just fixed auth.ts, tests pass"
├── "API rate limit hit at 10:23"
└── "Switching to minimal subagent due to timeout"

Task Memory (L2)
├── "Effect.gen errors in this repo usually need explicit types"
├── "Tests in src/tools/ are flaky when run in parallel"
└── "Healer successfully recovered from 3 init failures today"

Project Memory (L3)
├── "This codebase uses Effect for all async operations"
├── "Test suite takes ~45 seconds, typecheck takes ~8 seconds"
└── "Claude Code works better than minimal for multi-file changes"

Global Memory (L4)
├── "TypeScript strict mode catches more errors but needs more annotations"
├── "Git conflicts usually happen in package.json and lock files"
└── "Rate limits are recoverable with exponential backoff"
```

#### 6.3 Self-Directed Learning

The agent identifies its own improvement areas:

```
Capability Gap Detection:
├── "Success rate on 'refactoring' tasks: 67% (below 80% threshold)"
│   └── Proposed: Practice refactoring in Gym with controlled scenarios
├── "No skills for 'database migration' task type"
│   └── Proposed: Create training task for migration patterns
└── "Healer invoked 5 times this week for same error pattern"
    └── Proposed: Add skill to handle this pattern preemptively
```

---

## Part III: Implementation Proposals

### 7. Skill Library for MechaCoder

The highest-value addition. Directly maps to Voyager's key success factor.

#### 7.1 Schema

```typescript
interface CodeSkill {
  // Identity
  id: string;                      // "sk-" prefix
  version: number;                 // Increments on updates
  supersedes?: string;             // Previous version ID

  // Description (for retrieval)
  description: string;             // Natural language
  embedding: number[];             // For semantic search
  tags: string[];                  // ["effect", "error-handling", "typescript"]

  // The actual skill
  skillType: "pattern" | "template" | "procedure" | "composition";
  code: string;                    // The code itself
  language: "typescript" | "javascript" | "bash" | "markdown";

  // Context for applicability
  context: {
    projectPatterns: string[];     // Required patterns in project
    taskTypes: string[];           // Task types this applies to
    errorSignatures?: string[];    // Error messages that trigger this
    filePatterns?: string[];       // File glob patterns
  };

  // Verification tracking
  verification: {
    successCount: number;
    failureCount: number;
    lastUsed: string;
    lastSucceeded: string;
    successRate: number;           // Computed
  };

  // Composition
  composesFrom: string[];          // Skills this builds on
  composedBy: string[];            // Skills that use this

  // Provenance
  createdAt: string;
  createdFrom: {
    taskId?: string;
    sessionId?: string;
    source: "execution" | "reflection" | "manual";
  };
}
```

#### 7.2 Skill Extraction

After a successful task completion:

```typescript
async function extractSkills(
  task: Task,
  trajectory: Trajectory,
  diff: GitDiff
): Effect.Effect<CodeSkill[], SkillExtractionError> {
  // 1. Identify novel patterns in the diff
  const patterns = yield* identifyPatterns(diff);

  // 2. Check if patterns are already in skill library
  const existingSkills = yield* SkillLibrary.query({
    patterns: patterns.map(p => p.description)
  });

  // 3. For novel patterns, extract as skills
  const novelPatterns = patterns.filter(p =>
    !existingSkills.some(s => similarity(s, p) > 0.9)
  );

  // 4. Generate skill objects with descriptions
  const skills = yield* generateSkillDescriptions(novelPatterns);

  // 5. Add embeddings
  const withEmbeddings = yield* addEmbeddings(skills);

  return withEmbeddings;
}
```

#### 7.3 Skill Retrieval

Before generating code for a new task:

```typescript
async function retrieveRelevantSkills(
  task: Task,
  context: TaskContext
): Effect.Effect<CodeSkill[], SkillRetrievalError> {
  // 1. Generate query embedding from task description
  const queryEmbedding = yield* embed(task.description);

  // 2. Find semantically similar skills
  const candidates = yield* SkillLibrary.similaritySearch(queryEmbedding, {
    limit: 20,
    minSimilarity: 0.7
  });

  // 3. Filter by context compatibility
  const compatible = candidates.filter(skill =>
    isContextCompatible(skill.context, context)
  );

  // 4. Rank by success rate and recency
  const ranked = compatible.sort((a, b) => {
    const recencyA = recencyScore(a.verification.lastUsed);
    const recencyB = recencyScore(b.verification.lastUsed);
    const successA = a.verification.successRate;
    const successB = b.verification.successRate;

    return (0.6 * successB + 0.4 * recencyB) -
           (0.6 * successA + 0.4 * recencyA);
  });

  // 5. Return top skills
  return ranked.slice(0, 5);
}
```

#### 7.4 Integration with Coding Subagent

Skills are injected into the prompt:

```typescript
const systemPrompt = `
You are implementing task: ${task.title}

## Relevant Skills from Library

The following code patterns have been successful in similar tasks:

${relevantSkills.map(skill => `
### ${skill.description}
Success rate: ${(skill.verification.successRate * 100).toFixed(0)}%
Used ${skill.verification.successCount} times

\`\`\`${skill.language}
${skill.code}
\`\`\`
`).join('\n')}

Consider using these patterns where applicable. You may adapt them to fit the current context.
`;
```

### 8. Multi-Timescale Memory System

Implementing Nested Learning's Continuum Memory System for code agents.

#### 8.1 Memory Levels

```typescript
type MemoryLevel = "L1" | "L2" | "L3" | "L4" | "L5";

interface MemoryLevelConfig {
  level: MemoryLevel;
  name: string;
  updateTrigger: string;
  retention: string;
  consolidationTarget?: MemoryLevel;
  storage: string;
}

const MEMORY_LEVELS: MemoryLevelConfig[] = [
  {
    level: "L1",
    name: "Immediate",
    updateTrigger: "Every tool call",
    retention: "Current session only",
    consolidationTarget: "L2",
    storage: "In-memory + progress.md"
  },
  {
    level: "L2",
    name: "Subtask",
    updateTrigger: "Every subtask completion",
    retention: "Current task",
    consolidationTarget: "L3",
    storage: ".openagents/subtasks/*.json"
  },
  {
    level: "L3",
    name: "Task",
    updateTrigger: "Every task completion",
    retention: "Permanent (per-project)",
    consolidationTarget: "L4",
    storage: ".openagents/memory/lessons.jsonl"
  },
  {
    level: "L4",
    name: "Session",
    updateTrigger: "Every session end",
    retention: "Permanent (per-project)",
    consolidationTarget: "L5",
    storage: ".openagents/memory/strategic.jsonl"
  },
  {
    level: "L5",
    name: "Global",
    updateTrigger: "Periodic consolidation",
    retention: "Permanent (global)",
    storage: "~/.openagents/memory/global.jsonl"
  }
];
```

#### 8.2 Memory Consolidation

The key to preventing forgetting:

```typescript
interface ConsolidationEvent {
  fromLevel: MemoryLevel;
  toLevel: MemoryLevel;
  memories: Memory[];
  consolidatedAt: string;
}

async function consolidateMemories(
  fromLevel: MemoryLevel,
  toLevel: MemoryLevel
): Effect.Effect<ConsolidationEvent, ConsolidationError> {
  // 1. Get memories from source level
  const sourceMemories = yield* MemoryService.getByLevel(fromLevel);

  // 2. Score by importance and frequency
  const scored = sourceMemories.map(m => ({
    memory: m,
    score: computeConsolidationScore(m)
  }));

  // 3. Select top memories for consolidation
  const toConsolidate = scored
    .filter(s => s.score > CONSOLIDATION_THRESHOLD)
    .sort((a, b) => b.score - a.score)
    .slice(0, MAX_CONSOLIDATION_COUNT);

  // 4. Generate abstractions (reflections)
  const abstractions = yield* generateAbstractions(
    toConsolidate.map(s => s.memory)
  );

  // 5. Store in target level
  for (const abstraction of abstractions) {
    yield* MemoryService.store({
      ...abstraction,
      level: toLevel,
      consolidatedFrom: toConsolidate.map(s => s.memory.id)
    });
  }

  // 6. Mark source memories as consolidated (don't delete)
  for (const { memory } of toConsolidate) {
    yield* MemoryService.markConsolidated(memory.id, toLevel);
  }

  return {
    fromLevel,
    toLevel,
    memories: abstractions,
    consolidatedAt: new Date().toISOString()
  };
}
```

#### 8.3 Retrieval Across Levels

```typescript
async function retrieveMemories(
  query: MemoryQuery,
  context: RetrievalContext
): Effect.Effect<Memory[], MemoryError> {
  // Retrieve from all relevant levels
  const levelResults = await Promise.all(
    MEMORY_LEVELS
      .filter(level => shouldQueryLevel(level, context))
      .map(level => retrieveFromLevel(query, level))
  );

  // Combine and deduplicate
  const allMemories = levelResults.flat();
  const deduplicated = deduplicateByContent(allMemories);

  // Score by recency + importance + relevance
  const scored = deduplicated.map(memory => ({
    memory,
    score: computeRetrievalScore(memory, query, context)
  }));

  // Return top memories
  return scored
    .sort((a, b) => b.score - a.score)
    .slice(0, query.limit ?? 10)
    .map(s => s.memory);
}

function computeRetrievalScore(
  memory: Memory,
  query: MemoryQuery,
  context: RetrievalContext
): number {
  const recency = computeRecencyScore(memory.updatedAt);
  const importance = memory.importance ?? 0.5;
  const relevance = computeRelevanceScore(memory, query);

  // Level weighting: higher levels get slight boost for strategic queries
  const levelBoost = context.queryType === "strategic"
    ? LEVEL_WEIGHTS[memory.level]
    : 1.0;

  return (
    RECENCY_WEIGHT * recency +
    IMPORTANCE_WEIGHT * importance +
    RELEVANCE_WEIGHT * relevance
  ) * levelBoost;
}
```

### 9. Automatic Curriculum

Enabling self-directed learning by proposing tasks based on capability gaps.

#### 9.1 Capability Gap Detection

```typescript
interface CapabilityGap {
  domain: string;              // "refactoring", "testing", "api-design"
  evidence: GapEvidence[];
  severity: "low" | "medium" | "high";
  proposedRemedy: TaskProposal;
}

interface GapEvidence {
  type: "low_success_rate" | "missing_skill" | "repeated_failure" | "healer_pattern";
  details: string;
  dataPoints: string[];        // Task IDs, session IDs, etc.
}

async function detectCapabilityGaps(
  projectRoot: string,
  lookbackDays: number = 30
): Effect.Effect<CapabilityGap[], AnalysisError> {
  // 1. Analyze task completion history
  const tasks = yield* TaskService.getCompletedTasks({
    projectRoot,
    since: daysAgo(lookbackDays)
  });

  // 2. Group by task type and compute success rates
  const byType = groupBy(tasks, t => t.type);
  const successRates = Object.entries(byType).map(([type, tasks]) => ({
    type,
    successRate: tasks.filter(t => t.status === "closed").length / tasks.length,
    totalTasks: tasks.length
  }));

  // 3. Identify low success rate areas
  const lowSuccessGaps = successRates
    .filter(sr => sr.successRate < 0.8 && sr.totalTasks >= 3)
    .map(sr => ({
      domain: sr.type,
      evidence: [{
        type: "low_success_rate" as const,
        details: `Success rate: ${(sr.successRate * 100).toFixed(0)}%`,
        dataPoints: byType[sr.type].map(t => t.id)
      }],
      severity: sr.successRate < 0.5 ? "high" : "medium",
      proposedRemedy: generateTrainingTask(sr.type)
    }));

  // 4. Identify missing skills
  const recentTasks = tasks.slice(0, 20);
  const taskDescriptions = recentTasks.map(t => t.description);
  const existingSkills = yield* SkillLibrary.getAll();

  const missingSkillGaps = yield* identifyMissingSkills(
    taskDescriptions,
    existingSkills
  );

  // 5. Analyze Healer patterns
  const healerInvocations = yield* HealerService.getInvocations({
    projectRoot,
    since: daysAgo(lookbackDays)
  });

  const healerPatternGaps = identifyHealerPatterns(healerInvocations);

  return [...lowSuccessGaps, ...missingSkillGaps, ...healerPatternGaps];
}
```

#### 9.2 Task Proposal Generation

```typescript
interface TaskProposal {
  title: string;
  description: string;
  type: TaskType;
  priority: number;
  labels: string[];
  rationale: string;
  expectedLearning: string[];
  prerequisitesMet: boolean;
  estimatedDifficulty: number;
  discoveredFrom: {
    source: "capability_gap" | "exploration" | "pattern_detection";
    gapId?: string;
  };
}

async function generateTrainingTask(
  domain: string
): Effect.Effect<TaskProposal, GenerationError> {
  // 1. Find relevant Gym scenarios
  const gymScenarios = yield* GymService.getScenarios({
    domain,
    difficulty: "progressive"  // Start easy, get harder
  });

  // 2. Generate task based on capability gap
  const proposal: TaskProposal = {
    title: `Training: Improve ${domain} capability`,
    description: `
Run Gym scenarios to improve ${domain} skills.

Scenarios:
${gymScenarios.map(s => `- ${s.name}: ${s.description}`).join('\n')}

Success criteria:
- Complete all scenarios with >80% success rate
- Extract at least 2 new skills for the library
`,
    type: "task",
    priority: 2,  // Medium priority - training, not urgent
    labels: ["training", "gym", domain],
    rationale: `Success rate on ${domain} tasks is below threshold. Deliberate practice should improve performance.`,
    expectedLearning: [
      `New ${domain} patterns`,
      `Error handling for ${domain}`,
      `Best practices for ${domain}`
    ],
    prerequisitesMet: true,
    estimatedDifficulty: 0.6,
    discoveredFrom: {
      source: "capability_gap"
    }
  };

  return proposal;
}
```

#### 9.3 Curriculum Integration

```typescript
async function proposeNextTasks(
  projectRoot: string,
  count: number = 3
): Effect.Effect<TaskProposal[], CurriculumError> {
  // 1. Detect capability gaps
  const gaps = yield* detectCapabilityGaps(projectRoot);

  // 2. Get existing tasks to avoid duplicates
  const existingTasks = yield* TaskService.getOpenTasks(projectRoot);

  // 3. Generate proposals for each gap
  const proposals = gaps
    .sort((a, b) => severityScore(b.severity) - severityScore(a.severity))
    .slice(0, count)
    .map(gap => gap.proposedRemedy)
    .filter(proposal => !isDuplicate(proposal, existingTasks));

  // 4. Add exploration tasks if not enough gap-based tasks
  if (proposals.length < count) {
    const explorationTasks = yield* generateExplorationTasks(
      projectRoot,
      count - proposals.length
    );
    proposals.push(...explorationTasks);
  }

  return proposals;
}
```

### 10. Gym as Training Environment

TerminalBench is our "Minecraft" - a controlled environment for skill development.

#### 10.1 Skill Development Loop

```
┌────────────────────────────────────────────────────────────────┐
│                    SKILL DEVELOPMENT LOOP                       │
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │   Gap       │───▶│   Gym       │───▶│   Skill     │         │
│  │ Detection   │    │ Training    │    │ Extraction  │         │
│  └─────────────┘    └─────────────┘    └─────────────┘         │
│         ▲                                     │                 │
│         │                                     ▼                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │  Real Task  │◀───│   Skill     │◀───│   Skill     │         │
│  │  Execution  │    │  Retrieval  │    │  Library    │         │
│  └─────────────┘    └─────────────┘    └─────────────┘         │
│                                                                 │
│  The loop: Gaps → Practice → Skills → Better Performance → ...  │
└────────────────────────────────────────────────────────────────┘
```

#### 10.2 Transfer Learning Validation

```typescript
interface TransferTest {
  gymScenario: string;
  realTaskType: string;
  skillsLearned: string[];
  gymSuccessRate: number;
  realTaskSuccessRate: number;
  transferEfficiency: number;  // real / gym success rate
}

async function validateTransfer(
  skills: CodeSkill[],
  gymScenarios: GymScenario[],
  realTasks: Task[]
): Effect.Effect<TransferTest[], ValidationError> {
  const tests: TransferTest[] = [];

  for (const skill of skills) {
    // Find gym scenarios that taught this skill
    const relatedGymScenarios = gymScenarios.filter(s =>
      s.extractedSkills.includes(skill.id)
    );

    // Find real tasks that used this skill
    const relatedRealTasks = realTasks.filter(t =>
      t.skillsUsed?.includes(skill.id)
    );

    if (relatedGymScenarios.length > 0 && relatedRealTasks.length > 0) {
      const gymSuccessRate = relatedGymScenarios
        .reduce((sum, s) => sum + s.successRate, 0) / relatedGymScenarios.length;

      const realTaskSuccessRate = relatedRealTasks
        .filter(t => t.status === "closed").length / relatedRealTasks.length;

      tests.push({
        gymScenario: relatedGymScenarios[0].name,
        realTaskType: relatedRealTasks[0].type,
        skillsLearned: [skill.id],
        gymSuccessRate,
        realTaskSuccessRate,
        transferEfficiency: realTaskSuccessRate / gymSuccessRate
      });
    }
  }

  return tests;
}
```

---

## Part IV: Synthesis and Recommendations

### 11. The Unified Vision

Combining all three research papers, we arrive at a vision for MechaCoder as a genuinely learning coding agent:

```
┌────────────────────────────────────────────────────────────────────┐
│                    MECHACODER LIFELONG LEARNING                     │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    AUTOMATIC CURRICULUM                       │  │
│  │   Gap Detection → Task Proposal → Priority Scheduling         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                      GOLDEN LOOP                              │  │
│  │   Task → Understand → Implement → Test → Commit → Log         │  │
│  │              │                       │                        │  │
│  │              ▼                       ▼                        │  │
│  │      ┌─────────────┐         ┌─────────────┐                  │  │
│  │      │   SKILL     │         │   SELF-     │                  │  │
│  │      │  RETRIEVAL  │         │VERIFICATION │                  │  │
│  │      └─────────────┘         └─────────────┘                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   REFLECTION & MEMORY                         │  │
│  │   ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐ │  │
│  │   │   L1   │─▶│   L2   │─▶│   L3   │─▶│   L4   │─▶│   L5   │ │  │
│  │   │Immediate│  │Subtask │  │ Task   │  │Session │  │Global  │ │  │
│  │   └────────┘  └────────┘  └────────┘  └────────┘  └────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     SKILL LIBRARY                             │  │
│  │   Extraction → Storage → Retrieval → Composition → Deprecation │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                         GYM                                   │  │
│  │   Training Scenarios → Skill Practice → Transfer Validation   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  The loop: Gaps → Tasks → Execute → Learn → Store → Retrieve → ... │
└────────────────────────────────────────────────────────────────────┘
```

### 12. Priority Order for Implementation

Based on the research and MechaCoder's current state, I recommend this implementation order:

#### Phase 1: Skill Library (Highest Value)
**Rationale**: This is Voyager's key differentiator. Enables immediate improvement in task success rate by retrieving proven solutions.

1. Add `CodeSkill` schema to Archivist
2. Implement embedding generation for code patterns
3. Build retrieval pipeline into coding subagent
4. Add skill extraction from successful tasks
5. Track usage and success rates

**Estimated impact**: 20-40% improvement in task success rate based on Voyager's ablation studies.

#### Phase 2: Enhanced Archivist with Importance Scoring
**Rationale**: Low-hanging fruit that improves memory quality.

1. Add importance scoring during memory creation
2. Implement recency weighting in retrieval
3. Add hierarchical reflection (reflections on reflections)
4. Improve retrieval scoring formula

**Estimated impact**: Better context in prompts → fewer retry loops.

#### Phase 3: Multi-Timescale Memory
**Rationale**: Addresses the "each session starts fresh" problem.

1. Define L1-L5 memory levels
2. Implement level-specific storage
3. Build consolidation pipeline
4. Integrate cross-level retrieval

**Estimated impact**: Genuine cross-session learning → compound improvement over time.

#### Phase 4: Automatic Curriculum
**Rationale**: Enables autonomous improvement without human task creation.

1. Build capability gap detection
2. Implement task proposal generation
3. Create feedback loop (proposed → completed → learned)
4. Integrate with Gym for deliberate practice

**Estimated impact**: Self-improving agent that identifies and addresses its own weaknesses.

### 13. Key Insights and Takeaways

1. **The skill library is the highest-value addition**. Voyager's ablation studies show it's the key differentiator. Start here.

2. **Multi-timescale memory solves our biggest limitation**. Without it, each session starts fresh. With it, knowledge accumulates and compounds.

3. **Verification is already strong**. MechaCoder's test-based verification parallels Voyager's self-verification. This is a competitive advantage.

4. **The Gym is our Minecraft**. TerminalBench provides the safe environment for deliberate practice. Use it for skill development.

5. **Start with retrieval, not generation**. Before generating new solutions, try retrieving proven ones. This is more reliable and builds the skill library.

6. **Architecture matters more than model power**. Generative Agents showed an 8 standard deviation improvement from architecture alone. Invest in systems, not just model upgrades.

7. **The curriculum should be automatic, not manual**. Manual task creation doesn't scale. Let the agent identify its own growth areas.

8. **Catastrophic forgetting is manageable**. Nested Learning shows it's not about preventing forgetting - it's about managing where forgetting happens through multi-level memory.

### 14. Open Questions for Future Research

1. **Skill Composition**: How do we enable the agent to compose skills hierarchically, like Voyager's complex skills calling simpler ones?

2. **Skill Deprecation**: When should a skill be deprecated? How do we handle skills that become outdated?

3. **Cross-Project Transfer**: Can skills learned in one project transfer to another? What makes skills generalizable?

4. **Negative Skills**: Should we store "anti-patterns" - things that consistently fail? How do we use them?

5. **Human-in-the-Loop**: When should the automatic curriculum defer to human task creation? What tasks should never be self-proposed?

6. **Exploration vs. Exploitation**: How do we balance exploring new capabilities vs. exploiting known skills?

---

## Appendix A: Research Paper References

### Voyager
- **Paper**: "Voyager: An Open-Ended Embodied Agent with Large Language Models"
- **Authors**: Wang et al. (NVIDIA, Caltech, UT Austin, Stanford, UW Madison)
- **Key insight**: Skill library as executable code enables lifelong learning without gradient updates
- **Local summary**: `docs/research/paper-summaries/voyager-summary.md`

### Nested Learning
- **Paper**: "Nested Learning: The Illusion of Deep Learning Architecture"
- **Authors**: Behrouz et al. (Google Research)
- **Key insight**: Multi-timescale memory prevents catastrophic forgetting
- **Local summary**: `docs/research/paper-summaries/nested-learning-summary.md`

### Generative Agents
- **Paper**: "Generative Agents: Interactive Simulacra of Human Behavior"
- **Authors**: Park et al. (Stanford, Google Research)
- **Key insight**: Memory + Reflection + Planning architecture enables believable long-term behavior
- **Local summary**: `docs/research/paper-summaries/generative-agents-summary.md`

---

## Appendix B: MechaCoder Architecture References

- **Golden Loop v2**: `docs/mechacoder/GOLDEN-LOOP-v2.md`
- **Subagents Overview**: `docs/subagents/README.md`
- **Archivist Spec**: `docs/subagents/archivist.md`
- **Healer Spec**: `docs/subagents/healer.md`
- **Trainer/Gym Spec**: `docs/subagents/gym-trainer.md`
- **Project Spec**: `docs/mechacoder/spec.md`

---

*This analysis was written on December 4, 2025, synthesizing research from Voyager (May 2023), Nested Learning (NeurIPS 2025), and Generative Agents (UIST 2023) with the current MechaCoder architecture.*
