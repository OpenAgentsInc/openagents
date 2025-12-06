# FM Terminal-Bench Loop Working

## Summary
Successfully got Apple Foundation Models working with Terminal-Bench loop.
- 5/5 iterations: 100% pass rate
- ~0.9s per task execution

## Key Fixes

### 1. Context Limit Handling
- Determined FM max context empirically: ~1100 chars (~280 tokens)
- Added `truncateMessagesForFM()` to smartly truncate history
- Keep system prompt + last message, trim middle

### 2. Tool Call Parsing
FM outputs multiple formats:
- `<tool_call>{"name":"...","arguments":{...}}</tool_call>` 
- ```json {"response":"Using write_file tool with arguments: path=X, content=Y"}```
- `Using write_file tool with arguments: path=hello.txt, content=Hello, world!`

Added `parseDescriptiveToolCall()` to handle all formats.

### 3. Custom Verification Type
Fixed `runLocalVerification()` in tbench-iterate.ts - it was missing handling for "custom" verification type with script.

### 4. Simplified FM Prompt
Reduced prompt to bare minimum:
```
You must use tools. Output ONLY a tool_call tag.

Example: To create hello.txt with "Hi":
<tool_call>{"name":"write_file","arguments":{"path":"hello.txt","content":"Hi"}}</tool_call>

Available: write_file, read_file, run_command

After the tool runs, say TASK_COMPLETE.
```

## Files Changed
- `src/bench/model-adapter.ts` - Context limits, tool parsing, FM prompt
- `src/cli/tbench-iterate.ts` - Custom verification type support
- `tasks/hello-world-suite.json` - Updated to use custom verification

## Test Results
```
Iterations: 5
Total Duration: 0.1 minutes
Average Pass Rate: 100.0%
```

## Next Steps
- Get iterative improvement working (fail → succeed patterns)
- Test with more complex tasks
- Terminal-Bench #1 with Apple FM!
