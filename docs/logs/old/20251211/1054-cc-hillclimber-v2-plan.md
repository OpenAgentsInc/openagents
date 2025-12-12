# Plan: Claude Code SDK Architecture for HillClimber/TestGen

## Goal
Create a speculative design doc at `docs/claude/agent-sdk/HILLCLIMBER-CLAUDE-ARCHITECTURE.md` describing how to implement HillClimber/TestGen/MechaCoder using Claude Code SDK primitives.

## File to Create
`docs/claude/agent-sdk/HILLCLIMBER-CLAUDE-ARCHITECTURE.md`

---

## Document Structure

### 1. Overview
- Why Claude Code SDK for HillClimber (larger context, better reasoning, native tools)
- Architecture thesis: Claude as orchestrator, specialized subagents for components
- Key advantage: structured output enables typed contracts between components

### 2. Architecture Diagram (ASCII)
```
┌─────────────────────────────────────────────────────────────┐
│                    CLAUDE ORCHESTRATOR                       │
│  (main query - coordinates everything, has full context)     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   TestGen    │  │  Decomposer  │  │ MetaReasoner │       │
│  │  (subagent)  │  │  (subagent)  │  │  (subagent)  │       │
│  │ haiku model  │  │ haiku model  │  │ sonnet model │       │
│  │ JSON output  │  │ JSON output  │  │ JSON output  │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │  Evaluator   │  │   Monitor    │                         │
│  │  (MCP tool)  │  │ (PreToolUse) │                         │
│  │ runs pytest  │  │   hook       │                         │
│  └──────────────┘  └──────────────┘                         │
│                                                              │
│  Built-in: Read, Edit, Write, Bash, Grep, Glob              │
└─────────────────────────────────────────────────────────────┘
```

### 3. Subagent Definitions

#### TestGen Subagent
```typescript
agents: {
  'testgen': {
    description: 'Generate comprehensive test suites for a task. Use when you need tests to validate a solution.',
    prompt: `You are a test generation specialist. Given a task description, generate 15-30 tests across 5 categories:
    - anti_cheat: Tests that catch hardcoded solutions
    - existence: Tests that verify output file exists with correct format
    - correctness: Tests for correct behavior on typical inputs
    - boundary: Tests for edge cases and limits
    - integration: Tests for interaction between components

    Output ONLY valid JSON matching the schema. No explanation.`,
    tools: ['Read'],  // Only needs to read task description
    model: 'haiku',   // Fast, cheap for generation
  }
}
```

**Structured Output Schema:**
```typescript
const TestGenOutputSchema = z.object({
  tests: z.array(z.object({
    category: z.enum(['anti_cheat', 'existence', 'correctness', 'boundary', 'integration']),
    name: z.string(),
    input: z.string(),
    expected_output: z.string(),
    reasoning: z.string(),
  })),
  reflection: z.string(),
});
```

#### Decomposer Subagent
```typescript
agents: {
  'decomposer': {
    description: 'Break a complex task into subtasks. Use at the start of any multi-step task.',
    prompt: `You are a task decomposition specialist. Break the given task into 3-5 sequential subtasks.
    Each subtask should be completable in one focused session.

    Standard pattern for coding tasks:
    1. understand-requirements (read files, understand constraints)
    2. write-initial-solution (create output file)
    3. test-and-iterate (run tests, fix failures)
    4. final-validation (ensure 100% pass rate)

    Output ONLY valid JSON matching the schema.`,
    tools: ['Read', 'Grep'],
    model: 'haiku',
  }
}
```

**Structured Output Schema:**
```typescript
const DecomposerOutputSchema = z.object({
  subtasks: z.array(z.object({
    id: z.string(),
    goal: z.string(),
    hint: z.string(),
    max_turns: z.number(),
    checkpoint: z.string(),  // How to verify completion
  })),
  input_files: z.array(z.string()),
  output_files: z.array(z.string()),
});
```

