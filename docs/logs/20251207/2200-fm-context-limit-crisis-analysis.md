# FM Context Limit Crisis - Full Analysis

**Date**: 2025-12-07
**Time**: 22:00 CST
**Status**: Critical Issue - FM Model Runner Completely Broken

## Executive Summary

The FM (Foundation Models) runner is **completely non-functional** due to context window limits. Even with extremely aggressive truncation (236-238 chars of content), FM is rejecting requests with "Exceeded model context window size". This suggests FM's actual limit is **much smaller than documented** (~200-250 chars total) or there's significant unaccounted overhead.

## Evidence from Logs

### Pattern Observed

Every single task fails on Turn 1 with the same error, even with minimal content:

```
[Context] System prompt: 123 chars
[Context] Final context: 354-358 chars (System: 123, User: 111-115, JSON overhead: 120)
[FM Request] Total content chars: 234-238
[FM Request] Estimated JSON size: 303-309 chars
[Context] Hit context limit, retrying with minimal context
Error: Foundation Models request failed: Exceeded model context window size
```

### Specific Examples

**Example 1: path-tracing (Turn 1)**
- System: 123 chars
- User: 113 chars
- Total content: 236 chars
- JSON size: 305 chars
- **Result**: FAILED - "Exceeded model context window size"

**Example 2: model-extraction-relu-logits (Turn 1)**
- System: 123 chars
- User: 115 chars
- Total content: 238 chars
- JSON size: 307 chars
- **Result**: FAILED - "Exceeded model context window size"

**Example 3: path-tracing (Turn 2)**
- After tool call, messages grow to 4 messages
- Total content: 636 chars
- JSON size: 794 chars
- Truncation reduces to minimal context (454 chars content, 606 JSON)
- **Result**: FAILED - "Exceeded model context window size"

## Root Cause Analysis

### Problem 1: FM's Actual Limit is Much Smaller Than Documented

**Documented Limit** (from code comments):
- Max per-request: ~1373 chars = ~347 tokens
- Safe limit: ~1100 chars = ~280 tokens

**Actual Observed Limit**:
- Even 236 chars of content fails
- Even 305 chars of JSON fails
- Suggests actual limit is **~200-250 chars TOTAL** (including all overhead)

**Hypothesis**: The documented limits are for the raw model, but the FM bridge/server adds significant overhead that reduces the effective limit.

### Problem 2: Unaccounted Overhead

We're accounting for:
- System prompt: 123 chars
- User message: 111-115 chars
- JSON structure: ~120 chars (estimated)

**But we're missing**:
- HTTP request structure overhead
- Model name in request (`"model": "apple-foundation-model"`)
- Request metadata (`"stream": false`, etc.)
- FM bridge internal overhead
- Tokenization overhead (chars → tokens conversion)

**Estimated missing overhead**: 50-100+ chars

### Problem 3: Context Grows Too Fast

Even when Turn 1 succeeds (rare), Turn 2 immediately fails because:
- Assistant response adds ~136 chars
- Tool result adds ~264 chars
- Total grows from 236 → 636 chars in one turn
- Truncation can't keep up

### Problem 4: Truncation Strategy Ineffective

Current truncation:
- Preserves system message (123 chars)
- Preserves last 2-3 message pairs
- But even with minimal context (454 chars), it still fails

**Issue**: We're truncating content, but FM might be counting something else (tokens? request size?).

## Critical Findings

### Finding 1: Even Minimal Content Fails

The smallest successful request we've seen:
- System: 123 chars
- User: 111-115 chars
- Total: 234-238 chars content
- JSON: 303-309 chars

**This still fails**, meaning FM's effective limit is **< 300 chars total**.

### Finding 2: No Successful Requests

**100% failure rate** across all tasks:
- path-tracing: Failed Turn 1
- model-extraction-relu-logits: Failed Turn 1
- video-processing: Failed Turn 1
- dna-assembly: Failed Turn 1
- regex-log: Failed Turn 1
- large-scale-text-editing: Failed Turn 1

**Zero successful tool calls** in any task.

### Finding 3: Minimal Retry Also Fails

When we hit the limit and retry with minimal context (half the budget):
- Same messages, just more aggressive truncation
- Still fails with same error
- Suggests the problem isn't just truncation - it's fundamental

