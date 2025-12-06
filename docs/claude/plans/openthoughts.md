# OpenThoughts-Agent Integration Plan for MechaCoder/OpenAgents/TerminalBench

## Executive Summary

Integrate OpenThoughts-Agent's **TB-Dev benchmark** and **SFT traces** into the OpenAgents/MechaCoder/TerminalBench loop to improve Apple FM agent performance - with the ultimate goal of hitting #1 on Terminal-Bench using on-device inference.

**Focus**: No-gradient learning with Apple FM. No GPU training required.

## Key Resources from OpenThoughts

| Resource | Source | Size | Purpose |
|----------|--------|------|---------|
| TB-Dev | `open-thoughts/OpenThoughts-TB-dev` | 70 tasks | Easier benchmark correlated with TB-2.0 (r=0.85) |
| SFT Traces | `open-thoughts/OpenThoughts-Agent-v1-SFT` | 15k | High-quality trajectories for skill seeding |

---

## Phase 1: Progressive Benchmark Integration (TB-Dev) - PRIMARY

### 1.1 Goal

Add OpenThoughts-TB-Dev as an intermediate evaluation tier between FM-mini (7 tasks) and TB-2.0 (89 tasks). This provides measurable signal for incremental improvements on small/local models.

### 1.2 Rationale

- Small models (8B) and Apple FM get ~0% on TB-2.0 due to long-horizon complexity
- TB-Dev (70 easier tasks) has r=0.85 correlation with TB-2.0
- Allows tracking real progress before graduating to full benchmark
- OpenThoughts validated this approach with their OpenThinker-Agent-v1

### 1.3 Tier Progression Design

```
Tier 0: FM-mini (7 tasks)   - 100% threshold - Sanity check
Tier 1: TB-Dev (70 tasks)   - 70% threshold  - Intermediate milestone
Tier 2: TB-10/30 (10-30)    - 80% threshold  - TB-2.0 subset
Tier 3: TB-89 (89 tasks)    - N/A            - Full benchmark (goal)
```

### 1.4 Implementation Tasks

#### Task 1.4.1: Download and Analyze TB-Dev Dataset

**Description**: Download `open-thoughts/OpenThoughts-TB-dev` from HuggingFace and analyze the task format, categories, and difficulty distribution.

**Acceptance Criteria**:
- Dataset downloaded to `data/openthoughts/tb-dev/`
- Analysis document with: task count, categories, verification types, difficulty breakdown
- Sample tasks reviewed for compatibility with our schema

**Files**: `data/openthoughts/tb-dev/` (new directory)

---

#### Task 1.4.2: Create TB-Dev Task Importer

**Description**: Build an importer that converts OpenThoughts TB-Dev tasks to our Terminal-Bench schema format.

**Implementation Details**:
```typescript
// src/bench/tb-dev-importer.ts

interface OpenThoughtsTBDevTask {
  id: string;
  prompt: string;
  difficulty: "easy" | "medium";
  category: string;
  verification: { type: string; expected?: string; command?: string };
  correlation_score: number;
}

export const importTBDevDataset = (
  sourcePath: string
): Effect.Effect<TerminalBenchSuite, ImportError, FileSystem>

export const mapTaskToTBSchema = (
  task: OpenThoughtsTBDevTask
): TerminalBenchTask
```

**Acceptance Criteria**:
- Importer handles all TB-Dev task formats
- Tasks mapped to `TerminalBenchTask` schema
- Verification commands preserved
- Category tags maintained for analysis

**Files to Create**:
- `src/bench/tb-dev-importer.ts`

**Files to Read First**:
- `src/bench/terminal-bench.ts` (existing schema)

---

#### Task 1.4.3: Generate TB-Dev Suite JSON

**Description**: Run the importer to generate `tasks/tb-dev-suite.json` containing all 70 TB-Dev tasks in our format.

**Acceptance Criteria**:
- `tasks/tb-dev-suite.json` with 70 tasks
- All tasks have valid verification
- Schema matches `tasks/terminal-bench-2.json` format
- Metadata includes source attribution and correlation info

**Files to Create**:
- `tasks/tb-dev-suite.json`

---

#### Task 1.4.4: Extend TB_SUBSETS Schema

**Description**: Add `FM_MINI` and `TB_DEV` to the `TB_SUBSETS` constant with tier metadata.

