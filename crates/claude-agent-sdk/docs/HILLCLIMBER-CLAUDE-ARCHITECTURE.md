# HillClimber on Claude Agent SDK

> Speculative architecture for implementing HillClimber/TestGen/MechaCoder using Claude Code SDK primitives (subagents, skills, tools, hooks, structured output).

---

## Overview

### Why Claude Code SDK for HillClimber?

The current Rust HillClimber uses Apple Foundation Model (FM) with limited context. Claude Code SDK offers:

| Advantage | Impact |
|-----------|--------|
| 200K token context | Full task history, richer prompts |
| Native tool ecosystem | Read, Edit, Bash, Grep built-in |
| Subagents | Specialized components with model selection |
| Skills | Procedural knowledge that evolves |
| Hooks | Safety interception before execution |
| Structured output | Typed contracts, no parsing errors |

### Architecture Thesis

**Claude as orchestrator, specialized skills/subagents for components.**

The main Claude query coordinates everything while:
- **Skills** handle procedural knowledge (testgen, decomposer)
- **Subagents** handle deep reasoning (meta-reasoner)
- **Hooks** intercept dangerous operations (monitor)
- **MCP tools** provide execution (evaluator)

---

## Recommended Architecture: Hybrid Skills + Subagents

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

### Component Mapping

| Component | Implementation | Rationale |
|-----------|---------------|-----------|
| TestGen | **Skill** | Procedural knowledge, scripts, examples |
| Decomposer | **Skill** | Pattern library, reusable phases |
| Evaluator | **MCP Tool** | Pure execution, Docker integration |
| Monitor | **Hook** | Must intercept before execution |
| MetaReasoner | **Subagent** | Needs deep reasoning, not just scripts |
| Orchestrator | **Main Claude** | Coordinates, loads skills as needed |

---

## Skills Implementation

Based on Anthropic's "Don't Build Agents, Build Skills" talk:

> "Skills are organized collections of files that package composable procedural knowledge for agents. In other words, they're folders."

### Skills Directory Structure

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
└── learned/
    └── regex-log/            # Skills learned from successful runs
        └── SKILL.md
```

### TestGen Skill

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

## Anti-Cheating
- DO NOT include task-specific patterns
- Tests must be derived from task DESCRIPTION only
- No hardcoded expected outputs from TB2

## Examples
See `examples/good_tests.json` for reference output.
```

**`schemas/test_schema.json`:**
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "tests": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "category": {
            "type": "string",
            "enum": ["anti_cheat", "existence", "correctness", "boundary", "integration"]
          },
          "name": { "type": "string" },
          "input": { "type": "string" },
          "expected_output": { "type": "string" },
          "reasoning": { "type": "string" }
        },
        "required": ["category", "name", "input", "expected_output"]
      }
    },
    "reflection": { "type": "string" }
  },
  "required": ["tests"]
}
```

### Decomposer Skill

**`.claude/skills/decomposer/SKILL.md`:**
```markdown
---
name: decomposer
description: Break complex tasks into sequential subtasks
triggers:
  - "decompose task"
  - "break down"
  - "plan implementation"
---

# Decomposer Skill

Break any task into 3-5 sequential subtasks, each completable in one session.

## Standard Pattern
1. understand-requirements (3 turns) - Read files, understand constraints
2. write-initial-solution (5 turns) - Create output file
3. test-and-iterate (10 turns) - Run tests, fix failures
4. final-validation (5 turns) - Ensure 100% pass rate

## Usage
1. Read task description
2. Identify input/output files from description
3. Run `./decompose_task.py`
4. Output: subtasks.json

## Detecting Complexity
- Multiple files mentioned → more subtasks
- Regex/parsing task → add pattern-testing phase
- Data processing → add validation phase

## Scripts
- `decompose_task.py` - Main decomposition
- See `patterns/standard_phases.md` for phase templates
```

---

## Subagent Implementation

For components requiring deep reasoning (not just procedural execution), use subagents.

### MetaReasoner Subagent

```typescript
agents: {
  'meta-reasoner': {
    description: 'Analyze run history and propose improvements. Use after multiple failed attempts.',
    prompt: `You are a meta-learning specialist. Given the history of runs and their scores, propose specific config changes to improve performance.

    Guardrails (MUST follow):
    - Max temperature change: ±0.1
    - Max tests per category change: ±1
    - Never go below 2 tests per category
    - Never inject task-specific knowledge

    Analyze patterns across runs:
    - Which test categories are failing?
    - What types of solutions work better?
    - Is the decomposition appropriate?

    Output ONLY valid JSON matching the schema.`,
    tools: ['Read'],
    model: 'sonnet',  // Needs better reasoning for meta-analysis
  }
}
```

**Structured Output Schema:**
```typescript
const ConfigChangeSchema = z.object({
  analysis: z.string(),
  changes: z.array(z.object({
    target: z.enum(['testgen', 'decomposer', 'sampler']),
    field: z.string(),
    current_value: z.any(),
    proposed_value: z.any(),
    rationale: z.string(),
  })),
  confidence: z.number().min(0).max(1),
});
```

---

## Hook Implementation

### Monitor as PreToolUse Hook

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

---

## MCP Tool Implementation

### Evaluator Tool (in-process MCP)

```typescript
import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';