### Finding 4: JSON Size vs Content Size Mismatch

- Content: 236 chars
- JSON: 305 chars
- Difference: 69 chars (29% overhead)

But we're estimating 120 chars overhead, which suggests we're over-estimating. However, the actual HTTP request might have even more overhead we're not seeing.

## What We've Tried

### Attempt 1: Reduce Context Budget
- Started at 1100 chars
- Reduced to 900 chars
- Reduced to 700 chars
- Reduced to 400 chars
- **Result**: Still failing

### Attempt 2: Disable Skills/Memories/Reflections
- Disabled by default (saves ~300-500 chars)
- **Result**: Still failing

### Attempt 3: Aggressive Truncation
- Truncate task descriptions to 111-115 chars
- Use 80-90% of budget as target
- **Result**: Still failing

### Attempt 4: Better Logging
- Added comprehensive logging of all messages
- Shows exact JSON payloads
- **Result**: Revealed the problem is worse than we thought

## The Real Problem

**FM's effective context limit is ~200-250 chars TOTAL**, not 400, not 700, not 1100.

This means:
- System prompt (123 chars) = 50% of budget
- User message = ~100 chars max
- **No room for tool results, assistant responses, or conversation history**

## Implications

### FM Cannot Be Used for Multi-Turn Tasks

With a 200-250 char limit:
- Can't maintain conversation history
- Can't see tool results
- Can't build on previous turns
- **Effectively single-turn only**

### FM Cannot Handle Real Tasks

Most Terminal-Bench tasks require:
- Multiple tool calls
- Reading files
- Understanding context
- Iterative problem-solving

**None of this is possible with 200-250 char limit.**

## Possible Solutions

### Solution 1: Accept FM is Single-Turn Only

**Approach**: Treat FM as a stateless, single-request tool
- Send only the task description (truncated to ~100 chars)
- No conversation history
- No tool results
- Just: "Do this one thing"

**Pros**: Might work for very simple tasks
**Cons**: Severely limited, can't do most TB tasks

### Solution 2: Investigate FM Bridge Overhead

**Approach**:
- Check what the FM bridge actually sends
- Measure actual request size vs our estimates
- Find where overhead is coming from

**Pros**: Might reveal the real limit
**Cons**: Requires reverse-engineering the bridge

### Solution 3: Use FM Only for Micro-Tasks

**Approach**: Per coding thoughts doc, break tasks into tiny micro-steps
- Each step fits in 200 chars
- Orchestrator manages state
- FM just executes one tiny action

**Pros**: Aligns with micro-task philosophy
**Cons**: Requires major refactoring

### Solution 4: Give Up on FM for TB

**Approach**: Use Claude Code or Ollama for TB tasks
- FM is too limited for real coding tasks
- Keep FM for simpler use cases

**Pros**: Pragmatic
**Cons**: Defeats the purpose of FM integration

## Recommendations

### Immediate Actions

1. **Reduce FM_CONTEXT_BUDGET to 200 chars** - Match reality
2. **Make system prompt even smaller** - Target 50-60 chars
3. **Single-turn only mode** - Don't try to maintain history
4. **Test with absolute minimum** - See if even 200 chars works

### Long-Term Actions

1. **Investigate FM bridge** - Understand actual limits
2. **Consider micro-task architecture** - Per coding thoughts doc
3. **Document FM limitations** - Set realistic expectations
4. **Consider alternative models** - For complex tasks

## Questions to Answer

1. **What is FM's actual limit?** - Need to test with progressively smaller requests
2. **Where is the overhead?** - HTTP? Bridge? Tokenization?
3. **Can FM do ANY multi-turn tasks?** - Or is it single-turn only?
4. **Is FM worth the effort?** - Given these limitations

## Next Steps

1. Test with 200 char budget
2. Test with 50 char system prompt
3. Test single-turn mode (no history)
4. If still failing, investigate FM bridge internals
5. Consider abandoning FM for TB if limits are insurmountable

---

**Status**: Critical - FM model runner is non-functional for Terminal-Bench tasks. Need to either fix the context limit issue or accept FM's limitations and redesign the approach.










