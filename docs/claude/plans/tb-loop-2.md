# Plan: FM Terminal-Bench Hardening & LM Loop Foundation

## Context

FM Terminal-Bench loop is working (100% pass rate on hello-world). Now we need to:
1. Harden the FM adapter for production use
2. Create a proper FM mini-suite for regression testing
3. Wire up regression testing infrastructure
4. Prepare for FM-driven task selection (future: LM tasks/coding)

## Implementation Phases

### Phase 1: Harden FM Adapter (Task 1 from 1419)

**File:** `src/bench/model-adapter.ts`

#### 1.1 Configurable Context Limits

```typescript
// Extract constant and create config object
interface FMModelConfig {
  maxContextChars: number;
  maxResponseChars?: number;
  toolSet?: string[];
}

const FM_MODEL_CONFIGS: Record<string, FMModelConfig> = {
  "apple-foundation": { maxContextChars: 1100 },
  // Future: other FM models with different limits
};

// Default fallback
const FM_MAX_CONTEXT_CHARS_DEFAULT = 1100;

// Update truncateMessagesForFM() to accept maxChars parameter
```

#### 1.2 Robust Tool Parsing

Refactor `parseDescriptiveToolCall()`:
- Return structured type: `{ success: true, call: ToolCall } | { success: false, error: ParseError }`
- Handle all known formats:
  1. `<tool_call>{...}</tool_call>` tags
  2. Markdown JSON blocks
  3. `Using X tool with arguments: ...` format
  4. Multiple candidates (pick first valid)
- Support all tools (not just 3)

#### 1.3 Failure Logging

When all parse formats fail:
```typescript
interface FMToolParseError {
  type: "FM_TOOL_PARSE_ERROR";
  rawSnippet: string; // First 200 chars
  reason: "no_valid_format" | "json_parse_error" | "missing_required_fields";
  timestamp: string;
}

// Log to structured output, return to harness with clear error
```

#### 1.4 Unit Tests

**File:** `src/bench/model-adapter.test.ts` (new)

- `truncateMessagesForFM()`: keeps system + last, drops middle, deterministic
- `parseToolCalls()`: all formats, edge cases, adversarial inputs
- `parseDescriptiveToolCall()`: key=value parsing, missing fields

### Phase 2: FM Mini-Suite (Task 2 from 1419)

**File:** `tasks/fm-mini-suite.json`

5-10 tasks exercising FM's capabilities:

| # | Task | Tools | Verification |
|---|------|-------|--------------|
| 1 | hello-world | write_file | custom: check file exists + content |
| 2 | read-and-echo | write_file, read_file | custom: check echoed content |
| 3 | append-to-file | write_file (x2 or edit) | custom: check appended content |
| 4 | list-directory | run_command (ls) | output: expected files listed |
| 5 | create-and-run | write_file, run_command | custom: script output matches |
| 6 | file-from-template | read_file, write_file | custom: transformation correct |
| 7 | simple-edit | write_file, edit_file | custom: edit applied correctly |

**Directory:** `tasks/verify/` (verification scripts)

```bash
# tasks/verify/fm-mini-hello.sh
#!/bin/bash
test -f hello.txt && grep -q 'Hello, world!' hello.txt

# tasks/verify/fm-mini-read-echo.sh
#!/bin/bash
# Check source.txt was read and echo.txt contains its content
test -f echo.txt && diff -q source.txt echo.txt
```

### Phase 3: Regression Wiring (Task 3 from 1419)

#### 3.1 Package.json Scripts

```json
{
  "scripts": {
    "tbench:fm-mini": "bun src/cli/tbench-local.ts --model fm.apple-foundation --suite tasks/fm-mini-suite.json",
    "tbench:fm-mini:iterate": "bun src/cli/tbench-iterate.ts --model fm.apple-foundation --suite tasks/fm-mini-suite.json --max-tasks 10"
  }
}
```

#### 3.2 tbench-local.ts Model Selection

**File:** `src/cli/tbench-local.ts`

