# FM Micro-Task Supervisor Architecture - Implementation Plan

**Date**: 2025-12-07
**Time**: 22:15 CST
**Status**: Design Document - How to Make FM Actually Work

## The Core Insight

FM's context limit (~200-250 chars) means it **cannot** do multi-turn conversations or see tool results. But it **CAN** do single, focused actions if we:
1. Decompose tasks into micro-steps
2. Use a supervisor FM to plan/decompose
3. Use worker FMs to execute single actions
4. Orchestrator manages state and delegates

## Architecture Overview

```
User Task: "Write image.c program"
    ↓
Supervisor FM (200 chars): "Decompose: 1) read image.ppm, 2) write image.c, 3) compile"
    ↓
Orchestrator: Executes plan, manages state
    ↓
Worker FM #1 (200 chars): "Read first 50 lines of /app/image.ppm"
    ↓
Orchestrator: Stores result, updates state
    ↓
Worker FM #2 (200 chars): "Write image.c with read_ppm function"
    ↓
Orchestrator: Applies change, validates
    ↓
Worker FM #3 (200 chars): "Compile image.c"
    ↓
Done
```

## Example: Path Tracing Task

### Original Task (807 chars - TOO LARGE)
```
I've put an image at /app/image.ppm that I rendered programmatically.
Write a c program image.c that I can run with:
  gcc -static -o image image.c -lm && ./image
that reads /app/image.ppm and outputs the same image to stdout in PPM format.
```

### Step 1: Supervisor FM Decomposes (200 chars max)

**Supervisor Prompt** (150 chars):
```
Decompose task into micro-steps. Output JSON:
{"steps":["step1","step2",...]}
Task: [TASK_DESCRIPTION]
```

**Supervisor Response** (example):
```json
{
  "steps": [
    "read_file /app/image.ppm lines 1-20",
    "write_file image.c with read_ppm function",
    "write_file image.c add main function",
    "run_command gcc -static -o image image.c -lm",
    "run_command ./image > output.ppm",
    "read_file output.ppm lines 1-5"
  ]
}
```

**Orchestrator**: Parses steps, stores in state (outside FM context)

### Step 2: Worker FM Executes Step 1

**Worker Prompt** (180 chars):
```
Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: read_file /app/image.ppm lines 1-20
Context: Reading PPM header to understand format
Output: <tool_call>{"name":"read_file","arguments":{"path":"/app/image.ppm","start":1,"end":20}}</tool_call>
```

**Worker Response**:
```json
<tool_call>
{
  "name": "read_file",
  "arguments": {
    "path": "/app/image.ppm",
    "start": 1,
    "end": 20
  }
}
</tool_call>
```

**Orchestrator**:
- Executes tool: `read_file("/app/image.ppm", 1, 20)`
- Gets result: "P3\n800 600\n255\n..."
- Stores in state: `{ppmHeader: "P3\n800 600\n255\n...", step: 1, status: "complete"}`

### Step 3: Worker FM Executes Step 2

**Worker Prompt** (195 chars):
```
Tools: read_file, write_file, run_command
Action: write_file image.c with read_ppm function
Context: PPM format is P3, 800x600, max 255. Need read_ppm() function.
Previous: Step 1 read header "P3\n800 600\n255"
Output: <tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"#include <stdio.h>..."}}</tool_call>
```

**Worker Response**:
```json
<tool_call>
{
  "name": "write_file",
  "arguments": {
    "path": "image.c",
    "content": "#include <stdio.h>\n#include <stdlib.h>\n\ntypedef struct { int w, h, max; int *pixels; } Image;\n\nImage read_ppm(const char *path) {\n  FILE *f = fopen(path, \"r\");\n  char magic[3];\n  fscanf(f, \"%2s\", magic);\n  Image img;\n  fscanf(f, \"%d %d %d\", &img.w, &img.h, &img.max);\n  img.pixels = malloc(img.w * img.h * 3 * sizeof(int));\n  for (int i = 0; i < img.w * img.h * 3; i++) fscanf(f, \"%d\", &img.pixels[i]);\n  fclose(f);\n  return img;\n}\n"
  }
}
</tool_call>
```

**Orchestrator**:
- Executes tool: `write_file("image.c", content)`
- Validates: File created, compiles? (runs quick syntax check)
- Stores: `{step: 2, status: "complete", fileCreated: "image.c"}`

### Step 4: Worker FM Executes Step 3

