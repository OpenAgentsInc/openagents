# Agent Context Management

The intro agent (autopilot) uses an agentic loop that accumulates context with each tool call. Without management, context can grow unbounded, leading to:

- API token limit errors
- Increased latency
- Higher costs (tokens are billed per request)

This document describes the context management system implemented in `client/src/intro_agent.rs`.

## Problem

Each agent loop iteration sends ALL previous messages to the LLM API. With tool calls, this accumulates quickly:

| Iteration | Messages | Context Size | Est. Tokens |
|-----------|----------|--------------|-------------|
| 1         | 1        | ~2 KB        | ~500        |
| 5         | 10       | ~25 KB       | ~6,000      |
| 10        | 20       | ~60 KB       | ~15,000     |
| 15        | 30       | ~100 KB      | ~25,000     |

With 50 max iterations, unmanaged context could exceed 200KB (~50K tokens).

## Solution

### 1. File Truncation (4KB limit)

Large files are truncated at fetch time to 4KB:

```rust
let truncated = if content.len() > 4000 {
    let mut end = 4000;
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...\n[truncated, showing first {} bytes of {} total]",
            &content[..end], end, size)
} else {
    content
};
```

This happens in `execute_tool()` for `view_file` operations.

### 2. Context Compaction

The `compact_old_context()` function compresses old tool results while keeping recent ones intact:

```rust
fn compact_old_context(
    messages: &mut [Message],
    keep_recent: usize,      // Number of recent tool results to keep intact
    max_context_bytes: usize // Threshold to trigger compaction
)
```

**How it works:**
1. Estimates total context size
2. If over threshold, identifies tool message indices
3. Keeps the most recent N tool results intact
4. Compacts older tool results to summaries:
   - Before: Full file contents (4KB)
   - After: `[Previously viewed: File: src/main.rs]` (~50 bytes)

**Configuration (constants in `run_agent_loop`):**
```rust
const MAX_CONTEXT_BYTES: usize = 50_000;  // ~12.5K tokens
const KEEP_RECENT_TOOLS: usize = 6;       // Keep last 6 tool results full
```

### 3. Context Size Logging

Every iteration logs estimated context size:

```
Calling AI with 15 messages (~35000 bytes, ~8750 tokens)
```

When compaction triggers:
```
Context size 60000 bytes exceeds limit 50000, compacting old tool results...
Context compacted: 60000 bytes -> 32000 bytes (saved 28000 bytes)
```

### 4. Token Limit Error Recovery

If the API returns a token limit error, aggressive recovery kicks in:

```rust
ErrorClass::TokenLimit => {
    // Aggressive compaction: keep only 3 recent, target 30KB
    compact_old_context(&mut messages, 3, 30_000);
    // Truncate any remaining large results to 1KB
    truncate_tool_results(&mut messages, 1000);
    Some("Context was compacted and truncated to fit token limits.".to_string())
}
```

## Functions Reference

### `estimate_context_size(messages: &[Message]) -> usize`

Returns estimated byte size of all messages including content and tool_calls.

### `compact_old_context(messages: &mut [Message], keep_recent: usize, max_context_bytes: usize)`

Compacts old tool results to summaries when context exceeds threshold.

**Parameters:**
- `messages`: The conversation messages to compact
- `keep_recent`: Number of recent tool results to keep intact
- `max_context_bytes`: Threshold that triggers compaction

### `truncate_tool_results(messages: &mut [Message], max_len: usize)`

Truncates all tool results to a maximum character length. Used for emergency recovery.

## Tuning Guidelines

| Parameter | Default | Effect of Increase | Effect of Decrease |
|-----------|---------|-------------------|-------------------|
| `MAX_CONTEXT_BYTES` | 50,000 | More context, higher cost | Less context, may lose info |
| `KEEP_RECENT_TOOLS` | 6 | Better recall of recent work | More aggressive compaction |
| File truncation | 4,000 | More file content visible | Smaller context per file |

**Recommended settings by use case:**

- **Cost-sensitive**: MAX_CONTEXT_BYTES=30000, KEEP_RECENT_TOOLS=4
- **Quality-focused**: MAX_CONTEXT_BYTES=80000, KEEP_RECENT_TOOLS=10
- **Default (balanced)**: MAX_CONTEXT_BYTES=50000, KEEP_RECENT_TOOLS=6

## Database Logging

All LLM calls are logged to the `llm_calls` table with:
- `request_messages`: Full JSON of messages sent (for debugging)
- `prompt_tokens`, `completion_tokens`: Token counts (0 for streaming)
- `created_at`: Timestamp for tracking session progression

Query to check context growth:
```sql
SELECT id, LENGTH(request_messages) as bytes, created_at
FROM llm_calls
ORDER BY created_at DESC
LIMIT 20;
```

## Future Improvements

1. **Semantic summarization**: Use a smaller model to summarize tool results instead of just extracting the first line
2. **Selective retention**: Keep full content for "important" files (e.g., README, config) longer
3. **Token counting**: Use actual tokenizer instead of byte/4 estimate
4. **Streaming token capture**: Buffer streaming responses to capture actual usage stats