**Implementation Details**:
```typescript
// src/trainer/schema.ts

export const TB_SUBSETS = {
  FM_MINI: {
    name: "FM Mini Suite",
    description: "7 quick regression tasks for FM validation",
    count: 7,
    tier: 0,
    suitePath: "tasks/fm-mini-suite.json",
    progressionThreshold: 1.0,  // 100% to progress
  },
  TB_DEV: {
    name: "OpenThoughts-TB-Dev",
    description: "70 easier tasks strongly correlated with TB-2.0 (r=0.85)",
    count: 70,
    tier: 1,
    suitePath: "tasks/tb-dev-suite.json",
    progressionThreshold: 0.7,  // 70% to progress
    correlationWithTB2: 0.85,
  },
  TB_10: { /* existing */ tier: 2, progressionThreshold: 0.8 },
  TB_30: { /* existing */ tier: 2, progressionThreshold: 0.8 },
  TB_89: { /* existing */ tier: 3 },
} as const;
```

**Acceptance Criteria**:
- New tiers added without breaking existing code
- Type definitions updated
- Unit tests pass

**Files to Modify**:
- `src/trainer/schema.ts`

---

#### Task 1.4.5: Create Progressive Evaluator Service

**Description**: Build the orchestrator that runs benchmark tiers sequentially with auto-progression based on thresholds.

**Implementation Details**:
```typescript
// src/bench/progressive-evaluator.ts

export interface ProgressionConfig {
  startTier: TBSubset;
  maxTier: TBSubset;
  autoProgress: boolean;
  extractSkillsOnPass: boolean;
  createReflectionsOnFail: boolean;
}

export interface TierResult {
  tier: number;
  subset: TBSubset;
  passRate: number;
  tasksCompleted: number;
  tasksPassed: number;
  skillsLearned: number;
  durationMs: number;
  readyToProgress: boolean;
  estimatedTB2Score?: number;  // For TB-Dev tier
}

export class ProgressiveEvaluator {
  runProgressiveSweep(
    config: ProgressionConfig
  ): Effect.Effect<TierResult[], EvaluatorError, ModelAdapter | SkillService>

  checkProgression(result: TierResult): boolean

  estimateTB2Score(tbDevResult: TierResult): { score: number; confidence: number }
}
```

**Acceptance Criteria**:
- Runs tiers in order (0 → 1 → 2 → 3)
- Auto-progresses when threshold met
- Integrates with existing skill/memory/reflexion services
- Returns structured results for reporting

**Files to Create**:
- `src/bench/progressive-evaluator.ts`

**Files to Read First**:
- `src/bench/model-adapter.ts` (FM runner)
- `src/learning/loop.ts` (learning integration)

---

#### Task 1.4.6: Add Tier Tracking to Episode Schema

**Description**: Extend the Episode schema to track which tier/subset a run belongs to, enabling tier-level analysis.

**Implementation Details**:
```typescript
// src/bench/episode-store.ts - add fields

export const Episode = S.Struct({
  // ... existing fields ...

  tier: S.optional(S.Number),
  subset: S.optional(S.String),
  tbDevMetrics: S.optional(S.Struct({
    estimatedTB2Score: S.Number,
    correlationConfidence: S.Number,
    categoryBreakdown: S.Record({ key: S.String, value: S.Number }),
  })),
});
```

**Acceptance Criteria**:
- New fields added with backwards compatibility
- Existing episodes can be read without error
- New episodes include tier metadata

**Files to Modify**:
- `src/bench/episode-store.ts`

---

#### Task 1.4.7: Create Tier Performance Analyzer

**Description**: Build analyzer that identifies weak categories, tracks improvement velocity, and generates recommendations.

**Implementation Details**:
```typescript
// src/training/tier-analyzer.ts

export interface TierAnalysis {
  tier: TBSubset;
  passRate: number;
  categoryPerformance: Record<string, {
    passRate: number;
    avgTurns: number;
    skillsUsed: string[];
    suggestedSkills: string[];
  }>;
  skillEffectiveness: {
    topSkills: Array<{ skillId: string; successRate: number }>;
    gapCategories: string[];
  };
  readyForNextTier: boolean;
  estimatedNextTierScore: number;
  recommendations: string[];
}

export const analyzeTierPerformance = (
  episodes: Episode[],
  tier: TBSubset,
  existingSkills: Skill[]
): TierAnalysis

export const estimateTB2ScoreFromTBDev = (
  tbDevPassRate: number,
  correlationFactor?: number  // default 0.85
): { score: number; confidence: number }
```