#### MetaReasoner Subagent
```typescript
agents: {
  'meta-reasoner': {
    description: 'Analyze run history and propose improvements. Use after multiple failed attempts.',
    prompt: `You are a meta-learning specialist. Given the history of runs and their scores, propose specific config changes to improve performance.

    Guardrails (MUST follow):
    - Max temperature change: ±0.1
    - Max tests per category change: ±1
    - Never go below 2 tests per category

    Output ONLY valid JSON matching the schema.`,
    tools: ['Read'],
    model: 'sonnet',  // Needs better reasoning for meta-analysis
  }
}
```

### 4. MCP Tool Definitions

#### Evaluator Tool (in-process MCP)
```typescript
const evaluatorTool = createSdkMcpServer({
  name: 'evaluator',
  tools: [
    tool('run_tests', 'Run pytest on workspace and return results', {
      workspace: z.string().describe('Path to workspace'),
      test_file: z.string().optional().describe('Specific test file'),
    }, async ({ workspace, test_file }) => {
      // Run pytest in Docker container
      const result = await runPytestInDocker(workspace, test_file);
      return {
        content: [{
          type: 'text',
          text: JSON.stringify({
            passed: result.passed,
            failed: result.failed,
            total: result.total,
            progress: result.passed / result.total,
            failures: result.failures.map(f => ({
              test_name: f.name,
              error: f.error,
              expected: f.expected,
              actual: f.actual,
            })),
          }),
        }],
      };
    }),
  ],
});
```

### 5. Hook Integration

#### Monitor as PreToolUse Hook
```typescript
hooks: {
  PreToolUse: [{
    matcher: 'Bash',  // Only monitor Bash commands
    hooks: [async (input, toolUseId, { signal }) => {
      const command = input.tool_input.command;

      // Check dangerous patterns
      const dangerous = [
        /rm\s+-rf\s+\//, /sudo/, /curl.*\|.*bash/,
        /dd.*of=\/dev/, /mkfs/, />\s*\/dev/
      ];

      for (const pattern of dangerous) {
        if (pattern.test(command)) {
          return {
            continue: false,
            decision: 'block',
            reason: `Blocked dangerous command: ${command}`,
          };
        }
      }

      // Check workspace bounds
      if (!isWithinWorkspace(command, input.cwd)) {
        return {
          continue: false,
          decision: 'block',
          reason: 'Command accesses files outside workspace',
        };
      }

      return { continue: true };
    }],
  }],
}
```

### 6. Parallel Sampling Pattern
```typescript
// Run 3 parallel queries with different temperatures
async function sampleCandidates(task: string, tests: Test[]) {
  const configs = [
    { temp: 0.3, hint: 'Focus on precision' },
    { temp: 0.5, hint: 'Balance precision and creativity' },
    { temp: 0.7, hint: 'Explore creative solutions' },
  ];

  const results = await Promise.all(configs.map(async (config, i) => {
    const workspace = await createTempWorkspace(i);

    const result = await query({
      prompt: `${task}\n\nHint: ${config.hint}`,
      options: {
        cwd: workspace,
        model: 'claude-sonnet-4-5-20250929',
        // Note: temperature not directly exposed, use prompt hints
        maxTurns: 10,
        outputFormat: { type: 'json_schema', schema: SolutionSchema },
      }
    });

    // Evaluate this candidate
    const evalResult = await evaluator.run_tests({ workspace });
    return { workspace, config, evalResult, result };
  }));

  // Return best candidate
  return results.sort((a, b) => b.evalResult.progress - a.evalResult.progress)[0];
}
```

### 7. Orchestration Loop
```typescript
async function hillclimberLoop(task: TerminalBenchTask) {
  // 1. Generate tests via subagent
  const testgenResult = await query({
    prompt: `Generate tests for: ${task.description}`,
    options: {
      agents: { testgen: testgenAgent },
      outputFormat: { type: 'json_schema', schema: TestGenOutputSchema },
    }
  });
  const tests = testgenResult.structured_output.tests;

  // 2. Decompose task
  const decomposition = await query({
    prompt: `Decompose: ${task.description}`,
    options: {
      agents: { decomposer: decomposerAgent },
      outputFormat: { type: 'json_schema', schema: DecomposerOutputSchema },
    }
  });

  // 3. Execute subtasks with evaluation loop
  for (const subtask of decomposition.subtasks) {
    let attempts = 0;
    while (attempts < 3) {
      const candidate = await sampleCandidates(subtask.goal, tests);

      if (candidate.evalResult.progress === 1.0) {
        // All tests pass - move to next subtask
        break;
      }

      // Feed failure info back for next attempt
      attempts++;
    }
  }

  // 4. Meta-reasoning if needed
  if (overallProgress < 1.0) {
    const improvements = await query({
      prompt: `Analyze failures and suggest improvements: ${JSON.stringify(runHistory)}`,
      options: {
        agents: { 'meta-reasoner': metaReasonerAgent },
        outputFormat: { type: 'json_schema', schema: ConfigChangeSchema },
      }
    });
    // Apply improvements to next run
  }
}
```