const evaluatorServer = createSdkMcpServer({
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

---

## Orchestration Loop

### Main HillClimber Flow

```typescript
import { query } from '@anthropic-ai/claude-agent-sdk';

async function hillclimberLoop(task: TerminalBenchTask) {
  // 1. Load skills and generate tests
  const testgenResult = await query({
    prompt: `Generate tests for this task:

    ${task.description}

    Use the testgen skill to generate comprehensive tests.`,
    options: {
      settingSources: ['project'],  // Load skills from .claude/skills/
      outputFormat: { type: 'json_schema', schema: TestGenOutputSchema },
      maxTurns: 10,
    }
  });

  const tests = collectResult(testgenResult).structured_output;

  // 2. Decompose task
  const decomposition = await query({
    prompt: `Decompose this task into subtasks:

    ${task.description}

    Use the decomposer skill.`,
    options: {
      settingSources: ['project'],
      outputFormat: { type: 'json_schema', schema: DecomposerOutputSchema },
    }
  });

  const subtasks = collectResult(decomposition).structured_output.subtasks;

  // 3. Execute subtasks with parallel sampling
  for (const subtask of subtasks) {
    let attempts = 0;
    while (attempts < 3) {
      const candidate = await sampleCandidates(subtask, tests);

      if (candidate.evalResult.progress === 1.0) {
        break;  // All tests pass
      }

      attempts++;
    }
  }

  // 4. Meta-reasoning if not complete
  if (overallProgress < 1.0) {
    await runMetaReasoner(runHistory);
  }
}
```

### Parallel Sampling

```typescript
async function sampleCandidates(subtask: Subtask, tests: Test[]) {
  const configs = [
    { hint: 'Focus on precision and correctness' },
    { hint: 'Balance precision and creativity' },
    { hint: 'Explore alternative approaches' },
  ];

  const results = await Promise.all(configs.map(async (config, i) => {
    const workspace = await createTempWorkspace(i);

    const result = await query({
      prompt: `${subtask.goal}

      Hint: ${config.hint}

      Tests to pass:
      ${JSON.stringify(tests.slice(0, 5))}`,
      options: {
        cwd: workspace,
        model: 'claude-sonnet-4-5-20250929',
        maxTurns: subtask.max_turns,
        mcpServers: { evaluator: evaluatorServer },
        hooks: monitorHooks,
      }
    });

    // Evaluate candidate
    const evalResult = await runTests(workspace);
    return { workspace, config, evalResult, result };
  }));

  // Return best candidate
  return results.sort((a, b) =>
    b.evalResult.progress - a.evalResult.progress
  )[0];
}
```

---

## Continuous Learning

Skills enable Claude to improve over time:

```typescript
// After successful run, create/update skill
if (runResult.passRate > 0.9) {
  await query({
    prompt: `Create a skill from this successful approach:

    Task domain: ${task.domain}
    Solution patterns that worked: ${runResult.successfulPatterns}
    Tests that passed: ${runResult.passingTests}

    Save as a skill in .claude/skills/learned/${task.domain}/
    Include the patterns that led to success.`,
    options: {
      settingSources: ['project'],
      tools: ['Read', 'Write', 'Edit'],
    }
  });
}
```

**Evolution over time:**
```
Run 1:  Generic test generation → 60% TB2 pass
Run 5:  Added regex-specific patterns → 75% pass
Run 10: Refined anti-cheat tests → 85% pass
Run 20: Skill now expert at regex tasks → 95% pass
```

---

## Anti-Cheating Preserved

All original HillClimber guardrails apply:

1. **NO task-specific hardcoding** - No `if task_id == "regex-log"`
2. **TestGen discovers tests from description only** - No TB2 test files
3. **Monitor blocks dangerous commands** - Hook intercepts before execution
4. **Solutions must pass TestGen tests** - Not TB2 directly
5. **Meta-reasoner has bounded config changes** - Guardrails in prompt

---

## Comparison: Rust vs Claude SDK

| Aspect | Rust HillClimber | Claude SDK Version |
|--------|------------------|-------------------|
| Context window | Limited (FM) | 200K tokens |
| Error recovery | Rigid retry logic | Claude reasons about failures |
| Decomposition | Fixed 4-subtask pattern | Dynamic based on task |
| Test generation | FM generates tests | Claude with domain knowledge |
| Meta-learning | Rule-based config changes | Claude analyzes patterns |
| Tool creation | Compile-time Rust traits | Runtime MCP tools |
| Learning | None | Skills evolve with use |

---

## Implementation Checklist

- [ ] Create `.claude/skills/testgen/` with SKILL.md and scripts
- [ ] Create `.claude/skills/decomposer/` with phase patterns
- [ ] Implement evaluator MCP tool with Docker integration
- [ ] Add PreToolUse hook for monitor
- [ ] Define meta-reasoner subagent
- [ ] Build orchestration loop
- [ ] Add skill creation for successful runs
- [ ] Test on regex-log task

---

## References

- [Agent SDK Overview](./agent-sdk-overview.md)
- [Subagents Guide](./guides/subagents-in-the-sdk.md)
- [Skills Guide](./guides/agent-skills-in-the-sdk.md)
- [Structured Outputs](./guides/structured-outputs.md)
- [Don't Build Agents, Build Skills](../transcripts/dont-build-agents-build-skills.md)
- [HillClimber Architecture](../fm-hillclimber.md)
- [MechaCoder Golden Loop](../mechacoder/GOLDEN-LOOP-v2.md)