**Acceptance Criteria**:
- Aggregates episodes by tier
- Identifies weak vs strong categories
- Calculates skill effectiveness
- Generates actionable recommendations
- TB-Dev → TB-2.0 correlation estimation

**Files to Create**:
- `src/training/tier-analyzer.ts`

---

#### Task 1.4.8: Create Progressive Reporter

**Description**: Build reporter that generates comparison reports across tiers with improvement tracking.

**Implementation Details**:
```typescript
// src/bench/progressive-reporter.ts

export interface ProgressiveReport {
  runId: string;
  model: string;
  timestamp: string;
  tiers: Array<{
    name: TBSubset;
    tier: number;
    passRate: number;
    tasksCompleted: number;
    avgTurns: number;
    skillsUsed: number;
    skillsLearned: number;
    readyToProgress: boolean;
  }>;
  correlation: {
    tbDevPassRate: number;
    estimatedTB2Score: number;
    confidence: number;
  };
  recommendations: string[];
}

export const generateProgressiveReport = (
  episodes: Episode[],
  config: ProgressionConfig
): ProgressiveReport

export const formatAsMarkdown = (report: ProgressiveReport): string
```

**Acceptance Criteria**:
- Generates structured JSON report
- Markdown formatter for human readability
- Includes TB-Dev → TB-2.0 estimation
- Tracks improvement velocity if historical data available

**Files to Create**:
- `src/bench/progressive-reporter.ts`

---

#### Task 1.4.9: Add Progressive CLI Flags to tbench-iterate

**Description**: Extend the benchmark CLI with progressive evaluation flags.

**Implementation Details**:
```typescript
// src/cli/tbench-iterate.ts - add flags

const args = parseArgs({
  // ... existing ...

  // Progressive evaluation
  progressive: { type: "boolean" },
  "start-tier": { type: "string" },  // fm-mini | tb-dev | tb-10 | tb-89
  "max-tier": { type: "string" },
  "tier-threshold": { type: "string" },  // JSON override
  "auto-progress": { type: "boolean", default: true },
  "tier-report": { type: "boolean" },
  "estimate-tb2": { type: "boolean" },
});
```

**Usage Example**:
```bash
bun src/cli/tbench-iterate.ts \
  --progressive \
  --start-tier fm-mini \
  --max-tier tb-89 \
  --model fm \
  --skills --memory --reflect --learn \
  --iterations 10 \
  --tier-report
```

**Acceptance Criteria**:
- New flags work correctly
- Progressive mode integrates with existing learning flags
- Tier report generated when requested
- Help text updated

**Files to Modify**:
- `src/cli/tbench-iterate.ts`

---

#### Task 1.4.10: Integrate TB-Dev into Learning Loop

**Description**: Update the learning loop to recognize TB-Dev as a tier and handle tier-based skill extraction.

**Implementation Details**:
```typescript
// src/learning/loop.ts - modify

const TIER_PROGRESSION: Record<TBSubset, TBSubset | null> = {
  FM_MINI: "TB_DEV",
  TB_DEV: "TB_10",
  TB_10: "TB_30",
  TB_30: "TB_89",
  TB_89: null,
};

// Add tier-specific learning config
const getTierLearningConfig = (tier: TBSubset) => ({
  extractSkillsOnPass: true,
  createReflectionsOnFail: true,
  minSuccessRateForSkill: tier === "TB_DEV" ? 0.5 : 0.7,
});
```

**Acceptance Criteria**:
- Learning loop handles tier progression
- Skills extracted from TB-Dev successes
- Reflections created from TB-Dev failures
- Tier transition triggers analysis

**Files to Modify**:
- `src/learning/loop.ts`

---

#### Task 1.4.11: Add TB-Dev Config to ProjectConfig

**Description**: Extend `.openagents/project.json` schema to support progressive evaluation configuration.