**Worker Prompt** (190 chars):
```
Tools: read_file, write_file, run_command
Action: write_file image.c add main function
Context: File exists with read_ppm(). Need main() that reads /app/image.ppm and prints to stdout.
Previous: Step 2 created image.c with read_ppm()
Output: <tool_call>{"name":"read_file","arguments":{"path":"image.c"}}</tool_call>
```

**Worker Response** (first reads file):
```json
<tool_call>
{
  "name": "read_file",
  "arguments": {
    "path": "image.c"
  }
}
</tool_call>
```

**Orchestrator**: Returns file content (but this might be too large! See solution below)

**Better Approach - Worker asks for specific lines**:
```json
<tool_call>
{
  "name": "read_file",
  "arguments": {
    "path": "image.c",
    "start": 1,
    "end": 20
  }
}
</tool_call>
```

**Then Worker adds main**:
```json
<tool_call>
{
  "name": "edit_file",
  "arguments": {
    "path": "image.c",
    "old_text": "  return img;\n}",
    "new_text": "  return img;\n}\n\nint main() {\n  Image img = read_ppm(\"/app/image.ppm\");\n  printf(\"P3\\n%d %d\\n%d\\n\", img.w, img.h, img.max);\n  for (int i = 0; i < img.w * img.h * 3; i++) printf(\"%d\\n\", img.pixels[i]);\n  return 0;\n}"
  }
}
</tool_call>
```

### Step 5: Worker FM Compiles

**Worker Prompt** (150 chars):
```
Tools: read_file, write_file, run_command
Action: run_command gcc -static -o image image.c -lm
Context: Compile image.c to verify it works
Previous: Step 3 added main() function
Output: <tool_call>{"name":"run_command","arguments":{"command":"gcc -static -o image image.c -lm"}}</tool_call>
```

**Worker Response**:
```json
<tool_call>
{
  "name": "run_command",
  "arguments": {
    "command": "gcc -static -o image image.c -lm"
  }
}
</tool_call>
```

**Orchestrator**:
- Executes: `gcc -static -o image image.c -lm`
- Result: Success (exit 0) or failure (exit 1, stderr)
- If failure: Creates new micro-task "Fix compile error: [error message]"
- Stores: `{step: 4, status: "complete", compiled: true}`

### Step 6: Worker FM Tests

**Worker Prompt** (180 chars):
```
Tools: read_file, write_file, run_command
Action: run_command ./image > output.ppm
Context: Run compiled program, capture output
Previous: Step 4 compiled successfully
Output: <tool_call>{"name":"run_command","arguments":{"command":"./image > output.ppm"}}</tool_call>
```

## Key Design Patterns

### Pattern 1: Supervisor-Worker Split

**Supervisor FM** (150-180 chars):
- Input: Task description (truncated to ~100 chars)
- Output: JSON list of micro-steps
- No tool calls, just planning

**Worker FM** (180-200 chars):
- Input: Single action + minimal context (~50 chars)
- Output: Single tool call
- No conversation history

### Pattern 2: State Management Outside FM

**Orchestrator maintains**:
- Task state (current step, completed steps)
- File contents (cached, not in FM context)
- Tool results (summarized, not raw)
- Error messages (condensed)

**FM never sees**:
- Full file contents
- Long error messages
- Conversation history
- Multiple tool results

### Pattern 3: Context Summarization

Instead of:
```
Previous: Tool result: Working directory: /path/to/workspace
Command: gcc -static -o image image.c -lm
Exit code: 1
STDERR: clang: error: no such file or directory: 'image.c'
```

Use:
```
Previous: Step 2 failed - image.c not found. Need to create it first.
```

**Condensed from 150+ chars to 60 chars**

### Pattern 4: Progressive File Reading

Instead of:
```
read_file("image.c")  // Returns 500+ lines, too large
```

Use:
```
read_file("image.c", start: 1, end: 30)   // Just function signature
read_file("image.c", start: 50, end: 80)  // Just the part we need
```

**FM only sees what's needed for current action**

### Pattern 5: Error-Driven Micro-Tasks

When tool fails:
1. Orchestrator extracts error (condensed to 50 chars)
2. Creates new micro-task: "Fix: [error]"
3. Worker FM gets: "Action: Fix compile error 'image.c not found'. Previous: Tried to compile."
4. Worker creates the missing file

## Implementation Details

### Supervisor FM Prompt Template (150 chars)

```
Decompose into steps. Output JSON:
{"steps":["action1","action2",...]}
Task: [TASK_50_CHARS]
```