Add `--model` flag to enable FM runs:
```typescript
// Add to CLI options
model: {
  type: "string",
  description: "Model to use: claude-code, fm.apple-foundation, minimal",
  default: "claude-code"
}

// Route to FM runner when model starts with "fm."
```

#### 3.3 CI Job (Optional, non-blocking)

**File:** `.github/workflows/fm-tbench.yml`

```yaml
name: FM Terminal-Bench Regression
on:
  workflow_dispatch:  # Manual trigger only initially
  # Later: schedule for nightly

jobs:
  fm-mini:
    runs-on: macos-14  # M-series required
    if: github.event_name == 'workflow_dispatch'
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
      - run: bun install
      - run: ./swift/foundation-bridge/build.sh
      - run: ./swift/foundation-bridge/run.sh &
      - run: sleep 5 && bun run tbench:fm-mini
    continue-on-error: true  # Non-blocking initially
```

### Phase 4: Documentation Update

**File:** `docs/terminal-bench.md`

Add "Using Apple Foundation Models" section:
- Prerequisites (macOS 26, Apple Intelligence, bridge built)
- Example commands
- FM config in project.json
- Known limitations (context size)

---

## Future Integration: LM Tasks/Coding (from 1420-lm-tasks-instructions.md)

This work sets the foundation for FM-driven task selection:

### Task Selection Module (future: `src/tasks/selection.ts`)

```typescript
interface TaskSelectionPreferences {
  preferredLabels?: string[];
  avoidLabels?: string[];
  minPriorityForWork?: number;
  labelWeights?: Record<string, number>;
  typeWeights?: Record<TaskType, number>;
}

// Functions to implement later:
getReadyTaskCandidates(tasks) → TaskCandidateSummary[]
scoreAndSortCandidates(candidates, prefs?) → TaskCandidateSummary[]
fmChooseNextAction(context) → FMSelectionDecision
```

### Plan Session Module (future: `src/agent/planner/fm-plan-session.ts`)

When FM decides to plan instead of work:
- Generate proposed tasks via FM
- Create via TaskService
- Log to `docs/logs/YYYYMMDD/HHMM-fm-plan-session-log.md`

### Micro-Task Coding Strategy (from 1421)

FM operates as a "local coder" with ~1100 char context:
- **Handle-based navigation**: Reference functions/types by ID, not raw text
- **Tool-driven exploration**: grep, read_file(startLine, endLine), write_file
- **Verify aggressively**: Run tests after each micro-edit, feed errors back
- **Skills/Memory injection**: Top-K relevant skills + memories in prompt

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/bench/model-adapter.ts` | Configurable context, robust parsing, error logging |
| `src/bench/model-adapter.test.ts` | New: unit tests for FM adapter |
| `src/cli/tbench-local.ts` | Add `--model` flag |
| `tasks/fm-mini-suite.json` | New: 5-10 FM regression tasks |
| `tasks/verify/*.sh` | New: verification scripts |
| `package.json` | Add `tbench:fm-mini*` scripts |
| `docs/terminal-bench.md` | Add FM section |
| `.github/workflows/fm-tbench.yml` | Optional: CI job |

## Acceptance Criteria

1. **Hardened Adapter**:
   - `bun test src/bench/model-adapter.test.ts` passes
   - FM context size is configurable via constant
   - Tool parse failures produce structured logs

2. **FM Mini-Suite**:
   - `bun run tbench:fm-mini` runs 5+ tasks
   - ≥80% pass rate on fresh runs
   - Each task has deterministic verification

3. **Regression Ready**:
   - Single command to run FM regression
   - Results visible in TB dashboard/logs

---

## Execution Order

1. **Start with tests** - Write model-adapter.test.ts first (TDD)
2. **Harden adapter** - Fix issues tests expose
3. **Create mini-suite** - Start with existing hello-world, add incrementally
4. **Wire regression** - Scripts, CLI flags, docs
5. **Optional CI** - Only if time permits

## Time Estimate

Not providing time estimates per CLAUDE.md. These are the logical steps in priority order.