**Implementation Details**:
```typescript
// src/tasks/schema.ts - extend TBenchConfig

export const TBenchConfig = S.Struct({
  // ... existing ...

  progressiveEvaluation: S.optionalWith(S.Boolean, { default: () => false }),
  progressive: S.optional(S.Struct({
    startTier: S.optionalWith(S.String, { default: () => "FM_MINI" }),
    maxTier: S.optionalWith(S.String, { default: () => "TB_89" }),
    thresholds: S.optional(S.Record({ key: S.String, value: S.Number })),
    autoProgress: S.optionalWith(S.Boolean, { default: () => true }),
  })),
  tbDev: S.optional(S.Struct({
    enabled: S.optionalWith(S.Boolean, { default: () => true }),
    correlationFactor: S.optionalWith(S.Number, { default: () => 0.85 }),
  })),
});
```

**Acceptance Criteria**:
- Schema validates correctly
- Defaults are sensible
- Existing configs remain valid

**Files to Modify**:
- `src/tasks/schema.ts`

---

#### Task 1.4.12: Write Tests for Progressive Evaluation

**Description**: Add comprehensive tests for TB-Dev importer, progressive evaluator, and tier analyzer.

**Test Cases**:
- TB-Dev task mapping
- Tier progression logic
- TB-2.0 score estimation
- Skill extraction from tier runs
- Progressive reporter output

**Files to Create**:
- `src/bench/tb-dev-importer.test.ts`
- `src/bench/progressive-evaluator.test.ts`
- `src/training/tier-analyzer.test.ts`

---

#### Task 1.4.13: Document Progressive Evaluation

**Description**: Update documentation with progressive evaluation usage and TB-Dev integration.

**Content**:
- How to run progressive benchmark
- Tier progression thresholds
- TB-Dev → TB-2.0 correlation explanation
- Example workflows

**Files to Modify**:
- `docs/mechacoder/terminal-bench.md`

---

## Phase 2: Skill Seeding from SFT Traces - SECONDARY

### 2.1 Goal

Bootstrap our skill library with high-quality patterns from OpenThoughts' 15k curated SFT traces, accelerating FM learning without gradient-based training.

### 2.2 How Skills Work in Our System (Background)

Our skills system implements **Voyager-style no-gradient learning**. Here's the lifecycle:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        SKILL LIFECYCLE                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. STORAGE: .openagents/skills/library.jsonl (JSONL format)        │
│     - One skill per line                                            │
│     - Fields: id, name, description, code, category, embedding,     │
│               successRate, usageCount, status, source               │
│                                                                     │
│  2. RETRIEVAL: Semantic search via embeddings                       │
│     - FM gets task: "Fix import error in main.ts"                   │
│     - Query skill library: cosine similarity to task description    │
│     - Return top-5 skills with similarity > 0.3                     │
│                                                                     │
│  3. INJECTION: Skills → FM system prompt                            │
│     - Format: "### {skill.name} [{successRate}% success]"           │
│     - Include: description, parameters, code example                │
│     - FM sees these patterns when generating solutions              │
│                                                                     │
│  4. TRACKING: Usage & success rates                                 │
│     - Record which skills were used per task                        │
│     - Update successRate via exponential moving average             │
│     - Promote (draft→active), demote, or prune based on performance │
│                                                                     │
│  5. LEARNING: Extract new skills from successful episodes           │
│     - EpisodeLearner processes completed runs                       │
│     - Successful task outputs → new draft skills                    │
│     - Skills evolve: draft → active → archived                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Current Skill Sources:**
| Source | Count | Examples |
|--------|-------|----------|
| Bootstrap (primitives) | ~40 | readFile, writeFile, gitCommit, runTest |
| Learned (from episodes) | Variable | Patterns extracted from successful TB runs |

**Skill Schema (simplified):**
```typescript
interface Skill {
  id: string;              // "skill-fix-import-error-v1"
  name: string;            // "Fix Import Error"
  description: string;     // For embedding/retrieval
  code: string;            // Executable pattern
  category: SkillCategory; // file_operations, debugging, testing, git, shell
  embedding: number[];     // 768-dim vector for semantic search
  successRate: number;     // 0-1, updated via EMA
  usageCount: number;      // Times used
  source: "bootstrap" | "learned" | "manual" | "openthoughts-sft";
  status: "draft" | "active" | "archived";
}
```

### 2.3 How SFT Seeding Fits In

**The Connection:**

