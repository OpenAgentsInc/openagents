# HillClimber Improvement Plan

## Executive Summary

The overnight run (202 runs, 0 passes) revealed several critical issues:
1. **Model quality** - `mistral-7b-instruct:free` is too small; reasoning models (`gpt-5`, `trinity-mini`) return empty content
2. **Historical context bug** - `buildMetaPrompt` never receives `history` parameter (line 250)
3. **Hint stagnation** - 51% of hints rejected as "too similar"; stuck in local minima
4. **Task difficulty** - TB2 medium/hard tasks may be too hard for current approach

## Key Issues Identified

### 1. Model Problems
- **`mistralai/mistral-7b-instruct:free`** - Works but too weak (7B params)
- **`openai/gpt-5`** - Returns empty content (reasoning-only model, 192 tokens generated, 0 returned)
- **`arcee-ai/trinity-mini:free`** - Returns empty content
- **`qwen/qwen3-4b:free`** - Returns empty content

### 2. Code Bug (meta-reasoner.ts:250)
```typescript
// CURRENT (broken):
const prompt = buildMetaPrompt(task, config, result);

// CORRECT (includes history):
const prompt = buildMetaPrompt(task, config, result, history);
```

### 3. Free Models Available (Better Options)
```
meta-llama/llama-3.3-70b-instruct:free    # Best choice - 70B, high quality
qwen/qwen3-235b-a22b:free                  # Huge model
mistralai/mistral-small-3.1-24b-instruct:free  # 24B Mistral
google/gemma-3-27b-it:free                 # 27B Google
google/gemini-2.0-flash-exp:free           # Gemini Flash
qwen/qwen3-coder:free                      # Coding-focused
amazon/nova-2-lite-v1:free                 # Amazon's new model
```

---

## Implementation Plan

### Step 1: Fix Model Selection (meta-reasoner.ts)
**File:** `src/hillclimber/meta-reasoner.ts`

Create a ranked model list with fallback:
```typescript
// Priority order (best first, all free)
const FREE_MODELS = [
  "meta-llama/llama-3.3-70b-instruct:free",
  "qwen/qwen3-235b-a22b:free",
  "mistralai/mistral-small-3.1-24b-instruct:free",
  "google/gemma-3-27b-it:free",
  "google/gemini-2.0-flash-exp:free",
  "mistralai/mistral-7b-instruct:free",  // Last resort
];

// Models that return empty responses - avoid these
const BLOCKLIST = [
  "arcee-ai/trinity-mini:free",
  "openai/gpt-5",
  "qwen/qwen3-4b:free",
];
```

Changes:
1. Remove `openrouter/auto` usage - directly specify model
2. Add retry logic with next model on empty response
3. Track model success rate in DB (optional)

### Step 2: Fix Historical Context Bug (meta-reasoner.ts)
**File:** `src/hillclimber/meta-reasoner.ts`

1. Fix line ~250 to pass history parameter
2. Add history query in `proposeConfigChange`:
```typescript
// Query recent runs for this task
const history = yield* getHistoricalContext(taskId);
const prompt = buildMetaPrompt(task, config, result, history);
```

3. Create `getHistoricalContext` function that queries store:
- Recent runs (last 10)
- Total runs/passes
- Best score and hint
- List of tried hints

### Step 3: Add Hint Diversity Enforcement (meta-reasoner.ts)
**File:** `src/hillclimber/meta-reasoner.ts`

Problem: 51% of hints rejected as "too similar" - optimization is stuck.

Solutions:
1. **Staleness counter**: If same hint for N runs (e.g., 5), force new approach
2. **Diversity flag**: After 3 "too similar" rejections, add instruction to prompt:
   "The current hint has been tried multiple times without success. Propose a COMPLETELY DIFFERENT approach."
3. **Random perturbation**: Every 10th run, add randomness to break local minima

### Step 4: Strip Model Artifacts (meta-reasoner.ts)
**File:** `src/hillclimber/meta-reasoner.ts`

Add preprocessing to handle model quirks:
```typescript
// Strip common model artifacts before parsing
let cleaned = content.trim();
cleaned = cleaned.replace(/^<s>\s*/, "");  // Mistral <s> token
cleaned = cleaned.replace(/^\[INST\].*?\[\/INST\]\s*/s, "");  // Instruction wrappers
```

### Step 5: Add Model Fallback on Empty Response (meta-reasoner.ts)
**File:** `src/hillclimber/meta-reasoner.ts`

When response is empty (reasoning model issue):
```typescript
if (content.length === 0 && response.usage?.completion_tokens > 0) {
  log(`[MetaReasoner] Model ${model} returned empty content, trying fallback...`);
  // Retry with next model in FREE_MODELS list
}
```

### Step 6: Add --model CLI Override (cli.ts)
**File:** `src/hillclimber/cli.ts`

Add `--model` flag for experimentation:
```typescript
// CLI option
model: {
  type: "string",
  short: "m",
  default: "",  // Empty = use default FREE_MODELS[0]
},

// Usage
bun run hillclimber --model meta-llama/llama-3.3-70b-instruct:free
bun run hillclimber --model qwen/qwen3-235b-a22b:free
```

Pass through to meta-reasoner via options object.

---

## Files to Modify

1. **`src/hillclimber/meta-reasoner.ts`** (primary)
   - Add FREE_MODELS constant
   - Add BLOCKLIST constant
   - Fix history parameter bug (line 250)
   - Add model fallback logic
   - Add artifact stripping
   - Add hint diversity enforcement

2. **`src/hillclimber/store.ts`** (if needed)
   - Add `getHistoricalContext` query method

3. **`src/hillclimber/types.ts`** (if needed)
   - Add model tracking types

4. **`src/hillclimber/cli.ts`**
   - Add `--model` override flag
   - Pass model to runner/meta-reasoner

5. **`src/hillclimber/runner.ts`**
   - Accept model option and pass to proposeConfigChange

---

## Priority Order

1. **P0 (Critical)**: Fix model selection - switch to llama-3.3-70b-instruct:free as default
2. **P0 (Critical)**: Add empty response fallback with model rotation
3. **P1 (High)**: Add --model CLI flag for experimentation
4. **P1 (High)**: Fix historical context bug (pass history to buildMetaPrompt)
5. **P1 (High)**: Add hint diversity enforcement
6. **P2 (Medium)**: Strip model artifacts (<s> tokens, etc.)

---

## Expected Outcomes

After these changes:
- Better model = smarter hint generation
- Historical context = informed decisions, avoid repeated failures
- Diversity enforcement = escape local minima
- Fallback logic = no wasted runs on empty responses

This should dramatically improve the chances of getting actual passes in round two.
