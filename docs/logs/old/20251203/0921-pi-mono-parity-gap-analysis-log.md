# 0921 Pi-Mono Parity Gap Analysis

Task: `oa-a4ed60` - Epic: Complete pi-mono parity (non-TUI)

## Summary

Analyzed the 6 child tasks under the pi-mono parity epic to determine what's already implemented vs what still needs work.

## Already Complete (can close)

### 1. Tools Manager Auto-Install (oa-daba8b) - DONE

**Evidence:**
- `src/tools/tools-manager.ts` - Full Effect-based implementation
- `src/tools/tools-manager.test.ts` - Comprehensive tests

**Features implemented:**
- Platform detection (darwin/linux/windows, x64/arm64)
- Cache directory under `~/.openagents/bin`
- `findInPath()` / `findInCache()` / `ensureTool()` / `getToolPath()`
- Auto-download from GitHub releases for rg (14.1.1) and fd (10.2.0)
- Tar/zip extraction per platform
- chmod +x on Unix

**Recommendation:** Close with reason "Already implemented in src/tools/tools-manager.ts"

### 2. Model Registry Generator (oa-9f0ac0) - DONE

**Evidence:**
- `scripts/generate-models.ts` - Full generation script
- `src/llm/models.generated.ts` - Generated output
- `src/llm/models.test.ts` - Tests

**Features implemented:**
- Fetches from models.dev API and OpenRouter API
- Normalizes to unified Model type
- Supports Anthropic, Google, OpenAI, Groq, Cerebras, xAI, zAI providers
- Includes cost, context window, max tokens, modalities
- Override mechanism for custom/missing models

**Recommendation:** Close with reason "Already implemented in scripts/generate-models.ts"

## Still Needed (remain open)

### 3. Context Compaction (oa-fb59bb) - NOT DONE

**Current state:**
- `src/cli/session-manager.ts` has basic JSONL persistence
- No intelligent trimming or compression of long conversation histories

**Gap:**
- Need to implement token counting for conversations
- Need compaction strategy (summarization? truncation? importance weighting?)
- Need backpressure mechanism to avoid context overflow

### 4. Session Import/Export (oa-e4dbfc) - NOT DONE

**Current state:**
- Session manager saves in JSONL format
- No import from pi-mono format

**Gap:**
- Need schema mapping between pi-mono and OpenAgents session formats
- Need roundtrip validation tests
- Need export utilities for debugging/sharing

### 5. Retry/Backoff (oa-e3ad31) - PARTIAL

**Current state:**
- `src/agent/orchestrator/orchestrator.ts:474` mentions retry but no exponential backoff
- Some basic error handling in provider code

**Gap:**
- Need standardized retry config across all providers
- Need exponential backoff for rate limits (429)
- Need configurable retry counts and delays
- Need distinct handling for auth errors (no retry) vs transient errors (retry)

### 6. Log Trimming (oa-60d305) - NOT DONE

**Current state:**
- Work logs in `docs/logs/YYYYMMDD/` are manual
- No automatic pruning or rotation

**Gap:**
- Need configurable max log size
- Need pruning rules for old sessions
- Need retention policy (keep errors? keep recent N?)

### 7. Status Stream (oa-5f16d2) - PARTIAL

**Current state:**
- `src/hud/protocol.ts` defines HUD message types
- `src/hud/client.ts` has WebSocket client

**Gap:**
- Need headless RPC mode for external supervisors
- Need schema documentation
- Need auth mechanism for remote monitoring

## Actions Taken

1. Closing oa-daba8b (tools-manager) - already complete
2. Closing oa-9f0ac0 (model registry) - already complete
3. Remaining 5 tasks stay open for future work

## Next Steps

The remaining P1 task is:
- **oa-fb59bb** (Context Compaction) - Critical for long-running MechaCoder sessions

The remaining P2 tasks can be prioritized based on reliability needs:
- oa-e3ad31 (Retry/Backoff) - Important for production stability
- oa-60d305 (Log Trimming) - Important for disk space
- oa-e4dbfc (Session Import/Export) - Nice to have for debugging
- oa-5f16d2 (Status Stream) - Nice to have for monitoring