OpenThoughts' 15k SFT traces are **high-quality agent trajectories** (question → thinking → answer). We convert these to our Skill format, giving FM access to 500+ proven patterns **without training**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                  SFT SEEDING INTEGRATION                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  OpenThoughts SFT Trace:                                            │
│  {                                                                  │
│    "question": "List files larger than 1MB in /data",               │
│    "thinking": "I need to use find with -size...",                  │
│    "answer": "find /data -type f -size +1M",                        │
│    "domain": "nl2bash",                                             │
│    "verified": true                                                 │
│  }                                                                  │
│                              │                                      │
│                              ▼                                      │
│  Converted to Skill:                                                │
│  {                                                                  │
│    "id": "skill-nl2bash-find-large-files-001",                      │
│    "name": "Find Files by Size",                                    │
│    "description": "List files larger than 1MB in /data",            │
│    "code": "find /data -type f -size +1M",                          │
│    "category": "shell",                                             │
│    "embedding": [0.12, -0.34, ...],  // Generated from description  │
│    "successRate": 1.0,               // Verified trace = 100%       │
│    "source": "openthoughts-sft",                                    │
│    "status": "active"                // Skip draft (pre-verified)   │
│  }                                                                  │
│                              │                                      │
│                              ▼                                      │
│  Stored: .openagents/skills/library.jsonl                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**What Happens When FM Runs a Task:**

```
Task: "Find all log files over 10MB in /var/log"

Step 1: Query skill library (semantic search)
        → Match: "Find Files by Size" (similarity: 0.89)
        → Match: "Glob Files" (similarity: 0.72)
        → Match: "List Directory" (similarity: 0.65)

Step 2: Inject into FM prompt:
        "## Relevant Skills
         ### Find Files by Size [100% success]
         Description: List files larger than 1MB in /data
         Code: find /data -type f -size +1M

         ### Glob Files [78% success]
         ..."

Step 3: FM generates solution, referencing patterns:
        "find /var/log -name '*.log' -size +10M"

Step 4: Track: skill "Find Files by Size" was useful
        → Increment usageCount
        → Update successRate based on task outcome
```

**Why This Works (Voyager Paper Results):**
- Skills as code examples are more reliable than natural language
- Semantic retrieval finds relevant patterns even for novel tasks
- No training required - just prompt injection
- Skills evolve: successful ones get promoted, failures get pruned

### 2.4 Rationale for SFT Seeding

| Without SFT Seeding | With SFT Seeding |
|---------------------|------------------|
| ~40 bootstrap skills | 500+ skills |
| FM must invent patterns | FM sees proven patterns |
| Learning from scratch | Pre-seeded with expertise |
| Slow improvement | Faster baseline performance |

**Key Advantage**: OpenThoughts traces are **verified** (they ran in sandbox and passed). By converting them to skills, we give FM access to patterns that are known to work.

### 2.5 Implementation Tasks

#### Task 2.5.1: Download and Analyze SFT Dataset

**Description**: Download `open-thoughts/OpenThoughts-Agent-v1-SFT` and analyze trace format.

**Acceptance Criteria**:
- Dataset downloaded to `data/openthoughts/sft/`
- Analysis: trace count, domains (NL2Bash, InferredBugs), format schema
- Sample traces reviewed

**Files**: `data/openthoughts/sft/` (new directory)

---

#### Task 2.5.2: Define SFT Trace Schema

**Description**: Create Effect Schema for OpenThoughts SFT trace format.

**Implementation Details**:
```typescript
// src/skills/importers/schema.ts

export const SFTTrace = S.Struct({
  question: S.String,       // Task description
  thinking: S.optional(S.String),  // Chain-of-thought
  answer: S.String,         // Solution code/command
  domain: S.String,         // "nl2bash" | "inferredbugs" | etc
  source: S.String,
  verified: S.optional(S.Boolean),
});

export const SFTDataset = S.Array(SFTTrace);
```

**Files to Create**:
- `src/skills/importers/schema.ts`

---

#### Task 2.5.3: Create Quality Filter

**Description**: Build filter pipeline for SFT traces before skill conversion.

**Implementation Details**:
```typescript
// src/skills/importers/quality-filter.ts

export interface QualityConfig {
  minAnswerLength: number;     // 10 - filter trivial
  maxAnswerLength: number;     // 2000 - fit FM context
  requireVerified: boolean;    // true
  deduplicateSimilarity: number;  // 0.9
  excludeDomains?: string[];
}

export const filterTraces = (
  traces: SFTTrace[],
  config: QualityConfig
): Effect.Effect<SFTTrace[], FilterError>
```

