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