**Example**:
```
Decompose into steps. Output JSON:
{"steps":["action1","action2",...]}
Task: Write image.c that reads /app/image.ppm and outputs PPM
```

### Worker FM Prompt Template (180 chars)

```
Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: [ACTION_40_CHARS]
Context: [CONTEXT_50_CHARS]
Previous: [PREVIOUS_30_CHARS]
Output: <tool_call>{"name":"...","arguments":{...}}</tool_call>
```

**Example**:
```
Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: write_file image.c with read_ppm function
Context: PPM format P3, 800x600, max 255
Previous: Step 1 read header "P3\n800 600\n255"
Output: <tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"..."}}</tool_call>
```

### Orchestrator State Schema

```typescript
interface TaskState {
  taskId: string;
  currentStep: number;
  totalSteps: number;
  steps: Array<{
    id: number;
    action: string;
    status: "pending" | "in_progress" | "complete" | "failed";
    result?: string;  // Condensed summary, not raw output
    error?: string;   // Condensed error, max 50 chars
  }>;
  files: Record<string, string>;  // Cached file contents
  workspace: string;
}
```

### Tool Result Condensation

**Before** (264 chars):
```
Tool results:
run_command result: Working directory: /Users/christopherdavid/code/openagents/results/tb-20251208035650-f7jcan/path-tracing/workspace
Command: gcc -static -o image image.c -lm
Exit code: 1

STDERR:
clang: error: no such file or directory: 'image.c'
```

**After** (45 chars):
```
Previous: Compile failed - image.c not found
```

**Rules**:
- Extract only the error message
- Remove paths, working directories
- Remove success messages (assume success if no error)
- Max 50 chars per result

## Example Full Conversation Flow

### Initial State
```
Task: "Write image.c that reads /app/image.ppm and outputs PPM"
State: {step: 0, steps: [], files: {}}
```

### Turn 1: Supervisor Decomposes
```
Supervisor FM Input (150 chars):
"Decompose into steps. Output JSON: {\"steps\":[...]}
Task: Write image.c that reads /app/image.ppm and outputs PPM"

Supervisor FM Output:
{"steps":["read_file /app/image.ppm 1-20","write_file image.c read_ppm","write_file image.c main","run_command gcc image.c","run_command ./image"]}

Orchestrator: Creates 5 steps, sets step 1 to "in_progress"
```

### Turn 2: Worker Reads File
```
Worker FM Input (180 chars):
"Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: read_file /app/image.ppm lines 1-20
Context: Reading PPM header to understand format
Previous: (none)
Output: <tool_call>..."

Worker FM Output:
<tool_call>{"name":"read_file","arguments":{"path":"/app/image.ppm","start":1,"end":20}}</tool_call>

Orchestrator:
- Executes: read_file("/app/image.ppm", 1, 20)
- Result: "P3\n800 600\n255\n..."
- Stores: files["/app/image.ppm"] = "P3\n800 600\n255\n..."
- Updates: step 1 = "complete", result = "PPM header: P3, 800x600, max 255"
- Sets step 2 to "in_progress"
```

### Turn 3: Worker Writes Function
```
Worker FM Input (195 chars):
"Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: write_file image.c with read_ppm function
Context: PPM format is P3, 800x600, max 255. Need read_ppm() function.
Previous: Step 1: PPM header P3, 800x600, max 255
Output: <tool_call>..."

Worker FM Output:
<tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"#include <stdio.h>..."}}</tool_call>

Orchestrator:
- Executes: write_file("image.c", content)
- Validates: File exists, has read_ppm function
- Stores: files["image.c"] = content
- Updates: step 2 = "complete", result = "Created image.c with read_ppm()"
- Sets step 3 to "in_progress"
```

### Turn 4: Worker Adds Main
```
Worker FM Input (190 chars):
"Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: write_file image.c add main function
Context: File has read_ppm(). Need main() that reads /app/image.ppm and prints.
Previous: Step 2: Created image.c with read_ppm()
Output: <tool_call>..."

Worker FM Output:
<tool_call>{"name":"read_file","arguments":{"path":"image.c","start":1,"end":15}}</tool_call>

Orchestrator:
- Executes: read_file("image.c", 1, 15)
- Returns: First 15 lines (cached, not recalculated)
- Worker gets response, then outputs:
  <tool_call>{"name":"edit_file","arguments":{"path":"image.c","old_text":"  return img;\n}","new_text":"  return img;\n}\n\nint main() {...}"}}</tool_call>

Orchestrator:
- Executes: edit_file(...)
- Updates: files["image.c"] = new content
- Updates: step 3 = "complete", result = "Added main() function"
- Sets step 4 to "in_progress"
```