**Files to Create**:
- `src/skills/importers/quality-filter.ts`

---

#### Task 2.5.4: Create SFT-to-Skill Converter

**Description**: Build converter that maps SFT traces to our Skill schema.

**Implementation Details**:
```typescript
// src/skills/importers/sft-importer.ts

const DOMAIN_TO_CATEGORY: Record<string, SkillCategory> = {
  "nl2bash": "shell",
  "inferredbugs": "debugging",
  "file_ops": "file_operations",
  "git": "git",
};

export const convertTraceToSkill = (
  trace: SFTTrace
): Effect.Effect<Skill, ConversionError>

export const importSFTDataset = (
  datasetPath: string,
  qualityConfig: QualityConfig
): Effect.Effect<Skill[], ImportError, FileSystem | EmbeddingService>
```

**Skill Extraction Strategy**:
1. Extract answer as `code`
2. Use question as `description`
3. Map domain to `category`
4. Generate embedding for similarity search
5. Set `source: "openthoughts-sft"`

**Files to Create**:
- `src/skills/importers/sft-importer.ts`

**Files to Read First**:
- `src/skills/schema.ts` (target format)
- `src/training/episode-learner.ts` (reference pattern)

---

#### Task 2.5.5: Create Import CLI

**Description**: Build CLI tool for importing SFT data into skill library.

**Implementation Details**:
```typescript
// src/cli/import-sft.ts

const args = parseArgs({
  source: { type: "string" },     // HF dataset or local path
  domain: { type: "string" },     // all | nl2bash | inferredbugs
  "min-quality": { type: "string" },
  "max-skills": { type: "string" },
  output: { type: "string" },
  "dry-run": { type: "boolean" },
});
```

**Usage**:
```bash
bun src/cli/import-sft.ts \
  --source open-thoughts/OpenThoughts-Agent-v1-SFT \
  --domain all \
  --min-quality 0.7 \
  --output .openagents/skills/imported/
```

**Files to Create**:
- `src/cli/import-sft.ts`

---

#### Task 2.5.6: Add importFromSFT to SkillService

**Description**: Extend SkillService with method to import skills from SFT traces.

**Implementation Details**:
```typescript
// src/skills/service.ts - add method

export class SkillService {
  // ... existing methods ...

  importFromSFT(
    traces: SFTTrace[],
    config: ImportConfig
  ): Effect.Effect<{ imported: number; skipped: number; duplicates: number }>
}
```

**Acceptance Criteria**:
- Deduplicates against existing skills
- Respects category quotas
- Reports import statistics

**Files to Modify**:
- `src/skills/service.ts`

---

#### Task 2.5.7: Implement Context-Optimized Skill Format

**Description**: Create compressed skill format for FM's limited context (~1100 chars).

**Implementation Details**:
```typescript
// src/skills/format.ts

export const formatSkillsForFM = (
  skills: Skill[],
  maxChars: number = 300
): string => {
  // Compressed format:
  // 1. Fix Import Error (debug, 94%)
  //    Pattern: import { X } from './path.js';
}

// Context budget:
// System prompt: 400
// Task: 200
// Skills: 300 (top 3)
// Memory: 150 (top 2)
// Buffer: 50
```

**Files to Create**:
- `src/skills/format.ts`

---

#### Task 2.5.8: Add Memory Seeding from SFT

**Description**: Extract chain-of-thought from SFT traces as semantic memories.

**Implementation Details**:
```typescript
// src/memory/service.ts - add method

export class MemoryService {
  seedFromSFT(
    traces: SFTTrace[]
  ): Effect.Effect<{ memoriesCreated: number }>

  // For traces with 'thinking' field:
  // - Create semantic memory from reasoning
  // - Link to corresponding imported skill
}
```

**Files to Modify**:
- `src/memory/service.ts`

---

#### Task 2.5.9: Write Tests for SFT Import

**Description**: Test SFT import pipeline.

**Test Cases**:
- Trace parsing
- Quality filtering
- Skill conversion
- Deduplication
- Memory seeding

**Files to Create**:
- `src/skills/importers/sft-importer.test.ts`
- `src/skills/importers/quality-filter.test.ts`

---

## Architecture Overview