### 8. Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| TestGen as subagent (not tool) | Needs reasoning to generate good tests, not just execution |
| Evaluator as MCP tool | Pure execution, no reasoning needed, returns structured data |
| Monitor as hook | Intercepts BEFORE execution, can block dangerous actions |
| Haiku for TestGen/Decomposer | Fast, cheap, sufficient for structured generation |
| Sonnet for MetaReasoner | Needs deeper analysis of run patterns |
| Structured output everywhere | Typed contracts between components, no parsing errors |

### 9. Advantages Over Current Rust Implementation

| Aspect | Rust HillClimber | Claude SDK Version |
|--------|------------------|-------------------|
| Context window | Limited (FM) | 200K tokens |
| Error recovery | Rigid retry logic | Claude reasons about failures |
| Decomposition | Fixed 4-subtask pattern | Dynamic based on task |
| Test generation | FM generates tests | Claude with domain knowledge |
| Meta-learning | Rule-based config changes | Claude analyzes patterns |
| Tool creation | Compile-time Rust traits | Runtime MCP tools |

### 10. Anti-Cheating Preserved

All the same guardrails apply:
- NO task-specific hardcoding in prompts
- TestGen discovers tests from description only
- Monitor blocks dangerous commands
- Solutions must pass TestGen tests (not TB2 directly)
- Meta-reasoner has bounded config changes

---

## Alternative: Skills-First Architecture

Based on Anthropic's "Don't Build Agents, Build Skills" talk, an alternative approach uses **skills** instead of subagents.

### Key Insight from Talk
> "Skills are organized collections of files that package composable procedural knowledge for agents. In other words, they're folders."

Skills advantages over subagents:
- **Progressive disclosure**: Only metadata in context, full content loaded on demand
- **Scripts as tools**: Self-documenting, modifiable, live on disk
- **Composability**: Can load hundreds of skills without context bloat
- **Continuous learning**: Claude can create/evolve skills from experience

### Skills-Based HillClimber Architecture

```
.claude/skills/
├── testgen/
│   ├── SKILL.md              # Metadata + instructions
│   ├── generate_tests.py     # Script tool
│   ├── schemas/
│   │   └── test_schema.json  # Output schema
│   └── examples/
│       └── good_tests.json   # Few-shot examples
├── decomposer/
│   ├── SKILL.md
│   ├── decompose_task.py
│   └── patterns/
│       └── standard_phases.md
├── evaluator/
│   ├── SKILL.md
│   ├── run_pytest.py
│   └── docker/
│       └── Dockerfile
└── meta-reasoner/
    ├── SKILL.md
    ├── analyze_runs.py
    └── guardrails.md
```

### TestGen Skill Example

**`.claude/skills/testgen/SKILL.md`:**
```markdown
---
name: testgen
description: Generate comprehensive test suites for Terminal-Bench tasks
triggers:
  - "generate tests"
  - "create test suite"
  - "write tests for"
---

# TestGen Skill

Generate 15-30 tests across 5 categories for any task.

## Categories
- anti_cheat: Tests that catch hardcoded solutions
- existence: Verify output exists with correct format
- correctness: Typical input/output pairs
- boundary: Edge cases and limits
- integration: Component interactions

## Usage
1. Read task description from workspace
2. Run `./generate_tests.py --task <description>`
3. Output: tests.json matching schema in schemas/test_schema.json

## Scripts
- `generate_tests.py` - Main generation script
- `validate_tests.py` - Validate test quality

## Examples
See `examples/good_tests.json` for reference output.
```