### Turn 5: Worker Compiles
```
Worker FM Input (180 chars):
"Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: run_command gcc -static -o image image.c -lm
Context: Compile image.c to verify it works
Previous: Step 3: Added main() function
Output: <tool_call>..."

Worker FM Output:
<tool_call>{"name":"run_command","arguments":{"command":"gcc -static -o image image.c -lm"}}</tool_call>

Orchestrator:
- Executes: run_command("gcc -static -o image image.c -lm")
- Result: Success (exit 0)
- Updates: step 4 = "complete", result = "Compiled successfully"
- Sets step 5 to "in_progress"
```

### Turn 6: Worker Tests
```
Worker FM Input (175 chars):
"Tools: read_file(path,start,end), write_file(path,content), run_command(cmd)
Action: run_command ./image > output.ppm
Context: Run program, capture output to verify
Previous: Step 4: Compiled successfully
Output: <tool_call>..."

Worker FM Output:
<tool_call>{"name":"run_command","arguments":{"command":"./image > output.ppm"}}</tool_call>

Orchestrator:
- Executes: run_command("./image > output.ppm")
- Result: Success, output.ppm created
- Validates: output.ppm matches /app/image.ppm (or checks first few lines)
- Updates: step 5 = "complete", result = "Program runs, output created"
- All steps complete!
```

## Handling Errors

### Example: Compile Error

**Turn 4 fails**:
```
Worker: run_command("gcc -static -o image image.c -lm")
Result: Exit 1, stderr: "image.c:5: error: expected ';' before '}'"
```

**Orchestrator**:
1. Extracts error: "expected ';' before '}' at line 5"
2. Condenses: "Syntax error: missing ';' at line 5" (40 chars)
3. Creates new micro-task: "Fix syntax error: missing ';' at line 5"
4. Worker gets:
   ```
   Action: Fix syntax error: missing ';' at line 5
   Context: image.c has syntax error preventing compile
   Previous: Step 4: Compile failed - syntax error
   ```
5. Worker reads line 5, fixes it, retries compile

## Context Budget Breakdown

### Supervisor FM (150 chars)
- Prompt template: 80 chars
- Task description: 70 chars max
- **Total: 150 chars** ✅

### Worker FM (200 chars)
- Prompt template: 100 chars
- Action: 40 chars
- Context: 30 chars
- Previous: 30 chars
- **Total: 200 chars** ✅

### Tool Results (condensed to 50 chars)
- Success: "Step N: [action] complete" (30 chars)
- Error: "Step N: [error message]" (40 chars)
- **Always < 50 chars** ✅

## Implementation Checklist

### Phase 1: Supervisor FM
- [ ] Create supervisor prompt template (150 chars)
- [ ] Implement JSON parsing for step list
- [ ] Test with simple tasks
- [ ] Validate step decomposition quality

### Phase 2: Worker FM
- [ ] Create worker prompt template (200 chars)
- [ ] Implement single-action execution
- [ ] Add context summarization
- [ ] Test with individual actions

### Phase 3: Orchestrator
- [ ] State management (steps, files, results)
- [ ] Tool result condensation
- [ ] Error extraction and micro-task creation
- [ ] Progressive file reading (line ranges)
- [ ] Step progression logic

### Phase 4: Integration
- [ ] Connect supervisor → orchestrator → workers
- [ ] End-to-end test with path-tracing task
- [ ] Error handling and recovery
- [ ] Performance optimization

## Expected Outcomes

### Success Criteria
- ✅ FM can decompose tasks into micro-steps
- ✅ FM can execute single actions within 200 char limit
- ✅ Multi-step tasks complete successfully
- ✅ Errors are handled and recovered from

### Limitations
- ⚠️ No conversation history (by design)
- ⚠️ Requires orchestrator for state management
- ⚠️ Slower than single-model approach (multiple FM calls)
- ⚠️ More complex architecture

### Benefits
- ✅ Actually works within FM's constraints
- ✅ Follows micro-task philosophy from coding thoughts doc
- ✅ Scalable to complex tasks
- ✅ Error recovery built-in

---

**Status**: Design complete. Ready for implementation.

**Next Steps**:
1. Implement supervisor FM
2. Implement worker FM
3. Build orchestrator
4. Test with path-tracing task