```
                    OpenThoughts Data
                          │
          ┌───────────────┴───────────────┐
          │                               │
          ▼                               ▼
    SFT Traces (15k)                 TB-Dev (70)
          │                               │
          ▼                               ▼
    ┌─────────────┐               ┌───────────────┐
    │Skill Seeding│               │  Progressive  │
    │ (Phase 2)   │               │  Evaluation   │
    │             │               │  (Phase 1)    │
    └──────┬──────┘               └───────┬───────┘
           │                              │
           └──────────────┬───────────────┘
                          ▼
              ┌─────────────────────────┐
              │  OpenAgents Learning    │
              │  (Skills + Memory +     │
              │   Reflexion + ATIF)     │
              └───────────┬─────────────┘
                          │
                          ▼
                    ┌───────────┐
                    │ Apple FM  │
                    │(on-device)│
                    └───────────┘
```

---

## CLI Usage Summary

### Progressive Evaluation (Phase 1)
```bash
# Run progressive benchmark with learning
bun src/cli/tbench-iterate.ts \
  --progressive \
  --start-tier fm-mini \
  --max-tier tb-89 \
  --model fm \
  --skills --memory --reflect --learn \
  --iterations 10 \
  --tier-report

# Expected output:
# Tier 0 (FM-mini):  7/7 (100%) ✓ Progress
# Tier 1 (TB-Dev):  52/70 (74%) ✓ Progress
# Tier 2 (TB-10):   8/10 (80%) ✓ Progress
# Tier 3 (TB-89):  35/89 (39%) - Continue
#
# TB-2.0 Estimate: 63% [from TB-Dev correlation]
# Skills Learned: 23 new skills
```

### SFT Import (Phase 2)
```bash
# Import OpenThoughts SFT traces as skills
bun src/cli/import-sft.ts \
  --source open-thoughts/OpenThoughts-Agent-v1-SFT \
  --domain all \
  --min-quality 0.7

# Verify skill library
bun src/skills/service.ts stats
# Output: 542 skills (27 bootstrap + 515 imported)
```

---

## Success Metrics

| Metric | Current | Phase 1 Target | Phase 2 Target |
|--------|---------|----------------|----------------|
| Skill library size | 27 | 100+ (from TB-Dev) | 500+ (with SFT) |
| FM mini-suite | ~70% | 85%+ | 90%+ |
| TB-Dev (70 tasks) | N/A | 50%+ | 60%+ |
| TB-2.0 (89 tasks) | ~0% (FM) | 15%+ | 25%+ |
| TB-2.0 (Claude Code) | ~45% | - | - |

---

## Implementation Order

1. **Phase 1 Tasks 1.4.1-1.4.5** - Core TB-Dev integration (get dataset, import, schema, evaluator)
2. **Phase 1 Tasks 1.4.6-1.4.9** - Analysis and CLI (tier tracking, analyzer, reporter, CLI)
3. **Phase 1 Tasks 1.4.10-1.4.13** - Integration and docs (learning loop, config, tests, docs)
4. **Phase 2 Tasks 2.3.1-2.3.5** - SFT import pipeline
5. **Phase 2 Tasks 2.3.6-2.3.9** - Integration and tests

---

## Critical Files to Read Before Implementation

| File | Purpose |
|------|---------|
| `src/bench/terminal-bench.ts` | TB task schema |
| `src/trainer/schema.ts` | TB_SUBSETS, training config |
| `src/skills/schema.ts` | Skill format |
| `src/skills/service.ts` | SkillService API |
| `src/training/episode-learner.ts` | Skill extraction pattern |
| `src/bench/model-adapter.ts` | FM runner with skill injection |
| `src/cli/tbench-iterate.ts` | Benchmark CLI |
| `src/learning/loop.ts` | Learning orchestration |
| `src/bench/episode-store.ts` | Episode persistence |

---

## Alignment with Stakes

From `docs/local/stakes.md`:

> Hit #1 on Terminal-Bench using Apple on-device Foundation Model

This plan supports that goal by:

1. **TB-Dev as stepping stone** - Measure incremental progress toward TB-2.0
2. **Skill seeding** - Bootstrap FM with 15k high-quality patterns
3. **No-gradient learning** - Architecture > model size
4. **Correlation tracking** - TB-Dev r=0.85 with TB-2.0 predicts real performance

**Key Insight**: By combining TB-Dev's intermediate benchmark with skill seeding from SFT traces, we can systematically improve FM's effective capability without any training - proving that architecture and accumulated knowledge matter more than raw model scale.