**`.claude/skills/testgen/generate_tests.py`:**
```python
#!/usr/bin/env python3
"""Generate tests for a task description."""
import json
import sys

def generate_tests(task_description: str) -> dict:
    # Claude will execute this script and use the output
    # The script provides structure, Claude provides reasoning
    categories = ['anti_cheat', 'existence', 'correctness', 'boundary', 'integration']

    tests = []
    for category in categories:
        # Placeholder - Claude fills in via tool use
        tests.append({
            "category": category,
            "name": f"test_{category}_placeholder",
            "input": "",
            "expected_output": "",
            "reasoning": ""
        })

    return {"tests": tests, "reflection": ""}

if __name__ == "__main__":
    task = sys.argv[1] if len(sys.argv) > 1 else ""
    print(json.dumps(generate_tests(task), indent=2))
```

### Hybrid Approach: Skills + Subagents

The most powerful architecture combines both:

| Component | Implementation | Rationale |
|-----------|---------------|-----------|
| TestGen | **Skill** | Procedural knowledge, scripts, examples |
| Decomposer | **Skill** | Pattern library, reusable phases |
| Evaluator | **MCP Tool** | Pure execution, Docker integration |
| Monitor | **Hook** | Must intercept before execution |
| MetaReasoner | **Subagent** | Needs deep reasoning, not just scripts |
| Orchestrator | **Main Claude** | Coordinates, loads skills as needed |

### Skills Enable Continuous Learning

From the talk:
> "Our goal is that Claude on Day 30 of working with you is going to be a lot better than Claude on Day 1."

For HillClimber, this means:
1. Claude runs testgen skill, gets 70% pass rate
2. Claude analyzes what worked, updates skill
3. Next run: skill includes learned patterns
4. Over time: skill evolves to task domain

**Example evolution:**
```
Run 1: Generic test generation → 60% TB2 pass
Run 5: Added regex-specific patterns → 75% pass
Run 10: Refined anti-cheat tests → 85% pass
Run 20: Skill now expert at regex tasks → 95% pass
```

### Skill Creation Flow

```typescript
// After successful run, Claude can create/update skill
if (runResult.passRate > 0.9) {
  await query({
    prompt: `Create a skill from this successful approach:
    Task: ${task.description}
    Solution: ${runResult.solution}
    Tests that passed: ${runResult.passingTests}

    Save as a skill in .claude/skills/learned/${task.domain}/`,
    options: {
      settingSources: ['project'],  // Load existing skills
      tools: ['Read', 'Write', 'Edit'],
    }
  });
}
```

### Key Differences: Skills vs Subagents

| Aspect | Subagents | Skills |
|--------|-----------|--------|
| Context cost | Always loaded | Progressive disclosure |
| Modification | Requires code change | Claude can edit files |
| Learning | Static definitions | Evolves with use |
| Sharing | SDK config | Copy folders |
| Complexity | Simple definitions | Rich folder structure |
| Best for | Reasoning tasks | Procedural knowledge |

### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    CLAUDE ORCHESTRATOR                       │
│  Loads skills on demand, coordinates execution               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  SKILLS (folders, loaded on demand)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   testgen/   │  │ decomposer/  │  │  evaluator/  │       │
│  │  SKILL.md    │  │  SKILL.md    │  │  SKILL.md    │       │
│  │  scripts/    │  │  patterns/   │  │  docker/     │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                              │
│  SUBAGENT (for deep reasoning)         HOOK (safety)        │
│  ┌──────────────┐                      ┌──────────────┐     │
│  │ MetaReasoner │                      │   Monitor    │     │
│  │   sonnet     │                      │ PreToolUse   │     │
│  └──────────────┘                      └──────────────┘     │
│                                                              │
│  Built-in: Read, Edit, Write, Bash, Grep, Glob              │
└─────────────────────────────────────────────────────────────┘
```

This hybrid leverages:
- **Skills** for procedural knowledge (testgen, decomposer, evaluator)
- **Subagent** for deep reasoning (meta-reasoner)
- **Hook** for safety interception (monitor)
- **MCP** for external integrations (Docker, pytest)
