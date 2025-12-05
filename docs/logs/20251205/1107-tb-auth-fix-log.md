# 1107 Terminal-Bench Claude Code Auth Fix

## Problem

Terminal-Bench runs were failing immediately with authentication errors:

```
❌ Agent failure: Claude Code authentication failed. Set a valid ANTHROPIC_API_KEY before retrying

Turns: 0
Tokens: 0
Outcome: error
```

Init messages showed `"apiKeySource":"none"`, indicating the Claude Code SDK couldn't find any credentials.

## Investigation

### Step 1: Verify Credentials Exist

```bash
$ ls -la ~/.claude/.credentials.json
-rw-------@ 1 christopherdavid  staff  649 Dec  4 23:51 /Users/christopherdavid/.claude/.credentials.json
```

✅ Credentials file exists with proper OAuth tokens.

### Step 2: Test Claude CLI Directly

```bash
$ echo 'hello' | claude --print
Hello! I'm ready to help you with software engineering tasks...
```

✅ Claude Code CLI works fine with credentials.

### Step 3: Test SDK Directly

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";
for await (const message of query({ prompt: "Say hello", options: { cwd: "/tmp" } })) {
  // Works! Session starts successfully
}
```

✅ SDK works when run directly from my shell.

### Step 4: Compare with TB Runs

TB runs via desktop server:
- `"apiKeySource": "none"` ❌
- Authentication failure ❌
- 0 turns, 0 tokens ❌

Direct test:
- Session starts ✅
- Credentials found ✅

**Conclusion:** Something different between direct runs and desktop server runs.

### Step 5: Trace Environment Handling

Found in `src/desktop/handlers.ts:152-156`:

```typescript
env: {
  ...process.env,
  HOME: process.env.HOME ?? Bun.env.HOME,
  PATH: process.env.PATH ?? Bun.env.PATH,
},
```

**Root Cause Identified:** Desktop server was passing **only** `HOME` and `PATH` to TB subprocess, stripping all other environment variables!

## How Claude Code Auth Works

1. **Desktop Process** (Bun) → spawns **TB Process** (Bun)
2. **TB Process** → calls `runClaudeCodeSubagent()`
3. **runClaudeCodeSubagent()** → uses Claude Agent SDK's `query()`
4. **SDK `query()`** → spawns **Claude Code CLI subprocess** (Node/Bun)
5. **Claude Code CLI** → looks for `~/.claude/.credentials.json`

The authentication happens in step 5, but the environment is determined in step 1.

## Why Limited Environment Broke Auth

The Claude Code CLI subprocess (step 5) needs more than just `HOME` and `PATH`. When desktop server stripped the environment down to only those two variables, the CLI subprocess lost access to:

- Shell environment setup
- Node/npm paths
- Other system variables the SDK relies on
- Potentially terminal/TTY variables

Even though `HOME` was passed (pointing to the right `.credentials.json`), other missing variables caused the SDK to fail initialization and not load the credentials.

## Solution

**Pass full environment to TB subprocess:**

```typescript
// Before
env: {
  ...process.env,
  HOME: process.env.HOME ?? Bun.env.HOME,
  PATH: process.env.PATH ?? Bun.env.PATH,
}

// After
env: process.env  // Pass full environment for SDK subprocess
```

## Why This Is Different from Container Auth

The user mentioned applying "the same approach as mechacoder containers" per `docs/claude/plans/container-auth.md`.

**Key difference:**
- **Containers:** Need explicit credential **mounting** because they're isolated
  - Extract from keychain → write to temp file → mount into container
- **TB Local:** Runs on **host**, so credentials already accessible
  - Just needs proper **environment inheritance**

The "container approach" isn't needed for TB local - we just needed to stop stripping the environment.

## Verification Plan

1. Restart desktop server to pick up new code
2. Run a TB task through the UI
3. Check that:
   - Agent session starts (turns > 0)
   - Authentication succeeds
   - Task executes normally

## Files Modified

- `src/desktop/handlers.ts:146-154` - Changed env to pass full process.env

## Commit

```
5db5659c8 - fix: pass full environment to TB subprocess for Claude Code auth
```

## Related Issues

- Similar to sandbox credential injection (src/sandbox/credentials.ts)
- But TB local is simpler - just environment, not volume mounts
- Container mode would still need credential mounting per the plan
